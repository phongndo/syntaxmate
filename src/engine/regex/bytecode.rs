//! Ordered backtracking bytecode, with an optional compact capture layout.
//!
//! The program is immutable and compiled from the shared [`ParsedRegex`].
//! Mutable DFS, assertion, and repeat state lives in [`BytecodeScratch`], so a
//! caller can reuse its allocations across candidate attempts.

use super::analysis::RegexAnalysis;
use super::ast::{Ast, Backref, CharClass, ClassAtom, LookKind, ParsedRegex, RegexFlags};
use super::backtrack::{
    BudgetExceeded, StepBudget, anchor_matches, char_at, class_contains,
    is_cpp_space_comment_separator, match_literal_end, previous_char, unicode_case_eq,
};
use super::{AnchorContext, is_unicode_word_char};
use std::ops::Range;

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
}

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
    u32::try_from(index).map_err(|_| CompileError::TableOverflow)
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
#[derive(Debug, Clone, Default)]
struct LiteralTrie {
    nodes: Vec<LiteralTrieNode>,
    unicode_nodes: Vec<UnicodeLiteralTrieNode>,
}

#[derive(Debug, Clone)]
enum LiteralTrieEdges<T> {
    Empty,
    One((T, u32)),
    Many(Vec<(T, u32)>),
}

impl<T> Default for LiteralTrieEdges<T> {
    fn default() -> Self {
        Self::Empty
    }
}

impl<T: Copy> LiteralTrieEdges<T> {
    fn iter(&self) -> std::slice::Iter<'_, (T, u32)> {
        match self {
            Self::Empty => [].iter(),
            Self::One(edge) => std::slice::from_ref(edge).iter(),
            Self::Many(edges) => edges.iter(),
        }
    }

    fn get(&self, key: T) -> Option<u32>
    where
        T: Ord,
    {
        match self {
            Self::Empty => None,
            Self::One((edge, child)) => (*edge == key).then_some(*child),
            Self::Many(edges) => {
                if edges.len() <= 8 {
                    edges
                        .iter()
                        .find_map(|(edge, child)| (*edge == key).then_some(*child))
                } else {
                    edges
                        .binary_search_by_key(&key, |(edge, _)| *edge)
                        .ok()
                        .map(|index| edges[index].1)
                }
            }
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

#[derive(Debug, Clone, Default)]
struct LiteralTrieNode {
    // Most trie nodes have one child. Keeping that edge inline avoids one heap
    // allocation per byte while preserving a Vec only for actual branches.
    edges: LiteralTrieEdges<u8>,
    terminal_order: Option<u32>,
}

#[derive(Debug, Clone, Default)]
struct UnicodeLiteralTrieNode {
    edges: LiteralTrieEdges<char>,
    terminal_order: Option<u32>,
}

#[derive(Debug, Clone)]
struct CompiledClass {
    source: CharClass,
    ascii_sensitive: [u64; 2],
    ascii_insensitive: [u64; 2],
}

impl CompiledClass {
    fn new(source: CharClass) -> Self {
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

    fn matches_ascii(&self, byte: u8, case_insensitive: bool) -> bool {
        let bitmap = if case_insensitive {
            &self.ascii_insensitive
        } else {
            &self.ascii_sensitive
        };
        bitmap[byte as usize / 64] & (1u64 << (byte % 64)) != 0
    }
}

type AsciiMask = [u64; 2];

fn ascii_mask_set(mask: &mut AsciiMask, byte: u8) {
    debug_assert!(byte < 128);
    mask[byte as usize / 64] |= 1u64 << (byte % 64);
}

/// Exact ASCII membership bitmaps (case-sensitive, case-insensitive) for a
/// class, mirroring `class_contains` on `0..=127`.
fn ascii_class_masks(class: &CharClass) -> (AsciiMask, AsciiMask) {
    let (mut sensitive, mut insensitive) = ascii_union_masks(&class.atoms);
    for union in &class.intersections {
        let (term_sensitive, term_insensitive) = ascii_union_masks(union);
        sensitive[0] &= term_sensitive[0];
        sensitive[1] &= term_sensitive[1];
        insensitive[0] &= term_insensitive[0];
        insensitive[1] &= term_insensitive[1];
    }
    if class.negated {
        // Negation is exact within the ASCII range: membership of an ASCII
        // character depends only on the (complete) positive masks.
        sensitive = [!sensitive[0], !sensitive[1]];
        insensitive = [!insensitive[0], !insensitive[1]];
    }
    (sensitive, insensitive)
}

fn ascii_union_masks(atoms: &[ClassAtom]) -> (AsciiMask, AsciiMask) {
    let mut sensitive = [0u64; 2];
    let mut insensitive = [0u64; 2];
    for atom in atoms {
        let (atom_sensitive, atom_insensitive) = ascii_atom_masks(atom);
        sensitive[0] |= atom_sensitive[0];
        sensitive[1] |= atom_sensitive[1];
        insensitive[0] |= atom_insensitive[0];
        insensitive[1] |= atom_insensitive[1];
    }
    (sensitive, insensitive)
}

fn ascii_atom_masks(atom: &ClassAtom) -> (AsciiMask, AsciiMask) {
    match atom {
        ClassAtom::Char(ch) if ch.is_ascii() => {
            let byte = *ch as u8;
            let mut sensitive = [0u64; 2];
            ascii_mask_set(&mut sensitive, byte);
            let mut insensitive = sensitive;
            ascii_mask_set(&mut insensitive, byte.to_ascii_lowercase());
            ascii_mask_set(&mut insensitive, byte.to_ascii_uppercase());
            (sensitive, insensitive)
        }
        ClassAtom::Range(start, end) if start.is_ascii() && end.is_ascii() => {
            let (low, high) = (*start as u8, *end as u8);
            let mut sensitive = [0u64; 2];
            let mut insensitive = [0u64; 2];
            for byte in 0u8..=127 {
                let ch = byte as char;
                if low <= byte && byte <= high {
                    ascii_mask_set(&mut sensitive, byte);
                }
                // Mirror the evaluator's folded-range semantics with ASCII
                // case maps (exact for ASCII probes and bounds).
                let folded_low = ch.to_ascii_lowercase() as u8;
                let folded_up = ch.to_ascii_uppercase() as u8;
                if (low.to_ascii_lowercase() <= folded_low
                    && folded_low <= high.to_ascii_lowercase())
                    || (low.to_ascii_uppercase() <= folded_up
                        && folded_up <= high.to_ascii_uppercase())
                {
                    ascii_mask_set(&mut insensitive, byte);
                }
            }
            (sensitive, insensitive)
        }
        // Perl, POSIX, and Unicode-property atoms ignore the case flag, so
        // one cheap per-character pass fills both masks without any Unicode
        // case conversion.
        ClassAtom::Perl(kind) => {
            let mask = ascii_predicate_mask(|ch| super::backtrack::perl_class_contains(*kind, ch));
            (mask, mask)
        }
        ClassAtom::Posix { name, negated } => {
            let mask = ascii_predicate_mask(|ch| {
                super::backtrack::posix_class_contains(name, ch) != *negated
            });
            (mask, mask)
        }
        ClassAtom::Unicode { name, negated } => {
            let mask = ascii_predicate_mask(|ch| {
                super::backtrack::unicode_class_contains(name, ch) != *negated
            });
            (mask, mask)
        }
        ClassAtom::Nested(class) => ascii_class_masks(class),
        // Non-ASCII chars and ranges can still fold into ASCII under case
        // insensitivity (e.g. the Kelvin sign); evaluate the atom directly.
        ClassAtom::Char(_) | ClassAtom::Range(..) => {
            let sensitive = ascii_predicate_mask(|ch| {
                super::backtrack::atom_contains(atom, ch, RegexFlags::default())
            });
            let insensitive = ascii_predicate_mask(|ch| {
                super::backtrack::atom_contains(
                    atom,
                    ch,
                    RegexFlags {
                        case_insensitive: true,
                        ..RegexFlags::default()
                    },
                )
            });
            (sensitive, insensitive)
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
    Split {
        preferred: ProgramCounter,
        alternate: ProgramCounter,
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
    /// Possessive repeat of a single-consumer node (`\s*+`, `[^x]++`, …):
    /// consume greedily in place with no backtrack frames or cut bookkeeping.
    ScanRepeat {
        node: ScanNode,
        flags: InstructionFlags,
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

#[derive(Debug, Clone, Copy, Default)]
struct RepeatState {
    count: u32,
    last_position: usize,
    stalled: bool,
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
    call_depth: u32,
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
    parent_call_depth: u32,
    backtrack_base: u32,
    cut_base: u32,
    positive: bool,
    has_next_probe: bool,
}

#[derive(Debug, Clone, Copy)]
struct CallFrame {
    return_pc: ProgramCounter,
    capture_undo_mark: u32,
}

// These are performance contracts, not incidental implementation details.
// Keep them compile-time checked on the 64-bit targets used for profiling and
// normal desktop/server deployment.
#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(std::mem::size_of::<Instruction>() == 24);
    assert!(std::mem::size_of::<BacktrackFrame>() == 32);
    assert!(std::mem::size_of::<AssertionFrame>() == 64);
    assert!(std::mem::size_of::<CallFrame>() == 8);
    assert!(std::mem::size_of::<RepeatState>() == 16);
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
    repeat_undo: Vec<(VmSlot, RepeatState)>,
    capture_undo: Vec<(VmSlot, CaptureState)>,
    calls: Vec<CallFrame>,
    call_depth: u32,
    cuts: Vec<u32>,
    literal_matches: Vec<(u32, usize)>,
    scanner: super::scanner::ScannerScratch,
    prefilter_cursors: super::prefilter::PrefilterCursors,
    line_ptr: usize,
    line_len: usize,
    line_is_ascii: bool,
    line_block_comment: Option<bool>,
}

impl BytecodeScratch {
    pub(crate) fn begin_line(&mut self, line: &str) {
        self.prefilter_cursors.begin_line(line);
        self.line_ptr = line.as_ptr() as usize;
        self.line_len = line.len();
        self.line_is_ascii = line.is_ascii();
        self.line_block_comment = None;
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
        call_depth: scratch.call_depth,
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
        Compiler::with_captures(layout).compile(parsed)
    }

    #[allow(dead_code)] // Vertical-slice API; backtrack/tokenizer integration follows.
    pub(crate) fn capture_layout(&self) -> &[u32] {
        &self.capture_layout
    }

    pub(crate) fn is_beneficial(parsed: &ParsedRegex) -> bool {
        ordered_fanout_score(&parsed.ast) >= beneficial_fanout_threshold()
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
                    let class = &self.classes[id.0 as usize];
                    let matched = match line.as_bytes().get(position).copied() {
                        Some(byte) if byte.is_ascii() => class
                            .matches_ascii(byte, flags.case_insensitive())
                            .then_some(position + 1),
                        Some(_) => char_at(line, position)
                            .filter(|(ch, _)| class_contains(&class.source, *ch, flags.regex()))
                            .map(|(_, end)| end),
                        None => None,
                    };
                    if let Some(end) = matched {
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
                    if scratch.call_depth >= 128 {
                        if !self.backtrack_or_resolve(line, scratch, &mut pc, &mut position)? {
                            return Ok(None);
                        }
                    } else {
                        let frame = CallFrame {
                            return_pc: *next,
                            capture_undo_mark: arena_mark(scratch.capture_undo.len())?,
                        };
                        let call_depth = arena_index(scratch.call_depth);
                        if call_depth == scratch.calls.len() {
                            scratch.calls.push(frame);
                        } else {
                            scratch.calls[call_depth] = frame;
                        }
                        scratch.call_depth += 1;
                        pc = *entry;
                    }
                }
                Instruction::Return => {
                    debug_assert!(scratch.call_depth > 0, "Return outside subroutine");
                    scratch.call_depth -= 1;
                    let frame = scratch.calls[arena_index(scratch.call_depth)];
                    // Recursive calls to the same capturing group overwrite
                    // an enclosing pending start. Restore pending captures on
                    // return; completed captures remain observable.
                    for index in arena_index(frame.capture_undo_mark)..scratch.capture_undo.len() {
                        let (slot, previous) = &scratch.capture_undo[index];
                        if let CaptureState::Open(start) = previous
                            && !scratch.capture_undo[arena_index(frame.capture_undo_mark)..index]
                                .iter()
                                .any(|(earlier, _)| earlier == slot)
                        {
                            scratch.captures[arena_index(*slot)] = CaptureState::Open(*start);
                        }
                    }
                    pc = frame.return_pc;
                }
                Instruction::Split {
                    preferred,
                    alternate,
                } => {
                    scratch.backtrack.push(backtrack_frame(
                        scratch,
                        *alternate,
                        position,
                        ResumeAction::None,
                    )?);
                    pc = *preferred;
                }
                Instruction::RepeatInit { slot, next } => {
                    set_repeat(
                        scratch,
                        *slot,
                        RepeatState {
                            count: 0,
                            last_position: position,
                            stalled: false,
                        },
                    );
                    pc = *next;
                }
                Instruction::Repeat {
                    slot,
                    bounds,
                    greedy,
                    body,
                    next,
                } => {
                    let repeat = scratch.repeats[arena_index(*slot)];
                    let count = repeat.count;
                    let can_exit = count >= bounds.min;
                    let can_repeat = bounds.max().is_none_or(|max| count < max)
                        && (!repeat.stalled || count < bounds.min);
                    match (can_repeat, can_exit, greedy) {
                        (true, true, true) => {
                            scratch.backtrack.push(backtrack_frame(
                                scratch,
                                *next,
                                position,
                                ResumeAction::None,
                            )?);
                            enter_repeat(scratch, *slot, position);
                            pc = *body;
                        }
                        (true, true, false) => {
                            scratch.backtrack.push(backtrack_frame(
                                scratch,
                                *body,
                                position,
                                ResumeAction::EnterRepeat(*slot),
                            )?);
                            pc = *next;
                        }
                        (true, false, _) => {
                            enter_repeat(scratch, *slot, position);
                            pc = *body;
                        }
                        (false, true, _) => pc = *next,
                        (false, false, _) => {
                            if !self.backtrack_or_resolve(line, scratch, &mut pc, &mut position)? {
                                return Ok(None);
                            }
                        }
                    }
                }
                Instruction::RepeatEnd { slot, repeat } => {
                    let index = arena_index(*slot);
                    if scratch.repeats[index].last_position == position {
                        let mut value = scratch.repeats[index];
                        value.stalled = true;
                        set_repeat(scratch, *slot, value);
                    }
                    pc = *repeat;
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
                    let mut frame = AssertionFrame {
                        parent_position: position,
                        target_end: position,
                        next_probe: 0,
                        direction: *direction,
                        entry: *entry,
                        parent_pc: *next,
                        parent_repeat_undo_mark: arena_mark(scratch.repeat_undo.len())?,
                        parent_capture_undo_mark: arena_mark(scratch.capture_undo.len())?,
                        parent_call_depth: scratch.call_depth,
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
                    bounds,
                    next,
                } => {
                    let mut count = 0u32;
                    let mut cursor = position;
                    while bounds.max().is_none_or(|max| count < max) {
                        let advanced = match node {
                            ScanNode::Literal(id) => {
                                let value = &self.literals[id.0 as usize];
                                match_literal_end(line, cursor, value, flags.regex())
                            }
                            ScanNode::Class(id) => {
                                let class = &self.classes[id.0 as usize];
                                match line.as_bytes().get(cursor).copied() {
                                    Some(byte) if byte.is_ascii() => class
                                        .matches_ascii(byte, flags.case_insensitive())
                                        .then_some(cursor + 1),
                                    Some(_) => char_at(line, cursor).and_then(|(ch, end)| {
                                        class_contains(&class.source, ch, flags.regex())
                                            .then_some(end)
                                    }),
                                    None => None,
                                }
                            }
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
                scratch.call_depth = frame.call_depth;
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
                }
                return Ok(true);
            }

            let Some(mut assertion) = scratch.assertions.pop() else {
                return Ok(false);
            };
            undo_repeats_to(scratch, assertion.parent_repeat_undo_mark);
            undo_captures_to(scratch, assertion.parent_capture_undo_mark);
            scratch.call_depth = assertion.parent_call_depth;
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
        scratch.call_depth = assertion.parent_call_depth;
        *position = assertion.parent_position;
        if matched == assertion.positive {
            *pc = assertion.parent_pc;
        } else {
            *pc = INVALID_PROGRAM_COUNTER;
        }
    }
}

fn collect_group_definitions(
    ast: &Ast,
    flags: RegexFlags,
    definitions: &mut std::collections::BTreeMap<u32, (Ast, RegexFlags)>,
) {
    if let Ast::Group {
        index: Some(index), ..
    } = ast
    {
        definitions.insert(*index, (ast.clone(), flags));
    }
    match ast {
        Ast::Concat(nodes) | Ast::Alternation(nodes) => {
            for node in nodes {
                collect_group_definitions(node, flags, definitions);
            }
        }
        Ast::Conditional {
            matched, unmatched, ..
        } => {
            collect_group_definitions(matched, flags, definitions);
            collect_group_definitions(unmatched, flags, definitions);
        }
        Ast::Flags {
            flags: local,
            child,
        } => collect_group_definitions(child, *local, definitions),
        Ast::Repeat { node, .. }
        | Ast::Group { child: node, .. }
        | Ast::Look { child: node, .. } => {
            collect_group_definitions(node, flags, definitions);
        }
        Ast::Empty
        | Ast::Literal(_)
        | Ast::Dot
        | Ast::Grapheme
        | Ast::Class(_)
        | Ast::Anchor(_)
        | Ast::Backref(_)
        | Ast::Subroutine(_)
        | Ast::Unsupported(_) => {}
    }
}

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
    fn reset(&mut self, repeat_slots: usize, capture_slots: usize) {
        self.backtrack.clear();
        self.assertions.clear();
        self.repeat_undo.clear();
        self.capture_undo.clear();
        self.call_depth = 0;
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
    scratch.repeat_undo.push((slot, old));
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
    value.stalled = false;
    set_repeat(scratch, slot, value);
}

fn undo_repeats_to(scratch: &mut BytecodeScratch, mark: u32) {
    while scratch.repeat_undo.len() > arena_index(mark) {
        let (slot, value) = scratch.repeat_undo.pop().expect("repeat undo above mark");
        scratch.repeats[arena_index(slot)] = value;
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

struct Compiler {
    instructions: Vec<Instruction>,
    literals: Vec<String>,
    literal_tries: Vec<LiteralTrie>,
    classes: Vec<CompiledClass>,
    repeat_slots: VmSlot,
    capture_layout: Vec<u32>,
    named_captures: std::collections::BTreeMap<String, u32>,
    routine_entries: std::collections::BTreeMap<u32, ProgramCounter>,
}

impl Compiler {
    fn new() -> Self {
        Self {
            instructions: Vec::new(),
            literals: Vec::new(),
            literal_tries: Vec::new(),
            classes: Vec::new(),
            repeat_slots: 0,
            capture_layout: Vec::new(),
            named_captures: std::collections::BTreeMap::new(),
            routine_entries: std::collections::BTreeMap::new(),
        }
    }

    fn with_captures(capture_layout: Vec<u32>) -> Self {
        Self {
            capture_layout,
            ..Self::new()
        }
    }

    fn compile(mut self, parsed: &ParsedRegex) -> Result<Program, CompileError> {
        self.named_captures.clone_from(&parsed.named_captures);
        self.instructions
            .reserve(instruction_capacity_hint(&parsed.ast));
        if !self.capture_layout.is_empty() && parsed.features.subroutine {
            let mut definitions = std::collections::BTreeMap::new();
            collect_group_definitions(&parsed.ast, parsed.flags, &mut definitions);
            for group in definitions.keys() {
                let placeholder = self.push(Instruction::Fail);
                self.routine_entries.insert(*group, placeholder);
            }
            for (group, (node, flags)) in definitions {
                let return_pc = self.push(Instruction::Return);
                let actual = self.compile_node(&node, flags, return_pc)?;
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
        })
    }

    fn compile_node(
        &mut self,
        ast: &Ast,
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
                let captures_can_be_elided =
                    !branches_contain_live_capture(branches, &self.capture_layout);
                if captures_can_be_elided && is_cpp_space_comment_separator(branches) {
                    return Ok(self.push(Instruction::CppSpaceCommentSeparator { next }));
                }
                if captures_can_be_elided
                    && let Some(literals) = exact_literal_branches(branches, flags)
                {
                    let id = self.intern_literal_trie(&literals, flags)?;
                    return Ok(self.push(Instruction::LiteralTrie {
                        id,
                        flags: flags.into(),
                        next,
                    }));
                }
                let mut entries = Vec::with_capacity(branches.len());
                let mut branch = 0;
                while branch < branches.len() {
                    // Large grammar closures often contain a mostly-literal
                    // keyword alternation with a few structured variants
                    // (`foo|bar|create( or alter)?|...`). Treating one such
                    // variant as a reason to compile every literal into a
                    // Split chain makes each negative probe walk the full
                    // closure. Compact contiguous literal runs into the same
                    // reusable trie used by all-literal alternations. Keeping
                    // runs contiguous preserves ordered-alternation priority
                    // around the structured branches.
                    if captures_can_be_elided
                        && exact_literal_ast(&branches[branch], flags).is_some()
                    {
                        let run_start = branch;
                        let mut literals = Vec::new();
                        while branch < branches.len() {
                            let Some(literal) = exact_literal_ast(&branches[branch], flags) else {
                                break;
                            };
                            literals.push(literal);
                            branch += 1;
                        }
                        if literals.len() >= 4 {
                            let id = self.intern_literal_trie(&literals, flags)?;
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
                    entries.push(self.compile_node(&branches[branch], flags, next)?);
                    branch += 1;
                }
                let mut entry = entries.pop().unwrap_or(next);
                for preferred in entries.into_iter().rev() {
                    entry = self.push(Instruction::Split {
                        preferred,
                        alternate: entry,
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
                if cut && let Some(scan) = self.scan_node(node, flags) {
                    let (scan, scan_flags) = scan;
                    return Ok(self.push(Instruction::ScanRepeat {
                        node: scan,
                        flags: scan_flags.into(),
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
                    let body = self.compile_node(node, flags, end)?;
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
                    LookKind::Behind => (true, lookbehind_direction(child)?),
                    LookKind::NotBehind => (false, lookbehind_direction(child)?),
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

    fn push(&mut self, instruction: Instruction) -> ProgramCounter {
        let index = program_counter(self.instructions.len())
            .expect("bytecode program exceeds compact program-counter space");
        self.instructions.push(instruction);
        index
    }

    /// Extracts a single-consumer body for `ScanRepeat`, looking through flag
    /// scopes and non-captured groups. Empty literals are rejected because a
    /// scan must always make progress.
    fn scan_node(&mut self, ast: &Ast, flags: RegexFlags) -> Option<(ScanNode, RegexFlags)> {
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

    fn intern_literal(&mut self, literal: &str) -> Result<LiteralId, CompileError> {
        if let Some(index) = self.literals.iter().position(|value| value == literal) {
            return u32::try_from(index)
                .map(LiteralId)
                .map_err(|_| CompileError::TableOverflow);
        }
        let id = u32::try_from(self.literals.len()).map_err(|_| CompileError::TableOverflow)?;
        self.literals.push(literal.to_owned());
        Ok(LiteralId(id))
    }

    fn intern_class(&mut self, class: &CharClass) -> Result<ClassId, CompileError> {
        if let Some(index) = self.classes.iter().position(|value| value.source == *class) {
            return u32::try_from(index)
                .map(ClassId)
                .map_err(|_| CompileError::TableOverflow);
        }
        let id = u32::try_from(self.classes.len()).map_err(|_| CompileError::TableOverflow)?;
        self.classes.push(CompiledClass::new(class.clone()));
        Ok(ClassId(id))
    }

    fn intern_literal_trie(
        &mut self,
        literals: &[String],
        flags: RegexFlags,
    ) -> Result<LiteralTrieId, CompileError> {
        let id =
            u32::try_from(self.literal_tries.len()).map_err(|_| CompileError::TableOverflow)?;
        self.literal_tries.push(LiteralTrie::new(literals, flags)?);
        Ok(LiteralTrieId(id))
    }
}

impl LiteralTrie {
    fn new(literals: &[String], flags: RegexFlags) -> Result<Self, CompileError> {
        let unicode = flags.case_insensitive && literals.iter().any(|literal| !literal.is_ascii());
        let node_capacity = literals
            .iter()
            .fold(1usize, |nodes, literal| {
                nodes.saturating_add(if unicode {
                    literal.chars().count()
                } else {
                    literal.len()
                })
            })
            .min(LITERAL_TRIE_NODE_RESERVE_LIMIT);
        let mut trie = Self {
            nodes: if unicode {
                Vec::new()
            } else {
                Vec::with_capacity(node_capacity)
            },
            unicode_nodes: if unicode {
                Vec::with_capacity(node_capacity)
            } else {
                Vec::new()
            },
        };
        if unicode {
            trie.unicode_nodes.push(UnicodeLiteralTrieNode::default());
            for (order, literal) in literals.iter().enumerate() {
                let order = u32::try_from(order).map_err(|_| CompileError::TableOverflow)?;
                let mut node = 0usize;
                for ch in literal.chars() {
                    let edge = trie.unicode_nodes[node]
                        .edges
                        .iter()
                        .find(|(edge, _)| unicode_case_eq(*edge, ch))
                        .map(|(_, child)| *child);
                    node = if let Some(child) = edge {
                        child as usize
                    } else {
                        let child = u32::try_from(trie.unicode_nodes.len())
                            .map_err(|_| CompileError::TableOverflow)?;
                        trie.unicode_nodes.push(UnicodeLiteralTrieNode::default());
                        trie.unicode_nodes[node].edges.push((ch, child));
                        child as usize
                    };
                }
                let terminal = &mut trie.unicode_nodes[node].terminal_order;
                if terminal.is_none_or(|existing| order < existing) {
                    *terminal = Some(order);
                }
            }
            return Ok(trie);
        }
        trie.nodes.push(LiteralTrieNode::default());
        for (order, literal) in literals.iter().enumerate() {
            let order = u32::try_from(order).map_err(|_| CompileError::TableOverflow)?;
            let mut node = 0usize;
            for mut byte in literal.bytes() {
                if flags.case_insensitive {
                    byte.make_ascii_lowercase();
                }
                let edge = trie.nodes[node]
                    .edges
                    .iter()
                    .find(|(edge, _)| *edge == byte)
                    .map(|(_, child)| *child);
                node = if let Some(child) = edge {
                    child as usize
                } else {
                    let child =
                        u32::try_from(trie.nodes.len()).map_err(|_| CompileError::TableOverflow)?;
                    trie.nodes.push(LiteralTrieNode::default());
                    trie.nodes[node].edges.push((byte, child));
                    child as usize
                };
            }
            let terminal = &mut trie.nodes[node].terminal_order;
            if terminal.is_none_or(|existing| order < existing) {
                *terminal = Some(order);
            }
        }
        trie.finish_ascii_edges();
        Ok(trie)
    }

    fn finish_ascii_edges(&mut self) {
        for node in &mut self.nodes {
            if let LiteralTrieEdges::Many(edges) = &mut node.edges {
                edges.sort_unstable_by_key(|(byte, _)| *byte);
            }
        }
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
                let child = self.unicode_nodes[node]
                    .edges
                    .iter()
                    .find_map(|(edge, child)| unicode_case_eq(*edge, input).then_some(*child));
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
            let Some(child) = self.nodes[node].edges.get(input) else {
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

fn exact_literal_branches(branches: &[Ast], flags: RegexFlags) -> Option<Vec<String>> {
    // Small alternations do not amortize a second table and already execute
    // cheaply as ordered `Split`s.
    if branches.len() < 4 {
        return None;
    }
    branches
        .iter()
        .map(|branch| exact_literal_ast(branch, flags))
        .collect::<Option<Vec<_>>>()
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

fn exact_literal_ast(ast: &Ast, flags: RegexFlags) -> Option<String> {
    match ast {
        Ast::Empty => Some(String::new()),
        Ast::Literal(literal) => Some(literal.clone()),
        Ast::Concat(nodes) => {
            let mut literal = String::new();
            for node in nodes {
                literal.push_str(&exact_literal_ast(node, flags)?);
            }
            Some(literal)
        }
        Ast::Group { child, .. } => exact_literal_ast(child, flags),
        Ast::Flags {
            flags: local,
            child,
        } if *local == flags => exact_literal_ast(child, flags),
        _ => None,
    }
}

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

fn lookbehind_direction(ast: &Ast) -> Result<AssertDirection, CompileError> {
    let (min_width, max_width) = byte_width(ast);
    AssertDirection::behind(min_width, max_width)
}

fn byte_width(ast: &Ast) -> (usize, Option<usize>) {
    match ast {
        Ast::Empty | Ast::Anchor(_) | Ast::Look { .. } => (0, Some(0)),
        Ast::Literal(value) => (value.len(), Some(value.len())),
        Ast::Dot | Ast::Class(_) => (1, Some(4)),
        Ast::Concat(nodes) => nodes.iter().fold((0usize, Some(0usize)), |acc, node| {
            let width = byte_width(node);
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
                let width = byte_width(branch);
                min = min.min(width.0);
                max = max.zip(width.1).map(|(left, right)| left.max(right));
            }
            (min, max)
        }
        Ast::Conditional {
            matched, unmatched, ..
        } => {
            let matched = byte_width(matched);
            let unmatched = byte_width(unmatched);
            (
                matched.0.min(unmatched.0),
                matched
                    .1
                    .zip(unmatched.1)
                    .map(|(matched, unmatched)| matched.max(unmatched)),
            )
        }
        Ast::Repeat { node, min, max, .. } => {
            let width = byte_width(node);
            (
                width.0.saturating_mul(*min),
                max.and_then(|count| width.1.map(|width| width.saturating_mul(count))),
            )
        }
        Ast::Group { child, .. } | Ast::Flags { child, .. } => byte_width(child),
        Ast::Grapheme => (1, None),
        Ast::Backref(_) | Ast::Subroutine(_) | Ast::Unsupported(_) => (0, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::regex::ast::parse;
    use crate::engine::regex::backtrack::{FallbackMatcher, recursive_position_span};

    fn context() -> AnchorContext {
        AnchorContext {
            allow_a: true,
            allow_g: true,
            g_pos: 0,
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
        assert_eq!(std::mem::size_of::<CallFrame>(), 8);
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
    fn literal_trie_bounds_reservation_for_duplicate_branches() {
        let literals = vec!["a".to_owned(); LITERAL_TRIE_NODE_RESERVE_LIMIT * 2];
        let trie = LiteralTrie::new(&literals, RegexFlags::default()).unwrap();

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
    fn literal_trie_compacts_runs_around_structured_alternatives() {
        let mut branches = (0..250)
            .map(|index| format!("kw{index:03}"))
            .collect::<Vec<_>>();
        branches.push("special(?:ized)?".to_owned());
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
