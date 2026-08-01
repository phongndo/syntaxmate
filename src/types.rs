use std::sync::{
    Arc, RwLock,
    atomic::{AtomicU64, AtomicUsize, Ordering, fence},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyntaxClass {
    Attribute,
    Comment,
    Constant,
    Constructor,
    Function,
    Keyword,
    Label,
    Module,
    Number,
    Operator,
    Property,
    Punctuation,
    String,
    Tag,
    Type,
    Variable,
}

/// A compact reference to one complete, ordered TextMate scope stack.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ScopeStackRef(pub(crate) u32);

/// An interned TextMate scope name.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ScopeAtomId(pub(crate) u32);

// Rendering can resolve a base theme and a post-theme scope override for each
// segment. Keep both generations warm instead of making them evict each other.
const STYLE_CACHE_SLOTS: usize = 2;
// Stable slot epochs are always even. Writers publish this sentinel while a
// slot is being reset, then advance its previous epoch by two.
const STYLE_CACHE_UPDATING_EPOCH: u64 = 1;

/// Immutable scope data shared by every line in one highlighting result.
///
/// Entry zero is always the empty stack. Keeping this table separate from
/// segments allows themes to be changed without tokenizing the source again.
#[derive(Debug)]
pub struct HighlightScopeTable {
    atoms: Vec<Arc<str>>,
    stacks: Vec<Arc<[ScopeAtomId]>>,
    resolved_styles: Vec<[AtomicU64; STYLE_CACHE_SLOTS]>,
    style_cache_generations: [AtomicU64; STYLE_CACHE_SLOTS],
    style_cache_epochs: [AtomicU64; STYLE_CACHE_SLOTS],
    style_cache_next_slot: AtomicUsize,
    style_cache_lock: RwLock<()>,
    style_cache_hits: AtomicU64,
    style_cache_misses: AtomicU64,
    style_cache_stats_enabled: bool,
}

impl Clone for HighlightScopeTable {
    fn clone(&self) -> Self {
        Self {
            atoms: self.atoms.clone(),
            stacks: self.stacks.clone(),
            resolved_styles: (0..self.stacks.len())
                .map(|_| std::array::from_fn(|_| AtomicU64::new(0)))
                .collect(),
            style_cache_generations: std::array::from_fn(|_| AtomicU64::new(0)),
            style_cache_epochs: std::array::from_fn(|_| AtomicU64::new(0)),
            style_cache_next_slot: AtomicUsize::new(0),
            style_cache_lock: RwLock::new(()),
            style_cache_hits: AtomicU64::new(0),
            style_cache_misses: AtomicU64::new(0),
            style_cache_stats_enabled: self.style_cache_stats_enabled,
        }
    }
}

impl PartialEq for HighlightScopeTable {
    fn eq(&self, other: &Self) -> bool {
        self.atoms == other.atoms && self.stacks == other.stacks
    }
}

impl Eq for HighlightScopeTable {}

impl Default for HighlightScopeTable {
    fn default() -> Self {
        Self {
            atoms: Vec::new(),
            stacks: vec![Arc::from([])],
            resolved_styles: vec![std::array::from_fn(|_| AtomicU64::new(0))],
            style_cache_generations: std::array::from_fn(|_| AtomicU64::new(0)),
            style_cache_epochs: std::array::from_fn(|_| AtomicU64::new(0)),
            style_cache_next_slot: AtomicUsize::new(0),
            style_cache_lock: RwLock::new(()),
            style_cache_hits: AtomicU64::new(0),
            style_cache_misses: AtomicU64::new(0),
            style_cache_stats_enabled: style_cache_stats_enabled(),
        }
    }
}

impl HighlightScopeTable {
    pub(crate) fn empty_shared() -> Arc<Self> {
        static EMPTY: std::sync::OnceLock<Arc<HighlightScopeTable>> = std::sync::OnceLock::new();
        Arc::clone(EMPTY.get_or_init(|| Arc::new(HighlightScopeTable::default())))
    }

    /// Builds a small standalone table for diagnostics and theme tooling.
    pub fn from_scope_names(scopes: &[&str]) -> (Self, ScopeStackRef) {
        let atoms = scopes
            .iter()
            .map(|scope| Arc::<str>::from(*scope))
            .collect::<Vec<_>>();
        let stack = (0..atoms.len())
            .map(|index| ScopeAtomId(index as u32))
            .collect::<Vec<_>>();
        (
            Self {
                atoms,
                stacks: vec![Arc::from([]), Arc::from(stack)],
                resolved_styles: (0..2)
                    .map(|_| std::array::from_fn(|_| AtomicU64::new(0)))
                    .collect(),
                style_cache_generations: std::array::from_fn(|_| AtomicU64::new(0)),
                style_cache_epochs: std::array::from_fn(|_| AtomicU64::new(0)),
                style_cache_next_slot: AtomicUsize::new(0),
                style_cache_lock: RwLock::new(()),
                style_cache_hits: AtomicU64::new(0),
                style_cache_misses: AtomicU64::new(0),
                style_cache_stats_enabled: style_cache_stats_enabled(),
            },
            ScopeStackRef(1),
        )
    }

    pub fn stack(&self, stack: ScopeStackRef) -> Option<&[ScopeAtomId]> {
        self.stacks.get(stack.0 as usize).map(AsRef::as_ref)
    }

    pub fn atom(&self, atom: ScopeAtomId) -> Option<&str> {
        self.atoms.get(atom.0 as usize).map(AsRef::as_ref)
    }

    pub fn stack_names(&self, stack: ScopeStackRef) -> impl Iterator<Item = &str> {
        self.stack(stack)
            .unwrap_or_default()
            .iter()
            .filter_map(|atom| self.atom(*atom))
    }

    pub fn stack_count(&self) -> usize {
        self.stacks.len()
    }

    pub fn atom_count(&self) -> usize {
        self.atoms.len()
    }

    pub fn memory_bytes(&self) -> usize {
        let cached_styles =
            self.resolved_styles.capacity() * std::mem::size_of::<[AtomicU64; STYLE_CACHE_SLOTS]>();
        std::mem::size_of::<Self>()
            .saturating_add(self.atoms.len() * std::mem::size_of::<Arc<str>>())
            .saturating_add(self.atoms.iter().map(|atom| atom.len()).sum::<usize>())
            .saturating_add(self.stacks.len() * std::mem::size_of::<Arc<[ScopeAtomId]>>())
            .saturating_add(
                self.stacks
                    .iter()
                    .map(|stack| stack.len() * std::mem::size_of::<ScopeAtomId>())
                    .sum::<usize>(),
            )
            .saturating_add(cached_styles)
    }

    /// Cumulative resolved-style cache hits and misses for benchmark tooling.
    pub fn style_cache_stats(&self) -> (u64, u64) {
        (
            self.style_cache_hits.load(Ordering::Relaxed),
            self.style_cache_misses.load(Ordering::Relaxed),
        )
    }

    pub(crate) fn from_parts(atoms: Vec<Arc<str>>, stacks: Vec<Arc<[ScopeAtomId]>>) -> Self {
        let stack_count = stacks.len();
        Self {
            atoms,
            stacks,
            resolved_styles: (0..stack_count)
                .map(|_| std::array::from_fn(|_| AtomicU64::new(0)))
                .collect(),
            style_cache_generations: std::array::from_fn(|_| AtomicU64::new(0)),
            style_cache_epochs: std::array::from_fn(|_| AtomicU64::new(0)),
            style_cache_next_slot: AtomicUsize::new(0),
            style_cache_lock: RwLock::new(()),
            style_cache_hits: AtomicU64::new(0),
            style_cache_misses: AtomicU64::new(0),
            style_cache_stats_enabled: style_cache_stats_enabled(),
        }
    }

    pub(crate) fn cached_style(&self, theme: u64, stack: ScopeStackRef) -> (usize, Option<u64>) {
        let (slot, style) = loop {
            // Cached rendering is overwhelmingly a read-only operation. Use
            // a monotonically advancing slot epoch for seqlock-style
            // validation so a warm segment does not acquire the table-wide
            // RwLock. Unlike the theme generation, the epoch cannot return to
            // its prior value when a slot transitions A -> C -> A.
            let mut found = None;
            for (slot, generation) in self.style_cache_generations.iter().enumerate() {
                let epoch = self.style_cache_epochs[slot].load(Ordering::Acquire);
                if epoch == STYLE_CACHE_UPDATING_EPOCH
                    || generation.load(Ordering::Acquire) != theme
                {
                    continue;
                }
                let style = self
                    .resolved_styles
                    .get(stack.0 as usize)
                    .and_then(|styles| styles[slot].load(Ordering::Acquire).checked_sub(1));
                // The acquire keeps the entry load ahead of epoch validation.
                // If it observes a late release publication, the publisher's
                // stable epoch happens-before the validation load, preventing
                // that entry from being paired with stale slot metadata.
                if self.style_cache_epochs[slot].load(Ordering::Relaxed) == epoch {
                    found = Some((slot, style));
                    break;
                }
            }
            if let Some(found) = found {
                break found;
            }

            // Only installing a new theme generation needs exclusive access.
            // `cache_style` retains a shared lock while publishing an entry,
            // so a reset cannot overwrite another generation's value.
            let _write = self
                .style_cache_lock
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if self
                .style_cache_generations
                .iter()
                .all(|generation| generation.load(Ordering::Relaxed) != theme)
            {
                let slot =
                    self.style_cache_next_slot.fetch_add(1, Ordering::Relaxed) % STYLE_CACHE_SLOTS;
                // Mark the slot unstable before clearing it. The write lock
                // serializes installers, while the stamped epoch lets
                // lock-free readers detect every reuse, including A -> C -> A.
                let previous_epoch = self.style_cache_epochs[slot]
                    .swap(STYLE_CACHE_UPDATING_EPOCH, Ordering::Acquire);
                debug_assert_ne!(previous_epoch, STYLE_CACHE_UPDATING_EPOCH);
                fence(Ordering::Release);
                for styles in &self.resolved_styles {
                    styles[slot].store(0, Ordering::Relaxed);
                }
                self.style_cache_generations[slot].store(theme, Ordering::Release);
                self.style_cache_epochs[slot]
                    .store(previous_epoch.wrapping_add(2), Ordering::Release);
            }
        };
        if self.style_cache_stats_enabled {
            if style.is_some() {
                self.style_cache_hits.fetch_add(1, Ordering::Relaxed);
            } else {
                self.style_cache_misses.fetch_add(1, Ordering::Relaxed);
            }
        }
        // Return the slot even on a miss so cache_style can populate exactly
        // the generation reserved by this lookup.
        (slot, style)
    }

    pub(crate) fn cache_style(&self, theme: u64, stack: ScopeStackRef, slot: usize, style: u64) {
        let _read = self
            .style_cache_lock
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.style_cache_generations[slot].load(Ordering::Acquire) == theme
            && let Some(entries) = self.resolved_styles.get(stack.0 as usize)
        {
            // Readers validate the slot without taking the lock. Publish with
            // release ordering so a reader that observes this entry cannot
            // validate it against generation metadata from before this slot
            // was installed.
            entries[slot].store(style + 1, Ordering::Release);
        }
    }
}

const fn style_cache_stats_enabled() -> bool {
    cfg!(feature = "diagnostics")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxSegment {
    pub byte_start: usize,
    pub byte_end: usize,
    pub class: Option<SyntaxClass>,
    /// Exact TextMate scopes. `class` is retained only as a coarse fallback.
    pub scope_stack: ScopeStackRef,
}

impl SyntaxSegment {
    pub fn new(byte_start: usize, byte_end: usize, class: Option<SyntaxClass>) -> Self {
        debug_assert!(byte_start <= byte_end);
        Self {
            byte_start,
            byte_end,
            class,
            scope_stack: ScopeStackRef::default(),
        }
    }

    pub fn with_scope_stack(mut self, scope_stack: ScopeStackRef) -> Self {
        self.scope_stack = scope_stack;
        self
    }

    pub fn len(&self) -> usize {
        self.byte_end.saturating_sub(self.byte_start)
    }

    pub fn is_empty(&self) -> bool {
        self.byte_start >= self.byte_end
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightedLine {
    pub fingerprint: LineTextFingerprint,
    pub segments: Vec<SyntaxSegment>,
    pub scope_table: Arc<HighlightScopeTable>,
}

impl Default for HighlightedLine {
    fn default() -> Self {
        Self::new("")
    }
}

impl HighlightedLine {
    pub fn new(text: &str) -> Self {
        Self {
            fingerprint: LineTextFingerprint::from_text(text),
            segments: Vec::new(),
            scope_table: HighlightScopeTable::empty_shared(),
        }
    }

    pub fn matches_text(&self, text: &str) -> bool {
        self.fingerprint.matches(text)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LineTextFingerprint {
    byte_len: usize,
    hash: u64,
}

impl Default for LineTextFingerprint {
    fn default() -> Self {
        Self::from_text("")
    }
}

impl LineTextFingerprint {
    pub fn from_text(text: &str) -> Self {
        Self {
            byte_len: text.len(),
            hash: stable_text_hash(text.as_bytes()),
        }
    }

    pub fn byte_len(self) -> usize {
        self.byte_len
    }

    pub fn matches(self, text: &str) -> bool {
        self.byte_len == text.len() && self.hash == stable_text_hash(text.as_bytes())
    }

    pub(crate) fn without_trailing_byte(self, byte: u8) -> Self {
        // FNV-1a update is `(hash ^ byte) * PRIME`. PRIME is odd, so it has a
        // multiplicative inverse modulo 2^64 and the final byte can be removed
        // without hashing the line a second time.
        const PRIME_INVERSE: u64 = 0xce96_5057_aff6_957b;
        Self {
            byte_len: self.byte_len.saturating_sub(1),
            hash: self.hash.wrapping_mul(PRIME_INVERSE) ^ u64::from(byte),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightedText {
    pub lines: Vec<HighlightedLine>,
}

fn stable_text_hash(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

pub const DEFAULT_MAX_LINE_BYTES: usize = 8 * 1024;
pub const DEFAULT_LINE_CACHE_ENTRIES: usize = 32_768;

/// Resource options applied to one TextMate tokenizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenizerOptions {
    /// Maximum source-line size accepted by the tokenizer.
    pub max_line_bytes: usize,
    /// Maximum number of tokenized lines retained in the tokenizer-local cache.
    pub line_cache_entries: usize,
}

impl Default for TokenizerOptions {
    fn default() -> Self {
        Self {
            max_line_bytes: DEFAULT_MAX_LINE_BYTES,
            line_cache_entries: DEFAULT_LINE_CACHE_ENTRIES,
        }
    }
}

/// One caller-supplied TextMate theme rule.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Deserialize)]
pub struct ThemeRule {
    pub scope: String,
    pub foreground: Option<String>,
    pub background: Option<String>,
    pub font_style: Option<String>,
}
