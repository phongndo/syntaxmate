//! Capture-free, ordered, multi-pattern Thompson NFA.
//!
//! The NFA finds a winner only. Capture replay deliberately belongs to the
//! caller. Threads are kept in backtracking priority order, so an `Accept`
//! thread can wait behind a preferred (for example greedy) path without
//! losing the endpoint it reached.

use super::AnchorContext;
use super::ast::{Ast, CharClass, ParsedRegex, RegexFlags};
use super::backtrack::{anchor_matches, char_at, class_contains};

const NO_TARGET: usize = usize::MAX;
const MAX_STATES: usize = 16_384;

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
    Accept {
        pattern: usize,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct Scanner {
    insts: Vec<Inst>,
    classes: Vec<CharClass>,
    entries: Vec<usize>,
    starts: ScannerStarts,
}

#[derive(Debug, Clone)]
struct CompilerEntry {
    pc: usize,
    start_bitmap: Option<[u64; 4]>,
}

#[derive(Debug, Clone)]
struct ScannerStarts {
    unrestricted: Box<[u32]>,
    offsets: [u32; 257],
    restricted: Box<[u32]>,
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
}

impl Scanner {
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
            scratch.begin_generation();
            let next_position = char_at(line, position).map(|(_, end)| end);
            let mut index = 0;
            while index < scratch.current.len() {
                let thread = scratch.current[index];
                match &self.insts[thread.pc] {
                    Inst::Char { ch, flags, next } => {
                        if let Some((input, end)) = char_at(line, position)
                            && char_matches(*ch, input, *flags)
                        {
                            add_thread(
                                &self.insts,
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
                        if let Some((ch, end)) = char_at(line, position)
                            && class_contains(&self.classes[*class], ch, *flags)
                        {
                            add_thread(
                                &self.insts,
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
                        if let Some((ch, end)) = char_at(line, position)
                            && (ch != '\n' || flags.dot_matches_new_line)
                        {
                            add_thread(
                                &self.insts,
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
                            &self.insts,
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
                    Inst::Anchor { .. } | Inst::Split { .. } => {
                        unreachable!("epsilon instruction in thread list")
                    }
                }
                index += 1;
            }

            let Some(next_position) = next_position else {
                return first_accept(&self.insts, &scratch.next);
            };
            position = next_position;
            // Existing threads have priority over starts injected later.
            self.add_start_threads(line, position, ctx, scratch, List::Next);
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
        let restricted = line
            .as_bytes()
            .get(position)
            .map_or(&[][..], |byte| self.starts.restricted(*byte));
        let mut unrestricted_index = 0usize;
        let mut restricted_index = 0usize;
        while unrestricted_index < self.starts.unrestricted.len()
            || restricted_index < restricted.len()
        {
            let unrestricted = self.starts.unrestricted.get(unrestricted_index).copied();
            let restricted_entry = restricted.get(restricted_index).copied();
            let entry = match (unrestricted, restricted_entry) {
                (Some(left), Some(right)) if left < right => {
                    unrestricted_index += 1;
                    left
                }
                (Some(_), Some(right)) => {
                    restricted_index += 1;
                    right
                }
                (Some(left), None) => {
                    unrestricted_index += 1;
                    left
                }
                (None, Some(right)) => {
                    restricted_index += 1;
                    right
                }
                (None, None) => break,
            } as usize;
            add_thread(
                &self.insts,
                self.entries[entry],
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

impl ScannerStarts {
    fn new(entries: &[CompilerEntry]) -> Self {
        let mut unrestricted = Vec::new();
        let mut counts = [0u32; 256];
        for (index, entry) in entries.iter().enumerate() {
            let Some(bitmap) = &entry.start_bitmap else {
                unrestricted.push(u32::try_from(index).expect("scanner entry index fits in u32"));
                continue;
            };
            for byte in 0u8..=u8::MAX {
                if bitmap[byte as usize >> 6] & (1u64 << (byte & 63)) != 0 {
                    counts[byte as usize] += 1;
                }
            }
        }

        let mut offsets = [0u32; 257];
        for byte in 0..256 {
            offsets[byte + 1] = offsets[byte]
                .checked_add(counts[byte])
                .expect("scanner start offsets fit in u32");
        }
        let mut cursors: [u32; 256] = offsets[..256]
            .try_into()
            .expect("scanner offset prefix has 256 entries");
        let mut restricted = vec![0u32; offsets[256] as usize];
        for (index, entry) in entries.iter().enumerate() {
            let Some(bitmap) = &entry.start_bitmap else {
                continue;
            };
            for byte in 0u8..=u8::MAX {
                if bitmap[byte as usize >> 6] & (1u64 << (byte & 63)) == 0 {
                    continue;
                }
                let cursor = &mut cursors[byte as usize];
                restricted[*cursor as usize] =
                    u32::try_from(index).expect("scanner entry index fits in u32");
                *cursor += 1;
            }
        }
        Self {
            unrestricted: unrestricted.into_boxed_slice(),
            offsets,
            restricted: restricted.into_boxed_slice(),
        }
    }

    #[inline]
    fn restricted(&self, byte: u8) -> &[u32] {
        let index = byte as usize;
        &self.restricted[self.offsets[index] as usize..self.offsets[index + 1] as usize]
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
    if flags.case_insensitive {
        expected.eq_ignore_ascii_case(&actual)
    } else {
        expected == actual
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
    insts: &[Inst],
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
    if !matches!(insts[pc], Inst::Split { .. } | Inst::Anchor { .. }) {
        // Most transitions lead directly to a consuming instruction. Avoid
        // clearing, pushing, and popping the epsilon-work stack for that
        // overwhelmingly common one-state closure.
        scratch.seen[pc] = scratch.generation;
        match list {
            List::Current => scratch.current.push(initial),
            List::Next => scratch.next.push(initial),
        }
        return;
    }

    add_epsilon_threads(insts, initial, position, line, ctx, scratch, list);
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn add_epsilon_threads(
    insts: &[Inst],
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
        match insts[pc] {
            Inst::Split {
                preferred,
                alternate,
            } => {
                // LIFO: push the lower-priority edge first.
                scratch.work.push(alternate);
                scratch.work.push(preferred);
            }
            Inst::Anchor { kind, next } => {
                if anchor_matches(kind, line, position, ctx) {
                    scratch.work.push(next);
                }
            }
            _ => {
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
    classes: Vec<CharClass>,
    entries: Vec<CompilerEntry>,
}

impl Compiler {
    fn finish(self) -> Scanner {
        let starts = ScannerStarts::new(&self.entries);
        let entries = self.entries.into_iter().map(|entry| entry.pc).collect();
        Scanner {
            insts: self.insts,
            classes: self.classes,
            entries,
            starts,
        }
    }

    fn try_add(
        &mut self,
        pattern: usize,
        parsed: &ParsedRegex,
        start_bytes: Option<&[u8]>,
    ) -> Result<(), CompileFailure> {
        let inst_checkpoint = self.insts.len();
        let class_checkpoint = self.classes.len();
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
                self.classes.push(class.clone());
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
    use crate::engine::regex::{FallbackMatcher, ast::parse};

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
