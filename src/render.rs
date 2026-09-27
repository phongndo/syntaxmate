//! Safe, dependency-free HTML and ANSI rendering for highlighted documents.

#[cfg(any(feature = "ansi", feature = "html"))]
use std::{
    fmt::{self, Write},
    ops::Range,
};

use crate::HighlightStatus;
#[cfg(all(
    feature = "bundled-grammars",
    feature = "bundled-themes",
    any(feature = "ansi", feature = "html")
))]
use crate::HighlightedText;
#[cfg(feature = "html")]
use crate::Theme;
#[cfg(feature = "ansi")]
use crate::theme::RgbColor;
#[cfg(any(feature = "ansi", feature = "html"))]
use crate::{Error, HighlightedDocument, Result, Style, theme::FontModifiers};

/// A rendered string together with the tokenizer completion status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedOutput {
    content: String,
    status: HighlightStatus,
}

impl RenderedOutput {
    /// Returns the rendered text.
    pub fn as_str(&self) -> &str {
        &self.content
    }

    /// Consumes the result and returns the rendered text.
    pub fn into_string(self) -> String {
        self.content
    }

    /// Reports whether tokenization completed without exhausting a safety budget.
    pub fn status(&self) -> HighlightStatus {
        self.status
    }
}

/// Options for [`render_html`].
#[cfg(feature = "html")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlOptions {
    /// Wrap output in `<pre><code>...</code></pre>`, carrying default colors on `<pre>`.
    /// Without a wrapper, inline spans retain full colors; class mode needs a
    /// container with the wrapper class described by [`html_stylesheet`].
    pub include_wrapper: bool,
    /// Class placed on the `<pre>` wrapper. Ignored without a wrapper.
    pub class: Option<String>,
    /// Add a `data-scopes` attribute containing the exact TextMate scope stack.
    pub include_scopes: bool,
    /// Emit nested, theme-independent scope classes instead of inline styles.
    /// Pass the same prefix to [`html_stylesheet`]; prefixes and scopes are encoded
    /// safely. Switch themes by replacing the stylesheet without rendering again.
    pub class_prefix: Option<String>,
}

#[cfg(feature = "html")]
impl Default for HtmlOptions {
    fn default() -> Self {
        Self {
            include_wrapper: true,
            class: Some("syntaxmate".to_owned()),
            include_scopes: false,
            class_prefix: None,
        }
    }
}

/// Renders highlighted source as escaped HTML.
///
/// Source text, wrapper classes, and optional scope attributes are escaped.
/// Pass the source that produced `document`. Invalid ranges and mismatched
/// logical line counts return an error, but different text with compatible
/// ranges is not detected; the document does not retain the original source.
/// In inline mode, default colors are inherited from the wrapper; without it,
/// runs retain full colors. Adjacent runs with identical output merge within
/// each logical line. Class mode is described by [`html_stylesheet`].
/// Font modifiers stay on runs so tokens can clear default decorations.
/// See [`render_html_to`] to write output without allocating a complete string.
#[cfg(feature = "html")]
pub fn render_html(
    source: &str,
    document: &HighlightedDocument,
    options: &HtmlOptions,
) -> Result<RenderedOutput> {
    let mut output = String::with_capacity(source.len().saturating_mul(2));
    let status = render_html_to(source, document, options, &mut output)?;
    Ok(RenderedOutput {
        content: output,
        status,
    })
}

/// Writes escaped HTML to a caller-provided writer and returns tokenization status.
///
/// Uses the same options and source validation as [`render_html`]. Validation
/// finishes before writing anything. Writer failures return [`Error::Render`]
/// and may leave partial output; the renderer does not flush or roll it back.
#[cfg(feature = "html")]
pub fn render_html_to(
    source: &str,
    document: &HighlightedDocument,
    options: &HtmlOptions,
    output: &mut dyn Write,
) -> Result<HighlightStatus> {
    validate_document(source, document)?;
    let prefix = options.class_prefix.as_deref().map(encode_class_prefix);
    write_html_start(document.default_style, options, prefix.as_deref(), output)
        .map_err(write_error)?;
    for (index, (chunk, line)) in crate::engine::line::LineChunks::new(source)
        .zip(document.lines())
        .enumerate()
    {
        if index != 0 {
            output.write_char('\n').map_err(write_error)?;
        }
        let spans = line
            .tokens()
            .iter()
            .map(|span| (span.range(), span.style(), span.scopes()));
        render_html_line(
            chunk.text,
            spans,
            document.default_style,
            options,
            prefix.as_deref(),
            output,
        )
        .map_err(write_error)?;
    }
    write_html_end(options, output).map_err(write_error)?;
    Ok(document.status())
}

/// Renders compact scope-stack tokens without an owned styled-document intermediate.
#[cfg(all(
    feature = "html",
    feature = "bundled-grammars",
    feature = "bundled-themes"
))]
pub(crate) fn render_html_compact(
    source: &str,
    tokens: &HighlightedText,
    status: HighlightStatus,
    theme: &Theme,
    options: &HtmlOptions,
) -> Result<RenderedOutput> {
    let mut output = String::with_capacity(source.len().saturating_mul(2));
    let defaults = theme.resolve_scope_names(&[]);
    let prefix = options.class_prefix.as_deref().map(encode_class_prefix);
    write_html_start(defaults, options, prefix.as_deref(), &mut output).map_err(write_error)?;
    let mut chunks = crate::engine::line::LineChunks::new(source);
    for (index, line) in tokens.lines.iter().enumerate() {
        let chunk = chunks.next().ok_or_else(compact_line_count_error)?;
        if index != 0 {
            output.push('\n');
        }
        let spans = line.segments.iter().map(|span| {
            (
                span.byte_start..span.byte_end,
                theme.resolve_interned(&line.scope_table, span.scope_stack),
                line.scope_table.stack_names(span.scope_stack),
            )
        });
        render_html_line(
            chunk.text,
            spans,
            defaults,
            options,
            prefix.as_deref(),
            &mut output,
        )
        .map_err(write_error)?;
    }
    if chunks.next().is_some() {
        return Err(compact_line_count_error());
    }
    write_html_end(options, &mut output).map_err(write_error)?;
    Ok(RenderedOutput {
        content: output,
        status,
    })
}

#[cfg(feature = "html")]
fn write_html_start(
    defaults: Style,
    options: &HtmlOptions,
    prefix: Option<&str>,
    output: &mut dyn Write,
) -> fmt::Result {
    if !options.include_wrapper {
        return Ok(());
    }
    output.write_str("<pre")?;
    // Decorations on an ancestor cannot be cleared by a descendant token.
    let colors = Style {
        modifiers: FontModifiers::empty(),
        ..defaults
    };
    if options.class.is_some() || prefix.is_some() {
        output.write_str(" class=\"")?;
        if let Some(class) = &options.class {
            escape_html_attribute(class, output)?;
        }
        if let Some(prefix) = prefix {
            if options
                .class
                .as_ref()
                .is_some_and(|class| !class.is_empty())
            {
                output.write_char(' ')?;
            }
            write!(output, "{prefix}-root")?;
        }
        output.write_char('"')?;
    }
    if prefix.is_none() {
        write_html_style(colors, output)?;
    }
    output.write_str("><code>")
}

#[cfg(feature = "html")]
fn write_html_end(options: &HtmlOptions, output: &mut dyn Write) -> fmt::Result {
    if options.include_wrapper {
        output.write_str("</code></pre>")?;
    }
    Ok(())
}

#[cfg(feature = "html")]
fn render_html_line<'a>(
    text: &str,
    spans: impl Iterator<Item = (Range<usize>, Style, impl Iterator<Item = &'a str>)>,
    defaults: Style,
    options: &HtmlOptions,
    prefix: Option<&str>,
    output: &mut dyn Write,
) -> fmt::Result {
    if let Some(prefix) = prefix {
        return render_scope_line(text, spans, options, prefix, output);
    }
    let mut cursor = 0;
    let mut active: Option<(Style, Option<String>)> = None;
    for (range, mut style, scopes) in spans {
        if range.is_empty() {
            continue;
        }
        if cursor < range.start {
            if active.take().is_some() {
                output.write_str("</span>")?;
            }
            escape_html_text(&text[cursor..range.start], output)?;
        }
        if options.include_wrapper {
            if style.foreground == defaults.foreground {
                style.foreground = None;
            }
            if style.background == defaults.background {
                style.background = None;
            }
        }
        let scopes = if options.include_scopes {
            let mut attribute = String::new();
            for (index, scope) in scopes.enumerate() {
                if index != 0 {
                    attribute.push(' ');
                }
                escape_html_attribute(scope, &mut attribute)?;
            }
            Some(attribute)
        } else {
            None
        };
        let next = (style != Style::default() || scopes.is_some()).then_some((style, scopes));
        if next != active {
            if active.is_some() {
                output.write_str("</span>")?;
            }
            if let Some((style, scopes)) = &next {
                output.write_str("<span")?;
                write_html_style(*style, output)?;
                if let Some(scopes) = scopes {
                    write!(output, " data-scopes=\"{scopes}\"")?;
                }
                output.write_char('>')?;
            }
            active = next;
        }
        escape_html_text(&text[range.clone()], output)?;
        cursor = range.end;
    }
    if active.is_some() {
        output.write_str("</span>")?;
    }
    escape_html_text(&text[cursor..], output)
}

#[cfg(feature = "html")]
fn render_scope_line<'a>(
    text: &str,
    spans: impl Iterator<Item = (Range<usize>, Style, impl Iterator<Item = &'a str>)>,
    options: &HtmlOptions,
    prefix: &str,
    output: &mut dyn Write,
) -> fmt::Result {
    let mut active = Vec::new();
    let mut leaf_open = false;
    let mut cursor = 0;
    for (range, _, scopes) in spans {
        if range.is_empty() {
            continue;
        }
        let scopes = scopes.collect::<Vec<_>>();
        if cursor < range.start {
            close_scope_spans(&mut active, &mut leaf_open, 0, output)?;
            write_scope_text(&text[cursor..range.start], output)?;
        }
        let shared = active
            .iter()
            .zip(&scopes)
            .take_while(|(a, b)| a == b)
            .count();
        if shared != active.len() || shared != scopes.len() || !leaf_open {
            close_scope_spans(&mut active, &mut leaf_open, shared, output)?;
            for scope in &scopes[shared..] {
                write!(
                    output,
                    "<span class=\"{prefix}-s-{}\">",
                    encode_scope(scope)
                )?;
                active.push(*scope);
            }
            output.write_str("<span")?;
            if options.include_scopes {
                output.write_str(" data-scopes=\"")?;
                for (index, scope) in scopes.iter().enumerate() {
                    if index != 0 {
                        output.write_char(' ')?;
                    }
                    escape_html_attribute(scope, output)?;
                }
                output.write_char('"')?;
            }
            output.write_char('>')?;
            leaf_open = true;
        }
        escape_html_text(&text[range.clone()], output)?;
        cursor = range.end;
    }
    close_scope_spans(&mut active, &mut leaf_open, 0, output)?;
    if cursor < text.len() {
        write_scope_text(&text[cursor..], output)?;
    }
    Ok(())
}

#[cfg(feature = "html")]
fn close_scope_spans(
    active: &mut Vec<&str>,
    leaf_open: &mut bool,
    shared: usize,
    output: &mut dyn Write,
) -> fmt::Result {
    if *leaf_open {
        output.write_str("</span>")?;
        *leaf_open = false;
    }
    for _ in shared..active.len() {
        output.write_str("</span>")?;
    }
    active.truncate(shared);
    Ok(())
}

#[cfg(feature = "html")]
fn write_scope_text(text: &str, output: &mut dyn Write) -> fmt::Result {
    output.write_str("<span>")?;
    escape_html_text(text, output)?;
    output.write_str("</span>")
}

/// Generates a swappable theme stylesheet for [`HtmlOptions::class_prefix`].
///
/// Only the prefix must match the rendered HTML; the theme can be changed freely.
/// The wrapper class is `sm-{encoded_prefix}-root`. Supply that class on your own
/// container when `include_wrapper` is false. Prefix bytes except ASCII letters,
/// digits, and hyphens become `_xx` hex escapes (including underscores).
/// Each scope level has one class, `sm-{encoded_prefix}-s-{encoded_scope}`:
/// dots become hyphens; other non-alphanumeric scope bytes become `_xx` escapes.
/// CSS `|=` attribute selectors preserve ordered, dotted scope-prefix matching.
/// Do not add other classes to the generated scope spans.
///
/// Parent selectors become descendants; comma lists become separate equivalent
/// rules. Exclusions (`-` or `-scope`), child combinators (`>`), wildcards, and priority
/// prefixes (`L:`/`R:`) are skipped. Other syntax rejected by [`Theme::from_json`]
/// cannot reach this renderer. Rules use zero-specificity `:where()` selectors
/// ordered by TextMate target depth, nearest-first parent lengths/count, then
/// source order. This approximates TextMate through CSS inheritance and cascade;
/// unsupported selectors and surrounding page CSS can differ from inline output.
/// Font decorations use inherited custom properties applied only to text spans,
/// allowing inner scopes to clear them. Requires CSS custom properties and `:where()`.
#[cfg(feature = "html")]
pub fn html_stylesheet(theme: &Theme, class_prefix: &str) -> String {
    let prefix = encode_class_prefix(class_prefix);
    let mut output = String::new();
    write!(output, ":where(.{prefix}-root){{").unwrap();
    write_scope_declarations(theme.default_style(), true, &prefix, &mut output).unwrap();
    output.push_str("}\n");
    writeln!(output, ":where(.{prefix}-root) span:not([class]){{font-weight:var(--{prefix}-weight);font-style:var(--{prefix}-style);text-decoration:var(--{prefix}-decoration);}}").unwrap();
    for (selector, style, modifiers_set) in theme.rendering_rules() {
        let Some(scopes) = css_scope_selector(selector) else {
            continue;
        };
        write!(output, ":where(.{prefix}-root").unwrap();
        for scope in scopes {
            write!(output, " [class|=\"{prefix}-s-{}\"]", encode_scope(scope)).unwrap();
        }
        output.push_str("){");
        write_scope_declarations(style, modifiers_set, &prefix, &mut output).unwrap();
        output.push_str("}\n");
    }
    output
}

#[cfg(feature = "html")]
fn css_scope_selector(selector: &str) -> Option<Vec<&str>> {
    let scopes = selector.split_whitespace().collect::<Vec<_>>();
    (!scopes.is_empty()
        && scopes.iter().all(|scope| {
            *scope != ">"
                && !scope.starts_with('-')
                && !scope.contains('*')
                && !scope.starts_with("L:")
                && !scope.starts_with("R:")
        }))
    .then_some(scopes)
}

#[cfg(feature = "html")]
fn encode_scope(scope: &str) -> String {
    let mut encoded = String::new();
    for byte in scope.bytes() {
        match byte {
            b'.' => encoded.push('-'),
            byte if byte.is_ascii_alphanumeric() => encoded.push(char::from(byte)),
            byte => write!(encoded, "_{byte:02x}").unwrap(),
        }
    }
    encoded
}

#[cfg(feature = "html")]
fn write_scope_declarations(
    style: Style,
    modifiers_set: bool,
    prefix: &str,
    output: &mut dyn Write,
) -> fmt::Result {
    for (property, color) in [
        ("color", style.foreground),
        ("background-color", style.background),
    ] {
        if let Some(color) = color {
            write!(
                output,
                "{property}:#{:02x}{:02x}{:02x};",
                color.red, color.green, color.blue
            )?;
        }
    }
    if modifiers_set {
        let bits = modifier_bits(style.modifiers);
        write!(
            output,
            "--{prefix}-weight:{};--{prefix}-style:{};--{prefix}-decoration:{};",
            if bits & 1 != 0 { "bold" } else { "normal" },
            if bits & 2 != 0 { "italic" } else { "normal" },
            match bits & 12 {
                4 => "underline",
                8 => "line-through",
                12 => "underline line-through",
                _ => "none",
            }
        )?;
    }
    Ok(())
}

#[cfg(feature = "html")]
fn encode_class_prefix(prefix: &str) -> String {
    let mut encoded = String::from("sm-");
    for byte in prefix.bytes() {
        if byte.is_ascii_alphanumeric() || byte == b'-' {
            encoded.push(char::from(byte));
        } else {
            write!(encoded, "_{byte:02x}").expect("writing to a String cannot fail");
        }
    }
    encoded
}

#[cfg(feature = "html")]
fn modifier_bits(modifiers: FontModifiers) -> u8 {
    [
        FontModifiers::BOLD,
        FontModifiers::ITALIC,
        FontModifiers::UNDERLINED,
        FontModifiers::CROSSED_OUT,
    ]
    .into_iter()
    .enumerate()
    .fold(0, |bits, (index, flag)| {
        bits | (u8::from(modifiers.contains(flag)) << index)
    })
}

#[cfg(feature = "html")]
fn write_css_modifiers(bits: u8, output: &mut dyn Write) -> fmt::Result {
    if bits & 1 != 0 {
        output.write_str("font-weight:bold;")?;
    }
    if bits & 2 != 0 {
        output.write_str("font-style:italic;")?;
    }
    if bits & 12 != 0 {
        output.write_str("text-decoration:")?;
        if bits & 4 != 0 {
            output.write_str("underline")?;
        }
        if bits & 8 != 0 {
            if bits & 4 != 0 {
                output.write_char(' ')?;
            }
            output.write_str("line-through")?;
        }
        output.write_char(';')?;
    }
    Ok(())
}

#[cfg(feature = "html")]
fn write_html_style(style: Style, output: &mut dyn Write) -> fmt::Result {
    if style == Style::default() {
        return Ok(());
    }
    output.write_str(" style=\"")?;
    if let Some(color) = style.foreground {
        output.write_str("color:#")?;
        write_html_hex_byte(color.red, output)?;
        write_html_hex_byte(color.green, output)?;
        write_html_hex_byte(color.blue, output)?;
        output.write_char(';')?;
    }
    if let Some(color) = style.background {
        output.write_str("background-color:#")?;
        write_html_hex_byte(color.red, output)?;
        write_html_hex_byte(color.green, output)?;
        write_html_hex_byte(color.blue, output)?;
        output.write_char(';')?;
    }
    write_css_modifiers(modifier_bits(style.modifiers), output)?;
    output.write_char('"')?;
    Ok(())
}

#[cfg(feature = "html")]
fn write_html_hex_byte(byte: u8, output: &mut dyn Write) -> fmt::Result {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    output.write_char(char::from(HEX[(byte >> 4) as usize]))?;
    output.write_char(char::from(HEX[(byte & 0x0f) as usize]))?;
    Ok(())
}

#[cfg(feature = "html")]
fn escape_html_text(text: &str, output: &mut dyn Write) -> fmt::Result {
    for character in text.chars() {
        match character {
            '&' => output.write_str("&amp;")?,
            '<' => output.write_str("&lt;")?,
            '>' => output.write_str("&gt;")?,
            '"' => output.write_str("&quot;")?,
            '\'' => output.write_str("&#39;")?,
            character => output.write_char(character)?,
        }
    }
    Ok(())
}

#[cfg(feature = "html")]
fn escape_html_attribute(text: &str, output: &mut dyn Write) -> fmt::Result {
    for character in text.chars() {
        match character {
            '&' => output.write_str("&amp;")?,
            '<' => output.write_str("&lt;")?,
            '>' => output.write_str("&gt;")?,
            '"' => output.write_str("&quot;")?,
            '\'' => output.write_str("&#39;")?,
            '\0' => output.write_char('\u{fffd}')?,
            character if character.is_control() => {
                write!(output, "&#x{:x};", u32::from(character))?;
            }
            character => output.write_char(character)?,
        }
    }
    Ok(())
}

/// Options for [`render_ansi`].
#[cfg(feature = "ansi")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnsiOptions {
    /// Emit 24-bit foreground, background, and modifier SGR sequences.
    pub colors: bool,
    /// Replace source C0/C1 control characters with visible Unicode control pictures.
    /// Newlines inserted between logical lines and horizontal tabs are preserved.
    pub sanitize_control_characters: bool,
    /// Paint the theme default background, instead of preserving the terminal background.
    /// Explicit token backgrounds differing from the default are always emitted with colors.
    pub include_default_background: bool,
}

#[cfg(feature = "ansi")]
impl Default for AnsiOptions {
    fn default() -> Self {
        Self {
            colors: true,
            sanitize_control_characters: true,
            include_default_background: false,
        }
    }
}

/// Renders highlighted source using 24-bit ANSI SGR sequences.
///
/// Control-character sanitization is enabled by default so untrusted source
/// cannot inject terminal escape sequences. Disable it only for trusted input.
///
/// Pass the source that produced `document`. Validation checks ranges and
/// line counts, not source identity. See [`render_ansi_to`] for streaming output.
#[cfg(feature = "ansi")]
pub fn render_ansi(
    source: &str,
    document: &HighlightedDocument,
    options: &AnsiOptions,
) -> Result<RenderedOutput> {
    let mut output = String::with_capacity(source.len().saturating_mul(2));
    let status = render_ansi_to(source, document, options, &mut output)?;
    Ok(RenderedOutput {
        content: output,
        status,
    })
}

/// Writes ANSI output to a caller-provided writer and returns tokenization status.
///
/// Uses the same options, sanitization, and source validation as [`render_ansi`].
/// Validation finishes before writing anything. Writer failures return
/// [`Error::Render`] and may leave partial output (including an active SGR style);
/// the renderer does not flush or roll it back.
#[cfg(feature = "ansi")]
pub fn render_ansi_to(
    source: &str,
    document: &HighlightedDocument,
    options: &AnsiOptions,
    output: &mut dyn Write,
) -> Result<HighlightStatus> {
    validate_document(source, document)?;
    for (index, (chunk, line)) in crate::engine::line::LineChunks::new(source)
        .zip(document.lines())
        .enumerate()
    {
        if index != 0 {
            output.write_char('\n').map_err(write_error)?;
        }
        let spans = line
            .tokens()
            .iter()
            .map(|span| (span.range(), span.style()));
        render_ansi_line(chunk.text, spans, document.default_style, options, output)
            .map_err(write_error)?;
    }
    Ok(document.status())
}

/// ANSI counterpart to the compact HTML renderer.
#[cfg(all(
    feature = "ansi",
    feature = "bundled-grammars",
    feature = "bundled-themes"
))]
pub(crate) fn render_ansi_compact(
    source: &str,
    tokens: &HighlightedText,
    status: HighlightStatus,
    theme: &crate::Theme,
    options: &AnsiOptions,
) -> Result<RenderedOutput> {
    let mut output = String::with_capacity(source.len().saturating_mul(2));
    let defaults = theme.resolve_scope_names(&[]);
    let mut chunks = crate::engine::line::LineChunks::new(source);
    for (index, line) in tokens.lines.iter().enumerate() {
        let chunk = chunks.next().ok_or_else(compact_line_count_error)?;
        if index != 0 {
            output.push('\n');
        }
        let spans = line.segments.iter().map(|span| {
            (
                span.byte_start..span.byte_end,
                theme.resolve_interned(&line.scope_table, span.scope_stack),
            )
        });
        render_ansi_line(chunk.text, spans, defaults, options, &mut output).map_err(write_error)?;
    }
    if chunks.next().is_some() {
        return Err(compact_line_count_error());
    }
    Ok(RenderedOutput {
        content: output,
        status,
    })
}

#[cfg(feature = "ansi")]
fn render_ansi_line(
    text: &str,
    spans: impl Iterator<Item = (Range<usize>, Style)>,
    defaults: Style,
    options: &AnsiOptions,
    output: &mut dyn Write,
) -> fmt::Result {
    let mut cursor = 0;
    let mut active_style = Style::default();
    for (range, mut style) in spans {
        if range.is_empty() {
            continue;
        }
        if cursor < range.start {
            set_ansi_style(Style::default(), &mut active_style, output)?;
            write_ansi_source(&text[cursor..range.start], options, output)?;
        }
        if !options.colors {
            style = Style::default();
        } else if !options.include_default_background && style.background == defaults.background {
            style.background = None;
        }
        set_ansi_style(style, &mut active_style, output)?;
        write_ansi_source(&text[range.clone()], options, output)?;
        cursor = range.end;
    }
    set_ansi_style(Style::default(), &mut active_style, output)?;
    write_ansi_source(&text[cursor..], options, output)
}

#[cfg(feature = "ansi")]
fn set_ansi_style(style: Style, active: &mut Style, output: &mut dyn Write) -> fmt::Result {
    if style != *active {
        if *active != Style::default() {
            output.write_str("\x1b[0m")?;
        }
        write_ansi_style(style, output)?;
        *active = style;
    }
    Ok(())
}
#[cfg(feature = "ansi")]
fn write_ansi_style(style: Style, output: &mut dyn Write) -> fmt::Result {
    let has_codes =
        !style.modifiers.is_empty() || style.foreground.is_some() || style.background.is_some();
    if !has_codes {
        return Ok(());
    }
    output.write_str("\x1b[")?;
    let mut separator = "";
    for (enabled, code) in [
        (style.modifiers.contains(FontModifiers::BOLD), "1"),
        (style.modifiers.contains(FontModifiers::ITALIC), "3"),
        (style.modifiers.contains(FontModifiers::UNDERLINED), "4"),
        (style.modifiers.contains(FontModifiers::CROSSED_OUT), "9"),
    ] {
        if enabled {
            output.write_str(separator)?;
            output.write_str(code)?;
            separator = ";";
        }
    }
    if let Some(color) = style.foreground {
        output.write_str(separator)?;
        write_ansi_color("38", color, output)?;
        separator = ";";
    }
    if let Some(color) = style.background {
        output.write_str(separator)?;
        write_ansi_color("48", color, output)?;
    }
    output.write_char('m')?;
    Ok(())
}

#[cfg(feature = "ansi")]
fn write_ansi_color(prefix: &str, color: RgbColor, output: &mut dyn Write) -> fmt::Result {
    output.write_str(prefix)?;
    output.write_str(";2;")?;
    write_ansi_decimal_byte(color.red, output)?;
    output.write_char(';')?;
    write_ansi_decimal_byte(color.green, output)?;
    output.write_char(';')?;
    write_ansi_decimal_byte(color.blue, output)?;
    Ok(())
}

#[cfg(feature = "ansi")]
fn write_ansi_decimal_byte(mut byte: u8, output: &mut dyn Write) -> fmt::Result {
    if byte >= 100 {
        output.write_char(char::from(b'0' + byte / 100))?;
        byte %= 100;
        output.write_char(char::from(b'0' + byte / 10))?;
    } else if byte >= 10 {
        output.write_char(char::from(b'0' + byte / 10))?;
    }
    output.write_char(char::from(b'0' + byte % 10))?;
    Ok(())
}

#[cfg(feature = "ansi")]
fn write_ansi_source(text: &str, options: &AnsiOptions, output: &mut dyn Write) -> fmt::Result {
    if !options.sanitize_control_characters {
        output.write_str(text)?;
        return Ok(());
    }
    for character in text.chars() {
        if character == '\t' || !character.is_control() {
            output.write_char(character)?;
            continue;
        }
        match character {
            '\0'..='\x1f' => {
                let picture = char::from_u32(0x2400 + u32::from(character)).unwrap_or('\u{fffd}');
                output.write_char(picture)?;
            }
            '\x7f' => output.write_char('\u{2421}')?,
            _ => {
                write!(output, "\\u{{{:x}}}", u32::from(character))?;
            }
        }
    }
    Ok(())
}

#[cfg(all(
    feature = "bundled-grammars",
    feature = "bundled-themes",
    any(feature = "ansi", feature = "html")
))]
fn compact_line_count_error() -> Error {
    Error::Render(crate::RenderError::mismatch(
        "source and compact token document have different logical line counts".to_owned(),
    ))
}

#[cfg(any(feature = "ansi", feature = "html"))]
fn write_error(error: fmt::Error) -> Error {
    Error::Render(crate::RenderError::writer(error))
}

#[cfg(any(feature = "ansi", feature = "html"))]
fn validate_document(source: &str, document: &HighlightedDocument) -> Result<()> {
    let line_count = crate::engine::line::LineChunks::new(source).count();
    if line_count != document.lines().len() {
        return Err(Error::Render(crate::RenderError::mismatch(format!(
            "source has {} logical lines but the highlighted document has {}",
            line_count,
            document.lines().len()
        ))));
    }
    for (line_index, (chunk, line)) in crate::engine::line::LineChunks::new(source)
        .zip(document.lines())
        .enumerate()
    {
        let text = chunk.text;
        let mut cursor = 0;
        for span in line.tokens() {
            let range = span.range();
            if range.start < cursor
                || range.start > range.end
                || range.end > text.len()
                || !text.is_char_boundary(range.start)
                || !text.is_char_boundary(range.end)
            {
                return Err(Error::Render(crate::RenderError::mismatch(format!(
                    "invalid highlighted byte range {range:?} on line {line_index}"
                ))));
            }
            cursor = range.end;
        }
    }
    Ok(())
}

#[cfg(all(
    test,
    feature = "ansi",
    feature = "html",
    feature = "bundled-grammars",
    feature = "bundled-themes"
))]
mod tests {
    use super::*;
    use crate::Highlighter;

    #[test]
    fn html_escapes_source_and_can_expose_exact_scopes() {
        let source = "fn main() { println!(\"<script>&\"); }\n";
        let mut highlighter = Highlighter::bundled().unwrap();
        let document = highlighter
            .highlight("rust", source, "github-dark")
            .unwrap();
        let output = render_html(
            source,
            &document,
            &HtmlOptions {
                class: Some("syntaxmate\" data-injected=\"no".to_owned()),
                include_scopes: true,
                ..HtmlOptions::default()
            },
        )
        .unwrap();
        assert!(
            output
                .as_str()
                .starts_with("<pre class=\"syntaxmate&quot; data-injected=&quot;no\" style=\"")
        );
        assert!(!output.as_str().contains(" data-injected=\"no\""));
        assert!(output.as_str().contains("&lt;script&gt;&amp;"));
        assert!(!output.as_str().contains("<script>"));
        assert!(output.as_str().contains("data-scopes=\""));
        assert!(output.as_str().ends_with("</code></pre>"));
        assert!(output.status().is_complete());
    }

    #[test]
    fn direct_color_writers_cover_every_byte_value() {
        for byte in 0..=u8::MAX {
            let mut html = String::new();
            write_html_hex_byte(byte, &mut html).unwrap();
            assert_eq!(html, format!("{byte:02x}"));

            let mut ansi = String::new();
            write_ansi_decimal_byte(byte, &mut ansi).unwrap();
            assert_eq!(ansi, byte.to_string());
        }
    }

    #[test]
    fn ansi_style_writer_preserves_sgr_code_order_without_temporary_strings() {
        let mut output = String::new();
        write_ansi_style(
            Style {
                foreground: Some(RgbColor {
                    red: 1,
                    green: 2,
                    blue: 3,
                }),
                background: Some(RgbColor {
                    red: 4,
                    green: 5,
                    blue: 6,
                }),
                modifiers: FontModifiers::BOLD,
            },
            &mut output,
        )
        .unwrap();
        assert_eq!(output, "\x1b[1;38;2;1;2;3;48;2;4;5;6m");

        for (modifier, expected) in [
            (FontModifiers::ITALIC, "\x1b[3m"),
            (FontModifiers::UNDERLINED, "\x1b[4m"),
            (FontModifiers::CROSSED_OUT, "\x1b[9m"),
        ] {
            output.clear();
            write_ansi_style(
                Style {
                    modifiers: modifier,
                    ..Style::default()
                },
                &mut output,
            )
            .unwrap();
            assert_eq!(output, expected);
        }

        output.clear();
        write_ansi_style(Style::default(), &mut output).unwrap();
        assert!(output.is_empty());
    }

    #[test]
    fn ansi_sanitizes_source_escape_sequences() {
        let source = "let value = \"\x1b[31m\";";
        let mut highlighter = Highlighter::bundled().unwrap();
        let document = highlighter
            .highlight("rust", source, "github-dark")
            .unwrap();
        let output = render_ansi(source, &document, &AnsiOptions::default()).unwrap();
        assert!(output.as_str().contains('␛'));
        assert!(!output.as_str().contains("\x1b[31m"));
        assert!(output.as_str().contains("\x1b["));
    }

    #[test]
    fn direct_compact_rendering_is_byte_exact_with_owned_rendering() {
        let source = "fn main() {\n\tprintln!(\"λ<&>\");\n}\n";

        let mut direct = Highlighter::bundled().unwrap();
        let direct_html = direct
            .highlight_html("rust", source, "github-dark")
            .unwrap();
        let direct_ansi = direct
            .highlight_ansi("rust", source, "github-dark")
            .unwrap();

        let mut owned = Highlighter::bundled().unwrap();
        let document = owned.highlight("rust", source, "github-dark").unwrap();
        let owned_html = render_html(source, &document, &HtmlOptions::default()).unwrap();
        let owned_ansi = render_ansi(source, &document, &AnsiOptions::default()).unwrap();

        assert_eq!(direct_html, owned_html);
        assert_eq!(direct_ansi, owned_ansi);

        let html_options = HtmlOptions {
            include_wrapper: false,
            class: None,
            include_scopes: true,
            ..HtmlOptions::default()
        };
        let ansi_options = AnsiOptions {
            colors: false,
            sanitize_control_characters: false,
            ..AnsiOptions::default()
        };
        let direct_html = direct
            .highlight_html_with_options("rust", source, "github-dark", &html_options)
            .unwrap();
        let direct_ansi = direct
            .highlight_ansi_with_options("rust", source, "github-dark", &ansi_options)
            .unwrap();
        assert_eq!(
            direct_html,
            render_html(source, &document, &html_options).unwrap()
        );
        assert_eq!(
            direct_ansi,
            render_ansi(source, &document, &ansi_options).unwrap()
        );
    }

    #[test]
    fn renderers_reject_mismatched_line_counts() {
        let mut highlighter = Highlighter::bundled().unwrap();
        let document = highlighter
            .highlight("rust", "let x = 1;", "github-dark")
            .unwrap();
        let error = render_html("different\nshape", &document, &HtmlOptions::default())
            .expect_err("line mismatch must fail");
        assert!(matches!(error, Error::Render(_)));
    }
    fn custom_document(source: &str, theme: &Theme) -> HighlightedDocument {
        let mut registry = crate::GrammarRegistry::new();
        let root = registry
            .add_json(
                r#"{
            "scopeName":"source.test",
            "patterns":[
                {"match":"a","name":"first.test"},
                {"match":"b","name":"second.test"},
                {"match":"c","name":"special.test"}
            ]
        }"#,
            )
            .unwrap();
        let mut tokenizer =
            crate::Tokenizer::new(&registry, root, crate::TokenizerOptions::default()).unwrap();
        crate::style_document(tokenizer.tokenize(source), theme)
    }

    fn custom_theme() -> Theme {
        Theme::from_json(r##"{
            "colors":{"editor.foreground":"#112233","editor.background":"#040506"},
            "tokenColors":[
                {"scope":"first, second","settings":{"foreground":"#abcdef"}},
                {"scope":"special","settings":{"background":"#778899","fontStyle":"bold italic underline strikethrough"}}
            ]
        }"##).unwrap()
    }

    #[test]
    fn html_hoists_defaults_and_merges_only_equivalent_output() {
        let theme = custom_theme();
        let document = custom_document("ab x", &theme);
        let html = render_html("ab x", &document, &HtmlOptions::default()).unwrap();
        assert_eq!(
            html.as_str(),
            "<pre class=\"syntaxmate\" style=\"color:#112233;background-color:#040506;\"><code><span style=\"color:#abcdef;\">ab</span> x</code></pre>"
        );
        let scoped = render_html(
            "ab x",
            &document,
            &HtmlOptions {
                include_scopes: true,
                ..HtmlOptions::default()
            },
        )
        .unwrap();
        assert_eq!(scoped.as_str().matches("<span").count(), 3);
        assert!(
            scoped
                .as_str()
                .contains("data-scopes=\"source.test first.test\">a</span>")
        );
        assert!(
            scoped
                .as_str()
                .contains("data-scopes=\"source.test second.test\">b</span>")
        );
        let unwrapped = render_html(
            "ab x",
            &document,
            &HtmlOptions {
                include_wrapper: false,
                ..HtmlOptions::default()
            },
        )
        .unwrap();
        assert_eq!(
            unwrapped.as_str(),
            "<span style=\"color:#abcdef;background-color:#040506;\">ab</span><span style=\"color:#112233;background-color:#040506;\"> x</span>"
        );
    }

    #[test]
    fn html_default_background_is_constant_size() {
        let source = "let x = 42;\n".repeat(100);
        let mut highlighter = Highlighter::bundled().unwrap();
        let html = highlighter
            .highlight_html("rust", &source, "github-dark")
            .unwrap();
        assert_eq!(html.as_str().matches("background-color:").count(), 1);
        assert!(html.as_str().len() < source.len() * 12);
    }

    #[test]
    fn html_matching_scopes_merge_and_gaps_stay_unstyled() {
        let style = Style {
            foreground: Some(RgbColor {
                red: 1,
                green: 2,
                blue: 3,
            }),
            ..Style::default()
        };
        let spans = [(0..1, style), (1..2, style), (3..4, style)];
        let mut html = String::new();
        render_html_line(
            "ab c",
            spans
                .into_iter()
                .map(|(range, style)| (range, style, ["same"].into_iter())),
            Style::default(),
            &HtmlOptions {
                include_scopes: true,
                ..HtmlOptions::default()
            },
            None,
            &mut html,
        )
        .unwrap();
        assert_eq!(
            html,
            "<span style=\"color:#010203;\" data-scopes=\"same\">ab</span> <span style=\"color:#010203;\" data-scopes=\"same\">c</span>"
        );
    }

    #[test]
    fn scope_classes_are_theme_independent_and_merge_shared_ancestors() {
        let theme = custom_theme();
        let source = "abc x";
        let document = custom_document(source, &theme);
        let css = html_stylesheet(&theme, "test");
        assert!(css.contains(":where(.sm-test-root){color:#112233;background-color:#040506;"));
        assert!(css.contains("[class|=\"sm-test-s-first\"]){color:#abcdef;}"));
        assert!(css.contains("[class|=\"sm-test-s-second\"]){color:#abcdef;}"));
        assert!(css.contains("--sm-test-decoration:underline line-through;"));
        for include_wrapper in [true, false] {
            let options = HtmlOptions {
                class: None,
                class_prefix: Some("test".to_owned()),
                include_wrapper,
                ..HtmlOptions::default()
            };
            let html = render_html(source, &document, &options).unwrap();
            let other = custom_document(source, &Theme::bundled("github-light").unwrap());
            assert_eq!(html, render_html(source, &other, &options).unwrap());
            assert!(!html.as_str().contains("style="));
            assert!(!html.as_str().contains("abcdef"));
            assert_eq!(
                html.as_str()
                    .matches("class=\"sm-test-s-source-test\"")
                    .count(),
                1
            );
            assert!(
                html.as_str()
                    .contains("<span class=\"sm-test-s-first-test\"><span>a</span></span>")
            );
        }
    }

    #[test]
    fn class_prefix_and_scope_injection_cannot_escape_html_or_css() {
        let prefix = "9\"/><script>\0\n}body{color:red}/*λ_";
        let scope = "scope\"/><script>&'\0\n";
        let grammar = serde_json::json!({ "scopeName": scope, "patterns": [] }).to_string();
        let mut registry = crate::GrammarRegistry::new();
        let root = registry.add_json(&grammar).unwrap();
        let mut tokenizer =
            crate::Tokenizer::new(&registry, root, crate::TokenizerOptions::default()).unwrap();
        let theme = custom_theme();
        let source = "<script>&\"'";
        let document = crate::style_document(tokenizer.tokenize(source), &theme);
        let html = render_html(
            source,
            &document,
            &HtmlOptions {
                class: None,
                class_prefix: Some(prefix.to_owned()),
                include_scopes: true,
                ..HtmlOptions::default()
            },
        )
        .unwrap();
        assert!(!html.as_str().contains("<script>"));
        assert!(!html.as_str().contains("\0"));
        assert!(html.as_str().contains("&lt;script&gt;&amp;&quot;&#39;"));
        assert!(
            html.as_str()
                .contains("data-scopes=\"scope&quot;/&gt;&lt;script&gt;&amp;&#39;�")
        );
        let encoded = encode_class_prefix(prefix);
        assert!(
            encoded
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
        );
        assert_ne!(encode_class_prefix("_22"), encode_class_prefix("\""));
        assert!(html.as_str().contains(&format!("{encoded}-root")));
        assert!(html.as_str().contains(&format!(
            "{encoded}-s-{}",
            encode_scope("scope\"/><script>&'\0")
        )));
        assert_ne!(encode_scope("a.b"), encode_scope("a-b"));
        assert_ne!(encode_scope("_22"), encode_scope("\""));
        let css = html_stylesheet(&theme, prefix);
        assert!(!css.contains("<script>"));
        assert!(!css.contains("}body{"));
        assert!(!css.contains("/*"));
        assert!(css.contains(&format!(":where(.{encoded}-root){{color:#112233;")));
    }

    #[test]
    fn default_font_modifiers_remain_on_runs_so_token_resets_work() {
        let theme = Theme::from_json(
            r##"{"tokenColors":[
            {"settings":{"fontStyle":"bold italic underline strikethrough"}},
            {"scope":"first","settings":{"fontStyle":""}}
        ]}"##,
        )
        .unwrap();
        let document = custom_document("xa", &theme);
        let html = render_html("xa", &document, &HtmlOptions::default()).unwrap();
        assert_eq!(
            html.as_str(),
            "<pre class=\"syntaxmate\"><code><span style=\"font-weight:bold;font-style:italic;text-decoration:underline line-through;\">x</span>a</code></pre>"
        );
    }

    #[test]
    fn ansi_merges_styles_omits_default_background_and_keeps_token_backgrounds() {
        let theme = custom_theme();
        let document = custom_document("abc", &theme);
        let ansi = render_ansi("abc", &document, &AnsiOptions::default()).unwrap();
        assert_eq!(
            ansi.as_str(),
            "\x1b[38;2;171;205;239mab\x1b[0m\x1b[1;3;4;9;38;2;17;34;51;48;2;119;136;153mc\x1b[0m"
        );
        let ansi = render_ansi(
            "abc",
            &document,
            &AnsiOptions {
                include_default_background: true,
                ..AnsiOptions::default()
            },
        )
        .unwrap();
        assert!(
            ansi.as_str()
                .starts_with("\x1b[38;2;171;205;239;48;2;4;5;6mab\x1b[0m")
        );
        let mut gap = String::new();
        let style = theme.resolve_scope_names(&["first.test"]);
        render_ansi_line(
            "a b",
            [(0..1, style), (2..3, style)].into_iter(),
            document.default_style,
            &AnsiOptions::default(),
            &mut gap,
        )
        .unwrap();
        assert_eq!(
            gap,
            "\x1b[38;2;171;205;239ma\x1b[0m \x1b[38;2;171;205;239mb\x1b[0m"
        );
    }

    #[test]
    fn ansi_sanitizes_all_controls_and_preserves_line_and_tab_behavior() {
        let source: String = (0..=0x9f).filter_map(char::from_u32).collect();
        let document = custom_document(&source, &custom_theme());
        let options = AnsiOptions {
            colors: false,
            ..AnsiOptions::default()
        };
        let rendered = render_ansi(&source, &document, &options).unwrap();
        let mut expected = String::new();
        for ch in source.chars() {
            match ch {
                '\n' | '\t' => expected.push(ch),
                '\0'..='\x1f' => expected.push(char::from_u32(0x2400 + u32::from(ch)).unwrap()),
                '\x7f' => expected.push('␡'),
                '\u{80}'..='\u{9f}' => write!(expected, "\\u{{{:x}}}", u32::from(ch)).unwrap(),
                _ => expected.push(ch),
            }
        }
        assert_eq!(rendered.as_str(), expected);
        let trusted = render_ansi(
            &source,
            &document,
            &AnsiOptions {
                sanitize_control_characters: false,
                ..options
            },
        )
        .unwrap();
        assert_eq!(trusted.as_str(), source);
    }

    #[test]
    fn compact_public_and_writer_paths_match_for_all_modes_and_line_shapes() {
        let mut highlighter = Highlighter::bundled().unwrap();
        for source in [
            "",
            "\n",
            "\r\n\n",
            "fn main() {\n\tprintln!(\"λ<&>\x1b[31m\");\r\n}\n",
            "/* unterminated",
        ] {
            let document = highlighter
                .highlight("rust", source, "github-dark")
                .unwrap();
            for include_wrapper in [false, true] {
                for include_scopes in [false, true] {
                    for class_prefix in [None, Some("a\"<λ".to_owned())] {
                        let options = HtmlOptions {
                            include_wrapper,
                            include_scopes,
                            class_prefix,
                            ..HtmlOptions::default()
                        };
                        let owned = render_html(source, &document, &options).unwrap();
                        assert_eq!(
                            owned,
                            highlighter
                                .highlight_html_with_options(
                                    "rust",
                                    source,
                                    "github-dark",
                                    &options
                                )
                                .unwrap()
                        );
                        let mut writer = String::new();
                        assert_eq!(
                            render_html_to(source, &document, &options, &mut writer).unwrap(),
                            owned.status()
                        );
                        assert_eq!(writer, owned.as_str());
                    }
                }
            }
            for colors in [false, true] {
                for sanitize_control_characters in [false, true] {
                    for include_default_background in [false, true] {
                        let options = AnsiOptions {
                            colors,
                            sanitize_control_characters,
                            include_default_background,
                        };
                        let owned = render_ansi(source, &document, &options).unwrap();
                        assert_eq!(
                            owned,
                            highlighter
                                .highlight_ansi_with_options(
                                    "rust",
                                    source,
                                    "github-dark",
                                    &options
                                )
                                .unwrap()
                        );
                        let mut writer = String::new();
                        assert_eq!(
                            render_ansi_to(source, &document, &options, &mut writer).unwrap(),
                            owned.status()
                        );
                        assert_eq!(writer, owned.as_str());
                    }
                }
            }
        }
    }

    #[test]
    fn streaming_validates_before_writing_and_propagates_failures_and_status() {
        struct FailsAfter(usize);
        impl Write for FailsAfter {
            fn write_str(&mut self, text: &str) -> fmt::Result {
                self.0 = self.0.checked_sub(text.len()).ok_or(fmt::Error)?;
                Ok(())
            }
        }
        let document = custom_document("ab", &custom_theme());
        for limit in [0, 15, 70, 120] {
            assert!(matches!(
                render_html_to(
                    "ab",
                    &document,
                    &HtmlOptions::default(),
                    &mut FailsAfter(limit)
                ),
                Err(Error::Render(_))
            ));
        }
        for limit in [0, 5, 22] {
            assert!(matches!(
                render_ansi_to(
                    "ab",
                    &document,
                    &AnsiOptions::default(),
                    &mut FailsAfter(limit)
                ),
                Err(Error::Render(_))
            ));
        }
        // Same byte length, but the first token boundary splits this Unicode character.
        for invalid_source in ["a\nb", "a", "λ"] {
            let mut output = String::from("untouched");
            assert!(
                render_html_to(
                    invalid_source,
                    &document,
                    &HtmlOptions::default(),
                    &mut output
                )
                .is_err()
            );
            assert!(
                render_ansi_to(
                    invalid_source,
                    &document,
                    &AnsiOptions::default(),
                    &mut output
                )
                .is_err()
            );
            assert_eq!(output, "untouched");
        }
        let mut highlighter = Highlighter::with_options(crate::TokenizerOptions {
            max_line_bytes: 1,
            ..crate::TokenizerOptions::default()
        })
        .unwrap();
        let source = "let too_long = 1;";
        let document = highlighter
            .highlight("rust", source, "github-dark")
            .unwrap();
        assert_eq!(document.status(), HighlightStatus::Degraded);
        let mut output = String::new();
        assert_eq!(
            render_html_to(source, &document, &HtmlOptions::default(), &mut output).unwrap(),
            HighlightStatus::Degraded
        );
        assert_eq!(
            render_ansi_to(source, &document, &AnsiOptions::default(), &mut output).unwrap(),
            HighlightStatus::Degraded
        );
        assert_eq!(
            highlighter
                .highlight_html("rust", source, "github-dark")
                .unwrap(),
            render_html(source, &document, &HtmlOptions::default()).unwrap()
        );
        assert_eq!(
            highlighter
                .highlight_ansi("rust", source, "github-dark")
                .unwrap(),
            render_ansi(source, &document, &AnsiOptions::default()).unwrap()
        );
    }

    #[test]
    fn streaming_writes_incrementally_to_a_non_string_sink() {
        #[derive(Default)]
        struct Counter {
            bytes: usize,
            largest_write: usize,
        }
        impl Write for Counter {
            fn write_str(&mut self, text: &str) -> fmt::Result {
                self.bytes += text.len();
                self.largest_write = self.largest_write.max(text.len());
                Ok(())
            }
        }
        let source = "ab x\n".repeat(1000);
        let document = custom_document(&source, &custom_theme());
        let mut html = Counter::default();
        render_html_to(&source, &document, &HtmlOptions::default(), &mut html).unwrap();
        assert_eq!(
            html.bytes,
            render_html(&source, &document, &HtmlOptions::default())
                .unwrap()
                .as_str()
                .len()
        );
        assert!(html.largest_write < 100);
        let mut ansi = Counter::default();
        render_ansi_to(&source, &document, &AnsiOptions::default(), &mut ansi).unwrap();
        assert_eq!(
            ansi.bytes,
            render_ansi(&source, &document, &AnsiOptions::default())
                .unwrap()
                .as_str()
                .len()
        );
        assert!(ansi.largest_write < 100);
    }
}

#[cfg(all(
    test,
    feature = "html",
    feature = "bundled-grammars",
    feature = "bundled-themes"
))]
#[path = "render/css_tests.rs"]
mod css_tests;
