//! Single-pass multi-theme HTML through inherited CSS custom properties.

use std::collections::HashSet;
use std::fmt::{self, Write};

use super::{
    HtmlOptions, RenderedOutput, escape_html_attribute, escape_html_text, modifier_bits,
    write_css_modifiers, write_error, write_html_end, write_html_hex_byte,
};
use crate::theme::FontModifiers;
use crate::{Error, Result, Style, Theme, TokenizedDocument};

/// Renders one tokenized document as inline HTML for several themes at once.
///
/// The first theme renders exactly like [`render_html`](crate::render_html)
/// inline output, so the HTML works without a stylesheet. Every other theme
/// `name` adds inherited CSS custom properties with that theme's exactly
/// resolved styles:
///
/// | Property | Value |
/// | --- | --- |
/// | `--sm-{name}` | foreground color |
/// | `--sm-{name}-bg` | background color |
/// | `--sm-{name}-fw` | `font-weight` |
/// | `--sm-{name}-fs` | `font-style` |
/// | `--sm-{name}-td` | `text-decoration` |
///
/// Switch to a theme by applying its properties to the wrapper and its spans,
/// for example in a `prefers-color-scheme` media query; the
/// [rendering guide](https://github.com/phongndo/syntaxmate/blob/main/docs/rendering.md#multiple-themes)
/// shows the rule. No second render is needed. With the wrapper, runs omit
/// values equal to that theme's defaults and inherit them from `<pre>`;
/// without it, every run carries every value.
///
/// Theme names must be non-empty, unique, and contain only ASCII letters,
/// digits, `-`, or `_`, and their properties must not collide (`dark` and
/// `dark-bg` would both write `--sm-dark-bg`). Class mode is not supported: `class_prefix` must be
/// `None`. Invalid themes or options return [`Error::Render`] with
/// [`RenderErrorKind::InvalidOptions`](crate::RenderErrorKind::InvalidOptions).
/// Pass the source that produced `document`, as for `render_html`.
///
/// ```
/// use syntaxmate::{Highlighter, HtmlOptions, Theme, render_html_themes};
///
/// let source = "fn main() {}";
/// let highlighter = Highlighter::bundled()?;
/// let document = highlighter.tokenize("rust", source)?;
/// let light = Theme::bundled("github-light")?;
/// let dark = Theme::bundled("github-dark")?;
/// let html = render_html_themes(
///     source,
///     &document,
///     &[("light", &light), ("dark", &dark)],
///     &HtmlOptions::default(),
/// )?;
/// assert!(html.as_str().contains("--sm-dark:#"));
/// # Ok::<(), syntaxmate::Error>(())
/// ```
pub fn render_html_themes(
    source: &str,
    document: &TokenizedDocument,
    themes: &[(&str, &Theme)],
    options: &HtmlOptions,
) -> Result<RenderedOutput> {
    validate_themes(themes, options)?;
    super::validate_lines(
        source,
        document
            .lines()
            .iter()
            .map(|line| line.tokens().iter().map(crate::Token::range)),
    )?;
    let defaults = themes
        .iter()
        .map(|(_, theme)| theme.default_style())
        .collect::<Vec<_>>();
    let mut output = String::with_capacity(source.len().saturating_mul(3));
    write_start(themes, &defaults, options, &mut output).map_err(write_error)?;
    let mut styles = Vec::with_capacity(themes.len());
    for (chunk, line) in crate::engine::line::LineChunks::new(source).zip(document.lines()) {
        let text = chunk.text;
        let mut cursor = 0;
        let mut active: Option<Run> = None;
        for token in line.tokens() {
            let range = token.range();
            if range.is_empty() {
                continue;
            }
            if cursor < range.start {
                if active.take().is_some() {
                    output.push_str("</span>");
                }
                escape_html_text(&text[cursor..range.start], &mut output).map_err(write_error)?;
            }
            styles.clear();
            for ((_, theme), default) in themes.iter().zip(&defaults) {
                let mut style = theme.resolve(token.scopes());
                if options.include_wrapper {
                    if style.foreground == default.foreground {
                        style.foreground = None;
                    }
                    if style.background == default.background {
                        style.background = None;
                    }
                }
                styles.push(style);
            }
            let scopes = options
                .include_scopes
                .then(|| scope_attribute(token.scopes()))
                .transpose()
                .map_err(write_error)?;
            let next = (styles.iter().any(|style| *style != Style::default()) || scopes.is_some())
                .then(|| Run {
                    styles: styles.clone(),
                    scopes,
                });
            if next != active {
                if active.is_some() {
                    output.push_str("</span>");
                }
                if let Some(run) = &next {
                    write_run_start(themes, run, options.include_wrapper, &mut output)
                        .map_err(write_error)?;
                }
                active = next;
            }
            escape_html_text(&text[range.clone()], &mut output).map_err(write_error)?;
            cursor = range.end;
        }
        if active.is_some() {
            output.push_str("</span>");
        }
        escape_html_text(&text[cursor..], &mut output).map_err(write_error)?;
        output.push_str(chunk.terminator);
    }
    write_html_end(options, &mut output).map_err(write_error)?;
    Ok(RenderedOutput {
        content: output,
        status: document.status(),
    })
}

/// One output run: per-theme styles and an optional scope attribute.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Run {
    styles: Vec<Style>,
    scopes: Option<String>,
}

fn validate_themes(themes: &[(&str, &Theme)], options: &HtmlOptions) -> Result<()> {
    let invalid =
        |message: String| Err(Error::Render(crate::RenderError::invalid_options(message)));
    if themes.is_empty() {
        return invalid("at least one theme is required".to_owned());
    }
    if options.class_prefix.is_some() {
        return invalid("multi-theme output does not support class_prefix".to_owned());
    }
    let mut names = HashSet::new();
    for (name, _) in themes {
        let valid = !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
        if !valid {
            return invalid(format!(
                "theme name {name:?} must be non-empty ASCII letters, digits, '-', or '_'"
            ));
        }
        if !names.insert(*name) {
            return invalid(format!("duplicate theme name {name:?}"));
        }
    }
    // Names such as `dark` and `dark-bg` would write the same property.
    let mut properties = HashSet::new();
    for (name, _) in &themes[1..] {
        for suffix in ["", "-bg", "-fw", "-fs", "-td"] {
            if !properties.insert(format!("{name}{suffix}")) {
                return invalid(format!(
                    "theme name {name:?} collides with another theme's CSS properties"
                ));
            }
        }
    }
    Ok(())
}

fn write_start(
    themes: &[(&str, &Theme)],
    defaults: &[Style],
    options: &HtmlOptions,
    output: &mut dyn Write,
) -> fmt::Result {
    if !options.include_wrapper {
        return Ok(());
    }
    output.write_str("<pre")?;
    if let Some(class) = &options.class {
        output.write_str(" class=\"")?;
        escape_html_attribute(class, output)?;
        output.write_char('"')?;
    }
    output.write_str(" style=\"")?;
    // Decorations on an ancestor cannot be cleared by a descendant token, so
    // the wrapper carries colors and resets every theme's font properties.
    let first = defaults[0];
    write_colors(first.foreground, first.background, output)?;
    for ((name, _), default) in themes.iter().zip(defaults).skip(1) {
        write_variables(name, default.foreground, default.background, output)?;
        write_modifier_variables(name, FontModifiers::empty(), output)?;
    }
    output.write_str("\"><code>")
}

fn write_run_start(
    themes: &[(&str, &Theme)],
    run: &Run,
    include_wrapper: bool,
    output: &mut dyn Write,
) -> fmt::Result {
    output.write_str("<span")?;
    let first = run.styles[0];
    let mut style = String::new();
    write_colors(first.foreground, first.background, &mut style)?;
    write_css_modifiers(modifier_bits(first.modifiers), &mut style)?;
    for ((name, _), theme_style) in themes.iter().zip(&run.styles).skip(1) {
        write_variables(
            name,
            theme_style.foreground,
            theme_style.background,
            &mut style,
        )?;
        // With a wrapper, runs inherit the wrapper's reset font properties.
        if !include_wrapper || !theme_style.modifiers.is_empty() {
            write_modifier_variables(name, theme_style.modifiers, &mut style)?;
        }
    }
    if !style.is_empty() {
        write!(output, " style=\"{style}\"")?;
    }
    if let Some(scopes) = &run.scopes {
        write!(output, " data-scopes=\"{scopes}\"")?;
    }
    output.write_char('>')
}

fn write_colors(
    foreground: Option<crate::RgbColor>,
    background: Option<crate::RgbColor>,
    output: &mut dyn Write,
) -> fmt::Result {
    for (property, color) in [("color", foreground), ("background-color", background)] {
        if let Some(color) = color {
            write!(output, "{property}:#")?;
            write_hex(color, output)?;
            output.write_char(';')?;
        }
    }
    Ok(())
}

fn write_variables(
    name: &str,
    foreground: Option<crate::RgbColor>,
    background: Option<crate::RgbColor>,
    output: &mut dyn Write,
) -> fmt::Result {
    for (suffix, color) in [("", foreground), ("-bg", background)] {
        if let Some(color) = color {
            write!(output, "--sm-{name}{suffix}:#")?;
            write_hex(color, output)?;
            output.write_char(';')?;
        }
    }
    Ok(())
}

fn write_modifier_variables(
    name: &str,
    modifiers: FontModifiers,
    output: &mut dyn Write,
) -> fmt::Result {
    let bits = modifier_bits(modifiers);
    write!(
        output,
        "--sm-{name}-fw:{};--sm-{name}-fs:{};--sm-{name}-td:{};",
        if bits & 1 != 0 { "bold" } else { "normal" },
        if bits & 2 != 0 { "italic" } else { "normal" },
        match bits & 12 {
            4 => "underline",
            8 => "line-through",
            12 => "underline line-through",
            _ => "none",
        }
    )
}

fn write_hex(color: crate::RgbColor, output: &mut dyn Write) -> fmt::Result {
    write_html_hex_byte(color.red, output)?;
    write_html_hex_byte(color.green, output)?;
    write_html_hex_byte(color.blue, output)
}

fn scope_attribute<'a>(
    scopes: impl Iterator<Item = &'a str>,
) -> std::result::Result<String, fmt::Error> {
    let mut attribute = String::new();
    for (index, scope) in scopes.enumerate() {
        if index != 0 {
            attribute.push(' ');
        }
        escape_html_attribute(scope, &mut attribute)?;
    }
    Ok(attribute)
}

#[cfg(all(test, feature = "bundled-grammars", feature = "bundled-themes"))]
mod tests;
