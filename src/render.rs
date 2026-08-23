//! Safe, dependency-free HTML and ANSI rendering for highlighted documents.

#[cfg(any(feature = "ansi", feature = "html"))]
use std::fmt::Write as _;

use crate::HighlightStatus;
#[cfg(feature = "ansi")]
use crate::theme::RgbColor;
#[cfg(all(
    feature = "bundled-grammars",
    feature = "bundled-themes",
    any(feature = "ansi", feature = "html")
))]
use crate::{EngineHighlightedLine, HighlightedText, Theme};
#[cfg(any(feature = "ansi", feature = "html"))]
use crate::{Error, HighlightedDocument, HighlightedLine, Result, Style, theme::SyntaxModifiers};

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
    /// Wrap output in `<pre><code>...</code></pre>`.
    pub include_wrapper: bool,
    /// Class placed on the `<pre>` wrapper. Ignored without a wrapper.
    pub class: Option<String>,
    /// Add a `data-scopes` attribute containing the exact TextMate scope stack.
    pub include_scopes: bool,
}

#[cfg(feature = "html")]
impl Default for HtmlOptions {
    fn default() -> Self {
        Self {
            include_wrapper: true,
            class: Some("syntaxmate".to_owned()),
            include_scopes: false,
        }
    }
}

/// Renders highlighted source as escaped HTML.
///
/// Source text, wrapper classes, and optional scope attributes are escaped.
/// The document must have been produced from `source`; a mismatch returns an
/// error rather than slicing unchecked byte ranges.
#[cfg(feature = "html")]
pub fn render_html(
    source: &str,
    document: &HighlightedDocument,
    options: &HtmlOptions,
) -> Result<RenderedOutput> {
    let lines = validated_lines(source, document)?;
    let mut output = String::with_capacity(source.len().saturating_mul(2));
    if options.include_wrapper {
        output.push_str("<pre");
        if let Some(class) = options.class.as_deref() {
            output.push_str(" class=\"");
            escape_html_attribute(class, &mut output);
            output.push('"');
        }
        output.push_str("><code>");
    }

    for (line_index, (text, line)) in lines.into_iter().enumerate() {
        render_html_line(text, line, options, &mut output);
        if line_index + 1 < document.lines().len() {
            output.push('\n');
        }
    }

    if options.include_wrapper {
        output.push_str("</code></pre>");
    }
    Ok(RenderedOutput {
        content: output,
        status: document.status(),
    })
}

/// Renders the engine's compact scope-stack tokens directly, avoiding owned
/// public token and styled-document intermediates.
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
    if options.include_wrapper {
        output.push_str("<pre");
        if let Some(class) = options.class.as_deref() {
            output.push_str(" class=\"");
            escape_html_attribute(class, &mut output);
            output.push('"');
        }
        output.push_str("><code>");
    }

    let mut source_lines = crate::engine::line::LineChunks::new(source);
    for (line_index, line) in tokens.lines.iter().enumerate() {
        let chunk = source_lines.next().ok_or_else(compact_line_count_error)?;
        render_html_compact_line(chunk.text, line, theme, options, &mut output);
        if line_index + 1 < tokens.lines.len() {
            output.push('\n');
        }
    }
    if source_lines.next().is_some() {
        return Err(compact_line_count_error());
    }

    if options.include_wrapper {
        output.push_str("</code></pre>");
    }
    Ok(RenderedOutput {
        content: output,
        status,
    })
}

#[cfg(all(
    feature = "html",
    feature = "bundled-grammars",
    feature = "bundled-themes"
))]
fn render_html_compact_line(
    text: &str,
    line: &EngineHighlightedLine,
    theme: &Theme,
    options: &HtmlOptions,
    output: &mut String,
) {
    let mut cursor = 0;
    for span in &line.segments {
        debug_assert!(
            cursor <= span.byte_start
                && span.byte_start <= span.byte_end
                && span.byte_end <= text.len()
                && text.is_char_boundary(span.byte_start)
                && text.is_char_boundary(span.byte_end)
        );
        escape_html_text(&text[cursor..span.byte_start], output);
        output.push_str("<span");
        write_html_style(theme.resolve(&line.scope_table, span.scope_stack), output);
        if options.include_scopes {
            output.push_str(" data-scopes=\"");
            for (index, scope) in line.scope_table.stack_names(span.scope_stack).enumerate() {
                if index != 0 {
                    output.push(' ');
                }
                escape_html_attribute(scope, output);
            }
            output.push('"');
        }
        output.push('>');
        escape_html_text(&text[span.byte_start..span.byte_end], output);
        output.push_str("</span>");
        cursor = span.byte_end;
    }
    escape_html_text(&text[cursor..], output);
}

#[cfg(feature = "html")]
fn render_html_line(
    text: &str,
    line: &HighlightedLine,
    options: &HtmlOptions,
    output: &mut String,
) {
    let mut cursor = 0;
    for span in line.spans() {
        let range = span.range();
        escape_html_text(&text[cursor..range.start], output);
        output.push_str("<span");
        write_html_style(span.style(), output);
        if options.include_scopes {
            output.push_str(" data-scopes=\"");
            for (index, scope) in line.scope_names(span.scope_stack()).enumerate() {
                if index != 0 {
                    output.push(' ');
                }
                escape_html_attribute(scope, output);
            }
            output.push('"');
        }
        output.push('>');
        escape_html_text(&text[range.clone()], output);
        output.push_str("</span>");
        cursor = range.end;
    }
    escape_html_text(&text[cursor..], output);
}

#[cfg(feature = "html")]
fn write_html_style(style: Style, output: &mut String) {
    if style == Style::default() {
        return;
    }
    output.push_str(" style=\"");
    if let Some(color) = style.foreground {
        output.push_str("color:#");
        write_html_hex_byte(color.red, output);
        write_html_hex_byte(color.green, output);
        write_html_hex_byte(color.blue, output);
        output.push(';');
    }
    if let Some(color) = style.background {
        output.push_str("background-color:#");
        write_html_hex_byte(color.red, output);
        write_html_hex_byte(color.green, output);
        write_html_hex_byte(color.blue, output);
        output.push(';');
    }
    if style.modifiers.contains(SyntaxModifiers::BOLD) {
        output.push_str("font-weight:bold;");
    }
    if style.modifiers.contains(SyntaxModifiers::ITALIC) {
        output.push_str("font-style:italic;");
    }
    if style.modifiers.contains(SyntaxModifiers::UNDERLINED)
        || style.modifiers.contains(SyntaxModifiers::CROSSED_OUT)
    {
        output.push_str("text-decoration:");
        if style.modifiers.contains(SyntaxModifiers::UNDERLINED) {
            output.push_str("underline");
        }
        if style.modifiers.contains(SyntaxModifiers::CROSSED_OUT) {
            if style.modifiers.contains(SyntaxModifiers::UNDERLINED) {
                output.push(' ');
            }
            output.push_str("line-through");
        }
        output.push(';');
    }
    output.push('"');
}

#[cfg(feature = "html")]
fn write_html_hex_byte(byte: u8, output: &mut String) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    output.push(char::from(HEX[(byte >> 4) as usize]));
    output.push(char::from(HEX[(byte & 0x0f) as usize]));
}

#[cfg(feature = "html")]
fn escape_html_text(text: &str, output: &mut String) {
    for character in text.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&#39;"),
            character => output.push(character),
        }
    }
}

#[cfg(feature = "html")]
fn escape_html_attribute(text: &str, output: &mut String) {
    for character in text.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&#39;"),
            '\0' => output.push('\u{fffd}'),
            character if character.is_control() => {
                let _ = write!(output, "&#x{:x};", u32::from(character));
            }
            character => output.push(character),
        }
    }
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
}

#[cfg(feature = "ansi")]
impl Default for AnsiOptions {
    fn default() -> Self {
        Self {
            colors: true,
            sanitize_control_characters: true,
        }
    }
}

/// Renders highlighted source using 24-bit ANSI SGR sequences.
///
/// Control-character sanitization is enabled by default so untrusted source
/// cannot inject terminal escape sequences. Disable it only for trusted input.
#[cfg(feature = "ansi")]
pub fn render_ansi(
    source: &str,
    document: &HighlightedDocument,
    options: &AnsiOptions,
) -> Result<RenderedOutput> {
    let lines = validated_lines(source, document)?;
    let mut output = String::with_capacity(source.len().saturating_mul(2));
    for (line_index, (text, line)) in lines.into_iter().enumerate() {
        render_ansi_line(text, line, options, &mut output);
        if line_index + 1 < document.lines().len() {
            output.push('\n');
        }
    }
    Ok(RenderedOutput {
        content: output,
        status: document.status(),
    })
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
    theme: &Theme,
    options: &AnsiOptions,
) -> Result<RenderedOutput> {
    let mut output = String::with_capacity(source.len().saturating_mul(2));
    let mut source_lines = crate::engine::line::LineChunks::new(source);
    for (line_index, line) in tokens.lines.iter().enumerate() {
        let chunk = source_lines.next().ok_or_else(compact_line_count_error)?;
        render_ansi_compact_line(chunk.text, line, theme, options, &mut output);
        if line_index + 1 < tokens.lines.len() {
            output.push('\n');
        }
    }
    if source_lines.next().is_some() {
        return Err(compact_line_count_error());
    }
    Ok(RenderedOutput {
        content: output,
        status,
    })
}

#[cfg(all(
    feature = "ansi",
    feature = "bundled-grammars",
    feature = "bundled-themes"
))]
fn render_ansi_compact_line(
    text: &str,
    line: &EngineHighlightedLine,
    theme: &Theme,
    options: &AnsiOptions,
    output: &mut String,
) {
    let mut cursor = 0;
    let mut active_style = Style::default();
    for span in &line.segments {
        debug_assert!(
            cursor <= span.byte_start
                && span.byte_start <= span.byte_end
                && span.byte_end <= text.len()
                && text.is_char_boundary(span.byte_start)
                && text.is_char_boundary(span.byte_end)
        );
        write_ansi_source(&text[cursor..span.byte_start], options, output);
        let style = if options.colors {
            theme.resolve(&line.scope_table, span.scope_stack)
        } else {
            Style::default()
        };
        if style != active_style {
            if active_style != Style::default() {
                output.push_str("\x1b[0m");
            }
            write_ansi_style(style, output);
            active_style = style;
        }
        write_ansi_source(&text[span.byte_start..span.byte_end], options, output);
        cursor = span.byte_end;
    }
    if active_style != Style::default() {
        output.push_str("\x1b[0m");
    }
    write_ansi_source(&text[cursor..], options, output);
}

#[cfg(feature = "ansi")]
fn render_ansi_line(
    text: &str,
    line: &HighlightedLine,
    options: &AnsiOptions,
    output: &mut String,
) {
    let mut cursor = 0;
    let mut active_style = Style::default();
    for span in line.spans() {
        let range = span.range();
        write_ansi_source(&text[cursor..range.start], options, output);
        let style = if options.colors {
            span.style()
        } else {
            Style::default()
        };
        if style != active_style {
            if active_style != Style::default() {
                output.push_str("\x1b[0m");
            }
            write_ansi_style(style, output);
            active_style = style;
        }
        write_ansi_source(&text[range.clone()], options, output);
        cursor = range.end;
    }
    if active_style != Style::default() {
        output.push_str("\x1b[0m");
    }
    write_ansi_source(&text[cursor..], options, output);
}

#[cfg(feature = "ansi")]
fn write_ansi_style(style: Style, output: &mut String) {
    let has_codes =
        !style.modifiers.is_empty() || style.foreground.is_some() || style.background.is_some();
    if !has_codes {
        return;
    }
    output.push_str("\x1b[");
    let mut separator = "";
    for (enabled, code) in [
        (style.modifiers.contains(SyntaxModifiers::BOLD), "1"),
        (style.modifiers.contains(SyntaxModifiers::ITALIC), "3"),
        (style.modifiers.contains(SyntaxModifiers::UNDERLINED), "4"),
        (style.modifiers.contains(SyntaxModifiers::CROSSED_OUT), "9"),
    ] {
        if enabled {
            output.push_str(separator);
            output.push_str(code);
            separator = ";";
        }
    }
    if let Some(color) = style.foreground {
        output.push_str(separator);
        write_ansi_color("38", color, output);
        separator = ";";
    }
    if let Some(color) = style.background {
        output.push_str(separator);
        write_ansi_color("48", color, output);
    }
    output.push('m');
}

#[cfg(feature = "ansi")]
fn write_ansi_color(prefix: &str, color: RgbColor, output: &mut String) {
    output.push_str(prefix);
    output.push_str(";2;");
    write_ansi_decimal_byte(color.red, output);
    output.push(';');
    write_ansi_decimal_byte(color.green, output);
    output.push(';');
    write_ansi_decimal_byte(color.blue, output);
}

#[cfg(feature = "ansi")]
fn write_ansi_decimal_byte(mut byte: u8, output: &mut String) {
    if byte >= 100 {
        output.push(char::from(b'0' + byte / 100));
        byte %= 100;
        output.push(char::from(b'0' + byte / 10));
    } else if byte >= 10 {
        output.push(char::from(b'0' + byte / 10));
    }
    output.push(char::from(b'0' + byte % 10));
}

#[cfg(feature = "ansi")]
fn write_ansi_source(text: &str, options: &AnsiOptions, output: &mut String) {
    if !options.sanitize_control_characters {
        output.push_str(text);
        return;
    }
    for character in text.chars() {
        if character == '\t' || !character.is_control() {
            output.push(character);
            continue;
        }
        match character {
            '\0'..='\x1f' => {
                let picture = char::from_u32(0x2400 + u32::from(character)).unwrap_or('\u{fffd}');
                output.push(picture);
            }
            '\x7f' => output.push('\u{2421}'),
            _ => {
                let _ = write!(output, "\\u{{{:x}}}", u32::from(character));
            }
        }
    }
}

#[cfg(all(
    feature = "bundled-grammars",
    feature = "bundled-themes",
    any(feature = "ansi", feature = "html")
))]
fn compact_line_count_error() -> Error {
    Error::Render("source and compact token document have different logical line counts".to_owned())
}

#[cfg(any(feature = "ansi", feature = "html"))]
fn validated_lines<'a>(
    source: &'a str,
    document: &'a HighlightedDocument,
) -> Result<Vec<(&'a str, &'a HighlightedLine)>> {
    let source_lines = crate::engine::line::LineChunks::new(source)
        .map(|line| line.text)
        .collect::<Vec<_>>();
    if source_lines.len() != document.lines().len() {
        return Err(Error::Render(format!(
            "source has {} logical lines but the highlighted document has {}",
            source_lines.len(),
            document.lines().len()
        )));
    }

    source_lines
        .into_iter()
        .zip(document.lines())
        .enumerate()
        .map(|(line_index, (text, line))| {
            let mut cursor = 0;
            for span in line.spans() {
                let range = span.range();
                if range.start < cursor
                    || range.start > range.end
                    || range.end > text.len()
                    || !text.is_char_boundary(range.start)
                    || !text.is_char_boundary(range.end)
                {
                    return Err(Error::Render(format!(
                        "invalid highlighted byte range {range:?} on line {line_index}"
                    )));
                }
                cursor = range.end;
            }
            Ok((text, line))
        })
        .collect()
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
                .starts_with("<pre class=\"syntaxmate&quot; data-injected=&quot;no\"><code>")
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
            write_html_hex_byte(byte, &mut html);
            assert_eq!(html, format!("{byte:02x}"));

            let mut ansi = String::new();
            write_ansi_decimal_byte(byte, &mut ansi);
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
                modifiers: SyntaxModifiers::BOLD,
            },
            &mut output,
        );
        assert_eq!(output, "\x1b[1;38;2;1;2;3;48;2;4;5;6m");

        for (modifier, expected) in [
            (SyntaxModifiers::ITALIC, "\x1b[3m"),
            (SyntaxModifiers::UNDERLINED, "\x1b[4m"),
            (SyntaxModifiers::CROSSED_OUT, "\x1b[9m"),
        ] {
            output.clear();
            write_ansi_style(
                Style {
                    modifiers: modifier,
                    ..Style::default()
                },
                &mut output,
            );
            assert_eq!(output, expected);
        }

        output.clear();
        write_ansi_style(Style::default(), &mut output);
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
        };
        let ansi_options = AnsiOptions {
            colors: false,
            sanitize_control_characters: false,
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
    fn renderers_reject_a_document_from_different_source() {
        let mut highlighter = Highlighter::bundled().unwrap();
        let document = highlighter
            .highlight("rust", "let x = 1;", "github-dark")
            .unwrap();
        let error = render_html("different\nshape", &document, &HtmlOptions::default())
            .expect_err("line mismatch must fail");
        assert!(matches!(error, Error::Render(_)));
    }
}
