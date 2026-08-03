use std::{ops::Range, sync::Arc};

use crate::engine::checkpoint::CheckpointTable as EngineCheckpointTable;

use crate::{
    Error, HighlightScopeTable, Result, ScopeStackRef, TokenizerOptions,
    engine::tokenizer::{
        GrammarSet as EngineGrammarSet, TextMateTokenizer, TokenizerState as EngineTokenizerState,
    },
};

static NEXT_REGISTRY_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
static NEXT_TOKENIZER_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

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
        let mut inner = TextMateTokenizer::new(registry.inner.clone(), root.inner);
        inner.configure_options(options);
        Ok(Self {
            id: NEXT_TOKENIZER_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            inner,
            parse_line_buffer: String::new(),
        })
    }

    /// Constructs a tokenizer from the bundled grammar catalog.
    #[cfg(feature = "bundled-grammars")]
    pub fn for_bundled_language(language: &str, options: TokenizerOptions) -> Result<Self> {
        let canonical = crate::grammars::canonical_language(language)
            .ok_or_else(|| Error::UnknownLanguage(language.to_owned()))?;
        let (grammars, root) = crate::engine::load_grammar_set(&canonical)?;
        let mut inner = TextMateTokenizer::new(grammars, root);
        inner.configure_options(options);
        Ok(Self {
            id: NEXT_TOKENIZER_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            inner,
            parse_line_buffer: String::new(),
        })
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
        if state.owner != self.id {
            return Err(Error::StateMismatch);
        }
        if line.contains('\n') {
            return Err(Error::InvalidLine);
        }

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
            .filter_map(|token| {
                let start = token.range.start.min(line.len());
                let end = token.range.end.min(line.len());
                (start < end && line.is_char_boundary(start) && line.is_char_boundary(end))
                    .then_some(ScopedToken {
                        range: start..end,
                        scopes: token.scopes,
                    })
            })
            .collect();
        let status = if self.inner.take_degraded() {
            HighlightStatus::Degraded
        } else {
            HighlightStatus::Complete
        };
        Ok(TokenizedLine { tokens, status })
    }

    /// Tokenizes a complete UTF-8 source document.
    pub fn tokenize(&mut self, source: &str) -> TokenizedDocument {
        let highlighted = self.inner.tokenize_source(source);
        self.finish_document(highlighted)
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
        Ok(self.finish_document(highlighted))
    }

    fn finish_document(&mut self, highlighted: crate::HighlightedText) -> TokenizedDocument {
        let status = if self.inner.take_degraded() {
            HighlightStatus::Degraded
        } else {
            HighlightStatus::Complete
        };
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

    #[cfg(feature = "bundled-grammars")]
    pub(crate) fn into_parts(self) -> (Range<usize>, Arc<[Arc<str>]>) {
        (self.range, self.scopes)
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

    #[cfg(feature = "bundled-grammars")]
    pub(crate) fn into_parts(self) -> (Vec<ScopedToken>, HighlightStatus) {
        (self.tokens, self.status)
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
