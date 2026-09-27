//! Ordered backtracking bytecode, with an optional compact capture layout.
//!
//! The program is immutable and compiled from the shared [`ParsedRegex`].
//! Mutable DFS, assertion, and repeat state lives in [`BytecodeScratch`], so a
//! caller can reuse its allocations across candidate attempts.

use super::analysis::RegexAnalysis;
use super::ast::{
    Ast, Backref, CharClass, ClassAtom, LookKind, ParsedRegex, PerlClassKind, RegexFlags,
};
use super::backtrack::{
    BudgetExceeded, CaseFoldKey, StepBudget, anchor_matches, char_at, class_contains,
    class_positive_contains, is_cpp_space_comment_separator, literal_byte_width, match_literal_end,
    previous_char, unicode_case_eq,
};
use super::case_fold::CaseVariants;
use super::{AnchorContext, is_unicode_word_char};
use std::{
    borrow::Cow,
    ops::Range,
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CompileError {
    Backreference,
    Conditional,
    Subroutine,
    Unsupported,
    TableOverflow,
}

type ProgramCounter = u32;
type VmSlot = u32;

const INVALID_PROGRAM_COUNTER: ProgramCounter = ProgramCounter::MAX;
const UNBOUNDED_COUNT: u32 = u32::MAX;

#[derive(Debug, Clone)]
pub(crate) struct Program {
    instructions: Vec<Instruction>,
    literals: Vec<String>,
    literal_tries: Vec<LiteralTrie>,
    classes: Vec<CompiledClass>,
    entry: ProgramCounter,
    repeat_slots: VmSlot,
    /// Regex group numbers indexed by their compact VM slot. Position-only
    /// programs leave this empty. Group zero is always slot zero when present.
    capture_layout: Vec<u32>,
    /// First-character guards: one per `Split` (indexed by its `guard`
    /// operand) followed by one per repeat slot (from `repeat_guard_base`).
    guards: Box<[GuardCell]>,
    repeat_guard_base: u32,
}

/// Lazily derived first-character guard for a branch entry (a `Split`'s
/// preferred branch or a loop body).
///
/// Most alternation branches and optional/repeated groups fail on their
/// first character; without a guard each such attempt costs a backtrack
/// frame push, the failing instruction(s), and a pop with undo replay. The
/// guard is derived from the bytecode on the first execution that reaches
/// it rather than at compile time, because large grammars compile thousands
/// of branches that never run in a cold process. Racing threads derive the
/// same value, so release/acquire publication suffices.
///
/// The two words are an ASCII byte mask whose bits for bytes 0x00 and 0x01
/// are repurposed as the derivation state; those two bytes are always
/// allowed, which only forgoes skipping on them.
#[derive(Debug, Default)]
struct GuardCell([AtomicU64; 2]);

impl GuardCell {
    const STATE_MASK: u64 = 0b11;
    const UNKNOWN: u64 = 0;
    /// The entry may succeed without consuming, or starts with anything.
    const OPEN: u64 = 1;
    /// Must consume an ASCII byte from the mask.
    const ASCII: u64 = 2;
    /// Must consume an ASCII byte from the mask or some non-ASCII scalar.
    const ASCII_OR_NON_ASCII: u64 = 3;

    fn cells(count: usize) -> Box<[Self]> {
        (0..count).map(|_| Self::default()).collect()
    }

    /// False only when the guarded entry provably fails at `position`
    /// before consuming anything.
    #[inline]
    fn allows(
        &self,
        program: &Program,
        entry: ProgramCounter,
        line: &str,
        position: usize,
    ) -> bool {
        let mut low = self.0[0].load(Ordering::Acquire);
        if low & Self::STATE_MASK == Self::UNKNOWN {
            low = self.derive(program, entry);
        }
        let state = low & Self::STATE_MASK;
        if state == Self::OPEN {
            return true;
        }
        match line.as_bytes().get(position).copied() {
            Some(0..=1) => true,
            Some(byte @ 2..64) => low & (1 << byte) != 0,
            Some(byte @ 64..128) => self.0[1].load(Ordering::Relaxed) & (1 << (byte - 64)) != 0,
            Some(_) => state == Self::ASCII_OR_NON_ASCII,
            // A path that must consume cannot succeed at the end of the line.
            None => false,
        }
    }

    #[cold]
    #[inline(never)]
    fn derive(&self, program: &Program, entry: ProgramCounter) -> u64 {
        let mut steps = GUARD_WALK_STEPS;
        let low = match program.must_consume_first_chars(entry, &mut steps) {
            Some(first) if first != FirstChars::ALL => {
                self.0[1].store(first.ascii[1], Ordering::Relaxed);
                let state = if first.non_ascii == 0 {
                    Self::ASCII
                } else {
                    Self::ASCII_OR_NON_ASCII
                };
                (first.ascii[0] & !Self::STATE_MASK) | state
            }
            _ => Self::OPEN,
        };
        self.0[0].store(low, Ordering::Release);
        low
    }

    fn state(&self) -> u64 {
        self.0[0].load(Ordering::Acquire) & Self::STATE_MASK
    }
}

impl Clone for GuardCell {
    fn clone(&self) -> Self {
        let low = self.0[0].load(Ordering::Acquire);
        Self([
            AtomicU64::new(low),
            AtomicU64::new(self.0[1].load(Ordering::Relaxed)),
        ])
    }
}

/// Bounds the bytecode walked when deriving a guard; giving up only leaves
/// the entry unguarded.
const GUARD_WALK_STEPS: u32 = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // Vertical-slice API; backtrack/tokenizer integration follows.
pub(crate) struct CaptureMatch {
    pub(crate) end: usize,
    /// Compact captures in the order returned by [`Program::capture_layout`].
    pub(crate) captures: Vec<Option<Range<usize>>>,
}

#[derive(Debug, Clone, Copy)]
struct LiteralId(u32);

#[derive(Debug, Clone, Copy)]
struct ClassId(u32);

#[derive(Debug, Clone, Copy)]
struct LiteralTrieId(u32);

/// Four parsed regex booleans packed into the bytecode operand itself.
/// Parsing keeps the ergonomic public `RegexFlags`; the VM should not pay four
/// bytes in every instruction that happens to consume one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InstructionFlags(u8);

impl InstructionFlags {
    const CASE_INSENSITIVE: u8 = 1 << 0;
    const MULTI_LINE: u8 = 1 << 1;
    const DOT_MATCHES_NEW_LINE: u8 = 1 << 2;
    const IGNORE_WHITESPACE: u8 = 1 << 3;

    fn case_insensitive(self) -> bool {
        self.0 & Self::CASE_INSENSITIVE != 0
    }

    fn dot_matches_new_line(self) -> bool {
        self.0 & Self::DOT_MATCHES_NEW_LINE != 0
    }

    fn regex(self) -> RegexFlags {
        RegexFlags {
            case_insensitive: self.case_insensitive(),
            multi_line: self.0 & Self::MULTI_LINE != 0,
            dot_matches_new_line: self.dot_matches_new_line(),
            ignore_whitespace: self.0 & Self::IGNORE_WHITESPACE != 0,
        }
    }
}

impl From<RegexFlags> for InstructionFlags {
    fn from(flags: RegexFlags) -> Self {
        Self(
            (u8::from(flags.case_insensitive) * Self::CASE_INSENSITIVE)
                | (u8::from(flags.multi_line) * Self::MULTI_LINE)
                | (u8::from(flags.dot_matches_new_line) * Self::DOT_MATCHES_NEW_LINE)
                | (u8::from(flags.ignore_whitespace) * Self::IGNORE_WHITESPACE),
        )
    }
}

/// Inclusive repeat bounds. `u32::MAX` is reserved for an unbounded maximum;
/// compilation rejects larger source operands rather than silently truncating.
#[derive(Debug, Clone, Copy)]
struct RepeatBounds {
    min: u32,
    max: u32,
}

impl RepeatBounds {
    fn new(min: usize, max: Option<usize>) -> Result<Self, CompileError> {
        let min = u32::try_from(min).map_err(|_| CompileError::TableOverflow)?;
        let max = match max {
            Some(max) => {
                let max = u32::try_from(max).map_err(|_| CompileError::TableOverflow)?;
                if max == UNBOUNDED_COUNT {
                    return Err(CompileError::TableOverflow);
                }
                max
            }
            None => UNBOUNDED_COUNT,
        };
        Ok(Self { min, max })
    }

    fn max(self) -> Option<u32> {
        (self.max != UNBOUNDED_COUNT).then_some(self.max)
    }
}

fn program_counter(index: usize) -> Result<ProgramCounter, CompileError> {
    let index = u32::try_from(index).map_err(|_| CompileError::TableOverflow)?;
    (index != INVALID_PROGRAM_COUNTER)
        .then_some(index)
        .ok_or(CompileError::TableOverflow)
}

fn vm_slot(index: usize) -> Result<VmSlot, CompileError> {
    // The top bit is reserved for `RepeatUndo`'s stalled flag and the slot
    // below it for call credits.
    u32::try_from(index)
        .ok()
        .filter(|slot| *slot < CREDIT_UNDO_SLOT)
        .ok_or(CompileError::TableOverflow)
}

fn arena_mark(index: usize) -> Result<u32, BudgetExceeded> {
    u32::try_from(index).map_err(|_| BudgetExceeded)
}

fn arena_index(index: u32) -> usize {
    index as usize
}

// Keep speculative trie allocation proportional for small inventories while
// bounding over-reservation when many branches share the same prefixes.
const LITERAL_TRIE_NODE_RESERVE_LIMIT: usize = 4 * 1024;

/// Ordered trie for an alternation whose branches are all exact literals.
///
/// A normal bytecode alternation tests every branch prefix independently.
/// Large keyword expressions in the C/C++ and TypeScript grammars contain
/// hundreds of branches, so that duplicates both dispatch and byte compares.
/// Terminals retain the original branch order because Oniguruma chooses the
/// first matching alternative, not necessarily the longest one.
///
/// Byte tries keep each node's outgoing edges as one contiguous, byte-sorted
/// run of shared edge arrays, so building a keyword inventory with thousands
/// of branches performs a handful of allocations rather than one per
/// branching node.
#[derive(Debug, Clone, Default)]
struct LiteralTrie {
    nodes: Vec<LiteralTrieNode>,
    edge_bytes: Vec<u8>,
    edge_targets: Vec<u32>,
    unicode_nodes: Vec<UnicodeLiteralTrieNode>,
}

#[derive(Debug, Clone, Default)]
enum LiteralTrieEdges<T> {
    #[default]
    Empty,
    One((T, u32)),
    Many(Vec<(T, u32)>),
}

impl<T: Copy> LiteralTrieEdges<T> {
    fn iter(&self) -> std::slice::Iter<'_, (T, u32)> {
        match self {
            Self::Empty => [].iter(),
            Self::One(edge) => std::slice::from_ref(edge).iter(),
            Self::Many(edges) => edges.iter(),
        }
    }

    fn push(&mut self, edge: (T, u32)) {
        match self {
            Self::Empty => *self = Self::One(edge),
            Self::One(first) => *self = Self::Many(vec![*first, edge]),
            Self::Many(edges) => edges.push(edge),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct LiteralTrieNode {
    edge_start: u32,
    edge_len: u32,
    terminal_order: Option<u32>,
}

/// Sentinel for an absent child while a byte trie is being built.
const NO_TRIE_NODE: u32 = u32::MAX;

/// Construction-only child links of one byte-trie node.
#[derive(Clone, Copy)]
struct TrieBuildLink {
    first_child: u32,
    next_sibling: u32,
    byte: u8,
}

impl Default for TrieBuildLink {
    fn default() -> Self {
        Self {
            first_child: NO_TRIE_NODE,
            next_sibling: NO_TRIE_NODE,
            byte: 0,
        }
    }
}

/// Incremental byte-trie construction. Children are linked first-child /
/// next-sibling (with a dense root table) so that no node owns an edge
/// allocation; `finish` lays every node's children out as one sorted run of
/// the shared edge arrays.
struct ByteTrieBuilder {
    nodes: Vec<LiteralTrieNode>,
    links: Vec<TrieBuildLink>,
    root_children: [u32; 256],
    case_insensitive: bool,
    /// Order of the next string emitted through [`FiniteSink`].
    next_order: u32,
    /// Nodes along the previous [`Self::insert`]ed literal and its folded
    /// bytes. Keyword inventories are usually sorted, so each literal resumes
    /// below its common prefix with the previous one instead of re-walking
    /// sibling lists from the root.
    path: Vec<u32>,
    previous: Vec<u8>,
    current: Vec<u8>,
}

impl ByteTrieBuilder {
    fn new(node_capacity: usize, case_insensitive: bool) -> Self {
        let node_capacity = node_capacity.min(LITERAL_TRIE_NODE_RESERVE_LIMIT);
        let mut nodes = Vec::with_capacity(node_capacity);
        let mut links = Vec::with_capacity(node_capacity);
        nodes.push(LiteralTrieNode::default());
        links.push(TrieBuildLink::default());
        Self {
            nodes,
            links,
            root_children: [NO_TRIE_NODE; 256],
            case_insensitive,
            next_order: 0,
            path: vec![0],
            previous: Vec::new(),
            current: Vec::new(),
        }
    }

    /// The node reached by `literal`, created as needed.
    fn insert(&mut self, literal: &str) -> Result<u32, CompileError> {
        let mut current = std::mem::take(&mut self.current);
        current.clear();
        current.extend_from_slice(literal.as_bytes());
        if self.case_insensitive {
            current.make_ascii_lowercase();
        }
        let common = current
            .iter()
            .zip(&self.previous)
            .take_while(|(current, previous)| current == previous)
            .count();
        self.path.truncate(common + 1);
        let mut node = self.path[common];
        for &byte in &current[common..] {
            node = self.child(node, byte)?;
            self.path.push(node);
        }
        self.current = std::mem::replace(&mut self.previous, current);
        Ok(node)
    }

    /// The child of `node` along `byte`, created when missing.
    fn child(&mut self, node: u32, mut byte: u8) -> Result<u32, CompileError> {
        if self.case_insensitive {
            byte.make_ascii_lowercase();
        }
        let parent = node as usize;
        let existing = if parent == 0 {
            self.root_children[byte as usize]
        } else {
            let mut child = self.links[parent].first_child;
            while child != NO_TRIE_NODE && self.links[child as usize].byte != byte {
                child = self.links[child as usize].next_sibling;
            }
            child
        };
        if existing != NO_TRIE_NODE {
            return Ok(existing);
        }
        let child = u32::try_from(self.nodes.len()).map_err(|_| CompileError::TableOverflow)?;
        if child == NO_TRIE_NODE {
            return Err(CompileError::TableOverflow);
        }
        self.nodes.push(LiteralTrieNode::default());
        self.links.push(TrieBuildLink {
            first_child: NO_TRIE_NODE,
            next_sibling: self.links[parent].first_child,
            byte,
        });
        self.links[parent].first_child = child;
        self.nodes[parent].edge_len += 1;
        if parent == 0 {
            self.root_children[byte as usize] = child;
        }
        Ok(child)
    }

    /// Duplicate strings keep their earliest (preferred) order.
    fn mark_terminal(&mut self, node: u32, order: u32) {
        let terminal = &mut self.nodes[node as usize].terminal_order;
        if terminal.is_none_or(|existing| order < existing) {
            *terminal = Some(order);
        }
    }

    fn finish(self) -> LiteralTrie {
        let Self {
            mut nodes, links, ..
        } = self;
        let mut edge_start = 0u32;
        for node in &mut nodes {
            node.edge_start = edge_start;
            edge_start += node.edge_len;
        }
        let mut edge_bytes = vec![0; edge_start as usize];
        let mut edge_targets = vec![0; edge_start as usize];
        for (node, link) in nodes.iter().zip(&links) {
            let start = node.edge_start as usize;
            let end = start + node.edge_len as usize;
            // Sibling links run newest first; fill the run from its end so
            // children created in ascending byte order need no sort.
            let mut child = link.first_child;
            let mut sorted = true;
            for slot in (start..end).rev() {
                let byte = links[child as usize].byte;
                sorted &= slot + 1 == end || byte < edge_bytes[slot + 1];
                edge_bytes[slot] = byte;
                edge_targets[slot] = child;
                child = links[child as usize].next_sibling;
            }
            if !sorted {
                let mut run = edge_bytes[start..end]
                    .iter()
                    .copied()
                    .zip(edge_targets[start..end].iter().copied())
                    .collect::<Vec<_>>();
                run.sort_unstable_by_key(|(byte, _)| *byte);
                for (slot, (byte, target)) in (start..).zip(run) {
                    edge_bytes[slot] = byte;
                    edge_targets[slot] = target;
                }
            }
        }
        LiteralTrie {
            nodes,
            edge_bytes,
            edge_targets,
            unicode_nodes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct UnicodeLiteralTrieNode {
    // Edge scalars carry their case mappings so a lookup maps the input
    // scalar once instead of once per edge comparison.
    edges: LiteralTrieEdges<CaseFoldKey>,
    terminal_order: Option<u32>,
}

/// A character class with exact ASCII membership bitmaps, shared by the
/// bytecode VM and the multi-pattern scanner.
#[derive(Debug, Clone)]
pub(crate) struct CompiledClass {
    pub(crate) source: CharClass,
    ascii_sensitive: [u64; 2],
    ascii_insensitive: [u64; 2],
}

impl CompiledClass {
    pub(crate) fn new(source: CharClass) -> Self {
        // Build the ASCII bitmaps per atom instead of evaluating the whole
        // class 128 × 2 times: the per-character evaluation runs Unicode case
        // conversions for every probe and dominated one-shot grammar compile
        // time on C-family grammars.
        let (ascii_sensitive, ascii_insensitive) = ascii_class_masks(&source);
        debug_assert_eq!(
            (ascii_sensitive, ascii_insensitive),
            ascii_masks_by_evaluation(&source),
            "atom-mask construction must agree with class_contains for {source:?}",
        );
        Self {
            source,
            ascii_sensitive,
            ascii_insensitive,
        }
    }

    /// Membership of a non-ASCII scalar under `flags`.
    #[inline]
    fn matches_char(&self, ch: char, flags: RegexFlags) -> bool {
        class_contains(&self.source, ch, flags)
    }

    #[inline]
    pub(crate) fn matches_ascii(&self, byte: u8, case_insensitive: bool) -> bool {
        debug_assert!(byte < 128);
        let bitmap = if case_insensitive {
            &self.ascii_insensitive
        } else {
            &self.ascii_sensitive
        };
        bitmap[byte as usize / 64] & (1u64 << (byte % 64)) != 0
    }
}

/// Conservative set of characters that can start a match, used to prove
/// that a greedy repeat never needs to give characters back. ASCII is exact
/// per byte; non-ASCII is tracked only as whitespace versus everything else,
/// which is enough to separate identifier and number classes from spacing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FirstChars {
    ascii: AsciiMask,
    non_ascii: u8,
}

impl FirstChars {
    const NON_ASCII_SPACE: u8 = 1 << 0;
    const NON_ASCII_OTHER: u8 = 1 << 1;
    const NON_ASCII_ANY: u8 = Self::NON_ASCII_SPACE | Self::NON_ASCII_OTHER;
    const NONE: Self = Self {
        ascii: [0; 2],
        non_ascii: 0,
    };
    const ALL: Self = Self {
        ascii: [u64::MAX; 2],
        non_ascii: Self::NON_ASCII_ANY,
    };

    fn union(&mut self, other: &Self) {
        self.ascii[0] |= other.ascii[0];
        self.ascii[1] |= other.ascii[1];
        self.non_ascii |= other.non_ascii;
    }

    fn intersect(&mut self, other: &Self) {
        self.ascii[0] &= other.ascii[0];
        self.ascii[1] &= other.ascii[1];
        self.non_ascii &= other.non_ascii;
    }

    fn is_disjoint(&self, other: &Self) -> bool {
        self.ascii[0] & other.ascii[0] == 0
            && self.ascii[1] & other.ascii[1] == 0
            && self.non_ascii & other.non_ascii == 0
    }

    fn non_ascii_kind(ch: char) -> u8 {
        debug_assert!(!ch.is_ascii());
        if ch.is_whitespace() {
            Self::NON_ASCII_SPACE
        } else {
            Self::NON_ASCII_OTHER
        }
    }

    fn of_literal(literal: &str, case_insensitive: bool) -> Self {
        let Some(first) = literal.chars().next() else {
            return Self::NONE;
        };
        let mut chars = Self::NONE;
        if !first.is_ascii() {
            if case_insensitive {
                // Full case folding can match multi-character ASCII text.
                return Self::ALL;
            }
            chars.non_ascii = Self::non_ascii_kind(first);
            return chars;
        }
        let byte = first as u8;
        ascii_mask_set(&mut chars.ascii, byte);
        if case_insensitive {
            ascii_mask_set(&mut chars.ascii, byte.to_ascii_lowercase());
            ascii_mask_set(&mut chars.ascii, byte.to_ascii_uppercase());
            // Letters such as the Kelvin sign and long s fold to ASCII.
            chars.non_ascii = Self::NON_ASCII_OTHER;
        }
        chars
    }

    fn of_class(class: &CompiledClass, case_insensitive: bool) -> Self {
        Self {
            ascii: if case_insensitive {
                class.ascii_insensitive
            } else {
                class.ascii_sensitive
            },
            non_ascii: class_non_ascii_kinds(&class.source, case_insensitive),
        }
    }

    /// First characters of an AST node and whether it can match empty.
    fn of_ast(ast: &Ast, flags: RegexFlags) -> (Self, bool) {
        match ast {
            Ast::Empty | Ast::Anchor(_) | Ast::Look { .. } => (Self::NONE, true),
            Ast::Literal(literal) => (
                Self::of_literal(literal, flags.case_insensitive),
                literal.is_empty(),
            ),
            Ast::Dot | Ast::Grapheme => (Self::ALL, false),
            Ast::Class(class) => {
                let (sensitive, insensitive) = ascii_class_masks(class);
                let chars = Self {
                    ascii: if flags.case_insensitive {
                        insensitive
                    } else {
                        sensitive
                    },
                    non_ascii: class_non_ascii_kinds(class, flags.case_insensitive),
                };
                (chars, false)
            }
            Ast::Concat(nodes) => {
                let mut chars = Self::NONE;
                for node in nodes {
                    let (first, nullable) = Self::of_ast(node, flags);
                    chars.union(&first);
                    if !nullable {
                        return (chars, false);
                    }
                }
                (chars, true)
            }
            Ast::Alternation(branches) => {
                let mut chars = Self::NONE;
                let mut nullable = false;
                for branch in branches {
                    let (first, branch_nullable) = Self::of_ast(branch, flags);
                    chars.union(&first);
                    nullable |= branch_nullable;
                }
                (chars, nullable)
            }
            Ast::Repeat { node, min, max, .. } => {
                if *max == Some(0) {
                    return (Self::NONE, true);
                }
                let (chars, nullable) = Self::of_ast(node, flags);
                (chars, nullable || *min == 0)
            }
            Ast::Group { child, .. } => Self::of_ast(child, flags),
            Ast::Flags {
                flags: local,
                child,
            } => Self::of_ast(child, *local),
            Ast::Backref(_)
            | Ast::Subroutine(_)
            | Ast::Conditional { .. }
            | Ast::Unsupported(_) => (Self::ALL, true),
        }
    }
}

/// Non-ASCII kinds a class can match. Intersections only narrow the first
/// union, so ignoring them stays conservative.
fn class_non_ascii_kinds(class: &CharClass, case_insensitive: bool) -> u8 {
    if class.negated {
        return FirstChars::NON_ASCII_ANY;
    }
    // A folded bracketed class also matches the case variants of its members,
    // such as the Kelvin sign for `(?i)[[:ascii:]]`; case variants are never
    // whitespace.
    let folded = if case_insensitive && class.bracketed {
        FirstChars::NON_ASCII_OTHER
    } else {
        0
    };
    class.atoms.iter().fold(folded, |kinds, atom| {
        kinds | atom_non_ascii_kinds(atom, case_insensitive)
    })
}

fn atom_non_ascii_kinds(atom: &ClassAtom, case_insensitive: bool) -> u8 {
    // Case-insensitive ASCII atoms can match non-ASCII letters that fold to
    // them; whitespace has no case mapping.
    let folded = if case_insensitive {
        FirstChars::NON_ASCII_OTHER
    } else {
        0
    };
    match atom {
        ClassAtom::Char(ch) if ch.is_ascii() => folded,
        ClassAtom::Char(ch) if !case_insensitive => FirstChars::non_ascii_kind(*ch),
        ClassAtom::Range(start, end) if start.is_ascii() && end.is_ascii() => folded,
        ClassAtom::Char(_) | ClassAtom::Range(..) => FirstChars::NON_ASCII_ANY,
        ClassAtom::Perl(kind) => match kind {
            PerlClassKind::Digit
            | PerlClassKind::HorizontalSpace
            | PerlClassKind::VerticalSpace => 0,
            PerlClassKind::Space => FirstChars::NON_ASCII_SPACE,
            PerlClassKind::NotSpace | PerlClassKind::Word => FirstChars::NON_ASCII_OTHER,
            PerlClassKind::NotDigit
            | PerlClassKind::NotWord
            | PerlClassKind::NotHorizontalSpace
            | PerlClassKind::NotVerticalSpace
            | PerlClassKind::NotNewline => FirstChars::NON_ASCII_ANY,
        },
        ClassAtom::Posix {
            name,
            negated: false,
        } => posix_non_ascii_kinds(name),
        ClassAtom::Posix { negated: true, .. } | ClassAtom::Unicode { .. } => {
            FirstChars::NON_ASCII_ANY
        }
        ClassAtom::Nested(class) => class_non_ascii_kinds(class, case_insensitive),
    }
}

/// Non-ASCII kinds matched by a POSIX class, mirroring
/// `backtrack::posix_class_predicate` (unknown names match nothing).
fn posix_non_ascii_kinds(name: &str) -> u8 {
    const SPACE: u8 = FirstChars::NON_ASCII_SPACE;
    const OTHER: u8 = FirstChars::NON_ASCII_OTHER;
    const KINDS: [(&str, u8); 14] = [
        ("alnum", OTHER),
        ("alpha", OTHER),
        ("ascii", 0),
        ("blank", 0),
        ("cntrl", SPACE | OTHER),
        ("digit", 0),
        ("graph", OTHER),
        ("lower", OTHER),
        ("print", SPACE | OTHER),
        ("punct", 0),
        ("space", SPACE),
        ("upper", OTHER),
        ("word", OTHER),
        ("xdigit", 0),
    ];
    KINDS
        .iter()
        .find(|(class, _)| class.eq_ignore_ascii_case(name))
        .map_or(0, |(_, kinds)| *kinds)
}

type AsciiMask = [u64; 2];

fn ascii_mask_set(mask: &mut AsciiMask, byte: u8) {
    debug_assert!(byte < 128);
    mask[byte as usize / 64] |= 1u64 << (byte % 64);
}

/// Exact ASCII membership bitmaps (case-sensitive, case-insensitive) for a
/// class, mirroring `class_contains` on `0..=127`.
pub(crate) fn ascii_class_masks(class: &CharClass) -> (AsciiMask, AsciiMask) {
    let positive = ascii_positive_mask(class);
    if !class.bracketed {
        let mask = if class.negated {
            ascii_mask_complement(positive)
        } else {
            positive
        };
        return (mask, mask);
    }
    // Folding admits a probe when a case variant is in the literal set: its
    // other-case letter, or the Kelvin sign and long s for `k` and `s`.
    let mut folded = [
        // Word 0 holds no letters.
        positive[0],
        // 'a'..='z' sit exactly 32 bits above 'A'..='Z' in word 1.
        positive[1]
            | ((positive[1] & ASCII_LOWER_MASK[1]) >> 32)
            | ((positive[1] & ASCII_UPPER_MASK[1]) << 32),
    ];
    for (variant, letter) in [('\u{212a}', b'k'), ('\u{17f}', b's')] {
        if class_positive_contains(class, variant) {
            ascii_mask_set(&mut folded, letter);
            ascii_mask_set(&mut folded, letter.to_ascii_uppercase());
        }
    }
    if class.negated {
        (
            ascii_mask_complement(positive),
            ascii_mask_complement(folded),
        )
    } else {
        (positive, folded)
    }
}

/// Case-sensitive ASCII members of `class` ignoring its top-level negation.
fn ascii_positive_mask(class: &CharClass) -> AsciiMask {
    let union = |atoms: &[ClassAtom]| {
        atoms
            .iter()
            .map(ascii_atom_mask)
            .fold([0u64; 2], |mask, atom| {
                [mask[0] | atom[0], mask[1] | atom[1]]
            })
    };
    class
        .intersections
        .iter()
        .fold(union(&class.atoms), |mask, atoms| {
            let term = union(atoms);
            [mask[0] & term[0], mask[1] & term[1]]
        })
}

fn ascii_atom_mask(atom: &ClassAtom) -> AsciiMask {
    match atom {
        ClassAtom::Char(ch) if ch.is_ascii() => {
            let mut mask = [0u64; 2];
            ascii_mask_set(&mut mask, *ch as u8);
            mask
        }
        ClassAtom::Char(_) => [0; 2],
        ClassAtom::Range(start, end) if start.is_ascii() => {
            ascii_bounded_range_mask(*start as u8, (*end).min('\x7f') as u8)
        }
        ClassAtom::Range(..) => [0; 2],
        ClassAtom::Perl(kind) => perl_ascii_mask(*kind),
        ClassAtom::Posix { name, negated } => {
            let mask = posix_ascii_mask(name);
            if *negated {
                ascii_mask_complement(mask)
            } else {
                mask
            }
        }
        ClassAtom::Unicode { name, negated } => ascii_predicate_mask(|ch| {
            super::backtrack::unicode_class_contains(name, ch) != *negated
        }),
        ClassAtom::Nested(class) => ascii_class_masks(class).0,
    }
}

const fn ascii_range_mask(low: u8, high: u8) -> AsciiMask {
    let mut mask = [0u64; 2];
    let mut byte = low;
    while byte <= high {
        mask[byte as usize / 64] |= 1u64 << (byte % 64);
        byte += 1;
    }
    mask
}

const fn ascii_mask_union(left: AsciiMask, right: AsciiMask) -> AsciiMask {
    [left[0] | right[0], left[1] | right[1]]
}

const fn ascii_mask_complement(mask: AsciiMask) -> AsciiMask {
    [!mask[0], !mask[1]]
}

const ASCII_DIGIT_MASK: AsciiMask = ascii_range_mask(b'0', b'9');
const ASCII_UPPER_MASK: AsciiMask = ascii_range_mask(b'A', b'Z');
const ASCII_LOWER_MASK: AsciiMask = ascii_range_mask(b'a', b'z');

/// `low..=high` as a mask, empty when the bounds are reversed.
fn ascii_bounded_range_mask(low: u8, high: u8) -> AsciiMask {
    if low > high {
        return [0; 2];
    }
    let bits = |word: u8| {
        let (start, end) = (u32::from(word) * 64, u32::from(word) * 64 + 63);
        let (from, to) = (u32::from(low).max(start), u32::from(high).min(end));
        if from > to {
            0
        } else {
            (u64::MAX >> (63 - (to - from))) << (from - start)
        }
    };
    [bits(0), bits(1)]
}
const ASCII_HEX_DIGIT_MASK: AsciiMask = ascii_mask_union(
    ASCII_DIGIT_MASK,
    ascii_mask_union(ascii_range_mask(b'A', b'F'), ascii_range_mask(b'a', b'f')),
);
/// `char::is_whitespace` on ASCII: U+0009..=U+000D and space.
const ASCII_SPACE_MASK: AsciiMask =
    ascii_mask_union(ascii_range_mask(b'\t', b'\r'), ascii_range_mask(b' ', b' '));
const ASCII_WORD_MASK: AsciiMask = ascii_mask_union(
    ascii_mask_union(ASCII_DIGIT_MASK, ascii_range_mask(b'_', b'_')),
    ascii_mask_union(ascii_range_mask(b'A', b'Z'), ascii_range_mask(b'a', b'z')),
);
const ASCII_ALPHA_MASK: AsciiMask =
    ascii_mask_union(ascii_range_mask(b'A', b'Z'), ascii_range_mask(b'a', b'z'));
const ASCII_CONTROL_MASK: AsciiMask =
    ascii_mask_union(ascii_range_mask(0, 0x1f), ascii_range_mask(0x7f, 0x7f));
/// ASCII members of each POSIX bracket class, in the order and with the
/// predicates of `backtrack::posix_class_predicate`.
const POSIX_ASCII_MASKS: [(&str, AsciiMask); 14] = [
    (
        "alnum",
        ascii_mask_union(ASCII_ALPHA_MASK, ASCII_DIGIT_MASK),
    ),
    ("alpha", ASCII_ALPHA_MASK),
    ("ascii", [u64::MAX; 2]),
    (
        "blank",
        ascii_mask_union(ascii_range_mask(b'\t', b'\t'), ascii_range_mask(b' ', b' ')),
    ),
    ("cntrl", ASCII_CONTROL_MASK),
    ("digit", ASCII_DIGIT_MASK),
    ("graph", ascii_range_mask(b'!', b'~')),
    ("lower", ascii_range_mask(b'a', b'z')),
    ("print", ascii_range_mask(b' ', b'~')),
    (
        "punct",
        ascii_mask_union(
            ascii_mask_union(ascii_range_mask(b'!', b'/'), ascii_range_mask(b':', b'@')),
            ascii_mask_union(ascii_range_mask(b'[', b'`'), ascii_range_mask(b'{', b'~')),
        ),
    ),
    ("space", ASCII_SPACE_MASK),
    ("upper", ascii_range_mask(b'A', b'Z')),
    ("word", ASCII_WORD_MASK),
    ("xdigit", ASCII_HEX_DIGIT_MASK),
];

/// ASCII members of a POSIX class, resolving names like
/// `posix_class_predicate`: unknown names match nothing.
fn posix_ascii_mask(name: &str) -> AsciiMask {
    POSIX_ASCII_MASKS
        .iter()
        .find(|(class, _)| class.eq_ignore_ascii_case(name))
        .map_or([0; 2], |(_, mask)| *mask)
}

const ASCII_VERTICAL_SPACE_MASK: AsciiMask = ascii_mask_union(
    ascii_range_mask(b'\n', b'\x0c'),
    ascii_range_mask(b'\r', b'\r'),
);
const ASCII_NEWLINE_MASK: AsciiMask = ascii_range_mask(b'\n', b'\n');

/// ASCII membership of a Perl class, identical to `perl_class_contains` on
/// `0..=127` (checked by `perl_ascii_masks_match_the_evaluator`) without 128
/// predicate calls per class atom.
fn perl_ascii_mask(kind: PerlClassKind) -> AsciiMask {
    match kind {
        PerlClassKind::Digit => ASCII_DIGIT_MASK,
        PerlClassKind::NotDigit => ascii_mask_complement(ASCII_DIGIT_MASK),
        PerlClassKind::Space => ASCII_SPACE_MASK,
        PerlClassKind::NotSpace => ascii_mask_complement(ASCII_SPACE_MASK),
        PerlClassKind::Word => ASCII_WORD_MASK,
        PerlClassKind::NotWord => ascii_mask_complement(ASCII_WORD_MASK),
        PerlClassKind::HorizontalSpace => ASCII_HEX_DIGIT_MASK,
        PerlClassKind::NotHorizontalSpace => ascii_mask_complement(ASCII_HEX_DIGIT_MASK),
        PerlClassKind::VerticalSpace => ASCII_VERTICAL_SPACE_MASK,
        PerlClassKind::NotVerticalSpace => ascii_mask_complement(ASCII_VERTICAL_SPACE_MASK),
        PerlClassKind::NotNewline => ascii_mask_complement(ASCII_NEWLINE_MASK),
    }
}

/// The ASCII byte a case mapping yields when it is exactly one ASCII scalar.
fn single_ascii_mapping(mut mapped: impl Iterator<Item = char>) -> Option<u8> {
    let first = mapped.next()?;
    (first.is_ascii() && mapped.next().is_none()).then_some(first as u8)
}

fn ascii_mask_set_where(mask: &mut AsciiMask, predicate: impl Fn(u8) -> bool) {
    for byte in 0u8..=127 {
        if predicate(byte) {
            ascii_mask_set(mask, byte);
        }
    }
}

fn ascii_predicate_mask(predicate: impl Fn(char) -> bool) -> AsciiMask {
    let mut mask = [0u64; 2];
    for byte in 0u8..=127 {
        if predicate(byte as char) {
            ascii_mask_set(&mut mask, byte);
        }
    }
    mask
}

fn ascii_masks_by_evaluation(class: &CharClass) -> (AsciiMask, AsciiMask) {
    let sensitive = ascii_predicate_mask(|ch| class_contains(class, ch, RegexFlags::default()));
    let insensitive = ascii_predicate_mask(|ch| {
        class_contains(
            class,
            ch,
            RegexFlags {
                case_insensitive: true,
                ..RegexFlags::default()
            },
        )
    });
    (sensitive, insensitive)
}

#[derive(Debug, Clone)]
enum Instruction {
    Literal {
        id: LiteralId,
        flags: InstructionFlags,
        next: ProgramCounter,
    },
    LiteralTrie {
        id: LiteralTrieId,
        flags: InstructionFlags,
        next: ProgramCounter,
    },
    Class {
        id: ClassId,
        flags: InstructionFlags,
        next: ProgramCounter,
    },
    Any {
        flags: InstructionFlags,
        next: ProgramCounter,
    },
    Anchor {
        kind: super::ast::AnchorKind,
        next: ProgramCounter,
    },
    Jump {
        target: ProgramCounter,
    },
    Call {
        entry: ProgramCounter,
        next: ProgramCounter,
    },
    Return,
    /// Ordered choice; `guard` indexes `Program::guards`.
    Split {
        preferred: ProgramCounter,
        alternate: ProgramCounter,
        guard: u32,
    },
    RepeatInit {
        slot: VmSlot,
        next: ProgramCounter,
    },
    Repeat {
        slot: VmSlot,
        bounds: RepeatBounds,
        greedy: bool,
        body: ProgramCounter,
        next: ProgramCounter,
    },
    RepeatEnd {
        slot: VmSlot,
        repeat: ProgramCounter,
    },
    SaveStart {
        slot: VmSlot,
        next: ProgramCounter,
    },
    SaveEnd {
        slot: VmSlot,
        next: ProgramCounter,
    },
    Backref {
        slot: VmSlot,
        flags: InstructionFlags,
        next: ProgramCounter,
    },
    Conditional {
        slot: VmSlot,
        matched: ProgramCounter,
        unmatched: ProgramCounter,
    },
    Assert {
        entry: ProgramCounter,
        positive: bool,
        direction: AssertDirection,
        next: ProgramCounter,
    },
    /// Opens an atomic region: records the backtrack depth and a landing-pad
    /// frame so a total failure of the region unwinds the cut bookkeeping.
    CutStart {
        next: ProgramCounter,
    },
    /// Commits an atomic region by discarding backtrack frames created inside
    /// it. Captures and repeat effects stay committed; outer frames keep
    /// their undo marks, so backtracking past the region still restores them.
    CutEnd {
        next: ProgramCounter,
    },
    /// Repeat of a single-consumer node (`\s*+`, `[^x]++`, `.*`, …):
    /// consume greedily in place. A possessive scan pushes no backtrack
    /// frames or cut bookkeeping; with `give_back` (plain greedy repeats) it
    /// pushes one `ResumeAction::GiveBack` frame that later returns one
    /// iteration at a time instead of a frame and undo entries per iteration.
    ScanRepeat {
        node: ScanNode,
        flags: InstructionFlags,
        give_back: bool,
        bounds: RepeatBounds,
        next: ProgramCounter,
    },
    /// C/C++ grammars repeat this separator between declaration fragments:
    /// block-comments, whitespace, word-boundary assertions, and text/line
    /// anchors. In position-only mode captures are irrelevant, so the VM can
    /// emit the branch-ordered endpoints directly instead of expanding the
    /// nested possessive/comment regex at every candidate offset.
    CppSpaceCommentSeparator {
        next: ProgramCounter,
    },
    Accept,
    Fail,
}

#[derive(Debug, Clone, Copy)]
enum ScanNode {
    Literal(LiteralId),
    Class(ClassId),
    Any,
}

/// Packed assertion direction. `min_width == u32::MAX` denotes lookahead;
/// otherwise `max_width == u32::MAX` denotes unbounded lookbehind.
#[derive(Debug, Clone, Copy)]
struct AssertDirection {
    min_width: u32,
    max_width: u32,
}

impl AssertDirection {
    const AHEAD: Self = Self {
        min_width: u32::MAX,
        max_width: u32::MAX,
    };

    fn behind(min_width: usize, max_width: Option<usize>) -> Result<Self, CompileError> {
        let min_width = u32::try_from(min_width).map_err(|_| CompileError::TableOverflow)?;
        if min_width == u32::MAX {
            return Err(CompileError::TableOverflow);
        }
        let max_width = match max_width {
            Some(max_width) => {
                let max_width =
                    u32::try_from(max_width).map_err(|_| CompileError::TableOverflow)?;
                if max_width == u32::MAX {
                    return Err(CompileError::TableOverflow);
                }
                max_width
            }
            None => u32::MAX,
        };
        Ok(Self {
            min_width,
            max_width,
        })
    }

    fn is_ahead(self) -> bool {
        self.min_width == u32::MAX
    }

    fn min_width(self) -> usize {
        self.min_width as usize
    }

    fn max_width(self) -> Option<usize> {
        (self.max_width != u32::MAX).then_some(self.max_width as usize)
    }
}

/// Live repeat counter. Every field is a full scalar with no padding bytes:
/// a `bool` here made copies split into byte-sized stores that the following
/// wide load of the same slot could not store-forward, stalling the hot
/// `Repeat`/`RepeatEnd` sequence.
#[derive(Debug, Clone, Copy, Default)]
struct RepeatState {
    last_position: usize,
    count: u32,
    stalled: Stall,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(u32)]
enum Stall {
    #[default]
    Advanced = 0,
    Stalled = 1,
}

/// Repeat undo-log entry. Kept at 16 bytes with scalar fields (the stalled
/// flag rides in the top bit of the slot) so the hot push is two plain stores
/// instead of a padded tuple copy that defeats store forwarding.
#[derive(Debug, Clone, Copy)]
struct RepeatUndo {
    last_position: usize,
    count: u32,
    slot_and_stalled: u32,
}

const REPEAT_UNDO_STALLED: u32 = 1 << 31;

/// `RepeatUndo` slot marking a call frame's `nested_scanned` credit rather
/// than a repeat; `vm_slot` never hands it out.
const CREDIT_UNDO_SLOT: VmSlot = REPEAT_UNDO_STALLED - 1;

impl RepeatUndo {
    /// Restores call frame `frame`'s (zero-based) `nested_scanned` credit.
    fn credit(frame: u32, previous: u32) -> Self {
        Self {
            last_position: arena_index(frame),
            count: previous,
            slot_and_stalled: CREDIT_UNDO_SLOT,
        }
    }

    fn is_credit(self) -> bool {
        self.slot_and_stalled == CREDIT_UNDO_SLOT
    }

    fn new(slot: VmSlot, state: RepeatState) -> Self {
        debug_assert!(slot < REPEAT_UNDO_STALLED);
        Self {
            last_position: state.last_position,
            count: state.count,
            slot_and_stalled: slot
                | if state.stalled == Stall::Stalled {
                    REPEAT_UNDO_STALLED
                } else {
                    0
                },
        }
    }

    fn vm_slot(self) -> VmSlot {
        self.slot_and_stalled & !REPEAT_UNDO_STALLED
    }

    fn slot(self) -> usize {
        arena_index(self.vm_slot())
    }

    fn state(self) -> RepeatState {
        RepeatState {
            count: self.count,
            last_position: self.last_position,
            stalled: if self.slot_and_stalled & REPEAT_UNDO_STALLED != 0 {
                Stall::Stalled
            } else {
                Stall::Advanced
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum CaptureState {
    #[default]
    Unset,
    Open(usize),
    Matched(Range<usize>),
}

#[derive(Debug, Clone, Copy)]
enum ResumeAction {
    None,
    EnterRepeat(VmSlot),
    /// Landing pad for an atomic region: the region failed outright, so pop
    /// its cut mark and keep failing outward.
    PopCut,
    /// A give-back `ScanRepeat` at `pc` ended at `position` and may still
    /// return this many iterations.
    GiveBack(u32),
}

/// Hot DFS frame. Positions remain native-width because they index caller
/// strings; all program and arena indexes are bounded 32-bit operands.
#[derive(Debug, Clone, Copy)]
struct BacktrackFrame {
    position: usize,
    action: ResumeAction,
    pc: ProgramCounter,
    repeat_undo_mark: u32,
    capture_undo_mark: u32,
    call_frame: u32,
}

#[derive(Debug, Clone, Copy)]
struct AssertionFrame {
    parent_position: usize,
    target_end: usize,
    next_probe: usize,
    direction: AssertDirection,
    entry: ProgramCounter,
    parent_pc: ProgramCounter,
    parent_repeat_undo_mark: u32,
    parent_capture_undo_mark: u32,
    parent_call_frame: u32,
    backtrack_base: u32,
    cut_base: u32,
    positive: bool,
    has_next_probe: bool,
}

/// Call frames are never overwritten: backtracking can resume inside a
/// routine that already returned, and its `Return` must still find the frame
/// of the call that entered it. Frames link to their caller instead.
#[derive(Debug, Clone, Copy)]
struct CallFrame {
    return_pc: ProgramCounter,
    capture_undo_mark: u32,
    repeat_undo_mark: u32,
    /// Undo entries already scanned by returns of calls this frame made.
    nested_scanned: u32,
    /// The caller's `BytecodeScratch::call_frame` value.
    parent: u32,
    depth: u32,
}

// These are performance contracts, not incidental implementation details.
// Keep them compile-time checked on the 64-bit targets used for profiling and
// normal desktop/server deployment.
#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(std::mem::size_of::<Instruction>() == 24);
    assert!(std::mem::size_of::<BacktrackFrame>() == 32);
    assert!(std::mem::size_of::<AssertionFrame>() == 64);
    assert!(std::mem::size_of::<CallFrame>() == 24);
    assert!(std::mem::size_of::<RepeatState>() == 16);
    assert!(std::mem::size_of::<RepeatUndo>() == 16);
    assert!(std::mem::size_of::<GuardCell>() == 16);
    assert!(std::mem::size_of::<ResumeAction>() == 8);
    assert!(std::mem::size_of::<AssertDirection>() == 8);
};

/// Reusable position-only VM arena. Lengths are cleared for each root run;
/// capacities are retained.
#[derive(Debug, Clone, Default)]
pub(crate) struct BytecodeScratch {
    backtrack: Vec<BacktrackFrame>,
    assertions: Vec<AssertionFrame>,
    repeats: Vec<RepeatState>,
    captures: Vec<CaptureState>,
    repeat_undo: Vec<RepeatUndo>,
    capture_undo: Vec<(VmSlot, CaptureState)>,
    calls: Vec<CallFrame>,
    /// Per-slot marks of the `Return` restore that last visited the slot.
    repeat_restore_stamps: Vec<u32>,
    capture_restore_stamps: Vec<u32>,
    restore_generation: u32,
    /// One-based index of the current call frame; 0 outside subroutines.
    call_frame: u32,
    cuts: Vec<u32>,
    literal_matches: Vec<(u32, usize)>,
    scanner: super::scanner::ScannerScratch,
    prefilter_cursors: super::prefilter::PrefilterCursors,
    line_ptr: usize,
    line_len: usize,
    line_is_ascii: bool,
    line_block_comment: Option<bool>,
    /// Bumped whenever the line identity changes; keys per-line memos.
    line_generation: u64,
    exhausted_entries: super::dfa::ExhaustedEntryMemo,
}

impl BytecodeScratch {
    pub(crate) fn begin_line(&mut self, line: &str) {
        self.prefilter_cursors.begin_line(line);
        self.line_ptr = line.as_ptr() as usize;
        self.line_len = line.len();
        self.line_is_ascii = line.is_ascii();
        self.line_block_comment = None;
        self.line_generation = self.line_generation.wrapping_add(1).max(1);
    }

    pub(crate) fn line_generation(&mut self, line: &str) -> u64 {
        self.refresh_line_identity(line);
        self.line_generation
    }

    pub(crate) fn exhausted_entries(&mut self) -> &mut super::dfa::ExhaustedEntryMemo {
        &mut self.exhausted_entries
    }

    pub(crate) fn line_is_ascii(&mut self, line: &str) -> bool {
        self.refresh_line_identity(line);
        self.line_is_ascii
    }

    /// Whether the line contains a `/*` block-comment opener; cached per
    /// line for the skip-prefix gates.
    pub(crate) fn line_has_block_comment(&mut self, line: &str) -> bool {
        self.refresh_line_identity(line);
        *self
            .line_block_comment
            .get_or_insert_with(|| memchr::memmem::find(line.as_bytes(), b"/*").is_some())
    }

    fn refresh_line_identity(&mut self, line: &str) {
        let ptr = line.as_ptr() as usize;
        if self.line_ptr != ptr || self.line_len != line.len() {
            self.line_ptr = ptr;
            self.line_len = line.len();
            self.line_is_ascii = line.is_ascii();
            self.line_block_comment = None;
            self.line_generation = self.line_generation.wrapping_add(1).max(1);
        }
    }

    pub(crate) fn scanner(&mut self) -> &mut super::scanner::ScannerScratch {
        &mut self.scanner
    }

    pub(crate) fn prefilter_cursors(&mut self) -> &mut super::prefilter::PrefilterCursors {
        &mut self.prefilter_cursors
    }
}

fn push_cpp_space_comment_separator_positions(
    line: &str,
    position: usize,
    ctx: AnchorContext,
    out: &mut Vec<(u32, usize)>,
) {
    out.clear();
    push_cpp_comment_sequence_ends(line, position, out);

    let space_end = consume_whitespace(line, position);
    if space_end > position {
        out.push((0, space_end));
    }
    if previous_char(line, position).is_some_and(|ch| !cpp_is_word_char(ch)) {
        out.push((0, position));
    }
    if char_at(line, position).is_some_and(|(ch, _)| !cpp_is_word_char(ch)) {
        out.push((0, position));
    }
    if position == 0 {
        out.push((0, position));
    }
    if is_line_end_position(line, position) {
        if line.as_bytes().get(position) == Some(&b'\n') {
            out.push((0, position + 1));
        }
        out.push((0, position));
    }
    if ctx.allow_a && position == 0 {
        out.push((0, position));
    }
    if position == line.len() || line.get(position..).is_some_and(|tail| tail == "\n") {
        out.push((0, position));
    }
}

fn push_cpp_comment_sequence_ends(line: &str, start: usize, out: &mut Vec<(u32, usize)>) {
    let base = out.len();
    let mut pos = start;
    loop {
        pos = consume_whitespace(line, pos);
        let Some(after_comment) = consume_c_block_comment(line, pos) else {
            break;
        };
        pos = consume_whitespace(line, after_comment);
        out.push((0, pos));
    }
    out[base..].reverse();
}

fn consume_whitespace(line: &str, mut pos: usize) -> usize {
    while let Some((ch, next)) = char_at(line, pos) {
        if !ch.is_whitespace() {
            break;
        }
        pos = next;
    }
    pos
}

fn consume_c_block_comment(line: &str, pos: usize) -> Option<usize> {
    let rest = line.get(pos..)?;
    if !rest.starts_with("/*") {
        return None;
    }
    let end = rest.get(2..)?.find("*/")?;
    Some(pos + 2 + end + 2)
}

fn cpp_is_word_char(ch: char) -> bool {
    is_unicode_word_char(ch)
}

fn backtrack_frame(
    scratch: &BytecodeScratch,
    pc: ProgramCounter,
    position: usize,
    action: ResumeAction,
) -> Result<BacktrackFrame, BudgetExceeded> {
    Ok(BacktrackFrame {
        position,
        action,
        pc,
        repeat_undo_mark: arena_mark(scratch.repeat_undo.len())?,
        capture_undo_mark: arena_mark(scratch.capture_undo.len())?,
        call_frame: scratch.call_frame,
    })
}

fn is_line_end_position(line: &str, pos: usize) -> bool {
    pos == line.len() || line.as_bytes().get(pos) == Some(&b'\n')
}

impl Program {
    pub(crate) fn compile(parsed: &ParsedRegex) -> Result<Self, CompileError> {
        Compiler::new().compile(parsed)
    }

    /// Compile capture replay bytecode for only the requested group numbers.
    /// Group zero is included automatically. Invalid group numbers are ignored,
    /// which lets callers pass a grammar-level liveness set without trimming it.
    #[allow(dead_code)] // Vertical-slice API; backtrack/tokenizer integration follows.
    pub(crate) fn compile_captures(
        parsed: &ParsedRegex,
        live_captures: &[u32],
    ) -> Result<Self, CompileError> {
        Self::compile_captures_with_analysis(parsed, parsed.analysis(), live_captures)
    }

    pub(crate) fn compile_captures_with_analysis(
        parsed: &ParsedRegex,
        analysis: &RegexAnalysis,
        live_captures: &[u32],
    ) -> Result<Self, CompileError> {
        Self::compile_capture_layout(parsed, analysis, live_captures, None)
    }

    /// Compiles capture replay bytecode that also serves position selection,
    /// or `None` when a live capture sits under an alternation or repeat.
    /// Such captures would block the literal tries and possessive scans that
    /// selection relies on; otherwise the program differs from the
    /// position-only one only by its save instructions.
    pub(crate) fn compile_selection_captures(
        parsed: &ParsedRegex,
        analysis: &RegexAnalysis,
        live_captures: &[u32],
    ) -> Option<Result<Self, CompileError>> {
        live_captures_keep_selection_shape(&parsed.ast, live_captures)
            .then(|| Self::compile_capture_layout(parsed, analysis, live_captures, Some(false)))
    }

    fn compile_capture_layout(
        parsed: &ParsedRegex,
        analysis: &RegexAnalysis,
        live_captures: &[u32],
        captures_under_choice: Option<bool>,
    ) -> Result<Self, CompileError> {
        if !analysis.capture().capture_bytecode_supported() {
            return Err(CompileError::Unsupported);
        }
        let mut layout = Vec::with_capacity(
            live_captures
                .len()
                .saturating_add(analysis.capture().referenced_groups().len())
                .saturating_add(1),
        );
        layout.push(0);
        layout.extend(
            live_captures
                .iter()
                .copied()
                .filter(|index| *index > 0 && *index <= parsed.capture_count),
        );
        layout.extend_from_slice(analysis.capture().referenced_groups());
        layout.sort_unstable();
        layout.dedup();
        let mut compiler = Compiler::with_captures(layout);
        // Referenced groups join the layout only for backreferences, which
        // `compile_selection_captures` callers never share.
        compiler.captures_under_choice =
            captures_under_choice.filter(|_| analysis.capture().referenced_groups().is_empty());
        compiler.compile(parsed)
    }

    #[allow(dead_code)] // Vertical-slice API; backtrack/tokenizer integration follows.
    pub(crate) fn capture_layout(&self) -> &[u32] {
        &self.capture_layout
    }

    /// Bytecode pays off once the pattern has an ordered choice point; the
    /// fanout score is computed by [`RegexAnalysis`]'s structural walk.
    pub(crate) fn is_beneficial_fanout(ordered_fanout: usize) -> bool {
        ordered_fanout >= beneficial_fanout_threshold()
    }
}

fn beneficial_fanout_threshold() -> usize {
    1
}

impl Program {
    pub(crate) fn execute(
        &self,
        line: &str,
        start: usize,
        ctx: AnchorContext,
        budget: &mut StepBudget,
        scratch: &mut BytecodeScratch,
    ) -> Result<Option<usize>, BudgetExceeded> {
        self.execute_inner(line, start, ctx, budget, scratch)
    }

    /// Execute a capture program while leaving its compact capture slots in
    /// `scratch`. The caller can then copy them directly into its final output
    /// layout, avoiding an intermediate winner allocation.
    pub(crate) fn execute_capture_slots(
        &self,
        line: &str,
        start: usize,
        ctx: AnchorContext,
        budget: &mut StepBudget,
        scratch: &mut BytecodeScratch,
    ) -> Result<Option<usize>, BudgetExceeded> {
        assert!(
            !self.capture_layout.is_empty(),
            "capture execution requires Program::compile_captures"
        );
        self.execute_inner(line, start, ctx, budget, scratch)
    }

    /// Copies the successful execution still resident in `scratch` into a
    /// full group-number-indexed result. `output` may be larger than the
    /// compact live layout; groups the grammar does not consume remain unset.
    pub(crate) fn copy_capture_slots_into(
        &self,
        start: usize,
        end: usize,
        scratch: &BytecodeScratch,
        output: &mut [Option<Range<usize>>],
    ) {
        output.fill(None);
        for (slot, group) in self.capture_layout.iter().copied().enumerate() {
            let Some(output) = output.get_mut(group as usize) else {
                continue;
            };
            *output = if group == 0 {
                Some(start..end)
            } else {
                match scratch.captures.get(slot) {
                    Some(CaptureState::Matched(range)) => Some(range.clone()),
                    Some(CaptureState::Unset | CaptureState::Open(_)) | None => None,
                }
            };
        }
    }

    /// Execute capture replay and return values in the program's compact
    /// layout. Retained for the regex API and differential tests; the tokenizer
    /// writes directly into its final full-group vector instead.
    #[allow(dead_code)]
    pub(crate) fn execute_captures(
        &self,
        line: &str,
        start: usize,
        ctx: AnchorContext,
        budget: &mut StepBudget,
        scratch: &mut BytecodeScratch,
    ) -> Result<Option<CaptureMatch>, BudgetExceeded> {
        let Some(end) = self.execute_capture_slots(line, start, ctx, budget, scratch)? else {
            return Ok(None);
        };
        let mut captures = vec![None; self.capture_layout.len()];
        for (slot, capture) in captures.iter_mut().enumerate() {
            *capture = if slot == 0 {
                Some(start..end)
            } else {
                match scratch.captures.get(slot) {
                    Some(CaptureState::Matched(range)) => Some(range.clone()),
                    Some(CaptureState::Unset | CaptureState::Open(_)) | None => None,
                }
            };
        }
        Ok(Some(CaptureMatch { end, captures }))
    }

    fn execute_inner(
        &self,
        line: &str,
        start: usize,
        ctx: AnchorContext,
        budget: &mut StepBudget,
        scratch: &mut BytecodeScratch,
    ) -> Result<Option<usize>, BudgetExceeded> {
        scratch.reset(arena_index(self.repeat_slots), self.capture_layout.len());
        let mut pc = self.entry;
        let mut position = start;

        loop {
            budget.step()?;
            match &self.instructions[arena_index(pc)] {
                Instruction::Literal { id, flags, next } => {
                    let value = &self.literals[id.0 as usize];
                    if let Some(end) = match_literal_end(line, position, value, flags.regex()) {
                        position = end;
                        pc = *next;
                    } else if !self.backtrack_or_resolve(line, scratch, &mut pc, &mut position)? {
                        return Ok(None);
                    }
                }
                Instruction::LiteralTrie { id, flags, next } => {
                    let trie = &self.literal_tries[id.0 as usize];
                    trie.collect_matches(
                        line,
                        position,
                        flags.regex(),
                        budget,
                        &mut scratch.literal_matches,
                    )?;
                    if scratch.literal_matches.len() > 1 {
                        scratch
                            .literal_matches
                            .sort_unstable_by_key(|(order, _)| *order);
                    }
                    if !scratch.literal_matches.is_empty() {
                        // Preserve ordered-regex backtracking. A shorter
                        // preferred keyword may match now but fail in the
                        // suffix; alternate terminal ends resume directly at
                        // `next` without re-walking the shared trie.
                        for index in (1..scratch.literal_matches.len()).rev() {
                            let (_, alternate_position) = scratch.literal_matches[index];
                            scratch.backtrack.push(backtrack_frame(
                                scratch,
                                *next,
                                alternate_position,
                                ResumeAction::None,
                            )?);
                        }
                        position = scratch.literal_matches[0].1;
                        pc = *next;
                    } else if !self.backtrack_or_resolve(line, scratch, &mut pc, &mut position)? {
                        return Ok(None);
                    }
                }
                Instruction::Class { id, flags, next } => {
                    if let Some(end) = self.class_end(*id, *flags, line, position) {
                        position = end;
                        pc = *next;
                    } else if !self.backtrack_or_resolve(line, scratch, &mut pc, &mut position)? {
                        return Ok(None);
                    }
                }
                Instruction::Any { flags, next } => {
                    if let Some((ch, end)) = char_at(line, position)
                        && (ch != '\n' || flags.dot_matches_new_line())
                    {
                        position = end;
                        pc = *next;
                    } else if !self.backtrack_or_resolve(line, scratch, &mut pc, &mut position)? {
                        return Ok(None);
                    }
                }
                Instruction::Anchor { kind, next } => {
                    if anchor_matches(*kind, line, position, ctx) {
                        pc = *next;
                    } else if !self.backtrack_or_resolve(line, scratch, &mut pc, &mut position)? {
                        return Ok(None);
                    }
                }
                Instruction::Jump { target } => pc = *target,
                Instruction::Call { entry, next } => {
                    let depth = scratch
                        .call_frame
                        .checked_sub(1)
                        .map_or(0, |caller| scratch.calls[arena_index(caller)].depth + 1);
                    if depth >= 128 {
                        if !self.backtrack_or_resolve(line, scratch, &mut pc, &mut position)? {
                            return Ok(None);
                        }
                    } else {
                        let frame = CallFrame {
                            return_pc: *next,
                            capture_undo_mark: arena_mark(scratch.capture_undo.len())?,
                            repeat_undo_mark: arena_mark(scratch.repeat_undo.len())?,
                            nested_scanned: 0,
                            parent: scratch.call_frame,
                            depth,
                        };
                        scratch.calls.push(frame);
                        scratch.call_frame = arena_mark(scratch.calls.len())?;
                        pc = *entry;
                    }
                }
                Instruction::Return => {
                    debug_assert!(scratch.call_frame > 0, "Return outside subroutine");
                    let frame = scratch.calls[arena_index(scratch.call_frame - 1)];
                    scratch.call_frame = frame.parent;
                    // Recursive calls to the same capturing group overwrite
                    // an enclosing pending start. Restore pending captures on
                    // return; completed captures remain observable. Restores
                    // are logged so backtracking into the routine undoes them.
                    // Each slot is restored at most once, from its oldest
                    // undo entry, so nested returns add at most one log entry
                    // per slot instead of replaying their callees' logs.
                    let generation = scratch.next_restore_generation();
                    let capture_end = scratch.capture_undo.len();
                    let repeat_end = scratch.repeat_undo.len();
                    // The scan is linear in the undo entries since the call,
                    // which backtracking into a returned routine can revisit;
                    // charge it like the steps that logged them. Entries a
                    // nested return already scanned are charged there, so
                    // deep nesting does not pay for them once per level.
                    let scanned = (capture_end - arena_index(frame.capture_undo_mark))
                        + (repeat_end - arena_index(frame.repeat_undo_mark));
                    budget.charge(scanned.saturating_sub(arena_index(frame.nested_scanned)))?;
                    if let Some(caller) = frame.parent.checked_sub(1) {
                        // Logged like a repeat write so backtracking past this
                        // return also takes the caller's credit back.
                        let credit = &mut scratch.calls[arena_index(caller)].nested_scanned;
                        let previous = *credit;
                        *credit = credit.saturating_add(u32::try_from(scanned).unwrap_or(u32::MAX));
                        scratch
                            .repeat_undo
                            .push(RepeatUndo::credit(caller, previous));
                    }
                    for index in arena_index(frame.capture_undo_mark)..capture_end {
                        let (slot, previous) = scratch.capture_undo[index].clone();
                        let stamp = &mut scratch.capture_restore_stamps[arena_index(slot)];
                        if *stamp == generation {
                            continue;
                        }
                        *stamp = generation;
                        if let CaptureState::Open(start) = previous {
                            set_capture(scratch, slot, CaptureState::Open(start));
                        }
                    }
                    // A recursive call reuses its caller's loop slots. Put
                    // every slot the call changed back to its value at the
                    // call.
                    for index in arena_index(frame.repeat_undo_mark)..repeat_end {
                        let undo = scratch.repeat_undo[index];
                        if undo.is_credit() {
                            continue;
                        }
                        let stamp = &mut scratch.repeat_restore_stamps[undo.slot()];
                        if *stamp == generation {
                            continue;
                        }
                        *stamp = generation;
                        set_repeat(scratch, undo.vm_slot(), undo.state());
                    }
                    pc = frame.return_pc;
                }
                Instruction::Split {
                    preferred,
                    alternate,
                    guard,
                } => {
                    let (mut preferred, mut alternate, mut guard) =
                        (*preferred, *alternate, *guard);
                    loop {
                        if self.guards[arena_index(guard)].allows(self, preferred, line, position) {
                            scratch.backtrack.push(backtrack_frame(
                                scratch,
                                alternate,
                                position,
                                ResumeAction::None,
                            )?);
                            pc = preferred;
                            break;
                        }
                        // Walk a rejected alternation chain in place, still
                        // charging one step per `Split` visited.
                        pc = alternate;
                        let Instruction::Split {
                            preferred: next_preferred,
                            alternate: next_alternate,
                            guard: next_guard,
                        } = self.instructions[arena_index(pc)]
                        else {
                            break;
                        };
                        budget.step()?;
                        (preferred, alternate, guard) =
                            (next_preferred, next_alternate, next_guard);
                    }
                }
                Instruction::RepeatInit { slot, next } => {
                    set_repeat(
                        scratch,
                        *slot,
                        RepeatState {
                            count: 0,
                            last_position: position,
                            stalled: Stall::Advanced,
                        },
                    );
                    // Always continues at its loop's `Repeat`; run it here
                    // (charging its step) instead of re-dispatching.
                    pc = *next;
                    budget.step()?;
                    if !self.repeat(line, scratch, &mut pc, &mut position)? {
                        return Ok(None);
                    }
                }
                Instruction::Repeat { .. } => {
                    if !self.repeat(line, scratch, &mut pc, &mut position)? {
                        return Ok(None);
                    }
                }
                Instruction::RepeatEnd { slot, repeat } => {
                    let index = arena_index(*slot);
                    if scratch.repeats[index].last_position == position {
                        let mut value = scratch.repeats[index];
                        value.stalled = Stall::Stalled;
                        set_repeat(scratch, *slot, value);
                    }
                    pc = *repeat;
                    budget.step()?;
                    if !self.repeat(line, scratch, &mut pc, &mut position)? {
                        return Ok(None);
                    }
                }
                Instruction::SaveStart { slot, next } => {
                    set_capture(scratch, *slot, CaptureState::Open(position));
                    pc = *next;
                }
                Instruction::SaveEnd { slot, next } => {
                    let CaptureState::Open(start) = scratch.captures[arena_index(*slot)] else {
                        unreachable!("SaveEnd without SaveStart")
                    };
                    set_capture(scratch, *slot, CaptureState::Matched(start..position));
                    pc = *next;
                }
                Instruction::Backref { slot, flags, next } => {
                    let matched = match &scratch.captures[arena_index(*slot)] {
                        CaptureState::Matched(range) => {
                            line.get(range.clone()).and_then(|captured| {
                                match_literal_end(line, position, captured, flags.regex())
                            })
                        }
                        CaptureState::Unset | CaptureState::Open(_) => None,
                    };
                    if let Some(end) = matched {
                        position = end;
                        pc = *next;
                    } else if !self.backtrack_or_resolve(line, scratch, &mut pc, &mut position)? {
                        return Ok(None);
                    }
                }
                Instruction::Conditional {
                    slot,
                    matched,
                    unmatched,
                } => {
                    pc = if matches!(
                        scratch.captures[arena_index(*slot)],
                        CaptureState::Matched(_)
                    ) {
                        *matched
                    } else {
                        *unmatched
                    };
                }
                Instruction::Assert {
                    entry,
                    positive,
                    direction,
                    next,
                } => {
                    if let Some(matched) =
                        self.single_consumer_assertion(line, position, *entry, *direction)
                    {
                        if matched == *positive {
                            pc = *next;
                        } else if !self.backtrack_or_resolve(
                            line,
                            scratch,
                            &mut pc,
                            &mut position,
                        )? {
                            return Ok(None);
                        }
                        continue;
                    }
                    let mut frame = AssertionFrame {
                        parent_position: position,
                        target_end: position,
                        next_probe: 0,
                        direction: *direction,
                        entry: *entry,
                        parent_pc: *next,
                        parent_repeat_undo_mark: arena_mark(scratch.repeat_undo.len())?,
                        parent_capture_undo_mark: arena_mark(scratch.capture_undo.len())?,
                        parent_call_frame: scratch.call_frame,
                        backtrack_base: arena_mark(scratch.backtrack.len())?,
                        cut_base: arena_mark(scratch.cuts.len())?,
                        positive: *positive,
                        has_next_probe: false,
                    };
                    if let Some(probe) = first_probe(line, position, *direction, &mut frame) {
                        scratch.assertions.push(frame);
                        position = probe;
                        pc = *entry;
                    } else {
                        let passed = !*positive;
                        if passed {
                            pc = *next;
                        } else if !self.backtrack_or_resolve(
                            line,
                            scratch,
                            &mut pc,
                            &mut position,
                        )? {
                            return Ok(None);
                        }
                    }
                }
                Instruction::Accept => {
                    let Some(assertion) = scratch.assertions.last().copied() else {
                        return Ok(Some(position));
                    };
                    let assertion_match =
                        assertion.direction.is_ahead() || position == assertion.target_end;
                    if assertion_match {
                        self.finish_assertion(scratch, true, &mut pc, &mut position);
                        if pc == INVALID_PROGRAM_COUNTER
                            && !self.backtrack_or_resolve(line, scratch, &mut pc, &mut position)?
                        {
                            return Ok(None);
                        }
                    } else if !self.backtrack_or_resolve(line, scratch, &mut pc, &mut position)? {
                        return Ok(None);
                    }
                }
                Instruction::CutStart { next } => {
                    scratch.cuts.push(arena_mark(scratch.backtrack.len())?);
                    scratch.backtrack.push(backtrack_frame(
                        scratch,
                        INVALID_PROGRAM_COUNTER,
                        position,
                        ResumeAction::PopCut,
                    )?);
                    pc = *next;
                }
                Instruction::CutEnd { next } => {
                    let mark = scratch.cuts.pop().expect("CutEnd without CutStart");
                    scratch.backtrack.truncate(arena_index(mark));
                    pc = *next;
                }
                Instruction::ScanRepeat {
                    node,
                    flags,
                    give_back,
                    bounds,
                    next,
                } => {
                    let (count, cursor) = self.scan_forward(*node, *flags, *bounds, line, position);
                    if *give_back && count > bounds.min {
                        scratch.backtrack.push(backtrack_frame(
                            scratch,
                            pc,
                            cursor,
                            ResumeAction::GiveBack(count - bounds.min),
                        )?);
                    }
                    if count >= bounds.min {
                        position = cursor;
                        pc = *next;
                    } else if !self.backtrack_or_resolve(line, scratch, &mut pc, &mut position)? {
                        return Ok(None);
                    }
                }
                Instruction::CppSpaceCommentSeparator { next } => {
                    push_cpp_space_comment_separator_positions(
                        line,
                        position,
                        ctx,
                        &mut scratch.literal_matches,
                    );
                    if let Some((_, preferred)) = scratch.literal_matches.first().copied() {
                        for &(_, alternate) in scratch.literal_matches[1..].iter().rev() {
                            scratch.backtrack.push(backtrack_frame(
                                scratch,
                                *next,
                                alternate,
                                ResumeAction::None,
                            )?);
                        }
                        position = preferred;
                        pc = *next;
                    } else if !self.backtrack_or_resolve(line, scratch, &mut pc, &mut position)? {
                        return Ok(None);
                    }
                }
                Instruction::Fail => {
                    if !self.backtrack_or_resolve(line, scratch, &mut pc, &mut position)? {
                        return Ok(None);
                    }
                }
            }
        }
    }

    /// Executes the `Repeat` instruction at `pc`. Returns false when
    /// failing out of it exhausts every alternative.
    #[inline(always)]
    fn repeat(
        &self,
        line: &str,
        scratch: &mut BytecodeScratch,
        pc: &mut ProgramCounter,
        position: &mut usize,
    ) -> Result<bool, BudgetExceeded> {
        let Instruction::Repeat {
            slot,
            bounds,
            greedy,
            body,
            next,
        } = self.instructions[arena_index(*pc)]
        else {
            unreachable!("loop entry and end always target their Repeat")
        };
        let repeat = scratch.repeats[arena_index(slot)];
        let count = repeat.count;
        let can_exit = count >= bounds.min;
        // An iteration whose body cannot start here would fail before
        // consuming anything; skip it and its frame.
        let can_repeat = bounds.max().is_none_or(|max| count < max)
            && (repeat.stalled == Stall::Advanced || count < bounds.min)
            && self.guards[arena_index(self.repeat_guard_base) + arena_index(slot)]
                .allows(self, body, line, *position);
        match (can_repeat, can_exit, greedy) {
            (true, true, true) => {
                scratch.backtrack.push(backtrack_frame(
                    scratch,
                    next,
                    *position,
                    ResumeAction::None,
                )?);
                enter_repeat(scratch, slot, *position);
                *pc = body;
            }
            (true, true, false) => {
                scratch.backtrack.push(backtrack_frame(
                    scratch,
                    body,
                    *position,
                    ResumeAction::EnterRepeat(slot),
                )?);
                *pc = next;
            }
            (true, false, _) => {
                enter_repeat(scratch, slot, *position);
                *pc = body;
            }
            (false, true, _) => *pc = next,
            (false, false, _) => return self.backtrack_or_resolve(line, scratch, pc, position),
        }
        Ok(true)
    }

    /// Greedily matches `node` from `position`, at most `bounds.max` times.
    /// Returns the iteration count and end position.
    #[inline]
    fn scan_forward(
        &self,
        node: ScanNode,
        flags: InstructionFlags,
        bounds: RepeatBounds,
        line: &str,
        position: usize,
    ) -> (u32, usize) {
        let mut count = 0u32;
        let mut cursor = position;
        while bounds.max().is_none_or(|max| count < max) {
            let advanced = match node {
                ScanNode::Literal(id) => {
                    let value = &self.literals[id.0 as usize];
                    match_literal_end(line, cursor, value, flags.regex())
                }
                ScanNode::Class(id) => self.class_end(id, flags, line, cursor),
                ScanNode::Any => char_at(line, cursor).and_then(|(ch, end)| {
                    (ch != '\n' || flags.dot_matches_new_line()).then_some(end)
                }),
            };
            match advanced {
                Some(end) if end > cursor => {
                    cursor = end;
                    count += 1;
                }
                _ => break,
            }
        }
        (count, cursor)
    }

    /// Start of the last iteration of a give-back scan ending at `end`.
    /// Classes and `.` consume exactly one scalar; give-back literals are
    /// case-sensitive, so each iteration is exactly the literal's bytes.
    fn give_back_one(&self, node: ScanNode, line: &str, end: usize) -> usize {
        match node {
            ScanNode::Literal(id) => end - self.literals[id.0 as usize].len(),
            ScanNode::Class(_) | ScanNode::Any => {
                let width = line[..end]
                    .chars()
                    .next_back()
                    .expect("a scanned iteration precedes the give-back position")
                    .len_utf8();
                end - width
            }
        }
    }

    #[inline]
    fn class_end(
        &self,
        id: ClassId,
        flags: InstructionFlags,
        line: &str,
        position: usize,
    ) -> Option<usize> {
        let class = &self.classes[id.0 as usize];
        match line.as_bytes().get(position).copied() {
            Some(byte) if byte.is_ascii() => class
                .matches_ascii(byte, flags.case_insensitive())
                .then_some(position + 1),
            Some(_) => char_at(line, position)
                .filter(|(ch, _)| class.matches_char(*ch, flags.regex()))
                .map(|(_, end)| end),
            None => None,
        }
    }

    /// Evaluates an assertion whose body is one class or case-sensitive
    /// literal directly followed by its `Accept` (`(?<![$\w])`, `(?!\.)`,
    /// `(?<=\.\.\.)`, …) without entering the assertion sub-machine.
    /// Such bodies have no captures, repeats, or alternatives, so a single
    /// direct test decides the result. Returns whether the body matched, or
    /// `None` when the general path is needed.
    #[inline]
    fn single_consumer_assertion(
        &self,
        line: &str,
        position: usize,
        entry: ProgramCounter,
        direction: AssertDirection,
    ) -> Option<bool> {
        let (next, lookbehind_start) = match &self.instructions[arena_index(entry)] {
            Instruction::Class { id, flags, next } => {
                if direction.is_ahead() {
                    let matched = self.class_end(*id, *flags, line, position).is_some();
                    return self.is_accept(*next).then_some(matched);
                }
                // The only probe that can end exactly at `position` starts
                // at the previous character.
                let start = line
                    .get(..position)?
                    .chars()
                    .next_back()
                    .map(|ch| position - ch.len_utf8());
                let matched = start.is_some_and(|start| {
                    self.class_end(*id, *flags, line, start) == Some(position)
                });
                (*next, start.filter(|_| matched))
            }
            Instruction::Literal { id, flags, next } if !flags.case_insensitive() => {
                let literal = &self.literals[id.0 as usize];
                if direction.is_ahead() {
                    let matched =
                        match_literal_end(line, position, literal, flags.regex()).is_some();
                    return self.is_accept(*next).then_some(matched);
                }
                let start = position.checked_sub(literal.len()).filter(|start| {
                    line.is_char_boundary(*start)
                        && line.as_bytes()[*start..position] == *literal.as_bytes()
                });
                (*next, start)
            }
            _ => return None,
        };
        if !self.is_accept(next) {
            return None;
        }
        // Honour the probe window exactly as `first_probe` would.
        Some(lookbehind_start.is_some_and(|start| {
            let width = position - start;
            width >= direction.min_width() && direction.max_width().is_none_or(|max| width <= max)
        }))
    }

    fn is_accept(&self, pc: ProgramCounter) -> bool {
        matches!(self.instructions[arena_index(pc)], Instruction::Accept)
    }

    /// Characters one of which every successful path from `pc` must consume
    /// at the current position. `None` when a path may succeed without
    /// consuming (reaching `Accept`, a loop end, or a subroutine boundary)
    /// or the walk gives up.
    fn must_consume_first_chars(&self, pc: ProgramCounter, steps: &mut u32) -> Option<FirstChars> {
        *steps = steps.checked_sub(1)?;
        match &self.instructions[arena_index(pc)] {
            Instruction::Literal { id, flags, next } => {
                let literal = &self.literals[id.0 as usize];
                if literal.is_empty() {
                    self.must_consume_first_chars(*next, steps)
                } else {
                    Some(FirstChars::of_literal(literal, flags.case_insensitive()))
                }
            }
            Instruction::LiteralTrie { id, flags, .. } => {
                self.literal_tries[id.0 as usize].first_chars(flags.case_insensitive())
            }
            Instruction::Class { id, flags, .. } => Some(FirstChars::of_class(
                &self.classes[id.0 as usize],
                flags.case_insensitive(),
            )),
            Instruction::ScanRepeat {
                node,
                flags,
                bounds,
                next,
                ..
            } => {
                let mut first = match node {
                    ScanNode::Literal(id) => FirstChars::of_literal(
                        &self.literals[id.0 as usize],
                        flags.case_insensitive(),
                    ),
                    ScanNode::Class(id) => {
                        FirstChars::of_class(&self.classes[id.0 as usize], flags.case_insensitive())
                    }
                    ScanNode::Any => FirstChars::ALL,
                };
                if bounds.min == 0 {
                    first.union(&self.must_consume_first_chars(*next, steps)?);
                }
                Some(first)
            }
            // Zero-width steps: the continuation consumes at this position.
            Instruction::Anchor { next, .. }
            | Instruction::RepeatInit { next, .. }
            | Instruction::SaveStart { next, .. }
            | Instruction::SaveEnd { next, .. }
            | Instruction::CutStart { next }
            | Instruction::CutEnd { next }
            | Instruction::Assert { next, .. } => self.must_consume_first_chars(*next, steps),
            Instruction::Jump { target } => self.must_consume_first_chars(*target, steps),
            Instruction::Split {
                preferred,
                alternate,
                ..
            } => {
                let mut first = self.must_consume_first_chars(*preferred, steps)?;
                first.union(&self.must_consume_first_chars(*alternate, steps)?);
                Some(first)
            }
            // Only reached through `RepeatInit` here, so the count is zero.
            Instruction::Repeat {
                bounds, body, next, ..
            } => {
                let mut first = self.must_consume_first_chars(*body, steps)?;
                if bounds.min == 0 {
                    first.union(&self.must_consume_first_chars(*next, steps)?);
                }
                Some(first)
            }
            Instruction::Any { .. }
            | Instruction::Call { .. }
            | Instruction::Return
            | Instruction::RepeatEnd { .. }
            | Instruction::Backref { .. }
            | Instruction::Conditional { .. }
            | Instruction::CppSpaceCommentSeparator { .. }
            | Instruction::Accept
            | Instruction::Fail => None,
        }
    }

    fn backtrack_or_resolve(
        &self,
        line: &str,
        scratch: &mut BytecodeScratch,
        pc: &mut ProgramCounter,
        position: &mut usize,
    ) -> Result<bool, BudgetExceeded> {
        loop {
            let base = scratch
                .assertions
                .last()
                .map_or(0, |assertion| assertion.backtrack_base);
            if scratch.backtrack.len() > arena_index(base) {
                let frame = scratch.backtrack.pop().expect("frame above base");
                undo_repeats_to(scratch, frame.repeat_undo_mark);
                undo_captures_to(scratch, frame.capture_undo_mark);
                scratch.call_frame = frame.call_frame;
                match frame.action {
                    ResumeAction::PopCut => {
                        // The whole atomic region failed; unwind its mark and
                        // keep failing outward.
                        scratch.cuts.pop();
                        continue;
                    }
                    ResumeAction::EnterRepeat(slot) => {
                        *pc = frame.pc;
                        *position = frame.position;
                        enter_repeat(scratch, slot, *position);
                    }
                    ResumeAction::None => {
                        *pc = frame.pc;
                        *position = frame.position;
                    }
                    ResumeAction::GiveBack(remaining) => {
                        let Instruction::ScanRepeat { node, next, .. } =
                            self.instructions[arena_index(frame.pc)]
                        else {
                            unreachable!("give-back frames belong to a ScanRepeat")
                        };
                        let back = self.give_back_one(node, line, frame.position);
                        if remaining > 1 {
                            // The undo logs were just truncated to this
                            // frame's marks, so the re-pushed frame keeps them.
                            scratch.backtrack.push(BacktrackFrame {
                                position: back,
                                action: ResumeAction::GiveBack(remaining - 1),
                                ..frame
                            });
                        }
                        *pc = next;
                        *position = back;
                    }
                }
                return Ok(true);
            }

            let Some(mut assertion) = scratch.assertions.pop() else {
                return Ok(false);
            };
            undo_repeats_to(scratch, assertion.parent_repeat_undo_mark);
            undo_captures_to(scratch, assertion.parent_capture_undo_mark);
            scratch.call_frame = assertion.parent_call_frame;
            scratch.cuts.truncate(arena_index(assertion.cut_base));
            if let Some(probe) = next_probe(line, &mut assertion) {
                let entry = assertion.entry;
                scratch.assertions.push(assertion);
                *pc = entry;
                *position = probe;
                return Ok(true);
            }
            scratch
                .backtrack
                .truncate(arena_index(assertion.backtrack_base));
            let passed = !assertion.positive;
            *position = assertion.parent_position;
            if passed {
                *pc = assertion.parent_pc;
                return Ok(true);
            }
            // The failed positive assertion is a normal failure in its parent.
        }
    }

    fn finish_assertion(
        &self,
        scratch: &mut BytecodeScratch,
        matched: bool,
        pc: &mut ProgramCounter,
        position: &mut usize,
    ) {
        let assertion = scratch.assertions.pop().expect("assertion accept");
        scratch
            .backtrack
            .truncate(arena_index(assertion.backtrack_base));
        scratch.cuts.truncate(arena_index(assertion.cut_base));
        undo_repeats_to(scratch, assertion.parent_repeat_undo_mark);
        let exports_captures = matched && assertion.positive;
        if !exports_captures {
            undo_captures_to(scratch, assertion.parent_capture_undo_mark);
        }
        scratch.call_frame = assertion.parent_call_frame;
        *position = assertion.parent_position;
        if matched == assertion.positive {
            *pc = assertion.parent_pc;
        } else {
            *pc = INVALID_PROGRAM_COUNTER;
        }
    }
}

/// Collects every numbered group with its effective flags, and the group
/// numbers that subroutine calls resolve to (named calls through
/// `named_captures`, exactly as `compile_node` resolves them).
fn collect_group_definitions<'a>(
    ast: &'a Ast,
    flags: RegexFlags,
    named_captures: &std::collections::BTreeMap<String, u32>,
    definitions: &mut std::collections::BTreeMap<u32, (&'a Ast, RegexFlags)>,
    called: &mut Vec<u32>,
) {
    if let Ast::Group {
        index: Some(index), ..
    } = ast
    {
        definitions.insert(*index, (ast, flags));
    }
    match ast {
        Ast::Concat(nodes) | Ast::Alternation(nodes) => {
            for node in nodes {
                collect_group_definitions(node, flags, named_captures, definitions, called);
            }
        }
        Ast::Conditional {
            matched, unmatched, ..
        } => {
            collect_group_definitions(matched, flags, named_captures, definitions, called);
            collect_group_definitions(unmatched, flags, named_captures, definitions, called);
        }
        Ast::Flags {
            flags: local,
            child,
        } => collect_group_definitions(child, *local, named_captures, definitions, called),
        Ast::Repeat { node, .. }
        | Ast::Group { child: node, .. }
        | Ast::Look { child: node, .. } => {
            collect_group_definitions(node, flags, named_captures, definitions, called);
        }
        Ast::Subroutine(call) => {
            let target = match &call.target {
                Backref::Number(group) => Some(*group),
                Backref::Name(name) => named_captures.get(name).copied(),
            };
            called.extend(target);
        }
        Ast::Empty
        | Ast::Literal(_)
        | Ast::Dot
        | Ast::Grapheme
        | Ast::Class(_)
        | Ast::Anchor(_)
        | Ast::Backref(_)
        | Ast::Unsupported(_) => {}
    }
}

#[cfg(test)]
fn ordered_fanout_score(ast: &Ast) -> usize {
    match ast {
        Ast::Alternation(branches) => {
            branches.len().saturating_sub(1)
                + branches.iter().map(ordered_fanout_score).sum::<usize>()
        }
        Ast::Repeat { node, .. } => {
            usize::from(!matches!(
                node.as_ref(),
                Ast::Literal(_) | Ast::Class(_) | Ast::Dot
            )) + ordered_fanout_score(node)
        }
        Ast::Concat(nodes) => nodes.iter().map(ordered_fanout_score).sum(),
        Ast::Group { child, .. } | Ast::Flags { child, .. } | Ast::Look { child, .. } => {
            ordered_fanout_score(child)
        }
        Ast::Conditional {
            matched, unmatched, ..
        } => 1 + ordered_fanout_score(matched) + ordered_fanout_score(unmatched),
        Ast::Empty
        | Ast::Literal(_)
        | Ast::Dot
        | Ast::Grapheme
        | Ast::Class(_)
        | Ast::Anchor(_)
        | Ast::Backref(_)
        | Ast::Subroutine(_)
        | Ast::Unsupported(_) => 0,
    }
}

impl BytecodeScratch {
    /// A generation no restore stamp holds yet.
    fn next_restore_generation(&mut self) -> u32 {
        self.restore_generation = self.restore_generation.wrapping_add(1);
        if self.restore_generation == 0 {
            self.repeat_restore_stamps.fill(0);
            self.capture_restore_stamps.fill(0);
            self.restore_generation = 1;
        }
        self.restore_generation
    }

    fn reset(&mut self, repeat_slots: usize, capture_slots: usize) {
        self.backtrack.clear();
        self.assertions.clear();
        self.repeat_undo.clear();
        self.capture_undo.clear();
        self.calls.clear();
        self.repeat_restore_stamps.resize(repeat_slots, 0);
        self.capture_restore_stamps.resize(capture_slots, 0);
        self.call_frame = 0;
        self.cuts.clear();
        self.repeats.resize(repeat_slots, RepeatState::default());
        // Every repeat entry executes RepeatInit before its slot can be read.
        // Leaving top-level slots stale avoids clearing the whole repeat arena
        // for every exact-start probe; nested/recursive reuse is still
        // restored through the undo log populated by RepeatInit.
        self.captures.resize(capture_slots, CaptureState::Unset);
        self.captures.fill(CaptureState::Unset);
    }
}

fn set_repeat(scratch: &mut BytecodeScratch, slot: VmSlot, value: RepeatState) {
    let index = arena_index(slot);
    let old = scratch.repeats[index];
    scratch.repeat_undo.push(RepeatUndo::new(slot, old));
    scratch.repeats[index] = value;
}

fn set_capture(scratch: &mut BytecodeScratch, slot: VmSlot, value: CaptureState) {
    let index = arena_index(slot);
    let old = std::mem::replace(&mut scratch.captures[index], value);
    scratch.capture_undo.push((slot, old));
}

fn enter_repeat(scratch: &mut BytecodeScratch, slot: VmSlot, position: usize) {
    let mut value = scratch.repeats[arena_index(slot)];
    value.count = value.count.saturating_add(1);
    value.last_position = position;
    value.stalled = Stall::Advanced;
    set_repeat(scratch, slot, value);
}

fn undo_repeats_to(scratch: &mut BytecodeScratch, mark: u32) {
    while scratch.repeat_undo.len() > arena_index(mark) {
        let undo = scratch.repeat_undo.pop().expect("repeat undo above mark");
        if undo.is_credit() {
            scratch.calls[undo.last_position].nested_scanned = undo.count;
        } else {
            scratch.repeats[undo.slot()] = undo.state();
        }
    }
}

fn undo_captures_to(scratch: &mut BytecodeScratch, mark: u32) {
    while scratch.capture_undo.len() > arena_index(mark) {
        let (slot, value) = scratch.capture_undo.pop().expect("capture undo above mark");
        scratch.captures[arena_index(slot)] = value;
    }
}

fn first_probe(
    line: &str,
    position: usize,
    direction: AssertDirection,
    frame: &mut AssertionFrame,
) -> Option<usize> {
    if direction.is_ahead() {
        return Some(position);
    }
    let latest = position.checked_sub(direction.min_width())?;
    let earliest = direction
        .max_width()
        .map_or(0, |max| position.saturating_sub(max));
    let probe = boundary_at_or_before(line, latest, earliest)?;
    set_next_probe(frame, probe.checked_sub(1).filter(|next| *next >= earliest));
    Some(probe)
}

fn next_probe(line: &str, frame: &mut AssertionFrame) -> Option<usize> {
    if frame.direction.is_ahead() || !frame.has_next_probe {
        return None;
    }
    let latest = frame.target_end.checked_sub(frame.direction.min_width())?;
    let earliest = frame
        .direction
        .max_width()
        .map_or(0, |max| frame.target_end.saturating_sub(max));
    let probe = boundary_at_or_before(line, frame.next_probe.min(latest), earliest)?;
    set_next_probe(frame, probe.checked_sub(1).filter(|next| *next >= earliest));
    Some(probe)
}

fn set_next_probe(frame: &mut AssertionFrame, next: Option<usize>) {
    frame.has_next_probe = next.is_some();
    frame.next_probe = next.unwrap_or(0);
}

fn boundary_at_or_before(line: &str, mut position: usize, earliest: usize) -> Option<usize> {
    loop {
        if line.is_char_boundary(position) {
            return Some(position);
        }
        if position == earliest {
            return None;
        }
        position = position.saturating_sub(1);
        if position < earliest {
            return None;
        }
    }
}

struct Compiler<'a> {
    instructions: Vec<Instruction>,
    literals: Vec<String>,
    literal_tries: Vec<LiteralTrie>,
    classes: Vec<CompiledClass>,
    repeat_slots: VmSlot,
    capture_layout: Vec<u32>,
    named_captures: std::collections::BTreeMap<String, u32>,
    duplicate_names: std::collections::BTreeMap<String, Vec<u32>>,
    routine_entries: std::collections::BTreeMap<u32, ProgramCounter>,
    /// Loops whose body is being compiled. Their `Repeat` instruction is
    /// still a placeholder, so continuation analysis reads the loop's exit
    /// and body from here instead.
    open_repeats: Vec<OpenRepeat<'a>>,
    /// Number of `Split` instructions, each owning one guard cell.
    split_guards: u32,
    literal_ids: crate::engine::hashing::FastMap<&'a str, LiteralId>,
    /// Whether any live capture sits under an alternation or repeat. When
    /// not, no alternation needs its branches searched for live captures.
    /// Computed by `compile` unless the caller already knows.
    captures_under_choice: Option<bool>,
}

struct OpenRepeat<'a> {
    placeholder: ProgramCounter,
    exit: ProgramCounter,
    body: &'a Ast,
    flags: RegexFlags,
    /// `Some(1)` loops exit after their only iteration.
    max: Option<usize>,
}

/// Bounds the bytecode walked when proving a repeat can be made possessive.
/// Continuations almost always reveal their first consumer within a few
/// instructions; giving up early only forgoes the optimization.
const POSSESSIVE_CONTINUATION_STEPS: u32 = 64;

impl<'a> Compiler<'a> {
    fn new() -> Self {
        Self {
            instructions: Vec::new(),
            literals: Vec::new(),
            literal_tries: Vec::new(),
            classes: Vec::new(),
            repeat_slots: 0,
            capture_layout: Vec::new(),
            named_captures: std::collections::BTreeMap::new(),
            duplicate_names: std::collections::BTreeMap::new(),
            routine_entries: std::collections::BTreeMap::new(),
            open_repeats: Vec::new(),
            split_guards: 0,
            literal_ids: crate::engine::hashing::fast_map(),
            captures_under_choice: None,
        }
    }

    fn with_captures(capture_layout: Vec<u32>) -> Self {
        Self {
            capture_layout,
            ..Self::new()
        }
    }

    fn compile(mut self, parsed: &'a ParsedRegex) -> Result<Program, CompileError> {
        self.named_captures.clone_from(&parsed.named_captures);
        self.duplicate_names.clone_from(&parsed.duplicate_names);
        if self.captures_under_choice.is_none() {
            self.captures_under_choice = Some(
                !self.capture_layout.is_empty()
                    && !live_captures_keep_selection_shape(&parsed.ast, &self.capture_layout),
            );
        }
        self.instructions
            .reserve(parsed.analysis().instruction_capacity_hint());
        if !self.capture_layout.is_empty() && parsed.features.subroutine {
            let mut definitions = std::collections::BTreeMap::new();
            let mut called = Vec::new();
            collect_group_definitions(
                &parsed.ast,
                parsed.flags,
                &parsed.named_captures,
                &mut definitions,
                &mut called,
            );
            // Only called groups need out-of-line routine bodies. Inline
            // captures remain in the main program; nested capturing groups
            // otherwise duplicate each subtree once per enclosing group.
            called.sort_unstable();
            called.dedup();
            definitions.retain(|group, _| called.binary_search(group).is_ok());
            for group in definitions.keys() {
                let placeholder = self.push(Instruction::Fail);
                self.routine_entries.insert(*group, placeholder);
            }
            for (group, (node, flags)) in definitions {
                let return_pc = self.push(Instruction::Return);
                let actual = self.compile_node(node, flags, return_pc)?;
                let placeholder = self.routine_entries[&group];
                self.instructions[arena_index(placeholder)] = Instruction::Jump { target: actual };
            }
        }
        let accept = self.push(Instruction::Accept);
        let entry = self.compile_node(&parsed.ast, parsed.flags, accept)?;
        Ok(Program {
            instructions: self.instructions,
            literals: self.literals,
            literal_tries: self.literal_tries,
            classes: self.classes,
            entry,
            repeat_slots: self.repeat_slots,
            capture_layout: self.capture_layout,
            guards: GuardCell::cells(
                arena_index(self.split_guards) + arena_index(self.repeat_slots),
            ),
            repeat_guard_base: self.split_guards,
        })
    }

    fn compile_node(
        &mut self,
        ast: &'a Ast,
        flags: RegexFlags,
        next: ProgramCounter,
    ) -> Result<ProgramCounter, CompileError> {
        Ok(match ast {
            Ast::Empty => next,
            Ast::Literal(value) => {
                let id = self.intern_literal(value)?;
                self.push(Instruction::Literal {
                    id,
                    flags: flags.into(),
                    next,
                })
            }
            Ast::Dot => self.push(Instruction::Any {
                flags: flags.into(),
                next,
            }),
            Ast::Class(class) => {
                let id = self.intern_class(class)?;
                self.push(Instruction::Class {
                    id,
                    flags: flags.into(),
                    next,
                })
            }
            Ast::Anchor(kind) => self.push(Instruction::Anchor { kind: *kind, next }),
            Ast::Concat(nodes) => {
                let mut entry = next;
                for node in nodes.iter().rev() {
                    entry = self.compile_node(node, flags, entry)?;
                }
                entry
            }
            Ast::Alternation(branches) => {
                let captures_can_be_elided = self.captures_under_choice == Some(false)
                    || !branches_contain_live_capture(branches, &self.capture_layout);
                if captures_can_be_elided && is_cpp_space_comment_separator(branches) {
                    return Ok(self.push(Instruction::CppSpaceCommentSeparator { next }));
                }
                let mut entries = Vec::with_capacity(branches.len());
                let mut branch = 0;
                while branch < branches.len() {
                    // Large grammar closures often contain a mostly-literal
                    // keyword alternation with a few structured variants
                    // (`foo|bar|create( or alter)?|...`), and entity or
                    // attribute inventories spelled as nested prefix trees
                    // (`a(s(ymp(eq)?|cr)|nd)|...`). Every capture-free branch
                    // that denotes a finite set of strings expands, in
                    // backtracking priority order, into one reusable trie
                    // instead of a Split chain whose negative probes walk the
                    // whole closure. Keeping runs contiguous preserves
                    // ordered-alternation priority around structured
                    // branches.
                    if captures_can_be_elided {
                        let run_start = branch;
                        let mut run = FiniteSize::NONE;
                        while let Some(size) = branches.get(branch).and_then(|branch| {
                            finite_language_size(branch, flags, run.remaining_limits())
                        }) {
                            run = run.union(size);
                            branch += 1;
                        }
                        if branch > run_start {
                            let trie = if run.worth_a_trie() {
                                self.intern_finite_trie(&branches[run_start..branch], flags, run)?
                            } else {
                                None
                            };
                            if let Some(id) = trie {
                                entries.push(self.push(Instruction::LiteralTrie {
                                    id,
                                    flags: flags.into(),
                                    next,
                                }));
                            } else {
                                for branch in &branches[run_start..branch] {
                                    entries.push(self.compile_node(branch, flags, next)?);
                                }
                            }
                            continue;
                        }
                    }
                    entries.push(self.compile_node(&branches[branch], flags, next)?);
                    branch += 1;
                }
                let mut entry = entries.pop().unwrap_or(next);
                for preferred in entries.into_iter().rev() {
                    let guard = self.split_guards;
                    self.split_guards = guard.checked_add(1).ok_or(CompileError::TableOverflow)?;
                    entry = self.push(Instruction::Split {
                        preferred,
                        alternate: entry,
                        guard,
                    });
                }
                entry
            }
            Ast::Repeat {
                node,
                min,
                max,
                greedy,
                possessive,
                atomic,
            } => {
                // Possessive exact-count repeats ({n}+) have nothing to give
                // back, so only atomic groups and variable-width possessive
                // repeats commit via an explicit cut. Mirrors the recursive VM.
                let cut = *possessive && (*atomic || *max != Some(*min));
                // A greedy repeat whose continuation provably rejects every
                // character the body can consume never profits from giving
                // characters back, so it can run as a possessive scan.
                let auto_possessive = !*possessive
                    && *greedy
                    && *max != Some(0)
                    && *max != Some(1)
                    && ascii_consumer_members(node, flags)
                        .is_some_and(|members| self.continuation_rejects(next, &members));
                if (cut || auto_possessive)
                    && let Some(scan) = self.scan_node(node, flags)
                {
                    let (scan, scan_flags) = scan;
                    return Ok(self.push(Instruction::ScanRepeat {
                        node: scan,
                        flags: scan_flags.into(),
                        give_back: false,
                        bounds: RepeatBounds::new(*min, *max)?,
                        next,
                    }));
                }
                // Automatic possessification (as Oniguruma does): when no
                // character the repeated node can start with can begin a
                // successful continuation, giving characters back can never
                // succeed, so the greedy repeat is equivalent to its
                // possessive form and needs no backtrack frame per iteration.
                if !*possessive
                    && *greedy
                    && *max != Some(*min)
                    && *max != Some(0)
                    && let Some((scan, scan_flags)) = self.scan_node(node, flags)
                    && let Some(continuation) = self.continuation_first_chars(next)
                    && self
                        .scan_first_chars(scan, scan_flags)
                        .is_disjoint(&continuation)
                {
                    return Ok(self.push(Instruction::ScanRepeat {
                        node: scan,
                        flags: scan_flags.into(),
                        give_back: false,
                        bounds: RepeatBounds::new(*min, *max)?,
                        next,
                    }));
                }
                // Any other greedy single-consumer repeat still scans in
                // place, keeping one give-back frame instead of a loop slot,
                // frame, and undo entries per iteration. Case-insensitive
                // literals are excluded because their iterations can differ
                // in width.
                if !*possessive
                    && *greedy
                    && *max != Some(0)
                    && let Some((scan, scan_flags)) = self.scan_node(node, flags)
                    && !(matches!(scan, ScanNode::Literal(_)) && scan_flags.case_insensitive)
                {
                    return Ok(self.push(Instruction::ScanRepeat {
                        node: scan,
                        flags: scan_flags.into(),
                        give_back: true,
                        bounds: RepeatBounds::new(*min, *max)?,
                        next,
                    }));
                }
                let exit = if cut {
                    self.push(Instruction::CutEnd { next })
                } else {
                    next
                };
                let entry = if *max == Some(0) {
                    exit
                } else if *min == 1 && *max == Some(1) {
                    self.compile_node(node, flags, exit)?
                } else {
                    let slot = self.repeat_slots;
                    self.repeat_slots = self
                        .repeat_slots
                        .checked_add(1)
                        .ok_or(CompileError::TableOverflow)?;
                    let repeat = self.push(Instruction::Fail);
                    let end = self.push(Instruction::RepeatEnd { slot, repeat });
                    self.open_repeats.push(OpenRepeat {
                        placeholder: repeat,
                        exit,
                        body: node,
                        flags,
                        max: *max,
                    });
                    let body = self.compile_node(node, flags, end);
                    self.open_repeats.pop();
                    let body = body?;
                    self.instructions[arena_index(repeat)] = Instruction::Repeat {
                        slot,
                        bounds: RepeatBounds::new(*min, *max)?,
                        greedy: *greedy,
                        body,
                        next: exit,
                    };
                    self.push(Instruction::RepeatInit { slot, next: repeat })
                };
                if cut {
                    self.push(Instruction::CutStart { next: entry })
                } else {
                    entry
                }
            }
            Ast::Group { index, child, .. } => {
                if let Some(slot) = index
                    .and_then(|index| {
                        self.capture_layout
                            .binary_search(&index)
                            .ok()
                            .filter(|slot| *slot != 0)
                    })
                    .map(vm_slot)
                    .transpose()?
                {
                    let end = self.push(Instruction::SaveEnd { slot, next });
                    let child = self.compile_node(child, flags, end)?;
                    self.push(Instruction::SaveStart { slot, next: child })
                } else {
                    self.compile_node(child, flags, next)?
                }
            }
            Ast::Look { kind, child } => {
                let accept = self.push(Instruction::Accept);
                let entry = self.compile_node(child, flags, accept)?;
                let (positive, direction) = match kind {
                    LookKind::Ahead => (true, AssertDirection::AHEAD),
                    LookKind::NotAhead => (false, AssertDirection::AHEAD),
                    LookKind::Behind => (true, lookbehind_direction(child, flags)?),
                    LookKind::NotBehind => (false, lookbehind_direction(child, flags)?),
                };
                self.push(Instruction::Assert {
                    entry,
                    positive,
                    direction,
                    next,
                })
            }
            Ast::Flags {
                flags: local,
                child,
            } => self.compile_node(child, *local, next)?,
            Ast::Backref(backref) => {
                let group = match backref {
                    Backref::Number(group) => *group,
                    // A shared name refers to several groups; the evaluator
                    // implements Oniguruma's choice among them.
                    Backref::Name(name) if self.duplicate_names.contains_key(name) => {
                        return Err(CompileError::Backreference);
                    }
                    Backref::Name(name) => self
                        .named_captures
                        .get(name)
                        .copied()
                        .ok_or(CompileError::Backreference)?,
                };
                let slot = vm_slot(
                    self.capture_layout
                        .binary_search(&group)
                        .map_err(|_| CompileError::Backreference)?,
                )?;
                self.push(Instruction::Backref {
                    slot,
                    flags: flags.into(),
                    next,
                })
            }
            Ast::Conditional {
                condition,
                matched,
                unmatched,
            } => {
                let group = match condition {
                    Backref::Number(group) => *group,
                    // The evaluator checks every group of a shared name.
                    Backref::Name(name) if self.duplicate_names.contains_key(name) => {
                        return Err(CompileError::Conditional);
                    }
                    Backref::Name(name) => self
                        .named_captures
                        .get(name)
                        .copied()
                        .ok_or(CompileError::Conditional)?,
                };
                let slot = vm_slot(
                    self.capture_layout
                        .binary_search(&group)
                        .map_err(|_| CompileError::Conditional)?,
                )?;
                let matched = self.compile_node(matched, flags, next)?;
                let unmatched = self.compile_node(unmatched, flags, next)?;
                self.push(Instruction::Conditional {
                    slot,
                    matched,
                    unmatched,
                })
            }
            Ast::Subroutine(call) => {
                let group = match &call.target {
                    Backref::Number(group) => *group,
                    Backref::Name(name) => self
                        .named_captures
                        .get(name)
                        .copied()
                        .ok_or(CompileError::Subroutine)?,
                };
                let entry = self
                    .routine_entries
                    .get(&group)
                    .copied()
                    .ok_or(CompileError::Subroutine)?;
                self.push(Instruction::Call { entry, next })
            }
            Ast::Grapheme | Ast::Unsupported(_) => return Err(CompileError::Unsupported),
        })
    }

    /// True when every path from `pc` fails before consuming anything if
    /// the character at the current position is one of `members` (all
    /// ASCII). Zero-width instructions are looked through; any instruction
    /// the walk does not understand, or reaching `Accept`, answers false.
    ///
    /// Used to auto-possessify a greedy single-character repeat: giving back
    /// a character leaves a member at the current position, so every such
    /// backtracking path fails and dropping it cannot change the result.
    fn continuation_rejects(&self, pc: ProgramCounter, members: &AsciiMask) -> bool {
        const WALK_LIMIT: usize = 64;
        let member_bytes = || (0u8..128).filter(|byte| ascii_mask_contains(members, *byte));
        let class_rejects = |id: ClassId, flags: InstructionFlags| {
            let class = &self.classes[id.0 as usize];
            member_bytes().all(|byte| !class.matches_ascii(byte, flags.case_insensitive()))
        };
        let literal_rejects = |id: LiteralId, flags: InstructionFlags| {
            let first = self.literals[id.0 as usize].chars().next()?;
            Some(member_bytes().all(|byte| {
                let byte = byte as char;
                byte != first && !(flags.case_insensitive() && unicode_case_eq(first, byte))
            }))
        };
        let mut pending = vec![pc];
        let mut visited = 0usize;
        while let Some(pc) = pending.pop() {
            visited += 1;
            if visited > WALK_LIMIT {
                return false;
            }
            match &self.instructions[arena_index(pc)] {
                Instruction::SaveStart { next, .. }
                | Instruction::SaveEnd { next, .. }
                | Instruction::Anchor { next, .. }
                | Instruction::CutStart { next }
                | Instruction::CutEnd { next }
                | Instruction::RepeatInit { next, .. } => pending.push(*next),
                Instruction::Jump { target } => pending.push(*target),
                Instruction::Split {
                    preferred,
                    alternate,
                    ..
                } => pending.extend([*preferred, *alternate]),
                Instruction::Conditional {
                    matched, unmatched, ..
                } => pending.extend([*matched, *unmatched]),
                // Reached through `RepeatInit`, so the count is zero.
                Instruction::Repeat {
                    bounds, body, next, ..
                } => {
                    pending.push(*body);
                    if bounds.min == 0 {
                        pending.push(*next);
                    }
                }
                Instruction::Assert {
                    entry,
                    positive,
                    direction,
                    next,
                } => {
                    let guard = direction
                        .is_ahead()
                        .then(|| self.single_class_body(*entry))
                        .flatten();
                    let rejects = guard.is_some_and(|(id, flags)| {
                        let class = &self.classes[id.0 as usize];
                        member_bytes().all(|byte| {
                            class.matches_ascii(byte, flags.case_insensitive()) != *positive
                        })
                    });
                    if !rejects {
                        pending.push(*next);
                    }
                }
                Instruction::Class { id, flags, .. } => {
                    if !class_rejects(*id, *flags) {
                        return false;
                    }
                }
                Instruction::Literal { id, flags, next } => match literal_rejects(*id, *flags) {
                    Some(true) => {}
                    Some(false) => return false,
                    None => pending.push(*next),
                },
                Instruction::LiteralTrie { id, flags, .. } => {
                    let trie = &self.literal_tries[id.0 as usize];
                    let Some(root) = trie.nodes.first() else {
                        return false;
                    };
                    if !trie.unicode_nodes.is_empty() || root.terminal_order.is_some() {
                        return false;
                    }
                    let rejects = member_bytes().all(|byte| {
                        let byte = if flags.case_insensitive() {
                            byte.to_ascii_lowercase()
                        } else {
                            byte
                        };
                        trie.edge(0, byte).is_none()
                    });
                    if !rejects {
                        return false;
                    }
                }
                Instruction::ScanRepeat {
                    node,
                    flags,
                    bounds,
                    next,
                    ..
                } => {
                    let rejects = match node {
                        ScanNode::Class(id) => class_rejects(*id, *flags),
                        ScanNode::Literal(id) => literal_rejects(*id, *flags) == Some(true),
                        ScanNode::Any => false,
                    };
                    if !rejects {
                        return false;
                    }
                    if bounds.min == 0 {
                        pending.push(*next);
                    }
                }
                Instruction::Any { .. }
                | Instruction::Accept
                | Instruction::Fail
                | Instruction::Call { .. }
                | Instruction::Return
                | Instruction::RepeatEnd { .. }
                | Instruction::Backref { .. }
                | Instruction::CppSpaceCommentSeparator { .. } => return false,
            }
        }
        true
    }

    /// `(id, flags)` when an assertion body is exactly one class followed by
    /// `Accept`.
    fn single_class_body(&self, entry: ProgramCounter) -> Option<(ClassId, InstructionFlags)> {
        match &self.instructions[arena_index(entry)] {
            Instruction::Class { id, flags, next }
                if matches!(self.instructions[arena_index(*next)], Instruction::Accept) =>
            {
                Some((*id, *flags))
            }
            _ => None,
        }
    }

    fn push(&mut self, instruction: Instruction) -> ProgramCounter {
        let index = program_counter(self.instructions.len())
            .expect("bytecode program exceeds compact program-counter space");
        self.instructions.push(instruction);
        index
    }

    /// Extracts a single-consumer body for `ScanRepeat`, looking through flag
    /// scopes and non-captured groups. Empty literals are rejected because a
    /// scan must always make progress.
    fn scan_node(&mut self, ast: &'a Ast, flags: RegexFlags) -> Option<(ScanNode, RegexFlags)> {
        match ast {
            Ast::Literal(value) if !value.is_empty() => {
                let id = self.intern_literal(value).ok()?;
                Some((ScanNode::Literal(id), flags))
            }
            Ast::Class(class) => {
                let id = self.intern_class(class).ok()?;
                Some((ScanNode::Class(id), flags))
            }
            Ast::Dot => Some((ScanNode::Any, flags)),
            Ast::Flags {
                flags: local,
                child,
            } => self.scan_node(child, *local),
            Ast::Group { index, child, .. } => {
                let captured = index.is_some_and(|index| {
                    self.capture_layout
                        .binary_search(&index)
                        .is_ok_and(|slot| slot != 0)
                });
                if captured {
                    None
                } else {
                    self.scan_node(child, flags)
                }
            }
            _ => None,
        }
    }

    fn scan_first_chars(&self, node: ScanNode, flags: RegexFlags) -> FirstChars {
        match node {
            ScanNode::Literal(id) => {
                FirstChars::of_literal(&self.literals[id.0 as usize], flags.case_insensitive)
            }
            ScanNode::Class(id) => {
                FirstChars::of_class(&self.classes[id.0 as usize], flags.case_insensitive)
            }
            ScanNode::Any => FirstChars::ALL,
        }
    }

    /// Superset of the characters a successful match can consume first when
    /// execution resumes at `pc`. `None` means a match may succeed without
    /// that first character being constrained (for example by accepting
    /// immediately) or that the continuation could not be analyzed.
    fn continuation_first_chars(&self, pc: ProgramCounter) -> Option<FirstChars> {
        let mut steps = POSSESSIVE_CONTINUATION_STEPS;
        self.first_chars_from(pc, &mut steps, &mut Vec::new())
    }

    fn first_chars_from(
        &self,
        pc: ProgramCounter,
        steps: &mut u32,
        visited: &mut Vec<ProgramCounter>,
    ) -> Option<FirstChars> {
        // A revisited instruction adds no new first consumer to the union
        // already being accumulated for it.
        if visited.contains(&pc) {
            return Some(FirstChars::NONE);
        }
        *steps = steps.checked_sub(1)?;
        visited.push(pc);
        match &self.instructions[arena_index(pc)] {
            Instruction::Literal { id, flags, next } => {
                let literal = &self.literals[id.0 as usize];
                if literal.is_empty() {
                    self.first_chars_from(*next, steps, visited)
                } else {
                    Some(FirstChars::of_literal(literal, flags.case_insensitive()))
                }
            }
            Instruction::Class { id, flags, .. } => Some(FirstChars::of_class(
                &self.classes[id.0 as usize],
                flags.case_insensitive(),
            )),
            Instruction::Any { .. } => Some(FirstChars::ALL),
            Instruction::ScanRepeat {
                node,
                flags,
                bounds,
                next,
                ..
            } => {
                let mut first = self.scan_first_chars(*node, flags.regex());
                if bounds.min == 0 {
                    first.union(&self.first_chars_from(*next, steps, visited)?);
                }
                Some(first)
            }
            Instruction::Anchor { next, .. }
            | Instruction::RepeatInit { next, .. }
            | Instruction::SaveStart { next, .. }
            | Instruction::SaveEnd { next, .. }
            | Instruction::CutStart { next }
            | Instruction::CutEnd { next } => self.first_chars_from(*next, steps, visited),
            Instruction::Jump { target } => self.first_chars_from(*target, steps, visited),
            Instruction::Split {
                preferred,
                alternate,
                ..
            } => {
                let mut first = self.first_chars_from(*preferred, steps, visited)?;
                first.union(&self.first_chars_from(*alternate, steps, visited)?);
                Some(first)
            }
            Instruction::Repeat { body, next, .. } => {
                let mut first = self.first_chars_from(*body, steps, visited)?;
                first.union(&self.first_chars_from(*next, steps, visited)?);
                Some(first)
            }
            Instruction::RepeatEnd { repeat, .. } => {
                if matches!(
                    self.instructions[arena_index(*repeat)],
                    Instruction::Repeat { .. }
                ) {
                    return self.first_chars_from(*repeat, steps, visited);
                }
                let open = self
                    .open_repeats
                    .iter()
                    .rev()
                    .find(|open| open.placeholder == *repeat)?;
                let mut first = self.first_chars_from(open.exit, steps, visited)?;
                if open.max != Some(1) {
                    first.union(&FirstChars::of_ast(open.body, open.flags).0);
                }
                Some(first)
            }
            Instruction::Assert {
                entry,
                positive,
                direction,
                next,
            } => {
                if !*positive || !direction.is_ahead() {
                    // Negative and behind assertions only narrow the paths
                    // through `next`.
                    return self.first_chars_from(*next, steps, visited);
                }
                // A positive lookahead must match at the same position as
                // the continuation, so both constraints hold for that
                // character. Instructions reached under this intersection
                // contribute only narrowed sets, so they must not be marked
                // as fully accounted for outside it. The lookahead body is
                // separate code ending in its own `Accept`.
                let rest = self.first_chars_from(*next, steps, &mut visited.clone());
                let ahead = self.first_chars_from(*entry, steps, &mut Vec::new());
                match (ahead, rest) {
                    (Some(mut ahead), Some(rest)) => {
                        ahead.intersect(&rest);
                        Some(ahead)
                    }
                    (Some(first), None) | (None, Some(first)) => Some(first),
                    (None, None) => None,
                }
            }
            Instruction::LiteralTrie { id, flags, next } => {
                let trie = &self.literal_tries[id.0 as usize];
                let mut first = trie.non_empty_first_chars(flags.case_insensitive())?;
                if trie.nodes[0].terminal_order.is_some() {
                    first.union(&self.first_chars_from(*next, steps, visited)?);
                }
                Some(first)
            }
            // Accepting, calls, backreferences, conditionals, the
            // specialized separator, and placeholders are not analyzed.
            Instruction::Accept
            | Instruction::Fail
            | Instruction::Call { .. }
            | Instruction::Return
            | Instruction::Backref { .. }
            | Instruction::Conditional { .. }
            | Instruction::CppSpaceCommentSeparator { .. } => None,
        }
    }

    fn intern_literal(&mut self, literal: &'a str) -> Result<LiteralId, CompileError> {
        // Captured keyword alternations intern hundreds of literals; a
        // linear scan per literal made their compilation quadratic.
        if let Some(id) = self.literal_ids.get(literal) {
            return Ok(*id);
        }
        let id =
            LiteralId(u32::try_from(self.literals.len()).map_err(|_| CompileError::TableOverflow)?);
        self.literals.push(literal.to_owned());
        self.literal_ids.insert(literal, id);
        Ok(id)
    }

    fn intern_class(&mut self, class: &CharClass) -> Result<ClassId, CompileError> {
        let index = match self.classes.iter().position(|value| value.source == *class) {
            Some(index) => index,
            None => {
                self.classes.push(CompiledClass::new(class.clone()));
                self.classes.len() - 1
            }
        };
        u32::try_from(index)
            .map(ClassId)
            .map_err(|_| CompileError::TableOverflow)
    }

    /// Builds one trie from alternation branches that
    /// [`finite_language_size`] admitted with combined size `size`.
    fn intern_finite_trie(
        &mut self,
        branches: &'a [Ast],
        flags: RegexFlags,
        size: FiniteSize,
    ) -> Result<Option<LiteralTrieId>, CompileError> {
        let id =
            u32::try_from(self.literal_tries.len()).map_err(|_| CompileError::TableOverflow)?;
        let mut pending = Vec::new();
        let trie = if flags.case_insensitive && size.non_ascii {
            // Unicode case-insensitive inventories need the scalar trie.
            let mut strings = FiniteStrings::default();
            for branch in branches {
                enumerate_finite_language(branch, flags, &mut pending, &mut strings)?;
            }
            let Some(trie) = LiteralTrie::new(&strings.strings(), flags)? else {
                return Ok(None);
            };
            trie
        } else {
            // Byte tries are built while enumerating, so a shared prefix is
            // walked once rather than once per expanded string.
            let mut builder =
                ByteTrieBuilder::new(size.bytes.saturating_add(1), flags.case_insensitive);
            for branch in branches {
                if let Ast::Literal(literal) = branch {
                    let node = builder.insert(literal)?;
                    builder.emit(node)?;
                } else {
                    enumerate_finite_language(branch, flags, &mut pending, &mut builder)?;
                }
            }
            builder.finish()
        };
        self.literal_tries.push(trie);
        Ok(Some(LiteralTrieId(id)))
    }
}

/// Sorted, deduplicated case variants of one trie edge scalar.
#[derive(Clone, Copy, PartialEq, Eq)]
struct EdgeVariants {
    chars: [char; 5],
    len: u8,
}

impl EdgeVariants {
    fn new(ch: char) -> Self {
        let mut this = Self {
            chars: ['\0'; 5],
            len: 0,
        };
        for variant in CaseVariants::new(ch).iter() {
            let members = &this.chars[..usize::from(this.len)];
            if let Err(at) = members.binary_search(&variant) {
                this.chars.copy_within(at..usize::from(this.len), at + 1);
                this.chars[at] = variant;
                this.len += 1;
            }
        }
        this
    }

    fn members(&self) -> &[char] {
        &self.chars[..usize::from(self.len)]
    }
}

/// The existing edge `ch` joins, `Ok(None)` for a new edge, or `Err(())` when
/// the scalar trie cannot represent it. Lookup follows the one edge that is
/// case-equal to the input, but case equality is not transitive (`ϑ` and `ϴ`
/// both equal `θ`, not each other). Edges therefore merge only scalars with
/// identical case variants, and no input may be case-equal to two edges.
fn unicode_edge(
    edges: &LiteralTrieEdges<CaseFoldKey>,
    ch: &CaseFoldKey,
) -> Result<Option<u32>, ()> {
    // Existing edges are pairwise unambiguous, and a scalar with the same
    // case mappings as an edge has the same variants, so it joins that edge
    // without re-checking the others.
    if let Some((_, child)) = edges.iter().find(|(edge, _)| edge.same_mappings(ch)) {
        return Ok(Some(*child));
    }
    if matches!(edges, LiteralTrieEdges::Empty) {
        return Ok(None);
    }
    let wanted = EdgeVariants::new(ch.ch());
    let wanted_keys = wanted
        .members()
        .iter()
        .map(|variant| CaseFoldKey::new(*variant))
        .collect::<Vec<_>>();
    let mut joined = None;
    for (edge, child) in edges.iter() {
        // An edge shares a variant with `ch` exactly when it is case-equal
        // to one of `ch`'s variants.
        if !wanted_keys.iter().any(|variant| edge.case_eq(variant)) {
            continue;
        }
        if EdgeVariants::new(edge.ch()) == wanted {
            joined = Some(*child);
        } else {
            return Err(());
        }
    }
    Ok(joined)
}

impl LiteralTrie {
    /// `None` when Unicode case folding makes the scalar trie ambiguous (see
    /// `unicode_edge`).
    fn new<S: AsRef<str>>(literals: &[S], flags: RegexFlags) -> Result<Option<Self>, CompileError> {
        let unicode =
            flags.case_insensitive && literals.iter().any(|literal| !literal.as_ref().is_ascii());
        let node_capacity = literals
            .iter()
            .fold(1usize, |nodes, literal| {
                nodes.saturating_add(if unicode {
                    literal.as_ref().chars().count()
                } else {
                    literal.as_ref().len()
                })
            })
            .min(LITERAL_TRIE_NODE_RESERVE_LIMIT);
        if unicode {
            let mut trie = Self {
                unicode_nodes: Vec::with_capacity(node_capacity),
                ..Self::default()
            };
            trie.unicode_nodes.push(UnicodeLiteralTrieNode::default());
            for (order, literal) in literals.iter().enumerate() {
                let order = u32::try_from(order).map_err(|_| CompileError::TableOverflow)?;
                let mut node = 0usize;
                for ch in literal.as_ref().chars() {
                    let key = CaseFoldKey::new(ch);
                    let Ok(edge) = unicode_edge(&trie.unicode_nodes[node].edges, &key) else {
                        return Ok(None);
                    };
                    node = if let Some(child) = edge {
                        child as usize
                    } else {
                        let child = u32::try_from(trie.unicode_nodes.len())
                            .map_err(|_| CompileError::TableOverflow)?;
                        trie.unicode_nodes.push(UnicodeLiteralTrieNode::default());
                        trie.unicode_nodes[node].edges.push((key, child));
                        child as usize
                    };
                }
                let terminal = &mut trie.unicode_nodes[node].terminal_order;
                if terminal.is_none_or(|existing| order < existing) {
                    *terminal = Some(order);
                }
            }
            return Ok(Some(trie));
        }
        let mut builder = ByteTrieBuilder::new(node_capacity, flags.case_insensitive);
        for literal in literals {
            let node = builder.insert(literal.as_ref())?;
            builder.emit(node)?;
        }
        Ok(Some(builder.finish()))
    }

    /// First characters of the non-empty literals of a byte trie; `None` for
    /// the scalar (Unicode case-folding) trie.
    fn non_empty_first_chars(&self, case_insensitive: bool) -> Option<FirstChars> {
        if !self.unicode_nodes.is_empty() {
            return None;
        }
        let root = self.nodes[0];
        let start = root.edge_start as usize;
        let mut first = FirstChars::NONE;
        for &byte in &self.edge_bytes[start..start + root.edge_len as usize] {
            if byte.is_ascii() {
                first.union(&FirstChars::of_literal(
                    char::from(byte).encode_utf8(&mut [0; 4]),
                    case_insensitive,
                ));
            } else {
                first.non_ascii = FirstChars::NON_ASCII_ANY;
            }
        }
        Some(first)
    }

    /// Characters a non-empty match must start with; `None` when the empty
    /// literal is a member or the trie is Unicode case-folded.
    fn first_chars(&self, case_insensitive: bool) -> Option<FirstChars> {
        let root = self.nodes.first()?;
        if !self.unicode_nodes.is_empty() || root.terminal_order.is_some() {
            return None;
        }
        let start = root.edge_start as usize;
        let mut first = FirstChars::NONE;
        for &byte in &self.edge_bytes[start..start + root.edge_len as usize] {
            if byte.is_ascii() {
                ascii_mask_set(&mut first.ascii, byte);
                if case_insensitive {
                    ascii_mask_set(&mut first.ascii, byte.to_ascii_uppercase());
                }
            } else {
                first.non_ascii = FirstChars::NON_ASCII_ANY;
            }
        }
        if case_insensitive {
            // U+017F and U+212A fold onto `s` and `k`.
            first.non_ascii = FirstChars::NON_ASCII_ANY;
        }
        Some(first)
    }

    fn edge(&self, node: usize, byte: u8) -> Option<u32> {
        let node = self.nodes[node];
        let start = node.edge_start as usize;
        let bytes = &self.edge_bytes[start..start + node.edge_len as usize];
        let index = if bytes.len() <= 8 {
            bytes.iter().position(|candidate| *candidate == byte)?
        } else {
            bytes.binary_search(&byte).ok()?
        };
        Some(self.edge_targets[start + index])
    }

    fn collect_matches(
        &self,
        line: &str,
        start: usize,
        flags: RegexFlags,
        budget: &mut StepBudget,
        matches: &mut Vec<(u32, usize)>,
    ) -> Result<(), BudgetExceeded> {
        matches.clear();
        if !self.unicode_nodes.is_empty() {
            let mut node = 0usize;
            if let Some(order) = self.unicode_nodes[0].terminal_order {
                matches.push((order, start));
            }
            for (offset, input) in line.get(start..).unwrap_or_default().char_indices() {
                // Keep the same resource accounting as the byte trie: one
                // unit per consumed input symbol, independent of inventory
                // size. The scalar path is necessary for Oniguruma-compatible
                // Unicode case-insensitive literal sets such as BSL keywords.
                budget.step()?;
                let input_key = CaseFoldKey::new(input);
                let child = self.unicode_nodes[node]
                    .edges
                    .iter()
                    .find_map(|(edge, child)| edge.case_eq(&input_key).then_some(*child));
                let Some(child) = child else {
                    break;
                };
                node = child as usize;
                if let Some(order) = self.unicode_nodes[node].terminal_order {
                    matches.push((order, start + offset + input.len_utf8()));
                }
            }
            return Ok(());
        }
        let mut node = 0usize;
        if let Some(order) = self.nodes[0].terminal_order {
            matches.push((order, start));
        }
        let bytes = line.as_bytes();
        let mut position = start;
        while let Some(&input) = bytes.get(position) {
            // Charge input traversal as useful VM work. This keeps resource
            // limits comparable rather than making a large trie lookup free.
            budget.step()?;
            let (input, width) = if flags.case_insensitive && !input.is_ascii() {
                match bytes.get(position..) {
                    Some([0xc5, 0xbf, ..]) => (b's', 2),       // U+017F LONG S
                    Some([0xe2, 0x84, 0xaa, ..]) => (b'k', 3), // U+212A KELVIN SIGN
                    _ => break,
                }
            } else {
                (
                    if flags.case_insensitive {
                        input.to_ascii_lowercase()
                    } else {
                        input
                    },
                    1,
                )
            };
            let Some(child) = self.edge(node, input) else {
                break;
            };
            node = child as usize;
            position += width;
            if let Some(order) = self.nodes[node].terminal_order {
                matches.push((order, position));
            }
        }
        Ok(())
    }
}

/// The ASCII characters a single-character repeat body can consume, when it
/// provably consumes nothing else: a case-sensitive single ASCII literal or
/// a case-sensitive class built only from ASCII-only atoms. Case-insensitive
/// bodies are excluded because ASCII letters fold to non-ASCII scalars such
/// as the Kelvin sign and dotted capital I.
fn ascii_consumer_members(ast: &Ast, flags: RegexFlags) -> Option<AsciiMask> {
    match ast {
        Ast::Flags {
            flags: local,
            child,
        } => ascii_consumer_members(child, *local),
        Ast::Group {
            index: None, child, ..
        } => ascii_consumer_members(child, flags),
        _ if flags.case_insensitive => None,
        Ast::Literal(value) => {
            let mut chars = value.chars();
            match (chars.next(), chars.next()) {
                (Some(ch), None) if ch.is_ascii() => {
                    let mut mask = [0u64; 2];
                    ascii_mask_set(&mut mask, ch as u8);
                    Some(mask)
                }
                _ => None,
            }
        }
        Ast::Class(class) if class_is_ascii_only(class) => Some(ascii_class_masks(class).0),
        _ => None,
    }
}

/// True when no non-ASCII scalar can match the class case-sensitively.
fn class_is_ascii_only(class: &CharClass) -> bool {
    !class.negated
        && class.atoms.iter().all(|atom| match atom {
            ClassAtom::Char(ch) => ch.is_ascii(),
            ClassAtom::Range(start, end) => start.is_ascii() && end.is_ascii(),
            ClassAtom::Perl(kind) => {
                matches!(kind, PerlClassKind::Digit | PerlClassKind::HorizontalSpace)
            }
            ClassAtom::Nested(class) => class_is_ascii_only(class),
            ClassAtom::Posix { .. } | ClassAtom::Unicode { .. } => false,
        })
}

fn ascii_mask_contains(mask: &AsciiMask, byte: u8) -> bool {
    byte < 128 && mask[byte as usize / 64] & (1u64 << (byte % 64)) != 0
}

/// True when no live capture group sits inside an alternation or repeat.
fn live_captures_keep_selection_shape(ast: &Ast, live: &[u32]) -> bool {
    fn visit(ast: &Ast, live: &[u32], under_choice: bool) -> bool {
        match ast {
            Ast::Group { index, child, .. } => {
                !(under_choice && index.is_some_and(|index| index != 0 && live.contains(&index)))
                    && visit(child, live, under_choice)
            }
            Ast::Concat(nodes) => nodes.iter().all(|node| visit(node, live, under_choice)),
            Ast::Alternation(nodes) => nodes.iter().all(|node| visit(node, live, true)),
            Ast::Repeat { node, .. } => visit(node, live, true),
            Ast::Look { child, .. } | Ast::Flags { child, .. } => visit(child, live, under_choice),
            Ast::Conditional {
                matched, unmatched, ..
            } => visit(matched, live, true) && visit(unmatched, live, true),
            Ast::Empty
            | Ast::Literal(_)
            | Ast::Dot
            | Ast::Grapheme
            | Ast::Class(_)
            | Ast::Anchor(_)
            | Ast::Backref(_)
            | Ast::Subroutine(_)
            | Ast::Unsupported(_) => true,
        }
    }
    visit(ast, live, false)
}

fn branches_contain_live_capture(branches: &[Ast], capture_layout: &[u32]) -> bool {
    !capture_layout.is_empty()
        && branches
            .iter()
            .any(|branch| ast_contains_live_capture(branch, capture_layout))
}

fn ast_contains_live_capture(ast: &Ast, capture_layout: &[u32]) -> bool {
    match ast {
        Ast::Group { index, child, .. } => {
            index.is_some_and(|index| index != 0 && capture_layout.binary_search(&index).is_ok())
                || ast_contains_live_capture(child, capture_layout)
        }
        Ast::Concat(nodes) | Ast::Alternation(nodes) => nodes
            .iter()
            .any(|node| ast_contains_live_capture(node, capture_layout)),
        Ast::Repeat { node, .. }
        | Ast::Look { child: node, .. }
        | Ast::Flags { child: node, .. } => ast_contains_live_capture(node, capture_layout),
        Ast::Conditional {
            matched, unmatched, ..
        } => {
            ast_contains_live_capture(matched, capture_layout)
                || ast_contains_live_capture(unmatched, capture_layout)
        }
        Ast::Empty
        | Ast::Literal(_)
        | Ast::Dot
        | Ast::Grapheme
        | Ast::Class(_)
        | Ast::Anchor(_)
        | Ast::Backref(_)
        | Ast::Subroutine(_)
        | Ast::Unsupported(_) => false,
    }
}

/// Most strings one alternation run may expand into before it keeps its
/// structured form. HTML's named-entity inventory (about 2,200 names) is the
/// largest bundled nested literal tree.
const FINITE_LANGUAGE_LIMIT: usize = 4096;
/// Bounds the expanded text so a long literal repeated across many
/// alternatives cannot inflate compilation.
const FINITE_LANGUAGE_BYTE_LIMIT: usize = 64 * 1024;
/// Largest bounded repeat expanded into literals (`(eq)?`, `x{2}`).
const FINITE_REPEAT_LIMIT: usize = 4;
/// Largest character class expanded into single-character literals.
const FINITE_CLASS_LIMIT: u32 = 16;
/// Most expanded strings per literal or class leaf a trie may replace.
/// Nested inventories expand to about one string per leaf.
const FINITE_EXPANSION_RATIO: usize = 2;

/// Upper bounds for an expandable finite language: how many strings it
/// denotes and their total length, whether it matches empty, and whether any
/// literal is non-ASCII.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FiniteSize {
    strings: usize,
    bytes: usize,
    /// Literal and class leaves: roughly the instructions the structured
    /// form would compile to.
    atoms: usize,
    nullable: bool,
    non_ascii: bool,
}

impl FiniteSize {
    const NONE: Self = Self {
        strings: 0,
        bytes: 0,
        atoms: 0,
        nullable: false,
        non_ascii: false,
    };
    const EMPTY: Self = Self {
        strings: 1,
        bytes: 0,
        atoms: 0,
        nullable: true,
        non_ascii: false,
    };

    /// Whether a trie is worth building. Products of small classes (such
    /// as `[Ii][Nn][Ff]`) multiply into far more strings than the
    /// instructions they replace, which costs more to build on first use
    /// than a short Split chain costs to run.
    fn worth_a_trie(self) -> bool {
        self.strings >= 4 && self.strings <= self.atoms.saturating_mul(FINITE_EXPANSION_RATIO)
    }

    fn remaining_limits(self) -> FiniteLimits {
        FiniteLimits {
            strings: FINITE_LANGUAGE_LIMIT - self.strings,
            bytes: FINITE_LANGUAGE_BYTE_LIMIT - self.bytes,
        }
    }

    /// Either language (alternation).
    fn union(self, other: Self) -> Self {
        Self {
            strings: self.strings.saturating_add(other.strings),
            bytes: self.bytes.saturating_add(other.bytes),
            atoms: self.atoms.saturating_add(other.atoms),
            nullable: self.nullable || other.nullable,
            non_ascii: self.non_ascii || other.non_ascii,
        }
    }

    /// Every string of `self` followed by every string of `other`.
    fn product(self, other: Self) -> Self {
        Self {
            strings: self.strings.saturating_mul(other.strings),
            bytes: self
                .bytes
                .saturating_mul(other.strings)
                .saturating_add(other.bytes.saturating_mul(self.strings)),
            atoms: self.atoms.saturating_add(other.atoms),
            nullable: self.nullable && other.nullable,
            non_ascii: self.non_ascii || other.non_ascii,
        }
    }

    fn within(self, limits: FiniteLimits) -> Option<Self> {
        (self.strings <= limits.strings && self.bytes <= limits.bytes).then_some(self)
    }
}

#[derive(Debug, Clone, Copy)]
struct FiniteLimits {
    strings: usize,
    bytes: usize,
}

/// Sizes `ast` when it denotes a finite set of strings within `limits` built
/// only from literals, small case-sensitive ASCII classes, groups,
/// alternations, and small bounded repeats. Callers must already know that no
/// live capture is inside. The walk allocates nothing, so a failed attempt is
/// cheap to retry at each nested alternation of a structured subtree.
///
/// Trying the expanded strings in order at the same position, and resuming
/// the continuation after each, is exactly what the ordered VM does for such a
/// subtree: it has no anchors, assertions, or state other than the consumed
/// text. Duplicate strings are harmless because the trie keeps the earliest
/// order, and a later duplicate would only retry the same continuation.
fn finite_language_size(ast: &Ast, flags: RegexFlags, limits: FiniteLimits) -> Option<FiniteSize> {
    let size = match ast {
        Ast::Empty => FiniteSize::EMPTY,
        Ast::Literal(literal) => FiniteSize {
            strings: 1,
            bytes: literal.len(),
            atoms: 1,
            nullable: literal.is_empty(),
            non_ascii: !literal.is_ascii(),
        },
        // Counting atoms bounds the members without building class masks;
        // overlapping atoms only make the bound conservative.
        Ast::Class(class) => {
            if flags.case_insensitive || class.negated || !class.intersections.is_empty() {
                return None;
            }
            let mut members = 0usize;
            for atom in &class.atoms {
                members += match atom {
                    ClassAtom::Char(ch) if ch.is_ascii() => 1,
                    ClassAtom::Range(start, end) if start.is_ascii() && end.is_ascii() => {
                        (*end as usize + 1).saturating_sub(*start as usize)
                    }
                    _ => return None,
                };
            }
            if members > FINITE_CLASS_LIMIT as usize {
                return None;
            }
            FiniteSize {
                strings: members,
                bytes: members,
                atoms: 1,
                nullable: false,
                non_ascii: false,
            }
        }
        Ast::Group { child, .. } => finite_language_size(child, flags, limits)?,
        Ast::Flags {
            flags: local,
            child,
        } if *local == flags => finite_language_size(child, flags, limits)?,
        Ast::Alternation(branches) => {
            let mut total = FiniteSize::NONE;
            for branch in branches {
                let remaining = FiniteLimits {
                    strings: limits.strings - total.strings,
                    bytes: limits.bytes - total.bytes,
                };
                total = total.union(finite_language_size(branch, flags, remaining)?);
            }
            total
        }
        Ast::Concat(nodes) => {
            let mut total = FiniteSize::EMPTY;
            for node in nodes {
                total = total
                    .product(finite_language_size(node, flags, limits)?)
                    .within(limits)?;
            }
            total
        }
        Ast::Repeat {
            node,
            min,
            max: Some(max),
            possessive: false,
            atomic: false,
            ..
        } if *max <= FINITE_REPEAT_LIMIT && min <= max => {
            let body = finite_language_size(node, flags, limits)?;
            // An iteration that can match empty is subject to the VM's
            // empty-loop check, which plain enumeration does not model.
            if body.nullable {
                return None;
            }
            let mut exactly = FiniteSize::EMPTY;
            let mut total = FiniteSize::NONE;
            for iterations in 0..=*max {
                if iterations >= *min {
                    total = total.union(exactly);
                }
                exactly = exactly.product(body);
            }
            // The structured loop compiles its body once.
            FiniteSize {
                atoms: body.atoms,
                ..total
            }
        }
        _ => return None,
    };
    size.within(limits)
}

/// Receives expanded strings in priority order. `State` identifies the
/// prefix consumed so far.
trait FiniteSink {
    type State: Copy;

    fn root(&self) -> Self::State;
    fn extend(&mut self, state: Self::State, text: &str) -> Result<Self::State, CompileError>;
    fn emit(&mut self, state: Self::State) -> Result<(), CompileError>;
}

impl FiniteSink for ByteTrieBuilder {
    type State = u32;

    fn root(&self) -> u32 {
        0
    }

    fn extend(&mut self, mut node: u32, text: &str) -> Result<u32, CompileError> {
        for &byte in text.as_bytes() {
            node = self.child(node, byte)?;
        }
        Ok(node)
    }

    fn emit(&mut self, node: u32) -> Result<(), CompileError> {
        self.mark_terminal(node, self.next_order);
        self.next_order = self
            .next_order
            .checked_add(1)
            .ok_or(CompileError::TableOverflow)?;
        Ok(())
    }
}

/// Expanded strings in one buffer, for the scalar (Unicode case-folding)
/// trie. The state is the length of the current prefix in `prefix`.
#[derive(Default)]
struct FiniteStrings {
    prefix: String,
    text: String,
    ends: Vec<usize>,
}

impl FiniteStrings {
    fn strings(&self) -> Vec<&str> {
        let mut start = 0;
        self.ends
            .iter()
            .map(|&end| {
                let string = &self.text[start..end];
                start = end;
                string
            })
            .collect()
    }
}

impl FiniteSink for FiniteStrings {
    type State = usize;

    fn root(&self) -> usize {
        0
    }

    fn extend(&mut self, len: usize, text: &str) -> Result<usize, CompileError> {
        // Depth-first order never revisits a longer prefix after a shorter
        // one was extended, so the bytes up to `len` are still current.
        self.prefix.truncate(len);
        self.prefix.push_str(text);
        Ok(self.prefix.len())
    }

    fn emit(&mut self, len: usize) -> Result<(), CompileError> {
        self.text.push_str(&self.prefix[..len]);
        self.ends.push(self.text.len());
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum Pending<'a> {
    Node(&'a Ast),
    /// A bounded repeat that has completed `done` iterations.
    Repeat {
        node: &'a Ast,
        done: usize,
        min: usize,
        max: usize,
        greedy: bool,
    },
}

/// Emits every string of a subtree admitted by [`finite_language_size`]
/// into `sink`, in the order the backtracking VM would try them.
fn enumerate_finite_language<'a, S: FiniteSink>(
    ast: &'a Ast,
    flags: RegexFlags,
    pending: &mut Vec<Pending<'a>>,
    sink: &mut S,
) -> Result<(), CompileError> {
    pending.clear();
    pending.push(Pending::Node(ast));
    let root = sink.root();
    enumerate_pending(pending, flags, root, sink)
}

/// Depth-first enumeration. `pending` is the remaining sequence, last item
/// first; it is restored before returning.
fn enumerate_pending<'a, S: FiniteSink>(
    pending: &mut Vec<Pending<'a>>,
    flags: RegexFlags,
    state: S::State,
    sink: &mut S,
) -> Result<(), CompileError> {
    let Some(item) = pending.pop() else {
        return sink.emit(state);
    };
    let result = match item {
        Pending::Node(ast) => match ast {
            Ast::Empty => enumerate_pending(pending, flags, state, sink),
            Ast::Literal(literal) => {
                let state = sink.extend(state, literal)?;
                enumerate_pending(pending, flags, state, sink)
            }
            Ast::Class(class) => {
                let members =
                    small_ascii_class_members(class, flags).ok_or(CompileError::Unsupported)?;
                let mut result = Ok(());
                for byte in (0u8..128).filter(|byte| ascii_mask_contains(&members, *byte)) {
                    let mut buffer = [0; 4];
                    result = sink
                        .extend(state, char::from(byte).encode_utf8(&mut buffer))
                        .and_then(|state| enumerate_pending(pending, flags, state, sink));
                    if result.is_err() {
                        break;
                    }
                }
                result
            }
            Ast::Group { child, .. } | Ast::Flags { child, .. } => {
                pending.push(Pending::Node(child));
                let result = enumerate_pending(pending, flags, state, sink);
                pending.pop();
                result
            }
            Ast::Concat(nodes) => {
                let len = pending.len();
                pending.extend(nodes.iter().rev().map(Pending::Node));
                let result = enumerate_pending(pending, flags, state, sink);
                pending.truncate(len);
                result
            }
            Ast::Alternation(branches) => {
                let mut result = Ok(());
                for branch in branches {
                    pending.push(Pending::Node(branch));
                    result = enumerate_pending(pending, flags, state, sink);
                    pending.pop();
                    if result.is_err() {
                        break;
                    }
                }
                result
            }
            Ast::Repeat {
                node,
                min,
                max: Some(max),
                greedy,
                ..
            } => {
                pending.push(Pending::Repeat {
                    node,
                    done: 0,
                    min: *min,
                    max: *max,
                    greedy: *greedy,
                });
                let result = enumerate_pending(pending, flags, state, sink);
                pending.pop();
                result
            }
            // `finite_language_size` admitted only the shapes above.
            _ => Err(CompileError::Unsupported),
        },
        Pending::Repeat {
            node,
            done,
            min,
            max,
            greedy,
        } => {
            // Greedy loops try another iteration before exiting; lazy loops
            // the reverse.
            let mut result = Ok(());
            for iterate in [greedy, !greedy] {
                if iterate {
                    if done < max {
                        pending.push(Pending::Repeat {
                            node,
                            done: done + 1,
                            min,
                            max,
                            greedy,
                        });
                        pending.push(Pending::Node(node));
                        result = enumerate_pending(pending, flags, state, sink);
                        pending.truncate(pending.len() - 2);
                    }
                } else if done >= min {
                    result = enumerate_pending(pending, flags, state, sink);
                }
                if result.is_err() {
                    break;
                }
            }
            result
        }
    };
    pending.push(item);
    result
}

/// Members of a small case-sensitive class built only from ASCII characters
/// and ranges. Such a class matches exactly one of these bytes, so it is
/// equivalent to an alternation of one-byte literals.
fn small_ascii_class_members(class: &CharClass, flags: RegexFlags) -> Option<AsciiMask> {
    if flags.case_insensitive
        || class.negated
        || !class.intersections.is_empty()
        || !class.atoms.iter().all(|atom| match atom {
            ClassAtom::Char(ch) => ch.is_ascii(),
            ClassAtom::Range(start, end) => start.is_ascii() && end.is_ascii(),
            _ => false,
        })
    {
        return None;
    }
    let (members, _) = ascii_class_masks(class);
    (members[0].count_ones() + members[1].count_ones() <= FINITE_CLASS_LIMIT).then_some(members)
}

#[cfg(test)]
fn instruction_capacity_hint(ast: &Ast) -> usize {
    match ast {
        Ast::Empty => 0,
        Ast::Literal(_) | Ast::Dot | Ast::Class(_) | Ast::Anchor(_) => 1,
        Ast::Concat(nodes) => nodes.iter().map(instruction_capacity_hint).sum(),
        Ast::Alternation(branches) => {
            branches
                .iter()
                .map(instruction_capacity_hint)
                .sum::<usize>()
                + branches.len().saturating_sub(1) * 2
        }
        Ast::Repeat { node, .. } => instruction_capacity_hint(node).saturating_add(3),
        Ast::Group { child, .. } | Ast::Flags { child, .. } => instruction_capacity_hint(child),
        Ast::Look { child, .. } => instruction_capacity_hint(child).saturating_add(2),
        Ast::Conditional {
            matched, unmatched, ..
        } => instruction_capacity_hint(matched)
            .saturating_add(instruction_capacity_hint(unmatched))
            .saturating_add(1),
        Ast::Grapheme | Ast::Backref(_) | Ast::Subroutine(_) | Ast::Unsupported(_) => 1,
    }
}

fn lookbehind_direction(ast: &Ast, flags: RegexFlags) -> Result<AssertDirection, CompileError> {
    let (min_width, max_width) = byte_width(ast, flags);
    AssertDirection::behind(min_width, max_width)
}

fn byte_width(ast: &Ast, flags: RegexFlags) -> (usize, Option<usize>) {
    match ast {
        Ast::Empty | Ast::Anchor(_) | Ast::Look { .. } => (0, Some(0)),
        Ast::Literal(value) => {
            let (min, max) = literal_byte_width(value, flags);
            (min, Some(max))
        }
        Ast::Dot | Ast::Class(_) => (1, Some(4)),
        Ast::Concat(nodes) => nodes.iter().fold((0usize, Some(0usize)), |acc, node| {
            let width = byte_width(node, flags);
            (
                acc.0.saturating_add(width.0),
                acc.1
                    .zip(width.1)
                    .map(|(left, right)| left.saturating_add(right)),
            )
        }),
        Ast::Alternation(branches) => {
            if branches.is_empty() {
                return (0, Some(0));
            }
            let mut min = usize::MAX;
            let mut max = Some(0usize);
            for branch in branches {
                let width = byte_width(branch, flags);
                min = min.min(width.0);
                max = max.zip(width.1).map(|(left, right)| left.max(right));
            }
            (min, max)
        }
        Ast::Conditional {
            matched, unmatched, ..
        } => {
            let matched = byte_width(matched, flags);
            let unmatched = byte_width(unmatched, flags);
            (
                matched.0.min(unmatched.0),
                matched
                    .1
                    .zip(unmatched.1)
                    .map(|(matched, unmatched)| matched.max(unmatched)),
            )
        }
        Ast::Repeat { node, min, max, .. } => {
            let width = byte_width(node, flags);
            (
                width.0.saturating_mul(*min),
                max.and_then(|count| width.1.map(|width| width.saturating_mul(count))),
            )
        }
        Ast::Group { child, .. } => byte_width(child, flags),
        Ast::Flags { flags, child } => byte_width(child, *flags),
        Ast::Grapheme => (1, None),
        Ast::Backref(_) | Ast::Subroutine(_) | Ast::Unsupported(_) => (0, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::regex::ast::parse;
    use crate::engine::regex::backtrack::{FallbackMatcher, recursive_position_span};

    #[test]
    fn non_ascii_class_atoms_derive_exact_ascii_masks() {
        // Scalars and ranges whose Unicode case maps reach ASCII (Kelvin,
        // long s, dotted/dotless i, Å/ſ spans) and ones that do not.
        for pattern in [
            "[\u{212a}]",
            "[\u{17f}]",
            "[\u{130}\u{131}]",
            "[é⍺∇]",
            "[\u{c0}-\u{17f}]",
            "[\u{17f}-\u{212a}]",
            "[\u{100}-\u{130}]",
            "[\u{131}-\u{2000}]",
            "[a-\u{212a}]",
            "[\u{80}-\u{10ffff}]",
            "[^\u{212a}x]",
            r"[+\--9<-\[^_a-{}~]",
            "[@-C]",
            "[Z-a]",
            "[\u{7f}-\u{212a}]",
        ] {
            let parsed = parse(pattern);
            let Ast::Class(class) = &parsed.ast else {
                panic!("{pattern:?} should parse as a class");
            };
            assert_eq!(
                ascii_class_masks(class),
                ascii_masks_by_evaluation(class),
                "{pattern:?}"
            );
        }
    }

    fn context() -> AnchorContext {
        AnchorContext {
            allow_a: true,
            allow_g: true,
            g_pos: 0,
        }
    }

    #[test]
    fn perl_ascii_masks_match_the_evaluator() {
        use PerlClassKind::*;
        for kind in [
            Digit,
            NotDigit,
            Space,
            NotSpace,
            Word,
            NotWord,
            HorizontalSpace,
            NotHorizontalSpace,
            VerticalSpace,
            NotVerticalSpace,
            NotNewline,
        ] {
            assert_eq!(
                perl_ascii_mask(kind),
                ascii_predicate_mask(|ch| crate::engine::regex::backtrack::perl_class_contains(
                    kind, ch
                )),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn posix_ascii_masks_match_the_predicates() {
        for (name, mask) in POSIX_ASCII_MASKS {
            for spelling in [name.to_owned(), name.to_ascii_uppercase()] {
                let contains = super::super::backtrack::posix_class_predicate(&spelling);
                assert_eq!(
                    posix_ascii_mask(&spelling),
                    ascii_predicate_mask(contains),
                    "{spelling}"
                );
            }
            assert_eq!(posix_ascii_mask(name), mask);
        }
        let unknown = super::super::backtrack::posix_class_predicate("nope");
        assert_eq!(posix_ascii_mask("nope"), ascii_predicate_mask(unknown));
    }

    #[test]
    fn every_ascii_range_mask_matches_class_evaluation() {
        for low in 0u8..128 {
            for high in 0u8..128 {
                let class = CharClass {
                    bracketed: true,
                    negated: false,
                    intersections: Vec::new(),
                    atoms: vec![ClassAtom::Range(low as char, high as char)],
                };
                assert_eq!(
                    ascii_class_masks(&class),
                    ascii_masks_by_evaluation(&class),
                    "{low}..={high}"
                );
            }
        }
    }

    #[test]
    fn atom_ascii_masks_match_class_evaluation() {
        for pattern in [
            r"[\w\-.]",
            r"[^\s\d]",
            r"[\h\v]",
            r"[\S&&[^\n]]",
            r"[a-zA-Z0-9_$]",
            r"[^A-Z]",
            r"[\x{212A}\x{17F}]",
            r"[\x{212A}-\x{212B}]",
            r"[[:alpha:][:^digit:]]",
            r"[\p{L}\P{N}_]",
            r"[\p{XIDS}\p{XIDC}]",
            r"[[a-f]&&[^c]]",
            r"\W",
            r"\N",
        ] {
            let parsed = parse(pattern);
            let Ast::Class(class) = &parsed.ast else {
                panic!("{pattern} is not a class: {:?}", parsed.ast);
            };
            assert_eq!(
                ascii_class_masks(class),
                ascii_masks_by_evaluation(class),
                "{pattern}"
            );
        }
    }

    #[test]
    fn analysis_walk_matches_standalone_fanout_and_capacity_walks() {
        for pattern in [
            "",
            "a",
            "abc|d",
            r"(?:a|b|c)*x+[yz]?",
            r"(?=(<\s*(keyof|infer)\s+)|\{[^{}]*})",
            r"(a)(?(1)b|c)\X",
            r"(?i:foo|bar)(?-i)baz",
            r"(?<n>x)\k<n>\g<n>",
            r"(?<=\.\.\.)(?!\$)\b(async)?\s*(?:(\*)\s*)?",
            r"a{2,5}?|(?>b+)|c*+",
        ] {
            let parsed = parse(pattern);
            let analysis = parsed.analysis();
            assert_eq!(
                analysis.instruction_capacity_hint(),
                instruction_capacity_hint(&parsed.ast),
                "{pattern:?}"
            );
            assert_eq!(
                analysis.bytecode_beneficial(),
                ordered_fanout_score(&parsed.ast) >= beneficial_fanout_threshold(),
                "{pattern:?}"
            );
        }
    }

    fn bytecode_span(pattern: &str, line: &str, start: usize) -> Option<std::ops::Range<usize>> {
        let program = Program::compile(&parse(pattern)).expect("supported bytecode pattern");
        let mut budget = StepBudget::new(100_000);
        let end = program
            .execute(
                line,
                start,
                context(),
                &mut budget,
                &mut BytecodeScratch::default(),
            )
            .unwrap()?;
        Some(start..end)
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn bytecode_and_hot_frame_layouts_stay_compact() {
        // Baseline before compact operands: 56, 56, 120, 16, and 24 bytes.
        assert_eq!(std::mem::size_of::<Instruction>(), 24);
        assert_eq!(std::mem::size_of::<BacktrackFrame>(), 32);
        assert_eq!(std::mem::size_of::<AssertionFrame>(), 64);
        assert_eq!(std::mem::size_of::<CallFrame>(), 24);
        assert_eq!(std::mem::size_of::<RepeatState>(), 16);
        assert_eq!(std::mem::size_of::<ResumeAction>(), 8);
        assert_eq!(std::mem::size_of::<AssertDirection>(), 8);
        assert_eq!(std::mem::size_of::<InstructionFlags>(), 1);
        assert_eq!(std::mem::size_of::<RepeatBounds>(), 8);
    }

    #[test]
    fn packed_instruction_flags_round_trip_every_combination() {
        for bits in 0u8..16 {
            let flags = RegexFlags {
                case_insensitive: bits & 1 != 0,
                multi_line: bits & 2 != 0,
                dot_matches_new_line: bits & 4 != 0,
                ignore_whitespace: bits & 8 != 0,
            };
            assert_eq!(InstructionFlags::from(flags).regex(), flags);
        }
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn oversized_compact_operands_fall_back_instead_of_truncating() {
        assert_eq!(
            Program::compile(&parse(r"a{4294967295}")).unwrap_err(),
            CompileError::TableOverflow
        );
    }

    #[test]
    fn c_family_space_comment_separator_compiles_to_deterministic_instruction() {
        let pattern =
            r"((?:\s*+(/\*)((?:[^*]++|\*+(?!/))*+(\*/))\s*+)+|\s++|(?<=\W)|(?=\W)|^|\n?$|\A|\Z)foo";
        let program = Program::compile(&parse(pattern)).expect("separator bytecode");
        assert!(
            program.instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::CppSpaceCommentSeparator { .. }
            )),
            "expected C/C++ separator instruction in {:#?}",
            program.instructions
        );
        let mut scratch = BytecodeScratch::default();
        let mut budget = StepBudget::new(100_000);
        let end = program
            .execute("/* c */  foo", 0, context(), &mut budget, &mut scratch)
            .unwrap();
        assert_eq!(end, Some("/* c */  foo".len()));
        let mut budget = StepBudget::new(100_000);
        let end = program
            .execute("bar foo", 3, context(), &mut budget, &mut scratch)
            .unwrap();
        assert_eq!(end, Some("bar foo".len()));
    }

    fn assert_capture_replay(pattern: &str, line: &str, start: usize, live: &[u32]) {
        let parsed = parse(pattern);
        let program = Program::compile_captures(&parsed, live).expect("capture-safe pattern");
        let mut budget = StepBudget::new(100_000);
        let actual = program
            .execute_captures(
                line,
                start,
                context(),
                &mut budget,
                &mut BytecodeScratch::default(),
            )
            .unwrap();
        let expected = FallbackMatcher::new(pattern)
            .try_find_at(line, start, context())
            .unwrap()
            .result;

        assert_eq!(
            actual.as_ref().map(|matched| matched.end),
            expected.as_ref().map(|matched| matched.end),
            "end mismatch for {pattern:?} on {line:?}"
        );
        if let (Some(actual), Some(expected)) = (actual, expected) {
            let compact_expected = program
                .capture_layout()
                .iter()
                .map(|index| expected.captures[*index as usize].clone())
                .collect::<Vec<_>>();
            assert_eq!(
                actual.captures, compact_expected,
                "capture mismatch for {pattern:?} on {line:?}"
            );
        }
    }

    #[test]
    fn literal_inventory_expands_and_subroutine_definitions_borrow_the_parsed_ast() {
        let parsed = parse(r"(?<word>alpha|beta|gamma|delta)");
        let mut definitions = std::collections::BTreeMap::new();
        let mut called = Vec::new();
        collect_group_definitions(
            &parsed.ast,
            parsed.flags,
            &parsed.named_captures,
            &mut definitions,
            &mut called,
        );
        assert!(called.is_empty());
        let (definition, _) = definitions[&1];
        let Ast::Group { child, .. } = definition else {
            panic!("capturing group definition");
        };
        let Ast::Alternation(branches) = child.as_ref() else {
            panic!("literal alternation");
        };
        assert_eq!(
            expanded_strings(branches, parsed.flags),
            ["alpha", "beta", "gamma", "delta"]
        );
    }

    fn expanded_strings(branches: &[Ast], flags: RegexFlags) -> Vec<String> {
        let mut strings = FiniteStrings::default();
        let mut pending = Vec::new();
        for branch in branches {
            let limits = FiniteSize::NONE.remaining_limits();
            assert!(finite_language_size(branch, flags, limits).is_some());
            enumerate_finite_language(branch, flags, &mut pending, &mut strings).unwrap();
        }
        strings.strings().into_iter().map(str::to_owned).collect()
    }

    #[test]
    fn finite_expansion_follows_backtracking_priority() {
        for (pattern, expected) in [
            (
                r"a(s(ymp(eq)?|cr)|nd)|b",
                &["asympeq", "asymp", "ascr", "and", "b"][..],
            ),
            (r"x(y)??z|w", &["xz", "xyz", "w"]),
            (r"[db]|[c-d]x", &["b", "d", "cx", "dx"]),
            (r"(?:a|b){2}|c", &["aa", "ab", "ba", "bb", "c"]),
            (
                r"(?:a|b){0,2}?c|d",
                &["c", "ac", "aac", "abc", "bc", "bac", "bbc", "d"],
            ),
        ] {
            let parsed = parse(pattern);
            let Ast::Alternation(branches) = &parsed.ast else {
                panic!("{pattern} is not an alternation");
            };
            assert_eq!(
                expanded_strings(branches, parsed.flags),
                expected,
                "{pattern}"
            );
        }
    }

    #[test]
    fn literal_trie_bounds_reservation_for_duplicate_branches() {
        let literals = vec![Cow::Borrowed("a"); LITERAL_TRIE_NODE_RESERVE_LIMIT * 2];
        let trie = LiteralTrie::new(&literals, RegexFlags::default())
            .unwrap()
            .unwrap();

        assert_eq!(trie.nodes.len(), 2);
        assert_eq!(trie.nodes.capacity(), LITERAL_TRIE_NODE_RESERVE_LIMIT);
    }

    #[test]
    fn literal_trie_preserves_order_prefixes_flags_and_utf8() {
        for (pattern, line, expected) in [
            (r"(?:foo|foobar|fool|bar)", "foobar", 3),
            (r"(?:foobar|foo|fool|bar)", "foobar", 6),
            (r"(?:foo|foobar|fool|bar)z", "foobarz", 7),
            (r"(?i:alpha|BETA|gamma|delta)", "BeTa!", 4),
            (r"(?i:ask|foo|bar|baz)", "aſK", "aſK".len()),
            (r"(?:λx|λ|rust|type)", "λx", "λx".len()),
        ] {
            assert_eq!(
                bytecode_span(pattern, line, 0),
                Some(0..expected),
                "{pattern:?} on {line:?}"
            );
            assert_eq!(
                bytecode_span(pattern, line, 0),
                Some(recursive_position_span(&parse(pattern), line, 0, context()).unwrap()),
            );
        }
        assert_eq!(bytecode_span(r"(?:foo|bar|baz|quux)", "nope", 0), None);
    }

    #[test]
    fn literal_trie_wide_fanouts_match_the_recursive_engine() {
        // Unsorted branches with wide root and interior fanouts (binary
        // searched edge runs), duplicate and prefix branches, and a suffix
        // that forces backtracking into shorter preferred alternatives.
        let mut branches = Vec::new();
        for first in ['q', 'b', 'z', 'a', 'm'] {
            for second in "zyxwvutsrqponmlkjihgfedcba".chars() {
                branches.push(format!("{first}{second}"));
                branches.push(format!("{first}{second}{second}"));
            }
        }
        branches.push("ab".to_owned());
        branches.push("q".to_owned());
        for flags in ["", "(?i)"] {
            let pattern = format!("{flags}(?:{})(?:b|!)", branches.join("|"));
            for line in ["abb!", "qq", "QZZb", "mab", "zzz!", "q!", "ab!", "nope"] {
                for start in 0..line.len() {
                    assert_eq!(
                        bytecode_span(&pattern, line, start),
                        recursive_position_span(&parse(&pattern), line, start, context()),
                        "{pattern:?} on {line:?} at {start}"
                    );
                }
            }
        }
    }

    #[test]
    fn literal_trie_compacts_runs_around_structured_alternatives() {
        let mut branches = (0..250)
            .map(|index| format!("kw{index:03}"))
            .collect::<Vec<_>>();
        branches.push(r"special\w*".to_owned());
        branches.extend((250..500).map(|index| format!("kw{index:03}")));
        let pattern = format!("(?:{})", branches.join("|"));
        let program = Program::compile(&parse(&pattern)).expect("mixed literal inventory");
        assert_eq!(
            program
                .instructions
                .iter()
                .filter(|instruction| matches!(instruction, Instruction::LiteralTrie { .. }))
                .count(),
            2,
            "literal runs on both sides of the structured branch should be reusable tries"
        );

        let mut budget = StepBudget::new(64);
        let result = program
            .execute(
                "unknown",
                0,
                context(),
                &mut budget,
                &mut BytecodeScratch::default(),
            )
            .expect("a negative probe should not exhaust the existing VM budget");
        assert_eq!(result, None);
        assert_eq!(bytecode_span(&pattern, "specialized!", 0), Some(0..11));
        assert_eq!(bytecode_span(&pattern, "kw499!", 0), Some(0..5));

        // The structured branch remains ahead of the second literal run.
        let ordered = r"(?:foo|bar|baz|quux|x(?:y)?|xyz|xyzz|xyzzy)";
        assert_eq!(bytecode_span(ordered, "xyz", 0), Some(0..2));
    }

    #[test]
    fn nested_finite_alternations_expand_in_priority_order() {
        // Entity-style prefix trees, optional and counted suffixes, lazy
        // repeats, small classes, and suffixes that force backtracking into
        // later alternatives must all behave like the ordered Split chain.
        let patterns = [
            r"(?:a(s(ymp(eq)?|cr|t)|n(d(slope|[dv]|and)?|g(s(t|ph)|e)?))|b(e(ta)?|[12]))(?:;|!)",
            r"(?:a(s(ymp(eq)?|cr|t)|n(d(slope|[dv]|and)?|g(s(t|ph)|e)?))|b(e(ta)?|[12]))",
            r"(?:x(y)??|xy(z)?|w{2}|v[a-c]{1,2}|(?:p|q)(?:r|s))(?:z|$)",
            r"(?:ab|a(b)?c|a(?:bc)??d|[ab][ab])(?:c|d|e)",
            r"(?i:foo(bar)?|BAZ(qux)?|ab(c|d))(?:!|$)",
        ];
        let lines = [
            "asympeq;",
            "asymp;",
            "asympeq!",
            "ascr;",
            "and;",
            "andv;",
            "andslope!",
            "angst;",
            "ange;",
            "an;",
            "beta;",
            "be!",
            "b2;",
            "b3;",
            "asy",
            "xyz",
            "xz",
            "xyzz",
            "ww",
            "wwz",
            "vabz",
            "vcz",
            "prz",
            "qs",
            "abce",
            "abcd",
            "abd",
            "bac",
            "FOOBAR!",
            "baz",
            "BaZQuX",
            "abD!",
            "",
        ];
        for pattern in patterns {
            let program = Program::compile(&parse(pattern)).expect("finite pattern compiles");
            assert!(
                program
                    .instructions
                    .iter()
                    .any(|instruction| matches!(instruction, Instruction::LiteralTrie { .. })),
                "{pattern:?} should expand into a literal trie"
            );
            for line in lines {
                for start in 0..=line.len() {
                    assert_eq!(
                        bytecode_span(pattern, line, start),
                        recursive_position_span(&parse(pattern), line, start, context()),
                        "{pattern:?} on {line:?} at {start}"
                    );
                }
            }
        }
        // A live capture keeps its structured form; dead ones are elided.
        let entity = r"(&)((a(s(ymp(eq)?|cr)|nd)|b(e(ta)?|[12])))(;)";
        for line in ["&asympeq;", "&asymp;", "&beta;", "&be;", "&b2;", "&nd;"] {
            assert_capture_replay(entity, line, 0, &[1, 2, 9]);
            assert_capture_replay(entity, line, 0, &[1, 2, 7, 9]);
        }
        assert_capture_replay(r"(?:x(y)?|xy(z)?|w|v)(z)", "xyzz", 0, &[1, 2, 3]);
    }

    #[test]
    fn finite_expansion_rejects_unbounded_and_nullable_repeats() {
        for pattern in [
            r"(?:a(b)*|c|d|e)",
            r"(?:a(b?){2}|c|d|e)",
            r"(?:a(?=b)|c|d|e)",
        ] {
            let program = Program::compile(&parse(pattern)).unwrap();
            assert!(
                !program
                    .instructions
                    .iter()
                    .any(|instruction| matches!(instruction, Instruction::LiteralTrie { .. })),
                "{pattern:?} must not expand"
            );
        }
        // Expansion limits fall back to the structured form.
        let wide = format!("(?:{})", ["[a-p][a-p][a-p][a-p]"; 4].join("|"));
        let program = Program::compile(&parse(&wide)).unwrap();
        assert!(
            !program
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::LiteralTrie { .. }))
        );
    }

    #[test]
    fn literal_trie_handles_unicode_case_insensitive_inventories() {
        let pattern = "(?i:Начать|Транзакция|Отменить|Зафиксировать|Begin|Commit|Rollback)";
        let program = Program::compile(&parse(pattern)).expect("Unicode literal trie");
        assert!(
            program
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::LiteralTrie { .. })),
            "expected Unicode alternatives to use the reusable literal trie"
        );
        for (line, end) in [
            ("НАЧАТЬ!", "НАЧАТЬ".len()),
            ("транзакция(", "транзакция".len()),
            ("rOlLbAcK ", "rOlLbAcK".len()),
        ] {
            assert_eq!(bytecode_span(pattern, line, 0), Some(0..end), "{line:?}");
        }
        assert_eq!(bytecode_span(pattern, "Неизвестно", 0), None);
    }

    #[test]
    fn literal_trie_elides_only_dead_branch_captures() {
        let parsed = parse(r"(?:(aa)|(ab)|(ac)|(ad))z");
        let position_only = Program::compile_captures(&parsed, &[]).unwrap();
        assert!(
            position_only
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::LiteralTrie { .. }))
        );

        let capture_replay = Program::compile_captures(&parsed, &[3]).unwrap();
        assert!(
            !capture_replay
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, Instruction::LiteralTrie { .. }))
        );
        assert_capture_replay(r"(?:(aa)|(ab)|(ac)|(ad))z", "acz", 0, &[3]);
    }

    #[test]
    fn rejects_position_capture_dependent_constructs() {
        assert_eq!(
            Program::compile(&parse(r"(a)\1")).unwrap_err(),
            CompileError::Backreference
        );
        assert_eq!(
            Program::compile(&parse(r"(?<x>a)\g<x>")).unwrap_err(),
            CompileError::Subroutine
        );
    }

    #[test]
    fn interns_literal_and_class_operands() {
        let program = Program::compile(&parse(r"(?i:foo)|foo|(?i:[a])|[a]"))
            .expect("supported bytecode pattern");

        assert_eq!(program.literals, ["foo"]);
        assert_eq!(program.classes.len(), 1);
    }

    #[test]
    fn ordered_dfs_matches_recursive_capture_replay_spans() {
        let cases = [
            (r"(a|aa)*a", "aaaa"),
            (r"(ab|a)+?b", "aaab"),
            (r"(?:a?)*b", "aaab"),
            (r"a{1,3}?a", "aaaa"),
            (r"a{1,3}a", "aaaa"),
            (r"(?i:(ab|c))+D", "ABcD"),
            (r"(é|λ)+z", "éλz"),
            (r"(?=(a|aa)+b)a+b", "aaab"),
            (r"(?!foo)([a-z])+[0-9]", "bar7"),
            (r"(?<=(a|aa))b", "aab"),
            (r"(?<!foo)([a-z])+[0-9]", "bar7"),
            (r"(?<=a{1,3})b", "aaab"),
            (r"(?<=a+)b", "aaab"),
            (r"(?=(?<!x)a)a", "a"),
            (r"^\w+\s.$", "abc λ"),
        ];

        for (pattern, line) in cases {
            let recursive = FallbackMatcher::new(pattern)
                .try_find_at(line, 0, context())
                .unwrap()
                .result
                .map(|result| result.start..result.end);
            assert_eq!(
                bytecode_span(pattern, line, 0),
                recursive,
                "pattern {pattern:?}, line {line:?}"
            );
        }
    }

    #[test]
    fn capture_replay_preserves_zero_width_repeat_iterations() {
        assert_capture_replay(r"((?=a))+", "a", 0, &[1]);
        assert_capture_replay(r"((?=a))*", "a", 0, &[1]);
        assert_eq!(bytecode_span(r"(?:){2}a", "a", 0), Some(0..1));
    }

    #[test]
    fn differential_across_utf8_start_positions() {
        let patterns = [
            r"(a|ab){0,3}?b",
            r"(?:a?)*b",
            r"(?=a|ab)a+",
            r"(?<!aa)(?:a|é)*b",
            r"(?<=a*)b",
        ];
        let lines = ["", "b", "aaab", "xabab", "éaab", "aaaa"];

        for pattern in patterns {
            let parsed = parse(pattern);
            for line in lines {
                for start in line
                    .char_indices()
                    .map(|(index, _)| index)
                    .chain(std::iter::once(line.len()))
                {
                    let recursive = recursive_position_span(&parsed, line, start, context());
                    assert_eq!(
                        bytecode_span(pattern, line, start),
                        recursive,
                        "pattern {pattern:?}, line {line:?}, start {start}"
                    );
                }
            }
        }
    }

    #[test]
    fn greedy_repeats_become_possessive_only_when_giving_back_must_fail() {
        for pattern in [
            r"[A-Z_a-z][$0-9A-Z_a-z]*(?![$0-9A-Z_a-z])",
            r"([a-z]+)(::)",
            r"[a-z]*(?=[0-9])",
            r"[ \t]*(?:\b(in|out)\b)?[a-z]",
            r"[a-z]*K",
            r"x*(?:(y)|z)",
            r"(?:[a-z]*(?<=b))1",
            // Proved by the first-character analysis rather than the ASCII
            // member walk; the capture is untracked in position-only code.
            r"[^0-9]*1",
            r"([a-z])*1",
        ] {
            assert_eq!(scan_repeat_count(pattern), 1, "{pattern}");
        }
        for pattern in [
            r"a*a",
            r"[a-z]*(?!x)",
            r"[a-z]*(?=[a-z0-9])",
            r"[a-z]*",
            r"x[a-z]*",
            r"(?i)[a-z]*1",
            r"[a-z]*(?i:K)",
            r"\w*1",
            r"[a-z]*?1",
            r"[a-z]*(?:|one|two|three|four)1",
            r"[a-z]*(?:x{0}|[0-9])",
            r"[a-z]*.",
        ] {
            assert_eq!(scan_repeat_count(pattern), 0, "{pattern}");
        }
    }

    #[test]
    fn auto_possessive_repeats_match_recursive_engine() {
        let patterns = [
            r"([A-Z_a-z][$0-9A-Z_a-z]*)(?![$0-9A-Z_a-z])(\s*)",
            r"(?:([a-z]+)(::))?([a-z]+)\b",
            r"([a-z]*)(?=[0-9])",
            r"[ \t]*(?:\b(in|out)\b[ \t]*)?([a-z]+)",
            r"([a-z]*)(?:alpha|beta|gamma|delta|[0-9])",
            r"([a-z]{2,4})K",
            r"([a-z]*)(?i:K)",
            r"[a-z]*(?!x)",
            r"([0-9]*)(?=[a-z])[a-z]",
            r"(x*)(?:(y)|z)",
        ];
        let lines = [
            "",
            "abc",
            "foo::bar baz",
            "abc123",
            "  in abc",
            "abcK",
            "abcdK",
            "abc\u{212a}",
            "abx",
            "12ab",
            "xxxz xxy",
            "foo_bar$1 x",
            "é1 aé",
        ];
        for pattern in patterns {
            let parsed = parse(pattern);
            let live = (1..=parsed.capture_count).collect::<Vec<_>>();
            for line in lines {
                for start in line
                    .char_indices()
                    .map(|(index, _)| index)
                    .chain(std::iter::once(line.len()))
                {
                    let recursive = recursive_position_span(&parsed, line, start, context());
                    assert_eq!(
                        bytecode_span(pattern, line, start),
                        recursive,
                        "pattern {pattern:?}, line {line:?}, start {start}"
                    );
                    assert_capture_replay(pattern, line, start, &live);
                }
            }
        }
    }

    #[test]
    fn first_char_guards_match_recursive_engine() {
        let patterns = [
            r"(?:foo|[0-9]+|\s*x|(?=a)ab|bar)y?",
            r"(?:(async\s+)?(?:function|[a-z]+\s*=>)|\()",
            r"(?:(?:a|b)c|(?:d|)e|f)",
            r"(?:alpha|beta|gamma|delta|epsilon|[0-9])+z",
            r"(?i:kelvin|sigma|psi|omega|x)",
            r"(?i:(?:k|s)x|q)",
            r"(?:é|ß|a)+",
            r"(?:\bab|(?<=a)b|$|c)",
            r"(?:[a-z]\d){2,3}?b",
            r"(ab|cd)*?c",
            r"(?:(a)|b)*c?",
            r"(?:x(?:y|z)|(?:q))++w",
        ];
        let lines = [
            "",
            "fooy",
            "123y bary",
            "  xy",
            "aby",
            "async  foo=>(",
            "function(",
            "ace de e f",
            "alphabeta12z",
            "\u{212a}elvin \u{17f}igma PSI",
            "\u{212a}x sx Q",
            "éßaé ß",
            "ab b c",
            "a1b2c3b",
            "abcdc",
            "aab",
            "xyxzqw",
        ];
        for pattern in patterns {
            let parsed = parse(pattern);
            let live = (1..=parsed.capture_count).collect::<Vec<_>>();
            for line in lines {
                for start in line
                    .char_indices()
                    .map(|(index, _)| index)
                    .chain(std::iter::once(line.len()))
                {
                    let recursive = recursive_position_span(&parsed, line, start, context());
                    assert_eq!(
                        bytecode_span(pattern, line, start),
                        recursive,
                        "pattern {pattern:?}, line {line:?}, start {start}"
                    );
                    assert_capture_replay(pattern, line, start, &live);
                }
            }
        }
    }

    #[test]
    fn give_back_scans_match_recursive_engine() {
        let patterns = [
            r"(.*)x",
            r"a.*b",
            r"([a-z]*)ab",
            r"(?:ab)*abc",
            r"é*é",
            r"[^,]{2,4}+?,|[^,]{1,3},",
            r"\s*\s\S",
            r"(=)(?!\s*.*=>\s*$)",
            r"(?:(x*)y)*z",
            r"(?i)[k]*k",
            r"(?s:.*)\n",
            r"(?:\w{2,}?)(\w*)\w",
        ];
        let lines = [
            "",
            "axbx",
            "a..b..b",
            "xyzab abab",
            "ababababc",
            "ééé é",
            "aa,bbbbb,c,",
            "   x  ",
            "x = y => z",
            "x = y =>",
            "xyxxyz",
            "kK\u{212a}k",
            "a\nb\n",
            "hello world",
        ];
        for pattern in patterns {
            let parsed = parse(pattern);
            let live = (1..=parsed.capture_count).collect::<Vec<_>>();
            for line in lines {
                for start in line
                    .char_indices()
                    .map(|(index, _)| index)
                    .chain(std::iter::once(line.len()))
                {
                    let recursive = recursive_position_span(&parsed, line, start, context());
                    assert_eq!(
                        bytecode_span(pattern, line, start),
                        recursive,
                        "pattern {pattern:?}, line {line:?}, start {start}"
                    );
                    assert_capture_replay(pattern, line, start, &live);
                }
            }
        }
    }

    #[test]
    fn single_consumer_assertions_match_recursive_engine() {
        let patterns = [
            r"(?<![$_[:alnum:]])[a-z]+(?![$_[:alnum:]])",
            r"(?:(?<=\.\.\.)|(?<!\.))\b[a-z]+",
            r"(?<=é)x|(?<![é])y",
            r"(?<=[^a])b(?=[é\s])",
            r"(?<=ab)c(?!d)",
            r"(?i:(?<=AB)c(?=D))",
            r"(?<=)a(?=)",
            r"(?<!\w)(?=\d)\d+(?<=\d)(?!\.)",
            r"(?<=[a-z]{2})x",
        ];
        let lines = [
            "",
            "abc",
            "$abc def1 ...xyz .q",
            "éx ay éy ëy",
            "ab b\u{e9} ab\u{e9} cb ",
            "abcd abce ABcD abcx",
            "a1 12.5 x99",
            "xx abx",
        ];
        for pattern in patterns {
            let parsed = parse(pattern);
            let live = (1..=parsed.capture_count).collect::<Vec<_>>();
            for line in lines {
                for start in line
                    .char_indices()
                    .map(|(index, _)| index)
                    .chain(std::iter::once(line.len()))
                {
                    let recursive = recursive_position_span(&parsed, line, start, context());
                    assert_eq!(
                        bytecode_span(pattern, line, start),
                        recursive,
                        "pattern {pattern:?}, line {line:?}, start {start}"
                    );
                    assert_capture_replay(pattern, line, start, &live);
                }
            }
        }
    }

    #[test]
    fn first_char_guards_persist_across_executions() {
        let parsed = parse(r"(?:ab|cd|[0-9])+x");
        let program = Program::compile(&parsed).expect("compile");
        let mut scratch = BytecodeScratch::default();
        for (line, expected) in [
            ("zz", None),
            ("abx", Some(3)),
            ("", None),
            ("cd9abx", Some(6)),
            ("é", None),
        ] {
            let mut budget = StepBudget::new(10_000);
            assert_eq!(
                program
                    .execute(line, 0, context(), &mut budget, &mut scratch)
                    .expect("budget"),
                expected,
                "line {line:?}"
            );
        }
        assert!(
            program
                .guards
                .iter()
                .any(|guard| guard.state() == GuardCell::ASCII),
            "an executed alternation should derive an ASCII guard"
        );
    }

    #[test]
    fn non_ascii_atom_ascii_masks_match_class_evaluation() {
        for pattern in [
            "[\u{212a}]",
            "[\u{17f}]",
            "[\u{131}]",
            "[\u{130}]",
            "[\u{e9}\u{c9}]",
            "[\u{212a}-\u{212b}]",
            "[\\x{20}-\\x{10ffff}]",
            "[a-\u{e9}]",
            "[\u{c0}-\u{17f}]",
            "[^\u{212a}]",
        ] {
            let Ast::Class(class) = parse(pattern).ast else {
                panic!("{pattern:?} should parse as a class");
            };
            assert_eq!(
                ascii_class_masks(&class),
                ascii_masks_by_evaluation(&class),
                "{pattern:?}"
            );
        }
    }

    fn scan_repeat_count(pattern: &str) -> usize {
        let parsed = parse(pattern);
        Program::compile(&parsed)
            .or_else(|_| Program::compile_captures(&parsed, &[]))
            .expect("supported bytecode pattern")
            .instructions
            .iter()
            .filter(|instruction| {
                matches!(
                    instruction,
                    Instruction::ScanRepeat {
                        give_back: false,
                        ..
                    }
                )
            })
            .count()
    }

    #[test]
    fn greedy_repeats_before_literal_tries_use_the_trie_first_bytes() {
        // `\w` is not ASCII-only, so only the first-character analysis can
        // prove these continuations exclusive; it must look into the trie.
        let exclusive = r"\w+(?:;a|;b|;c|:d)";
        let overlapping = r"\w+(?:;a|;b|;c|dd)";
        let nullable = r"\w+(?:;a|;b|;c|)d";
        assert_eq!(scan_repeat_count(exclusive), 1);
        assert_eq!(scan_repeat_count(overlapping), 0);
        assert_eq!(scan_repeat_count(nullable), 0);
        for pattern in [exclusive, overlapping, nullable] {
            for line in ["ab;a", "abdd", "abd", "ab:d", "λx;c", "ab"] {
                for start in 0..line.len() {
                    if !line.is_char_boundary(start) {
                        continue;
                    }
                    assert_eq!(
                        bytecode_span(pattern, line, start),
                        recursive_position_span(&parse(pattern), line, start, context()),
                        "{pattern:?} on {line:?} at {start}"
                    );
                }
            }
        }
    }

    #[test]
    fn greedy_repeats_become_possessive_only_before_exclusive_continuations() {
        // Nothing the repeat consumes can start what follows.
        for pattern in [
            r"[a-z]*\d",
            r"\w+\s*=",
            r"([$_[:alpha:]][$_[:alnum:]]*)?\s*(?=<)",
            r"(#?[$_[:alpha:]][$_[:alnum:]]*)(?:(\?)|(!))?(?=\s*=)",
            r"\d*(?=a)[a-z]",
            r"(?i)\d*x",
            r"(?:[a-z]+,)*;",
        ] {
            assert!(scan_repeat_count(pattern) > 0, "{pattern:?}");
        }
        // Giving characters back can still succeed, or the continuation is
        // not provably exclusive.
        for pattern in [
            r"[a-z]*c",
            r"[a-z]*",
            r"[a-z]*$",
            r"[[:alnum:]]*\S",
            r"\w*[^a]",
            r"(?i)[a-z]*K",
            r"[a-z]*(?=x|1)[a-z0-9]",
            r"(?:[a-z]+)*[a-z]x",
            r"([a-z]*)\1",
            r"[a-z]*?\d",
            r"\s*\x{2003}",
            r"[a-zé]*\x{e9}",
        ] {
            assert_eq!(scan_repeat_count(pattern), 0, "{pattern:?}");
        }
    }

    #[test]
    fn automatic_possessification_preserves_backtracking_results() {
        let patterns = [
            r"[a-z]*\d",
            r"[a-z]*c",
            r"\w+\s*=",
            r"\w*\s",
            r"[[:alnum:]]*\S",
            r"([$_[:alpha:]][$_[:alnum:]]*)?\s*(?=<)",
            r"(#?[$_[:alpha:]][$_[:alnum:]]*)(?:(\?)|(!))?(?=\s*\s*=)",
            r"\d*(?=a)[a-z]",
            r"(?i)\d*x",
            r"(?i)[a-z]*K",
            r"(?:[a-z]+,)*[a-z]+;",
            r"(?:[a-z]+\s?)*[a-z]x",
            r"[a-z]{2,4}\d",
            r"[a-z]{2,4}[a-e]",
            r"\s*(?!\s)\w+",
        ];
        let lines = [
            "",
            "abc1",
            "abc",
            "ab = 1",
            "ab\u{a0}x",
            "foo <T>",
            "foo<",
            "#bar? = 1",
            "bar! =",
            "12a",
            "12X",
            "abK",
            "ab\u{212a}",
            "ab,cd;",
            "ab cdx",
            "ñandú\u{2003}=",
            "abcde",
        ];
        for pattern in patterns {
            let parsed = parse(pattern);
            for line in lines {
                for start in line
                    .char_indices()
                    .map(|(index, _)| index)
                    .chain(std::iter::once(line.len()))
                {
                    assert_eq!(
                        bytecode_span(pattern, line, start),
                        recursive_position_span(&parsed, line, start, context()),
                        "pattern {pattern:?}, line {line:?}, start {start}"
                    );
                }
            }
        }
        assert_capture_replay(r"([a-z]+)(\d)", "abc1", 0, &[1, 2]);
        assert_capture_replay(r"(\w+)\s*(=)", "ab = 1", 0, &[1, 2]);
    }

    #[test]
    fn non_ascii_first_char_kinds_cover_every_scalar() {
        use crate::engine::regex::backtrack::{
            perl_class_contains, posix_class_predicate, unicode_case_eq,
        };
        let spaces = (0x80..=0x10_ffff)
            .filter_map(char::from_u32)
            .filter(|ch| ch.is_whitespace())
            .collect::<Vec<_>>();
        type Predicate = (String, u8, Box<dyn Fn(char) -> bool>);
        let mut predicates: Vec<Predicate> = Vec::new();
        for name in [
            "alnum", "alpha", "ascii", "blank", "cntrl", "digit", "graph", "lower", "print",
            "punct", "space", "upper", "word", "xdigit",
        ] {
            let contains = posix_class_predicate(name);
            predicates.push((
                format!("[:{name}:]"),
                posix_non_ascii_kinds(name),
                Box::new(contains),
            ));
        }
        for kind in [
            PerlClassKind::Digit,
            PerlClassKind::HorizontalSpace,
            PerlClassKind::VerticalSpace,
            PerlClassKind::Space,
            PerlClassKind::NotSpace,
            PerlClassKind::Word,
        ] {
            predicates.push((
                format!("{kind:?}"),
                atom_non_ascii_kinds(&ClassAtom::Perl(kind), false),
                Box::new(move |ch| perl_class_contains(kind, ch)),
            ));
        }
        // Case-insensitive ASCII atoms reach only non-space scalars.
        predicates.push((
            "ASCII case fold".to_owned(),
            FirstChars::NON_ASCII_OTHER,
            Box::new(|ch| ('\0'..='\x7f').any(|ascii| unicode_case_eq(ascii, ch))),
        ));

        for (label, claimed, contains) in &predicates {
            if claimed & FirstChars::NON_ASCII_SPACE == 0 {
                assert!(!spaces.iter().any(|ch| contains(*ch)), "{label} space");
            }
        }
        let others = predicates
            .iter()
            .filter(|(_, claimed, _)| claimed & FirstChars::NON_ASCII_OTHER == 0)
            .collect::<Vec<_>>();
        for ch in (0x80..=0x10_ffff)
            .filter_map(char::from_u32)
            .filter(|ch| !ch.is_whitespace())
        {
            for (label, _, contains) in &others {
                assert!(!contains(ch), "{label} {ch:?}");
            }
        }
    }

    #[test]
    fn stale_repeat_slots_are_never_observed_between_executions() {
        let pattern = r"(?:a?)*b(?:c{1,3})?";
        let parsed = parse(pattern);
        let program = Program::compile(&parsed).unwrap();
        let mut scratch = BytecodeScratch::default();
        for line in ["aaabccc", "b", "aaaa", "bc", "aabcc", ""] {
            let expected = recursive_position_span(&parsed, line, 0, context());
            let mut budget = StepBudget::new(100_000);
            let actual = program
                .execute(line, 0, context(), &mut budget, &mut scratch)
                .unwrap()
                .map(|end| 0..end);
            assert_eq!(actual, expected, "line={line:?}");
        }
    }

    #[test]
    fn capture_replay_matches_recursive_alternation_and_repeats() {
        for (pattern, line) in [
            (r"((ab)|(a))+b", "aabb"),
            (r"(ab|a)+?b", "aaab"),
            (r"(a(b)?)+", "aba"),
            (r"(a{1,3}?)(a)", "aaaa"),
        ] {
            let count = parse(pattern).capture_count;
            let live = (1..=count).collect::<Vec<_>>();
            assert_capture_replay(pattern, line, 0, &live);
        }
    }

    #[test]
    fn capture_undo_clears_abandoned_optional_path() {
        // Group 2 is set on the first branch before that branch fails. Taking
        // the alternate must restore it to unset.
        assert_capture_replay(r"((a)b|a)c", "ac", 0, &[1, 2]);
        assert_capture_replay(r"((a)?b|a)c", "ac", 0, &[1, 2]);
    }

    #[test]
    fn capture_assertions_preserve_only_successful_positive_writes() {
        assert_capture_replay(r"(?=(a|aa))a+", "aa", 0, &[1]);
        assert_capture_replay(r"(?!((a))c)ab", "ab", 0, &[1, 2]);
        assert_capture_replay(r"(?=(a|ab))(?:ac|a)b", "ab", 0, &[1]);
        assert_capture_replay(r"(?<=(a))b", "ab", 1, &[1]);
        assert_capture_replay(r"(?<=(a))\1", "aa", 1, &[1]);
        assert_capture_replay(r"(?<!(a))b", "bb", 1, &[1]);
    }

    #[test]
    fn capture_backreferences_use_internal_live_slots_and_backtrack() {
        assert_capture_replay(r"(a|b)\1", "aa", 0, &[]);
        assert_capture_replay(r"(?<x>a|b)\k<x>", "bb", 0, &[]);
        assert_capture_replay(r"((a)|b)\1", "bb", 0, &[2]);
        assert_capture_replay(r"(a|ab)\1", "abab", 0, &[1]);
    }

    #[test]
    fn capture_subroutines_use_bounded_explicit_call_stack() {
        assert_capture_replay(r"(?<x>a|b)\g<x>", "aa", 0, &[1]);
        assert_capture_replay(r"(?<parens>\((?:[^()]|\g<parens>)*\))", "((a)(b))", 0, &[1]);
    }

    #[test]
    fn recursive_subroutines_keep_their_callers_loop_counts() {
        // The inner call re-enters `{2}` on the same repeat slot; the outer
        // loop must resume with its own count. Oniguruma matches 0..18.
        let pattern = r"(?<n>a(?:b\g<n>?c){2}d)";
        let line = "abababcbcdcbcdcbcd";
        assert_capture_replay(pattern, line, 0, &[1]);
        // Position selection uses the internal capture layout.
        let program = Program::compile_captures(&parse(pattern), &[]).expect("selection program");
        let end = program
            .execute(
                line,
                0,
                context(),
                &mut StepBudget::new(100_000),
                &mut BytecodeScratch::default(),
            )
            .unwrap();
        assert_eq!(end, Some(18));
    }

    #[test]
    fn deep_recursion_restores_each_slot_once_per_return() {
        // Replaying every callee undo entry on return doubled the log per
        // nesting level; 60 levels would never finish.
        let pattern = r"(?<n>\((?:[^()]|\g<n>)*\))";
        let line = format!("{}{}", "(".repeat(60), ")".repeat(60));
        let program = Program::compile_captures(&parse(pattern), &[]).expect("selection program");
        let mut scratch = BytecodeScratch::default();
        let end = program
            .execute(
                &line,
                0,
                context(),
                &mut StepBudget::new(100_000),
                &mut scratch,
            )
            .unwrap();
        assert_eq!(end, Some(line.len()));
        assert!(
            scratch.repeat_undo.len() < 10_000,
            "{}",
            scratch.repeat_undo.len()
        );
        assert_capture_replay(pattern, &line, 0, &[1]);
    }

    #[test]
    fn nested_returns_charge_each_undo_entry_once() {
        // Charging every return for its callees' entries again made the
        // cost grow with depth, so nested template arguments ran out of
        // budget in capture replay.
        let pattern = r"(?<n><(?:[^<>]|\g<n>)*>)";
        let line = format!("{}{}{}", "<".repeat(60), "x".repeat(2000), ">".repeat(60));
        let program = Program::compile_captures(&parse(pattern), &[1]).expect("capture program");
        let mut budget = StepBudget::new(100_000);
        let matched = program
            .execute_captures(
                &line,
                0,
                context(),
                &mut budget,
                &mut BytecodeScratch::default(),
            )
            .expect("within budget")
            .expect("nested match");
        assert_eq!(matched.end, line.len());
        assert!(budget.used() < 20_000, "{}", budget.used());
    }

    #[test]
    fn backtracking_takes_back_nested_return_credit() {
        // The first branch returns from `k` once per `a`, crediting `n`.
        // When it fails, that credit must go with it, or `n`'s later returns
        // rescan their growing undo log for free.
        let pattern = r"\g<n>z|(?<n>(?:\g<k>)*b|(?:ac?)*)|(?<k>a)";
        let parsed = parse(pattern);
        let live = (1..=parsed.capture_count).collect::<Vec<_>>();
        let program = Program::compile_captures(&parsed, &live).expect("capture program");
        let mut budget = StepBudget::new(100_000);
        let result = program.execute_captures(
            &"a".repeat(3_000),
            0,
            context(),
            &mut budget,
            &mut BytecodeScratch::default(),
        );
        assert!(result.is_err(), "used {} steps", budget.used());
    }

    #[test]
    fn capture_subroutines_return_to_their_caller_after_backtracking() {
        // The second call fails, so matching backtracks into the first
        // call's routine after it returned. Its second return must reach the
        // first call site, not the second call that reused the stack depth.
        assert_capture_replay(r"(?<n>a+)b\g<n>(\g<n>)", "abaa", 0, &[2]);
        assert_capture_replay(r"(?<n>a+)b\g<n>(\g<n>)c", "abaaac", 0, &[1, 2]);
        // Calls under choices and repeats rewrite the called group's capture.
        assert_capture_replay(r"(?<n>a|b)(?:\g<n>|c)d", "abd", 0, &[1]);
        assert_capture_replay(r"(?<n>a|b)(?:\g<n>)*c", "abac", 0, &[1]);
        assert_capture_replay(r"(?<n>a|b)(?:x|\g<n>)+", "abxb", 0, &[1]);
        assert_capture_replay(r"(?<n>a(b)?)\g<n>+(x)", "aabaax", 0, &[1, 2, 3]);
    }

    #[test]
    fn subroutine_programs_compile_only_called_routines() {
        // Uncalled groups (including ones nesting the called group) stay
        // inline-only; a call inside a routine body reaches another routine.
        for (pattern, line, live) in [
            (
                r"((a)(?<b>b|c))x\g<b>(d(e))",
                "abxcde",
                &[1, 2, 3, 4, 5][..],
            ),
            (
                r"(?<outer>(?<inner>[a-c])\g<inner>)-\g<outer>",
                "ab-ca",
                &[1, 2],
            ),
            (r"((x)|(?<y>y\g<2>?))z\g<y>", "yxzy", &[1, 2, 3]),
            (r"(q)(?<n>\d+)\g<1>\g<n>", "q12q3", &[]),
            (r"(?i:(?<word>ab)){0}(x)\g<word>", "xAB", &[1, 2]),
            (r"(?<a>a|b\g<a>)(c)\g<a>", "bacba", &[1, 2]),
        ] {
            assert_capture_replay(pattern, line, 0, live);
            let parsed = parse(pattern);
            let all = Program::compile_captures(&parsed, live).expect("capture program");
            let mut definitions = std::collections::BTreeMap::new();
            let mut called = Vec::new();
            collect_group_definitions(
                &parsed.ast,
                parsed.flags,
                &parsed.named_captures,
                &mut definitions,
                &mut called,
            );
            called.sort_unstable();
            called.dedup();
            let routine_count = all
                .instructions
                .iter()
                .filter(|instruction| matches!(instruction, Instruction::Return))
                .count();
            assert_eq!(routine_count, called.len(), "{pattern:?}");
        }
    }

    #[test]
    fn possessive_cut_repro_cpp_scope_pattern() {
        // Distilled from the C++ scope-resolution pattern: an inner capture
        // followed by a possessive spacer inside an optional group.
        assert_capture_replay(
            r"([a-z]+)\s*+((<[^<>]*>)\s*+)?(::)",
            "abc<T> ::",
            0,
            &[1, 2, 3, 4],
        );
        assert_capture_replay(r"((<[^<>]*>)\s*+)?(::)", "<T> ::", 0, &[1, 2, 3]);
        assert_capture_replay(
            r"(?:([a-z]+)((<[^<>]*>)\s*+)?::)*([a-z]+)",
            "ab<T> ::cd",
            0,
            &[1, 2, 3, 4],
        );
        assert_capture_replay(r"a*+b", "aaab", 0, &[]);
        assert_capture_replay(r"(?>a+)b", "aaab", 0, &[]);
        assert_capture_replay(r"(a|ab)*+c", "ababc", 0, &[1]);
        // Closer distillations of the C++ scope pattern's inner template
        // group: possessive repeats inside the captured group.
        assert_capture_replay(r"((<[^<>]*+>)\s*+)?(::)", "<T> ::", 0, &[1, 2, 3]);
        assert_capture_replay(r"((<(?:[^<>]++|x)*>)\s*+)?(::)", "<T> ::", 0, &[1, 2, 3]);
        assert_capture_replay(
            r"(?<g>(<(?:[^<>]++|\g<g>)*>)\s*+)?(::)",
            "<a<b>> ::",
            0,
            &[1, 2, 3],
        );
        assert_capture_replay(
            r"([a-z]+)\s*+((<(?:x|[^<>]++)*>)\s*+)?(::)",
            "vec<T, A> ::",
            0,
            &[1, 2, 3, 4],
        );
    }

    #[test]
    fn folded_lookbehind_replays_captures_at_utf8_boundaries() {
        for (pattern, line, start) in [
            (r"(?<=(?i:(k|s)))x", "🛰Kx", 7),
            (r"(?<=(?i:(K|ſ)))x", "sx", 1),
            (r"(?i:(?<=(k))x)", "Kx", 3),
        ] {
            let parsed = parse(pattern);
            let program = Program::compile_captures(&parsed, &[1]).unwrap();
            let matched = program
                .execute_captures(
                    line,
                    start,
                    context(),
                    &mut StepBudget::new(1000),
                    &mut BytecodeScratch::default(),
                )
                .unwrap()
                .unwrap();
            assert_eq!(matched.end, line.len());
            assert_eq!(
                matched.captures,
                vec![
                    Some(start..line.len()),
                    Some(if start == 7 { 4..7 } else { 0..start })
                ]
            );
        }
    }

    #[test]
    fn compact_layout_handles_nested_sparse_and_utf8_captures() {
        let parsed = parse(r"((a)(éλ))(z)");
        let program = Program::compile_captures(&parsed, &[4, 3, 3, 99]).unwrap();
        assert_eq!(program.capture_layout(), &[0, 3, 4]);
        assert_capture_replay(r"((a)(éλ))(z)", "xaéλz", 1, &[4, 3, 3, 99]);
        assert_capture_replay(r"((β|é)+)(λ)?", "βéλ", 0, &[1, 2, 3]);
    }

    #[test]
    fn capture_conditionals_select_numbered_named_and_empty_branches() {
        for (pattern, line, expected_end) in [
            (r"(a)?(?(1)b|c)d", "abd", 3),
            (r"(a)?(?(1)b|c)d", "cd", 2),
            (r"(?<x>a)?(?(<x>)b|c)d", "abd", 3),
            (r"(?<x>a)?(?(<x>)b|c)d", "cd", 2),
            (r"(a)?(?(1)b)d", "d", 1),
        ] {
            let parsed = parse(pattern);
            let program = Program::compile_captures(&parsed, &[]).unwrap();
            let mut budget = StepBudget::new(100_000);
            let matched = program
                .execute_captures(
                    line,
                    0,
                    context(),
                    &mut budget,
                    &mut BytecodeScratch::default(),
                )
                .unwrap()
                .unwrap();
            assert_eq!(matched.end, expected_end, "{pattern:?} on {line:?}");
        }
    }

    #[test]
    fn out_of_range_numeric_backrefs_reject_capture_bytecode() {
        let parsed = parse(r"(?<=:)\3*(?<value>[^,}]+)");
        let error = Program::compile_captures_with_analysis(&parsed, parsed.analysis(), &[])
            .expect_err("backrefs to missing groups must not compile to capture bytecode");
        assert!(matches!(error, CompileError::Backreference));
    }
}
