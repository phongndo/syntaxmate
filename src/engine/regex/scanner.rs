//! Capture-free, ordered, multi-pattern Thompson NFA.
//!
//! The NFA finds a winner only. Capture replay deliberately belongs to the
//! caller. Threads are kept in backtracking priority order, so an `Accept`
//! thread can wait behind a preferred (for example greedy) path without
//! losing the endpoint it reached.

use super::AnchorContext;
use super::ast::{Ast, CharClass, ClassAtom, ParsedRegex, RegexFlags};
use super::backtrack::{anchor_matches, char_at, class_contains};
use super::bytecode::CompiledClass;

const NO_TARGET: usize = usize::MAX;
const MAX_STATES: usize = 16_384;
/// Alternations with at least this many branches get a first-byte dispatch
/// table instead of a linear `Split` chain.
const DISPATCH_MIN_BRANCHES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompileError {
    Lookaround,
    Backreference,
    Subroutine,
    Possessive,
    Unsupported,
    InvalidRepeat,
    TooLarge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CompileFailure {
    pub(crate) pattern: usize,
    pub(crate) error: CompileError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScanMatch {
    pub(crate) pattern: usize,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

#[derive(Debug, Clone)]
enum Inst {
    Char {
        ch: char,
        flags: RegexFlags,
        next: usize,
    },
    Class {
        class: usize,
        flags: RegexFlags,
        next: usize,
    },
    Any {
        flags: RegexFlags,
        next: usize,
    },
    Anchor {
        kind: super::ast::AnchorKind,
        next: usize,
    },
    Split {
        preferred: usize,
        alternate: usize,
    },
    /// Ordered alternation whose live branches depend only on the next input
    /// byte. See [`Dispatch`].
    Dispatch {
        table: usize,
    },
    Accept {
        pattern: usize,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct Scanner {
    insts: Vec<Inst>,
    classes: Vec<CompiledClass>,
    dispatches: Vec<Dispatch>,
    entries: Vec<usize>,
    /// Start entries (indexes into `entries`) that can begin at each byte.
    starts: ByteTable,
}

/// Ordered targets that can make progress for each next input byte.
///
/// Threads whose first consuming instruction cannot accept the next byte die
/// on the next step without producing anything, so leaving them out of a
/// closure is unobservable. Each byte class lists, in the original priority
/// order, the targets that can consume a byte of that class or reach
/// `Accept` without consuming; the final class is the end-of-input list and
/// keeps only the latter. First-byte sets come from [`Compiler::first_bytes`]
/// and are conservative.
#[derive(Debug, Clone)]
struct ByteTable {
    byte_class: Box<[u8; 256]>,
    /// `offsets[class]..offsets[class + 1]` indexes `targets`.
    offsets: Box<[u32]>,
    targets: Box<[u32]>,
}

impl ByteTable {
    /// Returns the table and whether any byte excludes at least one target.
    fn build(items: &[(u32, FirstBytes)]) -> (Self, bool) {
        // Transpose the per-item byte sets into one item bitset per byte, so
        // grouping bytes into classes compares a few words instead of
        // rebuilding every byte's target list.
        let words = items.len().div_ceil(64).max(1);
        let mut members = vec![0u64; 256 * words];
        for (index, (_, first)) in items.iter().enumerate() {
            let bit = 1u64 << (index % 64);
            let word = index / 64;
            for (chunk, bits) in first.bytes.iter().enumerate() {
                let mut bits = if first.nullable { u64::MAX } else { *bits };
                while bits != 0 {
                    let byte = chunk * 64 + bits.trailing_zeros() as usize;
                    members[byte * words + word] |= bit;
                    bits &= bits - 1;
                }
            }
        }
        let mut byte_class = Box::new([0u8; 256]);
        // (first byte with this membership, target range)
        let mut classes: Vec<(usize, usize, usize)> = Vec::new();
        // Membership fingerprint per class, so wide rows (large dispatches)
        // are compared in full only when they probably match.
        let mut fingerprints: Vec<u64> = Vec::new();
        let mut targets = Vec::new();
        let mut pruned = false;
        for byte in 0..256 {
            let row = &members[byte * words..(byte + 1) * words];
            let fingerprint = match row {
                [word] => *word,
                row => row.iter().fold(0u64, |hash, word| {
                    (hash ^ word)
                        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
                        .rotate_left(29)
                }),
            };
            let class = classes
                .iter()
                .zip(&fingerprints)
                .position(|(&(first, _, _), &known)| {
                    known == fingerprint && members[first * words..(first + 1) * words] == *row
                })
                .unwrap_or_else(|| {
                    fingerprints.push(fingerprint);
                    let start = targets.len();
                    targets.extend(
                        items
                            .iter()
                            .enumerate()
                            .filter(|(index, _)| row[index / 64] & (1u64 << (index % 64)) != 0)
                            .map(|(_, (target, _))| *target),
                    );
                    classes.push((byte, start, targets.len()));
                    classes.len() - 1
                });
            let (_, start, end) = classes[class];
            pruned |= end - start < items.len();
            // At most 256 distinct classes exist for 256 bytes.
            byte_class[byte] = class as u8;
        }
        let end_start = targets.len();
        targets.extend(
            items
                .iter()
                .filter(|(_, first)| first.nullable)
                .map(|(target, _)| *target),
        );
        let mut offsets = classes
            .iter()
            .map(|&(_, start, _)| start as u32)
            .collect::<Vec<_>>();
        offsets.push(end_start as u32);
        offsets.push(targets.len() as u32);
        (
            Self {
                byte_class,
                offsets: offsets.into_boxed_slice(),
                targets: targets.into_boxed_slice(),
            },
            pruned,
        )
    }

    #[inline]
    fn targets(&self, byte: Option<u8>) -> &[u32] {
        let class = match byte {
            Some(byte) => usize::from(self.byte_class[byte as usize]),
            None => self.offsets.len() - 2,
        };
        &self.targets[self.offsets[class] as usize..self.offsets[class + 1] as usize]
    }

    fn retained_heap_bytes(&self) -> usize {
        256usize
            .saturating_add(self.offsets.len().saturating_mul(4))
            .saturating_add(self.targets.len().saturating_mul(4))
    }
}

/// First-byte dispatch replacing the `Split` chain of a large ordered
/// alternation, whose branches (for example the words of a keyword list)
/// mostly cannot start at any given byte.
#[derive(Debug, Clone)]
struct Dispatch {
    table: ByteTable,
    /// Every branch entry, in priority order, for compile-time analysis.
    branches: Box<[u32]>,
}

#[derive(Debug, Clone)]
struct CompilerEntry {
    pc: usize,
    start_bitmap: Option<[u64; 4]>,
}

#[derive(Debug, Clone, Copy)]
struct Thread {
    pc: usize,
    start: usize,
    // Only meaningful for Accept. It makes an earlier fallback endpoint
    // survive while a higher-priority consuming thread is explored.
    end: usize,
}

/// All mutable NFA storage. Once it has seen a scanner of a given size, scans
/// with that scanner do not allocate.
#[derive(Debug, Clone, Default)]
pub(crate) struct ScannerScratch {
    current: Vec<Thread>,
    next: Vec<Thread>,
    work: Vec<usize>,
    seen: Vec<u32>,
    generation: u32,
    /// Whether an `Accept` thread was added to the list being built.
    accepted: bool,
}

fn char_class_heap_bytes(class: &CharClass) -> usize {
    let mut bytes = class
        .atoms
        .capacity()
        .saturating_mul(std::mem::size_of::<ClassAtom>())
        .saturating_add(
            class
                .intersections
                .capacity()
                .saturating_mul(std::mem::size_of::<Vec<ClassAtom>>()),
        );
    for atom in &class.atoms {
        bytes = bytes.saturating_add(class_atom_heap_bytes(atom));
    }
    for intersection in &class.intersections {
        bytes = bytes.saturating_add(
            intersection
                .capacity()
                .saturating_mul(std::mem::size_of::<ClassAtom>()),
        );
        for atom in intersection {
            bytes = bytes.saturating_add(class_atom_heap_bytes(atom));
        }
    }
    bytes
}

fn class_atom_heap_bytes(atom: &ClassAtom) -> usize {
    match atom {
        ClassAtom::Posix { name, .. } | ClassAtom::Unicode { name, .. } => name.capacity(),
        ClassAtom::Nested(class) => {
            std::mem::size_of::<CharClass>().saturating_add(char_class_heap_bytes(class))
        }
        ClassAtom::Char(_) | ClassAtom::Range(_, _) | ClassAtom::Perl(_) => 0,
    }
}

impl Scanner {
    pub(crate) fn retained_heap_bytes(&self) -> usize {
        let mut bytes = self
            .insts
            .capacity()
            .saturating_mul(std::mem::size_of::<Inst>())
            .saturating_add(
                self.entries
                    .capacity()
                    .saturating_mul(std::mem::size_of::<usize>()),
            )
            .saturating_add(self.starts.retained_heap_bytes())
            .saturating_add(
                self.classes
                    .capacity()
                    .saturating_mul(std::mem::size_of::<CompiledClass>()),
            );
        for class in &self.classes {
            bytes = bytes.saturating_add(char_class_heap_bytes(&class.source));
        }
        bytes = bytes.saturating_add(
            self.dispatches
                .capacity()
                .saturating_mul(std::mem::size_of::<Dispatch>()),
        );
        for dispatch in &self.dispatches {
            bytes = bytes
                .saturating_add(dispatch.table.retained_heap_bytes())
                .saturating_add(dispatch.branches.len().saturating_mul(4));
        }
        bytes
    }

    pub(crate) fn supports(parsed: &ParsedRegex) -> bool {
        !parsed.features.possessive_or_atomic && ast_is_supported(&parsed.ast)
    }

    #[cfg(test)]
    pub(crate) fn compile<'a>(
        patterns: impl IntoIterator<Item = &'a ParsedRegex>,
    ) -> Result<Self, CompileFailure> {
        Self::compile_with_hints(
            patterns
                .into_iter()
                .enumerate()
                .map(|(index, parsed)| (index, parsed, None)),
        )
    }

    #[allow(dead_code)] // Retained for exact all-or-nothing scanner experiments.
    pub(crate) fn compile_with_hints<'a>(
        patterns: impl IntoIterator<Item = (usize, &'a ParsedRegex, Option<&'a [u8]>)>,
    ) -> Result<Self, CompileFailure> {
        let mut compiler = Compiler::default();
        for (pattern, parsed, start_bytes) in patterns {
            compiler.try_add(pattern, parsed, start_bytes)?;
        }
        Ok(compiler.finish())
    }

    /// Compile the regular subset of an ordered candidate set, returning the
    /// exact original candidate indexes that could not be represented by this
    /// capture-free NFA. The caller must run those opaque candidates through
    /// the authoritative matcher when they can beat the regular frontier.
    pub(crate) fn compile_partial_with_hints<'a>(
        patterns: impl IntoIterator<Item = (usize, &'a ParsedRegex, Option<&'a [u8]>)>,
    ) -> (Self, Vec<CompileFailure>) {
        let mut compiler = Compiler::default();
        let mut failures = Vec::new();
        for (pattern, parsed, start_bytes) in patterns {
            if let Err(failure) = compiler.try_add(pattern, parsed, start_bytes) {
                failures.push(failure);
            }
        }
        (compiler.finish(), failures)
    }

    pub(crate) fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Search from the first UTF-8 boundary at or after `from`.
    pub(crate) fn find(
        &self,
        line: &str,
        from: usize,
        ctx: AnchorContext,
        scratch: &mut ScannerScratch,
    ) -> Option<ScanMatch> {
        let mut position = from.min(line.len());
        while position < line.len() && !line.is_char_boundary(position) {
            position += 1;
        }
        scratch.prepare(self.insts.len());
        scratch.current.clear();
        scratch.begin_generation();
        self.add_start_threads(line, position, ctx, scratch, List::Current);

        loop {
            if let Some(found) = first_accept(&self.insts, &scratch.current) {
                return Some(found);
            }

            scratch.next.clear();
            scratch.accepted = false;
            scratch.begin_generation();
            let next_position = match line.as_bytes().get(position).copied() {
                Some(byte) if byte < 0x80 => Some(position + 1),
                Some(_) => char_at(line, position).map(|(_, end)| end),
                None => None,
            };
            let mut index = 0;
            while index < scratch.current.len() {
                let thread = scratch.current[index];
                match &self.insts[thread.pc] {
                    Inst::Char { ch, flags, next } => {
                        let matched = match line.as_bytes().get(position).copied() {
                            Some(byte) if byte < 0x80 => {
                                char_matches(*ch, byte as char, *flags).then_some(position + 1)
                            }
                            Some(_) => char_at(line, position).and_then(|(input, end)| {
                                char_matches(*ch, input, *flags).then_some(end)
                            }),
                            None => None,
                        };
                        if let Some(end) = matched {
                            add_thread(
                                self,
                                *next,
                                thread.start,
                                end,
                                line,
                                ctx,
                                scratch,
                                List::Next,
                            );
                        }
                    }
                    Inst::Class { class, flags, next } => {
                        let class = &self.classes[*class];
                        let matched = match line.as_bytes().get(position).copied() {
                            Some(byte) if byte < 0x80 => class
                                .matches_ascii(byte, flags.case_insensitive)
                                .then_some(position + 1),
                            Some(_) => char_at(line, position).and_then(|(ch, end)| {
                                class_contains(&class.source, ch, *flags).then_some(end)
                            }),
                            None => None,
                        };
                        if let Some(end) = matched {
                            add_thread(
                                self,
                                *next,
                                thread.start,
                                end,
                                line,
                                ctx,
                                scratch,
                                List::Next,
                            );
                        }
                    }
                    Inst::Any { flags, next } => {
                        let matched = match line.as_bytes().get(position).copied() {
                            Some(byte) if byte < 0x80 => (byte != b'\n'
                                || flags.dot_matches_new_line)
                                .then_some(position + 1),
                            Some(_) => char_at(line, position).and_then(|(ch, end)| {
                                (ch != '\n' || flags.dot_matches_new_line).then_some(end)
                            }),
                            None => None,
                        };
                        if let Some(end) = matched {
                            add_thread(
                                self,
                                *next,
                                thread.start,
                                end,
                                line,
                                ctx,
                                scratch,
                                List::Next,
                            );
                        }
                    }
                    Inst::Accept { .. } => {
                        // Keep the fallback match, and discard everything of
                        // lower ordered priority.
                        add_thread(
                            self,
                            thread.pc,
                            thread.start,
                            thread.end,
                            line,
                            ctx,
                            scratch,
                            List::Next,
                        );
                        break;
                    }
                    Inst::Anchor { .. } | Inst::Split { .. } | Inst::Dispatch { .. } => {
                        unreachable!("epsilon instruction in thread list")
                    }
                }
                index += 1;
            }

            let Some(next_position) = next_position else {
                return first_accept(&self.insts, &scratch.next);
            };
            position = next_position;
            // Existing threads have priority over starts injected later. Once
            // an `Accept` is queued, the result starts no later than it, and
            // every later start ranks below it, so new starts cannot win.
            if !scratch.accepted {
                self.add_start_threads(line, position, ctx, scratch, List::Next);
            }
            std::mem::swap(&mut scratch.current, &mut scratch.next);
        }
    }

    fn add_start_threads(
        &self,
        line: &str,
        position: usize,
        ctx: AnchorContext,
        scratch: &mut ScannerScratch,
        list: List,
    ) {
        for &entry in self.starts.targets(line.as_bytes().get(position).copied()) {
            add_thread(
                self,
                self.entries[entry as usize],
                position,
                position,
                line,
                ctx,
                scratch,
                list,
            );
        }
    }
}

/// Whether the scanner's winning end equals the backtracking end for this
/// pattern, so an exact replay can be skipped when no captures are needed.
///
/// With no captures, lookaround, backreferences, or atomic groups, a
/// priority-ordered Thompson simulation that keeps the first thread per
/// instruction and position reproduces the backtracking priority order: a
/// discarded duplicate has exactly the same future as the higher-priority
/// thread that was kept. Backtracking's empty-iteration check for loops
/// whose body can match empty depends on history instead, so any repeat of a
/// nullable body is excluded. Case-insensitive patterns are excluded as well
/// to keep fold handling on the authoritative path.
pub(crate) fn match_end_is_exact(parsed: &ParsedRegex) -> bool {
    fn nullable(ast: &Ast) -> bool {
        match ast {
            Ast::Empty | Ast::Anchor(_) => true,
            Ast::Literal(value) => value.is_empty(),
            Ast::Concat(nodes) => nodes.iter().all(nullable),
            Ast::Alternation(branches) => branches.iter().any(nullable),
            Ast::Repeat { node, min, .. } => *min == 0 || nullable(node),
            Ast::Group { child, .. } | Ast::Flags { child, .. } => nullable(child),
            // Consuming or unsupported; exactness rejects the latter below.
            _ => false,
        }
    }
    fn exact(ast: &Ast) -> bool {
        match ast {
            Ast::Empty | Ast::Literal(_) | Ast::Dot | Ast::Class(_) | Ast::Anchor(_) => true,
            Ast::Concat(nodes) | Ast::Alternation(nodes) => nodes.iter().all(exact),
            Ast::Repeat {
                node, possessive, ..
            } => !*possessive && !nullable(node) && exact(node),
            Ast::Group { child, .. } => exact(child),
            Ast::Flags { flags, child } => !flags.case_insensitive && exact(child),
            Ast::Look { .. }
            | Ast::Backref(_)
            | Ast::Conditional { .. }
            | Ast::Subroutine(_)
            | Ast::Grapheme
            | Ast::Unsupported(_) => false,
        }
    }
    Scanner::supports(parsed) && !parsed.flags.case_insensitive && exact(&parsed.ast)
}

fn ast_is_supported(ast: &Ast) -> bool {
    match ast {
        Ast::Empty | Ast::Literal(_) | Ast::Dot | Ast::Class(_) | Ast::Anchor(_) => true,
        Ast::Concat(nodes) | Ast::Alternation(nodes) => nodes.iter().all(ast_is_supported),
        Ast::Repeat {
            node,
            min,
            max,
            possessive,
            ..
        } => !*possessive && max.is_none_or(|max| max >= *min) && ast_is_supported(node),
        Ast::Group { child, .. } | Ast::Flags { child, .. } => ast_is_supported(child),
        Ast::Look { .. }
        | Ast::Backref(_)
        | Ast::Conditional { .. }
        | Ast::Subroutine(_)
        | Ast::Grapheme
        | Ast::Unsupported(_) => false,
    }
}

fn first_accept(insts: &[Inst], threads: &[Thread]) -> Option<ScanMatch> {
    let thread = *threads.first()?;
    let Inst::Accept { pattern } = insts[thread.pc] else {
        return None;
    };
    Some(ScanMatch {
        pattern,
        start: thread.start,
        end: thread.end,
    })
}

fn char_matches(expected: char, actual: char, flags: RegexFlags) -> bool {
    if expected == actual {
        return true;
    }
    if !flags.case_insensitive {
        return false;
    }
    if expected.is_ascii() && actual.is_ascii() {
        expected.eq_ignore_ascii_case(&actual)
    } else {
        super::backtrack::unicode_case_eq(expected, actual)
    }
}

#[derive(Clone, Copy)]
enum List {
    Current,
    Next,
}

#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn add_thread(
    scanner: &Scanner,
    pc: usize,
    start: usize,
    position: usize,
    line: &str,
    ctx: AnchorContext,
    scratch: &mut ScannerScratch,
    list: List,
) {
    if scratch.seen[pc] == scratch.generation {
        return;
    }
    let initial = Thread {
        pc,
        start,
        end: position,
    };
    if !matches!(
        scanner.insts[pc],
        Inst::Split { .. } | Inst::Anchor { .. } | Inst::Dispatch { .. }
    ) {
        // Most transitions lead directly to a consuming instruction. Avoid
        // clearing, pushing, and popping the epsilon-work stack for that
        // overwhelmingly common one-state closure.
        scratch.seen[pc] = scratch.generation;
        scratch.accepted |= matches!(scanner.insts[pc], Inst::Accept { .. });
        match list {
            List::Current => scratch.current.push(initial),
            List::Next => scratch.next.push(initial),
        }
        return;
    }

    add_epsilon_threads(scanner, initial, position, line, ctx, scratch, list);
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn add_epsilon_threads(
    scanner: &Scanner,
    initial: Thread,
    position: usize,
    line: &str,
    ctx: AnchorContext,
    scratch: &mut ScannerScratch,
    list: List,
) {
    debug_assert!(scratch.work.is_empty());
    scratch.work.push(initial.pc);
    while let Some(pc) = scratch.work.pop() {
        if scratch.seen[pc] == scratch.generation {
            continue;
        }
        scratch.seen[pc] = scratch.generation;
        match scanner.insts[pc] {
            Inst::Split {
                preferred,
                alternate,
            } => {
                // LIFO: push the lower-priority edge first.
                scratch.work.push(alternate);
                scratch.work.push(preferred);
            }
            Inst::Dispatch { table } => {
                let targets = scanner.dispatches[table]
                    .table
                    .targets(line.as_bytes().get(position).copied());
                // LIFO: push in reverse so the first branch is explored first,
                // exactly as the equivalent `Split` chain would.
                scratch
                    .work
                    .extend(targets.iter().rev().map(|&target| target as usize));
            }
            Inst::Anchor { kind, next } => {
                if anchor_matches(kind, line, position, ctx) {
                    scratch.work.push(next);
                }
            }
            ref inst => {
                scratch.accepted |= matches!(inst, Inst::Accept { .. });
                let thread = Thread { pc, ..initial };
                match list {
                    List::Current => scratch.current.push(thread),
                    List::Next => scratch.next.push(thread),
                }
            }
        }
    }
}

impl ScannerScratch {
    fn prepare(&mut self, states: usize) {
        self.seen.resize(states, 0);
        self.current
            .reserve(states.saturating_sub(self.current.capacity()));
        self.next
            .reserve(states.saturating_sub(self.next.capacity()));
        self.work
            .reserve(states.saturating_sub(self.work.capacity()));
    }

    fn begin_generation(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.seen.fill(0);
            self.generation = 1;
        }
    }
}

#[derive(Default)]
struct Compiler {
    insts: Vec<Inst>,
    classes: Vec<CompiledClass>,
    dispatches: Vec<Dispatch>,
    entries: Vec<CompilerEntry>,
    visit_stamps: Vec<u32>,
    visit_generation: u32,
}

/// Conservative set of first bytes a closure can consume, and whether it can
/// reach `Accept` (or an unresolved target) without consuming.
struct FirstBytes {
    bytes: [u64; 4],
    nullable: bool,
}

impl FirstBytes {
    fn insert(&mut self, byte: u8) {
        self.bytes[byte as usize >> 6] |= 1u64 << (byte & 63);
    }

    fn insert_non_ascii(&mut self) {
        self.bytes[2] = u64::MAX;
        self.bytes[3] = u64::MAX;
    }

    fn contains(&self, byte: u8) -> bool {
        self.bytes[byte as usize >> 6] & (1u64 << (byte & 63)) != 0
    }
}

impl Compiler {
    fn finish(mut self) -> Scanner {
        let items = (0..self.entries.len())
            .map(|index| {
                let mut first = self.first_bytes(self.entries[index].pc);
                if let Some(hint) = self.entries[index].start_bitmap {
                    // Hints come from non-nullable patterns and were the only
                    // bytes at which such an entry started; keep that rule.
                    if first.nullable {
                        first.bytes = [u64::MAX; 4];
                        first.nullable = false;
                    }
                    for (bits, hint) in first.bytes.iter_mut().zip(hint) {
                        *bits &= hint;
                    }
                }
                (index as u32, first)
            })
            .collect::<Vec<_>>();
        let (starts, _) = ByteTable::build(&items);
        let entries = self.entries.into_iter().map(|entry| entry.pc).collect();
        Scanner {
            insts: self.insts,
            classes: self.classes,
            dispatches: self.dispatches,
            entries,
            starts,
        }
    }

    /// First bytes of the epsilon closure rooted at `pc`, matching the
    /// runtime consume checks in [`Scanner::find`]. Anchors are treated as
    /// always passing, which only widens the set.
    fn first_bytes(&mut self, pc: usize) -> FirstBytes {
        let mut out = FirstBytes {
            bytes: [0; 4],
            nullable: false,
        };
        if self.visit_stamps.len() < self.insts.len() {
            self.visit_stamps.resize(self.insts.len(), 0);
        }
        self.visit_generation = self.visit_generation.wrapping_add(1);
        if self.visit_generation == 0 {
            self.visit_stamps.fill(0);
            self.visit_generation = 1;
        }
        let generation = self.visit_generation;
        let mut work = vec![pc];
        while let Some(pc) = work.pop() {
            if pc == NO_TARGET {
                // A loop placeholder that has not been patched yet.
                out.bytes = [u64::MAX; 4];
                out.nullable = true;
                continue;
            }
            if self.visit_stamps[pc] == generation {
                continue;
            }
            self.visit_stamps[pc] = generation;
            match &self.insts[pc] {
                Inst::Char { ch, flags, .. } => {
                    if flags.case_insensitive {
                        if ch.is_ascii() {
                            out.insert(ch.to_ascii_lowercase() as u8);
                            out.insert(ch.to_ascii_uppercase() as u8);
                            // Unicode folds (for example U+017F for `s`).
                            out.insert_non_ascii();
                        } else {
                            out.bytes = [u64::MAX; 4];
                        }
                    } else {
                        let mut buffer = [0u8; 4];
                        out.insert(ch.encode_utf8(&mut buffer).as_bytes()[0]);
                    }
                }
                Inst::Class { class, flags, .. } => {
                    let class = &self.classes[*class];
                    for byte in 0u8..0x80 {
                        if class.matches_ascii(byte, flags.case_insensitive) {
                            out.insert(byte);
                        }
                    }
                    out.insert_non_ascii();
                }
                Inst::Any { .. } => out.bytes = [u64::MAX; 4],
                Inst::Anchor { next, .. } => work.push(*next),
                Inst::Split {
                    preferred,
                    alternate,
                } => {
                    work.push(*alternate);
                    work.push(*preferred);
                }
                Inst::Dispatch { table } => {
                    work.extend(
                        self.dispatches[*table]
                            .branches
                            .iter()
                            .map(|&branch| branch as usize),
                    );
                }
                Inst::Accept { .. } => out.nullable = true,
            }
        }
        out
    }

    /// Builds a dispatch for `branches` (entry pcs in priority order), or
    /// returns `None` when no input byte would prune any branch.
    fn dispatch(&mut self, branches: &[usize]) -> Option<Dispatch> {
        let items = branches
            .iter()
            .map(|&branch| Some((u32::try_from(branch).ok()?, self.first_bytes(branch))))
            .collect::<Option<Vec<_>>>()?;
        let (table, pruned) = ByteTable::build(&items);
        pruned.then(|| Dispatch {
            table,
            branches: items.iter().map(|(branch, _)| *branch).collect(),
        })
    }

    fn try_add(
        &mut self,
        pattern: usize,
        parsed: &ParsedRegex,
        start_bytes: Option<&[u8]>,
    ) -> Result<(), CompileFailure> {
        let inst_checkpoint = self.insts.len();
        let class_checkpoint = self.classes.len();
        let dispatch_checkpoint = self.dispatches.len();
        let result = (|| {
            if parsed.features.possessive_or_atomic {
                return Err(CompileError::Possessive);
            }
            let accept = self.push(Inst::Accept { pattern })?;
            let entry = self.node(&parsed.ast, parsed.flags, accept)?;
            self.entries.push(CompilerEntry {
                pc: entry,
                start_bitmap: start_bytes.map(|bytes| {
                    let mut bitmap = [0u64; 4];
                    for byte in bytes {
                        bitmap[*byte as usize >> 6] |= 1u64 << (*byte & 63);
                    }
                    bitmap
                }),
            });
            Ok(())
        })();
        if let Err(error) = result {
            self.insts.truncate(inst_checkpoint);
            self.classes.truncate(class_checkpoint);
            self.dispatches.truncate(dispatch_checkpoint);
            return Err(CompileFailure { pattern, error });
        }
        Ok(())
    }

    fn push(&mut self, inst: Inst) -> Result<usize, CompileError> {
        if self.insts.len() >= MAX_STATES {
            return Err(CompileError::TooLarge);
        }
        let pc = self.insts.len();
        self.insts.push(inst);
        Ok(pc)
    }

    fn node(&mut self, ast: &Ast, flags: RegexFlags, next: usize) -> Result<usize, CompileError> {
        match ast {
            Ast::Empty => Ok(next),
            Ast::Literal(value) => {
                let mut entry = next;
                for ch in value.chars().rev() {
                    entry = self.push(Inst::Char {
                        ch,
                        flags,
                        next: entry,
                    })?;
                }
                Ok(entry)
            }
            Ast::Dot => self.push(Inst::Any { flags, next }),
            Ast::Class(class) => {
                let id = self.classes.len();
                self.classes.push(CompiledClass::new(class.clone()));
                self.push(Inst::Class {
                    class: id,
                    flags,
                    next,
                })
            }
            Ast::Anchor(kind) => self.push(Inst::Anchor { kind: *kind, next }),
            Ast::Concat(nodes) => {
                let mut entry = next;
                for node in nodes.iter().rev() {
                    entry = self.node(node, flags, entry)?;
                }
                Ok(entry)
            }
            Ast::Alternation(branches) if branches.len() >= DISPATCH_MIN_BRANCHES => {
                let mut entries = Vec::with_capacity(branches.len());
                for branch in branches.iter().rev() {
                    entries.push(self.node(branch, flags, next)?);
                }
                entries.reverse();
                if let Some(dispatch) = self.dispatch(&entries) {
                    let table = self.dispatches.len();
                    self.dispatches.push(dispatch);
                    return self.push(Inst::Dispatch { table });
                }
                let (&last, rest) = entries.split_last().expect("alternation has branches");
                let mut entry = last;
                for &preferred in rest.iter().rev() {
                    entry = self.push(Inst::Split {
                        preferred,
                        alternate: entry,
                    })?;
                }
                Ok(entry)
            }
            Ast::Alternation(branches) => {
                let Some((last, rest)) = branches.split_last() else {
                    return Ok(next);
                };
                let mut entry = self.node(last, flags, next)?;
                for branch in rest.iter().rev() {
                    let preferred = self.node(branch, flags, next)?;
                    entry = self.push(Inst::Split {
                        preferred,
                        alternate: entry,
                    })?;
                }
                Ok(entry)
            }
            Ast::Repeat {
                node,
                min,
                max,
                greedy,
                possessive,
                ..
            } => {
                if *possessive {
                    return Err(CompileError::Possessive);
                }
                if max.is_some_and(|max| max < *min) {
                    return Err(CompileError::InvalidRepeat);
                }
                let optional = max.map(|max| max - min);
                let mut entry = next;
                match optional {
                    None => {
                        let split = self.push(Inst::Split {
                            preferred: NO_TARGET,
                            alternate: NO_TARGET,
                        })?;
                        let body = self.node(node, flags, split)?;
                        self.insts[split] = if *greedy {
                            Inst::Split {
                                preferred: body,
                                alternate: entry,
                            }
                        } else {
                            Inst::Split {
                                preferred: entry,
                                alternate: body,
                            }
                        };
                        entry = split;
                    }
                    Some(count) => {
                        for _ in 0..count {
                            let body = self.node(node, flags, entry)?;
                            entry = if *greedy {
                                self.push(Inst::Split {
                                    preferred: body,
                                    alternate: entry,
                                })?
                            } else {
                                self.push(Inst::Split {
                                    preferred: entry,
                                    alternate: body,
                                })?
                            };
                        }
                    }
                }
                for _ in 0..*min {
                    entry = self.node(node, flags, entry)?;
                }
                Ok(entry)
            }
            Ast::Group { child, .. } => self.node(child, flags, next),
            Ast::Flags { flags, child } => self.node(child, *flags, next),
            Ast::Look { .. } => Err(CompileError::Lookaround),
            Ast::Backref(_) => Err(CompileError::Backreference),
            Ast::Conditional { .. } => Err(CompileError::Backreference),
            Ast::Subroutine(_) => Err(CompileError::Subroutine),
            Ast::Grapheme | Ast::Unsupported(_) => Err(CompileError::Unsupported),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::regex::{AnchorContext, FallbackMatcher, Matcher, ast::parse};

    fn scanner(patterns: &[&str]) -> Scanner {
        let parsed: Vec<_> = patterns.iter().map(|pattern| parse(pattern)).collect();
        Scanner::compile(parsed.iter()).unwrap()
    }

    fn find(patterns: &[&str], text: &str) -> Option<ScanMatch> {
        scanner(patterns).find(
            text,
            0,
            AnchorContext::line_start(),
            &mut ScannerScratch::default(),
        )
    }

    #[test]
    fn leftmost_start_then_pattern_order() {
        assert_eq!(
            find(&["z", "bc", "b"], "abc z"),
            Some(ScanMatch {
                pattern: 1,
                start: 1,
                end: 3
            })
        );
        assert_eq!(
            find(&["b", "bc"], "abc"),
            Some(ScanMatch {
                pattern: 0,
                start: 1,
                end: 2
            })
        );
        assert_eq!(
            find(&["bc", "b"], "abc"),
            Some(ScanMatch {
                pattern: 0,
                start: 1,
                end: 3
            })
        );
    }

    #[test]
    fn start_byte_buckets_preserve_mixed_entry_priority() {
        let preferred = parse("bc");
        let fallback = parse("b");
        let x = parse("x");
        let b = *b"b";
        let x_byte = *b"x";
        let nfa = Scanner::compile_with_hints([
            (9, &preferred, Some(b.as_slice())),
            (11, &fallback, None),
            (17, &x, Some(x_byte.as_slice())),
        ])
        .unwrap();
        let mut scratch = ScannerScratch::default();

        assert_eq!(
            nfa.find("abc", 0, AnchorContext::line_start(), &mut scratch),
            Some(ScanMatch {
                pattern: 9,
                start: 1,
                end: 3,
            })
        );
        assert_eq!(
            nfa.find("a x", 0, AnchorContext::line_start(), &mut scratch)
                .unwrap()
                .pattern,
            17
        );
    }

    #[test]
    fn ordered_alternation_and_repeat_priority() {
        assert_eq!(find(&["a|ab"], "ab").unwrap().end, 1);
        assert_eq!(find(&["ab|a"], "ab").unwrap().end, 2);
        assert_eq!(find(&["a*"], "aaa").unwrap().end, 3);
        assert_eq!(find(&["a*?"], "aaa").unwrap().end, 0);
        assert_eq!(find(&["a{2,4}"], "aaaaa").unwrap().end, 4);
        assert_eq!(find(&["a{2,4}?"], "aaaaa").unwrap().end, 2);
        assert_eq!(find(&["a.*b", "a"], "axbyb").unwrap().end, 5);
        assert_eq!(find(&["a.*?b", "a.*b"], "axbyb").unwrap().end, 3);
        assert_eq!(find(&["a.*b", "a.*?b"], "axbyb").unwrap().end, 5);
    }

    #[test]
    fn empty_zero_width_and_nullable_loop() {
        assert_eq!(
            find(&["", "a"], "a").unwrap(),
            ScanMatch {
                pattern: 0,
                start: 0,
                end: 0
            }
        );
        assert_eq!(find(&["(?:a?)*b"], "aaab").unwrap().end, 4);
        assert_eq!(find(&["^$"], "").unwrap().end, 0);
        assert_eq!(find(&[r"\bword\b"], "a word!").unwrap().start, 2);
    }

    #[test]
    fn case_insensitive_literals_fold_non_ascii_scalars() {
        // Regression: the scanner used ASCII-only folding, so a non-ASCII
        // `(?i)` literal in a multi-pattern set never selected its candidate
        // even though the authoritative engines match it.
        for text in ["CAFÉ x", "café x", "CafÉ x"] {
            let selected = find(&[r"\b(?:for|while)\b", "(?i:café)"], text)
                .unwrap_or_else(|| panic!("scanner must select (?i:café) in {text:?}"));
            assert_eq!(selected.pattern, 1);
            assert_eq!(selected.start, 0);
            // "CAFÉ" is five bytes; É encodes as two.
            assert_eq!(selected.end, 5);

            let reference = FallbackMatcher::new("(?i:café)")
                .find(text, 0, AnchorContext::line_start())
                .expect("fallback engine matches");
            assert_eq!(
                selected.start..selected.end,
                reference.start..reference.end,
                "scanner must agree with the fallback engine on {text:?}"
            );
        }
    }

    #[test]
    fn ascii_case_insensitive_literals_still_fold() {
        let selected = find(&[r"\b(?:for|while)\b", "(?i:cafe)"], "CAFE x").unwrap();
        assert_eq!(selected.pattern, 1);
        assert_eq!(selected.start..selected.end, 0..4);
    }

    #[test]
    fn unicode_classes_dot_and_flags() {
        assert_eq!(
            find(&["é+[[:digit:]]"], "xéé7").unwrap(),
            ScanMatch {
                pattern: 0,
                start: 1,
                end: 6
            }
        );
        assert_eq!(find(&["(?i:rust)"], "xxRuSt").unwrap().start, 2);
        assert!(find(&["a.b"], "a\nb").is_none());
        assert_eq!(find(&["(?s:a.b)"], "a\nb").unwrap().end, 3);
    }

    #[test]
    fn honors_search_offset_and_anchor_context() {
        let nfa = scanner(&[r"\Afoo", r"\Gfoo", "foo"]);
        let mut scratch = ScannerScratch::default();
        assert_eq!(
            nfa.find("xfoo", 1, AnchorContext::continuation(1), &mut scratch)
                .unwrap()
                .pattern,
            1
        );
        assert_eq!(
            nfa.find("foo", 0, AnchorContext::start_of_file(), &mut scratch)
                .unwrap()
                .pattern,
            0
        );
        assert_eq!(
            nfa.find("éfoo", 1, AnchorContext::line_start(), &mut scratch)
                .unwrap()
                .start,
            2
        );
        let continuation = scanner(&[r"\Gfoo"]);
        assert_eq!(
            continuation
                .find("foo xxfoo", 0, AnchorContext::continuation(6), &mut scratch)
                .unwrap()
                .start,
            6
        );
    }

    #[test]
    fn rejects_non_regular_and_possessive_constructs() {
        let cases = [
            ("(?=a)", CompileError::Lookaround),
            (r"(a)\1", CompileError::Backreference),
            (r"(a)\g<1>", CompileError::Subroutine),
            ("a++", CompileError::Possessive),
            ("(?>a)", CompileError::Possessive),
            ("(?q:a)", CompileError::Unsupported),
        ];
        for (pattern, expected) in cases {
            let parsed = parse(pattern);
            assert_eq!(
                Scanner::compile([&parsed]).unwrap_err().error,
                expected,
                "{pattern}"
            );
        }
    }

    /// Leftmost start, then pattern order, using the fallback interpreter at
    /// every exact start as the semantic oracle.
    fn reference_set_find(patterns: &[&str], text: &str, from: usize) -> Option<ScanMatch> {
        let oracles = patterns
            .iter()
            .map(|pattern| FallbackMatcher::new(pattern))
            .collect::<Vec<_>>();
        text.char_indices()
            .map(|(start, _)| start)
            .chain(std::iter::once(text.len()))
            .filter(|start| *start >= from)
            .find_map(|start| {
                oracles.iter().enumerate().find_map(|(pattern, oracle)| {
                    oracle
                        .try_find_at(text, start, AnchorContext::line_start())
                        .unwrap()
                        .result
                        .map(|found| ScanMatch {
                            pattern,
                            start: found.start,
                            end: found.end,
                        })
                })
            })
    }

    fn assert_matches_reference(patterns: &[&str], texts: &[&str]) {
        let nfa = scanner(patterns);
        let mut scratch = ScannerScratch::default();
        for text in texts {
            for from in text
                .char_indices()
                .map(|(start, _)| start)
                .chain(std::iter::once(text.len()))
            {
                assert_eq!(
                    nfa.find(text, from, AnchorContext::line_start(), &mut scratch),
                    reference_set_find(patterns, text, from),
                    "patterns={patterns:?} text={text:?} from={from}"
                );
            }
        }
    }

    #[test]
    fn large_alternation_dispatch_preserves_branch_priority() {
        // Eight or more branches compile to a first-byte dispatch. Shared
        // prefixes, a `.` inside a branch, and trailing boundaries make branch
        // order observable.
        let keywords = r"\b(co|coroutine|coroutine.create|string|string.sub|select|s|sub|table)\b";
        assert_matches_reference(
            &[keywords, r"\w+"],
            &[
                "coroutine.create(x)",
                "string.sub s sub",
                "co coroutine coroutineXcreate",
                "tables table",
                "",
            ],
        );
        let nfa = scanner(&[keywords]);
        assert!(!nfa.dispatches.is_empty(), "keyword list uses a dispatch");
    }

    #[test]
    fn dispatch_keeps_nullable_anchor_and_looping_branches() {
        assert_matches_reference(
            &[
                "x(?:a|b|c|d|e|f|g|)y",
                "(?:^a|b$|\\bc|d|e|f|g|h)+",
                "(?:(?:aa|bb|cc|dd|ee|ff|gg|hh)x)*z",
                "(?:q|r|s|t|u|v|w|(?:z*))!",
            ],
            &["xy xgy", "a b c", "aaxbbxz", "hhxgz", "zzz! !", "q!", "b"],
        );
    }

    #[test]
    fn dispatch_keeps_case_folds_across_utf8_widths() {
        // U+212A KELVIN SIGN folds to `k` and U+017F LONG S folds to `s`:
        // an ASCII-only first-byte set would drop those branches.
        let folded = "(?i:a|b|c|d|e|k|s|é)";
        assert_matches_reference(&[folded], &["\u{212a}", "x\u{17f}", "É", "é", "K S", "zzz"]);
        let nfa = scanner(&[folded]);
        let mut scratch = ScannerScratch::default();
        let found = nfa
            .find("\u{212a}", 0, AnchorContext::line_start(), &mut scratch)
            .expect("kelvin sign folds to k");
        assert_eq!(found.start..found.end, 0..3);
        let found = nfa
            .find("\u{17f}", 0, AnchorContext::line_start(), &mut scratch)
            .expect("long s folds to s");
        assert_eq!(found.start..found.end, 0..2);
    }

    #[test]
    fn start_tables_preserve_entry_priority_and_line_end() {
        // Word classes, literal starts, nullable patterns, and end-of-line
        // anchors all start through the byte table.
        assert_matches_reference(
            &[
                r"\w+:\w+",
                r"\w+(?:\.\w+)+",
                r"[#%*+]|\?\.|:",
                "$",
                r"\w+",
                "x*",
            ],
            &["math.max a:b", ":k +1", "é.ü", "", "  "],
        );
        assert_matches_reference(&["$", "a"], &["", "ba", "b"]);
        assert_matches_reference(&["a", "", "b"], &["ba", ""]);
    }

    #[test]
    fn later_starts_do_not_outrank_queued_accept() {
        // Greedy loops keep a higher-priority consuming thread ahead of their
        // queued accept; no later start may win meanwhile.
        assert_matches_reference(&["a+b", "a+", "b", "ab"], &["aaac", "aaab", "caab", "b"]);
        assert_matches_reference(&["x.*?y", "y"], &["x y y", "yy"]);
    }

    #[test]
    fn exact_end_excludes_history_dependent_and_folded_patterns() {
        for (pattern, exact) in [
            (r"\w+", true),
            (r"\w+(?:\.\w+)+", true),
            (r"\b(?:co|coroutine|coroutine.create)\b", true),
            ("a{2,3}?b*", true),
            ("(?:ab|c)+d?", true),
            ("(?:a?)*b", false),
            ("(?:a|)+", false),
            ("(?:x|y?){2}", false),
            ("(?:$|a)*", false),
            ("(?i)abc", false),
            ("a(?i:b)", false),
            ("(?=a)a", false),
            (r"(a)\1", false),
            ("a++", false),
        ] {
            assert_eq!(match_end_is_exact(&parse(pattern)), exact, "{pattern}");
        }
    }

    #[test]
    fn exact_patterns_agree_with_backtracking_end() {
        let patterns = [
            r"\w+(?:\.\w+)+",
            r"\w+:\w+",
            r"-?\d+(?:\.\d+)?(?:[Ee][-+]?\d+)?",
            r"\b(?:co|coroutine|coroutine.create|s|sub|string|string.sub|table)\b",
            "a{2,4}?b|a+?",
            "(?:ab|a)(?:bc|b)c",
            r"[^ ]+",
            r"\w+",
        ];
        for pattern in patterns {
            assert!(match_end_is_exact(&parse(pattern)), "{pattern}");
        }
        assert_matches_reference(
            &patterns,
            &[
                "coroutine.create math.max a:b",
                "-1.5e+3 12 x.y.z",
                "aaab aab abcc abbc",
                "string.sub(s) é.ü",
            ],
        );
        for pattern in patterns {
            assert_matches_reference(&[pattern], &["coroutine.create -1.5e+3 aaab abcc é:x"]);
        }
    }

    #[test]
    fn differential_single_pattern_regular_subset() {
        let patterns = [
            "",
            "abc",
            "a|ab",
            "ab|a",
            "a*",
            "a+?",
            "a{1,3}b",
            "(?:ab|c)+d?",
            r"[a-zA-Z0-9_]+",
            r"[^a-z]+",
            r"^foo$",
            r"\bcat\b",
            "(?i:hello)",
            "é+",
            ".*x",
        ];
        let texts = [
            "",
            "abc",
            "zab",
            "aaaaab",
            "ccabd",
            "CAT cat",
            "hello HELLO",
            "ééx",
            "foo\n",
            "123!x",
        ];
        for pattern in patterns {
            let nfa = scanner(&[pattern]);
            // The fallback interpreter shares the AST's Unicode class
            // semantics and is therefore the direct semantic oracle here.
            let oracle = FallbackMatcher::new(pattern);
            for text in texts {
                let got = nfa.find(
                    text,
                    0,
                    AnchorContext::line_start(),
                    &mut ScannerScratch::default(),
                );
                // Probe exact starts to compare against its AST interpreter
                // rather than its whole-line search loop.
                let expected = text
                    .char_indices()
                    .map(|(start, _)| start)
                    .chain(std::iter::once(text.len()))
                    .find_map(|start| {
                        oracle
                            .try_find_at(text, start, AnchorContext::line_start())
                            .unwrap()
                            .result
                    });
                assert_eq!(
                    got.map(|m| (m.start, m.end)),
                    expected.map(|m| (m.start, m.end)),
                    "pattern={pattern:?} text={text:?}"
                );
            }
        }
    }

    #[test]
    fn scratch_capacities_stabilize_after_warmup() {
        let nfa = scanner(&["(?:alpha|alphabet|β+)*z", r"\w{2,8}", "x"]);
        let mut scratch = ScannerScratch::default();
        let _ = nfa.find(
            "alphabetββalphabetz",
            0,
            AnchorContext::default(),
            &mut scratch,
        );
        let capacities = (
            scratch.current.capacity(),
            scratch.next.capacity(),
            scratch.work.capacity(),
            scratch.seen.capacity(),
        );
        for _ in 0..20 {
            let _ = nfa.find(
                "nothing alphabetβz here",
                0,
                AnchorContext::default(),
                &mut scratch,
            );
        }
        assert_eq!(
            capacities,
            (
                scratch.current.capacity(),
                scratch.next.capacity(),
                scratch.work.capacity(),
                scratch.seen.capacity()
            )
        );
    }
}
