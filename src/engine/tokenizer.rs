use std::{
    cell::RefCell,
    collections::BTreeMap,
    collections::HashMap,
    collections::HashSet,
    hash::{BuildHasherDefault, Hash, Hasher},
    ops::{Deref, Range},
    sync::{
        Arc, Condvar, Mutex, OnceLock, Weak,
        atomic::{AtomicUsize, Ordering},
    },
    time::Instant,
};

use crate::{
    EngineHighlightedLine as HighlightedLine, HighlightScopeTable, HighlightedText,
    LineTextFingerprint, ScopeAtomId, ScopeStackRef, SyntaxClass, SyntaxSegment,
};

use super::cache::{CachedLine, LineCache, LineCacheKey};
use super::checkpoint::CheckpointTable;
use super::counters::{EngineCounters, PatternHotspot};
use super::grammar::{
    CaptureSpec, CompiledGrammar, GrammarLoadError, GrammarValidationError, InjectionPriority,
    RuleBody, RuleRef, load_dev_grammar_from_str, normalize_injection_selectors,
};
use super::hashing::{self, FastMap};
use super::line::{LineChunks, next_char_boundary};
use super::regex::captures::substitute_end_pattern;
use super::regex::{
    AnchorContext, CompiledPattern, FallbackError, MatchResult, PatternSetMatcher, RegexMatcher,
};
use super::scopes::{ScopeInterner, ScopeStackInterner, ScopeTemplateId, ScopeTemplateInterner};
use super::state::{GrammarId, LineTokens, PatternId, RuleId, ScopeId, ScopeStackId, StateId};

const MAX_INCLUDE_DEPTH: usize = 128;
const MAX_PREPARED_GRAMMAR_WALK_STATES: usize = 1_048_576;
const MAX_PREPARED_GRAMMAR_WALK_BYTES: usize = 32 * 1024 * 1024;
const MAX_PREPARED_GRAMMAR_PENDING_REFS: usize = 262_144;
const MAX_PREPARED_GRAMMAR_PENDING_BYTES: usize = 32 * 1024 * 1024;
const MAX_TOKENIZER_STEPS_PER_LINE: usize = 20_000;
const MAX_FALLBACK_STEPS_PER_LINE: u64 = 2_000_000;
const MIN_FALLBACK_STEPS_PER_CALL: u64 = 10_000_000;
const FALLBACK_STEPS_PER_SOURCE_BYTE: u64 = 512;
const MAX_SUBSTITUTED_END_PATTERN_LEN: usize = 4096;
const MAX_DYNAMIC_MATCHERS: usize = 512;
const MAX_INLINE_CANDIDATE_SETS: usize = 1024;
const MAX_CANDIDATE_SETS: usize = 4096;
const MAX_CANDIDATE_BLUEPRINTS: usize = 1024;
const MAX_PREPARED_BLUEPRINT_KEY_BYTES: usize = 1024 * 1024;
const MAX_PREPARED_BLUEPRINT_BYTES: usize = 8 * 1024 * 1024;
const MAX_PREPARED_PATTERN_SLOT_BYTES: usize = 1024 * 1024;
const MAX_PREPARED_PATTERN_BYTES: usize = 64 * 1024 * 1024;
const MAX_INJECTION_OUTCOMES: usize = 1024;
const MAX_PREPARED_INJECTION_OUTCOME_BYTES: usize = 4 * 1024 * 1024;
const MAX_PREPARED_CANDIDATE_BYTES: usize =
    MAX_PREPARED_BLUEPRINT_BYTES + MAX_PREPARED_INJECTION_OUTCOME_BYTES;
const MAX_SCOPE_STACK_CACHE_ENTRIES: usize = 8192;
const MAX_FRAME_NODE_CACHE_ENTRIES: usize = 16384;
const MAX_OUTPUT_SCOPE_TABLES: usize = 512;
const MAX_CAPTURE_RESULT_POOL_ENTRIES: usize = 16;
const MAX_POOLED_CAPTURE_CAPACITY: usize = 1024;

#[derive(Debug, Default)]
pub struct Tokenizer;

impl Tokenizer {
    pub fn new() -> Self {
        Self
    }

    pub fn tokenize_line(&mut self, line: &str, entry: StateId) -> LineTokens {
        // Compatibility seam retained for early engine tests. The real TextMate
        // tokenizer is `TextMateTokenizer` below.
        let tokens = if line.is_empty() {
            Vec::new()
        } else {
            vec![(0..line.len(), ScopeStackId::default())]
        };
        LineTokens {
            tokens,
            exit: entry,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedToken {
    pub range: Range<usize>,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SharedScopedToken {
    pub(crate) range: Range<usize>,
    pub(crate) scopes: Arc<[Arc<str>]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompactScopedToken {
    pub(crate) range: Range<usize>,
    pub(crate) stack: ScopeStackId,
}

pub(crate) trait SharedScopeSink {
    fn reserve(&mut self, token_count: usize);
    fn push(&mut self, range: Range<usize>, stack: ScopeStackId, scopes: Arc<[Arc<str>]>);
}

#[derive(Debug, Default, Clone)]
struct OutputScopeTableCache {
    tables: FastMap<Vec<ScopeStackId>, Weak<HighlightScopeTable>>,
}

struct OutputScopeTableBuilder {
    engine_to_output: FastMap<ScopeStackId, ScopeStackRef>,
    output_stacks: Vec<ScopeStackId>,
}

impl OutputScopeTableBuilder {
    fn new() -> Self {
        let mut engine_to_output = hashing::fast_map();
        engine_to_output.insert(ScopeStackId::default(), ScopeStackRef::default());
        Self {
            engine_to_output,
            output_stacks: vec![ScopeStackId::default()],
        }
    }

    fn intern_engine_stack(&mut self, stack: ScopeStackId) -> ScopeStackRef {
        if let Some(output) = self.engine_to_output.get(&stack) {
            return *output;
        }
        let output = ScopeStackRef(self.output_stacks.len() as u32);
        self.output_stacks.push(stack);
        self.engine_to_output.insert(stack, output);
        output
    }

    fn finish(
        self,
        scope_stacks: &ScopeStackInterner,
        scope_names: &ScopeInterner,
        cache: &mut OutputScopeTableCache,
    ) -> Arc<HighlightScopeTable> {
        if let Some(table) = cache
            .tables
            .get(self.output_stacks.as_slice())
            .and_then(Weak::upgrade)
        {
            return table;
        }

        let mut stacks = Vec::with_capacity(self.output_stacks.len());
        let mut atoms = Vec::<Arc<str>>::new();
        let mut atom_ids = hashing::fast_map();
        let mut resolved_ids = Vec::new();
        for engine_stack in &self.output_stacks {
            scope_stacks.resolve_ids_into(*engine_stack, &mut resolved_ids);
            let stack_atoms = resolved_ids
                .iter()
                .map(|&scope| {
                    if let Some(atom) = atom_ids.get(&scope) {
                        return *atom;
                    }
                    let name = scope_names
                        .get_arc(scope)
                        .expect("scope-stack IDs come from the scope interner");
                    let atom = ScopeAtomId(atoms.len() as u32);
                    atoms.push(name);
                    atom_ids.insert(scope, atom);
                    atom
                })
                .collect::<Arc<[ScopeAtomId]>>();
            stacks.push(stack_atoms);
        }
        let table = Arc::new(HighlightScopeTable::from_parts(atoms, stacks));
        if cache.tables.len() >= MAX_OUTPUT_SCOPE_TABLES {
            cache.tables.clear();
        }
        cache
            .tables
            .insert(self.output_stacks, Arc::downgrade(&table));
        table
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenizedLine {
    pub tokens: Arc<[ScopedToken]>,
    pub state: TokenizerState,
    pub entry_state_id: StateId,
    pub exit_state_id: StateId,
}

#[derive(Debug, Clone)]
pub(crate) struct SharedTokenizedLine {
    pub(crate) tokens: Vec<SharedScopedToken>,
    pub(crate) state: TokenizerState,
}

#[derive(Debug, Clone)]
struct CompactTokenizedLine {
    tokens: CompactLineTokens,
    state: TokenizerState,
    entry_state_id: StateId,
    exit_state_id: StateId,
    parse_fingerprint: LineTextFingerprint,
}

#[derive(Debug, Clone)]
enum CompactLineTokens {
    Owned(Vec<CompactScopedToken>),
    Shared(Arc<[CompactScopedToken]>),
}

impl Deref for CompactLineTokens {
    type Target = [CompactScopedToken];

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Owned(tokens) => tokens,
            Self::Shared(tokens) => tokens,
        }
    }
}

impl From<Vec<CompactScopedToken>> for CompactLineTokens {
    fn from(tokens: Vec<CompactScopedToken>) -> Self {
        Self::Owned(tokens)
    }
}

#[derive(Debug, Clone, Default)]
pub struct TokenizerState {
    // Parent-linked immutable chunks keep continuation updates bounded. Pushes
    // copy at most one 32-frame tail chunk instead of cloning every frame
    // pointer in a deep stack; a hash-consed stack id keeps equality exact and
    // O(1) even when equal states were built independently.
    frames: FrameStack,
    interner_hash: u64,
}

impl TokenizerState {
    pub fn is_initial(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn depth(&self) -> usize {
        self.frames.len()
    }

    pub fn state_id(&self) -> StateId {
        StateId(
            self.frames
                .last()
                .map_or(0x811c9dc5, |frame| frame.state_hash),
        )
    }

    fn refresh_interner_hash(&mut self) {
        self.interner_hash = u64::from(self.frames.interned_id().0);
    }

    /// Pushes a frame while maintaining the per-frame identity hash and the
    /// cumulative state hash in O(1), instead of re-hashing every frame on
    /// each state change (quadratic for deeply nested sources).
    #[cfg(test)]
    fn push_frame(&mut self, frame: Frame, interner: &mut FrameStackInternTable) {
        self.push_frame_cached(frame, None, interner, None);
    }

    /// `cached` carries a precomputed identity for fully static frames so
    /// repeat pushes skip string hashing; `edge_cache` memoizes (parent stack,
    /// frame) → stack id within the tokenizer-owned intern table.
    fn push_frame_cached(
        &mut self,
        mut frame: Frame,
        cached: Option<StaticFrameIdentity>,
        interner: &mut FrameStackInternTable,
        edge_cache: Option<
            &mut FastMap<(InternedFrameStackId, InternedFrameId), InternedFrameStackId>,
        >,
    ) -> StaticFrameIdentity {
        let (identity_hash, frame_id) = match cached {
            Some(cached) => (cached.identity_hash, cached.frame_id),
            None => {
                let identity_hash = frame.compute_identity_hash();
                frame.identity_hash = identity_hash;
                (identity_hash, interner.intern_frame(&frame))
            }
        };
        frame.identity_hash = identity_hash;
        let parent_state_hash = self
            .frames
            .last()
            .map_or(0x811c9dc5, |parent| parent.state_hash);
        frame.state_hash = fnv_mix(
            parent_state_hash,
            (identity_hash ^ (identity_hash >> 32)) as u32,
        );
        let parent_id = self.frames.interned_id();
        frame.interned_stack_id = match edge_cache {
            Some(edge_cache) => {
                let key = (parent_id, frame_id);
                if let Some(stack_id) = edge_cache.get(&key) {
                    *stack_id
                } else {
                    let stack_id = interner.intern_stack_edge(parent_id, frame_id);
                    edge_cache.insert(key, stack_id);
                    stack_id
                }
            }
            None => interner.intern_stack_edge(parent_id, frame_id),
        };
        let identity = StaticFrameIdentity {
            identity_hash,
            frame_id,
        };
        self.frames.push(frame);
        self.refresh_interner_hash();
        identity
    }

    fn push_frame_shared(&mut self, node: Arc<FrameNode>) {
        self.frames.push_shared_node(node);
        self.refresh_interner_hash();
    }

    fn pop_frame(&mut self) {
        self.frames.pop();
        self.refresh_interner_hash();
    }

    fn truncate_frames(&mut self, len: usize) {
        self.frames.truncate(len);
        self.refresh_interner_hash();
    }

    fn prefix(&self, len: usize) -> Self {
        let mut state = Self {
            frames: self.frames.prefix(len),
            interner_hash: 0,
        };
        state.refresh_interner_hash();
        state
    }
}

impl PartialEq for TokenizerState {
    fn eq(&self, other: &Self) -> bool {
        self.frames == other.frames
    }
}

impl Eq for TokenizerState {}

impl Hash for TokenizerState {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.interner_hash);
    }
}

fn fnv_mix(mut hash: u32, part: u32) -> u32 {
    for byte in part.to_le_bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

fn fnv64_mix(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn fnv64_mix_u64(hash: u64, value: u64) -> u64 {
    fnv64_mix(hash, &value.to_le_bytes())
}

fn fnv64_mix_opt_str(hash: u64, value: Option<&str>) -> u64 {
    let hash = fnv64_mix_u64(hash, value.map_or(u64::MAX, |value| value.len() as u64));
    value.map_or(hash, |value| fnv64_mix(hash, value.as_bytes()))
}

#[derive(Debug, Clone)]
struct Frame {
    grammar_id: GrammarId,
    base_grammar_id: GrammarId,
    rule_id: RuleId,
    scope_prefix: Option<Arc<str>>,
    name: Option<Arc<str>>,
    content_name: Option<Arc<str>>,
    end_pattern: Option<Arc<str>>,
    end_pattern_id: Option<PatternId>,
    while_pattern: Option<Arc<str>>,
    while_pattern_id: Option<PatternId>,
    end_captures: Arc<CaptureSpec>,
    while_captures: Arc<CaptureSpec>,
    patterns: Arc<[RuleRef]>,
    apply_end_pattern_last: bool,
    begin_captured_eol: bool,
    /// Cached hash of this frame's identity fields; maintained by
    /// `TokenizerState::push_frame`.
    identity_hash: u64,
    /// Cumulative public `StateId` hash up to and including this frame.
    state_hash: u32,
    /// Exact hash-consed identity of the full frame stack ending at this
    /// frame. `TokenizerState` equality uses this id instead of walking every
    /// frame in deep continuations.
    interned_stack_id: InternedFrameStackId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
struct InternedFrameStackId(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct InternedFrameId(u32);

/// Precomputed identity of a fully static frame: the identity hash plus the
/// globally interned frame id. Cached per candidate so repeat pushes of the
/// same begin rule skip both string hashing and the intern-table mutex.
#[derive(Debug, Clone, Copy)]
struct StaticFrameIdentity {
    identity_hash: u64,
    frame_id: InternedFrameId,
}

impl Frame {
    fn compute_identity_hash(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        hash = fnv64_mix_u64(hash, u64::from(self.grammar_id.0));
        hash = fnv64_mix_u64(hash, u64::from(self.base_grammar_id.0));
        hash = fnv64_mix_u64(hash, u64::from(self.rule_id.0));
        hash = fnv64_mix_opt_str(hash, self.scope_prefix.as_deref());
        hash = fnv64_mix_opt_str(hash, self.name.as_deref());
        hash = fnv64_mix_opt_str(hash, self.content_name.as_deref());
        hash = fnv64_mix_opt_str(hash, self.end_pattern.as_deref());
        hash = fnv64_mix_u64(
            hash,
            self.end_pattern_id
                .map_or(u64::MAX, |pattern| u64::from(pattern.0)),
        );
        hash = fnv64_mix_opt_str(hash, self.while_pattern.as_deref());
        hash = fnv64_mix_u64(
            hash,
            self.while_pattern_id
                .map_or(u64::MAX, |pattern| u64::from(pattern.0)),
        );
        hash = fnv64_mix_u64(
            hash,
            u64::from(self.apply_end_pattern_last) | (u64::from(self.begin_captured_eol) << 1),
        );
        hash
    }
}

impl PartialEq for Frame {
    fn eq(&self, other: &Self) -> bool {
        self.grammar_id == other.grammar_id
            && self.base_grammar_id == other.base_grammar_id
            && self.rule_id == other.rule_id
            && self.scope_prefix == other.scope_prefix
            && self.name == other.name
            && self.content_name == other.content_name
            && self.end_pattern == other.end_pattern
            && self.end_pattern_id == other.end_pattern_id
            && self.while_pattern == other.while_pattern
            && self.while_pattern_id == other.while_pattern_id
            && self.apply_end_pattern_last == other.apply_end_pattern_last
            && self.begin_captured_eol == other.begin_captured_eol
    }
}

impl Eq for Frame {}

impl Hash for Frame {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Capture specs and nested patterns are immutable payloads of
        // `(grammar_id, rule_id)` and add no state identity. The identity
        // fields themselves are pre-hashed once at push time.
        state.write_u64(self.identity_hash);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct FrameIdentityKey {
    grammar_id: GrammarId,
    base_grammar_id: GrammarId,
    rule_id: RuleId,
    scope_prefix: Option<Arc<str>>,
    name: Option<Arc<str>>,
    content_name: Option<Arc<str>>,
    end_pattern: Option<Arc<str>>,
    end_pattern_id: Option<PatternId>,
    while_pattern: Option<Arc<str>>,
    while_pattern_id: Option<PatternId>,
    apply_end_pattern_last: bool,
    begin_captured_eol: bool,
}

impl FrameIdentityKey {
    fn from_frame(frame: &Frame) -> Self {
        Self {
            grammar_id: frame.grammar_id,
            base_grammar_id: frame.base_grammar_id,
            rule_id: frame.rule_id,
            scope_prefix: frame.scope_prefix.clone(),
            name: frame.name.clone(),
            content_name: frame.content_name.clone(),
            end_pattern: frame.end_pattern.clone(),
            end_pattern_id: frame.end_pattern_id,
            while_pattern: frame.while_pattern.clone(),
            while_pattern_id: frame.while_pattern_id,
            apply_end_pattern_last: frame.apply_end_pattern_last,
            begin_captured_eol: frame.begin_captured_eol,
        }
    }

    fn matches_frame(&self, frame: &Frame) -> bool {
        self.grammar_id == frame.grammar_id
            && self.base_grammar_id == frame.base_grammar_id
            && self.rule_id == frame.rule_id
            && self.scope_prefix.as_deref() == frame.scope_prefix.as_deref()
            && self.name.as_deref() == frame.name.as_deref()
            && self.content_name.as_deref() == frame.content_name.as_deref()
            && self.end_pattern.as_deref() == frame.end_pattern.as_deref()
            && self.end_pattern_id == frame.end_pattern_id
            && self.while_pattern.as_deref() == frame.while_pattern.as_deref()
            && self.while_pattern_id == frame.while_pattern_id
            && self.apply_end_pattern_last == frame.apply_end_pattern_last
            && self.begin_captured_eol == frame.begin_captured_eol
    }
}

#[derive(Debug, Clone, Copy)]
struct InternedFrameStackNode {
    parent: InternedFrameStackId,
    frame: Option<InternedFrameId>,
    depth: usize,
}

#[derive(Debug, Clone)]
struct InternedFrameStackScopeData {
    parent: InternedFrameStackId,
    scope_prefix: Option<Arc<str>>,
    name: Option<Arc<str>>,
    content_name: Option<Arc<str>>,
}

#[derive(Debug, Clone)]
struct FrameStackInternTable {
    frame_ids_by_hash: FastMap<u64, Vec<InternedFrameId>>,
    frame_keys: Vec<FrameIdentityKey>,
    stack_edges: FastMap<(InternedFrameStackId, InternedFrameId), InternedFrameStackId>,
    stack_nodes: Vec<InternedFrameStackNode>,
}

impl FrameStackInternTable {
    fn new() -> Self {
        Self {
            frame_ids_by_hash: hashing::fast_map(),
            frame_keys: Vec::new(),
            stack_edges: hashing::fast_map(),
            stack_nodes: vec![InternedFrameStackNode {
                parent: InternedFrameStackId::default(),
                frame: None,
                depth: 0,
            }],
        }
    }

    fn intern_frame(&mut self, frame: &Frame) -> InternedFrameId {
        if let Some(ids) = self.frame_ids_by_hash.get(&frame.identity_hash) {
            for id in ids {
                if self
                    .frame_keys
                    .get(id.0 as usize)
                    .is_some_and(|key| key.matches_frame(frame))
                {
                    return *id;
                }
            }
        }
        let id = InternedFrameId(self.frame_keys.len() as u32);
        let key = FrameIdentityKey::from_frame(frame);
        self.frame_keys.push(key);
        self.frame_ids_by_hash
            .entry(frame.identity_hash)
            .or_default()
            .push(id);
        id
    }

    fn intern_stack_edge(
        &mut self,
        parent: InternedFrameStackId,
        frame_id: InternedFrameId,
    ) -> InternedFrameStackId {
        let edge = (parent, frame_id);
        if let Some(id) = self.stack_edges.get(&edge) {
            return *id;
        }
        let parent_depth = self
            .stack_nodes
            .get(parent.0 as usize)
            .map_or(0, |node| node.depth);
        let id = InternedFrameStackId(self.stack_nodes.len() as u32);
        self.stack_nodes.push(InternedFrameStackNode {
            parent,
            frame: Some(frame_id),
            depth: parent_depth + 1,
        });
        self.stack_edges.insert(edge, id);
        id
    }

    fn scope_data(&self, id: InternedFrameStackId) -> Option<InternedFrameStackScopeData> {
        let node = self.stack_nodes.get(id.0 as usize)?;
        let frame_id = node.frame?;
        let frame = self.frame_keys.get(frame_id.0 as usize)?;
        Some(InternedFrameStackScopeData {
            parent: node.parent,
            scope_prefix: frame.scope_prefix.clone(),
            name: frame.name.clone(),
            content_name: frame.content_name.clone(),
        })
    }
}

// Continuation stacks are immutable parent-linked nodes holding one frame
// each. Push allocates exactly one node and pop is a parent-pointer step, so
// neither ever clones frames — even when the stack is shared with interned
// states, line-cache entries, and checkpoints. Exact equality is the interned
// stack id maintained on each frame.
#[derive(Debug, Clone, Default)]
struct FrameStack {
    tail: Option<Arc<FrameNode>>,
    len: usize,
    interned_id: InternedFrameStackId,
}

#[derive(Debug)]
struct FrameNode {
    parent: Option<Arc<FrameNode>>,
    frame: Frame,
    depth: usize,
    /// Number of frames with a `while` pattern in the chain up to and
    /// including this node. Lets the per-line while-continuation pass skip
    /// the O(depth) stack walk entirely for grammars that never use `while`
    /// (deep-stack sources otherwise pay the walk on every line).
    while_frames: usize,
}

impl FrameStack {
    #[inline]
    fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    fn len(&self) -> usize {
        self.len
    }

    #[inline]
    fn last(&self) -> Option<&Frame> {
        self.tail.as_deref().map(|node| &node.frame)
    }

    fn nodes_in_order(&self) -> Vec<&FrameNode> {
        let mut nodes = Vec::with_capacity(self.len);
        let mut cursor = self.tail.as_deref();
        while let Some(node) = cursor {
            nodes.push(node);
            cursor = node.parent.as_deref();
        }
        nodes.reverse();
        nodes
    }

    fn get(&self, index: usize) -> Option<&Frame> {
        if index >= self.len {
            return None;
        }
        let mut cursor = self.tail.as_deref();
        while let Some(node) = cursor {
            if node.depth == index + 1 {
                return Some(&node.frame);
            }
            cursor = node.parent.as_deref();
        }
        None
    }

    #[inline]
    fn while_frame_count(&self) -> usize {
        self.tail.as_deref().map_or(0, |node| node.while_frames)
    }

    #[inline]
    fn push(&mut self, frame: Frame) {
        let interned_id = frame.interned_stack_id;
        let while_frames = self.while_frame_count() + usize::from(frame.while_pattern.is_some());
        self.tail = Some(Arc::new(FrameNode {
            parent: self.tail.take(),
            frame,
            depth: self.len + 1,
            while_frames,
        }));
        self.len += 1;
        self.interned_id = interned_id;
    }

    /// Reuses an immutable node from a previous identical (parent stack,
    /// frame) transition. Sound because the node's parent chain is
    /// value-equal to the current tail (same interned parent id) and every
    /// reader goes through values, never pointer identity.
    #[inline]
    fn push_shared_node(&mut self, node: Arc<FrameNode>) {
        debug_assert_eq!(node.depth, self.len + 1);
        self.interned_id = node.frame.interned_stack_id;
        self.len = node.depth;
        if let Some(old) = self.tail.replace(node) {
            drop_frame_node(old);
        }
    }

    #[inline]
    fn tail_node(&self) -> Option<&Arc<FrameNode>> {
        self.tail.as_ref()
    }

    #[inline]
    fn pop(&mut self) {
        let Some(tail) = self.tail.take() else {
            return;
        };
        self.tail = tail.parent.clone();
        drop_frame_node(tail);
        self.len -= 1;
        self.refresh_interned_id_from_top();
    }

    fn truncate(&mut self, len: usize) {
        if len >= self.len {
            return;
        }
        let mut cursor = self.tail.take();
        while let Some(node) = cursor.take() {
            if node.depth <= len {
                cursor = Some(node);
                break;
            }
            let parent = node.parent.clone();
            drop_frame_node(node);
            cursor = parent;
        }
        self.tail = cursor;
        self.len = len;
        self.refresh_interned_id_from_top();
    }

    fn prefix(&self, len: usize) -> Self {
        let mut s = self.clone();
        s.truncate(len);
        s
    }

    #[inline]
    fn interned_id(&self) -> InternedFrameStackId {
        self.interned_id
    }

    fn refresh_interned_id_from_top(&mut self) {
        self.interned_id = self
            .last()
            .map_or(InternedFrameStackId::default(), |frame| {
                frame.interned_stack_id
            });
    }

    #[cfg(test)]
    fn iter(&self) -> FrameStackIter<'_> {
        let frames = self
            .nodes_in_order()
            .into_iter()
            .map(|node| &node.frame)
            .collect();
        FrameStackIter { frames, index: 0 }
    }

    #[inline]
    fn for_each(&self, mut f: impl FnMut(usize, &Frame)) {
        for (index, node) in self.nodes_in_order().into_iter().enumerate() {
            f(index, &node.frame);
        }
    }
}

impl Drop for FrameStack {
    fn drop(&mut self) {
        if let Some(tail) = self.tail.take() {
            drop_frame_node(tail);
        }
    }
}

/// Drops a frame-node chain iteratively. Deep continuation stacks otherwise
/// recurse once per frame through `Arc`/`FrameNode` drop glue, which can
/// overflow the thread stack on adversarial nesting depths.
fn drop_frame_node(node: Arc<FrameNode>) {
    let mut cursor = Some(node);
    while let Some(node) = cursor {
        match Arc::try_unwrap(node) {
            Ok(mut owned) => cursor = owned.parent.take(),
            Err(_) => break,
        }
    }
}

impl PartialEq for FrameStack {
    fn eq(&self, other: &Self) -> bool {
        self.interned_id == other.interned_id
    }
}
impl Eq for FrameStack {}

#[cfg(test)]
struct FrameStackIter<'a> {
    frames: Vec<&'a Frame>,
    index: usize,
}

#[cfg(test)]
impl<'a> Iterator for FrameStackIter<'a> {
    type Item = &'a Frame;

    fn next(&mut self) -> Option<Self::Item> {
        let frame = self.frames.get(self.index).copied()?;
        self.index += 1;
        Some(frame)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.frames.len().saturating_sub(self.index);
        (remaining, Some(remaining))
    }
}
#[cfg(test)]
impl ExactSizeIterator for FrameStackIter<'_> {}

const REPOSITORY_BINDING_FLAT_ENTRIES: usize = 32;
const REPOSITORY_BINDING_BLOCK_LAYERS: u16 = 256;

#[derive(Debug, Default)]
struct RepositoryBindings {
    parent: Option<Arc<RepositoryBindings>>,
    local: BTreeMap<String, String>,
    uncompacted_layers: u16,
    has_bindings: bool,
}

impl RepositoryBindings {
    fn overlay(
        parent: Arc<RepositoryBindings>,
        local: BTreeMap<String, String>,
        flatten_small: bool,
    ) -> Arc<Self> {
        // Keep ordinary unbounded tokenizer contexts flat. Prepared contexts
        // disable this path so every binding has a strict retained-byte charge.
        // The fixed entry ceiling bounds direct-path copy work; larger/deeper
        // contexts switch to the persistent representation below.
        if flatten_small
            && parent.parent.is_none()
            && parent.local.len().saturating_add(local.len()) <= REPOSITORY_BINDING_FLAT_ENTRIES
        {
            let mut merged = parent.local.clone();
            merged.extend(local);
            return Arc::new(Self {
                has_bindings: !merged.is_empty(),
                parent: None,
                local: merged,
                uncompacted_layers: 0,
            });
        }

        let uncompacted_layers = parent.uncompacted_layers + 1;
        if uncompacted_layers < REPOSITORY_BINDING_BLOCK_LAYERS {
            return Arc::new(Self {
                has_bindings: parent.has_bindings || !local.is_empty(),
                parent: Some(parent),
                local,
                uncompacted_layers,
            });
        }

        // Coalesce each fixed-size run into one immutable lookup block. Older
        // contexts keep sharing their original nodes, while new contexts need
        // at most one BTreeMap lookup per block rather than one per overlay.
        let mut merged = local;
        let mut cursor = Some(parent);
        for _ in 1..REPOSITORY_BINDING_BLOCK_LAYERS {
            let bindings = cursor
                .take()
                .expect("the uncompacted layer count matches the parent chain");
            for (name, binding) in &bindings.local {
                merged
                    .entry(name.clone())
                    .or_insert_with(|| binding.clone());
            }
            cursor = bindings.parent.clone();
        }
        let has_bindings = !merged.is_empty()
            || cursor
                .as_ref()
                .is_some_and(|bindings| bindings.has_bindings);
        Arc::new(Self {
            parent: cursor,
            local: merged,
            uncompacted_layers: 0,
            has_bindings,
        })
    }

    fn get(&self, name: &str) -> Option<&String> {
        let mut current = Some(self);
        while let Some(bindings) = current {
            if let Some(binding) = bindings.local.get(name) {
                return Some(binding);
            }
            current = bindings.parent.as_deref();
        }
        None
    }

    fn is_empty(&self) -> bool {
        !self.has_bindings
    }
}

#[derive(Debug)]
struct GrammarRuleRepositoryContexts {
    dense: Box<[Option<Arc<RepositoryBindings>>]>,
    // `CompiledGrammar` is public and its rule IDs can therefore be sparse,
    // even though both native compilers produce dense IDs. Keep those unusual
    // callers correct without putting the ordinary lookup path behind a hash.
    sparse: Vec<(RuleId, Arc<RepositoryBindings>)>,
}

impl GrammarRuleRepositoryContexts {
    fn new(dense_len: usize) -> Self {
        Self {
            dense: std::iter::repeat_with(|| None)
                .take(dense_len)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            sparse: Vec::new(),
        }
    }

    fn get(&self, rule_id: RuleId) -> Option<&Arc<RepositoryBindings>> {
        if let Some(context) = self.dense.get(rule_id.0 as usize) {
            return context.as_ref();
        }
        self.sparse
            .iter()
            .find_map(|(candidate, context)| (*candidate == rule_id).then_some(context))
    }

    fn insert_first(&mut self, rule_id: RuleId, context: Arc<RepositoryBindings>) -> bool {
        if let Some(slot) = self.dense.get_mut(rule_id.0 as usize) {
            if slot.is_some() {
                return false;
            }
            *slot = Some(context);
            return true;
        }
        if self
            .sparse
            .iter()
            .any(|(candidate, _)| *candidate == rule_id)
        {
            return false;
        }
        self.sparse.push((rule_id, context));
        true
    }
}

/// Repository contexts indexed first by grammar and then directly by rule ID.
///
/// The outer table is compact, while each per-grammar rule table is allocated
/// only when the lazy-compilation walk first reaches one of that grammar's
/// rules. This keeps external grammar closures cheap without hashing the hot
/// `(GrammarId, RuleId)` pair.
#[derive(Debug)]
struct RuleRepositoryContexts {
    grammars: Box<[Option<Box<GrammarRuleRepositoryContexts>>]>,
}

impl RuleRepositoryContexts {
    fn new(grammar_count: usize) -> Self {
        Self {
            grammars: std::iter::repeat_with(|| None)
                .take(grammar_count)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
    }

    fn empty() -> Self {
        Self::new(0)
    }

    fn get(&self, grammar_id: GrammarId, rule_id: RuleId) -> Option<&Arc<RepositoryBindings>> {
        self.grammars
            .get(grammar_id.0 as usize)
            .and_then(Option::as_deref)
            .and_then(|grammar| grammar.get(rule_id))
    }

    fn has_grammar_table(&self, grammar_id: GrammarId) -> bool {
        self.grammars
            .get(grammar_id.0 as usize)
            .is_some_and(Option::is_some)
    }

    fn insert_first(
        &mut self,
        grammar_id: GrammarId,
        rule_id: RuleId,
        dense_len: usize,
        context: Arc<RepositoryBindings>,
    ) -> bool {
        let Some(grammar) = self.grammars.get_mut(grammar_id.0 as usize) else {
            return false;
        };
        let grammar =
            grammar.get_or_insert_with(|| Box::new(GrammarRuleRepositoryContexts::new(dense_len)));
        grammar.insert_first(rule_id, context)
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.grammars
            .iter()
            .filter_map(Option::as_deref)
            .map(|grammar| {
                grammar
                    .dense
                    .iter()
                    .filter(|context| context.is_some())
                    .count()
                    + grammar.sparse.len()
            })
            .sum()
    }

    #[cfg(test)]
    fn allocated_grammar_count(&self) -> usize {
        self.grammars
            .iter()
            .filter(|grammar| grammar.is_some())
            .count()
    }

    #[cfg(test)]
    fn dense_slot_count(&self, grammar_id: GrammarId) -> usize {
        self.grammars
            .get(grammar_id.0 as usize)
            .and_then(Option::as_deref)
            .map_or(0, |grammar| grammar.dense.len())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct RepositoryNameId(u32);

/// Assigns compact IDs to repository names for traversal sets. The grammar IR
/// retains its public string representation; only walk/cycle keys are interned.
#[derive(Debug, Clone, Default)]
struct RepositoryNameInterner {
    ids: FastMap<String, RepositoryNameId>,
}

impl RepositoryNameInterner {
    fn get(&self, name: &str) -> Option<RepositoryNameId> {
        self.ids.get(name).copied()
    }

    fn intern(&mut self, name: &str) -> (RepositoryNameId, bool) {
        if let Some(id) = self.get(name) {
            return (id, false);
        }
        let id = RepositoryNameId(
            u32::try_from(self.ids.len()).expect("repository-name count fits in u32"),
        );
        self.ids.insert(name.to_owned(), id);
        (id, true)
    }

    fn clear(&mut self) {
        self.ids.clear();
    }
}

#[derive(Debug, Clone, Default)]
pub struct GrammarSet {
    // Arc-shared so cloning a set (one clone per tokenizer instance) shares
    // immutable compiled grammars and live root-specific repository walks.
    // Weak values let those walks be reclaimed with their tokenizers.
    grammars: Arc<Vec<Arc<CompiledGrammar>>>,
    scope_to_id: Arc<HashMap<String, GrammarId>>,
    rule_repository_context_cache: Arc<Mutex<FastMap<GrammarId, Weak<RuleRepositoryContexts>>>>,
}

impl GrammarSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, grammar: CompiledGrammar) -> GrammarId {
        let id = grammar.id;
        Arc::make_mut(&mut self.scope_to_id).insert(grammar.scope_name.clone(), id);
        // A mutated set must not share root compilations produced from an
        // older grammar graph. Existing clones retain their valid cache.
        self.rule_repository_context_cache = Arc::new(Mutex::new(hashing::fast_map()));
        let index = id.0 as usize;
        let grammars = Arc::make_mut(&mut self.grammars);
        if index == grammars.len() {
            grammars.push(Arc::new(grammar));
        } else if index < grammars.len() {
            grammars[index] = Arc::new(grammar);
        } else {
            panic!("grammar ids must be dense and insertion ordered");
        }
        id
    }

    pub fn load_and_add(&mut self, contents: &str) -> Result<GrammarId, GrammarLoadError> {
        let id = GrammarId(self.grammars.len() as u16);
        let grammar = load_dev_grammar_from_str(id, contents)?;
        Ok(self.add(grammar))
    }

    pub fn grammar(&self, id: GrammarId) -> Option<&CompiledGrammar> {
        self.grammars.get(id.0 as usize).map(Arc::as_ref)
    }

    pub fn grammar_by_scope(&self, scope: &str) -> Option<&CompiledGrammar> {
        let id = *self.scope_to_id.get(scope)?;
        self.grammar(id)
    }

    pub fn grammar_id_by_scope(&self, scope: &str) -> Option<GrammarId> {
        self.scope_to_id.get(scope).copied()
    }

    pub fn grammars(&self) -> &[Arc<CompiledGrammar>] {
        self.grammars.as_slice()
    }

    fn into_prepared_closure(
        self,
        root: GrammarId,
        grammar_closure: &[bool],
    ) -> (Self, GrammarId, bool) {
        let selected = self
            .grammars
            .iter()
            .filter(|grammar| {
                grammar_closure
                    .get(grammar.id.0 as usize)
                    .copied()
                    .unwrap_or(false)
            })
            .cloned()
            .collect::<Vec<_>>();
        if selected.len() == self.grammars.len() {
            return (self, root, false);
        }

        // Preserve shared grammar allocations when filtering leaves a dense
        // ID prefix. Otherwise compact the selected records and remap their
        // grammar IDs; external references are scope-based and local rule,
        // pattern, scope, and string IDs remain unchanged.
        if selected
            .iter()
            .enumerate()
            .all(|(index, grammar)| grammar.id.0 as usize == index)
        {
            let scope_to_id = selected
                .iter()
                .map(|grammar| (grammar.scope_name.clone(), grammar.id))
                .collect();
            return (
                Self {
                    grammars: Arc::new(selected),
                    scope_to_id: Arc::new(scope_to_id),
                    rule_repository_context_cache: Arc::new(Mutex::new(hashing::fast_map())),
                },
                root,
                false,
            );
        }

        let mut subset = Self::new();
        let mut remapped_root = None;
        for grammar in selected {
            let is_root = grammar.id == root;
            let mut grammar = grammar.as_ref().clone();
            grammar.id = GrammarId(subset.grammars.len() as u16);
            let grammar_id = subset.add(grammar);
            if is_root {
                remapped_root = Some(grammar_id);
            }
        }
        (subset, remapped_root.unwrap_or(root), true)
    }

    fn rule_repository_contexts(
        &self,
        root: GrammarId,
        injections: &[CompiledInjectionSelector],
    ) -> Arc<RuleRepositoryContexts> {
        if let Some(contexts) = self
            .rule_repository_context_cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&root)
            .and_then(Weak::upgrade)
        {
            return contexts;
        }

        // Do the recursive work outside the lock. Concurrent first users may
        // compute the same immutable value, but only one is shared.
        let (compiled, _) = compile_rule_repository_contexts(self, root, injections, false);
        let compiled = Arc::new(compiled);
        let mut cache = self
            .rule_repository_context_cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(contexts) = cache.get(&root).and_then(Weak::upgrade) {
            return contexts;
        }
        cache.retain(|_, contexts| contexts.strong_count() != 0);
        cache.insert(root, Arc::downgrade(&compiled));
        compiled
    }

    fn prepared_rule_repository_contexts(
        &self,
        root: GrammarId,
        injections: &[CompiledInjectionSelector],
    ) -> Option<Arc<RuleRepositoryContexts>> {
        let (contexts, complete) = compile_rule_repository_contexts(self, root, injections, true);
        complete.then(|| Arc::new(contexts))
    }

    pub fn validate_include_graph(&self) -> Result<(), GrammarValidationError> {
        for grammar in self.grammars.iter() {
            grammar.validate_local_refs()?;
            self.validate_refs_for_grammar(grammar, &grammar.top_level, "patterns")?;
            for (name, rule_ref) in &grammar.repository {
                self.validate_refs_for_grammar(
                    grammar,
                    std::slice::from_ref(rule_ref),
                    format!("repository.{name}").as_str(),
                )?;
            }
            for injection in &grammar.injections {
                self.validate_refs_for_grammar(
                    grammar,
                    &injection.patterns,
                    format!("injections.{}", injection.selector).as_str(),
                )?;
            }
            for rule in &grammar.rules {
                match &rule.body {
                    RuleBody::Match { captures, .. } => {
                        self.validate_capture_refs(
                            grammar,
                            captures,
                            format!("rule.{}.captures", rule.id.0).as_str(),
                        )?;
                    }
                    RuleBody::BeginEnd {
                        begin_captures,
                        end_captures,
                        patterns,
                        ..
                    } => {
                        self.validate_capture_refs(
                            grammar,
                            begin_captures,
                            format!("rule.{}.beginCaptures", rule.id.0).as_str(),
                        )?;
                        self.validate_capture_refs(
                            grammar,
                            end_captures,
                            format!("rule.{}.endCaptures", rule.id.0).as_str(),
                        )?;
                        self.validate_refs_for_grammar(
                            grammar,
                            patterns,
                            format!("rule.{}.patterns", rule.id.0).as_str(),
                        )?;
                    }
                    RuleBody::BeginWhile {
                        begin_captures,
                        while_captures,
                        patterns,
                        ..
                    } => {
                        self.validate_capture_refs(
                            grammar,
                            begin_captures,
                            format!("rule.{}.beginCaptures", rule.id.0).as_str(),
                        )?;
                        self.validate_capture_refs(
                            grammar,
                            while_captures,
                            format!("rule.{}.whileCaptures", rule.id.0).as_str(),
                        )?;
                        self.validate_refs_for_grammar(
                            grammar,
                            patterns,
                            format!("rule.{}.patterns", rule.id.0).as_str(),
                        )?;
                    }
                    RuleBody::IncludeOnly { patterns } => {
                        self.validate_refs_for_grammar(
                            grammar,
                            patterns,
                            format!("rule.{}.patterns", rule.id.0).as_str(),
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_capture_refs(
        &self,
        grammar: &CompiledGrammar,
        captures: &CaptureSpec,
        path: &str,
    ) -> Result<(), GrammarValidationError> {
        for (group, entry) in &captures.entries {
            self.validate_refs_for_grammar(
                grammar,
                &entry.patterns,
                format!("{path}.{group}.patterns").as_str(),
            )?;
        }
        Ok(())
    }

    fn validate_refs_for_grammar(
        &self,
        grammar: &CompiledGrammar,
        refs: &[RuleRef],
        path: &str,
    ) -> Result<(), GrammarValidationError> {
        for (index, rule_ref) in refs.iter().enumerate() {
            match rule_ref {
                RuleRef::External { scope, repository } => {
                    let scope_text = grammar.scope(*scope).ok_or_else(|| {
                        GrammarValidationError::new(
                            grammar.scope_name.clone(),
                            format!("{path}[{index}]"),
                            "include",
                            format!("bad external scope id {}", scope.0),
                        )
                    })?;
                    let external = self.grammar_by_scope(scope_text).ok_or_else(|| {
                        GrammarValidationError::new(
                            grammar.scope_name.clone(),
                            format!("{path}[{index}]"),
                            "include",
                            format!("unknown external grammar {scope_text}"),
                        )
                    })?;
                    if let Some(repository) = repository
                        && !external.repository.contains_key(repository)
                    {
                        return Err(GrammarValidationError::new(
                            grammar.scope_name.clone(),
                            format!("{path}[{index}]"),
                            "include",
                            format!("unknown external include {scope_text}#{repository}"),
                        ));
                    }
                }
                other => {
                    grammar.validate_rule_ref(other, format!("{path}[{index}]").as_str(), false)?
                }
            }
        }
        Ok(())
    }
}

/// Caller-owned immutable preparation shared by independent tokenizers.
///
/// The pristine tokenizer retains the root candidate descriptor and its
/// tokenizer-local ID seed. Static regex slots are populated once across every
/// tokenizer made from this value and are bounded by both slot-table and
/// compiled-payload bytes; lazily discovered static descriptors use a
/// separate bounded cache.
#[derive(Debug)]
pub struct PreparedLanguage {
    prototype: Mutex<TextMateTokenizer>,
    static_patterns: Arc<PreparedPatternCache>,
    static_blueprints: Arc<PreparedBlueprintCache>,
    grammar_count: usize,
}

impl PreparedLanguage {
    #[cfg(test)]
    pub fn new(grammars: GrammarSet, root: GrammarId) -> Self {
        Self::try_new(grammars, root).expect("test grammar exceeds preparation bounds")
    }

    pub fn try_new(grammars: GrammarSet, root: GrammarId) -> Result<Self, &'static str> {
        let initial_injection_selectors = compile_injection_selectors(&grammars, root);
        let initial_rule_repository_contexts = grammars
            .prepared_rule_repository_contexts(root, &initial_injection_selectors)
            .ok_or("repository-context walk exceeded its preparation bound")?;
        let initial_grammar_closure = prepared_grammar_closure(
            &grammars,
            root,
            &initial_injection_selectors,
            &initial_rule_repository_contexts,
        )
        .ok_or("grammar-closure walk exceeded its preparation bound")?;

        let (grammars, root, injection_selectors, rule_repository_contexts, grammar_closure) = {
            let (grammars, root, ids_remapped) =
                grammars.into_prepared_closure(root, &initial_grammar_closure);
            if ids_remapped {
                let injection_selectors = compile_injection_selectors(&grammars, root);
                let contexts = grammars
                    .prepared_rule_repository_contexts(root, &injection_selectors)
                    .ok_or("remapped repository-context walk exceeded its preparation bound")?;
                let grammar_closure =
                    prepared_grammar_closure(&grammars, root, &injection_selectors, &contexts)
                        .ok_or("remapped grammar-closure walk exceeded its preparation bound")?;
                (
                    grammars,
                    root,
                    injection_selectors,
                    contexts,
                    grammar_closure,
                )
            } else {
                (
                    grammars,
                    root,
                    initial_injection_selectors,
                    initial_rule_repository_contexts,
                    initial_grammar_closure,
                )
            }
        };

        let grammar_count = grammar_closure
            .iter()
            .filter(|reachable| **reachable)
            .count();
        let static_patterns = Arc::new(PreparedPatternCache::new(&grammars, &grammar_closure));
        let static_blueprints = Arc::new(PreparedBlueprintCache::default());
        let mut prototype = TextMateTokenizer::new_with_prepared_caches(
            grammars,
            root,
            Arc::clone(&static_patterns),
            Arc::clone(&static_blueprints),
            injection_selectors,
            rule_repository_contexts,
        );
        prototype.prepare_root_candidate();
        Ok(Self {
            prototype: Mutex::new(prototype),
            static_patterns,
            static_blueprints,
            grammar_count,
        })
    }

    pub fn tokenizer(&self) -> TextMateTokenizer {
        self.prototype
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn grammar_count(&self) -> usize {
        self.grammar_count
    }

    pub fn static_pattern_capacity(&self) -> usize {
        self.static_patterns.capacity()
    }

    pub fn compiled_pattern_count(&self) -> usize {
        self.static_patterns.initialized_count()
    }

    pub fn static_pattern_byte_capacity(&self) -> usize {
        MAX_PREPARED_PATTERN_BYTES
    }

    pub fn static_pattern_retained_bytes(&self) -> usize {
        self.static_patterns.retained_bytes()
    }

    pub fn static_blueprint_capacity(&self) -> usize {
        MAX_CANDIDATE_BLUEPRINTS
    }

    pub fn static_blueprint_count(&self) -> usize {
        self.static_blueprints.len()
    }

    pub fn static_blueprint_byte_capacity(&self) -> usize {
        MAX_PREPARED_CANDIDATE_BYTES
    }

    pub fn static_blueprint_retained_bytes(&self) -> usize {
        self.static_blueprints.retained_bytes()
    }
}

#[derive(Clone)]
enum PreparedPendingRefs<'a> {
    Borrowed(&'a [RuleRef]),
    Owned(Vec<RuleRef>),
}

impl PreparedPendingRefs<'_> {
    fn as_slice(&self) -> &[RuleRef] {
        match self {
            Self::Borrowed(refs) => refs,
            Self::Owned(refs) => refs,
        }
    }
}

fn contextualize_pending_refs<'a>(
    refs: &'a [RuleRef],
    context: Option<&RepositoryBindings>,
) -> PreparedPendingRefs<'a> {
    let Some(context) = context.filter(|context| !context.is_empty()) else {
        return PreparedPendingRefs::Borrowed(refs);
    };
    if !refs.iter().any(
        |rule_ref| matches!(rule_ref, RuleRef::Repository(name) if context.get(name).is_some()),
    ) {
        return PreparedPendingRefs::Borrowed(refs);
    }
    PreparedPendingRefs::Owned(contextualize_refs(refs, Some(context)))
}

struct PreparedGrammarWalker<'a> {
    grammars: &'a GrammarSet,
    injections: &'a [CompiledInjectionSelector],
    rule_repository_contexts: &'a RuleRepositoryContexts,
    reachable: Vec<bool>,
    pending: Vec<(GrammarId, GrammarId, PreparedPendingRefs<'a>)>,
    visited_rules: HashSet<(GrammarId, GrammarId, RuleId)>,
    visited_repositories: HashSet<(GrammarId, GrammarId, RepositoryNameId)>,
    repository_names: RepositoryNameInterner,
    visited_top_levels: HashSet<(GrammarId, GrammarId)>,
    injection_bases: HashSet<GrammarId>,
    visited_state_count: usize,
    visited_state_bytes: usize,
    pending_ref_count: usize,
    pending_ref_bytes: usize,
    budget_exceeded: bool,
}

impl<'a> PreparedGrammarWalker<'a> {
    fn mark(&mut self, grammar_id: GrammarId) {
        if self.grammars.grammar(grammar_id).is_some()
            && let Some(reachable) = self.reachable.get_mut(grammar_id.0 as usize)
        {
            *reachable = true;
        }
    }

    fn charge_state(&mut self, dynamic_bytes: usize) -> bool {
        self.visited_state_count = self.visited_state_count.saturating_add(1);
        self.visited_state_bytes = self.visited_state_bytes.saturating_add(dynamic_bytes);
        if self.visited_state_count > MAX_PREPARED_GRAMMAR_WALK_STATES
            || self.visited_state_bytes > MAX_PREPARED_GRAMMAR_WALK_BYTES
        {
            self.budget_exceeded = true;
            return false;
        }
        true
    }

    fn enqueue_refs(
        &mut self,
        grammar_id: GrammarId,
        base_grammar_id: GrammarId,
        refs: PreparedPendingRefs<'a>,
    ) {
        self.mark(grammar_id);
        let refs_slice = refs.as_slice();
        if refs_slice.is_empty() || self.budget_exceeded {
            return;
        }
        let refs_bytes = rule_refs_retained_bytes(refs_slice);
        let pending_ref_count = self.pending_ref_count.saturating_add(refs_slice.len());
        let pending_ref_bytes = self.pending_ref_bytes.saturating_add(refs_bytes);
        if pending_ref_count > MAX_PREPARED_GRAMMAR_PENDING_REFS
            || pending_ref_bytes > MAX_PREPARED_GRAMMAR_PENDING_BYTES
        {
            self.budget_exceeded = true;
            return;
        }
        self.pending_ref_count = pending_ref_count;
        self.pending_ref_bytes = pending_ref_bytes;
        self.pending.push((grammar_id, base_grammar_id, refs));
    }

    fn visit_top_level(&mut self, grammar_id: GrammarId, base_grammar_id: GrammarId) {
        self.mark(grammar_id);
        let first_visit = self
            .visited_top_levels
            .insert((grammar_id, base_grammar_id));
        if !first_visit || !self.charge_state(0) {
            return;
        }
        let grammars = self.grammars;
        if let Some(grammar) = grammars.grammar(grammar_id) {
            self.enqueue_refs(
                grammar_id,
                base_grammar_id,
                PreparedPendingRefs::Borrowed(&grammar.top_level),
            );
        }
    }

    fn visit_repository(&mut self, grammar_id: GrammarId, base_grammar_id: GrammarId, name: &str) {
        self.mark(grammar_id);
        let (name_id, name_bytes) = if let Some(name_id) = self.repository_names.get(name) {
            (name_id, 0)
        } else {
            // Charge the one retained interner copy before allocating it.
            if !self.charge_state(name.len()) {
                return;
            }
            (self.repository_names.intern(name).0, name.len())
        };
        let first_visit = self
            .visited_repositories
            .insert((grammar_id, base_grammar_id, name_id));
        if !first_visit || (name_bytes == 0 && !self.charge_state(0)) {
            return;
        }
        let grammars = self.grammars;
        if let Some(rule_ref) = grammars
            .grammar(grammar_id)
            .and_then(|grammar| grammar.repository.get(name))
        {
            self.enqueue_refs(
                grammar_id,
                base_grammar_id,
                PreparedPendingRefs::Borrowed(std::slice::from_ref(rule_ref)),
            );
        }
    }

    fn add_injection_base(&mut self, base_grammar_id: GrammarId) {
        if !self.injection_bases.insert(base_grammar_id) || !self.charge_state(0) {
            return;
        }
        let mut injection_ref_count = 0usize;
        let mut injection_ref_bytes = 0usize;
        for injection in self.injections {
            injection_ref_count = injection_ref_count.saturating_add(injection.patterns.len());
            injection_ref_bytes =
                injection_ref_bytes.saturating_add(rule_refs_retained_bytes(&injection.patterns));
        }
        if self.pending_ref_count.saturating_add(injection_ref_count)
            > MAX_PREPARED_GRAMMAR_PENDING_REFS
            || self.pending_ref_bytes.saturating_add(injection_ref_bytes)
                > MAX_PREPARED_GRAMMAR_PENDING_BYTES
        {
            self.budget_exceeded = true;
            return;
        }
        let injections = self.injections;
        for injection in injections {
            self.enqueue_refs(
                injection.grammar_id,
                base_grammar_id,
                PreparedPendingRefs::Borrowed(&injection.patterns),
            );
        }
    }

    fn visit_captures(
        &mut self,
        grammar_id: GrammarId,
        captures: &'a CaptureSpec,
        context: Option<&RepositoryBindings>,
    ) {
        for entry in captures.entries.values() {
            if entry.patterns.is_empty() {
                continue;
            }
            self.add_injection_base(grammar_id);
            self.enqueue_refs(
                grammar_id,
                grammar_id,
                contextualize_pending_refs(&entry.patterns, context),
            );
        }
    }

    fn visit_rule(&mut self, grammar_id: GrammarId, base_grammar_id: GrammarId, rule_id: RuleId) {
        let first_visit = self
            .visited_rules
            .insert((grammar_id, base_grammar_id, rule_id));
        if !first_visit || !self.charge_state(0) {
            return;
        }
        let grammars = self.grammars;
        let Some(rule) = grammars
            .grammar(grammar_id)
            .and_then(|grammar| grammar.rule(rule_id))
        else {
            return;
        };
        let context = self
            .rule_repository_contexts
            .get(grammar_id, rule_id)
            .cloned();
        let context = context.as_deref();
        match &rule.body {
            RuleBody::Match { captures, .. } => {
                self.visit_captures(grammar_id, captures, context);
            }
            RuleBody::BeginEnd {
                begin_captures,
                end_captures,
                patterns,
                ..
            } => {
                self.visit_captures(grammar_id, begin_captures, context);
                self.visit_captures(grammar_id, end_captures, context);
                self.enqueue_refs(
                    grammar_id,
                    base_grammar_id,
                    contextualize_pending_refs(patterns, context),
                );
            }
            RuleBody::BeginWhile {
                begin_captures,
                while_captures,
                content_name,
                patterns,
                ..
            } => {
                self.visit_captures(grammar_id, begin_captures, context);
                self.visit_captures(grammar_id, while_captures, context);
                let patterns = contextualize_pending_refs(patterns, context);
                let retokenizes_begin = begin_captures.entries.is_empty()
                    && content_name.is_some()
                    && !patterns.as_slice().is_empty();
                if retokenizes_begin {
                    self.add_injection_base(grammar_id);
                    if grammar_id != base_grammar_id {
                        self.enqueue_refs(grammar_id, grammar_id, patterns.clone());
                    }
                }
                self.enqueue_refs(grammar_id, base_grammar_id, patterns);
            }
            RuleBody::IncludeOnly { patterns } => self.enqueue_refs(
                grammar_id,
                base_grammar_id,
                contextualize_pending_refs(patterns, context),
            ),
        }
    }

    fn walk(mut self, root: GrammarId) -> Option<Vec<bool>> {
        self.add_injection_base(root);
        self.visit_top_level(root, root);
        while !self.budget_exceeded {
            let Some((grammar_id, base_grammar_id, refs)) = self.pending.pop() else {
                break;
            };
            let refs = refs.as_slice();
            self.pending_ref_count = self.pending_ref_count.saturating_sub(refs.len());
            self.pending_ref_bytes = self
                .pending_ref_bytes
                .saturating_sub(rule_refs_retained_bytes(refs));
            for rule_ref in refs {
                if self.budget_exceeded {
                    break;
                }
                match rule_ref {
                    RuleRef::Rule(rule_id) => {
                        self.visit_rule(grammar_id, base_grammar_id, *rule_id);
                    }
                    RuleRef::Repository(name) => {
                        self.visit_repository(grammar_id, base_grammar_id, name);
                    }
                    RuleRef::SelfRef => {
                        self.visit_top_level(grammar_id, base_grammar_id);
                    }
                    RuleRef::BaseRef => {
                        self.visit_top_level(base_grammar_id, base_grammar_id);
                    }
                    RuleRef::External { scope, repository } => {
                        let external_id = self
                            .grammars
                            .grammar(grammar_id)
                            .and_then(|grammar| grammar.scope(*scope))
                            .and_then(|scope| self.grammars.grammar_id_by_scope(scope));
                        let Some(external_id) = external_id else {
                            continue;
                        };
                        if let Some(repository) = repository {
                            self.visit_repository(external_id, base_grammar_id, repository);
                        } else {
                            self.visit_top_level(external_id, base_grammar_id);
                        }
                    }
                }
            }
        }
        (!self.budget_exceeded).then_some(self.reachable)
    }
}

/// Walk exactly the rule/repository/capture graph that the root and its
/// registered injections can reach. Injection refs are revisited for each base
/// grammar introduced by capture retokenization, matching runtime `$base`.
fn prepared_grammar_closure(
    grammars: &GrammarSet,
    root: GrammarId,
    injections: &[CompiledInjectionSelector],
    rule_repository_contexts: &RuleRepositoryContexts,
) -> Option<Vec<bool>> {
    PreparedGrammarWalker {
        grammars,
        injections,
        rule_repository_contexts,
        reachable: vec![false; grammars.grammars().len()],
        pending: Vec::new(),
        visited_rules: HashSet::new(),
        visited_repositories: HashSet::new(),
        repository_names: RepositoryNameInterner::default(),
        visited_top_levels: HashSet::new(),
        injection_bases: HashSet::new(),
        visited_state_count: 0,
        visited_state_bytes: 0,
        pending_ref_count: 0,
        pending_ref_bytes: 0,
        budget_exceeded: false,
    }
    .walk(root)
}

type PreparedPatternSlot = OnceLock<Option<Arc<CompiledPattern>>>;
type PreparedGrammarPatternSlots = Box<[PreparedPatternSlot]>;

#[derive(Debug)]
struct PreparedPatternCache {
    slots: Box<[Option<PreparedGrammarPatternSlots>]>,
    capacity: usize,
    retained_bytes: AtomicUsize,
    compile_permit: Mutex<()>,
}

impl PreparedPatternCache {
    fn new(grammars: &GrammarSet, grammar_closure: &[bool]) -> Self {
        let outer_slot_bytes = std::mem::size_of::<Option<PreparedGrammarPatternSlots>>();
        let pattern_slot_bytes = std::mem::size_of::<PreparedPatternSlot>();
        let outer_capacity = grammars
            .grammars()
            .len()
            .min(MAX_PREPARED_PATTERN_SLOT_BYTES / outer_slot_bytes);
        let mut slot_bytes = outer_capacity.saturating_mul(outer_slot_bytes);
        let mut capacity = 0usize;
        let slots = grammars
            .grammars()
            .iter()
            .take(outer_capacity)
            .enumerate()
            .map(|(index, grammar)| {
                if !grammar_closure.get(index).copied().unwrap_or(false) {
                    return None;
                }
                let remaining_bytes = MAX_PREPARED_PATTERN_SLOT_BYTES.saturating_sub(slot_bytes);
                let slot_capacity = grammar
                    .patterns
                    .len()
                    .min(remaining_bytes / pattern_slot_bytes);
                let grammar_slot_bytes = slot_capacity.saturating_mul(pattern_slot_bytes);
                slot_bytes = slot_bytes.saturating_add(grammar_slot_bytes);
                capacity = capacity.saturating_add(slot_capacity);
                let grammar_slots = (0..slot_capacity)
                    .map(|_| OnceLock::new())
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                Some(grammar_slots)
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            slots,
            capacity,
            retained_bytes: AtomicUsize::new(slot_bytes),
            compile_permit: Mutex::new(()),
        }
    }

    fn try_reserve_pattern_bytes(&self, bytes: usize) -> bool {
        self.retained_bytes
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |retained| {
                retained
                    .checked_add(bytes)
                    .filter(|total| *total <= MAX_PREPARED_PATTERN_BYTES)
            })
            .is_ok()
    }

    fn get_or_compile(
        &self,
        grammar_id: GrammarId,
        pattern_id: PatternId,
        pattern: &str,
        live_captures: Option<Vec<u32>>,
    ) -> (Arc<CompiledPattern>, bool, bool) {
        let slot = self
            .slots
            .get(grammar_id.0 as usize)
            .and_then(Option::as_ref)
            .and_then(|grammar| grammar.get(pattern_id.0 as usize));
        if let Some(Some(compiled)) = slot.and_then(OnceLock::get)
            && compiled.has_live_captures(live_captures.as_deref())
        {
            return (Arc::clone(compiled), false, true);
        }

        // Distinct misses otherwise compile their full parser/matcher payloads
        // before byte admission can reject them. Bound transient preparation
        // memory by allowing one such construction at a time. Cache hits avoid
        // this permit entirely.
        let _compile_permit = self
            .compile_permit
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(slot) = slot else {
            let compiled = Arc::new(match live_captures {
                Some(live_captures) => {
                    CompiledPattern::new_with_live_captures(pattern, live_captures)
                }
                None => CompiledPattern::new(pattern),
            });
            return (compiled, true, false);
        };
        let mut compiled_now = false;
        let mut rejected = None;
        let winner = slot.get_or_init(|| {
            compiled_now = true;
            let compiled = Arc::new(match live_captures.as_ref() {
                Some(live_captures) => {
                    CompiledPattern::new_with_live_captures(pattern, live_captures.clone())
                }
                None => CompiledPattern::new(pattern),
            });
            if self.try_reserve_pattern_bytes(compiled.prepared_retained_bytes()) {
                Some(compiled)
            } else {
                rejected = Some(compiled);
                None
            }
        });
        let Some(winner) = winner else {
            let compiled = rejected.unwrap_or_else(|| {
                Arc::new(match live_captures {
                    Some(live_captures) => {
                        CompiledPattern::new_with_live_captures(pattern, live_captures)
                    }
                    None => CompiledPattern::new(pattern),
                })
            });
            return (compiled, true, false);
        };
        if winner.has_live_captures(live_captures.as_deref()) {
            return (Arc::clone(winner), compiled_now, true);
        }

        // A PatternId normally has one capture layout. Preserve the safe
        // fallback for malformed/synthetic grammars that request a genuinely
        // different layout without replacing the canonical prepared value.
        let compiled = Arc::new(match live_captures {
            Some(live_captures) => CompiledPattern::new_with_live_captures(pattern, live_captures),
            None => CompiledPattern::new(pattern),
        });
        debug_assert!(!winner.has_same_live_captures(&compiled));
        (compiled, true, false)
    }

    fn capacity(&self) -> usize {
        self.capacity
    }

    fn retained_bytes(&self) -> usize {
        self.retained_bytes.load(Ordering::Acquire)
    }

    fn initialized_count(&self) -> usize {
        let mut count = 0usize;
        for grammar in self.slots.iter().flatten() {
            for slot in grammar.as_ref() {
                count += usize::from(matches!(slot.get(), Some(Some(_))));
            }
        }
        count
    }
}

#[derive(Debug, Default)]
struct PreparedBlueprintCache {
    state: Mutex<PreparedBlueprintCacheState>,
    initialized: Condvar,
    build_permit: Mutex<()>,
}

#[derive(Debug, Default)]
struct PreparedBlueprintCacheState {
    blueprints: FastMap<PreparedBlueprintKey, (Arc<CandidateBlueprint>, usize)>,
    blueprint_bytes: usize,
    building: FastMap<PreparedBlueprintKey, usize>,
    building_key_bytes: usize,
    injection_outcome_ids: FastMap<InjectionOutcome, PreparedInjectionOutcomeId>,
    injection_outcome_bytes: usize,
    next_injection_outcome_id: u64,
    // The first insertion is the eagerly prepared root descriptor. Keep it in
    // the bounded map so statistics include every artifact retained solely by
    // the prepared value and fresh tokenizers can always bind it.
    pinned_root: Option<PreparedBlueprintKey>,
}

struct PreparedBlueprintBuildGuard<'a> {
    cache: &'a PreparedBlueprintCache,
    key: Option<PreparedBlueprintKey>,
}

impl Drop for PreparedBlueprintBuildGuard<'_> {
    fn drop(&mut self) {
        let Some(key) = self.key.take() else {
            return;
        };
        let mut state = self
            .cache
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(key_bytes) = state.building.remove(&key) {
            state.building_key_bytes = state.building_key_bytes.saturating_sub(key_bytes);
        }
        drop(state);
        self.cache.initialized.notify_all();
    }
}

impl PreparedBlueprintCache {
    fn intern_injection_outcome(
        &self,
        injection_outcome: &InjectionOutcome,
    ) -> Option<PreparedInjectionOutcomeId> {
        let outcome_bytes = injection_outcome_retained_bytes(injection_outcome);
        if outcome_bytes > MAX_PREPARED_INJECTION_OUTCOME_BYTES {
            // Oversized grammar-owned outcomes stay tokenizer-local rather
            // than defeating the preparation's retained-byte bound.
            return None;
        }
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let injection_outcome = if let Some(id) = state.injection_outcome_ids.get(injection_outcome)
        {
            *id
        } else {
            if state.injection_outcome_ids.len() >= MAX_INJECTION_OUTCOMES
                || state.injection_outcome_bytes.saturating_add(outcome_bytes)
                    > MAX_PREPARED_INJECTION_OUTCOME_BYTES
            {
                state.injection_outcome_ids = hashing::fast_map();
                state.injection_outcome_bytes = 0;
            }
            let id = PreparedInjectionOutcomeId(state.next_injection_outcome_id);
            state.next_injection_outcome_id = state.next_injection_outcome_id.wrapping_add(1);
            state
                .injection_outcome_ids
                .insert(injection_outcome.clone(), id);
            state.injection_outcome_bytes =
                state.injection_outcome_bytes.saturating_add(outcome_bytes);
            id
        };
        Some(injection_outcome)
    }

    fn get_or_insert_with(
        &self,
        key: PreparedBlueprintKey,
        build: impl FnOnce() -> (Arc<CandidateBlueprint>, bool),
    ) -> Arc<CandidateBlueprint> {
        let key_bytes = prepared_blueprint_key_retained_bytes(&key);
        if key_bytes > MAX_PREPARED_BLUEPRINT_KEY_BYTES {
            let _build_permit = self
                .build_permit
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            return build().0;
        }
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        loop {
            if let Some((blueprint, _)) = state.blueprints.get(&key) {
                return Arc::clone(blueprint);
            }
            if state.building.contains_key(&key)
                || state.building.len() >= MAX_CANDIDATE_BLUEPRINTS
                || state.building_key_bytes.saturating_add(key_bytes)
                    > MAX_PREPARED_BLUEPRINT_KEY_BYTES
            {
                state = self
                    .initialized
                    .wait(state)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                continue;
            }
            state.building.insert(key.clone(), key_bytes);
            state.building_key_bytes = state.building_key_bytes.saturating_add(key_bytes);
            break;
        }
        drop(state);

        // Candidate and scanner payload sizes are known only after building.
        // Serialize those payload builds so distinct oversized misses cannot
        // multiply their transient allocation before admission rejects them.
        let _build_permit = self
            .build_permit
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut build_guard = PreparedBlueprintBuildGuard {
            cache: self,
            key: Some(key.clone()),
        };
        let (blueprint, cacheable) = build();
        let blueprint_bytes = candidate_blueprint_retained_bytes(&blueprint);
        let retained_bytes = key_bytes.saturating_add(blueprint_bytes);
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let removed = state.building.remove(&key);
        debug_assert!(
            removed.is_some(),
            "prepared-blueprint build key was not registered"
        );
        if let Some(building_bytes) = removed {
            state.building_key_bytes = state.building_key_bytes.saturating_sub(building_bytes);
        }
        if cacheable && retained_bytes <= MAX_PREPARED_BLUEPRINT_BYTES {
            if state.pinned_root.is_none() {
                state.pinned_root = Some(key.clone());
            }
            if state.blueprints.len() >= MAX_CANDIDATE_BLUEPRINTS
                || state.blueprint_bytes.saturating_add(retained_bytes)
                    > MAX_PREPARED_BLUEPRINT_BYTES
            {
                let pinned = state.pinned_root.as_ref().and_then(|root| {
                    state
                        .blueprints
                        .get(root)
                        .map(|(blueprint, bytes)| (root.clone(), Arc::clone(blueprint), *bytes))
                });
                state.blueprints.clear();
                state.blueprint_bytes = 0;
                if let Some((root, blueprint, bytes)) = pinned {
                    state.blueprints.insert(root, (blueprint, bytes));
                    state.blueprint_bytes = bytes;
                }
            }
            if state.blueprint_bytes.saturating_add(retained_bytes) <= MAX_PREPARED_BLUEPRINT_BYTES
            {
                state
                    .blueprints
                    .insert(key, (Arc::clone(&blueprint), retained_bytes));
                state.blueprint_bytes = state.blueprint_bytes.saturating_add(retained_bytes);
            }
        }
        build_guard.key = None;
        drop(state);
        self.initialized.notify_all();
        blueprint
    }

    fn retains(&self, blueprint: &Arc<CandidateBlueprint>) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .blueprints
            .values()
            .any(|(cached, _)| Arc::ptr_eq(cached, blueprint))
    }

    fn len(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .blueprints
            .len()
    }

    fn retained_bytes(&self) -> usize {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state
            .blueprint_bytes
            .saturating_add(state.injection_outcome_bytes)
    }
}

#[derive(Debug, Clone)]
pub struct TextMateTokenizer {
    grammars: GrammarSet,
    root: GrammarId,
    root_scope_key: String,
    injection_selectors: Arc<Vec<CompiledInjectionSelector>>,
    matcher_cache: FastMap<(GrammarId, PatternId), Arc<CompiledPattern>>,
    unprepared_static_matcher_generation: usize,
    prepared_pattern_cache: Option<Arc<PreparedPatternCache>>,
    prepared_blueprint_cache: Option<Arc<PreparedBlueprintCache>>,
    dynamic_matcher_cache: FastMap<DynamicMatcherKey, Arc<CompiledPattern>>,
    scope_names: ScopeInterner,
    scope_templates: ScopeTemplateInterner,
    scope_stacks: ScopeStackInterner,
    current_scope_stack_cache: FastMap<CurrentScopeStackKey, CachedCurrentScopeStackIds>,
    resolved_scope_stack_cache: FastMap<ScopeStackId, Arc<[Arc<str>]>>,
    scope_resolution_scratch: Vec<ScopeId>,
    output_scope_table_cache: OutputScopeTableCache,
    capture_scope_templates: FastMap<(GrammarId, ScopeId), ScopeTemplateId>,
    state_interner: StateInterner,
    line_cache: LineCache<LineCacheKey, CachedLine>,
    candidate_cache: HashMap<StateId, Arc<CandidateSet>, BuildHasherDefault<StateIdentityHasher>>,
    candidate_blueprint_cache: FastMap<CandidateBlueprintKey, BoundCandidateBlueprint>,
    injection_outcomes: InjectionOutcomeInterner,
    injection_outcome_cache: FastMap<ScopeStackId, (InjectionOutcomeId, Arc<InjectionOutcome>)>,
    prepared_injection_outcome_ids: FastMap<InjectionOutcomeId, Option<PreparedInjectionOutcomeId>>,
    inline_candidate_cache: FastMap<InlineCandidateCacheKey, Arc<CandidateSet>>,
    include_availability_cache: RefCell<HashMap<IncludeAvailabilityNode, bool>>,
    include_repository_names: RefCell<RepositoryNameInterner>,
    rule_repository_contexts: Arc<RuleRepositoryContexts>,
    /// Owns exact frame identities and stack edges for this tokenizer.
    frame_stack_interner: FrameStackInternTable,
    /// Repeat pushes of a known (parent stack, frame) transition skip interner lookup.
    frame_edge_cache: FastMap<(InternedFrameStackId, InternedFrameId), InternedFrameStackId>,
    /// Precomputed identities for grammar-static frames, scoped to this interner.
    static_frame_identities: FastMap<(GrammarId, RuleId, bool), StaticFrameIdentity>,
    /// Immutable frame nodes from previous pushes, keyed by the same edge, so
    /// a repeated transition reuses one shared allocation instead of
    /// constructing and hashing a fresh `Frame`.
    frame_node_cache: FastMap<(InternedFrameStackId, InternedFrameId), Arc<FrameNode>>,
    regex_scratch: super::regex::bytecode::BytecodeScratch,
    /// Final capture vectors are needed only until the winning rule is
    /// applied. Recycle a bounded number across matches instead of allocating
    /// once per captured token; nested capture retokenization naturally uses
    /// another pool entry while the outer result is live.
    capture_result_pool: Vec<Vec<Option<Range<usize>>>>,
    pattern_hotspots: HashMap<PatternHotspotKey, PatternHotspot>,
    max_line_bytes: Option<usize>,
    fallback_call_budget_remaining: Option<u64>,
    counters: EngineCounters,
    counters_enabled: bool,
    hot_counters_enabled: bool,
    degraded_since_last: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum IncludeAvailabilityNode {
    Rule(GrammarId, GrammarId, RuleId),
    Repository(GrammarId, GrammarId, RepositoryNameId),
    TopLevel(GrammarId, GrammarId),
}

impl TextMateTokenizer {
    pub fn new(grammars: GrammarSet, root: GrammarId) -> Self {
        Self::new_inner(grammars, root, None, None, None, None)
    }

    fn new_with_prepared_caches(
        grammars: GrammarSet,
        root: GrammarId,
        prepared_patterns: Arc<PreparedPatternCache>,
        prepared_blueprints: Arc<PreparedBlueprintCache>,
        injection_selectors: Vec<CompiledInjectionSelector>,
        rule_repository_contexts: Arc<RuleRepositoryContexts>,
    ) -> Self {
        Self::new_inner(
            grammars,
            root,
            Some(prepared_patterns),
            Some(prepared_blueprints),
            Some(Arc::new(injection_selectors)),
            Some(rule_repository_contexts),
        )
    }

    fn new_inner(
        grammars: GrammarSet,
        root: GrammarId,
        prepared_pattern_cache: Option<Arc<PreparedPatternCache>>,
        prepared_blueprint_cache: Option<Arc<PreparedBlueprintCache>>,
        injection_selectors: Option<Arc<Vec<CompiledInjectionSelector>>>,
        rule_repository_contexts: Option<Arc<RuleRepositoryContexts>>,
    ) -> Self {
        let root_scope_key = grammars
            .grammar(root)
            .map(|grammar| grammar.scope_name.clone())
            .unwrap_or_else(|| format!("grammar:{}", root.0));
        let injection_selectors = injection_selectors
            .unwrap_or_else(|| Arc::new(compile_injection_selectors(&grammars, root)));
        let rule_repository_contexts = rule_repository_contexts
            .unwrap_or_else(|| grammars.rule_repository_contexts(root, &injection_selectors));
        Self {
            grammars,
            root,
            root_scope_key,
            injection_selectors,
            matcher_cache: hashing::fast_map(),
            unprepared_static_matcher_generation: 0,
            prepared_pattern_cache,
            prepared_blueprint_cache,
            dynamic_matcher_cache: hashing::fast_map(),
            scope_names: ScopeInterner::default(),
            scope_templates: ScopeTemplateInterner::default(),
            scope_stacks: ScopeStackInterner::default(),
            current_scope_stack_cache: hashing::fast_map(),
            resolved_scope_stack_cache: hashing::fast_map(),
            scope_resolution_scratch: Vec::new(),
            output_scope_table_cache: OutputScopeTableCache::default(),
            capture_scope_templates: hashing::fast_map(),
            state_interner: StateInterner::new(),
            line_cache: LineCache::new(0),
            candidate_cache: HashMap::with_hasher(BuildHasherDefault::default()),
            candidate_blueprint_cache: hashing::fast_map(),
            injection_outcomes: InjectionOutcomeInterner::default(),
            injection_outcome_cache: hashing::fast_map(),
            prepared_injection_outcome_ids: hashing::fast_map(),
            inline_candidate_cache: hashing::fast_map(),
            include_availability_cache: RefCell::new(HashMap::new()),
            include_repository_names: RefCell::new(RepositoryNameInterner::default()),
            rule_repository_contexts,
            frame_stack_interner: FrameStackInternTable::new(),
            frame_edge_cache: hashing::fast_map(),
            static_frame_identities: hashing::fast_map(),
            frame_node_cache: hashing::fast_map(),
            regex_scratch: super::regex::bytecode::BytecodeScratch::default(),
            capture_result_pool: Vec::new(),
            pattern_hotspots: HashMap::new(),
            max_line_bytes: None,
            fallback_call_budget_remaining: None,
            counters: EngineCounters::default(),
            counters_enabled: false,
            hot_counters_enabled: false,
            degraded_since_last: false,
        }
    }

    fn prepare_root_candidate(&mut self) {
        let candidates = self.cached_candidates_for_state(&TokenizerState::default());
        let rejected = self
            .prepared_blueprint_cache
            .as_ref()
            .is_some_and(|cache| !cache.retains(candidates.blueprint.blueprint_arc()));
        if rejected {
            // The root descriptor remains usable for this construction call,
            // but an oversized value must not become part of the clonable
            // preparation prototype after cache admission rejected it.
            drop(candidates);
            self.clear_candidate_cache();
            self.matcher_cache.clear();
            self.dynamic_matcher_cache.clear();
        }
    }

    pub fn from_grammar(contents: &str) -> Result<Self, GrammarLoadError> {
        let mut grammars = GrammarSet::new();
        let root = grammars.load_and_add(contents)?;
        Ok(Self::new(grammars, root))
    }

    pub fn tokenize_source(&mut self, source: &str) -> HighlightedText {
        let previous_budget = self
            .fallback_call_budget_remaining
            .replace(fallback_call_budget(source.len()));
        let mut state = TokenizerState::default();
        let mut lines = Vec::with_capacity(source.len().div_ceil(40).max(1));
        let mut scope_table = OutputScopeTableBuilder::new();
        // Reuse one placeholder while the result-wide scope table is built.
        // Constructing `Arc::default()` here for every line used to perform
        // several immediately discarded heap allocations per source line.
        let empty_scope_table = HighlightScopeTable::empty_shared();
        for (line_index, chunk) in LineChunks::new(source).enumerate() {
            let tokenized = self.tokenize_line_compact_at_line(chunk.parse_text, state, line_index);
            state = tokenized.state.clone();
            let fingerprint = if chunk.parse_text.ends_with('\n') {
                tokenized.parse_fingerprint.without_trailing_byte(b'\n')
            } else {
                tokenized.parse_fingerprint
            };
            lines.push(self.build_highlighted_line(
                chunk.text,
                fingerprint,
                &tokenized.tokens,
                &mut scope_table,
                &empty_scope_table,
            ));
        }
        self.fallback_call_budget_remaining = previous_budget;
        let scope_table = scope_table.finish(
            &self.scope_stacks,
            &self.scope_names,
            &mut self.output_scope_table_cache,
        );
        for line in &mut lines {
            line.scope_table = Arc::clone(&scope_table);
        }
        HighlightedText { lines }
    }

    fn tokenize_viewport_compact(
        &mut self,
        source: &str,
        visible: Range<usize>,
        checkpoints: &mut CheckpointTable,
    ) -> Vec<CompactTokenizedLine> {
        if visible.start >= visible.end || LineChunks::new(source).nth(visible.start).is_none() {
            return Vec::new();
        }
        let visible_end = visible.end;
        let checkpoint = checkpoints.nearest_before(visible.start).unwrap_or(
            super::checkpoint::LineCheckpoint {
                line_index: 0,
                state: StateId(0),
            },
        );
        let (resume_line, mut state) = self
            .state_for_id(checkpoint.state)
            .cloned()
            .map(|state| (checkpoint.line_index, state))
            .unwrap_or((0, TokenizerState::default()));
        self.record_checkpoint_replay_lines(visible.start.saturating_sub(resume_line));

        let mut visible_lines = Vec::new();
        for (line_index, chunk) in LineChunks::new(source)
            .enumerate()
            .take(visible_end)
            .skip(resume_line)
        {
            let tokenized = self.tokenize_line_compact_at_line(chunk.parse_text, state, line_index);
            state = tokenized.state.clone();
            checkpoints.record_if_boundary(line_index + 1, tokenized.exit_state_id);
            if line_index >= visible.start {
                visible_lines.push(tokenized);
            }
        }
        visible_lines
    }

    pub fn tokenize_viewport_scopes(
        &mut self,
        source: &str,
        visible: Range<usize>,
        checkpoints: &mut CheckpointTable,
    ) -> Vec<TokenizedLine> {
        self.tokenize_viewport_compact(source, visible, checkpoints)
            .into_iter()
            .map(|line| self.resolve_compact_line(line))
            .collect()
    }

    pub fn highlight_viewport(
        &mut self,
        source: &str,
        visible: Range<usize>,
        checkpoints: &mut CheckpointTable,
    ) -> HighlightedText {
        let visible_start = visible.start;
        let previous_budget = self
            .fallback_call_budget_remaining
            .replace(fallback_call_budget(source.len()));
        let tokenized = self.tokenize_viewport_compact(source, visible, checkpoints);
        self.fallback_call_budget_remaining = previous_budget;
        let mut scope_table = OutputScopeTableBuilder::new();
        let empty_scope_table = HighlightScopeTable::empty_shared();
        let mut lines = tokenized
            .iter()
            .zip(LineChunks::new(source).skip(visible_start))
            .map(|(tokenized, chunk)| {
                let fingerprint = if chunk.parse_text.ends_with('\n') {
                    tokenized.parse_fingerprint.without_trailing_byte(b'\n')
                } else {
                    tokenized.parse_fingerprint
                };
                self.build_highlighted_line(
                    chunk.text,
                    fingerprint,
                    &tokenized.tokens,
                    &mut scope_table,
                    &empty_scope_table,
                )
            })
            .collect::<Vec<_>>();
        let scope_table = scope_table.finish(
            &self.scope_stacks,
            &self.scope_names,
            &mut self.output_scope_table_cache,
        );
        for line in &mut lines {
            line.scope_table = Arc::clone(&scope_table);
        }
        HighlightedText { lines }
    }

    pub fn tokenize_line_scopes(
        &mut self,
        parse_text: &str,
        state: TokenizerState,
    ) -> TokenizedLine {
        self.tokenize_line_scopes_at_line(parse_text, state, 0)
    }

    pub fn tokenize_line_scopes_at_line(
        &mut self,
        parse_text: &str,
        state: TokenizerState,
        line_index: usize,
    ) -> TokenizedLine {
        let compact = self.tokenize_line_compact_at_line(parse_text, state, line_index);
        self.resolve_compact_line(compact)
    }

    pub(crate) fn tokenize_line_shared_scopes(
        &mut self,
        parse_text: &str,
        state: TokenizerState,
    ) -> SharedTokenizedLine {
        let compact = self.tokenize_line_compact_at_line(parse_text, state, 0);
        self.resolve_shared_compact_line(compact)
    }

    pub(crate) fn tokenize_line_shared_scopes_skipped(
        &mut self,
        parse_text: &str,
        state: TokenizerState,
    ) -> SharedTokenizedLine {
        let compact = self.tokenize_line_compact_at_line_inner(parse_text, state, 0, true);
        self.resolve_shared_compact_line(compact)
    }

    fn resolve_shared_compact_line(
        &mut self,
        compact: CompactTokenizedLine,
    ) -> SharedTokenizedLine {
        let mut tokens = Vec::with_capacity(compact.tokens.len());
        for token in compact.tokens.iter() {
            tokens.push(SharedScopedToken {
                range: token.range.clone(),
                scopes: self.resolve_scope_stack_cached(token.stack),
            });
        }
        SharedTokenizedLine {
            tokens,
            state: compact.state,
        }
    }

    pub(crate) fn tokenize_line_shared_scopes_with(
        &mut self,
        parse_text: &str,
        state: TokenizerState,
        sink: &mut impl SharedScopeSink,
    ) -> TokenizerState {
        let compact = self.tokenize_line_compact_at_line(parse_text, state, 0);
        self.resolve_shared_compact_line_with(compact, sink)
    }

    pub(crate) fn tokenize_line_shared_scopes_skipped_with(
        &mut self,
        parse_text: &str,
        state: TokenizerState,
        sink: &mut impl SharedScopeSink,
    ) -> TokenizerState {
        let compact = self.tokenize_line_compact_at_line_inner(parse_text, state, 0, true);
        self.resolve_shared_compact_line_with(compact, sink)
    }

    fn resolve_shared_compact_line_with(
        &mut self,
        compact: CompactTokenizedLine,
        sink: &mut impl SharedScopeSink,
    ) -> TokenizerState {
        sink.reserve(compact.tokens.len());
        for token in compact.tokens.iter() {
            sink.push(
                token.range.clone(),
                token.stack,
                self.resolve_scope_stack_cached(token.stack),
            );
        }
        compact.state
    }

    fn tokenize_line_compact_at_line(
        &mut self,
        parse_text: &str,
        state: TokenizerState,
        line_index: usize,
    ) -> CompactTokenizedLine {
        self.tokenize_line_compact_at_line_inner(parse_text, state, line_index, false)
    }

    fn tokenize_line_compact_at_line_inner(
        &mut self,
        parse_text: &str,
        mut state: TokenizerState,
        line_index: usize,
        force_degraded: bool,
    ) -> CompactTokenizedLine {
        let is_first_line = line_index == 0;
        self.record_line_tokenized();
        // Explicitly invalidate scan-local occurrence cursors even when a
        // caller reuses the same String allocation for different line text.
        // Pointer/length identity alone is insufficient in that API pattern.
        self.regex_scratch.begin_line(parse_text);
        let parse_fingerprint = LineTextFingerprint::from_text(parse_text);
        let entry_state_id = self.intern_state(&state);
        if force_degraded || self.fallback_call_budget_remaining == Some(0) {
            self.record_line_skipped();
            self.record_degraded_line();
            let stack = self.current_scope_stack_id(&state, true, None);
            return CompactTokenizedLine {
                tokens: plain_compact_tokens(parse_text, stack).into(),
                state,
                entry_state_id,
                exit_state_id: entry_state_id,
                parse_fingerprint,
            };
        }
        if self
            .max_line_bytes
            .is_some_and(|max_line_bytes| parse_text.len() > max_line_bytes)
        {
            self.record_line_skipped();
            self.record_degraded_line();
            let stack = self.current_scope_stack_id(&state, true, None);
            return CompactTokenizedLine {
                tokens: plain_compact_tokens(parse_text, stack).into(),
                state,
                entry_state_id,
                exit_state_id: entry_state_id,
                parse_fingerprint,
            };
        }
        let cache_key = self.line_cache_key(entry_state_id, parse_fingerprint, is_first_line);
        if self.line_cache.is_enabled() {
            if let Some(cached) = self.line_cache.get(&cache_key)
                && cached.text.as_ref() == parse_text
                && let Some(exit_state) = self.state_for_id(cached.exit).cloned()
            {
                self.record_line_cache_hit();
                return CompactTokenizedLine {
                    tokens: CompactLineTokens::Shared(cached.tokens),
                    state: exit_state,
                    entry_state_id,
                    exit_state_id: cached.exit,
                    parse_fingerprint,
                };
            }
            self.record_line_cache_miss();
        }

        let mut tokens = Vec::with_capacity(parse_text.len().div_ceil(2).min(256));
        let mut cursor = 0usize;
        let (suppressed_begin_rules, while_anchor_pos) =
            self.apply_while_continuations(parse_text, &mut state, &mut tokens, &mut cursor);

        let mut steps = 0usize;
        let mut fallback_steps = 0u64;
        let mut degraded = false;
        let mut anchor_pos = while_anchor_pos.or_else(|| {
            if cursor > 0 {
                Some(cursor)
            } else {
                state
                    .frames
                    .last()
                    .is_some_and(|frame| frame.begin_captured_eol)
                    .then_some(0)
            }
        });
        // vscode-textmate keeps a line-local anchor position stack for `\G`.
        // Existing frames only need a synthetic restore value when they pop;
        // avoid materializing one `None` per deep frame on every line.
        let line_entry_depth = state.depth();
        let mut frame_anchor_positions = Vec::new();
        let mut loop_candidates = None;
        let mut zero_width_states = HashSet::new();
        // End rules such as `$` are zero-width at the logical line end. Keep
        // evaluating while frames remain so line-scoped rules close even when
        // callers pass a line without its terminating newline.
        while (cursor < parse_text.len() || !state.frames.is_empty())
            && steps < MAX_TOKENIZER_STEPS_PER_LINE
        {
            steps += 1;
            if loop_candidates.is_none() {
                loop_candidates = Some(self.cached_candidates_for_state(&state));
            }
            let candidates = loop_candidates
                .as_ref()
                .expect("candidate set initialized for tokenizer step");
            let search = self.find_best_candidate(
                candidates,
                parse_text,
                cursor,
                is_first_line,
                anchor_pos,
                Some(&suppressed_begin_rules),
            );
            degraded |= search.fallback_budget_killed;
            fallback_steps = fallback_steps.saturating_add(search.fallback_steps);
            if fallback_steps > MAX_FALLBACK_STEPS_PER_LINE
                || !self.consume_fallback_call_budget(search.fallback_steps)
            {
                if let Some(counters) = self.counters_mut() {
                    counters.record_fallback_budget_kill();
                }
                degraded = true;
                self.push_token(
                    &mut tokens,
                    cursor..parse_text.len(),
                    candidates.active_stack_id,
                );
                break;
            }
            let Some((candidate_index, result)) = search.best else {
                self.push_token(
                    &mut tokens,
                    cursor..parse_text.len(),
                    candidates.active_stack_id,
                );
                break;
            };
            let state_changes = !matches!(
                candidates.candidates[candidate_index].kind,
                CandidateKind::Match { .. }
            );
            let result_start = result.start;
            let result_end = result.end;

            if result_start > cursor {
                self.push_token(
                    &mut tokens,
                    cursor..result_start,
                    candidates.active_stack_id,
                );
            }

            let depth_before = state.depth();
            let stack_before = state.frames.interned_id();
            let zero_width_state_before =
                (result_start == result_end && state_changes).then(|| state.clone());
            let zero_width_match_rule = result_start == result_end
                && matches!(
                    &candidates.candidates[candidate_index].kind,
                    CandidateKind::Match { .. }
                );
            let next_cursor = self.apply_candidate(
                parse_text,
                &mut state,
                &mut tokens,
                &candidates.candidates[candidate_index],
                candidates.blueprint.match_name_template(candidate_index),
                result,
                &mut anchor_pos,
                &mut frame_anchor_positions,
                line_entry_depth,
                candidates.active_stack_id,
                candidates.end_stack_id,
            );
            if zero_width_match_rule {
                // vscode-textmate stops the current line when an ordinary
                // MatchRule wins without consuming input. Advancing one scalar
                // would let lower-priority rules color text that the oracle
                // leaves in the active scope (and can skip byte zero entirely).
                let stack = self.current_scope_stack_id(&state, true, None);
                self.push_token(&mut tokens, result_start..parse_text.len(), stack);
                cursor = parse_text.len();
                break;
            }
            let zero_width_state_change =
                next_cursor == result_start && state.depth() != depth_before;
            if zero_width_state_change {
                zero_width_states.insert((result_start, stack_before));
                if !zero_width_states.insert((result_start, state.frames.interned_id())) {
                    // A zero-width begin/end pair can return to an already
                    // visited state without consuming input. vscode-textmate
                    // stops on the state before the operation that completed
                    // the cycle (for an immediate zero-width end, that means
                    // retaining the frame it just tried to pop).
                    if let Some(previous_state) = zero_width_state_before {
                        state = previous_state;
                    }
                    let stack = self.current_scope_stack_id(&state, true, None);
                    self.push_token(&mut tokens, result_start..parse_text.len(), stack);
                    cursor = parse_text.len();
                    break;
                }
            }
            cursor = if zero_width_state_change {
                next_cursor
            } else if next_cursor <= result_start {
                next_char_boundary(parse_text, result_start)
            } else {
                next_cursor
            };
            if state_changes {
                loop_candidates = None;
            }
        }

        if steps >= MAX_TOKENIZER_STEPS_PER_LINE && cursor < parse_text.len() {
            degraded = true;
            let stack = self.current_scope_stack_id(&state, true, None);
            self.push_token(&mut tokens, cursor..parse_text.len(), stack);
        }
        if degraded {
            self.record_degraded_line();
        }

        let exit_state_id = self.intern_state(&state);
        let tokens = if self.line_cache.is_enabled() {
            let tokens: Arc<[CompactScopedToken]> = tokens.into();
            let evicted = self.line_cache.insert(
                cache_key,
                CachedLine {
                    text: Arc::from(parse_text),
                    tokens: Arc::clone(&tokens),
                    exit: exit_state_id,
                },
            );
            if evicted {
                self.record_line_cache_eviction();
            }
            CompactLineTokens::Shared(tokens)
        } else {
            CompactLineTokens::Owned(tokens)
        };
        CompactTokenizedLine {
            tokens,
            state,
            entry_state_id,
            exit_state_id,
            parse_fingerprint,
        }
    }

    pub fn grammars(&self) -> &GrammarSet {
        &self.grammars
    }

    pub fn set_root(&mut self, root: GrammarId) {
        if self.root == root {
            return;
        }
        debug_assert!(self.grammars.grammar(root).is_some());
        // Prepared caches are scoped to their original root closure. A
        // tokenizer explicitly repointed at another root becomes an ordinary
        // tokenizer instead of reaching outside that immutable preparation.
        self.prepared_pattern_cache = None;
        self.prepared_blueprint_cache = None;
        self.root = root;
        self.root_scope_key = self
            .grammars
            .grammar(root)
            .map(|grammar| grammar.scope_name.clone())
            .unwrap_or_else(|| format!("grammar:{}", root.0));
        let injection_selectors = Arc::new(compile_injection_selectors(&self.grammars, root));
        let rule_repository_contexts = self
            .grammars
            .rule_repository_contexts(root, &injection_selectors);
        self.injection_selectors = injection_selectors;
        self.include_availability_cache.borrow_mut().clear();
        self.include_repository_names.borrow_mut().clear();
        self.rule_repository_contexts = rule_repository_contexts;
        self.current_scope_stack_cache.clear();
        self.clear_line_cache();
        self.clear_candidate_cache();
    }

    pub fn intern_state(&mut self, state: &TokenizerState) -> StateId {
        let (id, inserted) = self.state_interner.intern(state);
        if let Some(counters) = self.counters_mut() {
            if inserted {
                counters.record_state_cache_miss();
            } else {
                counters.record_state_cache_hit();
            }
        }
        id
    }

    pub fn state_for_id(&self, id: StateId) -> Option<&TokenizerState> {
        self.state_interner.get(id)
    }

    pub fn interned_state_count(&self) -> usize {
        self.state_interner.len()
    }

    pub fn set_line_cache_capacity(&mut self, capacity: usize) {
        self.line_cache.set_capacity(capacity);
    }

    pub fn line_cache_capacity(&self) -> usize {
        self.line_cache.capacity()
    }

    pub fn line_cache_len(&self) -> usize {
        self.line_cache.len()
    }

    pub fn clear_line_cache(&mut self) {
        self.line_cache.clear();
    }

    pub fn candidate_cache_len(&self) -> usize {
        self.candidate_cache.len()
    }

    pub fn clear_candidate_cache(&mut self) {
        self.candidate_cache.clear();
        self.candidate_blueprint_cache.clear();
        self.current_scope_stack_cache.clear();
        self.resolved_scope_stack_cache.clear();
        self.injection_outcomes.clear();
        self.injection_outcome_cache.clear();
        self.prepared_injection_outcome_ids.clear();
        self.inline_candidate_cache.clear();
    }

    pub fn set_max_line_bytes(&mut self, max_line_bytes: Option<usize>) {
        self.max_line_bytes = max_line_bytes;
    }

    pub fn max_line_bytes(&self) -> Option<usize> {
        self.max_line_bytes
    }

    pub fn configure_options(&mut self, options: crate::TokenizerOptions) {
        self.set_line_cache_capacity(options.line_cache_entries.max(1));
        self.set_max_line_bytes(Some(options.max_line_bytes));
    }

    pub fn set_counters_enabled(&mut self, enabled: bool) {
        self.counters_enabled = enabled;
    }

    pub fn set_hot_counters_enabled(&mut self, enabled: bool) {
        self.hot_counters_enabled = enabled;
    }

    pub fn counters_enabled(&self) -> bool {
        self.counters_enabled
    }

    pub fn counters(&self) -> EngineCounters {
        let mut counters = self.counters.clone();
        for hotspot in self.sorted_pattern_hotspots() {
            counters.merge_pattern_hotspot(hotspot);
        }
        counters.prune_pattern_hotspots();
        counters
    }

    pub fn reset_counters(&mut self) {
        self.counters = EngineCounters::default();
        self.pattern_hotspots.clear();
    }

    pub fn take_degraded(&mut self) -> bool {
        std::mem::take(&mut self.degraded_since_last)
    }

    pub fn take_counters(&mut self) -> EngineCounters {
        let mut counters = std::mem::take(&mut self.counters);
        for hotspot in self.sorted_pattern_hotspots() {
            counters.merge_pattern_hotspot(hotspot);
        }
        counters.prune_pattern_hotspots();
        self.pattern_hotspots.clear();
        counters
    }

    fn sorted_pattern_hotspots(&self) -> Vec<PatternHotspot> {
        let mut hotspots = self.pattern_hotspots.values().cloned().collect::<Vec<_>>();
        hotspots.sort_by(|left, right| {
            right
                .total_micros
                .cmp(&left.total_micros)
                .then_with(|| right.fallback_steps_total.cmp(&left.fallback_steps_total))
                .then_with(|| right.attempts.cmp(&left.attempts))
                .then_with(|| left.pattern.cmp(&right.pattern))
        });
        hotspots.truncate(128);
        hotspots
    }

    #[allow(clippy::too_many_arguments)]
    fn record_pattern_hotspot(
        &mut self,
        pattern: &str,
        pattern_id: Option<(GrammarId, PatternId)>,
        engine: &'static str,
        elapsed_micros: u64,
        matched: bool,
        fallback_steps: u64,
        fallback_budget_killed: bool,
        prefilter_may_match: Option<bool>,
    ) {
        if !self.counters_enabled || !self.hot_counters_enabled {
            return;
        }
        let grammar_id = pattern_id.map(|(grammar_id, _)| grammar_id.0);
        let pattern_id = pattern_id.map(|(_, pattern_id)| pattern_id.0);
        let key = PatternHotspotKey {
            root_scope: self.root_scope_key.clone(),
            grammar_id,
            pattern_id,
            engine: engine.to_owned(),
            pattern: pattern.to_owned(),
        };
        let hotspot = self
            .pattern_hotspots
            .entry(key)
            .or_insert_with(|| PatternHotspot {
                root_scope: self.root_scope_key.clone(),
                grammar_id,
                pattern_id,
                engine: engine.to_owned(),
                pattern: pattern.to_owned(),
                ..PatternHotspot::default()
            });
        hotspot.attempts = hotspot.attempts.saturating_add(1);
        if matched {
            hotspot.matches = hotspot.matches.saturating_add(1);
        }
        hotspot.total_micros = hotspot.total_micros.saturating_add(elapsed_micros);
        hotspot.fallback_steps_total = hotspot.fallback_steps_total.saturating_add(fallback_steps);
        hotspot.fallback_steps_max = hotspot.fallback_steps_max.max(fallback_steps);
        if fallback_budget_killed {
            hotspot.fallback_budget_kills = hotspot.fallback_budget_kills.saturating_add(1);
        }
        match prefilter_may_match {
            Some(true) => hotspot.prefilter_hits = hotspot.prefilter_hits.saturating_add(1),
            Some(false) => hotspot.prefilter_skips = hotspot.prefilter_skips.saturating_add(1),
            None => {}
        }
    }

    fn counters_mut(&mut self) -> Option<&mut EngineCounters> {
        if self.counters_enabled {
            Some(&mut self.counters)
        } else {
            None
        }
    }

    fn record_line_tokenized(&mut self) {
        if let Some(counters) = self.counters_mut() {
            counters.record_line_tokenized();
        }
    }

    fn record_line_skipped(&mut self) {
        if let Some(counters) = self.counters_mut() {
            counters.record_line_skipped();
        }
    }

    fn record_degraded_line(&mut self) {
        self.degraded_since_last = true;
        if let Some(counters) = self.counters_mut() {
            counters.record_degraded_line();
        }
    }

    fn record_line_cache_hit(&mut self) {
        if let Some(counters) = self.counters_mut() {
            counters.record_line_cache_hit();
        }
    }

    fn record_line_cache_miss(&mut self) {
        if let Some(counters) = self.counters_mut() {
            counters.record_line_cache_miss();
        }
    }

    fn record_line_cache_eviction(&mut self) {
        if let Some(counters) = self.counters_mut() {
            counters.record_line_cache_eviction();
        }
    }

    fn record_candidate_cache_hit(&mut self) {
        if let Some(counters) = self.counters_mut() {
            counters.record_candidate_list_cache_hit();
        }
    }

    fn record_candidate_cache_miss(&mut self) {
        if let Some(counters) = self.counters_mut() {
            counters.record_candidate_list_cache_miss();
        }
    }

    fn record_prefilter_check(&mut self, may_match: bool) {
        if let Some(counters) = self.counters_mut() {
            counters.record_prefilter_check(may_match);
        }
    }

    fn record_checkpoint_replay_lines(&mut self, lines: usize) {
        if lines > 0
            && let Some(counters) = self.counters_mut()
        {
            counters.record_checkpoint_replay_lines(lines);
        }
    }

    fn consume_fallback_call_budget(&mut self, steps: u64) -> bool {
        let Some(remaining) = self.fallback_call_budget_remaining.as_mut() else {
            return true;
        };
        if steps > *remaining {
            *remaining = 0;
            false
        } else {
            *remaining -= steps;
            true
        }
    }

    fn line_cache_key(
        &self,
        entry: StateId,
        fingerprint: LineTextFingerprint,
        first_line: bool,
    ) -> LineCacheKey {
        LineCacheKey {
            entry,
            first_line,
            fingerprint,
        }
    }

    fn build_highlighted_line(
        &self,
        text: &str,
        fingerprint: LineTextFingerprint,
        scoped_tokens: &[CompactScopedToken],
        scope_table: &mut OutputScopeTableBuilder,
        empty_scope_table: &Arc<HighlightScopeTable>,
    ) -> HighlightedLine {
        let mut line = HighlightedLine {
            fingerprint,
            segments: Vec::with_capacity(scoped_tokens.len()),
            scope_table: Arc::clone(empty_scope_table),
        };
        for token in scoped_tokens {
            let start = token.range.start.min(text.len());
            let end = token.range.end.min(text.len());
            if start >= end || !text.is_char_boundary(start) || !text.is_char_boundary(end) {
                continue;
            }
            let class = self.scope_stacks.class(token.stack);
            let stack = scope_table.intern_engine_stack(token.stack);
            push_segment(&mut line.segments, start, end, class, stack);
        }
        line
    }

    fn resolve_compact_line(&self, line: CompactTokenizedLine) -> TokenizedLine {
        let tokens = line
            .tokens
            .iter()
            .map(|token| ScopedToken {
                range: token.range.clone(),
                scopes: self.scope_stacks.resolve(token.stack, &self.scope_names),
            })
            .collect::<Vec<_>>()
            .into();
        TokenizedLine {
            tokens,
            state: line.state,
            entry_state_id: line.entry_state_id,
            exit_state_id: line.exit_state_id,
        }
    }

    fn apply_while_continuations(
        &mut self,
        line: &str,
        state: &mut TokenizerState,
        tokens: &mut Vec<CompactScopedToken>,
        cursor: &mut usize,
    ) -> (HashSet<(GrammarId, RuleId)>, Option<usize>) {
        let mut suppressed = HashSet::new();
        if state.frames.while_frame_count() == 0 {
            return (suppressed, None);
        }
        let mut anchor_pos = None;
        let mut while_frames = Vec::new();
        state.frames.for_each(|index, frame| {
            if frame.while_pattern.is_some() {
                while_frames.push(index);
            }
        });
        for index in while_frames {
            let Some(frame) = state.frames.get(index).cloned() else {
                break;
            };
            let Some(pattern) = frame.while_pattern.clone() else {
                continue;
            };
            let ctx = AnchorContext::continuation(*cursor);
            let result = self.find_pattern(
                &pattern,
                frame
                    .while_pattern_id
                    .map(|pattern_id| (frame.grammar_id, pattern_id)),
                line,
                *cursor,
                ctx,
            );
            match result {
                Some(result) if result.start == *cursor => {
                    let frame_state = state.prefix(index + 1);
                    let stack = self.current_scope_stack_id(&frame_state, true, None);
                    self.emit_match(
                        tokens,
                        line,
                        &result,
                        frame.grammar_id,
                        stack,
                        None,
                        None,
                        &frame.while_captures,
                    );
                    // A zero-width while match only validates continuation; it
                    // must not consume the first byte of the continued line.
                    *cursor = result.end;
                    // vscode-textmate uses the most recent successful while
                    // match as the line-local `\G` anchor. Nested begin/end
                    // rules can rely on that anchor to close at line start.
                    anchor_pos = Some(result.end);
                }
                _ => {
                    // A failed ancestor while condition also removes every
                    // child frame opened inside that continuation.
                    let mut has_child_end = false;
                    state.frames.for_each(|child_index, child| {
                        has_child_end |= child_index > index && child.end_pattern.is_some();
                    });
                    // A line-consuming container must not immediately reopen
                    // on its own closing delimiter after its while condition
                    // fails. Zero-width structural containers (notably YAML
                    // mappings) may legitimately start a sibling at the same
                    // position, so do not suppress those rules globally.
                    if frame.begin_captured_eol && has_child_end {
                        suppressed.insert((frame.grammar_id, frame.rule_id));
                    }
                    state.truncate_frames(index);
                    break;
                }
            }
        }
        (suppressed, anchor_pos)
    }

    fn candidates_for_state(
        &self,
        state: &TokenizerState,
        injections: &InjectionOutcome,
    ) -> Vec<Candidate> {
        let mut candidates = Vec::new();
        let mut order = 0usize;

        let (grammar_id, base_grammar_id, refs, end_candidate, apply_end_last) =
            if let Some(frame) = state.frames.last() {
                let end = frame.end_pattern.as_ref().map(|pattern| Candidate {
                    order: 0,
                    base_grammar_id: frame.base_grammar_id,
                    pattern: pattern.to_string(),
                    pattern_id: frame
                        .end_pattern_id
                        .map(|pattern_id| (frame.grammar_id, pattern_id)),
                    scope_prefix: frame.scope_prefix.clone(),
                    kind: CandidateKind::End {
                        grammar_id: frame.grammar_id,
                        captures: Arc::clone(&frame.end_captures),
                    },
                });
                (
                    frame.grammar_id,
                    frame.base_grammar_id,
                    frame.patterns.to_vec(),
                    end,
                    frame.apply_end_pattern_last,
                )
            } else {
                let Some(grammar) = self.grammars.grammar(self.root) else {
                    return candidates;
                };
                (self.root, self.root, grammar.top_level.clone(), None, false)
            };

        for injection in &injections.left {
            self.flatten_refs(
                injection.grammar_id,
                base_grammar_id,
                &injection.patterns,
                None,
                &mut candidates,
                &mut order,
                0,
            );
        }

        if let Some(end) = end_candidate.clone().filter(|_| !apply_end_last) {
            candidates.push(Candidate { order, ..end });
            order += 1;
        }

        self.flatten_refs(
            grammar_id,
            base_grammar_id,
            &refs,
            None,
            &mut candidates,
            &mut order,
            0,
        );

        if let Some(end) = end_candidate.filter(|_| apply_end_last) {
            candidates.push(Candidate { order, ..end });
            order += 1;
        }

        for injection in &injections.right {
            self.flatten_refs(
                injection.grammar_id,
                base_grammar_id,
                &injection.patterns,
                None,
                &mut candidates,
                &mut order,
                0,
            );
        }

        candidates
    }

    #[allow(clippy::too_many_arguments)]
    fn flatten_refs(
        &self,
        grammar_id: GrammarId,
        base_grammar_id: GrammarId,
        refs: &[RuleRef],
        scope_prefix: Option<Arc<str>>,
        out: &mut Vec<Candidate>,
        order: &mut usize,
        depth: usize,
    ) {
        if depth >= MAX_INCLUDE_DEPTH {
            return;
        }
        let Some(grammar) = self.grammars.grammar(grammar_id) else {
            return;
        };
        for rule_ref in refs {
            match rule_ref {
                RuleRef::Rule(rule_id) => {
                    let Some(rule) = grammar.rule(*rule_id) else {
                        continue;
                    };
                    let repository_context = self
                        .rule_repository_contexts
                        .get(grammar_id, *rule_id)
                        .map(Arc::as_ref);
                    match &rule.body {
                        RuleBody::Match {
                            pattern,
                            captures,
                            name,
                        } => {
                            let pattern_id = *pattern;
                            if let Some(pattern) = grammar.pattern(*pattern) {
                                out.push(Candidate {
                                    order: *order,
                                    base_grammar_id,
                                    pattern: pattern.to_owned(),
                                    pattern_id: Some((grammar_id, pattern_id)),
                                    scope_prefix: scope_prefix.clone(),
                                    kind: CandidateKind::Match {
                                        grammar_id,
                                        name: scope_name(grammar, *name),
                                        name_template: None,
                                        captures: contextualize_capture_spec(
                                            captures,
                                            repository_context,
                                        ),
                                    },
                                });
                                *order += 1;
                            }
                        }
                        RuleBody::BeginEnd {
                            begin,
                            end,
                            begin_captures,
                            end_captures,
                            name,
                            content_name,
                            apply_end_pattern_last,
                            patterns,
                        } => {
                            let patterns = contextualize_refs(patterns, repository_context);
                            if self.only_unavailable_includes(
                                grammar_id,
                                base_grammar_id,
                                &patterns,
                            ) {
                                continue;
                            }
                            let begin_pattern_id = *begin;
                            if let Some(begin) = grammar.pattern(*begin) {
                                let end_static = grammar
                                    .pattern(*end)
                                    .filter(|pattern| !pattern_has_backreference(pattern))
                                    .map(Arc::from);
                                out.push(Candidate {
                                    order: *order,
                                    base_grammar_id,
                                    pattern: begin.to_owned(),
                                    pattern_id: Some((grammar_id, begin_pattern_id)),
                                    scope_prefix: scope_prefix.clone(),
                                    kind: CandidateKind::BeginEnd {
                                        grammar_id,
                                        rule_id: rule.id,
                                        end: *end,
                                        begin_captures: contextualize_capture_spec(
                                            begin_captures,
                                            repository_context,
                                        ),
                                        end_captures: contextualize_capture_spec(
                                            end_captures,
                                            repository_context,
                                        ),
                                        name: scope_name(grammar, *name).map(Arc::from),
                                        content_name: scope_name(grammar, *content_name)
                                            .map(Arc::from),
                                        patterns: patterns.into(),
                                        apply_end_pattern_last: *apply_end_pattern_last,
                                        end_static,
                                    },
                                });
                                *order += 1;
                            }
                        }
                        RuleBody::BeginWhile {
                            begin,
                            while_pattern,
                            begin_captures,
                            while_captures,
                            name,
                            content_name,
                            patterns,
                        } => {
                            let patterns = contextualize_refs(patterns, repository_context);
                            if self.only_unavailable_includes(
                                grammar_id,
                                base_grammar_id,
                                &patterns,
                            ) {
                                continue;
                            }
                            let begin_pattern_id = *begin;
                            if let Some(begin) = grammar.pattern(*begin) {
                                let while_static = grammar
                                    .pattern(*while_pattern)
                                    .filter(|pattern| !pattern_has_backreference(pattern))
                                    .map(Arc::from);
                                out.push(Candidate {
                                    order: *order,
                                    base_grammar_id,
                                    pattern: begin.to_owned(),
                                    pattern_id: Some((grammar_id, begin_pattern_id)),
                                    scope_prefix: scope_prefix.clone(),
                                    kind: CandidateKind::BeginWhile {
                                        grammar_id,
                                        rule_id: rule.id,
                                        while_pattern: *while_pattern,
                                        begin_captures: contextualize_capture_spec(
                                            begin_captures,
                                            repository_context,
                                        ),
                                        while_captures: contextualize_capture_spec(
                                            while_captures,
                                            repository_context,
                                        ),
                                        name: scope_name(grammar, *name).map(Arc::from),
                                        content_name: scope_name(grammar, *content_name)
                                            .map(Arc::from),
                                        patterns: patterns.into(),
                                        while_static,
                                    },
                                });
                                *order += 1;
                            }
                        }
                        RuleBody::IncludeOnly { patterns } => {
                            let patterns = contextualize_refs(patterns, repository_context);
                            self.flatten_refs(
                                grammar_id,
                                base_grammar_id,
                                &patterns,
                                scope_prefix.clone(),
                                out,
                                order,
                                depth + 1,
                            )
                        }
                    }
                }
                RuleRef::Repository(name) => {
                    if let Some(rule_ref) = grammar.repository.get(name) {
                        self.flatten_refs(
                            grammar_id,
                            base_grammar_id,
                            std::slice::from_ref(rule_ref),
                            scope_prefix.clone(),
                            out,
                            order,
                            depth + 1,
                        );
                    }
                }
                RuleRef::SelfRef => {
                    self.flatten_refs(
                        grammar_id,
                        base_grammar_id,
                        &grammar.top_level,
                        scope_prefix.clone(),
                        out,
                        order,
                        depth + 1,
                    );
                }
                RuleRef::BaseRef => {
                    let Some(base) = self.grammars.grammar(base_grammar_id) else {
                        continue;
                    };
                    self.flatten_refs(
                        base_grammar_id,
                        base_grammar_id,
                        &base.top_level,
                        scope_prefix.clone(),
                        out,
                        order,
                        depth + 1,
                    );
                }
                RuleRef::External { scope, repository } => {
                    let Some(scope_text) = grammar.scope(*scope) else {
                        continue;
                    };
                    let Some(external_id) = self.grammars.grammar_id_by_scope(scope_text) else {
                        continue;
                    };
                    let Some(external) = self.grammars.grammar(external_id) else {
                        continue;
                    };
                    if let Some(repository) = repository {
                        if let Some(rule_ref) = external.repository.get(repository) {
                            self.flatten_refs(
                                external_id,
                                base_grammar_id,
                                std::slice::from_ref(rule_ref),
                                None,
                                out,
                                order,
                                depth + 1,
                            );
                        }
                    } else {
                        self.flatten_refs(
                            external_id,
                            base_grammar_id,
                            &external.top_level,
                            None,
                            out,
                            order,
                            depth + 1,
                        );
                    }
                }
            }
        }
    }

    fn only_unavailable_includes(
        &self,
        grammar_id: GrammarId,
        base_grammar_id: GrammarId,
        refs: &[RuleRef],
    ) -> bool {
        !refs.is_empty()
            && !self.refs_have_available_rule(
                grammar_id,
                base_grammar_id,
                refs,
                &mut HashSet::new(),
                0,
            )
    }

    fn refs_have_available_rule(
        &self,
        grammar_id: GrammarId,
        base_grammar_id: GrammarId,
        refs: &[RuleRef],
        visiting: &mut HashSet<IncludeAvailabilityNode>,
        depth: usize,
    ) -> bool {
        if depth >= MAX_INCLUDE_DEPTH {
            return false;
        }
        let Some(grammar) = self.grammars.grammar(grammar_id) else {
            return false;
        };
        refs.iter().any(|rule_ref| match rule_ref {
            RuleRef::Rule(rule_id) => {
                let key = IncludeAvailabilityNode::Rule(grammar_id, base_grammar_id, *rule_id);
                let cached = self.include_availability_cache.borrow().get(&key).copied();
                if let Some(available) = cached {
                    available
                } else if !visiting.insert(key.clone()) {
                    true
                } else {
                    let available = grammar.rule(*rule_id).is_some_and(|rule| match &rule.body {
                        RuleBody::Match { .. } => true,
                        RuleBody::BeginEnd { patterns, .. }
                        | RuleBody::BeginWhile { patterns, .. }
                        | RuleBody::IncludeOnly { patterns } => {
                            let repository_context = self
                                .rule_repository_contexts
                                .get(grammar_id, *rule_id)
                                .map(Arc::as_ref);
                            let patterns = contextualize_refs(patterns, repository_context);
                            // vscode-textmate drops a compiled container only when
                            // it had raw children but every child was omitted from
                            // the compiled pattern list. A genuinely empty
                            // container is retained.
                            patterns.is_empty()
                                || self.refs_have_available_rule(
                                    grammar_id,
                                    base_grammar_id,
                                    &patterns,
                                    visiting,
                                    depth + 1,
                                )
                        }
                    });
                    visiting.remove(&key);
                    self.include_availability_cache
                        .borrow_mut()
                        .insert(key, available);
                    available
                }
            }
            RuleRef::Repository(name) => self.repository_has_available_rule(
                grammar_id,
                base_grammar_id,
                name,
                visiting,
                depth + 1,
            ),
            RuleRef::SelfRef => {
                self.top_level_has_available_rule(grammar_id, base_grammar_id, visiting, depth + 1)
            }
            RuleRef::BaseRef => self.top_level_has_available_rule(
                base_grammar_id,
                base_grammar_id,
                visiting,
                depth + 1,
            ),
            RuleRef::External { scope, repository } => grammar
                .scope(*scope)
                .and_then(|scope| self.grammars.grammar_id_by_scope(scope))
                .and_then(|external_id| self.grammars.grammar(external_id).map(|_| external_id))
                .is_some_and(|external_id| match repository {
                    Some(repository) => self.repository_has_available_rule(
                        external_id,
                        base_grammar_id,
                        repository,
                        visiting,
                        depth + 1,
                    ),
                    None => self.top_level_has_available_rule(
                        external_id,
                        base_grammar_id,
                        visiting,
                        depth + 1,
                    ),
                }),
        })
    }

    fn repository_has_available_rule(
        &self,
        grammar_id: GrammarId,
        base_grammar_id: GrammarId,
        repository: &str,
        visiting: &mut HashSet<IncludeAvailabilityNode>,
        depth: usize,
    ) -> bool {
        let repository_id = self
            .include_repository_names
            .borrow_mut()
            .intern(repository)
            .0;
        let key = IncludeAvailabilityNode::Repository(grammar_id, base_grammar_id, repository_id);
        if let Some(available) = self.include_availability_cache.borrow().get(&key) {
            return *available;
        }
        if !visiting.insert(key.clone()) {
            return true;
        }
        let available = self
            .grammars
            .grammar(grammar_id)
            .and_then(|grammar| grammar.repository.get(repository))
            .is_some_and(|rule_ref| {
                self.refs_have_available_rule(
                    grammar_id,
                    base_grammar_id,
                    std::slice::from_ref(rule_ref),
                    visiting,
                    depth,
                )
            });
        visiting.remove(&key);
        self.include_availability_cache
            .borrow_mut()
            .insert(key, available);
        available
    }

    fn top_level_has_available_rule(
        &self,
        grammar_id: GrammarId,
        base_grammar_id: GrammarId,
        visiting: &mut HashSet<IncludeAvailabilityNode>,
        depth: usize,
    ) -> bool {
        let key = IncludeAvailabilityNode::TopLevel(grammar_id, base_grammar_id);
        if let Some(available) = self.include_availability_cache.borrow().get(&key) {
            return *available;
        }
        if !visiting.insert(key.clone()) {
            return true;
        }
        let available = self.grammars.grammar(grammar_id).is_some_and(|grammar| {
            grammar.top_level.is_empty()
                || self.refs_have_available_rule(
                    grammar_id,
                    base_grammar_id,
                    &grammar.top_level,
                    visiting,
                    depth,
                )
        });
        visiting.remove(&key);
        self.include_availability_cache
            .borrow_mut()
            .insert(key, available);
        available
    }

    fn injection_outcome(
        &mut self,
        stack: &[Arc<str>],
    ) -> (InjectionOutcomeId, Arc<InjectionOutcome>) {
        let mut left = Vec::new();
        let mut right = Vec::new();
        let mut seen = HashSet::new();
        for injection in self.injection_selectors.iter() {
            if selector_tokens_match(&injection.selector_tokens, stack) {
                if !seen.insert((
                    injection.priority,
                    injection.grammar_id,
                    injection.patterns.clone(),
                )) {
                    continue;
                }
                let candidate = InjectionCandidate {
                    grammar_id: injection.grammar_id,
                    patterns: injection.patterns.clone(),
                };
                if injection.priority == InjectionPriority::Left {
                    left.push(candidate);
                } else {
                    right.push(candidate);
                }
            }
        }
        let outcome = InjectionOutcome { left, right };
        if self.injection_outcomes.len() >= MAX_INJECTION_OUTCOMES
            && !self.injection_outcomes.contains(&outcome)
        {
            // Blueprint keys contain outcome IDs. Drop them together so an
            // evicted outcome never leaves an ID whose meaning must be
            // reconstructed approximately.
            self.injection_outcomes.clear();
            self.candidate_blueprint_cache.clear();
            self.injection_outcome_cache.clear();
            self.prepared_injection_outcome_ids.clear();
        }
        self.injection_outcomes.intern(outcome)
    }

    fn prepared_blueprint_key(
        &mut self,
        source: CandidateSourceKey,
        injection_outcome_id: InjectionOutcomeId,
        injection_outcome: &InjectionOutcome,
    ) -> Option<(Arc<PreparedBlueprintCache>, PreparedBlueprintKey)> {
        if !source.is_static() {
            return None;
        }
        let cache = self.prepared_blueprint_cache.as_ref().map(Arc::clone)?;
        let prepared_outcome_id = if let Some(prepared) = self
            .prepared_injection_outcome_ids
            .get(&injection_outcome_id)
        {
            *prepared
        } else {
            let prepared = cache.intern_injection_outcome(injection_outcome);
            self.prepared_injection_outcome_ids
                .insert(injection_outcome_id, prepared);
            prepared
        }?;
        Some((
            cache,
            PreparedBlueprintKey {
                source,
                injection_outcome: prepared_outcome_id,
            },
        ))
    }

    fn cached_candidates_for_state(&mut self, state: &TokenizerState) -> Arc<CandidateSet> {
        let state_id = self.intern_state(state);
        if let Some(candidates) = self.candidate_cache.get(&state_id).cloned() {
            self.record_candidate_cache_hit();
            return candidates;
        }
        self.record_candidate_cache_miss();
        let stacks = self.current_scope_stack_ids(state, None);
        let active_stack_id = stacks.active_stack_id;
        let end_stack_id = stacks.end_stack_id;
        // Injection selectors are pure functions of the resolved scope stack,
        // so one interned stack id never needs its selectors re-evaluated.
        let (injection_outcome_id, injection_outcome) =
            if let Some(cached) = self.injection_outcome_cache.get(&active_stack_id) {
                cached.clone()
            } else {
                let stack = self.resolve_scope_stack_cached(active_stack_id);
                let outcome = self.injection_outcome(stack.as_ref());
                if self.injection_outcome_cache.len() >= MAX_SCOPE_STACK_CACHE_ENTRIES {
                    self.injection_outcome_cache.clear();
                }
                self.injection_outcome_cache
                    .insert(active_stack_id, outcome.clone());
                outcome
            };
        let source = CandidateSourceKey::for_state(self.root, state);
        let blueprint_key = CandidateBlueprintKey {
            source: source.clone(),
            injection_outcome: injection_outcome_id,
        };
        let blueprint =
            if let Some(blueprint) = self.candidate_blueprint_cache.get(&blueprint_key).cloned() {
                blueprint
            } else {
                let prepared = self.prepared_blueprint_key(
                    source,
                    injection_outcome_id,
                    injection_outcome.as_ref(),
                );
                let blueprint = match prepared {
                    Some((cache, key)) => {
                        let shared = cache.get_or_insert_with(key, || {
                            let candidates = self.candidates_for_state(state, &injection_outcome);
                            self.build_shareable_candidate_blueprint(candidates)
                        });
                        self.bind_shared_candidate_blueprint(shared)
                    }
                    None => {
                        let candidates = self.candidates_for_state(state, &injection_outcome);
                        let owned = self.build_candidate_blueprint(candidates);
                        self.bind_owned_candidate_blueprint(owned)
                    }
                };
                if self.candidate_blueprint_cache.len() >= MAX_CANDIDATE_BLUEPRINTS {
                    self.candidate_blueprint_cache.clear();
                }
                self.candidate_blueprint_cache
                    .insert(blueprint_key, blueprint.clone());
                blueprint
            };
        let candidate_set = Arc::new(CandidateSet {
            blueprint,
            active_stack_id,
            end_stack_id,
        });
        if self.candidate_cache.len() >= MAX_CANDIDATE_SETS {
            self.candidate_cache.clear();
        }
        self.candidate_cache.insert(state_id, candidate_set.clone());
        candidate_set
    }

    fn build_candidate_set(
        &mut self,
        prepared: Option<(Arc<PreparedBlueprintCache>, PreparedBlueprintKey)>,
        active_stack_id: ScopeStackId,
        end_stack_id: ScopeStackId,
        candidates: impl FnOnce(&mut Self) -> Vec<Candidate>,
    ) -> CandidateSet {
        let blueprint = if let Some((cache, key)) = prepared {
            let shared = cache.get_or_insert_with(key, || {
                let candidates = candidates(self);
                self.build_shareable_candidate_blueprint(candidates)
            });
            self.bind_shared_candidate_blueprint(shared)
        } else {
            let candidates = candidates(self);
            let blueprint = self.build_candidate_blueprint(candidates);
            self.bind_owned_candidate_blueprint(blueprint)
        };
        CandidateSet {
            blueprint,
            active_stack_id,
            end_stack_id,
        }
    }

    fn bind_owned_candidate_blueprint(
        &mut self,
        mut blueprint: CandidateBlueprint,
    ) -> BoundCandidateBlueprint {
        for candidate in &mut blueprint.candidates {
            if let CandidateKind::Match {
                name,
                name_template,
                ..
            } = &mut candidate.kind
                && let Some(name) = name.as_deref().filter(|name| !name.contains('$'))
            {
                *name_template = Some(
                    self.scope_templates
                        .intern_scope_template(name, &mut self.scope_names),
                );
            }
        }
        BoundCandidateBlueprint::Owned(Arc::new(blueprint))
    }

    fn bind_shared_candidate_blueprint(
        &mut self,
        blueprint: Arc<CandidateBlueprint>,
    ) -> BoundCandidateBlueprint {
        let match_name_templates = blueprint
            .candidates
            .iter()
            .map(|candidate| match &candidate.kind {
                CandidateKind::Match { name, .. } => name
                    .as_deref()
                    .filter(|name| !name.contains('$'))
                    .map(|name| {
                        self.scope_templates
                            .intern_scope_template(name, &mut self.scope_names)
                    }),
                CandidateKind::BeginEnd { .. }
                | CandidateKind::BeginWhile { .. }
                | CandidateKind::End { .. } => None,
            })
            .collect::<Vec<_>>()
            .into();
        BoundCandidateBlueprint::Shared {
            blueprint,
            match_name_templates,
        }
    }

    fn build_shareable_candidate_blueprint(
        &mut self,
        candidates: Vec<Candidate>,
    ) -> (Arc<CandidateBlueprint>, bool) {
        let generation = self.unprepared_static_matcher_generation;
        let blueprint = Arc::new(self.build_candidate_blueprint(candidates));
        (
            blueprint,
            generation == self.unprepared_static_matcher_generation,
        )
    }

    fn build_candidate_blueprint(&mut self, candidates: Vec<Candidate>) -> CandidateBlueprint {
        let mut matchers = Vec::with_capacity(candidates.len());
        for candidate in &candidates {
            let live_captures = self.live_captures_for_candidate(candidate);
            let matcher = if let Some((grammar_id, pattern_id)) = candidate.pattern_id {
                self.cached_matcher_with_live_captures(
                    grammar_id,
                    pattern_id,
                    &candidate.pattern,
                    live_captures,
                )
            } else {
                if self.prepared_pattern_cache.is_some() {
                    self.unprepared_static_matcher_generation =
                        self.unprepared_static_matcher_generation.wrapping_add(1);
                }
                self.cached_dynamic_matcher_with_live_captures(&candidate.pattern, live_captures)
            };
            matchers.push(matcher);
        }
        let matchers: Arc<[Arc<CompiledPattern>]> = matchers.into();
        let pattern_set_search = (matchers.len() > 1).then(|| {
            if let Some(counters) = self.counters_mut() {
                counters.record_pattern_set_construction();
            }
            PatternSetMatcher::from_shared_compiled(Arc::clone(&matchers))
        });
        CandidateBlueprint {
            candidates,
            matchers,
            pattern_set_search,
        }
    }

    fn prepared_static_matcher(
        &mut self,
        grammar_id: GrammarId,
        pattern_id: PatternId,
        pattern: &str,
        live_captures: Option<Vec<u32>>,
    ) -> Option<Arc<CompiledPattern>> {
        let cache = Arc::clone(self.prepared_pattern_cache.as_ref()?);
        let (matcher, compiled_now, retained) =
            cache.get_or_compile(grammar_id, pattern_id, pattern, live_captures);
        debug_assert_eq!(matcher.source(), pattern);
        if !retained {
            self.unprepared_static_matcher_generation =
                self.unprepared_static_matcher_generation.wrapping_add(1);
            self.matcher_cache
                .insert((grammar_id, pattern_id), Arc::clone(&matcher));
        }
        if compiled_now && let Some(counters) = self.counters_mut() {
            counters.record_regex_compile(Some(grammar_id.0), Some(pattern_id.0), pattern);
        }
        Some(matcher)
    }

    fn cached_matcher(
        &mut self,
        grammar_id: GrammarId,
        pattern_id: PatternId,
        pattern: &str,
    ) -> Arc<CompiledPattern> {
        let key = (grammar_id, pattern_id);
        if let Some(matcher) = self.matcher_cache.get(&key).cloned() {
            if self.prepared_pattern_cache.is_some() {
                self.unprepared_static_matcher_generation =
                    self.unprepared_static_matcher_generation.wrapping_add(1);
            }
            return matcher;
        }
        if let Some(matcher) = self.prepared_static_matcher(grammar_id, pattern_id, pattern, None) {
            return matcher;
        }
        let matcher = Arc::new(CompiledPattern::new(pattern));
        self.matcher_cache.insert(key, matcher.clone());
        if let Some(counters) = self.counters_mut() {
            counters.record_regex_compile(Some(grammar_id.0), Some(pattern_id.0), pattern);
        }
        matcher
    }

    fn cached_matcher_with_live_captures(
        &mut self,
        grammar_id: GrammarId,
        pattern_id: PatternId,
        pattern: &str,
        live_captures: Vec<u32>,
    ) -> Arc<CompiledPattern> {
        let key = (grammar_id, pattern_id);
        if let Some(matcher) = self.matcher_cache.get(&key).cloned() {
            if self.prepared_pattern_cache.is_some() {
                self.unprepared_static_matcher_generation =
                    self.unprepared_static_matcher_generation.wrapping_add(1);
            }
            return matcher;
        }
        if self.prepared_pattern_cache.is_some()
            && let Some(matcher) = self.prepared_static_matcher(
                grammar_id,
                pattern_id,
                pattern,
                Some(live_captures.clone()),
            )
        {
            return matcher;
        }
        let matcher = Arc::new(CompiledPattern::new_with_live_captures(
            pattern,
            live_captures,
        ));
        self.matcher_cache.insert(key, matcher.clone());
        if let Some(counters) = self.counters_mut() {
            counters.record_regex_compile(Some(grammar_id.0), Some(pattern_id.0), pattern);
        }
        matcher
    }

    fn cached_dynamic_matcher(&mut self, pattern: &str) -> Arc<CompiledPattern> {
        let key = DynamicMatcherKey {
            pattern: pattern.to_owned(),
            live_captures: vec![u32::MAX],
        };
        if let Some(matcher) = self.dynamic_matcher_cache.get(&key) {
            return matcher.clone();
        }
        // Dynamic begin/end substitutions are source-derived and potentially
        // unbounded. Keep them separate from immutable grammar patterns and
        // put a hard ceiling on retained entries.
        if self.dynamic_matcher_cache.len() >= MAX_DYNAMIC_MATCHERS {
            self.dynamic_matcher_cache.clear();
        }
        let matcher = Arc::new(CompiledPattern::new(pattern));
        self.dynamic_matcher_cache.insert(key, matcher.clone());
        if let Some(counters) = self.counters_mut() {
            counters.record_regex_compile(None, None, pattern);
        }
        matcher
    }

    fn cached_dynamic_matcher_with_live_captures(
        &mut self,
        pattern: &str,
        live_captures: Vec<u32>,
    ) -> Arc<CompiledPattern> {
        let key = DynamicMatcherKey {
            pattern: pattern.to_owned(),
            live_captures: live_captures.clone(),
        };
        if let Some(matcher) = self.dynamic_matcher_cache.get(&key) {
            return matcher.clone();
        }
        if self.dynamic_matcher_cache.len() >= MAX_DYNAMIC_MATCHERS {
            self.dynamic_matcher_cache.clear();
        }
        let matcher = Arc::new(CompiledPattern::new_with_live_captures(
            pattern,
            live_captures,
        ));
        self.dynamic_matcher_cache.insert(key, matcher.clone());
        if let Some(counters) = self.counters_mut() {
            counters.record_regex_compile(None, None, pattern);
        }
        matcher
    }

    fn live_captures_for_candidate(&self, candidate: &Candidate) -> Vec<u32> {
        let mut live = Vec::new();
        match &candidate.kind {
            CandidateKind::Match {
                grammar_id,
                name,
                captures,
                ..
            } => {
                add_scope_capture_refs(name.as_deref(), &mut live);
                self.add_capture_spec_refs(*grammar_id, captures, &mut live);
            }
            CandidateKind::BeginEnd {
                grammar_id,
                end,
                begin_captures,
                name,
                content_name,
                ..
            } => {
                add_scope_capture_refs(name.as_deref(), &mut live);
                add_scope_capture_refs(content_name.as_deref(), &mut live);
                self.add_capture_spec_refs(*grammar_id, begin_captures, &mut live);
                if let Some(pattern) = self
                    .grammars
                    .grammar(*grammar_id)
                    .and_then(|grammar| grammar.pattern(*end))
                {
                    add_end_pattern_capture_refs(pattern, &mut live);
                }
            }
            CandidateKind::BeginWhile {
                grammar_id,
                while_pattern,
                begin_captures,
                name,
                content_name,
                ..
            } => {
                add_scope_capture_refs(name.as_deref(), &mut live);
                add_scope_capture_refs(content_name.as_deref(), &mut live);
                self.add_capture_spec_refs(*grammar_id, begin_captures, &mut live);
                if let Some(pattern) = self
                    .grammars
                    .grammar(*grammar_id)
                    .and_then(|grammar| grammar.pattern(*while_pattern))
                {
                    add_end_pattern_capture_refs(pattern, &mut live);
                }
            }
            CandidateKind::End {
                grammar_id,
                captures,
            } => self.add_capture_spec_refs(*grammar_id, captures, &mut live),
        }
        live.sort_unstable();
        live.dedup();
        live
    }

    fn add_capture_spec_refs(
        &self,
        grammar_id: GrammarId,
        captures: &CaptureSpec,
        live: &mut Vec<u32>,
    ) {
        let grammar = self.grammars.grammar(grammar_id);
        for (group, entry) in &captures.entries {
            if entry.name.is_some() || !entry.patterns.is_empty() {
                live.push(*group);
            }
            if let Some(name) = entry
                .name
                .and_then(|name| grammar.and_then(|grammar| grammar.scope(name)))
            {
                add_scope_capture_refs(Some(name), live);
            }
        }
    }

    fn take_capture_result_buffer(&mut self) -> Vec<Option<Range<usize>>> {
        self.capture_result_pool.pop().unwrap_or_default()
    }

    fn recycle_capture_result_buffer(&mut self, mut captures: Vec<Option<Range<usize>>>) {
        if captures.capacity() == 0
            || captures.capacity() > MAX_POOLED_CAPTURE_CAPACITY
            || self.capture_result_pool.len() >= MAX_CAPTURE_RESULT_POOL_ENTRIES
        {
            return;
        }
        captures.clear();
        self.capture_result_pool.push(captures);
    }

    fn find_best_candidate(
        &mut self,
        candidate_set: &CandidateSet,
        line: &str,
        from: usize,
        is_first_line: bool,
        anchor_pos: Option<usize>,
        suppressed_begin_rules: Option<&HashSet<(GrammarId, RuleId)>>,
    ) -> CandidateSearchResult {
        if let Some(counters) = self.counters_mut() {
            counters.record_candidate_search();
        }
        let mut best: Option<(usize, MatchResult)> = None;
        let mut fallback_budget_killed = false;
        let mut fallback_steps = 0u64;

        let suppression_active = suppressed_begin_rules.is_some_and(|rules| !rules.is_empty());
        let unified_search_active = !suppression_active && !self.counters_enabled;
        let ctx = scan_anchor_context(from, is_first_line, anchor_pos);
        if unified_search_active && let Some(pattern_set) = &candidate_set.pattern_set_search {
            let (set_match, set_budget_killed) =
                pattern_set.find_with_context_and_scratch(line, from, ctx, &mut self.regex_scratch);
            fallback_budget_killed |= set_budget_killed;
            if let Some((pattern_index, set_result)) = set_match
                && pattern_index < candidate_set.candidates.len()
                && set_result.start >= from
                && set_result.end <= line.len()
            {
                best = Some((pattern_index, set_result));
            }
        } else {
            for (index, candidate) in candidate_set.candidates.iter().enumerate() {
                if suppressed_begin_rules.is_some_and(|rules| {
                    !rules.is_empty() && candidate_is_suppressed(candidate, rules)
                }) {
                    continue;
                }
                if let Some((best_index, best_result)) = &best
                    && best_result.start == from
                    && candidate.order > candidate_set.candidates[*best_index].order
                {
                    break;
                }
                if let Some(counters) = self.counters_mut() {
                    counters.record_candidate_pattern_considered();
                }
                let pattern = self.find_cached_pattern_selection_report(
                    &candidate.pattern,
                    candidate.pattern_id,
                    candidate_set.matchers[index].matcher(),
                    line,
                    from,
                    ctx,
                );
                fallback_budget_killed |= pattern.fallback_budget_killed;
                fallback_steps = fallback_steps.saturating_add(pattern.fallback_steps);
                let Some(result) = pattern.result else {
                    continue;
                };
                if result.start < from || result.end > line.len() {
                    continue;
                }
                let replace = match &best {
                    None => true,
                    Some((best_index, best_result)) => {
                        result.start < best_result.start
                            || (result.start == best_result.start
                                && candidate.order < candidate_set.candidates[*best_index].order)
                    }
                };
                if replace {
                    best = Some((index, result));
                }
            }
        }
        if let Some((index, selection_result)) = &best
            && selection_result.captures.is_empty()
            && candidate_set.matchers[*index].needs_capture_replay_after_selection()
        {
            if let Some(counters) = self.counters_mut() {
                counters.record_capture_replay();
            }
            let ctx = scan_anchor_context(from, is_first_line, anchor_pos);
            let compiled = &candidate_set.matchers[*index];
            let mode = super::regex::backtrack::capture_engine_mode();
            let mut capture_buffer = self.take_capture_result_buffer();
            let capture_candidate = compiled.find_live_captures_at_into(
                line,
                selection_result.start,
                ctx,
                &mut self.regex_scratch,
                &mut capture_buffer,
            );
            self.recycle_capture_result_buffer(capture_buffer);
            let recursive = || {
                compiled
                    .matcher()
                    .find_report_at(line, selection_result.start, ctx)
                    .map(|(result, steps)| (result, steps.unwrap_or(0)))
            };
            let report = match (mode, capture_candidate) {
                (super::regex::backtrack::PositionEngineMode::Candidate, Some(candidate)) => {
                    candidate
                }
                (super::regex::backtrack::PositionEngineMode::Shadow, Some(candidate)) => {
                    let recursive = recursive();
                    let agrees = match (&candidate, &recursive) {
                        (Ok((candidate, _)), Ok((recursive, _))) => candidate == recursive,
                        (
                            Err(FallbackError::BudgetExceeded { .. }),
                            Err(FallbackError::BudgetExceeded { .. }),
                        )
                        | (
                            Err(FallbackError::InvalidStart { .. }),
                            Err(FallbackError::InvalidStart { .. }),
                        ) => true,
                        _ => false,
                    };
                    if !agrees {
                        eprintln!(
                            "SYNTAXMATE_CAPTURE_VM_MISMATCH pattern={:?} start={} candidate={candidate:?} recursive={recursive:?}",
                            candidate_set.candidates[*index].pattern, selection_result.start,
                        );
                    }
                    recursive
                }
                _ => recursive(),
            };
            match report {
                Ok((Some(result), steps)) => {
                    let steps = steps as u64;
                    fallback_steps = fallback_steps.saturating_add(steps);
                    best = Some((*index, result));
                }
                Ok((None, steps)) => {
                    fallback_steps = fallback_steps.saturating_add(steps as u64);
                    best = None;
                }
                Err(FallbackError::BudgetExceeded { steps }) => {
                    fallback_steps = fallback_steps.saturating_add(steps as u64);
                    fallback_budget_killed = true;
                    best = None;
                }
                Err(FallbackError::InvalidStart { .. }) => best = None,
            }
        }
        if best.is_some()
            && let Some(counters) = self.counters_mut()
        {
            counters.record_candidate_winner();
        }
        CandidateSearchResult {
            best,
            fallback_budget_killed,
            fallback_steps,
        }
    }

    fn find_pattern(
        &mut self,
        pattern: &str,
        pattern_id: Option<(GrammarId, PatternId)>,
        line: &str,
        from: usize,
        ctx: AnchorContext,
    ) -> Option<MatchResult> {
        self.find_pattern_report(pattern, pattern_id, line, from, ctx)
            .result
    }

    fn find_pattern_report(
        &mut self,
        pattern: &str,
        pattern_id: Option<(GrammarId, PatternId)>,
        line: &str,
        from: usize,
        ctx: AnchorContext,
    ) -> PatternSearchResult {
        let matcher = match pattern_id {
            Some((grammar_id, pattern_id)) => self.cached_matcher(grammar_id, pattern_id, pattern),
            None => self.cached_dynamic_matcher(pattern),
        };
        self.find_cached_pattern_report(pattern, pattern_id, matcher.matcher(), line, from, ctx)
    }

    fn find_cached_pattern_report(
        &mut self,
        pattern: &str,
        pattern_id: Option<(GrammarId, PatternId)>,
        matcher: &RegexMatcher,
        line: &str,
        from: usize,
        ctx: AnchorContext,
    ) -> PatternSearchResult {
        self.find_cached_pattern_report_impl(pattern, pattern_id, matcher, line, from, ctx, false)
    }

    fn find_cached_pattern_selection_report(
        &mut self,
        pattern: &str,
        pattern_id: Option<(GrammarId, PatternId)>,
        matcher: &RegexMatcher,
        line: &str,
        from: usize,
        ctx: AnchorContext,
    ) -> PatternSearchResult {
        self.find_cached_pattern_report_impl(pattern, pattern_id, matcher, line, from, ctx, true)
    }

    #[allow(clippy::too_many_arguments)]
    fn find_cached_pattern_report_impl(
        &mut self,
        pattern: &str,
        pattern_id: Option<(GrammarId, PatternId)>,
        matcher: &RegexMatcher,
        line: &str,
        from: usize,
        ctx: AnchorContext,
        selection_only: bool,
    ) -> PatternSearchResult {
        let counters_enabled = self.counters_enabled;
        let hot_counters_enabled = self.hot_counters_enabled;
        let start = hot_counters_enabled.then(Instant::now);
        let engine = matcher.engine_name();
        let prefilter_may_match = counters_enabled
            .then(|| matcher.prefilter_may_match(line, from))
            .flatten();
        trace_regex_search(pattern, line, from, ctx, engine);
        let report = if selection_only {
            matcher.find_report_for_selection(line, from, ctx)
        } else {
            matcher.find_report(line, from, ctx)
        };
        let elapsed_micros = start
            .map(|start| start.elapsed().as_micros() as u64)
            .unwrap_or(0);
        if let Some(counters) = self.counters_mut() {
            match engine {
                "dfa" => counters.record_dfa_attempt(),
                "fallback" => counters.record_fallback_attempt(),
                _ => {}
            }
        }
        if let Some(may_match) = prefilter_may_match {
            self.record_prefilter_check(may_match);
        }
        match report {
            Ok((result, steps)) => {
                let matched = result.is_some();
                let fallback_steps = steps.unwrap_or(0) as u64;
                if let Some(steps) = steps
                    && let Some(counters) = self.counters_mut()
                {
                    counters.record_fallback_steps(steps);
                }
                self.record_pattern_hotspot(
                    pattern,
                    pattern_id,
                    engine,
                    elapsed_micros,
                    matched,
                    fallback_steps,
                    false,
                    prefilter_may_match,
                );
                PatternSearchResult {
                    result,
                    fallback_budget_killed: false,
                    fallback_steps,
                }
            }
            Err(FallbackError::BudgetExceeded { steps }) => {
                if let Some(counters) = self.counters_mut() {
                    counters.record_fallback_steps(steps);
                    counters.record_fallback_budget_kill();
                }
                self.record_pattern_hotspot(
                    pattern,
                    pattern_id,
                    engine,
                    elapsed_micros,
                    false,
                    steps as u64,
                    true,
                    prefilter_may_match,
                );
                PatternSearchResult {
                    result: None,
                    fallback_budget_killed: true,
                    fallback_steps: steps as u64,
                }
            }
            Err(FallbackError::InvalidStart { .. }) => {
                self.record_pattern_hotspot(
                    pattern,
                    pattern_id,
                    engine,
                    elapsed_micros,
                    false,
                    0,
                    false,
                    prefilter_may_match,
                );
                PatternSearchResult {
                    result: None,
                    fallback_budget_killed: false,
                    fallback_steps: 0,
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_candidate(
        &mut self,
        line: &str,
        state: &mut TokenizerState,
        tokens: &mut Vec<CompactScopedToken>,
        candidate: &Candidate,
        match_name_template: Option<ScopeTemplateId>,
        result: MatchResult,
        anchor_pos: &mut Option<usize>,
        frame_anchor_positions: &mut Vec<Option<usize>>,
        line_entry_depth: usize,
        active_stack: ScopeStackId,
        end_stack: ScopeStackId,
    ) -> usize {
        let mut result_value = result;
        let result = &result_value;
        let consumed_end = match &candidate.kind {
            CandidateKind::Match {
                grammar_id,
                name,
                captures,
                ..
            } => {
                let consumed_end = specified_outside_capture_end(result, captures);
                let mut stack = active_stack;
                if let Some(prefix) = &candidate.scope_prefix {
                    stack = self.push_scope_prefix_once_id(stack, prefix);
                }
                self.emit_match(
                    tokens,
                    line,
                    result,
                    *grammar_id,
                    stack,
                    name.as_deref(),
                    match_name_template,
                    captures,
                );
                consumed_end
            }
            CandidateKind::BeginEnd {
                grammar_id,
                rule_id,
                end,
                begin_captures,
                end_captures,
                name,
                content_name,
                patterns,
                apply_end_pattern_last,
                end_static,
            } => {
                let consumed_end = specified_outside_capture_end(result, begin_captures);
                let names_static = !name.as_deref().is_some_and(|name| name.contains('$'))
                    && !content_name
                        .as_deref()
                        .is_some_and(|name| name.contains('$'));
                let name = frame_scope_text(name, line, result);
                let content_name = frame_scope_text(content_name, line, result);
                let mut stack = active_stack;
                if let Some(prefix) = candidate.scope_prefix.clone() {
                    stack = self.push_scope_prefix_once_id(stack, &prefix);
                }
                self.emit_match(
                    tokens,
                    line,
                    result,
                    *grammar_id,
                    stack,
                    name.as_deref(),
                    None,
                    begin_captures,
                );
                let (end_pattern, end_pattern_id, static_frame) =
                    if let Some(end_static) = end_static {
                        if is_non_matching_end_sentinel(end_static) {
                            (None, None, names_static)
                        } else {
                            (Some(Arc::clone(end_static)), Some(*end), names_static)
                        }
                    } else {
                        let end_pattern = self
                            .substituted_pattern(*grammar_id, *end, line, result)
                            .filter(|(pattern, _)| !is_non_matching_end_sentinel(pattern));
                        let end_pattern_id = end_pattern
                            .as_ref()
                            .and_then(|(_, is_static)| is_static.then_some(*end));
                        (
                            end_pattern.map(|(pattern, _)| Arc::<str>::from(pattern)),
                            end_pattern_id,
                            false,
                        )
                    };
                let begin_captured_eol = result.end == line.len() && line.ends_with('\n');
                let identity_key = (*grammar_id, *rule_id, begin_captured_eol);
                let cached = static_frame
                    .then(|| self.static_frame_identities.get(&identity_key).copied())
                    .flatten();
                let parent_id = state.frames.interned_id();
                let shared_node = cached.and_then(|cached| {
                    self.frame_node_cache
                        .get(&(parent_id, cached.frame_id))
                        .cloned()
                });
                if let Some(node) = shared_node {
                    state.push_frame_shared(node);
                } else {
                    let identity = state.push_frame_cached(
                        Frame {
                            grammar_id: *grammar_id,
                            base_grammar_id: candidate.base_grammar_id,
                            rule_id: *rule_id,
                            scope_prefix: candidate.scope_prefix.clone(),
                            name,
                            content_name,
                            end_pattern,
                            end_pattern_id,
                            while_pattern: None,
                            while_pattern_id: None,
                            end_captures: Arc::clone(end_captures),
                            while_captures: shared_empty_capture_spec(),
                            patterns: Arc::clone(patterns),
                            apply_end_pattern_last: *apply_end_pattern_last,
                            begin_captured_eol,
                            identity_hash: 0,
                            state_hash: 0,
                            interned_stack_id: InternedFrameStackId::default(),
                        },
                        cached,
                        &mut self.frame_stack_interner,
                        Some(&mut self.frame_edge_cache),
                    );
                    if static_frame && cached.is_none() {
                        self.static_frame_identities.insert(identity_key, identity);
                    }
                    self.remember_frame_node(parent_id, identity.frame_id, state);
                }
                frame_anchor_positions.push(*anchor_pos);
                *anchor_pos = Some(result.end);
                consumed_end
            }
            CandidateKind::BeginWhile {
                grammar_id,
                rule_id,
                while_pattern,
                begin_captures,
                while_captures,
                name,
                content_name,
                patterns,
                while_static,
            } => {
                let consumed_end = specified_outside_capture_end(result, begin_captures);
                let names_static = !name.as_deref().is_some_and(|name| name.contains('$'))
                    && !content_name
                        .as_deref()
                        .is_some_and(|name| name.contains('$'));
                let name = frame_scope_text(name, line, result);
                let content_name = frame_scope_text(content_name, line, result);
                let mut stack = active_stack;
                if let Some(prefix) = candidate.scope_prefix.clone() {
                    stack = self.push_scope_prefix_once_id(stack, &prefix);
                }
                if begin_captures.entries.is_empty()
                    && content_name.is_some()
                    && !patterns.is_empty()
                {
                    let mut content_stack = stack;
                    if let Some(name) = &name {
                        content_stack = self.push_scope_text_id(content_stack, name);
                    }
                    if let Some(content_name) = &content_name {
                        content_stack = self.push_scope_text_id(content_stack, content_name);
                    }
                    self.tokenize_inline_patterns(
                        tokens,
                        line,
                        result.start..result.end,
                        *grammar_id,
                        content_stack,
                        patterns,
                        false,
                    );
                } else {
                    self.emit_match(
                        tokens,
                        line,
                        result,
                        *grammar_id,
                        stack,
                        name.as_deref(),
                        None,
                        begin_captures,
                    );
                }
                let static_while_pattern_id = *while_pattern;
                let (while_pattern, while_pattern_id, static_frame) =
                    if let Some(while_static) = while_static {
                        (
                            Some(Arc::clone(while_static)),
                            Some(static_while_pattern_id),
                            names_static,
                        )
                    } else {
                        let while_pattern = self.substituted_pattern(
                            *grammar_id,
                            static_while_pattern_id,
                            line,
                            result,
                        );
                        let while_pattern_id = while_pattern.as_ref().and_then(|(_, is_static)| {
                            is_static.then_some(static_while_pattern_id)
                        });
                        (
                            while_pattern.map(|(pattern, _)| Arc::<str>::from(pattern)),
                            while_pattern_id,
                            false,
                        )
                    };
                let begin_captured_eol = result.end == line.len() && line.ends_with('\n');
                let identity_key = (*grammar_id, *rule_id, begin_captured_eol);
                let cached = static_frame
                    .then(|| self.static_frame_identities.get(&identity_key).copied())
                    .flatten();
                let parent_id = state.frames.interned_id();
                let shared_node = cached.and_then(|cached| {
                    self.frame_node_cache
                        .get(&(parent_id, cached.frame_id))
                        .cloned()
                });
                if let Some(node) = shared_node {
                    state.push_frame_shared(node);
                } else {
                    let identity = state.push_frame_cached(
                        Frame {
                            grammar_id: *grammar_id,
                            base_grammar_id: candidate.base_grammar_id,
                            rule_id: *rule_id,
                            scope_prefix: candidate.scope_prefix.clone(),
                            name,
                            content_name,
                            end_pattern: None,
                            end_pattern_id: None,
                            while_pattern,
                            while_pattern_id,
                            end_captures: shared_empty_capture_spec(),
                            while_captures: Arc::clone(while_captures),
                            patterns: Arc::clone(patterns),
                            apply_end_pattern_last: false,
                            begin_captured_eol,
                            identity_hash: 0,
                            state_hash: 0,
                            interned_stack_id: InternedFrameStackId::default(),
                        },
                        cached,
                        &mut self.frame_stack_interner,
                        Some(&mut self.frame_edge_cache),
                    );
                    if static_frame && cached.is_none() {
                        self.static_frame_identities.insert(identity_key, identity);
                    }
                    self.remember_frame_node(parent_id, identity.frame_id, state);
                }
                frame_anchor_positions.push(*anchor_pos);
                *anchor_pos = Some(result.end);
                consumed_end
            }
            CandidateKind::End {
                grammar_id,
                captures,
            } => {
                let consumed_end = specified_outside_capture_end(result, captures);
                self.emit_match(
                    tokens,
                    line,
                    result,
                    *grammar_id,
                    end_stack,
                    None,
                    None,
                    captures,
                );
                let depth_before_pop = state.depth();
                state.pop_frame();
                *anchor_pos = if depth_before_pop > line_entry_depth {
                    frame_anchor_positions.pop().flatten()
                } else {
                    state
                        .frames
                        .last()
                        .is_some_and(|frame| frame.begin_captured_eol)
                        .then_some(0)
                };
                consumed_end
            }
        };
        self.recycle_capture_result_buffer(std::mem::take(&mut result_value.captures));
        consumed_end
    }

    fn remember_frame_node(
        &mut self,
        parent_id: InternedFrameStackId,
        frame_id: InternedFrameId,
        state: &TokenizerState,
    ) {
        let Some(node) = state.frames.tail_node() else {
            return;
        };
        if self.frame_node_cache.len() >= MAX_FRAME_NODE_CACHE_ENTRIES {
            self.frame_node_cache.clear();
        }
        self.frame_node_cache
            .insert((parent_id, frame_id), Arc::clone(node));
    }

    fn substituted_pattern(
        &self,
        grammar_id: GrammarId,
        pattern_id: PatternId,
        line: &str,
        result: &MatchResult,
    ) -> Option<(String, bool)> {
        let grammar = self.grammars.grammar(grammar_id)?;
        let pattern = grammar.pattern(pattern_id)?;
        let capture_texts = (0..result.capture_count())
            .map(|group| result.capture(group).and_then(|range| line.get(range)))
            .collect::<Vec<_>>();
        let substituted =
            substitute_end_pattern(pattern, &capture_texts, MAX_SUBSTITUTED_END_PATTERN_LEN)
                .unwrap_or_else(|_| pattern.to_owned());
        let is_static = substituted == pattern;
        Some((substituted, is_static))
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_match(
        &mut self,
        tokens: &mut Vec<CompactScopedToken>,
        line: &str,
        result: &MatchResult,
        grammar_id: GrammarId,
        mut base_stack: ScopeStackId,
        name: Option<&str>,
        name_template: Option<ScopeTemplateId>,
        captures: &CaptureSpec,
    ) {
        if let Some(template) = name_template {
            base_stack = self.scope_stacks.push_template(
                base_stack,
                template,
                &self.scope_templates,
                &self.scope_names,
            );
        } else if let Some(name) = name {
            base_stack =
                self.push_scope_text_id(base_stack, &substitute_scope_text(name, line, result));
        }
        if captures.entries.is_empty() {
            self.push_token(tokens, result.start..result.end, base_stack);
            return;
        }
        let match_end = result.end;
        let outside = captures
            .entries
            .iter()
            .filter_map(|(group, entry)| {
                if entry.name.is_none() && entry.patterns.is_empty() {
                    return None;
                }
                let range = result.capture(*group as usize)?;
                (match_end > result.start && range.start >= match_end && range.end > match_end)
                    .then_some((range, entry.clone()))
            })
            .collect::<Vec<_>>();
        if outside.is_empty() {
            self.emit_capture_range(
                tokens,
                line,
                result.start..result.end,
                grammar_id,
                base_stack,
                captures,
                result,
            );
            return;
        }
        self.emit_capture_range(
            tokens,
            line,
            result.start..result.end,
            grammar_id,
            base_stack,
            captures,
            result,
        );
        for (range, entry) in outside {
            let range = range.start.max(match_end)..range.end;
            let mut stack = base_stack;
            if let Some(scope_id) = entry.name {
                let (name, template) =
                    self.capture_scope_application(grammar_id, scope_id, line, result);
                stack = self.push_scope_application(stack, name.as_deref(), template);
            }
            if entry.patterns.is_empty() {
                self.push_token(tokens, range, stack);
            } else {
                self.tokenize_inline_patterns(
                    tokens,
                    line,
                    range,
                    grammar_id,
                    stack,
                    &entry.patterns,
                    true,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_capture_range(
        &mut self,
        tokens: &mut Vec<CompactScopedToken>,
        line: &str,
        range: Range<usize>,
        grammar_id: GrammarId,
        base_stack: ScopeStackId,
        capture_spec: &CaptureSpec,
        result: &MatchResult,
    ) {
        if range.start >= range.end {
            return;
        }
        if self.grammars.grammar(grammar_id).is_none() {
            self.push_token(tokens, range, base_stack);
            return;
        }
        // Match vscode-textmate's ordered capture handling. Capture groups are
        // semantic events in numeric order, not a geometric range tree:
        // overlapping named captures form a small active stack, while a
        // retokenized capture always starts from the rule/content stack plus
        // that capture's own name. Inheriting unrelated overlapping capture
        // names here adds broad `meta.head.*` scopes to C++ child tokens.
        let mut cursor = range.start;
        let mut active = CaptureScopeStack::default();
        for (group, entry) in &capture_spec.entries {
            let Some(capture_range) = result.capture(*group as usize) else {
                continue;
            };
            if capture_range.start >= capture_range.end {
                continue;
            }
            if capture_range.start > range.end {
                break;
            }
            let capture_range = clamp_range(capture_range, range.clone());
            if capture_range.start >= capture_range.end {
                continue;
            }

            while active
                .last()
                .is_some_and(|(_, end)| *end <= capture_range.start)
            {
                let (stack, end) = active.pop().expect("checked active capture");
                let end = end.min(range.end);
                if cursor < end {
                    self.push_token(tokens, cursor..end, stack);
                    cursor = end;
                }
            }
            let current_stack = active.last().map_or(base_stack, |(stack, _)| *stack);
            if cursor < capture_range.start {
                self.push_token(tokens, cursor..capture_range.start, current_stack);
                cursor = capture_range.start;
            }

            let (name, name_template) = entry.name.map_or((None, None), |scope_id| {
                self.capture_scope_application(grammar_id, scope_id, line, result)
            });
            if !entry.patterns.is_empty() {
                let stack = self.push_scope_application(base_stack, name.as_deref(), name_template);
                self.tokenize_inline_patterns(
                    tokens,
                    line,
                    capture_range.clone(),
                    grammar_id,
                    stack,
                    &entry.patterns,
                    true,
                );
                cursor = cursor.max(capture_range.end);
            } else if entry.name.is_some() {
                let stack =
                    self.push_scope_application(current_stack, name.as_deref(), name_template);
                active.push((stack, capture_range.end));
            }
        }

        while let Some((stack, end)) = active.pop() {
            let end = end.min(range.end);
            if cursor < end {
                self.push_token(tokens, cursor..end, stack);
                cursor = end;
            }
        }
        if cursor < range.end {
            self.push_token(tokens, cursor..range.end, base_stack);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn tokenize_inline_patterns(
        &mut self,
        tokens: &mut Vec<CompactScopedToken>,
        line: &str,
        range: Range<usize>,
        grammar_id: GrammarId,
        base_stack: ScopeStackId,
        patterns: &[RuleRef],
        compound_patterns: bool,
    ) {
        let base_stack_id = base_stack;
        let mut state = TokenizerState::default();
        let mut local_candidate_cache = HashMap::<TokenizerState, Arc<CandidateSet>>::new();
        let mut cursor = range.start;
        let mut steps = 0usize;
        let mut fallback_steps = 0u64;
        let mut anchor_pos = Some(range.start);
        let mut frame_anchor_positions = Vec::new();
        let mut zero_width_states = HashSet::new();
        // Capture retokenization is bounded by the capture. Let lookbehind see
        // the original prefix, but do not let a greedy child consume text
        // after the capture (for example the closing `]` after a TOML key).
        let scan_line = line.get(..range.end).unwrap_or(line);
        while cursor < range.end && steps < MAX_TOKENIZER_STEPS_PER_LINE {
            steps += 1;
            let candidate_set = if let Some(cached) = local_candidate_cache.get(&state) {
                cached.clone()
            } else {
                let cache_key = InlineCandidateCacheKey {
                    grammar_id,
                    patterns: patterns.to_vec(),
                    compound_patterns,
                    state: state.clone(),
                    base_stack: base_stack_id,
                };
                let candidate_set = if let Some(cached) =
                    self.inline_candidate_cache.get(&cache_key)
                {
                    cached.clone()
                } else {
                    let candidate_set = if state.is_initial() {
                        let (injection_outcome_id, injection_outcome) =
                            self.injection_outcome(&[] as &[Arc<str>]);
                        let source = CandidateSourceKey::Inline {
                            grammar_id,
                            patterns: Arc::from(patterns),
                            compound_patterns,
                        };
                        let prepared = self.prepared_blueprint_key(
                            source,
                            injection_outcome_id,
                            injection_outcome.as_ref(),
                        );
                        self.build_candidate_set(
                            prepared,
                            base_stack_id,
                            base_stack_id,
                            |tokenizer| {
                                let mut candidates = Vec::new();
                                let mut order = 0usize;
                                tokenizer.flatten_refs(
                                    grammar_id,
                                    grammar_id,
                                    patterns,
                                    None,
                                    &mut candidates,
                                    &mut order,
                                    0,
                                );
                                candidates
                            },
                        )
                    } else {
                        let stacks = self.current_scope_stack_ids(&state, Some(base_stack_id));
                        let active_scopes = self.resolve_scope_stack_cached(stacks.active_stack_id);
                        let (injection_outcome_id, injection_outcome) =
                            self.injection_outcome(active_scopes.as_ref());
                        let source = CandidateSourceKey::for_state(self.root, &state);
                        let prepared = self.prepared_blueprint_key(
                            source,
                            injection_outcome_id,
                            injection_outcome.as_ref(),
                        );
                        self.build_candidate_set(
                            prepared,
                            stacks.active_stack_id,
                            stacks.end_stack_id,
                            |tokenizer| tokenizer.candidates_for_state(&state, &injection_outcome),
                        )
                    };
                    let candidate_set = Arc::new(candidate_set);
                    if self.inline_candidate_cache.len() >= MAX_INLINE_CANDIDATE_SETS {
                        self.inline_candidate_cache.clear();
                    }
                    self.inline_candidate_cache
                        .insert(cache_key, candidate_set.clone());
                    if let Some(counters) = self.counters_mut() {
                        counters.record_inline_candidate_set_construction();
                    }
                    candidate_set
                };
                local_candidate_cache.insert(state.clone(), candidate_set.clone());
                candidate_set
            };
            if candidate_set.candidates.is_empty() {
                self.push_token(tokens, cursor..range.end, candidate_set.active_stack_id);
                return;
            }
            let search = self.find_best_candidate(
                &candidate_set,
                scan_line,
                cursor,
                false,
                anchor_pos,
                None,
            );
            fallback_steps = fallback_steps.saturating_add(search.fallback_steps);
            if fallback_steps > MAX_FALLBACK_STEPS_PER_LINE
                || !self.consume_fallback_call_budget(search.fallback_steps)
            {
                if let Some(counters) = self.counters_mut() {
                    counters.record_fallback_budget_kill();
                }
                self.push_token(tokens, cursor..range.end, candidate_set.active_stack_id);
                return;
            }
            let Some((candidate_index, mut result)) = search.best else {
                self.push_token(tokens, cursor..range.end, candidate_set.active_stack_id);
                return;
            };
            let result_start = result.start;
            let result_end = result.end;
            if result_start >= range.end || result_end > range.end {
                self.recycle_capture_result_buffer(std::mem::take(&mut result.captures));
                self.push_token(tokens, cursor..range.end, candidate_set.active_stack_id);
                return;
            }
            if cursor < result_start {
                self.push_token(tokens, cursor..result_start, candidate_set.active_stack_id);
            }
            let candidate = &candidate_set.candidates[candidate_index];
            let zero_width_match_rule = result_start == result_end
                && matches!(&candidate.kind, CandidateKind::Match { .. });
            if !compound_patterns
                && state.is_initial()
                && !matches!(candidate.kind, CandidateKind::Match { .. })
            {
                self.push_token(tokens, result_start..result_end, base_stack_id);
                cursor = advance_zero_width(scan_line, &(result_start..result_end));
                self.recycle_capture_result_buffer(std::mem::take(&mut result.captures));
                continue;
            }
            let depth_before = state.depth();
            let stack_before = state.frames.interned_id();
            let zero_width_state_before = (result_start == result_end
                && !matches!(candidate.kind, CandidateKind::Match { .. }))
            .then(|| state.clone());
            let next_cursor = self.apply_candidate(
                scan_line,
                &mut state,
                tokens,
                candidate,
                candidate_set.blueprint.match_name_template(candidate_index),
                result,
                &mut anchor_pos,
                &mut frame_anchor_positions,
                0,
                candidate_set.active_stack_id,
                candidate_set.end_stack_id,
            );
            if zero_width_match_rule {
                self.push_token(
                    tokens,
                    result_start..range.end,
                    candidate_set.active_stack_id,
                );
                return;
            }
            let zero_width_state_change =
                next_cursor == result_start && state.depth() != depth_before;
            if zero_width_state_change {
                zero_width_states.insert((result_start, stack_before));
                if !zero_width_states.insert((result_start, state.frames.interned_id())) {
                    if let Some(previous_state) = zero_width_state_before {
                        state = previous_state;
                    }
                    let stack = self.current_scope_stack_id(&state, true, Some(base_stack_id));
                    self.push_token(tokens, result_start..range.end, stack);
                    return;
                }
            }
            cursor = if zero_width_state_change {
                next_cursor
            } else if next_cursor <= result_start {
                next_char_boundary(scan_line, result_start)
            } else {
                next_cursor
            };
        }
        if cursor < range.end {
            let stack = self.current_scope_stack_id(&state, true, Some(base_stack_id));
            self.push_token(tokens, cursor..range.end, stack);
        }
    }

    fn current_scope_stack_id(
        &mut self,
        state: &TokenizerState,
        include_top_content: bool,
        base_stack: Option<ScopeStackId>,
    ) -> ScopeStackId {
        let stacks = self.current_scope_stack_ids(state, base_stack);
        if include_top_content {
            stacks.active_stack_id
        } else {
            stacks.end_stack_id
        }
    }

    fn current_scope_stack_ids(
        &mut self,
        state: &TokenizerState,
        base_stack: Option<ScopeStackId>,
    ) -> CachedCurrentScopeStackIds {
        let base_stack = match base_stack {
            Some(base_stack) => base_stack,
            None => self.root_scope_stack_id(),
        };
        self.current_scope_stack_ids_for_stack(state.frames.interned_id(), base_stack)
    }

    fn current_scope_stack_ids_for_stack(
        &mut self,
        frame_stack: InternedFrameStackId,
        base_stack: ScopeStackId,
    ) -> CachedCurrentScopeStackIds {
        let mut cursor = frame_stack;
        let mut missing = Vec::new();
        let mut cached = loop {
            let key = CurrentScopeStackKey {
                root: self.root,
                base_stack,
                frame_stack: cursor,
            };
            if let Some(cached) = self.current_scope_stack_cache.get(&key).copied() {
                break cached;
            }
            if cursor == InternedFrameStackId::default() {
                let cached = CachedCurrentScopeStackIds {
                    active_stack_id: base_stack,
                    end_stack_id: base_stack,
                };
                self.insert_current_scope_stack_cache(key, cached);
                break cached;
            }
            let frame = self
                .frame_stack_interner
                .scope_data(cursor)
                .expect("interned frame stack id has scope data");
            let parent = frame.parent;
            missing.push((cursor, frame));
            cursor = parent;
        };

        while let Some((stack_id, frame)) = missing.pop() {
            cached = self.extend_current_scope_stack_ids(cached, &frame);
            let key = CurrentScopeStackKey {
                root: self.root,
                base_stack,
                frame_stack: stack_id,
            };
            self.insert_current_scope_stack_cache(key, cached);
        }
        cached
    }

    fn extend_current_scope_stack_ids(
        &mut self,
        parent: CachedCurrentScopeStackIds,
        frame: &InternedFrameStackScopeData,
    ) -> CachedCurrentScopeStackIds {
        let mut end_stack = parent.active_stack_id;
        if let Some(prefix) = frame.scope_prefix.as_deref() {
            end_stack = self.push_scope_prefix_once_id(end_stack, prefix);
        }
        if let Some(name) = frame.name.as_deref() {
            end_stack = self.push_scope_text_id(end_stack, name);
        }
        let mut active_stack = end_stack;
        if let Some(content) = frame.content_name.as_deref() {
            active_stack = self.push_scope_text_id(active_stack, content);
        }
        CachedCurrentScopeStackIds {
            active_stack_id: active_stack,
            end_stack_id: end_stack,
        }
    }

    fn insert_current_scope_stack_cache(
        &mut self,
        key: CurrentScopeStackKey,
        value: CachedCurrentScopeStackIds,
    ) {
        if self.current_scope_stack_cache.len() >= MAX_SCOPE_STACK_CACHE_ENTRIES {
            self.current_scope_stack_cache.clear();
        }
        self.current_scope_stack_cache.entry(key).or_insert(value);
    }

    fn resolve_scope_stack_cached(&mut self, stack: ScopeStackId) -> Arc<[Arc<str>]> {
        if let Some(scopes) = self.resolved_scope_stack_cache.get(&stack).cloned() {
            return scopes;
        }
        if self.resolved_scope_stack_cache.len() >= MAX_SCOPE_STACK_CACHE_ENTRIES {
            self.resolved_scope_stack_cache.clear();
        }
        self.scope_stacks
            .resolve_ids_into(stack, &mut self.scope_resolution_scratch);
        let scopes = self
            .scope_resolution_scratch
            .iter()
            .map(|scope| {
                self.scope_names
                    .get_arc(*scope)
                    .expect("scope-stack IDs come from the scope interner")
            })
            .collect::<Arc<[Arc<str>]>>();
        self.resolved_scope_stack_cache
            .insert(stack, Arc::clone(&scopes));
        scopes
    }

    fn root_scope_stack_id(&mut self) -> ScopeStackId {
        let Some(root_scope) = self
            .grammars
            .grammar(self.root)
            .map(|grammar| grammar.scope_name.clone())
        else {
            return self.scope_stacks.empty();
        };
        let empty = self.scope_stacks.empty();
        let root_scope = self.scope_names.intern(&root_scope);
        self.scope_stacks.push(empty, root_scope, &self.scope_names)
    }

    fn push_scope_text_id(&mut self, stack: ScopeStackId, text: &str) -> ScopeStackId {
        let template = self
            .scope_templates
            .intern_scope_template(text, &mut self.scope_names);
        self.scope_stacks
            .push_template(stack, template, &self.scope_templates, &self.scope_names)
    }

    fn capture_scope_application(
        &mut self,
        grammar_id: GrammarId,
        scope_id: ScopeId,
        line: &str,
        result: &MatchResult,
    ) -> (Option<String>, Option<ScopeTemplateId>) {
        let key = (grammar_id, scope_id);
        if let Some(template) = self.capture_scope_templates.get(&key) {
            return (None, Some(*template));
        }
        let Some(text) = self
            .grammars
            .grammar(grammar_id)
            .and_then(|grammar| grammar.scope(scope_id))
            .map(str::to_owned)
        else {
            return (None, None);
        };
        if text.contains('$') {
            return (Some(substitute_scope_text(&text, line, result)), None);
        }
        let template = self
            .scope_templates
            .intern_scope_template(&text, &mut self.scope_names);
        self.capture_scope_templates.insert(key, template);
        (None, Some(template))
    }

    fn push_scope_application(
        &mut self,
        stack: ScopeStackId,
        name: Option<&str>,
        template: Option<ScopeTemplateId>,
    ) -> ScopeStackId {
        if let Some(template) = template {
            self.scope_stacks.push_template(
                stack,
                template,
                &self.scope_templates,
                &self.scope_names,
            )
        } else if let Some(name) = name {
            self.push_scope_text_id(stack, name)
        } else {
            stack
        }
    }

    fn push_scope_prefix_once_id(&mut self, stack: ScopeStackId, text: &str) -> ScopeStackId {
        let template = self
            .scope_templates
            .intern_prefix_template(text, &mut self.scope_names);
        self.scope_stacks.push_template_once(
            stack,
            template,
            &self.scope_templates,
            &self.scope_names,
        )
    }

    fn push_token(
        &self,
        tokens: &mut Vec<CompactScopedToken>,
        mut range: Range<usize>,
        stack: ScopeStackId,
    ) {
        // Token production is monotone. Ordered capture handling can revisit
        // an overlapping group after a nested capture has already emitted its
        // range; vscode-textmate's LineTokens ignores that covered prefix.
        if let Some(last) = tokens.last() {
            range.start = range.start.max(last.range.end);
        }
        if range.start >= range.end {
            return;
        }
        if let Some(last) = tokens.last_mut()
            && last.range.end == range.start
            && last.stack == stack
        {
            last.range.end = range.end;
            return;
        }
        tokens.push(CompactScopedToken { range, stack });
    }
}

#[derive(Debug, Clone)]
struct CandidateSet {
    blueprint: BoundCandidateBlueprint,
    active_stack_id: ScopeStackId,
    end_stack_id: ScopeStackId,
}

/// Capture nesting is almost always one or two levels. Keep the common
/// ordered-capture stack inline so capture emission does not allocate per
/// match; pathological grammars retain an unbounded overflow path.
#[derive(Debug, Default)]
struct CaptureScopeStack {
    inline: [(ScopeStackId, usize); 8],
    inline_len: usize,
    overflow: Vec<(ScopeStackId, usize)>,
}

impl CaptureScopeStack {
    fn last(&self) -> Option<&(ScopeStackId, usize)> {
        self.overflow.last().or_else(|| {
            self.inline_len
                .checked_sub(1)
                .map(|index| &self.inline[index])
        })
    }

    fn push(&mut self, value: (ScopeStackId, usize)) {
        if self.inline_len < self.inline.len() && self.overflow.is_empty() {
            self.inline[self.inline_len] = value;
            self.inline_len += 1;
        } else {
            self.overflow.push(value);
        }
    }

    fn pop(&mut self) -> Option<(ScopeStackId, usize)> {
        if let Some(value) = self.overflow.pop() {
            Some(value)
        } else if self.inline_len != 0 {
            self.inline_len -= 1;
            Some(self.inline[self.inline_len])
        } else {
            None
        }
    }
}

impl Deref for CandidateSet {
    type Target = CandidateBlueprint;

    fn deref(&self) -> &Self::Target {
        self.blueprint.blueprint()
    }
}

#[derive(Debug, Clone)]
enum BoundCandidateBlueprint {
    Owned(Arc<CandidateBlueprint>),
    Shared {
        blueprint: Arc<CandidateBlueprint>,
        match_name_templates: Arc<[Option<ScopeTemplateId>]>,
    },
}

impl BoundCandidateBlueprint {
    fn blueprint(&self) -> &CandidateBlueprint {
        self.blueprint_arc()
    }

    fn blueprint_arc(&self) -> &Arc<CandidateBlueprint> {
        match self {
            Self::Owned(blueprint) => blueprint,
            Self::Shared { blueprint, .. } => blueprint,
        }
    }

    fn shared_blueprint(&self) -> Option<&Arc<CandidateBlueprint>> {
        match self {
            Self::Owned(_) => None,
            Self::Shared { blueprint, .. } => Some(blueprint),
        }
    }

    fn match_name_template(&self, index: usize) -> Option<ScopeTemplateId> {
        match self {
            Self::Owned(blueprint) => blueprint.candidates.get(index).and_then(|candidate| {
                if let CandidateKind::Match { name_template, .. } = &candidate.kind {
                    *name_template
                } else {
                    None
                }
            }),
            Self::Shared {
                match_name_templates,
                ..
            } => match_name_templates.get(index).copied().flatten(),
        }
    }
}

#[derive(Debug)]
struct CandidateBlueprint {
    candidates: Vec<Candidate>,
    matchers: Arc<[Arc<CompiledPattern>]>,
    pattern_set_search: Option<PatternSetMatcher>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CandidateBlueprintKey {
    source: CandidateSourceKey,
    injection_outcome: InjectionOutcomeId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PreparedInjectionOutcomeId(u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PreparedBlueprintKey {
    source: CandidateSourceKey,
    injection_outcome: PreparedInjectionOutcomeId,
}

fn rule_ref_retained_bytes(rule_ref: &RuleRef) -> usize {
    match rule_ref {
        RuleRef::Repository(name) => name.len(),
        RuleRef::External {
            repository: Some(name),
            ..
        } => name.len(),
        RuleRef::Rule(_)
        | RuleRef::SelfRef
        | RuleRef::BaseRef
        | RuleRef::External {
            repository: None, ..
        } => 0,
    }
}

fn injection_candidate_retained_bytes(candidate: &InjectionCandidate) -> usize {
    let mut owned_bytes = 0usize;
    for rule_ref in &candidate.patterns {
        owned_bytes = owned_bytes.saturating_add(rule_ref_retained_bytes(rule_ref));
    }
    std::mem::size_of::<InjectionCandidate>()
        .saturating_add(
            candidate
                .patterns
                .len()
                .saturating_mul(std::mem::size_of::<RuleRef>()),
        )
        .saturating_add(owned_bytes)
}

fn injection_outcome_retained_bytes(outcome: &InjectionOutcome) -> usize {
    let map_entry_bytes = std::mem::size_of::<(InjectionOutcome, PreparedInjectionOutcomeId)>();
    let mut bytes = std::mem::size_of::<InjectionOutcome>()
        .saturating_add(std::mem::size_of::<PreparedInjectionOutcomeId>())
        // Cover the hash table's spare buckets and control bytes, including
        // its relatively high first-allocation overhead.
        .saturating_add(map_entry_bytes.saturating_mul(3));
    for candidate in outcome.left.iter().chain(&outcome.right) {
        bytes = bytes.saturating_add(injection_candidate_retained_bytes(candidate));
    }
    bytes
}

fn prepared_blueprint_key_retained_bytes(key: &PreparedBlueprintKey) -> usize {
    let dynamic_bytes = match &key.source {
        CandidateSourceKey::Root(_) => 0,
        CandidateSourceKey::Inline { patterns, .. } => rule_refs_retained_bytes(patterns),
        CandidateSourceKey::Frame {
            scope_prefix,
            end_pattern,
            ..
        } => scope_prefix
            .as_deref()
            .map_or(0, str::len)
            .saturating_add(end_pattern.as_deref().map_or(0, str::len)),
    };
    std::mem::size_of::<PreparedBlueprintKey>().saturating_add(dynamic_bytes)
}

fn rule_refs_retained_bytes(refs: &[RuleRef]) -> usize {
    let mut bytes = refs.len().saturating_mul(std::mem::size_of::<RuleRef>());
    for rule_ref in refs {
        bytes = bytes.saturating_add(rule_ref_retained_bytes(rule_ref));
    }
    bytes
}

fn capture_spec_retained_bytes(captures: &CaptureSpec) -> usize {
    let mut bytes = std::mem::size_of::<CaptureSpec>();
    for entry in captures.entries.values() {
        bytes = bytes
            .saturating_add(std::mem::size_of_val(entry))
            .saturating_add(rule_refs_retained_bytes(&entry.patterns));
    }
    bytes
}

fn candidate_dynamic_retained_bytes(candidate: &Candidate) -> usize {
    let mut bytes = candidate
        .pattern
        .len()
        .saturating_add(candidate.scope_prefix.as_deref().map_or(0, str::len));
    bytes = bytes.saturating_add(match &candidate.kind {
        CandidateKind::Match { name, captures, .. } => name
            .as_deref()
            .map_or(0, str::len)
            .saturating_add(capture_spec_retained_bytes(captures)),
        CandidateKind::BeginEnd {
            begin_captures,
            end_captures,
            name,
            content_name,
            patterns,
            end_static,
            ..
        } => capture_spec_retained_bytes(begin_captures)
            .saturating_add(capture_spec_retained_bytes(end_captures))
            .saturating_add(name.as_deref().map_or(0, str::len))
            .saturating_add(content_name.as_deref().map_or(0, str::len))
            .saturating_add(rule_refs_retained_bytes(patterns))
            .saturating_add(end_static.as_deref().map_or(0, str::len)),
        CandidateKind::BeginWhile {
            begin_captures,
            while_captures,
            name,
            content_name,
            patterns,
            while_static,
            ..
        } => capture_spec_retained_bytes(begin_captures)
            .saturating_add(capture_spec_retained_bytes(while_captures))
            .saturating_add(name.as_deref().map_or(0, str::len))
            .saturating_add(content_name.as_deref().map_or(0, str::len))
            .saturating_add(rule_refs_retained_bytes(patterns))
            .saturating_add(while_static.as_deref().map_or(0, str::len)),
        CandidateKind::End { captures, .. } => capture_spec_retained_bytes(captures),
    });
    bytes
}

/// Conservative admission charge for data uniquely retained by a prepared
/// blueprint. Compiled regexes are charged to the separately bounded pattern
/// cache; the set surcharge covers its per-pattern scanner/index allocations.
fn candidate_blueprint_retained_bytes(blueprint: &CandidateBlueprint) -> usize {
    let mut candidate_bytes = blueprint
        .candidates
        .capacity()
        .saturating_mul(std::mem::size_of::<Candidate>());
    for candidate in &blueprint.candidates {
        let dynamic_bytes = candidate_dynamic_retained_bytes(candidate);
        candidate_bytes = candidate_bytes.saturating_add(dynamic_bytes);
    }
    let matcher_bytes = blueprint
        .matchers
        .len()
        .saturating_mul(std::mem::size_of::<Arc<CompiledPattern>>());
    let set_bytes = blueprint
        .pattern_set_search
        .as_ref()
        .map_or(0, PatternSetMatcher::retained_heap_bytes);
    std::mem::size_of::<CandidateBlueprint>()
        .saturating_add(candidate_bytes)
        .saturating_add(matcher_bytes)
        .saturating_add(set_bytes)
        .saturating_mul(2)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CurrentScopeStackKey {
    root: GrammarId,
    base_stack: ScopeStackId,
    frame_stack: InternedFrameStackId,
}

#[derive(Debug, Clone, Copy)]
struct CachedCurrentScopeStackIds {
    active_stack_id: ScopeStackId,
    end_stack_id: ScopeStackId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum CandidateSourceKey {
    Root(GrammarId),
    Inline {
        grammar_id: GrammarId,
        patterns: Arc<[RuleRef]>,
        compound_patterns: bool,
    },
    Frame {
        grammar_id: GrammarId,
        base_grammar_id: GrammarId,
        rule_id: RuleId,
        scope_prefix: Option<Arc<str>>,
        end_pattern: Option<Arc<str>>,
        end_pattern_id: Option<PatternId>,
        apply_end_pattern_last: bool,
    },
}

impl CandidateSourceKey {
    fn for_state(root: GrammarId, state: &TokenizerState) -> Self {
        state
            .frames
            .last()
            .map_or(Self::Root(root), |frame| Self::Frame {
                grammar_id: frame.grammar_id,
                base_grammar_id: frame.base_grammar_id,
                rule_id: frame.rule_id,
                scope_prefix: frame.scope_prefix.clone(),
                end_pattern: frame.end_pattern.clone(),
                end_pattern_id: frame.end_pattern_id,
                apply_end_pattern_last: frame.apply_end_pattern_last,
            })
    }

    fn is_static(&self) -> bool {
        match self {
            Self::Root(_) | Self::Inline { .. } => true,
            Self::Frame {
                end_pattern,
                end_pattern_id,
                ..
            } => end_pattern.is_none() || end_pattern_id.is_some(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DynamicMatcherKey {
    pattern: String,
    live_captures: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct InlineCandidateCacheKey {
    grammar_id: GrammarId,
    patterns: Vec<RuleRef>,
    compound_patterns: bool,
    state: TokenizerState,
    base_stack: ScopeStackId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PatternHotspotKey {
    root_scope: String,
    grammar_id: Option<u16>,
    pattern_id: Option<u32>,
    engine: String,
    pattern: String,
}

#[derive(Debug, Clone)]
struct Candidate {
    order: usize,
    base_grammar_id: GrammarId,
    pattern: String,
    pattern_id: Option<(GrammarId, PatternId)>,
    scope_prefix: Option<Arc<str>>,
    kind: CandidateKind,
}

#[derive(Debug, Clone)]
enum CandidateKind {
    Match {
        grammar_id: GrammarId,
        name: Option<String>,
        name_template: Option<ScopeTemplateId>,
        captures: Arc<CaptureSpec>,
    },
    BeginEnd {
        grammar_id: GrammarId,
        rule_id: RuleId,
        end: PatternId,
        begin_captures: Arc<CaptureSpec>,
        end_captures: Arc<CaptureSpec>,
        name: Option<Arc<str>>,
        content_name: Option<Arc<str>>,
        patterns: Arc<[RuleRef]>,
        apply_end_pattern_last: bool,
        /// End pattern text when it contains no backreferences, so pushes
        /// skip capture substitution and reuse one shared allocation.
        end_static: Option<Arc<str>>,
    },
    BeginWhile {
        grammar_id: GrammarId,
        rule_id: RuleId,
        while_pattern: PatternId,
        begin_captures: Arc<CaptureSpec>,
        while_captures: Arc<CaptureSpec>,
        name: Option<Arc<str>>,
        content_name: Option<Arc<str>>,
        patterns: Arc<[RuleRef]>,
        while_static: Option<Arc<str>>,
    },
    End {
        grammar_id: GrammarId,
        captures: Arc<CaptureSpec>,
    },
}

fn candidate_is_suppressed(
    candidate: &Candidate,
    suppressed: &HashSet<(GrammarId, RuleId)>,
) -> bool {
    match &candidate.kind {
        CandidateKind::BeginEnd {
            grammar_id,
            rule_id,
            ..
        }
        | CandidateKind::BeginWhile {
            grammar_id,
            rule_id,
            ..
        } => suppressed.contains(&(*grammar_id, *rule_id)),
        CandidateKind::Match { .. } | CandidateKind::End { .. } => false,
    }
}

#[cfg(test)]
fn candidate_requires_capture_replay(candidate: &Candidate) -> bool {
    match &candidate.kind {
        CandidateKind::Match { name, captures, .. } => {
            !captures.entries.is_empty() || name.as_ref().is_some_and(|name| name.contains('$'))
        }
        CandidateKind::End { captures, .. } => !captures.entries.is_empty(),
        CandidateKind::BeginEnd { .. } | CandidateKind::BeginWhile { .. } => true,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct InjectionCandidate {
    grammar_id: GrammarId,
    patterns: Vec<RuleRef>,
}

#[derive(Debug, Clone)]
struct CompiledInjectionSelector {
    grammar_id: GrammarId,
    priority: InjectionPriority,
    patterns: Vec<RuleRef>,
    selector_tokens: Arc<[SelectorToken]>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
struct InjectionOutcome {
    left: Vec<InjectionCandidate>,
    right: Vec<InjectionCandidate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct InjectionOutcomeId(u64);

#[derive(Debug, Clone, Default)]
struct InjectionOutcomeInterner {
    ids: HashMap<InjectionOutcome, InjectionOutcomeId>,
    values: HashMap<InjectionOutcomeId, Arc<InjectionOutcome>>,
    next_id: u64,
}

impl InjectionOutcomeInterner {
    fn len(&self) -> usize {
        self.ids.len()
    }

    fn contains(&self, outcome: &InjectionOutcome) -> bool {
        self.ids.contains_key(outcome)
    }

    fn intern(&mut self, outcome: InjectionOutcome) -> (InjectionOutcomeId, Arc<InjectionOutcome>) {
        if let Some(id) = self.ids.get(&outcome).copied() {
            return (
                id,
                self.values
                    .get(&id)
                    .cloned()
                    .expect("interned injection outcome has a value"),
            );
        }
        let id = InjectionOutcomeId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        let value = Arc::new(outcome.clone());
        self.ids.insert(outcome, id);
        self.values.insert(id, value.clone());
        (id, value)
    }

    fn clear(&mut self) {
        self.ids.clear();
        self.values.clear();
    }
}

#[derive(Debug, Clone)]
struct StateInterner {
    states: Vec<TokenizerState>,
    // `TokenizerState` equality is exactly interned-frame-stack-id equality
    // (`FrameStack::eq`), so the id map can key on the u32 id directly and
    // probing never clones or walks a state.
    ids: FastMap<InternedFrameStackId, StateId>,
}

#[derive(Debug, Clone, Default)]
struct StateIdentityHasher(u64);

impl Hasher for StateIdentityHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn write_u64(&mut self, value: u64) {
        self.0 = value;
    }

    fn write_u32(&mut self, value: u32) {
        self.0 = u64::from(value);
    }
}

impl StateInterner {
    fn new() -> Self {
        let mut interner = Self {
            states: Vec::new(),
            ids: hashing::fast_map(),
        };
        interner.intern(&TokenizerState::default());
        interner
    }

    fn intern(&mut self, state: &TokenizerState) -> (StateId, bool) {
        let key = state.frames.interned_id();
        if let Some(id) = self.ids.get(&key) {
            return (*id, false);
        }
        let id = StateId(self.states.len() as u32);
        self.states.push(state.clone());
        self.ids.insert(key, id);
        (id, true)
    }

    fn get(&self, id: StateId) -> Option<&TokenizerState> {
        self.states.get(id.0 as usize)
    }

    fn len(&self) -> usize {
        self.states.len()
    }
}

#[derive(Debug, Clone)]
struct CandidateSearchResult {
    best: Option<(usize, MatchResult)>,
    fallback_budget_killed: bool,
    fallback_steps: u64,
}

fn empty_repository_context() -> &'static Arc<RepositoryBindings> {
    static EMPTY: OnceLock<Arc<RepositoryBindings>> = OnceLock::new();
    EMPTY.get_or_init(|| Arc::new(RepositoryBindings::default()))
}

fn resolve_repository_in_context<'a>(
    grammar: &'a CompiledGrammar,
    name: &'a str,
    context: &RepositoryBindings,
) -> Option<&'a RuleRef> {
    let bound_name = context.get(name).map_or(name, String::as_str);
    grammar.repository.get(bound_name)
}

fn contextualize_refs(refs: &[RuleRef], context: Option<&RepositoryBindings>) -> Vec<RuleRef> {
    let Some(context) = context.filter(|context| !context.is_empty()) else {
        return refs.to_vec();
    };
    refs.iter()
        .map(|rule_ref| match rule_ref {
            RuleRef::Repository(name) => context
                .get(name)
                .map(|bound_name| RuleRef::Repository(bound_name.clone()))
                .unwrap_or_else(|| rule_ref.clone()),
            _ => rule_ref.clone(),
        })
        .collect()
}

fn contextualize_capture_spec(
    captures: &Arc<CaptureSpec>,
    context: Option<&RepositoryBindings>,
) -> Arc<CaptureSpec> {
    let Some(context) = context.filter(|context| !context.is_empty()) else {
        return Arc::clone(captures);
    };
    let mut contextualized = captures.as_ref().clone();
    for entry in contextualized.entries.values_mut() {
        entry.patterns = contextualize_refs(&entry.patterns, Some(context));
    }
    Arc::new(contextualized)
}

struct RepositoryContextBudget {
    bounded: bool,
    state_count: usize,
    retained_bytes: usize,
    exceeded: bool,
}

impl RepositoryContextBudget {
    fn new(bounded: bool) -> Self {
        Self {
            bounded,
            state_count: 0,
            retained_bytes: 0,
            exceeded: false,
        }
    }

    fn charge(&mut self, bytes: usize) -> bool {
        if !self.bounded {
            return true;
        }
        self.state_count = self.state_count.saturating_add(1);
        self.retained_bytes = self.retained_bytes.saturating_add(bytes);
        if self.state_count > MAX_PREPARED_GRAMMAR_WALK_STATES
            || self.retained_bytes > MAX_PREPARED_GRAMMAR_WALK_BYTES
        {
            self.exceeded = true;
            return false;
        }
        true
    }

    fn charge_ref(&mut self) -> bool {
        self.charge(std::mem::size_of::<RuleRef>())
    }

    fn charge_context_table(&mut self, grammar_count: usize) -> bool {
        self.charge(grammar_count.saturating_mul(std::mem::size_of::<
            Option<Box<GrammarRuleRepositoryContexts>>,
        >()))
    }

    fn charge_rule_table(&mut self, rule_count: usize) -> bool {
        self.charge(
            std::mem::size_of::<GrammarRuleRepositoryContexts>().saturating_add(
                rule_count.saturating_mul(std::mem::size_of::<Option<Arc<RepositoryBindings>>>()),
            ),
        )
    }

    fn charge_rule(&mut self, local: &BTreeMap<String, String>, sparse: bool) -> bool {
        let mut bytes = if local.is_empty() {
            0
        } else {
            std::mem::size_of::<RepositoryBindings>()
        };
        if sparse {
            // A public, hand-built grammar can have out-of-range IDs. Charge
            // conservatively for Vec growth in the uncommon fallback table.
            bytes =
                bytes.saturating_add(2 * std::mem::size_of::<(RuleId, Arc<RepositoryBindings>)>());
        }
        for (name, binding) in local {
            let entry_bytes = (4 * std::mem::size_of::<usize>())
                .saturating_add(2 * std::mem::size_of::<String>())
                .saturating_add(name.len())
                .saturating_add(binding.len());
            // Every binding is retained in its overlay and may be copied once
            // when that fixed-size overlay run is compacted.
            bytes = bytes.saturating_add(entry_bytes.saturating_mul(2));
        }
        self.charge(bytes)
    }

    fn charge_repository(&mut self, name: &str, newly_interned: bool) -> bool {
        let key_bytes = std::mem::size_of::<(GrammarId, RepositoryNameId, usize)>();
        let interned_bytes = newly_interned.then_some(
            (4 * std::mem::size_of::<usize>())
                .saturating_add(std::mem::size_of::<String>())
                .saturating_add(name.len()),
        );
        self.charge(
            key_bytes
                .saturating_mul(2)
                .saturating_add(interned_bytes.unwrap_or(0)),
        )
    }
}

/// Simulate vscode-textmate's lazy `RuleFactory.getCompiledRuleId` walk.
///
/// Raw rules receive an id the first time they are reached. That first walk's
/// repository object remains captured by the compiled rule, even if a shared
/// root rule is reached later through a different repository. The native
/// loader assigns ids ahead of time, so retain the first repository context in
/// a side table and apply it when candidates/capture rules are materialized.
fn compile_rule_repository_contexts<'a>(
    grammars: &'a GrammarSet,
    root: GrammarId,
    injections: &'a [CompiledInjectionSelector],
    bounded: bool,
) -> (RuleRepositoryContexts, bool) {
    enum Work<'a> {
        TopLevel {
            grammar_id: GrammarId,
            base_grammar_id: GrammarId,
            context: Arc<RepositoryBindings>,
        },
        Refs {
            grammar_id: GrammarId,
            base_grammar_id: GrammarId,
            refs: &'a [RuleRef],
            index: usize,
            context: Arc<RepositoryBindings>,
        },
        RepositoryExit((GrammarId, RepositoryNameId, usize)),
    }

    fn push_refs<'a>(
        work: &mut Vec<Work<'a>>,
        grammar_id: GrammarId,
        base_grammar_id: GrammarId,
        refs: &'a [RuleRef],
        context: Arc<RepositoryBindings>,
    ) {
        if !refs.is_empty() {
            work.push(Work::Refs {
                grammar_id,
                base_grammar_id,
                refs,
                index: 0,
                context,
            });
        }
    }

    fn push_captures<'a>(
        work: &mut Vec<Work<'a>>,
        grammar_id: GrammarId,
        captures: &'a CaptureSpec,
        context: &Arc<RepositoryBindings>,
    ) {
        for entry in captures.entries.values().rev() {
            push_refs(
                work,
                grammar_id,
                grammar_id,
                &entry.patterns,
                Arc::clone(context),
            );
        }
    }

    let empty_context = Arc::clone(empty_repository_context());
    let mut budget = RepositoryContextBudget::new(bounded);
    if !budget.charge_context_table(grammars.grammars().len()) {
        return (RuleRepositoryContexts::empty(), false);
    }
    let mut compiled = RuleRepositoryContexts::new(grammars.grammars().len());
    let mut compiled_top_levels = hashing::fast_set();
    let mut repository_names = RepositoryNameInterner::default();
    let mut visiting_repositories = HashSet::new();
    let mut work = Vec::new();
    for injection in injections.iter().rev() {
        push_refs(
            &mut work,
            injection.grammar_id,
            root,
            &injection.patterns,
            Arc::clone(&empty_context),
        );
    }
    work.push(Work::TopLevel {
        grammar_id: root,
        base_grammar_id: root,
        context: Arc::clone(&empty_context),
    });

    while !budget.exceeded {
        let Some(next) = work.pop() else {
            break;
        };
        match next {
            Work::RepositoryExit(key) => {
                visiting_repositories.remove(&key);
            }
            Work::TopLevel {
                grammar_id,
                base_grammar_id,
                context,
            } => {
                if !compiled_top_levels.insert(grammar_id) {
                    continue;
                }
                if let Some(grammar) = grammars.grammar(grammar_id) {
                    push_refs(
                        &mut work,
                        grammar_id,
                        base_grammar_id,
                        &grammar.top_level,
                        context,
                    );
                }
            }
            Work::Refs {
                grammar_id,
                base_grammar_id,
                refs,
                index,
                context,
            } => {
                let Some(rule_ref) = refs.get(index) else {
                    continue;
                };
                if !budget.charge_ref() {
                    break;
                }
                if index + 1 < refs.len() {
                    work.push(Work::Refs {
                        grammar_id,
                        base_grammar_id,
                        refs,
                        index: index + 1,
                        context: Arc::clone(&context),
                    });
                }
                let Some(grammar) = grammars.grammar(grammar_id) else {
                    continue;
                };
                match rule_ref {
                    RuleRef::Rule(rule_id) => {
                        if compiled.get(grammar_id, *rule_id).is_some() {
                            continue;
                        }
                        let Some(rule) = grammar.rule(*rule_id) else {
                            continue;
                        };
                        if !compiled.has_grammar_table(grammar_id)
                            && !budget.charge_rule_table(grammar.rules.len())
                        {
                            break;
                        }
                        let sparse = rule_id.0 as usize >= grammar.rules.len();
                        if !budget.charge_rule(&rule.local_repository, sparse) {
                            break;
                        }
                        let context = if rule.local_repository.is_empty() {
                            context
                        } else {
                            RepositoryBindings::overlay(
                                context,
                                rule.local_repository.clone(),
                                !bounded,
                            )
                        };
                        // Never overwrite an earlier context: vscode-textmate
                        // binds a raw rule to the repository from its first
                        // lazy-compilation path.
                        let inserted = compiled.insert_first(
                            grammar_id,
                            *rule_id,
                            grammar.rules.len(),
                            Arc::clone(&context),
                        );
                        debug_assert!(inserted);
                        match &rule.body {
                            RuleBody::Match { captures, .. } => {
                                push_captures(&mut work, grammar_id, captures, &context);
                            }
                            RuleBody::BeginEnd {
                                begin_captures,
                                end_captures,
                                patterns,
                                ..
                            } => {
                                push_refs(
                                    &mut work,
                                    grammar_id,
                                    base_grammar_id,
                                    patterns,
                                    Arc::clone(&context),
                                );
                                push_captures(&mut work, grammar_id, end_captures, &context);
                                push_captures(&mut work, grammar_id, begin_captures, &context);
                            }
                            RuleBody::BeginWhile {
                                begin_captures,
                                while_captures,
                                patterns,
                                ..
                            } => {
                                push_refs(
                                    &mut work,
                                    grammar_id,
                                    base_grammar_id,
                                    patterns,
                                    Arc::clone(&context),
                                );
                                push_captures(&mut work, grammar_id, while_captures, &context);
                                push_captures(&mut work, grammar_id, begin_captures, &context);
                            }
                            RuleBody::IncludeOnly { patterns } => {
                                push_refs(&mut work, grammar_id, base_grammar_id, patterns, context)
                            }
                        }
                    }
                    RuleRef::Repository(name) => {
                        let bound_name = context.get(name).map_or(name.as_str(), String::as_str);
                        let known_name = repository_names.get(bound_name);
                        if !budget.charge_repository(bound_name, known_name.is_none()) {
                            break;
                        }
                        let name_id =
                            known_name.unwrap_or_else(|| repository_names.intern(bound_name).0);
                        let key = (grammar_id, name_id, Arc::as_ptr(&context) as usize);
                        if !visiting_repositories.insert(key) {
                            continue;
                        }
                        work.push(Work::RepositoryExit(key));
                        if let Some(target) = resolve_repository_in_context(grammar, name, &context)
                        {
                            push_refs(
                                &mut work,
                                grammar_id,
                                base_grammar_id,
                                std::slice::from_ref(target),
                                context,
                            );
                        }
                    }
                    RuleRef::SelfRef => work.push(Work::TopLevel {
                        grammar_id,
                        base_grammar_id,
                        context,
                    }),
                    RuleRef::BaseRef => work.push(Work::TopLevel {
                        grammar_id: base_grammar_id,
                        base_grammar_id,
                        context: Arc::clone(&empty_context),
                    }),
                    RuleRef::External { scope, repository } => {
                        let Some(external_id) = grammar
                            .scope(*scope)
                            .and_then(|scope| grammars.grammar_id_by_scope(scope))
                        else {
                            continue;
                        };
                        let Some(external) = grammars.grammar(external_id) else {
                            continue;
                        };
                        if let Some(repository) = repository {
                            let known_name = repository_names.get(repository);
                            if !budget.charge_repository(repository, known_name.is_none()) {
                                break;
                            }
                            let name_id =
                                known_name.unwrap_or_else(|| repository_names.intern(repository).0);
                            let key = (external_id, name_id, Arc::as_ptr(&empty_context) as usize);
                            if !visiting_repositories.insert(key) {
                                continue;
                            }
                            work.push(Work::RepositoryExit(key));
                            if let Some(target) = external.repository.get(repository) {
                                push_refs(
                                    &mut work,
                                    external_id,
                                    base_grammar_id,
                                    std::slice::from_ref(target),
                                    Arc::clone(&empty_context),
                                );
                            }
                        } else {
                            work.push(Work::TopLevel {
                                grammar_id: external_id,
                                base_grammar_id,
                                context: Arc::clone(&empty_context),
                            });
                        }
                    }
                }
            }
        }
    }

    (compiled, !budget.exceeded)
}

#[derive(Debug, Clone)]
struct PatternSearchResult {
    result: Option<MatchResult>,
    fallback_budget_killed: bool,
    fallback_steps: u64,
}

fn scan_anchor_context(
    cursor: usize,
    is_first_line: bool,
    anchor_pos: Option<usize>,
) -> AnchorContext {
    AnchorContext {
        allow_a: is_first_line && cursor == 0,
        allow_g: anchor_pos == Some(cursor),
        g_pos: cursor,
    }
}

#[inline]
fn trace_regex_search(
    _pattern: &str,
    _line: &str,
    _from: usize,
    _ctx: AnchorContext,
    _engine: &str,
) {
}

pub fn advance_zero_width(line: &str, range: &Range<usize>) -> usize {
    if range.start == range.end {
        next_char_boundary(line, range.end)
    } else {
        range.end
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeSpan {
    pub range: Range<usize>,
    pub scope: &'static str,
}

pub fn tokenize_json_string_smoke(line: &str) -> Vec<ScopeSpan> {
    let bytes = line.as_bytes();
    let Some(start) = bytes.iter().position(|byte| *byte == b'"') else {
        return Vec::new();
    };
    let mut spans = vec![ScopeSpan {
        range: start..start + 1,
        scope: "punctuation.definition.string.begin.json",
    }];
    let mut cursor = start + 1;
    let mut content_start = cursor;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' => {
                if content_start < cursor {
                    spans.push(ScopeSpan {
                        range: content_start..cursor,
                        scope: "string.quoted.double.json",
                    });
                }
                let end = next_char_boundary(line, next_char_boundary(line, cursor));
                spans.push(ScopeSpan {
                    range: cursor..end,
                    scope: "constant.character.escape.json",
                });
                cursor = end;
                content_start = cursor;
            }
            b'"' => {
                if content_start < cursor {
                    spans.push(ScopeSpan {
                        range: content_start..cursor,
                        scope: "string.quoted.double.json",
                    });
                }
                spans.push(ScopeSpan {
                    range: cursor..cursor + 1,
                    scope: "punctuation.definition.string.end.json",
                });
                return spans;
            }
            _ => cursor = next_char_boundary(line, cursor),
        }
    }
    if content_start < line.len() {
        spans.push(ScopeSpan {
            range: content_start..line.len(),
            scope: "string.quoted.double.json",
        });
    }
    spans
}

fn scope_name(grammar: &CompiledGrammar, id: Option<super::state::ScopeId>) -> Option<String> {
    id.and_then(|id| grammar.scope(id).map(str::to_owned))
}

/// Mirrors `substitute_end_pattern`'s escape handling: a backslash consumes
/// the next character, and only `\1`..`\9` starts a backreference.
fn pattern_has_backreference(pattern: &str) -> bool {
    let mut chars = pattern.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' && matches!(chars.next(), Some('1'..='9')) {
            return true;
        }
    }
    false
}

fn is_non_matching_end_sentinel(pattern: &str) -> bool {
    // Missing and explicitly empty ends are persistent-frame sentinels. A
    // real `\z` remains matchable on the final logical line, whose parse text
    // has no synthetic trailing newline.
    pattern.is_empty()
}

fn shared_empty_capture_spec() -> Arc<CaptureSpec> {
    static EMPTY: OnceLock<Arc<CaptureSpec>> = OnceLock::new();
    Arc::clone(EMPTY.get_or_init(|| Arc::new(CaptureSpec::default())))
}

/// Resolves a possibly capture-referencing scope text: static names reuse the
/// candidate's shared allocation, `$n` names substitute per match.
fn frame_scope_text(name: &Option<Arc<str>>, line: &str, result: &MatchResult) -> Option<Arc<str>> {
    let name = name.as_ref()?;
    if name.contains('$') {
        Some(Arc::from(substitute_scope_text(name, line, result)))
    } else {
        Some(Arc::clone(name))
    }
}

fn substitute_scope_text(scope: &str, line: &str, result: &MatchResult) -> String {
    if !scope.contains('$') {
        return scope.to_owned();
    }
    let mut output = String::with_capacity(scope.len());
    let bytes = scope.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] != b'$' {
            let ch = scope[index..].chars().next().expect("valid scope char");
            output.push(ch);
            index += ch.len_utf8();
            continue;
        }
        if index + 1 < bytes.len() && bytes[index + 1] == b'{' {
            if let Some(close_offset) = scope[index + 2..].find('}') {
                let body_start = index + 2;
                let body_end = body_start + close_offset;
                let body = &scope[body_start..body_end];
                if let Some((group, transform)) = parse_scope_placeholder_body(body) {
                    push_scope_capture(&mut output, line, result, group, transform);
                    index = body_end + 1;
                    continue;
                }
            }
        } else if index + 1 < bytes.len() && bytes[index + 1].is_ascii_digit() {
            let mut end = index + 1;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            if let Ok(group) = scope[index + 1..end].parse::<usize>() {
                push_scope_capture(&mut output, line, result, group, ScopeTransform::None);
                index = end;
                continue;
            }
        }
        output.push('$');
        index += 1;
    }
    output
}

fn add_scope_capture_refs(scope: Option<&str>, live: &mut Vec<u32>) {
    let Some(scope) = scope.filter(|scope| scope.contains('$')) else {
        return;
    };
    let bytes = scope.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] != b'$' {
            index += scope[index..]
                .chars()
                .next()
                .expect("valid scope char")
                .len_utf8();
            continue;
        }
        if index + 1 < bytes.len() && bytes[index + 1] == b'{' {
            if let Some(close_offset) = scope[index + 2..].find('}') {
                let body_start = index + 2;
                let body_end = body_start + close_offset;
                if let Some((group, _)) = parse_scope_placeholder_body(&scope[body_start..body_end])
                {
                    if let Ok(group) = u32::try_from(group) {
                        live.push(group);
                    }
                    index = body_end + 1;
                    continue;
                }
            }
        } else if index + 1 < bytes.len() && bytes[index + 1].is_ascii_digit() {
            let mut end = index + 1;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            if let Ok(group) = scope[index + 1..end].parse::<u32>() {
                live.push(group);
                index = end;
                continue;
            }
        }
        index += 1;
    }
}

fn add_end_pattern_capture_refs(pattern: &str, live: &mut Vec<u32>) {
    let mut chars = pattern.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            continue;
        }
        let Some(next @ '1'..='9') = chars.peek().copied() else {
            // Consume the escaped character exactly as substitution does, so
            // `\\\\1` remains a literal backslash followed by `1`.
            chars.next();
            continue;
        };
        let mut digits = String::new();
        digits.push(next);
        chars.next();
        while let Some(digit @ '0'..='9') = chars.peek().copied() {
            digits.push(digit);
            chars.next();
        }
        if let Ok(group) = digits.parse::<u32>() {
            live.push(group);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScopeTransform {
    None,
    Downcase,
    Upcase,
}

fn parse_scope_placeholder_body(body: &str) -> Option<(usize, ScopeTransform)> {
    let (group, transform) = body.split_once(':').unwrap_or((body, ""));
    let group = group.parse::<usize>().ok()?;
    let transform = match transform {
        "" => ScopeTransform::None,
        "/downcase" => ScopeTransform::Downcase,
        "/upcase" => ScopeTransform::Upcase,
        _ => return None,
    };
    Some((group, transform))
}

fn push_scope_capture(
    output: &mut String,
    line: &str,
    result: &MatchResult,
    group: usize,
    transform: ScopeTransform,
) {
    let Some(text) = result.capture(group).and_then(|range| line.get(range)) else {
        return;
    };
    match transform {
        ScopeTransform::None => output.push_str(text),
        ScopeTransform::Downcase => output.push_str(&text.to_lowercase()),
        ScopeTransform::Upcase => output.push_str(&text.to_uppercase()),
    }
}

fn fallback_call_budget(source_bytes: usize) -> u64 {
    MIN_FALLBACK_STEPS_PER_CALL.max(
        u64::try_from(source_bytes)
            .unwrap_or(u64::MAX)
            .saturating_mul(FALLBACK_STEPS_PER_SOURCE_BYTE),
    )
}

fn specified_outside_capture_end(result: &MatchResult, captures: &CaptureSpec) -> usize {
    if result.start == result.end {
        return result.end;
    }
    captures
        .entries
        .iter()
        .filter(|(_, entry)| entry.name.is_some() || !entry.patterns.is_empty())
        .filter_map(|(group, _)| {
            result
                .capture(*group as usize)
                .filter(|range| range.start >= result.end)
                .map(|range| range.end)
        })
        .fold(result.end, usize::max)
}

fn plain_compact_tokens(parse_text: &str, stack: ScopeStackId) -> Vec<CompactScopedToken> {
    if parse_text.is_empty() {
        Vec::new()
    } else {
        vec![CompactScopedToken {
            range: 0..parse_text.len(),
            stack,
        }]
    }
}

fn push_segment(
    segments: &mut Vec<SyntaxSegment>,
    start: usize,
    end: usize,
    class: Option<SyntaxClass>,
    scope_stack: ScopeStackRef,
) {
    if start >= end {
        return;
    }
    if let Some(last) = segments.last_mut()
        && last.class == class
        && last.scope_stack == scope_stack
        && last.byte_end == start
    {
        last.byte_end = end;
        return;
    }
    segments.push(SyntaxSegment::new(start, end, class).with_scope_stack(scope_stack));
}

fn clamp_range(range: Range<usize>, parent: Range<usize>) -> Range<usize> {
    range.start.max(parent.start)..range.end.min(parent.end)
}

fn compile_injection_selectors(
    grammars: &GrammarSet,
    root: GrammarId,
) -> Vec<CompiledInjectionSelector> {
    // vscode-textmate has two separate injection sources:
    //
    // * the root grammar's `injections` map; and
    // * standalone grammars registered for the root scope through `injectTo`
    //   and `injectionSelector`, whose ordinary top-level patterns are used.
    //
    // Inline injections on include dependencies are not global registrations.
    // Treating them as such makes unrelated embedded grammars preempt the root
    // (notably the large dependency sets used by Astro and Svelte).
    let Some(root_grammar) = grammars.grammar(root) else {
        return Vec::new();
    };
    let mut compiled = root_grammar
        .injections
        .iter()
        .map(|injection| CompiledInjectionSelector {
            grammar_id: root_grammar.id,
            priority: injection.priority,
            patterns: injection.patterns.clone(),
            selector_tokens: tokenize_selector(&injection.selector_body).into(),
        })
        .collect::<Vec<_>>();

    for grammar in grammars.grammars() {
        if grammar.id == root
            || !grammar
                .metadata
                .inject_to
                .iter()
                .any(|scope| scope == &root_grammar.scope_name)
        {
            continue;
        }
        let Some(selector) = grammar.metadata.injection_selector.as_deref() else {
            continue;
        };
        compiled.extend(normalize_injection_selectors(selector).into_iter().map(
            |(priority, selector_body)| CompiledInjectionSelector {
                grammar_id: grammar.id,
                priority,
                patterns: grammar.top_level.clone(),
                selector_tokens: tokenize_selector(&selector_body).into(),
            },
        ));
    }
    compiled
}

#[cfg(test)]
fn selector_matches(selector: &str, stack: &[String]) -> bool {
    let tokens = tokenize_selector(selector);
    selector_tokens_match(&tokens, stack)
}

fn selector_tokens_match<T: AsRef<str>>(tokens: &[SelectorToken], stack: &[T]) -> bool {
    if tokens.is_empty() {
        return false;
    }
    let mut parser = SelectorParser {
        tokens,
        index: 0,
        stack,
    };
    parser.parse_expression()
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SelectorToken {
    Word(String),
    LeftParen,
    RightParen,
    Or,
    And,
    Not,
}

fn tokenize_selector(selector: &str) -> Vec<SelectorToken> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    let flush_word = |word: &mut String, tokens: &mut Vec<SelectorToken>| {
        if !word.is_empty() {
            let word = std::mem::take(word);
            // Whitespace between scope identifiers is the descendant-path
            // operator, not an unordered boolean AND. Keep the whole path in
            // one primary so `meta source` does not match a stack where
            // `source` is an ancestor of `meta`.
            if let Some(SelectorToken::Word(path)) = tokens.last_mut() {
                path.push(' ');
                path.push_str(&word);
            } else {
                tokens.push(SelectorToken::Word(word));
            }
        }
    };
    for ch in selector.chars() {
        match ch {
            '(' => {
                flush_word(&mut word, &mut tokens);
                tokens.push(SelectorToken::LeftParen);
            }
            ')' => {
                flush_word(&mut word, &mut tokens);
                tokens.push(SelectorToken::RightParen);
            }
            ',' | '|' => {
                flush_word(&mut word, &mut tokens);
                tokens.push(SelectorToken::Or);
            }
            '&' => {
                flush_word(&mut word, &mut tokens);
                tokens.push(SelectorToken::And);
            }
            '-' if word.is_empty() => {
                flush_word(&mut word, &mut tokens);
                tokens.push(SelectorToken::Not);
            }
            ch if ch.is_whitespace() => flush_word(&mut word, &mut tokens),
            ch => word.push(ch),
        }
    }
    flush_word(&mut word, &mut tokens);
    tokens
}

struct SelectorParser<'a, T> {
    tokens: &'a [SelectorToken],
    index: usize,
    stack: &'a [T],
}

impl<T: AsRef<str>> SelectorParser<'_, T> {
    fn parse_expression(&mut self) -> bool {
        self.parse_or()
    }

    fn parse_or(&mut self) -> bool {
        let mut value = self.parse_and();
        while self.consume_or() {
            value |= self.parse_and();
        }
        value
    }

    fn parse_and(&mut self) -> bool {
        let mut saw_term = false;
        let mut value = true;
        while self.index < self.tokens.len() {
            if matches!(self.tokens[self.index], SelectorToken::And) {
                self.index += 1;
                continue;
            }
            if matches!(
                self.tokens[self.index],
                SelectorToken::Or | SelectorToken::RightParen
            ) {
                break;
            }
            saw_term = true;
            value &= self.parse_unary();
        }
        saw_term && value
    }

    fn parse_unary(&mut self) -> bool {
        if matches!(self.tokens.get(self.index), Some(SelectorToken::Not)) {
            self.index += 1;
            return !self.parse_unary();
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> bool {
        match self.tokens.get(self.index) {
            Some(SelectorToken::Word(word)) => {
                self.index += 1;
                scope_path_matches(word, self.stack)
            }
            Some(SelectorToken::LeftParen) => {
                self.index += 1;
                let value = self.parse_expression();
                if matches!(self.tokens.get(self.index), Some(SelectorToken::RightParen)) {
                    self.index += 1;
                }
                value
            }
            Some(SelectorToken::RightParen | SelectorToken::Or | SelectorToken::And) | None => {
                false
            }
            Some(SelectorToken::Not) => unreachable!("parse_unary handles negation"),
        }
    }

    fn consume_or(&mut self) -> bool {
        if matches!(self.tokens.get(self.index), Some(SelectorToken::Or)) {
            self.index += 1;
            true
        } else {
            false
        }
    }
}

fn scope_path_matches<T: AsRef<str>>(path: &str, stack: &[T]) -> bool {
    let mut next_index = 0usize;
    for component in path.split_whitespace() {
        let Some(index) = stack[next_index..]
            .iter()
            .position(|scope| scope_component_matches(component, scope.as_ref()))
        else {
            return false;
        };
        next_index += index + 1;
    }
    true
}

fn scope_component_matches(component: &str, scope: &str) -> bool {
    if component.contains('*') {
        return wildcard_scope_component_matches(component, scope);
    }
    scope == component
        || scope
            .strip_prefix(component)
            .is_some_and(|rest| rest.starts_with('.'))
}

fn wildcard_scope_component_matches(component: &str, scope: &str) -> bool {
    let component_parts = component.split('.').collect::<Vec<_>>();
    let scope_parts = scope.split('.').collect::<Vec<_>>();
    if component_parts.len() > scope_parts.len() {
        return false;
    }
    component_parts
        .iter()
        .zip(scope_parts.iter())
        .all(|(component, scope)| *component == "*" || component == scope)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn continuation_frame(rule: u32) -> Frame {
        Frame {
            grammar_id: GrammarId(1),
            base_grammar_id: GrammarId(2),
            rule_id: RuleId(rule),
            scope_prefix: Some(Arc::from(format!("meta.prefix.{rule}"))),
            name: Some(Arc::from(format!("meta.name.{rule}"))),
            content_name: None,
            end_pattern: Some(Arc::from(format!("end-{rule}"))),
            end_pattern_id: Some(PatternId(rule)),
            while_pattern: None,
            while_pattern_id: None,
            end_captures: Arc::new(CaptureSpec::default()),
            while_captures: Arc::new(CaptureSpec::default()),
            patterns: Arc::from([]),
            apply_end_pattern_last: rule.is_multiple_of(2),
            begin_captured_eol: false,
            identity_hash: 0,
            state_hash: 0,
            interned_stack_id: InternedFrameStackId::default(),
        }
    }

    #[test]
    fn prepared_language_shares_immutable_root_work_but_not_mutable_caches() {
        let mut grammars = GrammarSet::new();
        let root = grammars
            .load_and_add(
                r#"{"scopeName":"source.prepared","patterns":[{"match":"true","name":"constant.language.prepared"}]}"#,
            )
            .unwrap();
        let prepared = PreparedLanguage::new(grammars, root);
        assert_eq!(prepared.static_pattern_capacity(), 1);
        assert_eq!(prepared.compiled_pattern_count(), 1);

        let mut first = prepared.tokenizer();
        let second = prepared.tokenizer();
        let first_root = first.candidate_cache.get(&StateId(0)).unwrap();
        let second_root = second.candidate_cache.get(&StateId(0)).unwrap();
        assert!(Arc::ptr_eq(first_root, second_root));
        assert!(Arc::ptr_eq(
            &first_root.matchers[0],
            &second_root.matchers[0]
        ));

        first.clear_candidate_cache();
        assert_eq!(first.candidate_cache_len(), 0);
        assert_eq!(second.candidate_cache_len(), 1);
    }

    #[test]
    fn prepared_pattern_slots_are_scoped_to_the_root_grammar_closure() {
        let mut grammars = GrammarSet::new();
        let root = grammars
            .load_and_add(
                r#"{
                    "scopeName":"source.prepared-closure",
                    "patterns":[
                        {"match":"root"},
                        {"include":"source.prepared-dependency"}
                    ]
                }"#,
            )
            .unwrap();
        grammars
            .load_and_add(
                r#"{
                    "scopeName":"source.prepared-dependency",
                    "patterns":[{"match":"dependency"}]
                }"#,
            )
            .unwrap();
        grammars
            .load_and_add(
                r#"{
                    "scopeName":"source.prepared-injection",
                    "injectionSelector":"L:source.prepared-closure",
                    "injectTo":["source.prepared-closure"],
                    "patterns":[{"match":"injected"}]
                }"#,
            )
            .unwrap();
        grammars
            .load_and_add(
                r#"{
                    "scopeName":"source.prepared-unrelated",
                    "patterns":[{"match":"unused-one"},{"match":"unused-two"}]
                }"#,
            )
            .unwrap();

        let prepared = PreparedLanguage::new(grammars, root);
        assert_eq!(prepared.grammar_count(), 3);
        assert_eq!(prepared.static_pattern_capacity(), 3);
        assert!(
            prepared
                .tokenizer()
                .grammars()
                .grammar_by_scope("source.prepared-unrelated")
                .is_none(),
            "preparation retained an unrelated registry grammar"
        );
    }

    #[test]
    fn prepared_closure_ignores_unreachable_repository_dependencies() {
        let root_grammar = r#"{
            "scopeName":"source.prepared-unused-root",
            "patterns":[{"match":"x","name":"constant.prepared-unused-root"}],
            "repository":{
                "unused":{
                    "patterns":[{"include":"source.prepared-unused-large"}]
                }
            }
        }"#;
        let unused_grammar = r#"{
            "scopeName":"source.prepared-unused-large",
            "patterns":[{"match":"unused","name":"constant.prepared-unused-large"}]
        }"#;
        let mut grammars = GrammarSet::new();
        let root = grammars.load_and_add(root_grammar).unwrap();
        grammars.load_and_add(unused_grammar).unwrap();

        let prepared = PreparedLanguage::new(grammars, root);
        assert_eq!(prepared.grammar_count(), 1);
        assert!(
            prepared
                .tokenizer()
                .grammars()
                .grammar_by_scope("source.prepared-unused-large")
                .is_none()
        );
    }

    #[test]
    fn prepared_closure_follows_inline_capture_base_dependencies() {
        let root_grammar = r#"{
            "scopeName":"source.prepared-base-root",
            "patterns":[{"include":"source.prepared-base-a#entry"}]
        }"#;
        let grammar_a = r#"{
            "scopeName":"source.prepared-base-a",
            "patterns":[{"include":"source.prepared-base-b"}],
            "repository":{
                "entry":{
                    "match":"x",
                    "captures":{"0":{"patterns":[{"include":"$base"}]}}
                }
            }
        }"#;
        let grammar_b = r#"{
            "scopeName":"source.prepared-base-b",
            "patterns":[{"match":"x","name":"constant.prepared-base-b"}]
        }"#;
        let mut grammars = GrammarSet::new();
        grammars
            .load_and_add(
                r#"{"scopeName":"source.prepared-base-unrelated","patterns":[{"match":"unused"}]}"#,
            )
            .unwrap();
        let root = grammars.load_and_add(root_grammar).unwrap();
        grammars.load_and_add(grammar_a).unwrap();
        grammars.load_and_add(grammar_b).unwrap();

        let mut direct = TextMateTokenizer::new(grammars.clone(), root);
        let direct_line = direct.tokenize_line_scopes("x", TokenizerState::default());
        assert!(line_has_scope(&direct_line, "constant.prepared-base-b"));

        let prepared = PreparedLanguage::new(grammars, root);
        assert_eq!(prepared.grammar_count(), 3);
        let mut tokenizer = prepared.tokenizer();
        let prepared_line = tokenizer.tokenize_line_scopes("x", TokenizerState::default());
        assert!(line_has_scope(&prepared_line, "constant.prepared-base-b"));
    }

    #[test]
    fn prepared_closure_follows_begin_while_inline_base() {
        let root_grammar = r#"{
            "scopeName":"source.prepared-while-root",
            "patterns":[{"include":"source.prepared-while-a#entry"}]
        }"#;
        let grammar_a = r#"{
            "scopeName":"source.prepared-while-a",
            "patterns":[{"include":"source.prepared-while-b"}],
            "repository":{
                "entry":{
                    "begin":">",
                    "while":"z",
                    "contentName":"meta.prepared-while-content",
                    "patterns":[{"include":"$base"}]
                }
            }
        }"#;
        let grammar_b = r#"{
            "scopeName":"source.prepared-while-b",
            "patterns":[{"match":">","name":"constant.prepared-while-b"}]
        }"#;
        let mut grammars = GrammarSet::new();
        let root = grammars.load_and_add(root_grammar).unwrap();
        grammars.load_and_add(grammar_a).unwrap();
        grammars.load_and_add(grammar_b).unwrap();

        let mut direct = TextMateTokenizer::new(grammars.clone(), root);
        let direct_line = direct.tokenize_line_scopes(">", TokenizerState::default());
        assert!(line_has_scope(&direct_line, "constant.prepared-while-b"));

        let prepared = PreparedLanguage::new(grammars, root);
        assert_eq!(prepared.grammar_count(), 3);
        let mut tokenizer = prepared.tokenizer();
        let line = tokenizer.tokenize_line_scopes(">", TokenizerState::default());
        assert!(line_has_scope(&line, "constant.prepared-while-b"));
    }

    #[test]
    fn prepared_closure_retains_external_forwarding_grammars() {
        let root_grammar = r#"{
            "scopeName":"source.prepared-forward-root",
            "patterns":[{"include":"source.prepared-forward-wrapper"}]
        }"#;
        let wrapper_grammar = r#"{
            "scopeName":"source.prepared-forward-wrapper",
            "patterns":[{"include":"source.prepared-forward-leaf"}]
        }"#;
        let leaf_grammar = r#"{
            "scopeName":"source.prepared-forward-leaf",
            "patterns":[{"match":"x","name":"constant.prepared-forward-leaf"}]
        }"#;
        let mut grammars = GrammarSet::new();
        let root = grammars.load_and_add(root_grammar).unwrap();
        grammars.load_and_add(wrapper_grammar).unwrap();
        grammars.load_and_add(leaf_grammar).unwrap();

        let prepared = PreparedLanguage::new(grammars, root);
        assert_eq!(prepared.grammar_count(), 3);
        let mut tokenizer = prepared.tokenizer();
        let line = tokenizer.tokenize_line_scopes("x", TokenizerState::default());
        assert!(line_has_scope(&line, "constant.prepared-forward-leaf"));
    }

    #[test]
    fn prepared_closure_retains_inline_injection_base_dependencies() {
        let root_grammar = r#"{
            "scopeName":"source.prepared-injection-base-root",
            "patterns":[{"include":"source.prepared-injection-base-a#entry"}],
            "injections":{
                "L:meta.inner.prepared-injection-base":{
                    "patterns":[{"include":"$base"}]
                }
            }
        }"#;
        let grammar_a = r#"{
            "scopeName":"source.prepared-injection-base-a",
            "patterns":[{"include":"source.prepared-injection-base-b"}],
            "repository":{
                "entry":{
                    "match":"x(\\[foo\\])",
                    "captures":{
                        "1":{
                            "patterns":[{
                                "begin":"\\[",
                                "end":"\\]",
                                "name":"meta.inner.prepared-injection-base"
                            }]
                        }
                    }
                }
            }
        }"#;
        let grammar_b = r#"{
            "scopeName":"source.prepared-injection-base-b",
            "patterns":[{"match":"foo","name":"constant.prepared-injection-base-b"}]
        }"#;
        let mut grammars = GrammarSet::new();
        let root = grammars.load_and_add(root_grammar).unwrap();
        grammars.load_and_add(grammar_a).unwrap();
        grammars.load_and_add(grammar_b).unwrap();

        let mut direct = TextMateTokenizer::new(grammars.clone(), root);
        let direct_line = direct.tokenize_line_scopes("x[foo]", TokenizerState::default());
        assert!(line_has_scope(
            &direct_line,
            "constant.prepared-injection-base-b"
        ));

        let prepared = PreparedLanguage::new(grammars, root);
        assert_eq!(prepared.grammar_count(), 3);
        let mut tokenizer = prepared.tokenizer();
        let prepared_line = tokenizer.tokenize_line_scopes("x[foo]", TokenizerState::default());
        assert!(line_has_scope(
            &prepared_line,
            "constant.prepared-injection-base-b"
        ));
    }

    #[test]
    fn prepared_pattern_slots_fall_back_safely_for_distinct_capture_layouts() {
        let mut grammars = GrammarSet::new();
        let root = grammars
            .load_and_add(r#"{"scopeName":"source.capture-layout","patterns":[{"match":"(a)"}]}"#)
            .unwrap();
        let cache = PreparedPatternCache::new(&grammars, &[true]);
        let first = cache
            .get_or_compile(root, PatternId(0), "(a)", Some(vec![0]))
            .0;
        let second = cache
            .get_or_compile(root, PatternId(0), "(a)", Some(vec![1]))
            .0;

        assert!(!Arc::ptr_eq(&first, &second));
        assert!(first.has_live_captures(Some(&[0])));
        assert!(second.has_live_captures(Some(&[1])));
        assert_eq!(cache.initialized_count(), 1);
    }

    #[test]
    fn prepared_pattern_cache_rejects_values_over_the_remaining_byte_budget() {
        let mut grammars = GrammarSet::new();
        let root = grammars
            .load_and_add(r#"{"scopeName":"source.oversized-pattern","patterns":[{"match":"a"}]}"#)
            .unwrap();
        let cache = PreparedPatternCache::new(&grammars, &[true]);
        cache
            .retained_bytes
            .store(MAX_PREPARED_PATTERN_BYTES, Ordering::Release);
        let (matcher, compiled_now, retained) = cache.get_or_compile(root, PatternId(0), "a", None);

        assert!(compiled_now);
        assert!(!retained);
        assert_eq!(matcher.source(), "a");
        assert_eq!(cache.initialized_count(), 0);
        assert_eq!(cache.retained_bytes(), MAX_PREPARED_PATTERN_BYTES);
    }

    #[test]
    fn rejected_prepared_patterns_do_not_leak_through_root_blueprints() {
        let mut grammars = GrammarSet::new();
        let root = grammars
            .load_and_add(
                r#"{"scopeName":"source.rejected-root-pattern","patterns":[{"match":"a"}]}"#,
            )
            .unwrap();
        let patterns = Arc::new(PreparedPatternCache::new(&grammars, &[true]));
        patterns
            .retained_bytes
            .store(MAX_PREPARED_PATTERN_BYTES, Ordering::Release);
        let blueprints = Arc::new(PreparedBlueprintCache::default());
        let mut tokenizer = TextMateTokenizer::new_with_prepared_caches(
            grammars,
            root,
            Arc::clone(&patterns),
            Arc::clone(&blueprints),
            Vec::new(),
            Arc::new(RuleRepositoryContexts::new(1)),
        );

        tokenizer.prepare_root_candidate();

        assert_eq!(patterns.initialized_count(), 0);
        assert_eq!(blueprints.len(), 0);
        assert!(tokenizer.matcher_cache.is_empty());
        assert_eq!(tokenizer.candidate_cache_len(), 0);
    }

    #[test]
    fn concurrent_prepared_pattern_use_compiles_one_canonical_value() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let mut grammars = GrammarSet::new();
        let root = grammars
            .load_and_add(
                r#"{"scopeName":"source.concurrent-pattern","patterns":[{"match":"(a)"}]}"#,
            )
            .unwrap();
        let cache = Arc::new(PreparedPatternCache::new(&grammars, &[true]));
        let barrier = Arc::new(std::sync::Barrier::new(8));
        let compiled = AtomicUsize::new(0);
        let matchers = std::thread::scope(|scope| {
            let threads = (0..8)
                .map(|_| {
                    let cache = Arc::clone(&cache);
                    let barrier = Arc::clone(&barrier);
                    let compiled = &compiled;
                    scope.spawn(move || {
                        barrier.wait();
                        let (matcher, compiled_now, retained) =
                            cache.get_or_compile(root, PatternId(0), "(a)", Some(vec![0]));
                        assert!(retained);
                        compiled.fetch_add(compiled_now as usize, Ordering::Relaxed);
                        matcher
                    })
                })
                .collect::<Vec<_>>();
            threads
                .into_iter()
                .map(|thread| thread.join().unwrap())
                .collect::<Vec<_>>()
        });

        assert_eq!(compiled.load(Ordering::Relaxed), 1);
        assert!(
            matchers
                .iter()
                .skip(1)
                .all(|matcher| Arc::ptr_eq(&matchers[0], matcher))
        );
    }

    #[test]
    fn concurrent_prepared_blueprint_use_runs_one_builder_per_key() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let cache = Arc::new(PreparedBlueprintCache::default());
        let key = PreparedBlueprintKey {
            source: CandidateSourceKey::Root(GrammarId(0)),
            injection_outcome: PreparedInjectionOutcomeId(0),
        };
        let barrier = Arc::new(std::sync::Barrier::new(8));
        let builds = AtomicUsize::new(0);
        let blueprints = std::thread::scope(|scope| {
            let threads = (0..8)
                .map(|_| {
                    let cache = Arc::clone(&cache);
                    let key = key.clone();
                    let barrier = Arc::clone(&barrier);
                    let builds = &builds;
                    scope.spawn(move || {
                        barrier.wait();
                        cache.get_or_insert_with(key, || {
                            builds.fetch_add(1, Ordering::Relaxed);
                            (
                                Arc::new(CandidateBlueprint {
                                    candidates: Vec::new(),
                                    matchers: Arc::from([]),
                                    pattern_set_search: None,
                                }),
                                true,
                            )
                        })
                    })
                })
                .collect::<Vec<_>>();
            threads
                .into_iter()
                .map(|thread| thread.join().unwrap())
                .collect::<Vec<_>>()
        });

        assert_eq!(builds.load(Ordering::Relaxed), 1);
        assert_eq!(cache.len(), 1);
        assert!(
            blueprints
                .iter()
                .skip(1)
                .all(|blueprint| Arc::ptr_eq(&blueprints[0], blueprint))
        );
    }

    #[test]
    fn prepared_blueprint_payload_builds_are_serialized() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let cache = Arc::new(PreparedBlueprintCache::default());
        let barrier = Arc::new(std::sync::Barrier::new(4));
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for grammar in 0..4 {
                let cache = Arc::clone(&cache);
                let barrier = Arc::clone(&barrier);
                let active = &active;
                let peak = &peak;
                scope.spawn(move || {
                    let key = PreparedBlueprintKey {
                        source: CandidateSourceKey::Root(GrammarId(grammar)),
                        injection_outcome: PreparedInjectionOutcomeId(0),
                    };
                    barrier.wait();
                    cache.get_or_insert_with(key, || {
                        let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(current, Ordering::SeqCst);
                        std::thread::sleep(std::time::Duration::from_millis(5));
                        active.fetch_sub(1, Ordering::SeqCst);
                        (
                            Arc::new(CandidateBlueprint {
                                candidates: Vec::new(),
                                matchers: Arc::from([]),
                                pattern_set_search: None,
                            }),
                            true,
                        )
                    });
                });
            }
        });

        assert_eq!(peak.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn prepared_blueprint_keys_intern_injection_outcomes_once() {
        let cache = PreparedBlueprintCache::default();
        let outcome = InjectionOutcome {
            left: vec![InjectionCandidate {
                grammar_id: GrammarId(1),
                patterns: vec![RuleRef::Repository("shared-injection".repeat(128))],
            }],
            right: Vec::new(),
        };

        let first = cache.intern_injection_outcome(&outcome).unwrap();
        let second = cache.intern_injection_outcome(&outcome).unwrap();
        assert_eq!(first, second);
        let state = cache
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        assert_eq!(state.injection_outcome_ids.len(), 1);
        assert!(state.injection_outcome_bytes <= MAX_PREPARED_INJECTION_OUTCOME_BYTES);
        let outcome_bytes = state.injection_outcome_bytes;
        drop(state);
        assert!(cache.retained_bytes() >= outcome_bytes);
        assert!(cache.retained_bytes() <= MAX_PREPARED_CANDIDATE_BYTES);
    }

    #[test]
    fn prepared_blueprint_cache_rejects_oversized_values() {
        let cache = PreparedBlueprintCache::default();
        let key = PreparedBlueprintKey {
            source: CandidateSourceKey::Root(GrammarId(0)),
            injection_outcome: PreparedInjectionOutcomeId(0),
        };
        let blueprint = Arc::new(CandidateBlueprint {
            candidates: vec![Candidate {
                order: 0,
                base_grammar_id: GrammarId(0),
                pattern: "x".repeat(MAX_PREPARED_BLUEPRINT_BYTES),
                pattern_id: None,
                scope_prefix: None,
                kind: CandidateKind::Match {
                    grammar_id: GrammarId(0),
                    name: None,
                    name_template: None,
                    captures: Arc::new(CaptureSpec::default()),
                },
            }],
            matchers: Arc::from([]),
            pattern_set_search: None,
        });

        let returned = cache.get_or_insert_with(key, || (Arc::clone(&blueprint), true));
        assert!(Arc::ptr_eq(&returned, &blueprint));
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn prepared_blueprint_charge_includes_pattern_set_scanner() {
        let pattern = Arc::new(CompiledPattern::new("(a){4000}"));
        let matchers: Arc<[Arc<CompiledPattern>]> =
            Arc::from([Arc::clone(&pattern), Arc::clone(&pattern)]);
        let pattern_set_search = PatternSetMatcher::from_shared_compiled(Arc::clone(&matchers));
        let scanner_bytes = pattern_set_search.retained_heap_bytes();
        let blueprint = CandidateBlueprint {
            candidates: Vec::new(),
            matchers,
            pattern_set_search: Some(pattern_set_search),
        };

        assert!(scanner_bytes > 100_000);
        assert!(candidate_blueprint_retained_bytes(&blueprint) >= scanner_bytes);
    }

    #[test]
    fn prepared_language_reuses_inline_candidate_blueprints() {
        let mut grammars = GrammarSet::new();
        let root = grammars
            .load_and_add(
                r#"{
                    "scopeName":"source.prepared-inline",
                    "patterns":[{
                        "match":"(x)",
                        "captures":{
                            "1":{"patterns":[
                                {"match":"x","name":"constant.prepared-inline"},
                                {"match":"y"}
                            ]}
                        }
                    }]
                }"#,
            )
            .unwrap();
        let prepared = PreparedLanguage::new(grammars, root);
        let mut first = prepared.tokenizer();
        first.tokenize_source("x");
        let first_inline = first
            .inline_candidate_cache
            .values()
            .find_map(|set| set.blueprint.shared_blueprint().map(Arc::clone))
            .expect("first tokenizer bound a shared inline blueprint");

        let mut second = prepared.tokenizer();
        second.tokenize_source("x");
        let second_inline = second
            .inline_candidate_cache
            .values()
            .find_map(|set| set.blueprint.shared_blueprint().map(Arc::clone))
            .expect("second tokenizer bound a shared inline blueprint");
        assert!(Arc::ptr_eq(&first_inline, &second_inline));
    }

    #[test]
    fn prepared_language_reuses_lazily_discovered_static_blueprints() {
        let mut grammars = GrammarSet::new();
        let root = grammars
            .load_and_add(
                r#"{
                    "scopeName":"source.prepared-nested",
                    "patterns":[{
                        "begin":"\"",
                        "end":"\"",
                        "name":"string.prepared",
                        "patterns":[{"match":"[a-z]+","name":"word.prepared"}]
                    }]
                }"#,
            )
            .unwrap();
        let prepared = PreparedLanguage::new(grammars, root);
        assert_eq!(prepared.static_blueprint_count(), 1);

        let mut first = prepared.tokenizer();
        first.tokenize_source("\"word\"");
        assert!(prepared.static_blueprint_count() >= 2);
        let first_nested = first
            .candidate_blueprint_cache
            .iter()
            .find_map(|(key, value)| {
                matches!(key.source, CandidateSourceKey::Frame { .. })
                    .then(|| value.shared_blueprint().map(Arc::clone))
                    .flatten()
            })
            .expect("first tokenizer built nested static blueprint");

        let mut second = prepared.tokenizer();
        second.tokenize_source("\"word\"");
        let second_nested = second
            .candidate_blueprint_cache
            .iter()
            .find_map(|(key, value)| {
                matches!(key.source, CandidateSourceKey::Frame { .. })
                    .then(|| value.shared_blueprint().map(Arc::clone))
                    .flatten()
            })
            .expect("second tokenizer bound nested static blueprint");
        assert!(Arc::ptr_eq(&first_nested, &second_nested));
        assert!(prepared.static_blueprint_count() <= MAX_CANDIDATE_BLUEPRINTS);
    }

    #[test]
    fn parent_linked_frame_stack_preserves_prefixes_hashes_and_exact_equality() {
        let mut state = TokenizerState::default();
        let mut independently_built = TokenizerState::default();
        let mut interner = FrameStackInternTable::new();
        let mut expected_state_hash = 0x811c9dc5u32;
        for rule in 0..300 {
            let frame = continuation_frame(rule);
            let identity_hash = frame.compute_identity_hash();
            expected_state_hash = fnv_mix(
                expected_state_hash,
                (identity_hash ^ (identity_hash >> 32)) as u32,
            );
            state.push_frame(frame, &mut interner);
            independently_built.push_frame(continuation_frame(rule), &mut interner);
        }
        assert_eq!(state.depth(), 300);
        assert_eq!(state.state_id(), StateId(expected_state_hash));
        assert_eq!(state, independently_built);
        assert_eq!(
            state
                .frames
                .iter()
                .map(|frame| frame.rule_id.0)
                .collect::<Vec<_>>(),
            (0..300).collect::<Vec<_>>()
        );

        let prefix = state.prefix(33);
        assert_eq!(prefix.depth(), 33);
        assert_eq!(prefix.frames.last().unwrap().rule_id, RuleId(32));
        let mut changed = state.clone();
        changed.truncate_frames(31);
        changed.push_frame(continuation_frame(500), &mut interner);
        assert_eq!(changed.depth(), 32);
        assert_eq!(changed.frames.last().unwrap().rule_id, RuleId(500));
        assert_eq!(state.depth(), 300, "persistent ancestor was mutated");
        assert_ne!(changed, state);
    }

    #[test]
    fn tokenizes_placeholder_line_without_copying_text() {
        let mut tokenizer = Tokenizer::new();
        let tokens = tokenizer.tokenize_line("let π = 1;", StateId(7));
        assert_eq!(tokens.exit, StateId(7));
        assert_eq!(tokens.tokens[0].0, 0..11);
    }

    #[test]
    fn zero_width_advance_stays_on_char_boundary() {
        assert_eq!(advance_zero_width("π", &(0..0)), 2);
    }

    #[test]
    fn json_string_smoke_matches_migration_worked_example() {
        let spans = tokenize_json_string_smoke(r#""a\n""#);
        assert_eq!(
            spans,
            vec![
                ScopeSpan {
                    range: 0..1,
                    scope: "punctuation.definition.string.begin.json",
                },
                ScopeSpan {
                    range: 1..2,
                    scope: "string.quoted.double.json",
                },
                ScopeSpan {
                    range: 2..4,
                    scope: "constant.character.escape.json",
                },
                ScopeSpan {
                    range: 4..5,
                    scope: "punctuation.definition.string.end.json",
                },
            ]
        );
    }

    #[test]
    fn text_start_anchor_only_matches_document_first_line() {
        let grammar = r##"{
            "scopeName": "source.anchor-a",
            "patterns": [
                {"match":"\\Afoo", "name":"keyword.anchor-a"},
                {"match":"foo", "name":"identifier.anchor-a"}
            ]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        let first = tokenizer.tokenize_line_scopes_at_line("foo", TokenizerState::default(), 0);
        let second = tokenizer.tokenize_line_scopes_at_line("foo", TokenizerState::default(), 1);

        assert!(line_has_scope(&first, "keyword.anchor-a"), "{first:#?}");
        assert!(!line_has_scope(&second, "keyword.anchor-a"), "{second:#?}");
        assert!(
            line_has_scope(&second, "identifier.anchor-a"),
            "{second:#?}"
        );
    }

    #[test]
    fn continuation_anchor_is_invalid_at_fresh_line_start() {
        let grammar = r##"{
            "scopeName": "source.anchor-g",
            "patterns": [
                {"match":"\\Gfoo", "name":"keyword.anchor-g"},
                {"match":"foo", "name":"identifier.anchor-g"}
            ]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        let line = tokenizer.tokenize_line_scopes_at_line("foo", TokenizerState::default(), 0);

        assert!(!line_has_scope(&line, "keyword.anchor-g"), "{line:#?}");
        assert!(line_has_scope(&line, "identifier.anchor-g"), "{line:#?}");
    }

    #[test]
    fn tokenizes_json_with_real_grammar() {
        let mut tokenizer = TextMateTokenizer::from_grammar(include_str!(
            "../../assets/grammars/languages/json.tmLanguage.json"
        ))
        .unwrap();
        let line = tokenizer.tokenize_line_scopes("{\"ok\": true}", TokenizerState::default());
        assert!(line.tokens.iter().any(|token| token.scopes.len() > 1));
        assert!(line.tokens.iter().any(|token| {
            token
                .scopes
                .iter()
                .any(|scope| scope.contains("constant.language.json"))
        }));
    }

    #[test]
    fn opt_in_counters_record_line_and_regex_attempts() {
        let grammar = r##"{
            "scopeName": "source.counters",
            "patterns": [{"match":"x", "name":"keyword.counter"}]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        assert_eq!(tokenizer.counters(), EngineCounters::default());

        tokenizer.tokenize_line_scopes("x", TokenizerState::default());
        assert_eq!(tokenizer.counters(), EngineCounters::default());

        tokenizer.set_counters_enabled(true);
        tokenizer.set_hot_counters_enabled(true);
        tokenizer.tokenize_line_scopes("x", TokenizerState::default());
        let counters = tokenizer.counters();
        assert_eq!(counters.lines_tokenized, 1);
        assert!(counters.regex_dfa_attempts > 0, "{counters:#?}");
        assert_eq!(counters.pattern_hotspots.len(), 1, "{counters:#?}");
        assert_eq!(counters.pattern_hotspots[0].root_scope, "source.counters");
        assert_eq!(counters.pattern_hotspots[0].pattern, "x");
        assert_eq!(counters.pattern_hotspots[0].engine, "dfa");
        assert_eq!(counters.pattern_hotspots[0].attempts, 1);
        assert_eq!(counters.pattern_hotspots[0].matches, 1);

        let taken = tokenizer.take_counters();
        assert_eq!(taken.lines_tokenized, 1);
        assert_eq!(taken.pattern_hotspots.len(), 1, "{taken:#?}");
        assert_eq!(tokenizer.counters(), EngineCounters::default());
    }

    #[test]
    fn counters_record_fallback_budget_kills_as_degraded_lines() {
        let grammar = r##"{
            "scopeName": "source.counter-budget",
            "patterns": [
                {"match":"(?=(a+)+b)(a+)+b", "name":"invalid.counter-budget"},
                {"match":"ok", "name":"keyword.counter-budget"}
            ]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        tokenizer.set_counters_enabled(true);
        let line = format!("{} ok", "a".repeat(256));
        tokenizer.tokenize_line_scopes(&line, TokenizerState::default());

        let counters = tokenizer.counters();
        assert!(counters.regex_fallback_attempts > 0, "{counters:#?}");
        assert!(counters.fallback_steps_total > 0, "{counters:#?}");
        assert!(counters.fallback_budget_kills > 0, "{counters:#?}");
        assert_eq!(counters.degraded_lines, 1, "{counters:#?}");
    }

    #[test]
    fn state_interner_assigns_stable_ids_across_replay() {
        let grammar = r##"{
            "scopeName": "source.state-counter",
            "patterns": [{"begin":"/\\*", "end":"\\*/", "name":"comment.block.state-counter"}]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        assert_eq!(tokenizer.interned_state_count(), 1);
        assert_eq!(
            tokenizer.intern_state(&TokenizerState::default()),
            StateId(0)
        );

        let first = tokenizer.tokenize_line_scopes("/* open", TokenizerState::default());
        assert_eq!(first.entry_state_id, StateId(0));
        assert_eq!(tokenizer.intern_state(&first.state), first.exit_state_id);
        assert_eq!(
            tokenizer.state_for_id(first.exit_state_id),
            Some(&first.state)
        );

        let second = tokenizer.tokenize_line_scopes("inside", first.state.clone());
        assert_eq!(second.entry_state_id, first.exit_state_id);

        let replay = tokenizer.tokenize_line_scopes("inside", first.state);
        assert_eq!(replay.entry_state_id, first.exit_state_id);
        assert_eq!(replay.exit_state_id, second.exit_state_id);
    }

    #[test]
    fn counters_record_state_interner_hits_and_misses() {
        let grammar = r##"{
            "scopeName": "source.state-counters",
            "patterns": [{"begin":"/\\*", "end":"\\*/", "name":"comment.block.state-counters"}]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        tokenizer.set_counters_enabled(true);

        let first = tokenizer.tokenize_line_scopes("/* open", TokenizerState::default());
        let after_first = tokenizer.counters();
        assert!(after_first.state_cache_hits >= 1, "{after_first:#?}");
        assert!(after_first.state_cache_misses >= 1, "{after_first:#?}");

        tokenizer.tokenize_line_scopes("inside", first.state);
        let after_second = tokenizer.counters();
        assert!(
            after_second.state_cache_hits > after_first.state_cache_hits,
            "before={after_first:#?} after={after_second:#?}"
        );
    }

    #[test]
    fn line_cache_reuses_same_entry_state_and_line() {
        let grammar = r##"{
            "scopeName": "source.line-cache",
            "patterns": [{"match":"x", "name":"keyword.line-cache"}]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        tokenizer.set_line_cache_capacity(8);
        tokenizer.set_counters_enabled(true);

        let first = tokenizer.tokenize_line_scopes("x", TokenizerState::default());
        let second = tokenizer.tokenize_line_scopes("x", TokenizerState::default());

        assert_eq!(first.tokens, second.tokens);
        assert_eq!(second.entry_state_id, StateId(0));
        assert_eq!(tokenizer.line_cache_len(), 1);
        let counters = tokenizer.counters();
        assert_eq!(counters.line_cache_misses, 1, "{counters:#?}");
        assert_eq!(counters.line_cache_hits, 1, "{counters:#?}");
    }

    #[test]
    fn line_cache_key_includes_entry_state() {
        let grammar = r##"{
            "scopeName": "source.line-cache-state",
            "patterns": [{"begin":"/\\*", "end":"\\*/", "name":"comment.block.line-cache-state"}]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        tokenizer.set_line_cache_capacity(8);
        tokenizer.set_counters_enabled(true);

        let first = tokenizer.tokenize_line_scopes("/* open", TokenizerState::default());
        tokenizer.tokenize_line_scopes("inside", first.state.clone());
        tokenizer.tokenize_line_scopes("inside", first.state);

        let counters = tokenizer.counters();
        assert_eq!(counters.line_cache_misses, 2, "{counters:#?}");
        assert_eq!(counters.line_cache_hits, 1, "{counters:#?}");
    }

    #[test]
    fn line_cache_evicts_oldest_entry() {
        let grammar = r##"{
            "scopeName": "source.line-cache-evict",
            "patterns": [{"match":"x|y", "name":"keyword.line-cache-evict"}]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        tokenizer.set_line_cache_capacity(1);
        tokenizer.set_counters_enabled(true);

        tokenizer.tokenize_line_scopes("x", TokenizerState::default());
        tokenizer.tokenize_line_scopes("y", TokenizerState::default());
        tokenizer.tokenize_line_scopes("x", TokenizerState::default());

        assert_eq!(tokenizer.line_cache_len(), 1);
        let counters = tokenizer.counters();
        assert_eq!(counters.line_cache_hits, 0, "{counters:#?}");
        assert_eq!(counters.line_cache_misses, 3, "{counters:#?}");
        assert_eq!(counters.line_cache_evictions, 2, "{counters:#?}");
    }

    #[test]
    fn checkpoint_viewport_replay_matches_replay_from_zero() {
        let grammar = r##"{
            "scopeName": "source.checkpoint-engine",
            "patterns": [
                {"begin":"/\\*", "end":"\\*/", "name":"comment.block.checkpoint-engine"},
                {"match":"\\b(let|return)\\b", "name":"keyword.control.checkpoint-engine"}
            ]
        }"##;
        let source = [
            "let before = 1;",
            "/* comment starts",
            "still in comment",
            "ends */ let after = 2;",
            "return after;",
        ]
        .join("\n");

        let mut full = TextMateTokenizer::from_grammar(grammar).unwrap();
        let mut state = TokenizerState::default();
        let mut full_lines = Vec::new();
        for (line_index, chunk) in LineChunks::new(&source).enumerate() {
            let tokenized = full.tokenize_line_scopes_at_line(chunk.parse_text, state, line_index);
            state = tokenized.state.clone();
            full_lines.push(tokenized);
        }

        let mut viewport = TextMateTokenizer::from_grammar(grammar).unwrap();
        viewport.set_counters_enabled(true);
        let mut checkpoints = crate::engine::checkpoint::CheckpointTable::new(2);

        let first = viewport.tokenize_viewport_scopes(&source, 0..2, &mut checkpoints);
        assert_eq!(first.len(), 2);
        assert!(
            checkpoints
                .nearest_before(3)
                .is_some_and(|checkpoint| checkpoint.line_index == 2)
        );

        let replayed = viewport.tokenize_viewport_scopes(&source, 3..5, &mut checkpoints);
        assert_eq!(replayed.len(), 2);
        assert_eq!(replayed[0].tokens, full_lines[3].tokens);
        assert_eq!(replayed[1].tokens, full_lines[4].tokens);

        let counters = viewport.counters();
        assert_eq!(counters.checkpoint_replay_lines, 1, "{counters:#?}");
    }

    #[test]
    fn viewport_start_past_eof_does_not_replay_source() {
        let grammar = r#"{
            "scopeName": "source.viewport-eof",
            "patterns": [{"match":"x", "name":"keyword.viewport-eof"}]
        }"#;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        tokenizer.set_counters_enabled(true);
        let mut checkpoints = crate::engine::checkpoint::CheckpointTable::new(2);
        let checkpoints_before = checkpoints.clone();

        let tokenized = tokenizer.tokenize_viewport_scopes("x\ny", 5..6, &mut checkpoints);

        assert!(tokenized.is_empty());
        assert_eq!(checkpoints, checkpoints_before);
        let counters = tokenizer.counters();
        assert_eq!(counters.lines_tokenized, 0, "{counters:#?}");
        assert_eq!(counters.checkpoint_replay_lines, 0, "{counters:#?}");
    }

    #[test]
    fn checkpoint_with_unknown_state_replays_from_zero() {
        let grammar = r##"{
            "scopeName": "source.checkpoint-missing",
            "patterns": [
                {"begin":"/\\*", "end":"\\*/", "name":"comment.block.checkpoint-missing"},
                {"match":"\\breturn\\b", "name":"keyword.control.checkpoint-missing"}
            ]
        }"##;
        let source = ["/* open", "still", "ends */", "return ok;"].join("\n");

        let mut full = TextMateTokenizer::from_grammar(grammar).unwrap();
        let mut state = TokenizerState::default();
        let mut full_lines = Vec::new();
        for (line_index, chunk) in LineChunks::new(&source).enumerate() {
            let tokenized = full.tokenize_line_scopes_at_line(chunk.parse_text, state, line_index);
            state = tokenized.state.clone();
            full_lines.push(tokenized);
        }

        let mut viewport = TextMateTokenizer::from_grammar(grammar).unwrap();
        viewport.set_counters_enabled(true);
        let mut checkpoints = crate::engine::checkpoint::CheckpointTable::new(2);
        checkpoints.record(2, StateId(999));

        let replayed = viewport.tokenize_viewport_scopes(&source, 3..4, &mut checkpoints);
        assert_eq!(replayed[0].tokens, full_lines[3].tokens);
        let counters = viewport.counters();
        assert_eq!(counters.checkpoint_replay_lines, 3, "{counters:#?}");
    }

    #[test]
    fn candidate_cache_reuses_state_across_lines_without_reprobing_within_a_line() {
        let grammar = r##"{
            "scopeName": "source.candidate-cache",
            "patterns": [
                {"match":"x", "name":"keyword.x.candidate-cache"},
                {"match":"y", "name":"keyword.y.candidate-cache"}
            ]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        tokenizer.set_counters_enabled(true);

        tokenizer.tokenize_line_scopes("x y", TokenizerState::default());
        tokenizer.tokenize_line_scopes("x y", TokenizerState::default());

        assert_eq!(tokenizer.candidate_cache_len(), 1);
        let counters = tokenizer.counters();
        assert_eq!(counters.candidate_list_cache_misses, 1, "{counters:#?}");
        assert_eq!(counters.candidate_list_cache_hits, 1, "{counters:#?}");
    }

    #[test]
    fn candidate_cache_key_includes_dynamic_end_state() {
        let grammar = r##"{
            "scopeName": "source.candidate-cache-end",
            "patterns": [
                {"begin":"/\\*", "end":"\\*/", "name":"comment.block.candidate-cache-end"},
                {"match":"x", "name":"keyword.x.candidate-cache-end"}
            ]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        tokenizer.set_counters_enabled(true);

        let first = tokenizer.tokenize_line_scopes("/* open", TokenizerState::default());
        tokenizer.tokenize_line_scopes("inside */ x", first.state);

        assert!(tokenizer.candidate_cache_len() >= 2);
        let counters = tokenizer.counters();
        assert!(counters.candidate_list_cache_misses >= 2, "{counters:#?}");
    }

    #[test]
    fn candidate_cache_distinguishes_same_length_dynamic_end_patterns() {
        let grammar = r##"{
            "scopeName": "source.candidate-cache-dynamic-end",
            "patterns": [
                {"begin":"^<<([A-Z]{3})$", "end":"^\\1$", "name":"string.heredoc.candidate-cache-dynamic-end"}
            ]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        tokenizer.set_counters_enabled(true);

        let foo = tokenizer.tokenize_line_scopes("<<FOO", TokenizerState::default());
        let bar = tokenizer.tokenize_line_scopes("<<BAR", TokenizerState::default());
        assert_ne!(foo.exit_state_id, bar.exit_state_id);

        tokenizer.tokenize_line_scopes("body", foo.state);
        tokenizer.tokenize_line_scopes("body", bar.state);

        assert!(tokenizer.candidate_cache_len() >= 3);
        let counters = tokenizer.counters();
        assert!(counters.candidate_list_cache_misses >= 3, "{counters:#?}");
    }

    #[test]
    fn candidate_cache_builds_multi_pattern_set_search() {
        let grammar = r##"{
            "scopeName": "source.candidate-dfa",
            "patterns": [
                {"match":"alpha", "name":"keyword.alpha.candidate-dfa"},
                {"match":"beta", "name":"keyword.beta.candidate-dfa"}
            ]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        tokenizer.tokenize_line_scopes("beta", TokenizerState::default());

        let set = tokenizer
            .candidate_cache
            .get(&StateId(0))
            .expect("initial state candidates should be cached");
        assert!(set.pattern_set_search.is_some());
    }

    #[test]
    fn candidate_blueprint_reuses_structure_across_distinct_scope_stacks() {
        let grammar = r##"{
            "scopeName": "source.blueprint-stacks",
            "patterns": [{
                "begin": "^([a-z]+):$",
                "end": "^end$",
                "name": "meta.block.$1.blueprint-stacks",
                "patterns": [
                    {"match":"(x)", "captures":{"1":{"name":"keyword.x.blueprint-stacks"}}},
                    {"match":"y", "name":"keyword.y.blueprint-stacks"}
                ]
            }]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        tokenizer.set_counters_enabled(true);

        let alpha = tokenizer.tokenize_line_scopes("alpha:", TokenizerState::default());
        let beta = tokenizer.tokenize_line_scopes("beta:", TokenizerState::default());
        assert_ne!(alpha.exit_state_id, beta.exit_state_id);

        let alpha_body = tokenizer.tokenize_line_scopes("x", alpha.state);
        let beta_body = tokenizer.tokenize_line_scopes("x", beta.state);
        let alpha_set = tokenizer
            .candidate_cache
            .get(&alpha_body.entry_state_id)
            .expect("alpha candidates");
        let beta_set = tokenizer
            .candidate_cache
            .get(&beta_body.entry_state_id)
            .expect("beta candidates");

        assert!(Arc::ptr_eq(
            alpha_set.blueprint.blueprint_arc(),
            beta_set.blueprint.blueprint_arc()
        ));
        assert_ne!(alpha_set.active_stack_id, beta_set.active_stack_id);
        assert_ne!(alpha_set.end_stack_id, beta_set.end_stack_id);
        assert!(
            alpha_body.tokens[0]
                .scopes
                .contains(&"meta.block.alpha.blueprint-stacks".to_owned())
        );
        assert!(
            beta_body.tokens[0]
                .scopes
                .contains(&"meta.block.beta.blueprint-stacks".to_owned())
        );
        assert!(
            alpha_body.tokens[0]
                .scopes
                .contains(&"keyword.x.blueprint-stacks".to_owned())
        );
        assert!(
            beta_body.tokens[0]
                .scopes
                .contains(&"keyword.x.blueprint-stacks".to_owned())
        );

        let counters = tokenizer.counters();
        assert_eq!(counters.pattern_set_construction_count, 1, "{counters:#?}");
    }

    #[test]
    fn candidate_blueprint_key_keeps_dynamic_end_patterns_exact() {
        let grammar = r##"{
            "scopeName": "source.blueprint-dynamic-end",
            "patterns": [{
                "begin": "^<<([A-Z]+)$",
                "end": "^\\1$",
                "patterns": [{"match":"body", "name":"string.body.blueprint-dynamic-end"}]
            }]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();

        let foo = tokenizer.tokenize_line_scopes("<<FOO", TokenizerState::default());
        let bar = tokenizer.tokenize_line_scopes("<<BAR", TokenizerState::default());
        tokenizer.tokenize_line_scopes("body", foo.state);
        tokenizer.tokenize_line_scopes("body", bar.state);

        let foo_set = tokenizer.candidate_cache.get(&foo.exit_state_id).unwrap();
        let bar_set = tokenizer.candidate_cache.get(&bar.exit_state_id).unwrap();
        assert!(!Arc::ptr_eq(
            foo_set.blueprint.blueprint_arc(),
            bar_set.blueprint.blueprint_arc()
        ));
        assert_eq!(foo_set.candidates[0].pattern, "^FOO$");
        assert_eq!(bar_set.candidates[0].pattern, "^BAR$");
    }

    #[test]
    fn candidate_blueprint_key_uses_exact_injection_outcome() {
        let grammar = r##"{
            "scopeName": "source.blueprint-injections",
            "patterns": [{
                "begin": "^([a-z]+):$",
                "end": "^end$",
                "name": "meta.$1.blueprint-injections",
                "patterns": [{"match":"!", "name":"plain.bang.blueprint-injections"}]
            }],
            "injections": {
                "L:meta.alpha.blueprint-injections": {
                    "match":"!", "name":"injected.bang.blueprint-injections"
                }
            }
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();

        let alpha = tokenizer.tokenize_line_scopes("alpha:", TokenizerState::default());
        let beta = tokenizer.tokenize_line_scopes("beta:", TokenizerState::default());
        let alpha_body = tokenizer.tokenize_line_scopes("!", alpha.state);
        let beta_body = tokenizer.tokenize_line_scopes("!", beta.state);

        let alpha_set = tokenizer
            .candidate_cache
            .get(&alpha_body.entry_state_id)
            .unwrap();
        let beta_set = tokenizer
            .candidate_cache
            .get(&beta_body.entry_state_id)
            .unwrap();
        assert!(!Arc::ptr_eq(
            alpha_set.blueprint.blueprint_arc(),
            beta_set.blueprint.blueprint_arc()
        ));
        assert!(
            alpha_body.tokens[0]
                .scopes
                .contains(&"injected.bang.blueprint-injections".to_owned())
        );
        assert!(
            beta_body.tokens[0]
                .scopes
                .contains(&"plain.bang.blueprint-injections".to_owned())
        );
    }

    #[test]
    fn embedded_grammar_inline_injections_do_not_leak_into_root() {
        let root = r##"{
            "scopeName": "source.injection-host",
            "patterns": [{"match":"x", "name":"plain.injection-host"}]
        }"##;
        let dependency = r##"{
            "scopeName": "source.injection-dependency",
            "injections": {
                "L:source.injection-host": {
                    "match":"x", "name":"leaked.injection-dependency"
                }
            }
        }"##;
        let mut set = GrammarSet::new();
        let root = set.load_and_add(root).unwrap();
        set.load_and_add(dependency).unwrap();
        let mut tokenizer = TextMateTokenizer::new(set, root);

        let line = tokenizer.tokenize_line_scopes("x", TokenizerState::default());
        assert!(line_has_scope(&line, "plain.injection-host"), "{line:#?}");
        assert!(
            !line_has_scope(&line, "leaked.injection-dependency"),
            "{line:#?}"
        );
    }

    #[test]
    fn standalone_injection_activates_only_for_registered_root_scope() {
        let host = r##"{
            "scopeName": "source.standalone-host",
            "patterns": [{"match":"x", "name":"plain.standalone-host"}]
        }"##;
        let registered = r##"{
            "scopeName": "source.standalone-injection",
            "injectionSelector": "L:source.standalone-host",
            "injectTo": ["source.standalone-host"],
            "patterns": [{"match":"x", "name":"injected.standalone-host"}]
        }"##;
        let unrelated = r##"{
            "scopeName": "source.unrelated-standalone-injection",
            "injectionSelector": "L:source.standalone-host",
            "injectTo": ["source.some-other-host"],
            "patterns": [{"match":"x", "name":"leaked.unrelated-standalone"}]
        }"##;
        let mut set = GrammarSet::new();
        let root = set.load_and_add(host).unwrap();
        set.load_and_add(registered).unwrap();
        set.load_and_add(unrelated).unwrap();
        let mut tokenizer = TextMateTokenizer::new(set, root);

        let line = tokenizer.tokenize_line_scopes("x", TokenizerState::default());
        assert!(
            line_has_scope(&line, "injected.standalone-host"),
            "{line:#?}"
        );
        assert!(!line_has_scope(&line, "plain.standalone-host"), "{line:#?}");
        assert!(
            !line_has_scope(&line, "leaked.unrelated-standalone"),
            "{line:#?}"
        );
    }

    #[test]
    fn standalone_injection_grammar_patterns_remain_normal_when_it_is_root() {
        let grammar = r##"{
            "scopeName": "source.standalone-root",
            "injectionSelector": "L:source.other-host",
            "injectTo": ["source.other-host"],
            "patterns": [{"match":"x", "name":"keyword.standalone-root"}]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();

        let line = tokenizer.tokenize_line_scopes("x", TokenizerState::default());
        assert!(
            line_has_scope(&line, "keyword.standalone-root"),
            "{line:#?}"
        );
    }

    #[test]
    fn changing_root_recomputes_standalone_injection_registrations() {
        let first_host = r##"{
            "scopeName": "source.first-host",
            "patterns": [{"match":"x", "name":"plain.first-host"}]
        }"##;
        let second_host = r##"{
            "scopeName": "source.second-host",
            "patterns": [{"match":"x", "name":"plain.second-host"}]
        }"##;
        let injection = r##"{
            "scopeName": "source.second-host-injection",
            "injectionSelector": "L:source.second-host",
            "injectTo": ["source.second-host"],
            "patterns": [{"match":"x", "name":"injected.second-host"}]
        }"##;
        let mut set = GrammarSet::new();
        let first = set.load_and_add(first_host).unwrap();
        let second = set.load_and_add(second_host).unwrap();
        set.load_and_add(injection).unwrap();
        let mut tokenizer = TextMateTokenizer::new(set, first);

        let first_line = tokenizer.tokenize_line_scopes("x", TokenizerState::default());
        assert!(line_has_scope(&first_line, "plain.first-host"));
        assert!(!line_has_scope(&first_line, "injected.second-host"));

        tokenizer.set_root(second);
        let second_line = tokenizer.tokenize_line_scopes("x", TokenizerState::default());
        assert!(
            line_has_scope(&second_line, "injected.second-host"),
            "{second_line:#?}"
        );
        assert!(!line_has_scope(&second_line, "plain.second-host"));
    }

    #[test]
    fn prepared_repository_context_walk_is_iterative() {
        let rule_count = 5_000u32;
        let rules = (0..rule_count)
            .map(|index| crate::engine::grammar::Rule {
                id: RuleId(index),
                local_repository: BTreeMap::new(),
                body: RuleBody::IncludeOnly {
                    patterns: if index + 1 < rule_count {
                        vec![RuleRef::Rule(RuleId(index + 1))]
                    } else {
                        Vec::new()
                    },
                },
            })
            .collect();
        let mut grammars = GrammarSet::new();
        let root = grammars.add(CompiledGrammar {
            id: GrammarId(0),
            scope_name: "source.deep-context".to_owned(),
            metadata: crate::engine::grammar::GrammarMetadata::default(),
            string_names: Vec::new(),
            patterns: Vec::new(),
            rules,
            repository: BTreeMap::new(),
            top_level: vec![RuleRef::Rule(RuleId(0))],
            injections: Vec::new(),
            scope_names: Vec::new(),
        });

        let (contexts, complete) = compile_rule_repository_contexts(&grammars, root, &[], true);

        assert!(complete);
        assert_eq!(contexts.len(), rule_count as usize);
    }

    #[test]
    fn repository_contexts_allocate_dense_tables_only_for_reached_grammars() {
        let mut grammars = GrammarSet::new();
        let root = grammars
            .load_and_add(
                r##"{
                    "scopeName": "source.dense-root",
                    "patterns": [{"include":"#entry"}],
                    "repository": {
                        "entry": {"match":"x", "name":"keyword.dense-root"},
                        "unreached": {"match":"y"}
                    }
                }"##,
            )
            .unwrap();
        grammars
            .load_and_add(
                r##"{
                    "scopeName": "source.dense-unreached",
                    "patterns": [{"match":"z"}]
                }"##,
            )
            .unwrap();

        let (contexts, complete) = compile_rule_repository_contexts(&grammars, root, &[], true);

        assert!(complete);
        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts.allocated_grammar_count(), 1);
        assert_eq!(
            contexts.dense_slot_count(root),
            grammars.grammar(root).unwrap().rules.len()
        );
        assert_eq!(contexts.dense_slot_count(GrammarId(1)), 0);
    }

    #[test]
    fn dense_repository_contexts_preserve_sparse_public_rule_ids() {
        let context = Arc::new(RepositoryBindings::default());
        let mut contexts = RuleRepositoryContexts::new(1);

        assert!(contexts.insert_first(GrammarId(0), RuleId(99), 1, Arc::clone(&context)));
        assert!(!contexts.insert_first(GrammarId(0), RuleId(99), 1, Arc::clone(&context)));
        assert!(Arc::ptr_eq(
            contexts.get(GrammarId(0), RuleId(99)).unwrap(),
            &context
        ));
    }

    #[test]
    fn repository_name_interner_deduplicates_traversal_keys() {
        let mut names = RepositoryNameInterner::default();

        let (first, inserted_first) = names.intern("shared");
        let (second, inserted_second) = names.intern("shared");
        let (other, inserted_other) = names.intern("other");

        assert!(inserted_first);
        assert!(!inserted_second);
        assert!(inserted_other);
        assert_eq!(first, second);
        assert_ne!(first, other);
        assert_eq!(names.ids.len(), 2);
    }

    #[test]
    fn prepared_language_rejects_closure_work_over_the_hard_bound() {
        let mut grammars = GrammarSet::new();
        let root = grammars.add(CompiledGrammar {
            id: GrammarId(0),
            scope_name: "source.over-budget-context".to_owned(),
            metadata: crate::engine::grammar::GrammarMetadata::default(),
            string_names: Vec::new(),
            patterns: Vec::new(),
            rules: vec![crate::engine::grammar::Rule {
                id: RuleId(0),
                local_repository: BTreeMap::new(),
                body: RuleBody::IncludeOnly {
                    patterns: Vec::new(),
                },
            }],
            repository: BTreeMap::new(),
            top_level: vec![RuleRef::Rule(RuleId(0)); MAX_PREPARED_GRAMMAR_PENDING_REFS + 1],
            injections: Vec::new(),
            scope_names: Vec::new(),
        });

        assert!(PreparedLanguage::try_new(grammars, root).is_err());
    }

    #[test]
    fn repository_bindings_flatten_small_unbounded_overlays() {
        let root = Arc::new(RepositoryBindings::default());
        let first = RepositoryBindings::overlay(
            root,
            BTreeMap::from([
                ("first".to_owned(), "one".to_owned()),
                ("$mark.local.0.user-entry".to_owned(), "prefixed".to_owned()),
            ]),
            true,
        );
        let second = RepositoryBindings::overlay(
            Arc::clone(&first),
            BTreeMap::from([("second".to_owned(), "two".to_owned())]),
            true,
        );

        assert_eq!(second.get("first").map(String::as_str), Some("one"));
        assert_eq!(second.get("second").map(String::as_str), Some("two"));
        assert_eq!(
            second.get("$mark.local.0.user-entry").map(String::as_str),
            Some("prefixed")
        );
        assert_eq!(first.local.len(), 2);
        assert_eq!(second.local.len(), 3);
        assert!(second.parent.is_none());
    }

    #[test]
    fn repository_binding_blocks_bound_deep_lookup_chains() {
        let layer_count = usize::from(REPOSITORY_BINDING_BLOCK_LAYERS) * 4;
        let mut context = Arc::new(RepositoryBindings::default());
        for index in 0..layer_count {
            context = RepositoryBindings::overlay(
                context,
                BTreeMap::from([(format!("name-{index}"), format!("value-{index}"))]),
                false,
            );
        }

        assert_eq!(context.get("name-0").map(String::as_str), Some("value-0"));
        assert_eq!(
            context
                .get(&format!("name-{}", layer_count - 1))
                .map(String::as_str),
            Some(format!("value-{}", layer_count - 1).as_str())
        );
        assert_eq!(context.get("$mark.local.999.entry"), None);

        let mut block_count = 0;
        let mut cursor = Some(context.as_ref());
        while let Some(bindings) = cursor {
            block_count += 1;
            cursor = bindings.parent.as_deref();
        }
        assert!(block_count <= layer_count / usize::from(REPOSITORY_BINDING_BLOCK_LAYERS) + 1);
    }

    #[test]
    fn prepared_repository_context_budget_stops_before_cloning_the_next_overlay() {
        let local = BTreeMap::from([(
            "large".to_owned(),
            "x".repeat(MAX_PREPARED_GRAMMAR_WALK_BYTES / 8),
        )]);
        let mut budget = RepositoryContextBudget::new(true);
        let mut admitted = 0usize;
        while budget.charge_rule(&local, false) {
            admitted += 1;
        }

        assert!(budget.exceeded);
        assert!(admitted < 8);
    }

    #[test]
    fn grammar_set_clones_share_repository_contexts_until_mutated() {
        let mut set = GrammarSet::new();
        let root = set
            .load_and_add(
                r##"{
                    "scopeName": "source.context-cache",
                    "patterns": [{"include":"#entry"}],
                    "repository": {
                        "entry": {"match":"x", "name":"keyword.context-cache"}
                    }
                }"##,
            )
            .unwrap();
        let selectors = compile_injection_selectors(&set, root);
        let first = set.rule_repository_contexts(root, &selectors);
        let cloned = set.clone();
        let second = cloned.rule_repository_contexts(root, &selectors);
        assert!(Arc::ptr_eq(&first, &second));

        let cached = Arc::downgrade(&first);
        set.load_and_add(r#"{"scopeName":"source.later","patterns":[]}"#)
            .unwrap();
        let selectors = compile_injection_selectors(&set, root);
        let after_mutation = set.rule_repository_contexts(root, &selectors);
        assert!(!Arc::ptr_eq(&first, &after_mutation));

        drop(first);
        drop(second);
        assert!(cached.upgrade().is_none());
    }

    #[test]
    fn shared_rule_keeps_first_lazy_repository_binding() {
        // vscode-textmate assigns a raw rule its id on first traversal. Here
        // `shared` is first reached from `nested`'s local repository, so its
        // later `#value` include remains bound to the local value even when
        // the same shared rule is also included directly from the root.
        let grammar = r##"{
            "scopeName": "source.lazy-repository",
            "patterns": [
                {"include":"#valid"},
                {"include":"#shared"},
                {"include":"#shared-container"},
                {"include":"#shared-capture"}
            ],
            "repository": {
                "valid": {"patterns":[{"include":"#nested"}]},
                "nested": {
                    "repository": {
                        "value": {
                            "match":"x",
                            "name":"local.value.lazy-repository"
                        },
                        "container-value": {
                            "match":"y",
                            "name":"local.container-value.lazy-repository"
                        },
                        "capture-value": {
                            "match":"z",
                            "name":"local.capture-value.lazy-repository"
                        },
                        "walk": {"patterns":[
                            {"include":"#shared"},
                            {"include":"#shared-container"},
                            {"include":"#shared-capture"}
                        ]}
                    },
                    "patterns":[{"include":"#walk"}]
                },
                "shared": {
                    "begin":"<",
                    "end":">",
                    "name":"meta.shared.lazy-repository",
                    "patterns":[{"include":"#value"}]
                },
                "value": {
                    "match":"x",
                    "name":"root.value.lazy-repository"
                },
                "shared-container": {
                    "patterns":[{"include":"#container-value"}]
                },
                "container-value": {
                    "match":"y",
                    "name":"root.container-value.lazy-repository"
                },
                "shared-capture": {
                    "match":"(z)",
                    "captures":{"1":{"patterns":[{"include":"#capture-value"}]}}
                },
                "capture-value": {
                    "match":"z",
                    "name":"root.capture-value.lazy-repository"
                }
            }
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        let line = tokenizer.tokenize_line_scopes("<x> y z", TokenizerState::default());

        for scope in [
            "local.value.lazy-repository",
            "local.container-value.lazy-repository",
            "local.capture-value.lazy-repository",
        ] {
            assert!(line_has_scope(&line, scope), "missing {scope}: {line:#?}");
        }
        assert!(
            line.tokens
                .iter()
                .flat_map(|token| &token.scopes)
                .all(|scope| !scope.starts_with("root.")),
            "{line:#?}"
        );
    }

    #[test]
    fn shared_rule_does_not_rebind_after_root_first_compilation() {
        let grammar = r##"{
            "scopeName": "source.lazy-repository-root-first",
            "patterns": [
                {"include":"#shared"},
                {"include":"#nested"}
            ],
            "repository": {
                "shared": {
                    "begin":"<", "end":">",
                    "patterns":[{"include":"#value"}]
                },
                "value": {"match":"x", "name":"root.value.lazy-root-first"},
                "nested": {
                    "repository": {
                        "value": {"match":"x", "name":"local.value.lazy-root-first"}
                    },
                    "patterns":[{"include":"#shared"}]
                }
            }
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        let line = tokenizer.tokenize_line_scopes("<x>", TokenizerState::default());

        assert!(
            line_has_scope(&line, "root.value.lazy-root-first"),
            "{line:#?}"
        );
        assert!(
            !line_has_scope(&line, "local.value.lazy-root-first"),
            "{line:#?}"
        );
    }

    #[test]
    fn begin_rule_with_transitively_missing_local_include_is_not_entered() {
        // vscode-textmate drops a begin/end rule when its non-empty pattern
        // closure contains no resolvable rule. Keeping the empty frame would
        // hide all host patterns until `end` (real grammars commonly contain
        // stale repository includes in grouping rules).
        let grammar = r##"{
            "scopeName": "source.missing-local-closure",
            "patterns": [
                {"include":"#stale-group"},
                {"match":"\\b(?:int|string)\\b", "name":"support.type.test"},
                {"match":"(?<=^|[(,])\\s*([_a-z][0-9_a-z]*)\\s*(:)",
                 "captures":{"1":{"name":"variable.parameter.test"},"2":{"name":"punctuation.colon.test"}}},
                {"match":"[+=]", "name":"keyword.operator.test"},
                {"begin":"\"", "end":"\"", "name":"string.quoted.test"}
            ],
            "repository": {
                "stale-group": {
                    "begin":"\\(", "end":"\\)",
                    "patterns":[{"include":"#stale-chain"}]
                },
                "stale-chain": {
                    "patterns":[{"include":"#absent"}]
                }
            }
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        let line = tokenizer.tokenize_line_scopes(
            "(int value = call(name: \"ok\"))",
            TokenizerState::default(),
        );

        for scope in [
            "support.type.test",
            "variable.parameter.test",
            "punctuation.colon.test",
            "keyword.operator.test",
            "string.quoted.test",
        ] {
            assert!(line_has_scope(&line, scope), "missing {scope}: {line:#?}");
        }
        assert!(line.state.is_initial(), "stale group must not be entered");
    }

    #[test]
    fn missing_external_repository_include_drops_only_empty_parent_patterns() {
        let root = r##"{
            "scopeName": "source.missing-external-repository",
            "patterns": [
                {
                    "begin":"\"", "end":"\"", "name":"string.dropped.test",
                    "patterns":[{"include":"source.dependency#absent"}]
                },
                {
                    "begin":"'", "end":"'", "name":"string.retained.test",
                    "patterns":[
                        {"include":"source.dependency#absent"},
                        {"match":"\\\\.", "name":"constant.character.escape.test"}
                    ]
                },
                {"match":"ok", "name":"keyword.control.test"}
            ]
        }"##;
        let dependency = r##"{
            "scopeName": "source.dependency",
            "patterns":[{"match":"dependency", "name":"support.dependency.test"}]
        }"##;
        let mut set = GrammarSet::new();
        let root = set.load_and_add(root).unwrap();
        set.load_and_add(dependency).unwrap();
        let mut tokenizer = TextMateTokenizer::new(set, root);

        let line = tokenizer.tokenize_line_scopes("\"plain\" 'kept' ok", TokenizerState::default());
        assert!(!line_has_scope(&line, "string.dropped.test"), "{line:#?}");
        assert!(line_has_scope(&line, "string.retained.test"), "{line:#?}");
        assert!(line_has_scope(&line, "keyword.control.test"), "{line:#?}");
    }

    #[test]
    fn empty_and_cyclic_containers_are_not_treated_as_missing_patterns() {
        // An empty compiled child is not necessarily a missing child.
        // vscode-textmate retains both genuinely empty containers and
        // resolved include cycles; only unresolved children set
        // `hasMissingPatterns`.
        let grammar = r##"{
            "scopeName": "source.resolved-empty-containers",
            "patterns": [
                {
                    "begin":"\"", "end":"\"", "name":"string.empty-child.test",
                    "patterns":[{}]
                },
                {
                    "begin":"'", "end":"'", "name":"string.cyclic-child.test",
                    "patterns":[{"include":"#cycle"}]
                },
                {
                    "begin":"`", "end":"`", "name":"string.alias-cycle.test",
                    "patterns":[{"include":"#alias-a"}]
                }
            ],
            "repository": {
                "cycle": {"patterns":[{"include":"#cycle"}]},
                "alias-a": {"include":"#alias-b"},
                "alias-b": {"include":"#alias-a"}
            }
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        let line =
            tokenizer.tokenize_line_scopes("\"empty\" 'cycle' `alias`", TokenizerState::default());

        assert!(
            line_has_scope(&line, "string.empty-child.test"),
            "{line:#?}"
        );
        assert!(
            line_has_scope(&line, "string.cyclic-child.test"),
            "{line:#?}"
        );
        assert!(
            line_has_scope(&line, "string.alias-cycle.test"),
            "{line:#?}"
        );
        assert!(line.state.is_initial(), "{:#?}", line.state);
    }

    #[test]
    fn zero_width_begin_end_cycle_stops_without_degrading_line() {
        let grammar = r##"{
            "scopeName": "source.zero-width-cycle",
            "patterns": [{
                "begin":"(?<=x)", "end":"(?=$)",
                "name":"meta.zero-width-cycle"
            }]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        tokenizer.set_counters_enabled(true);
        let line = tokenizer.tokenize_line_scopes("x\n", TokenizerState::default());

        assert!(line.tokens.iter().all(|token| {
            token.range.start <= token.range.end
                && token.range.end <= 2
                && "x\n".is_char_boundary(token.range.start)
                && "x\n".is_char_boundary(token.range.end)
        }));
        assert_eq!(line.state.depth(), 1, "oracle retains the entered frame");
        assert_eq!(tokenizer.counters().degraded_lines, 0);
        assert!(tokenizer.counters().candidate_searches < 10);

        let next = tokenizer.tokenize_line_scopes("next\n", line.state);
        assert!(line_has_scope(&next, "meta.zero-width-cycle"), "{next:#?}");
        assert!(next.state.is_initial(), "end should close on the next line");
    }

    #[test]
    fn empty_end_pattern_is_a_non_matching_sentinel() {
        // vscode-textmate does not compile an empty `end` as a zero-width
        // match. Some real grammars rely on that behavior for a frame that
        // remains open while its child patterns continue to tokenize.
        let grammar = r##"{
            "scopeName": "source.empty-end",
            "patterns": [{
                "begin":"@(?=[A-Za-z])", "end":"",
                "name":"meta.decorator.empty-end",
                "patterns":[{
                    "begin":"[A-Za-z]+\\(", "end":"\\)",
                    "name":"meta.call.empty-end"
                }]
            }]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        let first =
            tokenizer.tokenize_line_scopes("@description(value)", TokenizerState::default());

        assert!(
            line_has_scope(&first, "meta.decorator.empty-end"),
            "{first:#?}"
        );
        assert!(line_has_scope(&first, "meta.call.empty-end"), "{first:#?}");
        assert_eq!(
            first.state.depth(),
            1,
            "empty end must leave its frame open"
        );

        let second = tokenizer.tokenize_line_scopes("next", first.state);
        assert!(
            line_has_scope(&second, "meta.decorator.empty-end"),
            "{second:#?}"
        );
        assert_eq!(second.state.depth(), 1);
    }

    #[test]
    fn text_end_pattern_closes_on_final_unterminated_line() {
        let grammar = r##"{
            "scopeName": "source.text-end",
            "patterns": [{
                "begin": "BEGIN", "end": "\\z", "name": "meta.text-end"
            }]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        let first = tokenizer.tokenize_line_scopes("BEGIN\n", TokenizerState::default());
        assert_eq!(first.state.depth(), 1);

        let final_line = tokenizer.tokenize_line_scopes("tail", first.state);
        assert!(final_line.state.is_initial(), "{:#?}", final_line.state);
    }

    #[test]
    fn candidate_sets_reuse_compiled_patterns() {
        let grammar = r##"{
            "scopeName": "source.compiled-candidates",
            "patterns": [
                {"match":"alpha", "name":"keyword.alpha.compiled-candidates"},
                {"match":"beta", "name":"keyword.beta.compiled-candidates"}
            ]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        tokenizer.set_counters_enabled(true);

        tokenizer.tokenize_line_scopes("alpha", TokenizerState::default());
        tokenizer.clear_candidate_cache();
        tokenizer.tokenize_line_scopes("beta", TokenizerState::default());

        let counters = tokenizer.counters();
        assert_eq!(counters.regex_compile_count, 2, "{counters:#?}");
        assert_eq!(counters.pattern_set_construction_count, 2, "{counters:#?}");
    }

    #[test]
    fn warm_candidate_entry_does_not_recompile_or_rebuild_pattern_set() {
        let grammar = r##"{
            "scopeName": "source.warm-candidates",
            "patterns": [
                {"match":"alpha", "name":"keyword.alpha.warm-candidates"},
                {"match":"beta", "name":"keyword.beta.warm-candidates"}
            ]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        tokenizer.set_counters_enabled(true);

        tokenizer.tokenize_line_scopes("alpha", TokenizerState::default());
        tokenizer.tokenize_line_scopes("beta", TokenizerState::default());

        let counters = tokenizer.counters();
        assert_eq!(counters.regex_compile_count, 2, "{counters:#?}");
        assert_eq!(counters.pattern_set_construction_count, 1, "{counters:#?}");
        assert!(
            counters
                .pattern_compile_counts
                .iter()
                .all(|entry| entry.count == 1),
            "{counters:#?}"
        );
    }

    #[test]
    fn duplicate_pattern_text_keeps_distinct_static_pattern_identities() {
        let grammar = r##"{
            "scopeName": "source.duplicate-pattern-id",
            "patterns": [
                {"match":"x", "name":"keyword.first.duplicate-pattern-id"},
                {"match":"x", "name":"string.second.duplicate-pattern-id"}
            ]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        tokenizer.set_counters_enabled(true);

        let line = tokenizer.tokenize_line_scopes("x", TokenizerState::default());
        assert!(
            line.tokens[0]
                .scopes
                .contains(&"keyword.first.duplicate-pattern-id".to_owned())
        );
        assert!(
            !line.tokens[0]
                .scopes
                .contains(&"string.second.duplicate-pattern-id".to_owned())
        );

        let counters = tokenizer.counters();
        assert_eq!(counters.regex_compile_count, 2, "{counters:#?}");
        assert_eq!(counters.pattern_compile_counts.len(), 2, "{counters:#?}");
        assert_ne!(
            counters.pattern_compile_counts[0].pattern_id,
            counters.pattern_compile_counts[1].pattern_id
        );
    }

    #[test]
    fn dynamic_end_cache_reuses_only_equal_substitutions() {
        let grammar = r##"{
            "scopeName": "source.dynamic-compile-cache",
            "patterns": [{
                "begin":"^<<([A-Z]+)$",
                "end":"^\\1$",
                "name":"string.heredoc.dynamic-compile-cache"
            }]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        tokenizer.set_counters_enabled(true);

        for marker in ["FOO", "BAR", "FOO"] {
            let begin =
                tokenizer.tokenize_line_scopes(&format!("<<{marker}"), TokenizerState::default());
            let end = tokenizer.tokenize_line_scopes(marker, begin.state);
            assert!(end.state.is_initial());
        }

        let counters = tokenizer.counters();
        assert_eq!(counters.regex_compile_count, 3, "{counters:#?}");
        let dynamic = counters
            .pattern_compile_counts
            .iter()
            .filter(|entry| entry.pattern_id.is_none())
            .collect::<Vec<_>>();
        assert_eq!(dynamic.len(), 2, "{counters:#?}");
        assert!(dynamic.iter().all(|entry| entry.count == 1));
    }

    #[test]
    fn inline_candidate_sets_persist_across_capture_retokenization() {
        let grammar = r##"{
            "scopeName": "source.inline-cache",
            "patterns": [{
                "match":"(x)",
                "captures": {
                    "1": {"patterns": [
                        {"match":"x", "name":"keyword.x.inline-cache"}
                    ]}
                }
            }]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        tokenizer.set_counters_enabled(true);

        tokenizer.tokenize_line_scopes("x", TokenizerState::default());
        tokenizer.tokenize_line_scopes("x", TokenizerState::default());

        let counters = tokenizer.counters();
        assert_eq!(
            counters.inline_candidate_set_construction_count, 1,
            "{counters:#?}"
        );
        assert_eq!(counters.regex_compile_count, 2, "{counters:#?}");
    }

    #[test]
    fn capture_replay_is_skipped_only_for_static_capture_free_matches() {
        let candidate = |name: &str| Candidate {
            order: 0,
            base_grammar_id: GrammarId(0),
            pattern: "pattern".to_owned(),
            pattern_id: None,
            scope_prefix: None,
            kind: CandidateKind::Match {
                grammar_id: GrammarId(0),
                name: Some(name.to_owned()),
                name_template: None,
                captures: Arc::new(CaptureSpec::default()),
            },
        };

        assert!(!candidate_requires_capture_replay(&candidate(
            "keyword.static"
        )));
        assert!(candidate_requires_capture_replay(&candidate(
            "keyword.dynamic.$1"
        )));
    }

    #[test]
    fn capture_result_buffers_are_reused_and_hard_bounded() {
        let grammar = r##"{
            "scopeName": "source.capture-pool",
            "patterns": [{
                "match":"(x)",
                "captures":{"1":{"name":"keyword.capture-pool"}}
            }]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();

        let first = tokenizer.tokenize_line_scopes("xxx", TokenizerState::default());
        assert!(line_has_scope(&first, "keyword.capture-pool"));
        assert_eq!(tokenizer.capture_result_pool.len(), 1);
        let capacity = tokenizer.capture_result_pool[0].capacity();
        let allocation = tokenizer.capture_result_pool[0].as_ptr();

        let second = tokenizer.tokenize_line_scopes("xxx", TokenizerState::default());
        assert!(line_has_scope(&second, "keyword.capture-pool"));
        assert_eq!(tokenizer.capture_result_pool.len(), 1);
        assert_eq!(tokenizer.capture_result_pool[0].capacity(), capacity);
        assert_eq!(tokenizer.capture_result_pool[0].as_ptr(), allocation);

        tokenizer.capture_result_pool.clear();
        for _ in 0..MAX_CAPTURE_RESULT_POOL_ENTRIES + 4 {
            tokenizer.recycle_capture_result_buffer(vec![None]);
        }
        assert_eq!(
            tokenizer.capture_result_pool.len(),
            MAX_CAPTURE_RESULT_POOL_ENTRIES
        );
        tokenizer.capture_result_pool.clear();
        tokenizer
            .recycle_capture_result_buffer(Vec::with_capacity(MAX_POOLED_CAPTURE_CAPACITY + 1));
        assert!(tokenizer.capture_result_pool.is_empty());
    }

    #[test]
    fn capture_reference_scanners_match_substitution_syntax() {
        let mut live = Vec::new();
        add_scope_capture_refs(
            Some("entity.$1.${2}.${3:/downcase}.${4:/upcase}.$bad"),
            &mut live,
        );
        assert_eq!(live, [1, 2, 3, 4]);

        live.clear();
        add_end_pattern_capture_refs(r"^\1-\12-\\1$", &mut live);
        assert_eq!(live, [1, 12]);
    }

    #[test]
    fn dynamic_matcher_cache_identity_includes_capture_liveness() {
        let mut tokenizer =
            TextMateTokenizer::from_grammar(r#"{"scopeName":"source.live-cache","patterns":[]}"#)
                .unwrap();
        let first = tokenizer.cached_dynamic_matcher_with_live_captures("(x)", vec![1]);
        let reused = tokenizer.cached_dynamic_matcher_with_live_captures("(x)", vec![1]);
        let distinct = tokenizer.cached_dynamic_matcher_with_live_captures("(x)", vec![]);
        assert!(Arc::ptr_eq(&first, &reused));
        assert!(!Arc::ptr_eq(&first, &distinct));
    }

    #[test]
    fn multi_pattern_dfa_preserves_candidate_order_tie_break() {
        let grammar = r##"{
            "scopeName": "source.candidate-dfa-order",
            "patterns": [
                {"match":"ab", "name":"keyword.long.candidate-dfa-order"},
                {"match":"a", "name":"keyword.short.candidate-dfa-order"}
            ]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        let line = tokenizer.tokenize_line_scopes("ab", TokenizerState::default());

        assert!(
            line.tokens[0]
                .scopes
                .iter()
                .any(|scope| scope == "keyword.long.candidate-dfa-order"),
            "{:#?}",
            line.tokens
        );
    }

    #[test]
    fn fallback_candidates_can_beat_later_dfa_candidates() {
        let grammar = r##"{
            "scopeName": "source.candidate-fallback-order",
            "patterns": [
                {"match":"(?=a)a", "name":"keyword.fallback.candidate-fallback-order"},
                {"match":"a", "name":"keyword.dfa.candidate-fallback-order"}
            ]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        let line = tokenizer.tokenize_line_scopes("a", TokenizerState::default());

        assert!(
            line.tokens[0]
                .scopes
                .iter()
                .any(|scope| scope == "keyword.fallback.candidate-fallback-order"),
            "{:#?}",
            line.tokens
        );
    }

    #[test]
    fn counters_record_prefilter_hits_and_skips() {
        let grammar = r##"{
            "scopeName": "source.prefilter-counters",
            "patterns": [{"match":"z+", "name":"keyword.prefilter-counters"}]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        tokenizer.set_counters_enabled(true);

        tokenizer.tokenize_line_scopes("abc", TokenizerState::default());
        tokenizer.tokenize_line_scopes("zz", TokenizerState::default());

        let counters = tokenizer.counters();
        assert!(counters.prefilter_checks >= 2, "{counters:#?}");
        assert!(counters.prefilter_skips >= 1, "{counters:#?}");
        assert!(counters.prefilter_hits >= 1, "{counters:#?}");
    }

    #[test]
    fn line_byte_limit_degrades_only_that_line() {
        let grammar = r##"{
            "scopeName": "source.line-limit",
            "patterns": [{"match":"ok", "name":"keyword.line-limit"}]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        tokenizer.set_counters_enabled(true);
        tokenizer.set_max_line_bytes(Some(4));

        let long = tokenizer.tokenize_line_scopes("too long", TokenizerState::default());
        let short = tokenizer.tokenize_line_scopes("ok", TokenizerState::default());

        assert_eq!(long.tokens.len(), 1);
        assert_eq!(long.tokens[0].range, 0..8);
        assert!(
            short.tokens[0]
                .scopes
                .iter()
                .any(|scope| scope == "keyword.line-limit"),
            "{:#?}",
            short.tokens
        );
        let counters = tokenizer.counters();
        assert_eq!(counters.lines_skipped, 1, "{counters:#?}");
        assert_eq!(counters.degraded_lines, 1, "{counters:#?}");
    }

    #[test]
    fn successful_while_match_sets_continuation_anchor_for_nested_end() {
        let grammar = r##"{
            "scopeName": "source.while-g-anchor",
            "patterns": [{
                "begin": "\\A",
                "while": "^",
                "patterns": [{
                    "begin": "\\G%YAML 1\\.2",
                    "end": "\\G(?=---)",
                    "name": "meta.directive.while-g-anchor",
                    "patterns": [{"match": ".+", "name": "invalid.directive.while-g-anchor"}]
                }, {
                    "match": "---",
                    "name": "entity.document.while-g-anchor"
                }]
            }]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        let directive =
            tokenizer.tokenize_line_scopes_at_line("%YAML 1.2\n", TokenizerState::default(), 0);
        let document = tokenizer.tokenize_line_scopes_at_line("---\n", directive.state, 1);

        assert!(line_has_scope(&document, "entity.document.while-g-anchor"));
        assert!(!line_has_scope(&document, "meta.directive.while-g-anchor"));
    }

    #[test]
    fn applies_capture_zero_scope() {
        let grammar = r##"{
            "scopeName": "source.fixture",
            "patterns": [{
                "match":"x",
                "name":"meta.$0.fixture",
                "captures":{"0":{"name":"punctuation.$0.fixture"}}
            }]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        let line = tokenizer.tokenize_line_scopes("x", TokenizerState::default());
        for expected in ["meta.x.fixture", "punctuation.x.fixture"] {
            assert!(
                line.tokens[0].scopes.iter().any(|scope| scope == expected),
                "missing {expected:?} in {:#?}",
                line.tokens
            );
        }
    }

    #[test]
    fn unicode_word_tokens_preserve_utf8_boundaries_around_astral_emoji() {
        let grammar = r##"{
            "scopeName": "source.astral-word",
            "patterns": [{"match":"\\w", "name":"meta.word.astral-word"}]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        let line = "a🛰️‿z";
        let tokenized = tokenizer.tokenize_line_scopes(line, TokenizerState::default());
        let word_ranges = tokenized
            .tokens
            .iter()
            .filter(|token| {
                token
                    .scopes
                    .iter()
                    .any(|scope| scope == "meta.word.astral-word")
            })
            .map(|token| token.range.clone())
            .collect::<Vec<_>>();

        // The symbol itself is not a word character. The following variation
        // selector is, and starts at UTF-8 byte 5 (UTF-16 offset 3).
        assert_eq!(word_ranges, [0..1, 5..12]);
        assert!(tokenized.tokens.iter().all(|token| {
            line.is_char_boundary(token.range.start) && line.is_char_boundary(token.range.end)
        }));
    }

    #[test]
    fn retokenized_capture_does_not_inherit_overlapping_capture_scope() {
        let grammar = r##"{
            "scopeName": "source.capture-order",
            "patterns": [{
                "match": "(foo)",
                "captures": {
                    "0": {"name": "meta.head.capture-order"},
                    "1": {"patterns": [
                        {"match": "foo", "name": "entity.name.capture-order"}
                    ]}
                }
            }]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        let line = tokenizer.tokenize_line_scopes("foo", TokenizerState::default());
        assert_eq!(line.tokens.len(), 1, "{:#?}", line.tokens);
        assert!(
            line.tokens[0]
                .scopes
                .iter()
                .any(|scope| scope == "entity.name.capture-order"),
            "{:#?}",
            line.tokens
        );
        assert!(
            !line.tokens[0]
                .scopes
                .iter()
                .any(|scope| scope == "meta.head.capture-order"),
            "{:#?}",
            line.tokens
        );
    }

    #[test]
    fn substitutes_capture_text_in_scope_names() {
        let grammar = r##"{
            "scopeName": "source.dynamic-scope",
            "patterns": [
                {
                    "match":"^(#)([A-Z]+)",
                    "name":"meta.directive.${2:/downcase}.dynamic-scope",
                    "captures": {
                        "2": {"name":"keyword.control.directive.$2.dynamic-scope"}
                    }
                }
            ]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        let line = tokenizer.tokenize_line_scopes("#INCLUDE", TokenizerState::default());
        let scopes = line
            .tokens
            .iter()
            .flat_map(|token| token.scopes.iter())
            .collect::<Vec<_>>();
        assert!(
            scopes
                .iter()
                .any(|scope| *scope == "meta.directive.include.dynamic-scope"),
            "{scopes:#?}"
        );
        assert!(
            scopes
                .iter()
                .any(|scope| *scope == "keyword.control.directive.INCLUDE.dynamic-scope"),
            "{scopes:#?}"
        );
    }

    #[test]
    fn begin_end_state_crosses_lines() {
        let grammar = r##"{
            "scopeName": "source.fixture",
            "patterns": [{"begin":"/\\*", "end":"\\*/", "name":"comment.block.fixture"}]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        let first = tokenizer.tokenize_line_scopes("/* hello", TokenizerState::default());
        assert_eq!(first.state.depth(), 1);
        let second = tokenizer.tokenize_line_scopes("done */", first.state);
        assert!(second.state.is_initial());
        assert!(
            second.tokens[0]
                .scopes
                .iter()
                .any(|scope| scope == "comment.block.fixture")
        );
    }

    #[test]
    fn tokenize_source_produces_shape_compatible_highlighted_text() {
        let mut tokenizer = TextMateTokenizer::from_grammar(include_str!(
            "../../assets/grammars/languages/json.tmLanguage.json"
        ))
        .unwrap();
        let highlighted = tokenizer.tokenize_source("{\"ok\": true}\n");
        assert_eq!(highlighted.lines.len(), 2);
        assert!(highlighted.lines[0].matches_text("{\"ok\": true}"));
        assert!(highlighted.lines[1].matches_text(""));
        assert!(
            highlighted.lines[0]
                .segments
                .iter()
                .all(|segment| segment.byte_start < segment.byte_end)
        );
        assert!(highlighted.lines[0].segments.iter().all(|segment| {
            highlighted.lines[0]
                .scope_table
                .stack(segment.scope_stack)
                .is_some_and(|stack| !stack.is_empty())
        }));
        assert!(Arc::ptr_eq(
            &highlighted.lines[0].scope_table,
            &highlighted.lines[1].scope_table
        ));
    }

    #[test]
    fn identical_results_reuse_output_scope_tables() {
        let grammar = r#"{
            "scopeName": "source.scope-cache",
            "patterns": [{"match": "true", "name": "constant.language.boolean"}]
        }"#;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        let first = tokenizer.tokenize_source("true\nfalse\n");
        let second = tokenizer.tokenize_source("true\nfalse\n");

        assert!(Arc::ptr_eq(
            &first.lines[0].scope_table,
            &second.lines[0].scope_table
        ));
    }

    #[test]
    fn output_scope_table_remaps_sparse_engine_ids_to_dense_refs() {
        let mut scope_names = ScopeInterner::default();
        let mut scope_stacks = ScopeStackInterner::default();
        let mut high_stack = scope_stacks.empty();
        for index in 0..4_096 {
            let scope = scope_names.intern(&format!("entity.name.generated-{index}"));
            high_stack = scope_stacks.push(scope_stacks.empty(), scope, &scope_names);
        }
        assert!(high_stack.0 >= 4_096);

        let mut builder = OutputScopeTableBuilder::new();
        let output = builder.intern_engine_stack(high_stack);
        assert_eq!(output, ScopeStackRef(1));
        assert_eq!(builder.intern_engine_stack(high_stack), output);

        let mut cache = OutputScopeTableCache::default();
        let table = builder.finish(&scope_stacks, &scope_names, &mut cache);
        assert_eq!(table.stack_count(), 2);
        assert_eq!(table.atom_count(), 1);
        assert_eq!(
            table.stack_names(output).collect::<Vec<_>>(),
            ["entity.name.generated-4095"]
        );
    }

    #[test]
    fn begin_captures_preserve_text_consumed_after_continuation_anchor() {
        let grammar = r##"{
            "scopeName": "source.fixture",
            "patterns": [{
                "begin": "\\\\href",
                "end": "}",
                "name": "meta.link.fixture",
                "patterns": [{
                    "begin": "\\G(\\{)([^}]*)(})(?:\\{[^}]*}){2}?(\\{)",
                    "beginCaptures": {
                        "1": {"name": "punctuation.begin.fixture"},
                        "2": {"name": "markup.underline.link.fixture"},
                        "3": {"name": "punctuation.end.fixture"},
                        "4": {"name": "punctuation.begin.fixture"}
                    },
                    "end": "(?=})",
                    "contentName": "meta.link.text.fixture"
                }]
            }]
        }"##;
        let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
        let line = tokenizer.tokenize_line_scopes(
            "\\href{https://example.com}{link}",
            TokenizerState::default(),
        );
        let url = line
            .tokens
            .iter()
            .find(|token| token.range == (6..25))
            .expect("URL capture token");
        assert!(
            url.scopes
                .iter()
                .any(|scope| scope == "markup.underline.link.fixture"),
            "{:?}",
            line.tokens
        );
    }

    fn core_tokenizer(language: &str) -> TextMateTokenizer {
        let mut set = GrammarSet::new();
        let mut root = None;
        for asset in crate::grammars::registry::CORE_ASSETS {
            let id = set.load_and_add(asset.source).unwrap();
            if asset.language == language {
                root = Some(id);
            }
        }
        TextMateTokenizer::new(set, root.expect("root language"))
    }

    #[test]
    fn source_budget_allows_exact_exhaustion_and_zero_step_followups() {
        let mut tokenizer = core_tokenizer("rust");
        tokenizer.fallback_call_budget_remaining = Some(7);

        assert!(tokenizer.consume_fallback_call_budget(7));
        assert_eq!(tokenizer.fallback_call_budget_remaining, Some(0));
        assert!(tokenizer.consume_fallback_call_budget(0));
        assert!(!tokenizer.consume_fallback_call_budget(1));
    }

    #[test]
    fn html_script_uses_external_javascript_scope() {
        let mut tokenizer = core_tokenizer("html");
        let line = tokenizer
            .tokenize_line_scopes("<script>let x = 1;</script>", TokenizerState::default());
        assert!(
            line.tokens
                .iter()
                .any(|token| token.scopes.iter().any(|scope| scope == "source.js")),
            "{:#?}",
            line.tokens
        );
    }

    #[test]
    fn core_fixture_languages_tokenize_without_panics() {
        let mut set = GrammarSet::new();
        for asset in crate::grammars::registry::CORE_ASSETS {
            set.load_and_add(asset.source).unwrap();
        }
        let cases = [
            ("rust", "fn main() { println!(\"hi\"); }"),
            ("typescript", "const value: number = 1;"),
            ("json", "{\"ok\": true}"),
            ("yaml", "ok: true"),
            ("toml", "name = \"mark\""),
            ("markdown", "# title"),
            ("html", "<div class=\"x\">hi</div>"),
            ("css", ".x { color: red; }"),
            ("python", "def f(x): return x + 1"),
            ("go", "func main() { println(1) }"),
            ("c", "int main(void) { return 0; }"),
            ("cpp", "auto value = std::string{};"),
            ("bash", "echo $(pwd)"),
        ];
        for (language, source) in cases {
            let asset = crate::grammars::registry::GrammarRegistry::asset(language).unwrap();
            let root = set.grammar_id_by_scope(asset.scope_name).unwrap();
            let mut tokenizer = TextMateTokenizer::new(set.clone(), root);
            let line = tokenizer.tokenize_line_scopes(source, TokenizerState::default());
            assert!(!line.tokens.is_empty(), "{language} should emit tokens");
            assert!(line.tokens.iter().all(|token| {
                source.is_char_boundary(token.range.start.min(source.len()))
                    && source.is_char_boundary(token.range.end.min(source.len()))
            }));
        }
    }

    #[test]
    fn markdown_fence_uses_external_rust_scope() {
        let mut tokenizer = core_tokenizer("markdown");
        let first = tokenizer.tokenize_line_scopes("```rust", TokenizerState::default());
        let second = tokenizer.tokenize_line_scopes("fn main() {}", first.state);
        assert!(
            second.tokens.iter().any(|token| {
                token
                    .scopes
                    .iter()
                    .any(|scope| scope.contains("embedded.block.rust"))
            }),
            "{:#?}",
            second.tokens
        );
    }

    #[test]
    fn selector_prefix_matches_dot_boundary() {
        let stack = vec!["text.html.markdown".to_owned(), "markup.raw".to_owned()];
        assert!(selector_matches("text.html markup.raw", &stack));
        assert!(!selector_matches("text.htmlx", &stack));
    }

    #[test]
    fn selector_matches_grouped_or_and_subtractions() {
        let stack = vec![
            "text.html.markdown".to_owned(),
            "meta.script.svelte".to_owned(),
            "meta.lang.ts".to_owned(),
        ];
        assert!(selector_matches(
            "(meta.script.svelte | meta.style.svelte) (meta.lang.js | meta.lang.ts)",
            &stack
        ));
        assert!(selector_matches("source.js, meta.lang.ts", &stack));
        assert!(!selector_matches(
            "meta.script.svelte - (meta.lang.ts | comment.block)",
            &stack
        ));
        assert!(selector_matches(
            "meta.script.svelte - (meta.lang.js | comment.block)",
            &stack
        ));

        let html_stack = vec![
            "text.html.basic".to_owned(),
            "meta.tag.script.begin.html".to_owned(),
        ];
        assert!(selector_matches("meta.tag.*.*.html", &html_stack));
        assert!(!selector_matches(
            "text.html - (meta.tag.*.*.html)",
            &html_stack
        ));

        let ordered_stack = vec!["source.astro".to_owned(), "meta.style.astro".to_owned()];
        assert!(selector_matches("source meta", &ordered_stack));
        assert!(!selector_matches("meta source", &ordered_stack));
        assert!(selector_matches("meta & source", &ordered_stack));
    }

    #[test]
    fn grammar_set_validates_external_include_graph() {
        let host = r##"{
            "scopeName": "source.host",
            "patterns": [{"include":"source.external#value"}]
        }"##;
        let external = r##"{
            "scopeName": "source.external",
            "repository": {"value": {"match":"ok", "name":"keyword.external"}}
        }"##;
        let mut set = GrammarSet::new();
        set.load_and_add(host).unwrap();
        set.load_and_add(external).unwrap();
        set.validate_include_graph().unwrap();

        let mut missing = GrammarSet::new();
        missing.load_and_add(host).unwrap();
        let error = missing.validate_include_graph().unwrap_err().to_string();
        assert!(error.contains("source.external"), "{error}");
    }

    #[test]
    fn base_include_resolves_to_including_grammar() {
        let host = r##"{
            "scopeName": "source.host",
            "patterns": [
                {"match":"hostword", "name":"keyword.host"},
                {"include":"source.external#entry"}
            ]
        }"##;
        let external = r##"{
            "scopeName": "source.external",
            "repository": {"entry": {"patterns": [{"include":"$base"}]}}
        }"##;
        let mut set = GrammarSet::new();
        let root = set.load_and_add(host).unwrap();
        set.load_and_add(external).unwrap();
        let mut tokenizer = TextMateTokenizer::new(set, root);
        let line = tokenizer.tokenize_line_scopes("hostword", TokenizerState::default());
        assert!(
            line.tokens
                .iter()
                .any(|token| { token.scopes.iter().any(|scope| scope == "keyword.host") })
        );
    }

    fn line_has_scope(line: &TokenizedLine, expected: &str) -> bool {
        line.tokens
            .iter()
            .any(|token| token.scopes.iter().any(|scope| scope == expected))
    }
}
