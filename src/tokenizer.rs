use std::{ops::Range, sync::Arc};

use crate::types::ScopeStorage;

use crate::engine::checkpoint::CheckpointTable as EngineCheckpointTable;

use crate::{
    Error, HighlightScopeTable, Result, ScopeStackId, TokenizerOptions,
    engine::state::ScopeStackId as EngineScopeStackId,
    engine::tokenizer::{
        GrammarSet as EngineGrammarSet, PreparedLanguage as EnginePreparedLanguage,
        SharedScopeSink, TextMateTokenizer, TokenizerState as EngineTokenizerState,
    },
};

static NEXT_REGISTRY_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
static NEXT_TOKENIZER_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

struct ScopedTokenVecSink<'a> {
    line: &'a str,
    tokens: &'a mut Vec<Token>,
}

impl SharedScopeSink for ScopedTokenVecSink<'_> {
    fn reserve(&mut self, token_count: usize) {
        if self.tokens.capacity() == 0 {
            *self.tokens = Vec::with_capacity(token_count);
        } else if self.tokens.capacity() < token_count {
            self.tokens.reserve(token_count);
        }
    }

    fn push(&mut self, range: Range<usize>, stack: EngineScopeStackId, scopes: Arc<ScopeStorage>) {
        if let Some(token) = scoped_token(self.line, range, stack, scopes) {
            self.tokens.push(token);
        }
    }
}

struct ScopedTokenCallbackSink<'a, F> {
    line: &'a str,
    callback: F,
}

impl<F: FnMut(Token)> SharedScopeSink for ScopedTokenCallbackSink<'_, F> {
    fn reserve(&mut self, _token_count: usize) {}

    fn push(&mut self, range: Range<usize>, stack: EngineScopeStackId, scopes: Arc<ScopeStorage>) {
        if let Some(token) = scoped_token(self.line, range, stack, scopes) {
            (self.callback)(token);
        }
    }
}

fn scoped_token(
    line: &str,
    range: Range<usize>,
    stack: EngineScopeStackId,
    scopes: Arc<ScopeStorage>,
) -> Option<Token> {
    let start = range.start.min(line.len());
    let end = range.end.min(line.len());
    (start < end && line.is_char_boundary(start) && line.is_char_boundary(end)).then_some(Token {
        range: start..end,
        scopes: TokenScopes {
            owner: Some(scopes),
            stack: ScopeStackId(stack.0),
        },
    })
}

/// Resource limits applied while constructing a custom grammar registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrammarLimits {
    /// Maximum bytes accepted for one grammar JSON input; defaults to 4 MiB.
    pub max_grammar_bytes: usize,
    /// Maximum grammars in the registry; defaults to 4,096.
    pub max_grammars: usize,
}

impl Default for GrammarLimits {
    fn default() -> Self {
        Self {
            max_grammar_bytes: 4 * 1024 * 1024,
            max_grammars: 4_096,
        }
    }
}

/// A collection of TextMate JSON grammars and their external include scopes.
#[derive(Debug, Clone)]
pub struct GrammarRegistry {
    id: u64,
    limits: GrammarLimits,
    inner: EngineGrammarSet,
}

impl Default for GrammarRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl GrammarRegistry {
    /// Creates an empty grammar registry with default resource limits.
    pub fn new() -> Self {
        Self::with_limits(GrammarLimits::default())
    }

    /// Creates an empty registry with the supplied limits, clamped to at least one.
    pub fn with_limits(mut limits: GrammarLimits) -> Self {
        limits.max_grammars = limits.max_grammars.min(u16::MAX as usize);
        Self {
            id: NEXT_REGISTRY_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            limits,
            inner: EngineGrammarSet::new(),
        }
    }

    /// Parses and adds one JSON TextMate grammar.
    ///
    /// Add every grammar referenced through an external include before
    /// constructing a tokenizer.
    pub fn add_json(&mut self, json: &str) -> Result<GrammarId> {
        if json.len() > self.limits.max_grammar_bytes {
            return Err(Error::Grammar(crate::GrammarError::new(
                None,
                crate::GrammarErrorKind::LimitExceeded(crate::LimitExceeded::new(
                    crate::GrammarResource::GrammarBytes,
                    self.limits.max_grammar_bytes,
                    json.len(),
                )),
            )));
        }
        if self.inner.len() >= self.limits.max_grammars {
            return Err(Error::Grammar(crate::GrammarError::new(
                None,
                crate::GrammarErrorKind::LimitExceeded(crate::LimitExceeded::new(
                    crate::GrammarResource::GrammarCount,
                    self.limits.max_grammars,
                    self.inner.len() + 1,
                )),
            )));
        }
        let id = self
            .inner
            .load_and_add(json)
            .map_err(crate::error::grammar_load_error)?;
        Ok(GrammarId {
            registry: self.id,
            inner: id,
        })
    }

    /// Checks every registered regex for parser diagnostics, returning the first.
    ///
    /// This opt-in check does not change loading or tokenization. The engine is
    /// permissive and can skip malformed or unsupported constructs. A diagnostic
    /// may describe unsupported syntax rather than invalid Oniguruma syntax.
    /// Positions count Unicode scalar values from zero within the pattern.
    ///
    /// ```
    /// use syntaxmate::{Error, GrammarErrorKind, GrammarRegistry};
    /// let mut registry = GrammarRegistry::new();
    /// registry.add_json(r#"{"scopeName":"source.demo","patterns":[{"match":"é)"}]}"#)?;
    /// match registry.validate_regexes() {
    ///     Err(Error::Grammar(error)) => {
    ///         assert_eq!(error.scope_name(), Some("source.demo"));
    ///         if let GrammarErrorKind::InvalidRegex(regex) = error.kind() {
    ///             assert_eq!(regex.pattern(), "é)");
    ///             assert_eq!(regex.position(), 1);
    ///         }
    ///     }
    ///     result => panic!("expected a regex diagnostic: {result:?}"),
    /// }
    /// # Ok::<(), syntaxmate::Error>(())
    /// ```
    pub fn validate_regexes(&self) -> Result<()> {
        for grammar in self.inner.iter() {
            for pattern in &grammar.patterns {
                let parsed = crate::engine::regex::parse(pattern);
                if let Some(position) = parsed.first_diagnostic_position {
                    return Err(Error::Grammar(crate::GrammarError::new(
                        Some(grammar.scope_name.clone()),
                        crate::GrammarErrorKind::InvalidRegex(crate::RegexError::new(
                            pattern.to_string(),
                            position,
                            parsed.diagnostics[0].clone(),
                        )),
                    )));
                }
            }
        }
        Ok(())
    }

    /// Returns the number of registered grammars.
    pub fn grammar_count(&self) -> usize {
        self.inner.len()
    }

    /// Validates local and external include references across the registry.
    pub fn validate(&self) -> Result<()> {
        self.inner
            .validate_include_graph()
            .map_err(crate::error::grammar_validation_error)
    }
}

/// Opaque identity of a grammar in one [`GrammarRegistry`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GrammarId {
    registry: u64,
    inner: crate::engine::state::GrammarId,
}

fn preparation_limit_error(scope: Option<String>, detail: &str) -> Error {
    Error::Grammar(crate::GrammarError::new(
        scope,
        crate::GrammarErrorKind::PreparationLimit(detail.to_owned()),
    ))
}

/// Caller-owned immutable preparation for one root TextMate grammar.
///
/// Preparing retains the grammar closure, repository contexts, the compiled
/// root descriptor, and bounded lazily populated static regex/candidate caches.
/// Tokenizers created from this value have independent mutable state and caches;
/// they share only immutable preparation owned by this value. This is intended
/// for repeated independent tokenizers; one-off callers should use
/// [`Tokenizer::new`] or `Tokenizer::for_bundled_language` to avoid retaining
/// preparation after the tokenizer is dropped.
#[derive(Debug, Clone)]
pub struct PreparedLanguage {
    inner: Arc<EnginePreparedLanguage>,
}

impl PreparedLanguage {
    /// Prepares one root from a snapshot of a custom grammar registry.
    ///
    /// Returns [`Error::Grammar`] when the root is foreign or the grammar graph
    /// exceeds the hard preparation bounds; direct [`Tokenizer`] construction
    /// remains available for such inputs.
    pub fn new(registry: &GrammarRegistry, root: GrammarId) -> Result<Self> {
        if root.registry != registry.id || registry.inner.grammar(root.inner).is_none() {
            return Err(Error::Grammar(crate::GrammarError::new(
                None,
                crate::GrammarErrorKind::ForeignGrammarId,
            )));
        }
        let inner = EnginePreparedLanguage::try_new(registry.inner.clone(), root.inner).map_err(
            |detail| {
                preparation_limit_error(
                    registry
                        .inner
                        .grammar(root.inner)
                        .map(|grammar| grammar.scope_name.clone()),
                    detail,
                )
            },
        )?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Prepares one language from the bundled grammar catalog.
    ///
    /// Bundled grammars are checked to remain inside the preparation bounds.
    #[cfg(feature = "bundled-grammars")]
    pub fn for_bundled_language(language: &str) -> Result<Self> {
        Self::from_catalog(&crate::Catalog::bundled(), language)
    }

    /// Prepares a language and its recorded dependencies from a catalog.
    ///
    /// Retains the shared bundle and prepares the reachable root graph. Returns
    /// an error for an unknown language or exceeded preparation bounds.
    pub fn from_catalog(catalog: &crate::Catalog, language: &str) -> Result<Self> {
        let canonical = catalog
            .canonical_language(language)
            .ok_or_else(|| Error::UnknownLanguage(language.to_owned()))?;
        let (grammars, root) =
            crate::engine::load_catalog_grammar_set(catalog.bundle(), canonical)?;
        let scope = grammars
            .grammar(root)
            .map(|grammar| grammar.scope_name.clone());
        let inner = EnginePreparedLanguage::try_new(grammars, root)
            .map_err(|detail| preparation_limit_error(scope, detail))?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    // Highlighter caches the recorded closure and defers repository/candidate
    // walks just like direct tokenizers, preserving one-shot cold latency.
    pub(crate) fn for_highlighter(catalog: &crate::Catalog, canonical: &str) -> Result<Self> {
        let (grammars, root) =
            crate::engine::load_catalog_grammar_set(catalog.bundle(), canonical)?;
        Ok(Self {
            inner: Arc::new(EnginePreparedLanguage::from_catalog(grammars, root)),
        })
    }

    pub(crate) fn first_tokenizer(&self, options: TokenizerOptions) -> Tokenizer {
        Tokenizer::from_engine(self.inner.first_tokenizer(), options)
    }

    /// Creates a tokenizer with independent mutable state and caches.
    pub fn tokenizer(&self, options: TokenizerOptions) -> Tokenizer {
        Tokenizer::from_prepared(self, options)
    }

    /// Reports hard count/charged-byte bounds and current immutable population.
    pub fn stats(&self) -> PreparedLanguageStats {
        PreparedLanguageStats {
            grammar_count: self.inner.grammar_count(),
            static_pattern_capacity: self.inner.static_pattern_capacity(),
            compiled_pattern_count: self.inner.compiled_pattern_count(),
            static_pattern_byte_capacity: self.inner.static_pattern_byte_capacity(),
            static_pattern_retained_bytes: self.inner.static_pattern_retained_bytes(),
            static_candidate_capacity: self.inner.static_blueprint_capacity(),
            static_candidate_count: self.inner.static_blueprint_count(),
            static_candidate_byte_capacity: self.inner.static_blueprint_byte_capacity(),
            static_candidate_retained_bytes: self.inner.static_blueprint_retained_bytes(),
        }
    }
}

/// Bounded preparation statistics for a [`PreparedLanguage`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedLanguageStats {
    grammar_count: usize,
    static_pattern_capacity: usize,
    compiled_pattern_count: usize,
    static_pattern_byte_capacity: usize,
    static_pattern_retained_bytes: usize,
    static_candidate_capacity: usize,
    static_candidate_count: usize,
    static_candidate_byte_capacity: usize,
    static_candidate_retained_bytes: usize,
}

impl PreparedLanguageStats {
    /// Number of grammars retained in this prepared root's dependency closure.
    pub fn grammar_count(&self) -> usize {
        self.grammar_count
    }

    /// Number of static pattern slots reserved for this preparation.
    ///
    /// The charged-byte ceiling may stop population before every slot is used.
    pub fn static_pattern_capacity(&self) -> usize {
        self.static_pattern_capacity
    }

    /// Number of static patterns compiled so far across all derived tokenizers.
    pub fn compiled_pattern_count(&self) -> usize {
        self.compiled_pattern_count
    }

    /// Maximum charged bytes for the prepared regex slot table and matchers.
    pub fn static_pattern_byte_capacity(&self) -> usize {
        self.static_pattern_byte_capacity
    }

    /// Charged bytes currently retained by the regex slot table and matchers.
    pub fn static_pattern_retained_bytes(&self) -> usize {
        self.static_pattern_retained_bytes
    }

    /// Maximum number of static candidate descriptors retained for reuse.
    ///
    /// The candidate charged-byte ceiling may stop population sooner.
    pub fn static_candidate_capacity(&self) -> usize {
        self.static_candidate_capacity
    }

    /// Number of reusable static candidate descriptors currently retained.
    pub fn static_candidate_count(&self) -> usize {
        self.static_candidate_count
    }

    /// Maximum charged bytes for prepared candidate descriptors, scanners,
    /// and canonical injection outcomes.
    pub fn static_candidate_byte_capacity(&self) -> usize {
        self.static_candidate_byte_capacity
    }

    /// Charged bytes currently retained by candidate descriptors, scanners,
    /// and canonical injection outcomes.
    pub fn static_candidate_retained_bytes(&self) -> usize {
        self.static_candidate_retained_bytes
    }
}

/// A stateful tokenizer for one root TextMate grammar.
#[derive(Debug)]
pub struct Tokenizer {
    id: u64,
    inner: TextMateTokenizer,
    parse_line_buffer: String,
}

impl Tokenizer {
    /// Creates a tokenizer for a root grammar owned by `registry`.
    pub fn new(
        registry: &GrammarRegistry,
        root: GrammarId,
        options: TokenizerOptions,
    ) -> Result<Self> {
        if root.registry != registry.id || registry.inner.grammar(root.inner).is_none() {
            return Err(Error::Grammar(crate::GrammarError::new(
                None,
                crate::GrammarErrorKind::ForeignGrammarId,
            )));
        }
        Ok(Self::from_engine(
            TextMateTokenizer::new(registry.inner.clone(), root.inner),
            options,
        ))
    }

    /// Constructs a tokenizer from the bundled grammar catalog.
    #[cfg(feature = "bundled-grammars")]
    pub fn for_bundled_language(language: &str, options: TokenizerOptions) -> Result<Self> {
        let canonical = crate::grammars::canonical_language(language)
            .ok_or_else(|| Error::UnknownLanguage(language.to_owned()))?;
        let (grammars, root) = crate::engine::load_grammar_set(&canonical)?;
        Ok(Self::from_engine(
            TextMateTokenizer::new(grammars, root),
            options,
        ))
    }

    /// Constructs a tokenizer from caller-owned immutable preparation.
    pub fn from_prepared(prepared: &PreparedLanguage, options: TokenizerOptions) -> Self {
        Self::from_engine(prepared.inner.tokenizer(), options)
    }

    fn from_engine(mut inner: TextMateTokenizer, options: TokenizerOptions) -> Self {
        inner.configure_options(options);
        Self {
            id: NEXT_TOKENIZER_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            inner,
            parse_line_buffer: String::new(),
        }
    }

    /// Creates initial continuation state owned by this tokenizer.
    ///
    /// The returned state is document start: `\A` may match on the first
    /// subsequent line. Later continuation states do not rematch `\A`.
    pub fn initial_state(&self) -> TokenizerState {
        TokenizerState {
            owner: self.id,
            inner: EngineTokenizerState::default(),
            at_document_start: true,
        }
    }

    /// Tokenizes one logical line. `line` must not include a newline terminator.
    ///
    /// `\A` matches only while `state` is this tokenizer's initial
    /// document-start state, matching [`Tokenizer::tokenize`]. After a
    /// successful call, `state` is no longer at document start.
    pub fn tokenize_line(
        &mut self,
        line: &str,
        state: &mut TokenizerState,
    ) -> Result<TokenizedLine> {
        let mut tokens = Vec::new();
        let status = self.tokenize_line_into(line, state, &mut tokens)?;
        Ok(TokenizedLine { tokens, status })
    }

    /// Tokenizes one logical line into a caller-owned reusable buffer.
    ///
    /// The buffer is cleared after input validation and retains its capacity
    /// for subsequent calls. This is the allocation-conscious counterpart to
    /// [`Tokenizer::tokenize_line`].
    pub fn tokenize_line_into(
        &mut self,
        line: &str,
        state: &mut TokenizerState,
        tokens: &mut Vec<Token>,
    ) -> Result<HighlightStatus> {
        self.validate_line(line, state)?;
        tokens.clear();
        let mut sink = ScopedTokenVecSink { line, tokens };
        Ok(self.tokenize_line_with_validated(line, state, &mut sink))
    }

    /// Tokenizes one logical line and sends each token to `sink` in byte order.
    ///
    /// The callback receives owned tokens backed by shared immutable scope
    /// names, so no output collection is required. It is not called when input
    /// validation fails. The returned status covers the complete line.
    pub fn tokenize_line_with(
        &mut self,
        line: &str,
        state: &mut TokenizerState,
        sink: impl FnMut(Token),
    ) -> Result<HighlightStatus> {
        self.validate_line(line, state)?;
        let mut sink = ScopedTokenCallbackSink {
            line,
            callback: sink,
        };
        Ok(self.tokenize_line_with_validated(line, state, &mut sink))
    }

    pub(crate) fn tokenize_line_shared_with(
        &mut self,
        line: &str,
        state: &mut TokenizerState,
        sink: &mut impl SharedScopeSink,
    ) -> Result<HighlightStatus> {
        self.validate_line(line, state)?;
        Ok(self.tokenize_line_shared_with_validated(line, state, sink))
    }

    pub(crate) fn tokenize_line_shared_with_validated(
        &mut self,
        line: &str,
        state: &mut TokenizerState,
        sink: &mut impl SharedScopeSink,
    ) -> HighlightStatus {
        self.tokenize_line_with_validated(line, state, sink)
    }

    pub(crate) fn validate_line(&self, line: &str, state: &TokenizerState) -> Result<()> {
        if state.owner != self.id {
            return Err(Error::StateMismatch);
        }
        if line.contains('\n') {
            return Err(Error::InvalidLine);
        }
        Ok(())
    }

    fn tokenize_line_with_validated(
        &mut self,
        line: &str,
        state: &mut TokenizerState,
        sink: &mut impl SharedScopeSink,
    ) -> HighlightStatus {
        let line_index = state.anchor_line_index();
        let next_state = if self
            .inner
            .max_line_bytes()
            .is_some_and(|max_line_bytes| line.len() >= max_line_bytes)
        {
            // The parser adds one synthetic newline, so a line at the byte
            // limit is already too large. Skip it without filling the buffer.
            self.inner.tokenize_line_shared_scopes_skipped_with(
                line,
                state.inner.clone(),
                line_index,
                sink,
            )
        } else {
            self.parse_line_buffer.clear();
            self.parse_line_buffer.push_str(line);
            self.parse_line_buffer.push('\n');
            self.inner.tokenize_line_shared_scopes_with(
                &self.parse_line_buffer,
                state.inner.clone(),
                line_index,
                sink,
            )
        };
        state.finish_line(next_state);
        self.take_status()
    }

    /// Tokenizes a complete UTF-8 source document.
    pub fn tokenize(&mut self, source: &str) -> TokenizedDocument {
        let lines = self.inner.tokenize_source_output(source);
        TokenizedDocument {
            lines,
            status: self.take_status(),
        }
    }

    #[cfg(all(feature = "bundled-themes", any(feature = "html", feature = "ansi")))]
    pub(crate) fn tokenize_compact(
        &mut self,
        source: &str,
    ) -> (crate::HighlightedText, HighlightStatus) {
        let highlighted = self.inner.tokenize_source(source);
        (highlighted, self.take_status())
    }

    /// Creates an owned checkpoint table for viewport tokenization.
    pub fn checkpoints(&self, interval: usize) -> CheckpointTable {
        CheckpointTable {
            owner: self.id,
            inner: EngineCheckpointTable::new(interval),
        }
    }

    /// Tokenizes a line viewport while replaying from the nearest checkpoint.
    pub fn tokenize_viewport(
        &mut self,
        source: &str,
        visible_lines: Range<usize>,
        checkpoints: &mut CheckpointTable,
    ) -> Result<TokenizedDocument> {
        if checkpoints.owner != self.id {
            return Err(Error::StateMismatch);
        }
        let lines =
            self.inner
                .highlight_viewport_output(source, visible_lines, &mut checkpoints.inner);
        Ok(TokenizedDocument {
            lines,
            status: self.take_status(),
        })
    }

    fn take_status(&mut self) -> HighlightStatus {
        if self.inner.take_degraded() {
            HighlightStatus::Degraded
        } else {
            HighlightStatus::Complete
        }
    }

    #[cfg(feature = "diagnostics")]
    /// Enables or disables tokenizer diagnostic counters.
    pub fn set_diagnostics_enabled(&mut self, enabled: bool) {
        self.inner.set_counters_enabled(enabled);
    }

    #[cfg(feature = "diagnostics")]
    /// Returns accumulated diagnostic counters and resets them.
    pub fn take_diagnostics(&mut self) -> crate::diagnostics::EngineCounters {
        self.inner.take_counters()
    }
}

/// Opaque incremental continuation state.
///
/// [`Tokenizer::initial_state`] is document start, so `\A` may match on the
/// next tokenized line. A successful incremental line call clears that
/// document-start flag even when the rule stack stays empty.
///
/// Equality compares continuation context: equal states produce the same tokens
/// and next state for any following line. It includes the rule stack, captured
/// end/while delimiters, scopes, and document-start flag. States from different
/// tokenizers always compare unequal, including those sharing a
/// [`PreparedLanguage`]. Cloning preserves ownership.
///
/// Equality and hashing take constant time, independent of nesting depth, using
/// exact tokenizer-local interned identities. Cache population does not affect
/// equality. Hash values are not stable identifiers for storage or interchange.
///
/// An editor can retain the end-state of each line and stop re-highlighting once
/// it converges with the old state. Update the changed line's tokens before
/// stopping; only the unchanged suffix can reuse its old tokens. This example
/// replaces one line; insertions and deletions also require aligning old and new
/// line indices before comparing states.
///
/// ```
/// use syntaxmate::{GrammarRegistry, Tokenizer, TokenizerOptions};
///
/// let mut registry = GrammarRegistry::new();
/// let root = registry.add_json(r#"{
///     "scopeName": "source.example",
///     "patterns": [{"begin": "/\\*", "end": "\\*/", "name": "comment.block"}]
/// }"#)?;
/// let mut tokenizer = Tokenizer::new(&registry, root, TokenizerOptions::default())?;
/// let mut lines = ["/* comment", "inside", "*/", "unchanged"];
/// let mut state = tokenizer.initial_state();
/// let mut end_states = Vec::new();
/// let mut highlighted = Vec::new();
/// for line in &lines {
///     highlighted.push(tokenizer.tokenize_line(line, &mut state)?);
///     end_states.push(state.clone());
/// }
///
/// let edited_line = 1;
/// lines[edited_line] = "edited */";
/// let mut state = if edited_line == 0 {
///     tokenizer.initial_state()
/// } else {
///     end_states[edited_line - 1].clone()
/// };
/// for index in edited_line..lines.len() {
///     highlighted[index] = tokenizer.tokenize_line(lines[index], &mut state)?;
///     let converged = state == end_states[index];
///     end_states[index] = state.clone();
///     if converged {
///         break;
///     }
/// }
/// # Ok::<(), syntaxmate::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TokenizerState {
    // Owner qualifies all engine-local identities and immutable grammar/options.
    owner: u64,
    inner: EngineTokenizerState,
    // Empty stacks before and after the first line differ for the \A anchor.
    at_document_start: bool,
}

impl TokenizerState {
    /// Returns whether the continuation stack is empty; this does not imply document start.
    pub fn is_initial(&self) -> bool {
        self.inner.is_initial()
    }

    /// Returns the number of active continuation frames.
    pub fn depth(&self) -> usize {
        self.inner.depth()
    }

    fn anchor_line_index(&self) -> usize {
        usize::from(!self.at_document_start)
    }

    fn finish_line(&mut self, inner: EngineTokenizerState) {
        self.inner = inner;
        self.at_document_start = false;
    }
}

/// Checkpoints used to replay incremental state near a requested viewport.
#[derive(Debug, Clone)]
pub struct CheckpointTable {
    owner: u64,
    inner: EngineCheckpointTable,
}

impl CheckpointTable {
    /// Returns the checkpoint spacing in logical lines.
    pub fn interval(&self) -> usize {
        self.inner.interval()
    }

    /// Returns the number of retained checkpoints.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Invalidates checkpoints at and after the zero-based edited line.
    pub fn invalidate_from(&mut self, line_index: usize) {
        self.inner.invalidate_from(line_index);
    }
}

/// Whether configured safety budgets allowed complete tokenization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightStatus {
    /// Tokenization completed within configured resource limits.
    Complete,
    /// Some matching was skipped or stopped by resource limits.
    Degraded,
}

impl HighlightStatus {
    /// Returns whether tokenization completed within resource limits.
    pub fn is_complete(self) -> bool {
        self == Self::Complete
    }
}

/// Owned scope storage; each token keeps its scopes alive independently of its line.
#[derive(Debug, Clone)]
pub(crate) struct TokenScopes {
    // Filled by DocumentOutputLine::finish before document output leaves the engine.
    pub(crate) owner: Option<Arc<ScopeStorage>>,
    pub(crate) stack: ScopeStackId,
}

impl TokenScopes {
    pub(crate) fn view(&self) -> Scopes<'_> {
        Scopes {
            storage: self,
            position: 0,
        }
    }
}

impl PartialEq for TokenScopes {
    fn eq(&self, other: &Self) -> bool {
        self.view().eq(other.view())
    }
}

impl Eq for TokenScopes {}

/// A borrowed iterator over exact, ordered TextMate scope names.
///
/// Scope storage is shared with the owning token. Iteration allocates nothing
/// and works identically for whole-document and incremental output.
#[derive(Debug, Clone)]
pub struct Scopes<'a> {
    pub(crate) storage: &'a TokenScopes,
    position: usize,
}

impl<'a> Scopes<'a> {
    pub(crate) fn name(&self, index: usize) -> Option<&'a str> {
        let index = self.position + index;
        match self
            .storage
            .owner
            .as_deref()
            .expect("finished scope storage")
        {
            ScopeStorage::Table(table) => table
                .stack(self.storage.stack)
                .and_then(|atoms| atoms.get(index))
                .and_then(|atom| table.atom(*atom)),
            ScopeStorage::Shared(scopes) => scopes.get(index).map(AsRef::as_ref),
        }
    }
}

impl<'a> Iterator for Scopes<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        let name = self.name(0)?;
        self.position += 1;
        Some(name)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl ExactSizeIterator for Scopes<'_> {
    fn len(&self) -> usize {
        let total = match self
            .storage
            .owner
            .as_deref()
            .expect("finished scope storage")
        {
            ScopeStorage::Table(table) => table.stack(self.storage.stack).unwrap_or_default().len(),
            ScopeStorage::Shared(scopes) => scopes.len(),
        };
        total - self.position
    }
}

impl std::iter::FusedIterator for Scopes<'_> {}

/// One token with a line-relative UTF-8 byte range and exact ordered scopes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub(crate) range: Range<usize>,
    pub(crate) scopes: TokenScopes,
}

impl Token {
    /// Returns the line-relative UTF-8 byte range, excluding line terminators.
    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }

    /// Iterates scope names from outermost to innermost without allocating.
    pub fn scopes(&self) -> Scopes<'_> {
        self.scopes.view()
    }

    /// Returns the interned scope-stack key (including incremental output).
    ///
    /// Document keys are comparable only within the same document. Incremental
    /// keys are comparable across calls to the same tokenizer or session,
    /// including after reset, but not with document keys or other tokenizers.
    pub fn scope_stack(&self) -> Option<ScopeStackId> {
        Some(self.scopes.stack)
    }
}

/// One tokenized logical line, from a document or an incremental call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenizedLine {
    pub(crate) tokens: Vec<Token>,
    pub(crate) status: HighlightStatus,
}

impl TokenizedLine {
    /// Returns tokens in byte order.
    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    /// Reports whether this line was fully tokenized within resource limits.
    pub fn status(&self) -> HighlightStatus {
        self.status
    }
}

impl crate::engine::tokenizer::DocumentOutputLine for TokenizedLine {
    type ScopeOwner = Arc<ScopeStorage>;

    fn scope_owner(scopes: Arc<HighlightScopeTable>) -> Self::ScopeOwner {
        Arc::new(ScopeStorage::Table(scopes))
    }

    fn new(_: crate::LineTextFingerprint, capacity: usize, degraded: bool) -> Self {
        Self {
            tokens: Vec::with_capacity(capacity),
            status: if degraded {
                HighlightStatus::Degraded
            } else {
                HighlightStatus::Complete
            },
        }
    }

    fn push(&mut self, range: Range<usize>, _: Option<crate::SyntaxClass>, stack: ScopeStackId) {
        if let Some(last) = self.tokens.last_mut()
            && last.scope_stack() == Some(stack)
            && last.range.end == range.start
        {
            last.range.end = range.end;
            return;
        }
        self.tokens.push(Token {
            range,
            scopes: TokenScopes { owner: None, stack },
        });
    }

    fn finish(&mut self, scopes: &Self::ScopeOwner) {
        for token in &mut self.tokens {
            token.scopes.owner = Some(Arc::clone(scopes));
        }
    }
}

/// Tokenized logical lines and their aggregate completion status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenizedDocument {
    pub(crate) lines: Vec<TokenizedLine>,
    status: HighlightStatus,
}

impl TokenizedDocument {
    /// Returns logical lines in source order.
    pub fn lines(&self) -> &[TokenizedLine] {
        &self.lines
    }

    /// Reports whether the entire tokenization operation completed within limits.
    pub fn status(&self) -> HighlightStatus {
        self.status
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn equality_tokenizer(grammar: &str, line_cache_entries: usize) -> Tokenizer {
        let mut registry = GrammarRegistry::new();
        let root = registry.add_json(grammar).unwrap();
        Tokenizer::new(
            &registry,
            root,
            TokenizerOptions {
                line_cache_entries,
                ..TokenizerOptions::default()
            },
        )
        .unwrap()
    }

    fn state_hash(state: &TokenizerState) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        state.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn continuation_equality_includes_owner_and_document_start() {
        let grammar = r#"{"scopeName":"source.test","patterns":[
            {"match":"\\Afirst","name":"keyword.first"}
        ]}"#;
        let mut tokenizer = equality_tokenizer(grammar, 0);
        let other = equality_tokenizer(grammar, 0);
        let initial = tokenizer.initial_state();
        assert_eq!(initial, initial.clone());
        assert_ne!(initial, other.initial_state());
        let mut later = initial.clone();
        tokenizer.tokenize_line("", &mut later).unwrap();
        assert!(initial.is_initial() && later.is_initial());
        assert_ne!(initial, later);
        let mut first = initial.clone();
        assert_ne!(
            tokenizer.tokenize_line("first", &mut first).unwrap(),
            tokenizer.tokenize_line("first", &mut later).unwrap()
        );
        assert_eq!(first, later);
        assert_eq!(state_hash(&first), state_hash(&later));
    }

    #[test]
    fn continuation_equality_tracks_dynamic_end_and_while_delimiters() {
        for condition in ["end", "while"] {
            let grammar = format!(
                r#"{{"scopeName":"source.test","patterns":[{{
                    "begin":"^<<(.+)","{condition}":"^\\1$","name":"string.test"
                }}]}}"#
            );
            for cache_entries in [0, 1] {
                let mut tokenizer = equality_tokenizer(&grammar, cache_entries);
                let mut first = tokenizer.initial_state();
                let mut same = tokenizer.initial_state();
                let mut different = tokenizer.initial_state();
                tokenizer.tokenize_line("<<A.*[", &mut first).unwrap();
                tokenizer.tokenize_line("<<B", &mut different).unwrap();
                // Independently recreate the state after evicting the line cache.
                tokenizer.tokenize_line("<<A.*[", &mut same).unwrap();
                assert_eq!(first.depth(), 1);
                assert_eq!(first, same);
                assert_eq!(state_hash(&first), state_hash(&same));
                assert_ne!(first, different, "{condition}");
                let a = tokenizer.tokenize_line("A.*[", &mut first).unwrap();
                let b = tokenizer.tokenize_line("A.*[", &mut same).unwrap();
                tokenizer.tokenize_line("A.*[", &mut different).unwrap();
                assert_eq!(a, b);
                assert_eq!(first, same);
                assert_eq!(first.is_initial(), condition == "end");
                assert_eq!(different.is_initial(), condition == "while");
                assert_ne!(first, different);
                assert_ne!(
                    tokenizer.tokenize_line("A.*[", &mut first).unwrap(),
                    tokenizer.tokenize_line("A.*[", &mut different).unwrap(),
                    "the captured delimiter is literal, not regex"
                );
            }
        }
    }

    #[test]
    fn continuation_equality_tracks_captured_scopes_and_injections() {
        for scope_field in ["name", "contentName"] {
            let grammar = format!(
                r#"{{"scopeName":"source.test","patterns":[{{
                    "begin":"<(a|b)>","end":"!","{scope_field}":"meta.$1"
                }}],"injections":{{"L:meta.a":{{"patterns":[{{
                    "match":"word","name":"keyword.injected"
                }}]}}}}}}"#
            );
            let mut tokenizer = equality_tokenizer(&grammar, 0);
            let mut a = tokenizer.initial_state();
            let mut b = tokenizer.initial_state();
            tokenizer.tokenize_line("<a>", &mut a).unwrap();
            tokenizer.tokenize_line("<b>", &mut b).unwrap();
            assert_ne!(a, b, "{scope_field}");
            let a_line = tokenizer.tokenize_line("word", &mut a).unwrap();
            let b_line = tokenizer.tokenize_line("word", &mut b).unwrap();
            assert!(
                a_line
                    .tokens()
                    .iter()
                    .any(|token| { token.scopes().any(|scope| scope == "keyword.injected") })
            );
            assert!(
                b_line
                    .tokens()
                    .iter()
                    .all(|token| { token.scopes().all(|scope| scope != "keyword.injected") })
            );
            tokenizer.tokenize_line("!", &mut a).unwrap();
            tokenizer.tokenize_line("!", &mut b).unwrap();
            assert_eq!(a, b);
        }
    }

    #[test]
    fn edited_comment_converges_when_original_comment_closes() {
        let mut tokenizer = equality_tokenizer(
            r#"{"scopeName":"source.test","patterns":[
                {"begin":"/\\*","end":"\\*/","name":"comment.block"}
            ]}"#,
            0,
        );
        let lines = ["/* open", "body", "*/", "suffix"];
        let mut old_state = tokenizer.initial_state();
        let mut old_states = Vec::new();
        let mut old_tokens = Vec::new();
        for line in lines {
            old_tokens.push(tokenizer.tokenize_line(line, &mut old_state).unwrap());
            old_states.push(old_state.clone());
        }
        let mut edited = old_states[0].clone();
        tokenizer.tokenize_line("body */", &mut edited).unwrap();
        assert_ne!(edited, old_states[1]);
        let closing = tokenizer.tokenize_line(lines[2], &mut edited).unwrap();
        assert_ne!(
            closing, old_tokens[2],
            "replace tokens even on the convergence line"
        );
        assert_eq!(edited, old_states[2]);
        assert_eq!(
            tokenizer.tokenize_line(lines[3], &mut edited).unwrap(),
            old_tokens[3]
        );
        assert_eq!(edited, old_states[3]);
        assert_eq!(state_hash(&edited), state_hash(&old_states[3]));
    }

    #[test]
    fn prepared_language_creates_independent_equivalent_tokenizers() {
        let mut registry = GrammarRegistry::new();
        let root = registry
            .add_json(
                r#"{"scopeName":"source.test","patterns":[{"match":"true","name":"constant.language.test"}]}"#,
            )
            .unwrap();
        let prepared = PreparedLanguage::new(&registry, root).unwrap();
        let initial_stats = prepared.stats();
        assert_eq!(initial_stats.grammar_count(), 1);
        assert_eq!(initial_stats.static_pattern_capacity(), 1);
        assert_eq!(initial_stats.compiled_pattern_count(), 1);
        assert!(initial_stats.static_pattern_retained_bytes() > 0);
        assert!(
            initial_stats.static_pattern_retained_bytes()
                <= initial_stats.static_pattern_byte_capacity()
        );
        assert_eq!(initial_stats.static_candidate_capacity(), 1_024);
        assert_eq!(initial_stats.static_candidate_count(), 1);
        assert!(
            initial_stats.static_candidate_retained_bytes()
                <= initial_stats.static_candidate_byte_capacity()
        );

        // The prepared value is a snapshot. Later registry mutations do not
        // alter tokenizers made from it.
        registry
            .add_json(r#"{"scopeName":"source.other","patterns":[]}"#)
            .unwrap();
        assert_eq!(prepared.stats().grammar_count(), 1);

        let mut first = prepared.tokenizer(TokenizerOptions::default());
        let mut second = Tokenizer::from_prepared(&prepared, TokenizerOptions::default());
        assert_eq!(first.tokenize("true false"), second.tokenize("true false"));

        let mut first_state = first.initial_state();
        assert_ne!(first_state, second.initial_state());
        assert_eq!(
            second.tokenize_line("true", &mut first_state),
            Err(Error::StateMismatch)
        );
    }

    #[test]
    fn prepared_language_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<PreparedLanguage>();
    }

    #[test]
    fn prepared_language_handles_concurrent_first_use() {
        let mut registry = GrammarRegistry::new();
        let root = registry
            .add_json(
                r#"{
                    "scopeName":"source.concurrent-prepared",
                    "patterns":[{
                        "begin":"\"",
                        "end":"\"",
                        "name":"string.concurrent-prepared",
                        "patterns":[{"match":"[a-z]+","name":"word.concurrent-prepared"}]
                    }]
                }"#,
            )
            .unwrap();
        let prepared = PreparedLanguage::new(&registry, root).unwrap();
        let barrier = Arc::new(std::sync::Barrier::new(4));
        let outputs = std::thread::scope(|scope| {
            (0..4)
                .map(|_| {
                    let prepared = prepared.clone();
                    let barrier = Arc::clone(&barrier);
                    scope.spawn(move || {
                        let mut tokenizer = prepared.tokenizer(TokenizerOptions::default());
                        barrier.wait();
                        tokenizer.tokenize("\"word\"")
                    })
                })
                .collect::<Vec<_>>()
                .into_iter()
                .map(|thread| thread.join().unwrap())
                .collect::<Vec<_>>()
        });

        assert!(outputs.windows(2).all(|pair| pair[0] == pair[1]));
        let stats = prepared.stats();
        assert!(stats.compiled_pattern_count() <= stats.static_pattern_capacity());
        assert!(stats.static_pattern_retained_bytes() <= stats.static_pattern_byte_capacity());
        assert!(stats.static_candidate_count() <= stats.static_candidate_capacity());
        assert!(stats.static_candidate_retained_bytes() <= stats.static_candidate_byte_capacity());
    }

    #[test]
    fn reusable_and_callback_line_apis_match_owned_output() {
        let mut registry = GrammarRegistry::new();
        let root = registry
            .add_json(
                r#"{
                    "scopeName":"source.sink-test",
                    "patterns":[{
                        "begin":"\"",
                        "end":"\"",
                        "name":"string.sink-test"
                    },{
                        "match":"\\btrue\\b",
                        "name":"constant.sink-test"
                    }]
                }"#,
            )
            .unwrap();
        let mut owned = Tokenizer::new(&registry, root, TokenizerOptions::default()).unwrap();
        let mut reusable = Tokenizer::new(&registry, root, TokenizerOptions::default()).unwrap();
        let mut callback = Tokenizer::new(&registry, root, TokenizerOptions::default()).unwrap();
        let mut owned_state = owned.initial_state();
        let mut reusable_state = reusable.initial_state();
        let mut callback_state = callback.initial_state();
        let mut buffer = Vec::new();

        for line in ["true \"open", "inside\" true", "plain"] {
            let expected = owned.tokenize_line(line, &mut owned_state).unwrap();
            let status = reusable
                .tokenize_line_into(line, &mut reusable_state, &mut buffer)
                .unwrap();
            let mut emitted = Vec::new();
            let callback_status = callback
                .tokenize_line_with(line, &mut callback_state, |token| emitted.push(token))
                .unwrap();

            assert_eq!(status, expected.status());
            assert_eq!(callback_status, expected.status());
            assert_eq!(buffer, expected.tokens());
            assert_eq!(emitted, expected.tokens());
        }

        let capacity = buffer.capacity();
        let snapshot = buffer.clone();
        assert_eq!(
            reusable
                .tokenize_line_into("invalid\nline", &mut reusable_state, &mut buffer)
                .unwrap_err(),
            Error::InvalidLine
        );
        assert_eq!(
            buffer, snapshot,
            "validation errors leave the sink untouched"
        );
        assert_eq!(buffer.capacity(), capacity);
    }

    #[test]
    fn rejected_incremental_lines_do_not_grow_parse_buffer() {
        let mut registry = GrammarRegistry::new();
        let root = registry
            .add_json(r#"{"scopeName":"source.test","patterns":[]}"#)
            .unwrap();
        let options = TokenizerOptions {
            max_line_bytes: 8,
            ..TokenizerOptions::default()
        };
        let mut tokenizer = Tokenizer::new(&registry, root, options).unwrap();
        let mut state = tokenizer.initial_state();
        let initial_capacity = tokenizer.parse_line_buffer.capacity();

        for line in ["x".repeat(options.max_line_bytes), "x".repeat(64 * 1024)] {
            let tokenized = tokenizer.tokenize_line(&line, &mut state).unwrap();

            assert_eq!(tokenized.status(), HighlightStatus::Degraded);
            assert!(state.is_initial());
            assert_eq!(tokenizer.parse_line_buffer.capacity(), initial_capacity);
            assert_eq!(tokenized.tokens()[0].range(), 0..line.len());
        }
    }

    fn token_scopes(tokens: &[Token]) -> Vec<(std::ops::Range<usize>, Vec<String>)> {
        tokens
            .iter()
            .map(|token| (token.range(), token.scopes().map(str::to_owned).collect()))
            .collect()
    }

    fn document_line_scopes(line: &TokenizedLine) -> Vec<(std::ops::Range<usize>, Vec<String>)> {
        line.tokens()
            .iter()
            .map(|span| (span.range(), span.scopes().map(str::to_owned).collect()))
            .collect()
    }

    #[test]
    fn incremental_text_start_anchor_matches_only_document_start() {
        let grammar = r#"{
            "scopeName": "source.seed",
            "patterns": [
                {"match": "\\A(let|fn)\\b", "name": "keyword.anchor"}
            ]
        }"#;
        let mut registry = GrammarRegistry::new();
        let root = registry.add_json(grammar).unwrap();
        let options = TokenizerOptions::default();

        let mut complete = Tokenizer::new(&registry, root, options).unwrap();
        let document = complete.tokenize("fn foo\nfn bar\n");
        assert!(
            document_line_scopes(&document.lines()[0])
                .iter()
                .any(|(_, scopes)| scopes.iter().any(|scope| scope == "keyword.anchor")),
            "{:#?}",
            document_line_scopes(&document.lines()[0])
        );
        assert!(
            document_line_scopes(&document.lines()[1])
                .iter()
                .all(|(_, scopes)| scopes.iter().all(|scope| scope != "keyword.anchor")),
            "{:#?}",
            document_line_scopes(&document.lines()[1])
        );

        let mut incremental = Tokenizer::new(&registry, root, options).unwrap();
        let mut state = incremental.initial_state();
        let first = incremental.tokenize_line("fn foo", &mut state).unwrap();
        let second = incremental.tokenize_line("fn bar", &mut state).unwrap();
        assert_eq!(
            token_scopes(first.tokens()),
            document_line_scopes(&document.lines()[0])
        );
        assert_eq!(
            token_scopes(second.tokens()),
            document_line_scopes(&document.lines()[1])
        );

        let mut replay = Tokenizer::new(&registry, root, options).unwrap();
        let mut replay_state = replay.initial_state();
        assert_eq!(
            replay
                .tokenize_line("bad\nline", &mut replay_state)
                .unwrap_err(),
            Error::InvalidLine
        );
        let after_reject = replay.tokenize_line("fn foo", &mut replay_state).unwrap();
        assert_eq!(
            token_scopes(after_reject.tokens()),
            token_scopes(first.tokens())
        );

        let mut skipped = Tokenizer::new(
            &registry,
            root,
            TokenizerOptions {
                max_line_bytes: 8,
                ..TokenizerOptions::default()
            },
        )
        .unwrap();
        let mut skipped_state = skipped.initial_state();
        let long = skipped
            .tokenize_line("too long!", &mut skipped_state)
            .unwrap();
        assert_eq!(long.status(), HighlightStatus::Degraded);
        let after_skip = skipped.tokenize_line("fn x", &mut skipped_state).unwrap();
        assert!(
            token_scopes(after_skip.tokens())
                .iter()
                .all(|(_, scopes)| scopes.iter().all(|scope| scope != "keyword.anchor")),
            "{:#?}",
            token_scopes(after_skip.tokens())
        );
    }
}
