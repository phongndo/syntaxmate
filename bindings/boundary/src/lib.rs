//! Coarse, binding-friendly Syntaxmate API shared by the language bindings.
//!
//! Each call crosses a language boundary at most once per document or line:
//! output is either a rendered string or a [`TokenBuffer`] of flat arrays, never
//! one host object per token. Offsets are converted here, once, into the unit
//! each host language indexes strings by (see [`OffsetUnit`]).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::{collections::HashMap, fmt};

pub use syntaxmate::{AnsiOptions, HtmlOptions};
use syntaxmate::{
    Catalog, Error, HighlightSession, HighlightStatus, HighlightedToken, Highlighter, ScopeStackId,
    Style, Theme, html_stylesheet, render_ansi, render_html,
};

/// Version of the underlying `syntaxmate` engine.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Sentinel for an absent color in [`PackedStyle`].
pub const NO_COLOR: u32 = u32::MAX;
/// Bold bit in [`PackedStyle::modifiers`].
pub const BOLD: u8 = 1;
/// Italic bit in [`PackedStyle::modifiers`].
pub const ITALIC: u8 = 2;
/// Underline bit in [`PackedStyle::modifiers`].
pub const UNDERLINE: u8 = 4;
/// Strikethrough bit in [`PackedStyle::modifiers`].
pub const STRIKETHROUGH: u8 = 8;

/// Stable error categories; the numeric values are part of the C ABI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ErrorKind {
    /// The language ID or alias is not in the catalog.
    UnknownLanguage = 1,
    /// The bundled theme name is unknown.
    UnknownTheme = 2,
    /// A grammar could not be parsed or prepared.
    InvalidGrammar = 3,
    /// A custom theme could not be parsed.
    InvalidTheme = 4,
    /// A grammar bundle could not be decoded.
    InvalidBundle = 5,
    /// Caller input was rejected (multi-line session input, oversized source, …).
    InvalidInput = 6,
    /// Output rendering failed.
    Render = 7,
    /// An unexpected internal failure, including a caught panic.
    Internal = 8,
}

/// An error with a stable [`ErrorKind`] and a human-readable message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryError {
    /// Stable category.
    pub kind: ErrorKind,
    /// Human-readable description.
    pub message: String,
}

impl BoundaryError {
    /// Creates an error from a kind and message.
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for BoundaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for BoundaryError {}

impl From<Error> for BoundaryError {
    fn from(error: Error) -> Self {
        let kind = match &error {
            Error::UnknownLanguage(_) => ErrorKind::UnknownLanguage,
            Error::UnknownTheme(_) => ErrorKind::UnknownTheme,
            Error::Grammar(_) => ErrorKind::InvalidGrammar,
            Error::Theme(_) => ErrorKind::InvalidTheme,
            Error::Bundle(_) => ErrorKind::InvalidBundle,
            Error::Render(_) => ErrorKind::Render,
            Error::StateMismatch | Error::InvalidLine => ErrorKind::InvalidInput,
            _ => ErrorKind::Internal,
        };
        Self::new(kind, error.to_string())
    }
}

/// Result alias for boundary operations.
pub type Result<T> = std::result::Result<T, BoundaryError>;

/// The string index unit that token offsets are reported in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum OffsetUnit {
    /// UTF-8 bytes (C, C++, Rust).
    #[default]
    Utf8 = 0,
    /// UTF-16 code units (JavaScript, Java, C#).
    Utf16 = 1,
    /// Unicode scalar values (Python `str` indices).
    CodePoint = 2,
}

/// Options for [`Engine::tokens`] and [`Engine::session`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TokenOptions {
    /// Unit for every offset and length in the buffer.
    pub unit: OffsetUnit,
    /// Whether to fill [`TokenBuffer::token_scopes`] and [`TokenBuffer::scope_stacks`].
    pub include_scopes: bool,
}

/// A resolved style packed for flat transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PackedStyle {
    /// `0xRRGGBB`, or [`NO_COLOR`].
    pub foreground: u32,
    /// `0xRRGGBB`, or [`NO_COLOR`].
    pub background: u32,
    /// Bitset of [`BOLD`], [`ITALIC`], [`UNDERLINE`], [`STRIKETHROUGH`].
    pub modifiers: u8,
}

impl Default for PackedStyle {
    fn default() -> Self {
        Self {
            foreground: NO_COLOR,
            background: NO_COLOR,
            modifiers: 0,
        }
    }
}

impl From<Style> for PackedStyle {
    fn from(style: Style) -> Self {
        use syntaxmate::FontModifiers as M;
        let color = |color: Option<syntaxmate::RgbColor>| {
            color.map_or(NO_COLOR, |c| {
                u32::from(c.red) << 16 | u32::from(c.green) << 8 | u32::from(c.blue)
            })
        };
        let mut modifiers = 0;
        for (flag, bit) in [
            (M::BOLD, BOLD),
            (M::ITALIC, ITALIC),
            (M::UNDERLINED, UNDERLINE),
            (M::CROSSED_OUT, STRIKETHROUGH),
        ] {
            if style.modifiers.contains(flag) {
                modifiers |= bit;
            }
        }
        Self {
            foreground: color(style.foreground),
            background: color(style.background),
            modifiers,
        }
    }
}

/// Styled tokens as struct-of-arrays.
///
/// Lines are split on `\n` only; a `\r` before it belongs to the line text.
/// Token `i` spans `token_starts[i] .. token_starts[i] + token_lengths[i]`,
/// relative to the whole input (for a session, the input is one line). Tokens
/// of line `l` are indices `line_token_ranges[l] .. line_token_ranges[l + 1]`,
/// in order. Text not covered by any token uses [`Self::default_style`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TokenBuffer {
    /// Unit of every offset and length below.
    pub unit: OffsetUnit,
    /// Whether tokenization finished within resource limits.
    pub complete: bool,
    /// Offset of each logical line's first character.
    pub line_starts: Vec<u32>,
    /// Token index boundaries per line; length is `line_starts.len() + 1`.
    pub line_token_ranges: Vec<u32>,
    /// Token start offsets.
    pub token_starts: Vec<u32>,
    /// Token lengths.
    pub token_lengths: Vec<u32>,
    /// Index into [`Self::styles`] per token.
    pub token_styles: Vec<u32>,
    /// Index into [`Self::scope_stacks`] per token; empty unless requested.
    pub token_scopes: Vec<u32>,
    /// Distinct styles referenced by tokens.
    pub styles: Vec<PackedStyle>,
    /// Distinct scope stacks, outermost first; empty unless requested.
    pub scope_stacks: Vec<Vec<String>>,
    /// The theme's default foreground and background.
    pub default_style: PackedStyle,
}

/// Rendered HTML or ANSI output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    /// Output text.
    pub text: String,
    /// Whether tokenization finished within resource limits.
    pub complete: bool,
}

/// A bundled or custom theme.
#[derive(Debug, Clone)]
pub struct ThemeHandle {
    // Bundled names keep the engine's compact render path.
    bundled: Option<String>,
    theme: Theme,
}

impl ThemeHandle {
    /// Loads a bundled theme by name.
    pub fn bundled(name: &str) -> Result<Self> {
        Ok(Self {
            theme: Theme::bundled(name)?,
            bundled: Some(name.to_owned()),
        })
    }

    /// Parses a TextMate JSON theme.
    pub fn from_json(json: &str) -> Result<Self> {
        Ok(Self {
            theme: Theme::from_json(json)?,
            bundled: None,
        })
    }

    /// Returns the theme's name.
    pub fn name(&self) -> &str {
        self.theme.name()
    }

    /// Returns the default foreground and background.
    pub fn default_style(&self) -> PackedStyle {
        self.theme.default_style().into()
    }

    /// Returns CSS for HTML rendered with `class_prefix` (class mode).
    pub fn stylesheet(&self, class_prefix: &str) -> String {
        html_stylesheet(&self.theme, class_prefix)
    }
}

/// A thread-safe, cheaply cloned highlighter over one grammar catalog.
#[derive(Debug, Clone)]
pub struct Engine {
    catalog: Catalog,
    highlighter: Highlighter,
}

impl Engine {
    /// Creates an engine over the embedded grammar catalog.
    #[cfg(feature = "bundled-grammars")]
    pub fn bundled() -> Result<Self> {
        Ok(Self::with_catalog(Catalog::bundled()))
    }

    /// Creates an engine from grammar-bundle bytes (see `docs/assets.md`).
    pub fn from_bundle(bytes: &[u8]) -> Result<Self> {
        Ok(Self::with_catalog(Catalog::from_bytes(bytes)?))
    }

    fn with_catalog(catalog: Catalog) -> Self {
        Self {
            highlighter: Highlighter::new(&catalog),
            catalog,
        }
    }

    /// Lists canonical language IDs.
    pub fn languages(&self) -> Vec<String> {
        self.catalog
            .languages()
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    /// Lists bundled theme names.
    pub fn themes(&self) -> Vec<String> {
        self.catalog
            .themes()
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    /// Resolves an ID or alias to its canonical language ID.
    pub fn canonical_language(&self, language: &str) -> Option<String> {
        self.catalog.canonical_language(language).map(str::to_owned)
    }

    /// Detects a language from an optional path and the source's first line.
    pub fn detect(&self, path: Option<&str>, source: &str) -> Option<String> {
        self.catalog
            .detect(path.map(std::path::Path::new), source)
            .map(str::to_owned)
    }

    /// Highlights a document to escaped HTML.
    pub fn html(
        &self,
        language: &str,
        source: &str,
        theme: &ThemeHandle,
        options: &HtmlOptions,
    ) -> Result<Rendered> {
        let output = match &theme.bundled {
            Some(name) => self
                .highlighter
                .highlight_html_with_options(language, source, name, options)?,
            None => {
                let document =
                    self.highlighter
                        .highlight_with_theme(language, source, &theme.theme)?;
                render_html(source, &document, options)?
            }
        };
        Ok(rendered(output))
    }

    /// Highlights a document to 24-bit ANSI text.
    pub fn ansi(
        &self,
        language: &str,
        source: &str,
        theme: &ThemeHandle,
        options: &AnsiOptions,
    ) -> Result<Rendered> {
        let output = match &theme.bundled {
            Some(name) => self
                .highlighter
                .highlight_ansi_with_options(language, source, name, options)?,
            None => {
                let document =
                    self.highlighter
                        .highlight_with_theme(language, source, &theme.theme)?;
                render_ansi(source, &document, options)?
            }
        };
        Ok(rendered(output))
    }

    /// Highlights a document into a flat [`TokenBuffer`].
    pub fn tokens(
        &self,
        language: &str,
        source: &str,
        theme: &ThemeHandle,
        options: TokenOptions,
    ) -> Result<TokenBuffer> {
        check_len(source)?;
        let document = self
            .highlighter
            .highlight_with_theme(language, source, &theme.theme)?;
        let mut builder = BufferBuilder::new(options, theme.default_style());
        let mut lines = source.split('\n');
        let mut line_start = 0u32;
        for line in document.lines() {
            let text = lines.next().ok_or_else(line_mismatch)?;
            builder.push_line(text, line_start, line.tokens());
            line_start += units(text, options.unit) + 1;
        }
        if lines.next().is_some() {
            return Err(line_mismatch());
        }
        Ok(builder.finish(document.status()))
    }

    /// Starts an incremental session that highlights one line per call.
    pub fn session(
        &self,
        language: &str,
        theme: &ThemeHandle,
        options: TokenOptions,
    ) -> Result<Session> {
        Ok(Session {
            inner: self
                .highlighter
                .session_with_theme(language, &theme.theme)?,
            options,
            default_style: theme.default_style(),
            spans: Vec::new(),
        })
    }
}

/// Incremental highlighting: feed logical lines in order, without terminators.
#[derive(Debug)]
pub struct Session {
    inner: HighlightSession,
    options: TokenOptions,
    default_style: PackedStyle,
    spans: Vec<HighlightedToken>,
}

impl Session {
    /// Highlights the next line; returns a one-line buffer with line-relative offsets.
    pub fn line(&mut self, line: &str) -> Result<TokenBuffer> {
        check_len(line)?;
        let status = self.inner.highlight_line_into(line, &mut self.spans)?;
        let mut builder = BufferBuilder::new(self.options, self.default_style);
        builder.push_line(line, 0, &self.spans);
        Ok(builder.finish(status))
    }

    /// Returns to the start-of-document state, keeping caches.
    pub fn reset(&mut self) {
        self.inner.reset();
    }
}

fn rendered(output: syntaxmate::RenderedOutput) -> Rendered {
    Rendered {
        complete: output.status().is_complete(),
        text: output.into_string(),
    }
}

fn check_len(text: &str) -> Result<()> {
    u32::try_from(text.len()).map(drop).map_err(|_| {
        BoundaryError::new(ErrorKind::InvalidInput, "input exceeds 4 GiB offset limit")
    })
}

fn line_mismatch() -> BoundaryError {
    BoundaryError::new(
        ErrorKind::Internal,
        "document line count does not match source",
    )
}

// Offsets are bounded by `check_len`, so unit counts fit in `u32`.
fn units(text: &str, unit: OffsetUnit) -> u32 {
    (match unit {
        OffsetUnit::Utf8 => text.len(),
        _ if text.is_ascii() => text.len(),
        OffsetUnit::Utf16 => text.chars().map(char::len_utf16).sum(),
        OffsetUnit::CodePoint => text.chars().count(),
    }) as u32
}

/// Converts ascending byte offsets within one line in a single forward pass.
struct LineCursor<'a> {
    text: &'a str,
    unit: OffsetUnit,
    identity: bool,
    byte: usize,
    position: u32,
}

impl<'a> LineCursor<'a> {
    fn new(text: &'a str, unit: OffsetUnit) -> Self {
        Self {
            text,
            unit,
            identity: unit == OffsetUnit::Utf8 || text.is_ascii(),
            byte: 0,
            position: 0,
        }
    }

    fn advance(&mut self, byte: usize) -> u32 {
        if self.identity {
            return byte as u32;
        }
        if byte < self.byte {
            // Tokens are ordered, but stay correct if a caller ever is not.
            self.byte = 0;
            self.position = 0;
        }
        self.position += units(&self.text[self.byte..byte], self.unit);
        self.byte = byte;
        self.position
    }
}

struct BufferBuilder {
    buffer: TokenBuffer,
    include_scopes: bool,
    styles: HashMap<PackedStyle, u32>,
    scopes: HashMap<Option<ScopeStackId>, u32>,
}

impl BufferBuilder {
    fn new(options: TokenOptions, default_style: PackedStyle) -> Self {
        Self {
            buffer: TokenBuffer {
                unit: options.unit,
                default_style,
                line_token_ranges: vec![0],
                ..TokenBuffer::default()
            },
            include_scopes: options.include_scopes,
            styles: HashMap::new(),
            scopes: HashMap::new(),
        }
    }

    fn push_line(&mut self, text: &str, line_start: u32, tokens: &[HighlightedToken]) {
        let buffer = &mut self.buffer;
        buffer.line_starts.push(line_start);
        let mut cursor = LineCursor::new(text, buffer.unit);
        for token in tokens {
            let range = token.range();
            let start = cursor.advance(range.start);
            let end = cursor.advance(range.end);
            buffer.token_starts.push(line_start + start);
            buffer.token_lengths.push(end - start);
            let next = buffer.styles.len() as u32;
            let style = *self
                .styles
                .entry(token.style().into())
                .or_insert_with_key(|style| {
                    buffer.styles.push(*style);
                    next
                });
            buffer.token_styles.push(style);
            if self.include_scopes {
                let next = buffer.scope_stacks.len() as u32;
                let stack = token.scope_stack();
                // Unkeyed stacks cannot be deduplicated cheaply; give each its own entry.
                let index = match stack.and_then(|key| self.scopes.get(&Some(key))) {
                    Some(index) => *index,
                    None => {
                        buffer
                            .scope_stacks
                            .push(token.scopes().map(str::to_owned).collect());
                        if stack.is_some() {
                            self.scopes.insert(stack, next);
                        }
                        next
                    }
                };
                buffer.token_scopes.push(index);
            }
        }
        buffer
            .line_token_ranges
            .push(buffer.token_starts.len() as u32);
    }

    fn finish(mut self, status: HighlightStatus) -> TokenBuffer {
        self.buffer.complete = status.is_complete();
        self.buffer
    }
}
