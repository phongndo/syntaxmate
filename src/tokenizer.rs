use std::{ops::Range, sync::Arc};

use crate::engine::checkpoint::CheckpointTable as EngineCheckpointTable;

use crate::{
    Error, HighlightScopeTable, Result, ScopeStackRef, TokenizerOptions,
    engine::state::ScopeStackId,
    engine::tokenizer::{
        GrammarSet as EngineGrammarSet, PreparedLanguage as EnginePreparedLanguage,
        SharedScopeSink, TextMateTokenizer, TokenizerState as EngineTokenizerState,
    },
};

static NEXT_REGISTRY_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
static NEXT_TOKENIZER_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

struct ScopedTokenVecSink<'a> {
    line: &'a str,
    tokens: &'a mut Vec<ScopedToken>,
}

impl SharedScopeSink for ScopedTokenVecSink<'_> {
    fn reserve(&mut self, token_count: usize) {
        if self.tokens.capacity() == 0 {
            *self.tokens = Vec::with_capacity(token_count);
        } else if self.tokens.capacity() < token_count {
            self.tokens.reserve(token_count);
        }
    }

    fn push(&mut self, range: Range<usize>, _stack: ScopeStackId, scopes: Arc<[Arc<str>]>) {
        if let Some(token) = scoped_token(self.line, range, scopes) {
            self.tokens.push(token);
        }
    }
}

struct ScopedTokenCallbackSink<'a, F> {
    line: &'a str,
    callback: F,
}

impl<F: FnMut(ScopedToken)> SharedScopeSink for ScopedTokenCallbackSink<'_, F> {
    fn reserve(&mut self, _token_count: usize) {}

    fn push(&mut self, range: Range<usize>, _stack: ScopeStackId, scopes: Arc<[Arc<str>]>) {
        if let Some(token) = scoped_token(self.line, range, scopes) {
            (self.callback)(token);
        }
    }
}

fn scoped_token(line: &str, range: Range<usize>, scopes: Arc<[Arc<str>]>) -> Option<ScopedToken> {
    let start = range.start.min(line.len());
    let end = range.end.min(line.len());
    (start < end && line.is_char_boundary(start) && line.is_char_boundary(end)).then_some(
        ScopedToken {
            range: start..end,
            scopes,
        },
    )
}

/// Resource limits applied while constructing a custom grammar registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrammarLimits {
    pub max_grammar_bytes: usize,
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
    pub fn new() -> Self {
        Self::with_limits(GrammarLimits::default())
    }

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
            return Err(Error::Grammar(format!(
                "grammar is {} bytes, exceeding the {} byte limit",
                json.len(),
                self.limits.max_grammar_bytes
            )));
        }
        if self.inner.grammars().len() >= self.limits.max_grammars {
            return Err(Error::Grammar(format!(
                "grammar registry reached its {} grammar limit",
                self.limits.max_grammars
            )));
        }
        let id = self
            .inner
            .load_and_add(json)
            .map_err(|error| Error::Grammar(error.to_string()))?;
        Ok(GrammarId {
            registry: self.id,
            inner: id,
        })
    }

    pub fn grammar_count(&self) -> usize {
        self.inner.grammars().len()
    }

    /// Validates local and external include references across the registry.
    pub fn validate(&self) -> Result<()> {
        self.inner
            .validate_include_graph()
            .map_err(|error| Error::Grammar(error.to_string()))
    }
}

/// Opaque identity of a grammar in one [`GrammarRegistry`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GrammarId {
    registry: u64,
    inner: crate::engine::state::GrammarId,
}

fn preparation_limit_error(detail: &str) -> Error {
    Error::Grammar(format!(
        "grammar exceeds PreparedLanguage preparation bounds ({detail}); use Tokenizer directly"
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
            return Err(Error::Grammar(
                "root grammar does not belong to this registry".to_owned(),
            ));
        }
        let inner = EnginePreparedLanguage::try_new(registry.inner.clone(), root.inner)
            .map_err(preparation_limit_error)?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Prepares one language from the bundled grammar catalog.
    ///
    /// Bundled grammars are checked to remain inside the preparation bounds.
    #[cfg(feature = "bundled-grammars")]
    pub fn for_bundled_language(language: &str) -> Result<Self> {
        let canonical = crate::grammars::canonical_language(language)
            .ok_or_else(|| Error::UnknownLanguage(language.to_owned()))?;
        let (grammars, root) = crate::engine::load_grammar_set(&canonical)?;
        let inner =
            EnginePreparedLanguage::try_new(grammars, root).map_err(preparation_limit_error)?;
        Ok(Self {
            inner: Arc::new(inner),
        })
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
    pub fn new(
        registry: &GrammarRegistry,
        root: GrammarId,
        options: TokenizerOptions,
    ) -> Result<Self> {
        if root.registry != registry.id || registry.inner.grammar(root.inner).is_none() {
            return Err(Error::Grammar(
                "root grammar does not belong to this registry".to_owned(),
            ));
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
    pub fn initial_state(&self) -> TokenizerState {
        TokenizerState {
            owner: self.id,
            inner: EngineTokenizerState::default(),
        }
    }

    /// Tokenizes one logical line. `line` must not include a newline terminator.
    pub fn tokenize_line(
        &mut self,
        line: &str,
        state: &mut TokenizerState,
    ) -> Result<TokenizedLine> {
        self.validate_line(line, state)?;
        let tokenized = if self
            .inner
            .max_line_bytes()
            .is_some_and(|max_line_bytes| line.len() >= max_line_bytes)
        {
            // The parser adds one synthetic newline, so a line at the byte
            // limit is already too large. Skip it without filling the buffer.
            self.inner
                .tokenize_line_shared_scopes_skipped(line, state.inner.clone())
        } else {
            self.parse_line_buffer.clear();
            self.parse_line_buffer.push_str(line);
            self.parse_line_buffer.push('\n');
            self.inner
                .tokenize_line_shared_scopes(&self.parse_line_buffer, state.inner.clone())
        };
        state.inner = tokenized.state;
        let tokens = tokenized
            .tokens
            .into_iter()
            .filter_map(|token| scoped_token(line, token.range, token.scopes))
            .collect();
        Ok(TokenizedLine {
            tokens,
            status: self.take_status(),
        })
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
        tokens: &mut Vec<ScopedToken>,
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
        sink: impl FnMut(ScopedToken),
    ) -> Result<HighlightStatus> {
        self.validate_line(line, state)?;
        let mut sink = ScopedTokenCallbackSink {
            line,
            callback: sink,
        };
        Ok(self.tokenize_line_with_validated(line, state, &mut sink))
    }

    #[cfg(feature = "bundled-grammars")]
    pub(crate) fn tokenize_line_shared_with(
        &mut self,
        line: &str,
        state: &mut TokenizerState,
        sink: &mut impl SharedScopeSink,
    ) -> Result<HighlightStatus> {
        self.validate_line(line, state)?;
        Ok(self.tokenize_line_shared_with_validated(line, state, sink))
    }

    #[cfg(feature = "bundled-grammars")]
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
        let next_state = if self
            .inner
            .max_line_bytes()
            .is_some_and(|max_line_bytes| line.len() >= max_line_bytes)
        {
            // The parser adds one synthetic newline, so a line at the byte
            // limit is already too large. Skip it without filling the buffer.
            self.inner
                .tokenize_line_shared_scopes_skipped_with(line, state.inner.clone(), sink)
        } else {
            self.parse_line_buffer.clear();
            self.parse_line_buffer.push_str(line);
            self.parse_line_buffer.push('\n');
            self.inner.tokenize_line_shared_scopes_with(
                &self.parse_line_buffer,
                state.inner.clone(),
                sink,
            )
        };
        state.inner = next_state;
        self.take_status()
    }

    /// Tokenizes a complete UTF-8 source document.
    pub fn tokenize(&mut self, source: &str) -> TokenizedDocument {
        let (highlighted, status) = self.tokenize_compact(source);
        self.finish_document(highlighted, status)
    }

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
        let highlighted =
            self.inner
                .highlight_viewport(source, visible_lines, &mut checkpoints.inner);
        let status = self.take_status();
        Ok(self.finish_document(highlighted, status))
    }

    fn finish_document(
        &mut self,
        highlighted: crate::HighlightedText,
        status: HighlightStatus,
    ) -> TokenizedDocument {
        let lines = highlighted
            .lines
            .into_iter()
            .map(|line| DocumentLine {
                spans: line
                    .segments
                    .into_iter()
                    .map(|segment| TokenSpan {
                        range: segment.byte_start..segment.byte_end,
                        scope_stack: segment.scope_stack,
                    })
                    .collect(),
                scopes: line.scope_table,
            })
            .collect();
        TokenizedDocument { lines, status }
    }

    fn take_status(&mut self) -> HighlightStatus {
        if self.inner.take_degraded() {
            HighlightStatus::Degraded
        } else {
            HighlightStatus::Complete
        }
    }

    #[cfg(feature = "diagnostics")]
    pub fn set_diagnostics_enabled(&mut self, enabled: bool) {
        self.inner.set_counters_enabled(enabled);
    }

    #[cfg(feature = "diagnostics")]
    pub fn take_diagnostics(&mut self) -> crate::diagnostics::EngineCounters {
        self.inner.take_counters()
    }
}

/// Opaque incremental continuation state.
#[derive(Debug, Clone)]
pub struct TokenizerState {
    owner: u64,
    inner: EngineTokenizerState,
}

impl TokenizerState {
    pub fn is_initial(&self) -> bool {
        self.inner.is_initial()
    }

    pub fn depth(&self) -> usize {
        self.inner.depth()
    }
}

/// Checkpoints used to replay incremental state near a requested viewport.
#[derive(Debug, Clone)]
pub struct CheckpointTable {
    owner: u64,
    inner: EngineCheckpointTable,
}

impl CheckpointTable {
    pub fn interval(&self) -> usize {
        self.inner.interval()
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn invalidate_from(&mut self, line_index: usize) {
        self.inner.invalidate_from(line_index);
    }
}

/// Whether configured safety budgets allowed complete tokenization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightStatus {
    Complete,
    Degraded,
}

impl HighlightStatus {
    pub fn is_complete(self) -> bool {
        self == Self::Complete
    }
}

/// One token from the incremental line API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedToken {
    range: Range<usize>,
    scopes: Arc<[Arc<str>]>,
}

impl ScopedToken {
    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }

    pub fn scopes(&self) -> impl ExactSizeIterator<Item = &str> {
        self.scopes.iter().map(AsRef::as_ref)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenizedLine {
    tokens: Vec<ScopedToken>,
    status: HighlightStatus,
}

impl TokenizedLine {
    pub fn tokens(&self) -> &[ScopedToken] {
        &self.tokens
    }

    pub fn status(&self) -> HighlightStatus {
        self.status
    }
}

/// One exact-scope span in a complete tokenized document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenSpan {
    range: Range<usize>,
    scope_stack: ScopeStackRef,
}

impl TokenSpan {
    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }

    pub fn scope_stack(&self) -> ScopeStackRef {
        self.scope_stack
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentLine {
    spans: Vec<TokenSpan>,
    scopes: Arc<HighlightScopeTable>,
}

impl DocumentLine {
    pub fn spans(&self) -> &[TokenSpan] {
        &self.spans
    }

    pub fn scope_names(&self, stack: ScopeStackRef) -> impl Iterator<Item = &str> {
        self.scopes.stack_names(stack)
    }

    pub fn scope_table(&self) -> &Arc<HighlightScopeTable> {
        &self.scopes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenizedDocument {
    lines: Vec<DocumentLine>,
    status: HighlightStatus,
}

impl TokenizedDocument {
    pub fn lines(&self) -> &[DocumentLine] {
        &self.lines
    }

    pub fn status(&self) -> HighlightStatus {
        self.status
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
