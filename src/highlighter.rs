use std::collections::VecDeque;
use std::{
    ops::Range,
    sync::{Arc, Mutex},
};

#[cfg(feature = "bundled-themes")]
use crate::theme::BuiltinTextMateTheme;
use crate::{
    Catalog, PreparedLanguage, TokenizerOptions,
    engine::{state::ScopeStackId as EngineScopeStackId, tokenizer::SharedScopeSink},
    tokenizer::{TokenScopes, Tokenizer, TokenizerState},
};
use crate::{
    Error, Result, ScopeStackId,
    theme::{Style, TextMateTheme},
    tokenizer::{HighlightStatus, Scopes, Token, TokenizedDocument},
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
    /// Parses a TextMate JSON theme, returning [`Error::Theme`] for invalid input.
    pub fn from_json(json: &str) -> Result<Self> {
        TextMateTheme::from_json(json)
            .map(|theme| Self {
                inner: ThemeInner::Owned(Arc::new(theme)),
            })
            .map_err(Error::Theme)
    }

    /// Loads a bundled theme by name, returning an error for an unknown name.
    #[cfg(feature = "bundled-themes")]
    pub fn bundled(name: &str) -> Result<Self> {
        let theme = BuiltinTextMateTheme::from_name(name)
            .ok_or_else(|| Error::UnknownTheme(name.to_owned()))?;
        Ok(Self {
            inner: ThemeInner::Bundled(theme.get()),
        })
    }

    /// Returns the theme name.
    pub fn name(&self) -> &str {
        self.inner.get().name()
    }

    #[cfg(feature = "html")]
    pub(crate) fn rendering_styles(&self) -> impl Iterator<Item = Style> + '_ {
        self.inner.get().rendering_styles()
    }

    /// Builds a theme from ordered TextMate rules, returning [`Error::Theme`] on invalid settings.
    pub fn from_rules(rules: &[crate::ThemeRule]) -> Result<Self> {
        TextMateTheme::from_rules(rules)
            .map(|theme| Self {
                inner: ThemeInner::Owned(Arc::new(theme)),
            })
            .map_err(Error::Theme)
    }

    /// Returns the theme's default foreground, background, and font modifiers.
    pub fn default_style(&self) -> Style {
        self.inner.get().default_style()
    }

    /// Looks up a named UI color, such as `editor.background`.
    pub fn color(&self, name: &str) -> Option<crate::RgbColor> {
        self.inner.get().color(name)
    }

    /// Resolves the remaining ordered scope names in a borrowed view.
    /// Whole-document views reuse the intern table's resolved-style cache.
    pub fn resolve(&self, scopes: Scopes<'_>) -> Style {
        self.inner.get().resolve_scopes(scopes)
    }

    /// Resolves a style for a standalone ordered list of scope names.
    pub fn resolve_scope_names(&self, scopes: &[&str]) -> Style {
        self.inner.get().resolve_names(scopes)
    }

    /// Resolves a style together with diagnostic selector and match metadata.
    #[cfg(feature = "diagnostics")]
    pub fn resolve_with_match(&self, scopes: Scopes<'_>) -> crate::ThemeMatch<'_> {
        self.inner.get().inspect_scopes(&scopes)
    }

    /// Resolves property-match flags for diagnostic tooling.
    #[cfg(feature = "diagnostics")]
    pub fn resolve_style(&self, scopes: Scopes<'_>) -> crate::ResolvedThemeStyle {
        let matched = self.resolve_with_match(scopes);
        crate::ResolvedThemeStyle {
            foreground_matched: matched.foreground_matched,
            background_matched: matched.background_matched,
            modifiers_matched: matched.modifiers_matched,
            style: matched.style,
        }
    }

    #[cfg(all(feature = "bundled-themes", any(feature = "html", feature = "ansi")))]
    pub(crate) fn resolve_interned(
        &self,
        table: &crate::HighlightScopeTable,
        stack: ScopeStackId,
    ) -> Style {
        self.inner.get().resolve(table, stack)
    }

    pub(crate) fn resolve_shared_scope_names(&self, scopes: &[Arc<str>]) -> Style {
        self.inner.get().resolve_shared_scope_names(scopes)
    }
}

/// Cache and tokenizer limits for a shared [`Highlighter`].
#[derive(Debug, Clone, Copy)]
pub struct HighlighterOptions {
    /// Maximum retained prepared languages (LRU); defaults to 16. Zero disables retention.
    pub prepared_languages: usize,
    /// Maximum idle tokenizers per retained language; defaults to 2.
    /// Zero disables replay reuse. Active calls and sessions are caller-owned
    /// and may temporarily exceed this limit.
    pub idle_tokenizers_per_language: usize,
    /// Per-tokenizer limits. Defaults to 1,024 cached lines and otherwise
    /// [`TokenizerOptions::default`]. Zero line entries disables replay caching.
    pub tokenizer: TokenizerOptions,
}

impl Default for HighlighterOptions {
    fn default() -> Self {
        Self {
            prepared_languages: 16,
            idle_tokenizers_per_language: 2,
            tokenizer: TokenizerOptions {
                line_cache_entries: 1024,
                ..TokenizerOptions::default()
            },
        }
    }
}

/// A cheaply cloned, thread-safe highlighter backed by a [`Catalog`].
///
/// Clones share a bounded LRU of prepared languages and idle tokenizers.
/// Matching runs outside the cache lock; simultaneous calls use independent
/// mutable tokenizers. Sessions share preparation but own their continuation.
/// See [`HighlighterOptions`] for retention limits. Limits count entries, not
/// bytes: grammar size, line length and scope diversity also affect memory.
#[derive(Debug, Clone)]
pub struct Highlighter {
    inner: Arc<HighlighterInner>,
}

#[derive(Debug)]
struct HighlighterInner {
    catalog: Catalog,
    options: HighlighterOptions,
    cache: Mutex<VecDeque<CachedLanguage>>,
}

#[derive(Debug)]
struct CachedLanguage {
    id: String,
    prepared: Arc<PreparedLanguage>,
    idle: Vec<Tokenizer>,
}

impl Highlighter {
    /// Creates a highlighter backed by the supplied catalog.
    pub fn new(catalog: &Catalog) -> Self {
        Self::with_catalog_options(catalog, HighlighterOptions::default())
    }

    /// Creates a catalog-backed highlighter with explicit retention limits.
    pub fn with_catalog_options(catalog: &Catalog, options: HighlighterOptions) -> Self {
        Self {
            inner: Arc::new(HighlighterInner {
                catalog: catalog.clone(),
                options,
                cache: Mutex::new(VecDeque::new()),
            }),
        }
    }

    /// Creates a highlighter backed by the embedded catalog.
    #[cfg(feature = "bundled-grammars")]
    pub fn bundled() -> Result<Self> {
        Ok(Self::new(&Catalog::bundled()))
    }

    /// Creates a bundled highlighter with caller-supplied tokenizer limits.
    #[cfg(feature = "bundled-grammars")]
    pub fn with_options(options: TokenizerOptions) -> Result<Self> {
        Ok(Self::with_catalog_options(
            &Catalog::bundled(),
            HighlighterOptions {
                tokenizer: options,
                ..HighlighterOptions::default()
            },
        ))
    }

    /// Tokenizes a complete document and preserves exact TextMate scope stacks.
    pub fn tokenize(&self, language: &str, source: &str) -> Result<TokenizedDocument> {
        self.with_tokenizer(language, |tokenizer| tokenizer.tokenize(source))
    }

    fn checkout(
        &self,
        language: &str,
        take_idle: bool,
    ) -> Result<(Arc<PreparedLanguage>, Option<Tokenizer>)> {
        let canonical = self
            .inner
            .catalog
            .canonical_language(language)
            .ok_or_else(|| Error::UnknownLanguage(language.to_owned()))?;
        let mut cache = self
            .inner
            .cache
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(index) = cache.iter().position(|entry| entry.id == canonical) {
            let mut entry = cache.remove(index).expect("cache index exists");
            let tokenizer = if take_idle { entry.idle.pop() } else { None };
            let prepared = Arc::clone(&entry.prepared);
            cache.push_back(entry);
            return Ok((prepared, tokenizer));
        }
        let prepared = Arc::new(PreparedLanguage::for_highlighter(
            &self.inner.catalog,
            canonical,
        )?);
        let tokenizer = take_idle.then(|| prepared.first_tokenizer(self.inner.options.tokenizer));
        if self.inner.options.prepared_languages > 0 {
            if cache.len() == self.inner.options.prepared_languages {
                cache.pop_front();
            }
            cache.push_back(CachedLanguage {
                id: canonical.to_owned(),
                prepared: Arc::clone(&prepared),
                idle: Vec::new(),
            });
        }
        Ok((prepared, tokenizer))
    }

    fn with_tokenizer<T>(
        &self,
        language: &str,
        operation: impl FnOnce(&mut Tokenizer) -> T,
    ) -> Result<T> {
        let (prepared, tokenizer) = self.checkout(language, true)?;
        let mut tokenizer =
            tokenizer.unwrap_or_else(|| prepared.tokenizer(self.inner.options.tokenizer));
        let result = operation(&mut tokenizer);
        let mut cache = self
            .inner
            .cache
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(entry) = cache
            .iter_mut()
            .find(|entry| Arc::ptr_eq(&entry.prepared, &prepared))
            && entry.idle.len() < self.inner.options.idle_tokenizers_per_language
        {
            entry.idle.push(tokenizer);
        }
        Ok(result)
    }

    #[cfg(all(feature = "bundled-themes", any(feature = "html", feature = "ansi")))]
    fn tokenize_compact(
        &self,
        language: &str,
        source: &str,
    ) -> Result<(crate::HighlightedText, HighlightStatus)> {
        self.with_tokenizer(language, |tokenizer| tokenizer.tokenize_compact(source))
    }

    /// Tokenizes and styles a complete source document with a bundled theme.
    #[cfg(feature = "bundled-themes")]
    pub fn highlight(
        &self,
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
        &self,
        language: &str,
        source: &str,
        theme: &str,
    ) -> Result<crate::RenderedOutput> {
        self.highlight_html_with_options(language, source, theme, &crate::HtmlOptions::default())
    }

    /// Highlights and renders escaped HTML directly from compact tokens.
    #[cfg(all(feature = "bundled-themes", feature = "html"))]
    pub fn highlight_html_with_options(
        &self,
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
        &self,
        language: &str,
        source: &str,
        theme: &str,
    ) -> Result<crate::RenderedOutput> {
        self.highlight_ansi_with_options(language, source, theme, &crate::AnsiOptions::default())
    }

    /// Highlights and renders ANSI output directly from compact tokens.
    #[cfg(all(feature = "bundled-themes", feature = "ansi"))]
    pub fn highlight_ansi_with_options(
        &self,
        language: &str,
        source: &str,
        theme: &str,
        options: &crate::AnsiOptions,
    ) -> Result<crate::RenderedOutput> {
        let theme = Theme::bundled(theme)?;
        let (tokens, status) = self.tokenize_compact(language, source)?;
        crate::render::render_ansi_compact(source, &tokens, status, &theme, options)
    }

    /// Tokenizes and styles a document with a caller-supplied theme.
    pub fn highlight_with_theme(
        &self,
        language: &str,
        source: &str,
        theme: &Theme,
    ) -> Result<HighlightedDocument> {
        let tokenized = self.tokenize(language, source)?;
        Ok(style_document(tokenized, theme))
    }

    /// Detects a language using [`Catalog::detect`] and highlights the source.
    #[cfg(feature = "bundled-themes")]
    pub fn highlight_path(
        &self,
        path: impl AsRef<std::path::Path>,
        source: &str,
        theme: &str,
    ) -> Result<HighlightedDocument> {
        let path = path.as_ref().to_string_lossy();
        let language = self
            .inner
            .catalog
            .detect(Some(std::path::Path::new(path.as_ref())), source)
            .ok_or_else(|| Error::UnknownLanguage(path.into_owned()))?;
        self.highlight(language, source, theme)
    }

    /// Starts an incremental session with a bundled theme.
    #[cfg(feature = "bundled-themes")]
    pub fn session(&self, language: &str, theme: &str) -> Result<HighlightSession> {
        let theme = Theme::bundled(theme)?;
        self.session_with_theme(language, &theme)
    }

    /// Starts an incremental session with a caller-supplied theme.
    ///
    /// This remains available when `bundled-themes` is disabled.
    pub fn session_with_theme(&self, language: &str, theme: &Theme) -> Result<HighlightSession> {
        let (prepared, _) = self.checkout(language, false)?;
        let tokenizer = prepared.tokenizer(self.inner.options.tokenizer);
        let state = tokenizer.initial_state();
        Ok(HighlightSession {
            tokenizer,
            state,
            theme: theme.clone(),
            style_cache: IncrementalStyleCache::default(),
        })
    }
}

/// Resolves a tokenized document against a theme without retokenizing source.
pub fn style_document(tokenized: TokenizedDocument, theme: &Theme) -> HighlightedDocument {
    let status = tokenized.status();
    let lines = tokenized
        .lines
        .into_iter()
        .map(|line| HighlightedLine {
            tokens: line
                .tokens
                .into_iter()
                .map(|token| {
                    let style = theme.resolve(token.scopes());
                    HighlightedToken { token, style }
                })
                .collect(),
            status: line.status,
        })
        .collect();
    HighlightedDocument {
        lines,
        status,
        default_style: theme.inner.get().default_style(),
    }
}

const MAX_INCREMENTAL_STYLE_CACHE_ENTRIES: usize = 8_192;

/// A dense session-local cache keyed by the originating tokenizer's stable
/// scope-stack identity. IDs above the fixed slot bound remain uncached.
#[derive(Debug, Default)]
struct IncrementalStyleCache {
    styles: Vec<Option<Style>>,
}

impl IncrementalStyleCache {
    fn resolve(&mut self, stack: EngineScopeStackId, scopes: &[Arc<str>], theme: &Theme) -> Style {
        let index = stack.0 as usize;
        if let Some(style) = self.styles.get(index).copied().flatten() {
            return style;
        }

        let style = theme.resolve_shared_scope_names(scopes);
        if index < MAX_INCREMENTAL_STYLE_CACHE_ENTRIES {
            if self.styles.len() <= index {
                self.styles.resize(index + 1, None);
            }
            self.styles[index] = Some(style);
        }
        style
    }
}

struct IncrementalSpanVecSink<'a> {
    line: &'a str,
    theme: &'a Theme,
    style_cache: &'a mut IncrementalStyleCache,
    spans: &'a mut Vec<HighlightedToken>,
}

impl SharedScopeSink for IncrementalSpanVecSink<'_> {
    fn reserve(&mut self, span_count: usize) {
        if self.spans.capacity() == 0 {
            *self.spans = Vec::with_capacity(span_count);
        } else if self.spans.capacity() < span_count {
            self.spans.reserve(span_count);
        }
    }

    fn push(
        &mut self,
        range: Range<usize>,
        stack: EngineScopeStackId,
        scopes: Arc<crate::types::ScopeStorage>,
    ) {
        if let Some(span) = incremental_span(
            self.line,
            range,
            stack,
            scopes,
            self.theme,
            self.style_cache,
        ) {
            self.spans.push(span);
        }
    }
}

struct IncrementalSpanCallbackSink<'a, F> {
    line: &'a str,
    theme: &'a Theme,
    style_cache: &'a mut IncrementalStyleCache,
    callback: F,
}

impl<F: FnMut(HighlightedToken)> SharedScopeSink for IncrementalSpanCallbackSink<'_, F> {
    fn reserve(&mut self, _span_count: usize) {}

    fn push(
        &mut self,
        range: Range<usize>,
        stack: EngineScopeStackId,
        scopes: Arc<crate::types::ScopeStorage>,
    ) {
        if let Some(span) = incremental_span(
            self.line,
            range,
            stack,
            scopes,
            self.theme,
            self.style_cache,
        ) {
            (self.callback)(span);
        }
    }
}

fn incremental_span(
    line: &str,
    range: Range<usize>,
    stack: EngineScopeStackId,
    scopes: Arc<crate::types::ScopeStorage>,
    theme: &Theme,
    style_cache: &mut IncrementalStyleCache,
) -> Option<HighlightedToken> {
    let start = range.start.min(line.len());
    let end = range.end.min(line.len());
    (start < end && line.is_char_boundary(start) && line.is_char_boundary(end)).then(|| {
        HighlightedToken {
            style: style_cache.resolve(stack, scopes.shared_names(), theme),
            token: Token {
                range: start..end,
                scopes: TokenScopes {
                    owner: Some(scopes),
                    stack: ScopeStackId(stack.0),
                },
            },
        }
    })
}

/// A reusable incremental highlighter. Input lines exclude newline terminators.
///
/// Resolved styles are retained in a session-local cache for up to 8,192
/// tokenizer scope-stack identities. Higher identities remain uncached.
#[derive(Debug)]
pub struct HighlightSession {
    tokenizer: Tokenizer,
    state: TokenizerState,
    theme: Theme,
    style_cache: IncrementalStyleCache,
}

impl HighlightSession {
    /// Highlights one logical line without newline terminators and advances continuation state.
    pub fn highlight_line(&mut self, line: &str) -> Result<HighlightedLine> {
        let mut spans = Vec::new();
        let status = self.highlight_line_into(line, &mut spans)?;
        Ok(HighlightedLine {
            tokens: spans,
            status,
        })
    }

    /// Highlights one logical line into a caller-owned reusable span buffer.
    ///
    /// The buffer is cleared after input validation, retains its capacity, and
    /// is left untouched when validation fails.
    pub fn highlight_line_into(
        &mut self,
        line: &str,
        spans: &mut Vec<HighlightedToken>,
    ) -> Result<HighlightStatus> {
        self.tokenizer.validate_line(line, &self.state)?;
        spans.clear();
        let mut sink = IncrementalSpanVecSink {
            line,
            theme: &self.theme,
            style_cache: &mut self.style_cache,
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
        sink: impl FnMut(HighlightedToken),
    ) -> Result<HighlightStatus> {
        let mut sink = IncrementalSpanCallbackSink {
            line,
            theme: &self.theme,
            style_cache: &mut self.style_cache,
            callback: sink,
        };
        self.tokenizer
            .tokenize_line_shared_with(line, &mut self.state, &mut sink)
    }

    /// Resets continuation state to the start of a document.
    ///
    /// Tokenizer and resolved-style caches are retained, making this suitable
    /// for replaying a document after edits without rebuilding the session.
    pub fn reset(&mut self) {
        self.state = self.tokenizer.initial_state();
    }

    /// Borrows the current continuation state.
    pub fn state(&self) -> &TokenizerState {
        &self.state
    }
}

/// One styled token with a line-relative byte range and exact ordered scopes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightedToken {
    token: Token,
    style: Style,
}

impl HighlightedToken {
    /// Returns the line-relative UTF-8 byte range, excluding line terminators.
    pub fn range(&self) -> Range<usize> {
        self.token.range()
    }

    /// Iterates scope names from outermost to innermost without allocating.
    pub fn scopes(&self) -> Scopes<'_> {
        self.token.scopes()
    }

    /// Returns the scope-stack key; see [`Token::scope_stack`] for comparability.
    pub fn scope_stack(&self) -> Option<ScopeStackId> {
        self.token.scope_stack()
    }

    /// Returns the resolved theme style.
    pub fn style(&self) -> Style {
        self.style
    }
}

/// One styled logical line, from a document or an incremental session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightedLine {
    tokens: Vec<HighlightedToken>,
    status: HighlightStatus,
}

impl HighlightedLine {
    /// Returns styled tokens in byte order.
    pub fn tokens(&self) -> &[HighlightedToken] {
        &self.tokens
    }

    /// Reports whether this line was fully tokenized within resource limits.
    pub fn status(&self) -> HighlightStatus {
        self.status
    }
}

/// Styled logical lines and their aggregate completion status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightedDocument {
    lines: Vec<HighlightedLine>,
    status: HighlightStatus,
    pub(crate) default_style: Style,
}

impl HighlightedDocument {
    /// Returns logical lines in source order.
    pub fn lines(&self) -> &[HighlightedLine] {
        &self.lines
    }

    /// Reports whether the entire tokenization operation completed within limits.
    pub fn status(&self) -> HighlightStatus {
        self.status
    }
}

#[cfg(all(test, feature = "bundled-grammars"))]
mod incremental_style_cache_tests {
    use super::*;

    fn test_theme() -> Theme {
        Theme::from_json(
            r##"{
                "name": "Incremental cache test",
                "colors": {"editor.foreground": "#010203"},
                "tokenColors": [{
                    "scope": "keyword",
                    "settings": {"foreground": "#112233", "fontStyle": "bold"}
                }]
            }"##,
        )
        .unwrap()
    }

    #[test]
    fn incremental_style_cache_is_exact_and_hard_bounded() {
        let theme = test_theme();
        let scopes = [Arc::<str>::from("source.test"), Arc::from("keyword.test")];
        let expected = theme.resolve_shared_scope_names(&scopes);
        let mut cache = IncrementalStyleCache::default();

        assert_eq!(
            cache.resolve(EngineScopeStackId(3), &scopes, &theme),
            expected
        );
        assert_eq!(cache.styles.len(), 4);
        assert_eq!(cache.styles[3], Some(expected));
        assert_eq!(
            cache.resolve(EngineScopeStackId(3), &scopes, &theme),
            expected
        );

        assert_eq!(
            cache.resolve(
                EngineScopeStackId(MAX_INCREMENTAL_STYLE_CACHE_ENTRIES as u32 - 1),
                &scopes,
                &theme,
            ),
            expected
        );
        assert_eq!(cache.styles.len(), MAX_INCREMENTAL_STYLE_CACHE_ENTRIES);
        assert_eq!(
            cache.resolve(
                EngineScopeStackId(MAX_INCREMENTAL_STYLE_CACHE_ENTRIES as u32),
                &scopes,
                &theme,
            ),
            expected
        );
        assert_eq!(cache.styles.len(), MAX_INCREMENTAL_STYLE_CACHE_ENTRIES);
    }

    #[test]
    fn incremental_session_reuses_scope_identity_styles_across_lines() {
        let highlighter = Highlighter::bundled().unwrap();
        let theme = test_theme();
        let mut session = highlighter.session_with_theme("rust", &theme).unwrap();

        let first = session.highlight_line("fn cached() {}").unwrap();
        let cached_slots = session
            .style_cache
            .styles
            .iter()
            .filter(|style| style.is_some())
            .count();
        let cache_len = session.style_cache.styles.len();
        assert!(cached_slots > 0);

        session.reset();
        let second = session.highlight_line("fn cached() {}").unwrap();
        assert_eq!(second, first);
        assert_eq!(session.style_cache.styles.len(), cache_len);
        assert_eq!(
            session
                .style_cache
                .styles
                .iter()
                .filter(|style| style.is_some())
                .count(),
            cached_slots
        );
    }
}

#[cfg(all(test, feature = "bundled-grammars"))]
mod cache_tests {
    use super::*;

    #[test]
    fn cache_reuses_preparation_for_sessions_and_bounds_idle_state() {
        let highlighter = Highlighter::with_catalog_options(
            &Catalog::bundled(),
            HighlighterOptions {
                prepared_languages: 1,
                idle_tokenizers_per_language: 1,
                ..HighlighterOptions::default()
            },
        );
        let (first, _) = highlighter.checkout("rs", false).unwrap();
        let (second, _) = highlighter.clone().checkout("rust", false).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        let theme = Theme::from_json(r#"{"tokenColors":[]}"#).unwrap();
        let mut session = highlighter.session_with_theme("rust", &theme).unwrap();
        highlighter.tokenize("rust", "fn main() {}").unwrap();
        let (prepared, tokenizer) = highlighter.checkout("rust", true).unwrap();
        assert!(tokenizer.is_some());
        assert!(Arc::ptr_eq(&first, &prepared));
        highlighter.tokenize("json", "{}").unwrap();
        let cache = highlighter.inner.cache.lock().unwrap();
        assert_eq!(cache.len(), 1);
        assert_eq!(cache[0].id, "json");
        assert_eq!(cache[0].idle.len(), 1);
        drop(cache);
        // An evicted language remains usable by an already active session.
        assert!(
            session
                .highlight_line("fn main() {}")
                .unwrap()
                .status()
                .is_complete()
        );
        highlighter
            .with_tokenizer("json", |_| {
                highlighter.tokenize("json", "[]").unwrap();
            })
            .unwrap();
        assert_eq!(highlighter.inner.cache.lock().unwrap()[0].idle.len(), 1);
    }

    #[test]
    fn zero_capacity_disables_retention() {
        let highlighter = Highlighter::with_catalog_options(
            &Catalog::bundled(),
            HighlighterOptions {
                prepared_languages: 0,
                ..HighlighterOptions::default()
            },
        );
        highlighter.tokenize("json", "{}").unwrap();
        assert!(highlighter.inner.cache.lock().unwrap().is_empty());
        let first = highlighter.checkout("json", false).unwrap().0;
        let second = highlighter.checkout("json", false).unwrap().0;
        assert!(!Arc::ptr_eq(&first, &second));
    }
}
