#[cfg(feature = "bundled-grammars")]
use std::collections::BTreeMap;
use std::{ops::Range, sync::Arc};

#[cfg(feature = "bundled-themes")]
use crate::theme::BuiltinTextMateTheme;
use crate::{
    Error, HighlightScopeTable, Result, ScopeStackRef,
    theme::{ResolvedSyntaxStyle, TextMateTheme},
    tokenizer::{HighlightStatus, TokenizedDocument},
};
#[cfg(feature = "bundled-grammars")]
use crate::{
    TokenizerOptions,
    engine::tokenizer::SharedScopeSink,
    tokenizer::{Tokenizer, TokenizerState},
};

/// A parsed TextMate theme that can be shared across highlighting sessions.
#[derive(Debug, Clone)]
pub struct Theme {
    inner: ThemeInner,
}

#[derive(Debug, Clone)]
enum ThemeInner {
    #[cfg(feature = "bundled-themes")]
    Bundled(&'static TextMateTheme),
    Owned(Arc<TextMateTheme>),
}

impl ThemeInner {
    fn get(&self) -> &TextMateTheme {
        match self {
            #[cfg(feature = "bundled-themes")]
            Self::Bundled(theme) => theme,
            Self::Owned(theme) => theme,
        }
    }
}

impl Theme {
    pub fn from_json(json: &str) -> Result<Self> {
        TextMateTheme::from_json(json)
            .map(|theme| Self {
                inner: ThemeInner::Owned(Arc::new(theme)),
            })
            .map_err(Error::Theme)
    }

    #[cfg(feature = "bundled-themes")]
    pub fn bundled(name: &str) -> Result<Self> {
        let theme = BuiltinTextMateTheme::from_name(name)
            .ok_or_else(|| Error::UnknownTheme(name.to_owned()))?;
        Ok(Self {
            inner: ThemeInner::Bundled(theme.get()),
        })
    }

    pub fn name(&self) -> &str {
        self.inner.get().name()
    }

    /// Resolves a style for an interned exact TextMate scope stack.
    pub fn resolve(
        &self,
        table: &HighlightScopeTable,
        stack: ScopeStackRef,
    ) -> ResolvedSyntaxStyle {
        self.inner.get().resolve(table, stack)
    }

    /// Resolves a style for a standalone ordered list of scope names.
    pub fn resolve_scope_names(&self, scopes: &[&str]) -> ResolvedSyntaxStyle {
        let (table, stack) = HighlightScopeTable::from_scope_names(scopes);
        self.inner.get().resolve(&table, stack)
    }

    #[cfg(feature = "bundled-grammars")]
    pub(crate) fn resolve_shared_scope_names(&self, scopes: &[Arc<str>]) -> ResolvedSyntaxStyle {
        self.inner.get().resolve_shared_scope_names(scopes)
    }
}

/// Batteries-included TextMate highlighter with the bundled language catalog.
#[cfg(feature = "bundled-grammars")]
#[derive(Debug)]
pub struct Highlighter {
    options: TokenizerOptions,
    tokenizers: BTreeMap<String, Tokenizer>,
}

#[cfg(feature = "bundled-grammars")]
impl Highlighter {
    #[cfg(feature = "bundled-grammars")]
    pub fn bundled() -> Result<Self> {
        if crate::grammars::available_languages().is_empty() {
            return Err(Error::Bundle(
                "the bundled language catalog is empty".to_owned(),
            ));
        }
        Ok(Self {
            options: TokenizerOptions::default(),
            tokenizers: BTreeMap::new(),
        })
    }

    #[cfg(feature = "bundled-grammars")]
    pub fn with_options(options: TokenizerOptions) -> Result<Self> {
        let mut highlighter = Self::bundled()?;
        highlighter.options = options;
        Ok(highlighter)
    }

    /// Tokenizes a complete document and preserves exact TextMate scope stacks.
    #[cfg(feature = "bundled-grammars")]
    pub fn tokenize(&mut self, language: &str, source: &str) -> Result<TokenizedDocument> {
        Ok(self.tokenizer_for(language)?.tokenize(source))
    }

    fn tokenizer_for(&mut self, language: &str) -> Result<&mut Tokenizer> {
        let canonical = crate::grammars::canonical_language(language)
            .ok_or_else(|| Error::UnknownLanguage(language.to_owned()))?;
        if !self.tokenizers.contains_key(&canonical) {
            self.tokenizers.insert(
                canonical.clone(),
                Tokenizer::for_bundled_language(&canonical, self.options)?,
            );
        }
        Ok(self
            .tokenizers
            .get_mut(&canonical)
            .expect("tokenizer inserted before use"))
    }

    #[cfg(all(feature = "bundled-themes", any(feature = "html", feature = "ansi")))]
    fn tokenize_compact(
        &mut self,
        language: &str,
        source: &str,
    ) -> Result<(crate::HighlightedText, HighlightStatus)> {
        Ok(self.tokenizer_for(language)?.tokenize_compact(source))
    }

    /// Tokenizes and styles a complete source document with a bundled theme.
    #[cfg(all(feature = "bundled-grammars", feature = "bundled-themes"))]
    pub fn highlight(
        &mut self,
        language: &str,
        source: &str,
        theme: &str,
    ) -> Result<HighlightedDocument> {
        let theme = Theme::bundled(theme)?;
        self.highlight_with_theme(language, source, &theme)
    }

    /// Highlights source and renders escaped HTML with safe defaults.
    #[cfg(all(feature = "bundled-themes", feature = "html"))]
    pub fn highlight_html(
        &mut self,
        language: &str,
        source: &str,
        theme: &str,
    ) -> Result<crate::RenderedOutput> {
        self.highlight_html_with_options(language, source, theme, &crate::HtmlOptions::default())
    }

    /// Highlights and renders escaped HTML directly from compact tokens.
    #[cfg(all(feature = "bundled-themes", feature = "html"))]
    pub fn highlight_html_with_options(
        &mut self,
        language: &str,
        source: &str,
        theme: &str,
        options: &crate::HtmlOptions,
    ) -> Result<crate::RenderedOutput> {
        let theme = Theme::bundled(theme)?;
        let (tokens, status) = self.tokenize_compact(language, source)?;
        crate::render::render_html_compact(source, &tokens, status, &theme, options)
    }

    /// Highlights source and renders 24-bit ANSI output with control sanitization.
    #[cfg(all(feature = "bundled-themes", feature = "ansi"))]
    pub fn highlight_ansi(
        &mut self,
        language: &str,
        source: &str,
        theme: &str,
    ) -> Result<crate::RenderedOutput> {
        self.highlight_ansi_with_options(language, source, theme, &crate::AnsiOptions::default())
    }

    /// Highlights and renders ANSI output directly from compact tokens.
    #[cfg(all(feature = "bundled-themes", feature = "ansi"))]
    pub fn highlight_ansi_with_options(
        &mut self,
        language: &str,
        source: &str,
        theme: &str,
        options: &crate::AnsiOptions,
    ) -> Result<crate::RenderedOutput> {
        let theme = Theme::bundled(theme)?;
        let (tokens, status) = self.tokenize_compact(language, source)?;
        crate::render::render_ansi_compact(source, &tokens, status, &theme, options)
    }

    #[cfg(feature = "bundled-grammars")]
    pub fn highlight_with_theme(
        &mut self,
        language: &str,
        source: &str,
        theme: &Theme,
    ) -> Result<HighlightedDocument> {
        let tokenized = self.tokenize(language, source)?;
        Ok(style_document(tokenized, theme))
    }

    /// Detects a bundled language from a path and highlights the source.
    #[cfg(all(feature = "bundled-grammars", feature = "bundled-themes"))]
    pub fn highlight_path(
        &mut self,
        path: impl AsRef<std::path::Path>,
        source: &str,
        theme: &str,
    ) -> Result<HighlightedDocument> {
        let path = path.as_ref().to_string_lossy();
        let language = crate::grammars::detect_language_from_path(&path)
            .ok_or_else(|| Error::UnknownLanguage(path.into_owned()))?;
        self.highlight(&language, source, theme)
    }

    /// Starts an incremental session with a bundled theme.
    #[cfg(all(feature = "bundled-grammars", feature = "bundled-themes"))]
    pub fn session(&self, language: &str, theme: &str) -> Result<HighlightSession> {
        let theme = Theme::bundled(theme)?;
        self.session_with_theme(language, &theme)
    }

    /// Starts an incremental session with a caller-supplied theme.
    ///
    /// This remains available when `bundled-themes` is disabled.
    pub fn session_with_theme(&self, language: &str, theme: &Theme) -> Result<HighlightSession> {
        let tokenizer = Tokenizer::for_bundled_language(language, self.options)?;
        let state = tokenizer.initial_state();
        Ok(HighlightSession {
            tokenizer,
            state,
            theme: theme.clone(),
        })
    }
}

/// Resolves a tokenized document against a theme without retokenizing source.
pub fn style_document(tokenized: TokenizedDocument, theme: &Theme) -> HighlightedDocument {
    let status = tokenized.status();
    let lines = tokenized
        .lines()
        .iter()
        .map(|line| HighlightedLine {
            spans: line
                .spans()
                .iter()
                .map(|span| HighlightedSpan {
                    range: span.range(),
                    scope_stack: span.scope_stack(),
                    style: theme.resolve(line.scope_table(), span.scope_stack()),
                })
                .collect(),
            scopes: Arc::clone(line.scope_table()),
        })
        .collect();
    HighlightedDocument { lines, status }
}

#[cfg(feature = "bundled-grammars")]
struct IncrementalSpanVecSink<'a> {
    line: &'a str,
    theme: &'a Theme,
    spans: &'a mut Vec<IncrementalHighlightedSpan>,
}

#[cfg(feature = "bundled-grammars")]
impl SharedScopeSink for IncrementalSpanVecSink<'_> {
    fn reserve(&mut self, span_count: usize) {
        if self.spans.capacity() == 0 {
            *self.spans = Vec::with_capacity(span_count);
        } else if self.spans.capacity() < span_count {
            self.spans.reserve(span_count);
        }
    }

    fn push(&mut self, range: Range<usize>, scopes: Arc<[Arc<str>]>) {
        if let Some(span) = incremental_span(self.line, range, scopes, self.theme) {
            self.spans.push(span);
        }
    }
}

#[cfg(feature = "bundled-grammars")]
struct IncrementalSpanCallbackSink<'a, F> {
    line: &'a str,
    theme: &'a Theme,
    callback: F,
}

#[cfg(feature = "bundled-grammars")]
impl<F: FnMut(IncrementalHighlightedSpan)> SharedScopeSink for IncrementalSpanCallbackSink<'_, F> {
    fn reserve(&mut self, _span_count: usize) {}

    fn push(&mut self, range: Range<usize>, scopes: Arc<[Arc<str>]>) {
        if let Some(span) = incremental_span(self.line, range, scopes, self.theme) {
            (self.callback)(span);
        }
    }
}

#[cfg(feature = "bundled-grammars")]
fn incremental_span(
    line: &str,
    range: Range<usize>,
    scopes: Arc<[Arc<str>]>,
    theme: &Theme,
) -> Option<IncrementalHighlightedSpan> {
    let start = range.start.min(line.len());
    let end = range.end.min(line.len());
    (start < end && line.is_char_boundary(start) && line.is_char_boundary(end)).then(|| {
        IncrementalHighlightedSpan {
            range: start..end,
            style: theme.resolve_shared_scope_names(&scopes),
            scopes,
        }
    })
}

/// A reusable incremental highlighter. Input lines exclude newline terminators.
#[cfg(feature = "bundled-grammars")]
#[derive(Debug)]
pub struct HighlightSession {
    tokenizer: Tokenizer,
    state: TokenizerState,
    theme: Theme,
}

#[cfg(feature = "bundled-grammars")]
impl HighlightSession {
    pub fn highlight_line(&mut self, line: &str) -> Result<IncrementalHighlightedLine> {
        let tokenized = self.tokenizer.tokenize_line(line, &mut self.state)?;
        let (tokens, status) = tokenized.into_parts();
        let spans = tokens
            .into_iter()
            .map(|token| {
                let (range, scopes) = token.into_parts();
                IncrementalHighlightedSpan {
                    range,
                    style: self.theme.resolve_shared_scope_names(&scopes),
                    scopes,
                }
            })
            .collect();
        Ok(IncrementalHighlightedLine { spans, status })
    }

    /// Highlights one logical line into a caller-owned reusable span buffer.
    ///
    /// The buffer is cleared after input validation, retains its capacity, and
    /// is left untouched when validation fails.
    pub fn highlight_line_into(
        &mut self,
        line: &str,
        spans: &mut Vec<IncrementalHighlightedSpan>,
    ) -> Result<HighlightStatus> {
        self.tokenizer.validate_line(line, &self.state)?;
        spans.clear();
        let mut sink = IncrementalSpanVecSink {
            line,
            theme: &self.theme,
            spans,
        };
        Ok(self
            .tokenizer
            .tokenize_line_shared_with_validated(line, &mut self.state, &mut sink))
    }

    /// Highlights one logical line and sends each styled span to `sink`.
    ///
    /// Spans arrive in byte order and the callback is not invoked when input
    /// validation fails. The returned status covers the complete line.
    pub fn highlight_line_with(
        &mut self,
        line: &str,
        sink: impl FnMut(IncrementalHighlightedSpan),
    ) -> Result<HighlightStatus> {
        let mut sink = IncrementalSpanCallbackSink {
            line,
            theme: &self.theme,
            callback: sink,
        };
        self.tokenizer
            .tokenize_line_shared_with(line, &mut self.state, &mut sink)
    }

    pub fn state(&self) -> &TokenizerState {
        &self.state
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightedSpan {
    range: Range<usize>,
    scope_stack: ScopeStackRef,
    style: ResolvedSyntaxStyle,
}

impl HighlightedSpan {
    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }

    pub fn scope_stack(&self) -> ScopeStackRef {
        self.scope_stack
    }

    pub fn style(&self) -> ResolvedSyntaxStyle {
        self.style
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightedLine {
    spans: Vec<HighlightedSpan>,
    scopes: Arc<HighlightScopeTable>,
}

impl HighlightedLine {
    pub fn spans(&self) -> &[HighlightedSpan] {
        &self.spans
    }

    pub fn scope_names(&self, stack: ScopeStackRef) -> impl Iterator<Item = &str> {
        self.scopes.stack_names(stack)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightedDocument {
    lines: Vec<HighlightedLine>,
    status: HighlightStatus,
}

impl HighlightedDocument {
    pub fn lines(&self) -> &[HighlightedLine] {
        &self.lines
    }

    pub fn status(&self) -> HighlightStatus {
        self.status
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncrementalHighlightedSpan {
    range: Range<usize>,
    scopes: Arc<[Arc<str>]>,
    style: ResolvedSyntaxStyle,
}

impl IncrementalHighlightedSpan {
    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }

    pub fn scopes(&self) -> impl ExactSizeIterator<Item = &str> {
        self.scopes.iter().map(AsRef::as_ref)
    }

    pub fn style(&self) -> ResolvedSyntaxStyle {
        self.style
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncrementalHighlightedLine {
    spans: Vec<IncrementalHighlightedSpan>,
    status: HighlightStatus,
}

impl IncrementalHighlightedLine {
    pub fn spans(&self) -> &[IncrementalHighlightedSpan] {
        &self.spans
    }

    pub fn status(&self) -> HighlightStatus {
        self.status
    }
}
