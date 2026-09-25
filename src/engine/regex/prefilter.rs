use std::borrow::Cow;

use super::ast::{Ast, CharClass, ClassAtom, LookKind, ParsedRegex};

/// Literals one of which every match must contain. Literals borrow from the
/// AST where possible; only the ones a prefilter retains are copied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequiredLiterals<'a> {
    None,
    One(Cow<'a, str>),
    Any(Vec<Cow<'a, str>>),
}

impl RequiredLiterals<'_> {
    fn is_empty(&self) -> bool {
        matches!(self, Self::None)
    }
}

/// Fast rejection prefilter. False positives are allowed; false negatives are not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prefilter {
    None,
    Byte(u8),
    ByteSet {
        bytes: Vec<u8>,
        bitmap: [u64; 4],
    },
    Literal(String),
    Any {
        /// Retained for searches without a compiled `finder`; a finder
        /// answers every query itself, so its literals are not kept.
        literals: Vec<String>,
        ascii_case_insensitive: bool,
        mixed_width_fold_mask: u8,
        finder: Option<MultiLiteralFinder>,
    },
    /// A mandatory run of byte-class items (see [`RequiredFactor`]) that must
    /// occur together with the pattern's literal prefilter.
    Factor {
        factor: RequiredFactor,
        literals: Box<Prefilter>,
    },
}

impl Prefilter {
    pub fn from_regex(parsed: &ParsedRegex) -> Self {
        parsed.prefilter().clone()
    }

    pub fn from_pattern(ast: &Ast) -> Self {
        Self::from_required(required_literals(ast), false)
    }

    pub(crate) fn from_required(
        required: RequiredLiterals<'_>,
        ascii_case_insensitive: bool,
    ) -> Self {
        if ascii_case_insensitive {
            let literals = match required {
                RequiredLiterals::None => return Self::None,
                RequiredLiterals::One(literal) => vec![literal.into_owned()],
                RequiredLiterals::Any(literals) => {
                    literals.into_iter().map(Cow::into_owned).collect()
                }
            };
            if literals.is_empty()
                || literals
                    .iter()
                    .any(|literal| literal.is_empty() || !literal.is_ascii())
            {
                return Self::None;
            }
            let mixed_width_fold_mask = literals.iter().fold(0u8, |mask, literal| {
                literal.bytes().fold(mask, |mask, byte| {
                    mask | u8::from(byte.eq_ignore_ascii_case(&b's'))
                        | (u8::from(byte.eq_ignore_ascii_case(&b'k')) << 1)
                })
            });
            return Self::Any {
                finder: MultiLiteralFinder::for_literals_ignore_ascii_case(&literals),
                literals,
                ascii_case_insensitive: true,
                mixed_width_fold_mask,
            };
        }
        match required {
            RequiredLiterals::None => Self::None,
            RequiredLiterals::One(literal) => prefilter_one(literal.into_owned()),
            RequiredLiterals::Any(literals) if literals.is_empty() => Self::None,
            RequiredLiterals::Any(literals) if literals.len() == 1 => prefilter_one(
                literals
                    .into_iter()
                    .next()
                    .expect("one literal")
                    .into_owned(),
            ),
            RequiredLiterals::Any(literals)
                if literals
                    .iter()
                    .all(|literal| literal.len() == 1 && literal.is_ascii()) =>
            {
                let bytes = literals
                    .into_iter()
                    .map(|literal| literal.as_bytes()[0])
                    .collect::<Vec<_>>();
                let mut bitmap = [0u64; 4];
                for &byte in &bytes {
                    bitmap[byte as usize >> 6] |= 1u64 << (byte & 63);
                }
                Self::ByteSet { bytes, bitmap }
            }
            RequiredLiterals::Any(literals) => {
                let finder = MultiLiteralFinder::for_literals(&literals);
                let literals = if finder.is_some() {
                    Vec::new()
                } else {
                    literals.into_iter().map(Cow::into_owned).collect()
                };
                Self::Any {
                    finder,
                    literals,
                    ascii_case_insensitive: false,
                    mixed_width_fold_mask: 0,
                }
            }
        }
    }

    pub fn may_match(&self, haystack: &str, from: usize) -> bool {
        if !haystack.is_char_boundary(from) {
            return false;
        }
        let Some(slice) = haystack.get(from..) else {
            return false;
        };
        match self {
            Self::None => true,
            Self::Byte(byte) => find_byte(slice.as_bytes(), *byte).is_some(),
            Self::ByteSet { bytes, bitmap } => {
                find_byte_set(slice.as_bytes(), bytes, bitmap).is_some()
            }
            Self::Literal(literal) => find_literal(slice, literal).is_some(),
            Self::Factor { factor, literals } => {
                literals.may_match(haystack, from) && factor.find(slice.as_bytes()).is_some()
            }
            Self::Any {
                literals,
                ascii_case_insensitive: false,
                finder,
                ..
            } => finder.as_ref().map_or_else(
                || {
                    literals
                        .iter()
                        .any(|literal| find_literal(slice, literal).is_some())
                },
                |finder| finder.find(slice.as_bytes()).is_some(),
            ),
            Self::Any {
                literals,
                ascii_case_insensitive: true,
                mixed_width_fold_mask,
                finder,
            } => {
                finder.as_ref().map_or_else(
                    || {
                        literals
                            .iter()
                            .any(|literal| contains_ignore_ascii_case(slice, literal))
                    },
                    |finder| finder.find(slice.as_bytes()).is_some(),
                ) || first_ascii_case_fold_candidate(slice, *mixed_width_fold_mask).is_some()
            }
        }
    }

    /// Position of the first viability point at or after `from`, or `None`
    /// when the required literal cannot occur again on this line. `Self::None`
    /// never filters, so it reports `from` itself.
    pub fn next_occurrence(&self, haystack: &str, from: usize) -> Option<usize> {
        if !haystack.is_char_boundary(from) {
            return None;
        }
        let slice = haystack.get(from..)?;
        match self {
            Self::None => Some(from),
            Self::Byte(byte) => find_byte(slice.as_bytes(), *byte).map(|pos| from + pos),
            Self::ByteSet { bytes, bitmap } => {
                find_byte_set(slice.as_bytes(), bytes, bitmap).map(|pos| from + pos)
            }
            Self::Literal(literal) => find_literal(slice, literal).map(|pos| from + pos),
            // Both conditions must still be satisfiable. Reporting the
            // earlier occurrence keeps the answer constant for every start up
            // to it, which scan-local cursor caching relies on.
            Self::Factor { factor, literals } => {
                let factor = factor.find(slice.as_bytes())? + from;
                if literals.is_enabled() {
                    Some(literals.next_occurrence(haystack, from)?.min(factor))
                } else {
                    Some(factor)
                }
            }
            Self::Any {
                literals,
                ascii_case_insensitive: false,
                finder,
                ..
            } => finder.as_ref().map_or_else(
                || {
                    literals
                        .iter()
                        .filter_map(|literal| find_literal(slice, literal))
                        .min()
                        .map(|pos| from + pos)
                },
                |finder| finder.find(slice.as_bytes()).map(|pos| from + pos),
            ),
            Self::Any {
                literals,
                ascii_case_insensitive: true,
                mixed_width_fold_mask,
                finder,
            } => {
                let literal = match finder {
                    Some(finder) => finder.find(slice.as_bytes()),
                    None => literals
                        .iter()
                        .filter_map(|literal| find_ignore_ascii_case(slice, literal))
                        .min(),
                };
                literal
                    .into_iter()
                    .chain(first_ascii_case_fold_candidate(
                        slice,
                        *mixed_width_fold_mask,
                    ))
                    .min()
                    .map(|pos| from + pos)
            }
        }
    }

    pub fn is_enabled(&self) -> bool {
        !matches!(self, Self::None)
    }

    pub fn literals(&self) -> &[String] {
        match self {
            Self::Any { literals, .. } => literals,
            Self::None
            | Self::Byte(_)
            | Self::ByteSet { .. }
            | Self::Literal(_)
            | Self::Factor { .. } => &[],
        }
    }
}

/// Compact failure-linked trie for large required-literal sets. Small sets
/// retain the standard library's highly tuned two-way search; the trie is
/// reserved for cases where rebuilding and running one searcher per
/// alternative dominates (notably C/C++ keyword inventories and
/// case-insensitive keyword lists such as ABAP's). An ASCII case-insensitive
/// finder stores lowercased literals and lowercases each input byte, which
/// matches `eq_ignore_ascii_case` windows exactly for ASCII literals.
///
/// Each node's outgoing edges occupy one contiguous, byte-sorted run of the
/// shared edge arrays. Large keyword inventories produce tens of thousands of
/// trie states; flat storage keeps construction and teardown to a handful of
/// allocations instead of one or more per state.
#[derive(Debug, Clone, PartialEq, Eq)]
#[doc(hidden)]
pub struct MultiLiteralFinder {
    nodes: Vec<FinderNode>,
    edge_bytes: Vec<u8>,
    edge_targets: Vec<u32>,
    /// Every input byte probes the root at least once. A dense root table
    /// avoids a linear scan over the large first-byte fanout while keeping
    /// deeper, usually tiny transition sets compact.
    root_edges: Box<[u32; 256]>,
    max_literal_len: usize,
    fold_ascii_case: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct FinderNode {
    edge_start: u32,
    edge_len: u32,
    failure: u32,
    /// Longest literal ending in this state or one of its failure states.
    /// The longest output has the earliest start for a fixed end position.
    output_len: u32,
}

impl MultiLiteralFinder {
    fn for_literals<S: AsRef<str>>(literals: &[S]) -> Option<Self> {
        Self::worthwhile(literals).then(|| Self::new(literals, false))
    }

    fn for_literals_ignore_ascii_case<S: AsRef<str>>(literals: &[S]) -> Option<Self> {
        Self::worthwhile(literals).then(|| Self::new(literals, true))
    }

    fn worthwhile<S: AsRef<str>>(literals: &[S]) -> bool {
        let total_bytes = literals
            .iter()
            .map(|literal| literal.as_ref().len())
            .sum::<usize>();
        literals.len() >= multi_literal_min_literals()
            && total_bytes >= multi_literal_min_total_bytes()
    }

    fn new<S: AsRef<str>>(literals: &[S], fold_ascii_case: bool) -> Self {
        // Required-literal sets arrive sorted; sort a borrowed view otherwise.
        // A case-folding finder stores lowercased literals, which can reorder
        // them, so it always owns and sorts its copies.
        let mut sorted = literals
            .iter()
            .map(|literal| {
                let bytes = literal.as_ref().as_bytes();
                if fold_ascii_case {
                    Cow::Owned(bytes.to_ascii_lowercase())
                } else {
                    Cow::Borrowed(bytes)
                }
            })
            .collect::<Vec<Cow<'_, [u8]>>>();
        if !sorted.is_sorted() {
            sorted.sort_unstable();
        }
        let total_bytes = sorted.iter().map(|literal| literal.len()).sum::<usize>();
        let capacity = total_bytes.saturating_add(1);
        let mut nodes = Vec::with_capacity(capacity);
        let mut parents = Vec::with_capacity(capacity);
        let mut node_bytes = Vec::with_capacity(capacity);
        nodes.push(FinderNode::default());
        parents.push(0u32);
        node_bytes.push(0u8);

        // In sorted order, a literal shares exactly its longest common prefix
        // with the previous literal's trie path and every later byte needs a
        // new state. Insertion is therefore one pass without edge searches,
        // and each state's children are created in ascending byte order.
        let mut path = vec![0u32];
        let mut previous: &[u8] = &[];
        let mut max_literal_len = 0usize;
        for literal in &sorted {
            let literal: &[u8] = literal;
            debug_assert!(!literal.is_empty());
            max_literal_len = max_literal_len.max(literal.len());
            let common = previous
                .iter()
                .zip(literal)
                .take_while(|(left, right)| left == right)
                .count();
            path.truncate(common + 1);
            for &byte in &literal[common..] {
                let parent = *path.last().expect("trie path keeps the root");
                let node = u32::try_from(nodes.len()).expect("prefilter trie exceeds u32");
                nodes[parent as usize].edge_len += 1;
                nodes.push(FinderNode::default());
                parents.push(parent);
                node_bytes.push(byte);
                path.push(node);
            }
            let state = *path.last().expect("trie path keeps the root") as usize;
            let output_len = u32::try_from(literal.len()).expect("prefilter literal exceeds u32");
            nodes[state].output_len = nodes[state].output_len.max(output_len);
            previous = literal;
        }

        // Lay out each state's edges contiguously. Creation order keeps every
        // run sorted by byte. `failure` is a temporary fill cursor here.
        let mut edge_start = 0u32;
        for node in &mut nodes {
            node.edge_start = edge_start;
            node.failure = edge_start;
            edge_start += node.edge_len;
        }
        let edge_count = edge_start as usize;
        let mut edge_bytes = vec![0u8; edge_count];
        let mut edge_targets = vec![0u32; edge_count];
        for child in 1..nodes.len() {
            let parent = parents[child] as usize;
            let slot = nodes[parent].failure as usize;
            nodes[parent].failure += 1;
            edge_bytes[slot] = node_bytes[child];
            edge_targets[slot] = child as u32;
        }
        drop(parents);
        drop(node_bytes);
        for node in &mut nodes {
            node.failure = 0;
        }

        let mut finder = Self {
            nodes,
            edge_bytes,
            edge_targets,
            root_edges: Box::new([u32::MAX; 256]),
            max_literal_len,
            fold_ascii_case,
        };
        let root = finder.nodes[0];
        for index in root.edge_start as usize..(root.edge_start + root.edge_len) as usize {
            finder.root_edges[finder.edge_bytes[index] as usize] = finder.edge_targets[index];
        }
        // Failure targets are strictly shallower, so a breadth-first sweep
        // finalizes each one before any state that depends on it.
        let mut queue = Vec::with_capacity(finder.nodes.len());
        queue.push(0u32);
        let mut cursor = 0usize;
        while let Some(&state) = queue.get(cursor) {
            cursor += 1;
            let node = finder.nodes[state as usize];
            for index in node.edge_start as usize..(node.edge_start + node.edge_len) as usize {
                let byte = finder.edge_bytes[index];
                let child = finder.edge_targets[index];
                queue.push(child);
                if state == 0 {
                    continue;
                }
                let mut failure = node.failure;
                let target = loop {
                    if let Some(next) = finder.goto(failure, byte) {
                        break next;
                    }
                    if failure == 0 {
                        break 0;
                    }
                    failure = finder.nodes[failure as usize].failure;
                };
                let failure_output = finder.nodes[target as usize].output_len;
                let child = &mut finder.nodes[child as usize];
                child.failure = target;
                child.output_len = child.output_len.max(failure_output);
            }
        }
        finder
    }

    /// Returns the leftmost literal start. Scanning may stop once the maximum
    /// literal length proves that no future match can begin earlier.
    fn find(&self, haystack: &[u8]) -> Option<usize> {
        let mut state = 0u32;
        let mut best = None;
        for (index, byte) in haystack.iter().copied().enumerate() {
            let byte = if self.fold_ascii_case {
                byte.to_ascii_lowercase()
            } else {
                byte
            };
            state = self.step(state, byte);
            let output_len = self.nodes[state as usize].output_len as usize;
            if output_len != 0 {
                let start = index + 1 - output_len;
                best = Some(best.map_or(start, |current: usize| current.min(start)));
            }
            if let Some(best) = best
                && index.saturating_add(2) >= best.saturating_add(self.max_literal_len)
            {
                break;
            }
        }
        best
    }

    fn step(&self, mut state: u32, byte: u8) -> u32 {
        loop {
            if let Some(next) = self.goto(state, byte) {
                return next;
            }
            if state == 0 {
                return 0;
            }
            state = self.nodes[state as usize].failure;
        }
    }

    #[inline]
    fn goto(&self, state: u32, byte: u8) -> Option<u32> {
        if state == 0 {
            let next = self.root_edges[byte as usize];
            return (next != u32::MAX).then_some(next);
        }
        let node = &self.nodes[state as usize];
        let start = node.edge_start as usize;
        let end = start + node.edge_len as usize;
        let bytes = &self.edge_bytes[start..end];
        let index = if bytes.len() <= 16 {
            bytes.iter().position(|candidate| *candidate == byte)?
        } else {
            bytes.binary_search(&byte).ok()?
        };
        Some(self.edge_targets[start + index])
    }
}

fn multi_literal_min_literals() -> usize {
    4
}

fn multi_literal_min_total_bytes() -> usize {
    32
}

fn prefilter_one(literal: String) -> Prefilter {
    if literal.len() == 1 && literal.is_ascii() {
        Prefilter::Byte(literal.as_bytes()[0])
    } else {
        Prefilter::Literal(literal)
    }
}

/// Native single-byte search (memchr substitute).
fn find_byte(haystack: &[u8], needle: u8) -> Option<usize> {
    memchr::memchr(needle, haystack)
}

#[inline]
pub(crate) fn find_byte_set(haystack: &[u8], bytes: &[u8], bitmap: &[u64; 4]) -> Option<usize> {
    match bytes {
        [] => None,
        [byte] => memchr::memchr(*byte, haystack),
        [a, b] => memchr::memchr2(*a, *b, haystack),
        [a, b, c] => memchr::memchr3(*a, *b, *c, haystack),
        _ => find_byte_set_bitmap(haystack, bitmap),
    }
}

#[inline]
fn byte_in_set(bitmap: &[u64; 4], byte: u8) -> bool {
    bitmap[byte as usize >> 6] & (1u64 << (byte & 63)) != 0
}

pub(crate) fn find_byte_set_bitmap(haystack: &[u8], bitmap: &[u64; 4]) -> Option<usize> {
    let mut index = 0;
    let len = haystack.len();
    while index + 8 <= len {
        if byte_in_set(bitmap, haystack[index]) {
            return Some(index);
        }
        if byte_in_set(bitmap, haystack[index + 1]) {
            return Some(index + 1);
        }
        if byte_in_set(bitmap, haystack[index + 2]) {
            return Some(index + 2);
        }
        if byte_in_set(bitmap, haystack[index + 3]) {
            return Some(index + 3);
        }
        if byte_in_set(bitmap, haystack[index + 4]) {
            return Some(index + 4);
        }
        if byte_in_set(bitmap, haystack[index + 5]) {
            return Some(index + 5);
        }
        if byte_in_set(bitmap, haystack[index + 6]) {
            return Some(index + 6);
        }
        if byte_in_set(bitmap, haystack[index + 7]) {
            return Some(index + 7);
        }
        index += 8;
    }
    while index < len {
        if byte_in_set(bitmap, haystack[index]) {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn find_literal(haystack: &str, needle: &str) -> Option<usize> {
    memchr::memmem::find(haystack.as_bytes(), needle.as_bytes())
}

fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    find_ignore_ascii_case(haystack, needle).is_some()
}

fn first_ascii_case_fold_candidate(haystack: &str, mask: u8) -> Option<usize> {
    if mask == 0 {
        return None;
    }
    let bytes = haystack.as_bytes();
    let mut from = 0usize;
    loop {
        let relative = match mask {
            1 => memchr::memchr(0xc5, bytes.get(from..)?),
            2 => memchr::memchr(0xe2, bytes.get(from..)?),
            _ => memchr::memchr2(0xc5, 0xe2, bytes.get(from..)?),
        }?;
        let offset = from + relative;
        if (mask & 1 != 0 && matches!(bytes.get(offset..), Some([0xc5, 0xbf, ..])))
            || (mask & 2 != 0 && matches!(bytes.get(offset..), Some([0xe2, 0x84, 0xaa, ..])))
        {
            return Some(offset);
        }
        from = offset + 1;
    }
}

fn find_ignore_ascii_case(haystack: &str, needle: &str) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    let hay = haystack.as_bytes();
    let needle = needle.as_bytes();
    if needle.len() > hay.len() {
        return None;
    }
    let first = needle[0];
    let lower = first.to_ascii_lowercase();
    let upper = first.to_ascii_uppercase();
    let mut from = 0usize;
    loop {
        let rest = hay.get(from..)?;
        let relative = if lower == upper {
            memchr::memchr(first, rest)?
        } else {
            memchr::memchr2(lower, upper, rest)?
        };
        let pos = from + relative;
        if hay
            .get(pos..pos + needle.len())
            .is_some_and(|window| window.eq_ignore_ascii_case(needle))
        {
            return Some(pos);
        }
        from = pos + 1;
    }
}

/// Scan-local memo of required-literal occurrences, keyed by compiled-pattern
/// slot. Anchored pattern-set attempts ask the same rejection question at
/// monotonically increasing positions; remembering the next occurrence turns
/// the per-position tail search into an O(1) check. Process-global matcher IDs
/// above the dense prefix are folded into a bounded direct-mapped overflow
/// table and retained in each entry, so collisions become safe cache misses
/// instead of growing tokenizer memory. Rejection-only: a stale or missing
/// entry falls back to a fresh search, so false negatives are impossible by
/// construction.
const MAX_PREFILTER_CURSOR_SLOTS: usize = 1024;

#[derive(Debug, Clone, Default)]
pub(crate) struct PrefilterCursors {
    line_ptr: usize,
    line_len: usize,
    generation: u64,
    slots: Vec<CursorSlot>,
    overflow_slots: Vec<OverflowCursorSlot>,
}

#[derive(Debug, Clone, Copy, Default)]
struct CursorSlot {
    generation: u64,
    searched_from: usize,
    next_occurrence: Option<usize>,
}

#[derive(Debug, Clone, Copy, Default)]
struct OverflowCursorSlot {
    matcher_slot: u32,
    cursor: CursorSlot,
}

impl PrefilterCursors {
    pub(crate) fn begin_line(&mut self, line: &str) {
        self.line_ptr = line.as_ptr() as usize;
        self.line_len = line.len();
        self.generation = self.generation.wrapping_add(1).max(1);
    }

    pub(crate) fn may_match(
        &mut self,
        slot: u32,
        prefilter: &Prefilter,
        line: &str,
        start: usize,
    ) -> bool {
        if !prefilter.is_enabled() {
            return true;
        }
        self.next_occurrence(slot, prefilter, line, start).is_some()
    }

    pub(crate) fn next_occurrence(
        &mut self,
        slot: u32,
        prefilter: &Prefilter,
        line: &str,
        start: usize,
    ) -> Option<usize> {
        if !prefilter.is_enabled() {
            return Some(start);
        }
        if slot == u32::MAX {
            return prefilter.next_occurrence(line, start);
        }
        let (line_ptr, line_len) = (line.as_ptr() as usize, line.len());
        if self.line_ptr != line_ptr || self.line_len != line_len {
            self.line_ptr = line_ptr;
            self.line_len = line_len;
            self.generation = self.generation.wrapping_add(1);
        }
        // Generation zero marks never-written slots; never treat it as bound.
        if self.generation == 0 {
            self.generation = 1;
        }
        let generation = self.generation;
        if (slot as usize) < MAX_PREFILTER_CURSOR_SLOTS {
            let slot = slot as usize;
            if slot >= self.slots.len() {
                self.slots.resize(slot + 1, CursorSlot::default());
            }
            return cached_next_occurrence(
                &mut self.slots[slot],
                true,
                generation,
                prefilter,
                line,
                start,
            );
        }

        let overflow_index = slot as usize % MAX_PREFILTER_CURSOR_SLOTS;
        if overflow_index >= self.overflow_slots.len() {
            self.overflow_slots
                .resize(overflow_index + 1, OverflowCursorSlot::default());
        }
        let entry = &mut self.overflow_slots[overflow_index];
        let same_matcher = entry.matcher_slot == slot;
        entry.matcher_slot = slot;
        cached_next_occurrence(
            &mut entry.cursor,
            same_matcher,
            generation,
            prefilter,
            line,
            start,
        )
    }
}

#[inline]
fn cached_next_occurrence(
    entry: &mut CursorSlot,
    same_matcher: bool,
    generation: u64,
    prefilter: &Prefilter,
    line: &str,
    start: usize,
) -> Option<usize> {
    let stale = !same_matcher || entry.generation != generation || entry.searched_from > start;
    if !stale {
        match entry.next_occurrence {
            None => return None,
            Some(occurrence) if occurrence >= start => return Some(occurrence),
            Some(_) => {}
        }
    }
    let next = prefilter.next_occurrence(line, start);
    *entry = CursorSlot {
        generation,
        searched_from: start,
        next_occurrence: next,
    };
    next
}

pub fn required_literal(pattern: &str) -> Option<String> {
    let parsed = super::ast::parse(pattern);
    match required_literals(&parsed.ast) {
        RequiredLiterals::One(literal) => Some(literal.into_owned()),
        RequiredLiterals::Any(literals) => literals
            .into_iter()
            .max_by_key(|literal| literal.len())
            .map(Cow::into_owned),
        RequiredLiterals::None => literal_prefix(pattern),
    }
}

/// Mandatory run of consecutive byte-class items, for example `\[ *]` in a
/// pattern that must match an empty array declarator. Literal extraction can
/// only require `[`, which ordinary subscript lines satisfy; the run rejects
/// them without executing the pattern.
///
/// Items describe a byte-level superset of the regex language: an atom that
/// may match a non-ASCII character admits every byte `0x80..=0xff` and one to
/// four bytes per character, and repeat modes (lazy, possessive, atomic) are
/// treated as plain repetition. A match of the regex therefore contains a
/// match of the run, so absence is a sound rejection. Every item with a
/// variable count is followed by an item that consumes at least one byte from
/// a disjoint set, which makes greedy verification from a fixed start exact
/// for that superset language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequiredFactor {
    items: Box<[FactorItem]>,
    /// The first item's byte when it is a single byte, for `memchr`.
    first_byte: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FactorItem {
    set: [u64; 4],
    min: u32,
    /// `FACTOR_UNBOUNDED` means no upper bound.
    max: u32,
}

const FACTOR_UNBOUNDED: u32 = u32::MAX;
/// Items drawn from at most this many bytes count toward selectivity.
const FACTOR_SELECTIVE_SET_BYTES: u32 = 8;
const FACTOR_MAX_ITEMS: usize = 64;

impl FactorItem {
    fn fixed(byte: u8) -> Self {
        let mut set = [0u64; 4];
        set[byte as usize >> 6] |= 1u64 << (byte & 63);
        Self {
            set,
            min: 1,
            max: 1,
        }
    }

    #[inline]
    fn contains(&self, byte: u8) -> bool {
        byte_in_set(&self.set, byte)
    }

    fn is_variable(&self) -> bool {
        self.min != self.max
    }

    fn disjoint(&self, other: &Self) -> bool {
        self.set.iter().zip(&other.set).all(|(a, b)| a & b == 0)
    }

    fn set_len(&self) -> u32 {
        self.set.iter().map(|word| word.count_ones()).sum()
    }

    fn single_byte(&self) -> Option<u8> {
        (self.set_len() == 1).then(|| {
            let word = self
                .set
                .iter()
                .position(|word| *word != 0)
                .expect("one set bit");
            (word * 64) as u8 + self.set[word].trailing_zeros() as u8
        })
    }
}

impl RequiredFactor {
    fn new(items: Vec<FactorItem>) -> Self {
        let first_byte = items.first().and_then(FactorItem::single_byte);
        Self {
            items: items.into_boxed_slice(),
            first_byte,
        }
    }

    /// Selective mandatory bytes, comparable with a required literal length.
    pub(crate) fn score(&self) -> usize {
        self.items
            .iter()
            .filter(|item| item.set_len() <= FACTOR_SELECTIVE_SET_BYTES)
            .map(|item| item.min as usize)
            .sum()
    }

    /// Leftmost start of a run occurrence in `haystack`.
    fn find(&self, haystack: &[u8]) -> Option<usize> {
        let first = self.items.first()?;
        let mut from = 0usize;
        while from < haystack.len() {
            let rest = &haystack[from..];
            let relative = match self.first_byte {
                Some(byte) => memchr::memchr(byte, rest)?,
                None => find_byte_set_bitmap(rest, &first.set)?,
            };
            let start = from + relative;
            if self.matches_at(haystack, start) {
                return Some(start);
            }
            from = start + 1;
        }
        None
    }

    fn matches_at(&self, haystack: &[u8], start: usize) -> bool {
        let mut position = start;
        for item in &self.items {
            let mut count = 0u32;
            while count < item.max
                && haystack
                    .get(position)
                    .is_some_and(|byte| item.contains(*byte))
            {
                count += 1;
                position += 1;
            }
            if count < item.min {
                return false;
            }
        }
        true
    }
}

/// Most selective mandatory byte-class run of a pattern. Callers must only
/// use it for patterns without case-insensitive scopes.
pub(crate) fn required_factor(ast: &Ast) -> Option<RequiredFactor> {
    let mut best = None;
    collect_required_factors(ast, &mut best);
    best
}

fn collect_required_factors(ast: &Ast, best: &mut Option<RequiredFactor>) {
    match ast {
        Ast::Concat(nodes) => {
            let mut run = Vec::new();
            for node in nodes {
                let checkpoint = run.len();
                if factor_items(node, &mut run) {
                    if let Ast::Look {
                        kind: LookKind::Ahead,
                        child,
                    } = node
                    {
                        collect_required_factors(child, best);
                    }
                    continue;
                }
                run.truncate(checkpoint);
                finish_factor_run(std::mem::take(&mut run), best);
                collect_required_factors(node, best);
            }
            finish_factor_run(run, best);
        }
        Ast::Group { child, .. }
        | Ast::Look {
            kind: LookKind::Ahead,
            child,
        } => collect_required_factors(child, best),
        Ast::Flags { flags, child } if !flags.case_insensitive => {
            collect_required_factors(child, best);
        }
        Ast::Repeat { node, min, .. } if *min > 0 => collect_required_factors(node, best),
        _ => {
            let mut run = Vec::new();
            if factor_items(ast, &mut run) {
                finish_factor_run(run, best);
            }
        }
    }
}

/// Appends the byte items consumed by a node built only from literals,
/// classes, zero-width assertions, and repeats of one such atom. Returns
/// `false` for anything else; the caller discards partial output.
fn factor_items(ast: &Ast, out: &mut Vec<FactorItem>) -> bool {
    if out.len() > FACTOR_MAX_ITEMS {
        return false;
    }
    match ast {
        Ast::Empty | Ast::Anchor(_) | Ast::Look { .. } => true,
        Ast::Literal(literal) => {
            out.extend(literal.bytes().map(FactorItem::fixed));
            true
        }
        Ast::Class(class) => {
            out.push(class_factor_item(class));
            true
        }
        Ast::Group { child, .. } => factor_items(child, out),
        Ast::Flags { flags, child } if !flags.case_insensitive => factor_items(child, out),
        Ast::Concat(nodes) => nodes.iter().all(|node| factor_items(node, out)),
        Ast::Repeat { node, min, max, .. } => {
            let mut inner = Vec::new();
            if !factor_items(node, &mut inner) || inner.len() > 1 {
                return false;
            }
            // A zero-width body consumes nothing however often it repeats.
            let Some(item) = inner.pop() else {
                return true;
            };
            let Ok(min) = u32::try_from(*min) else {
                return false;
            };
            let max = match max {
                None => FACTOR_UNBOUNDED,
                Some(max) => match u32::try_from(*max) {
                    Ok(max) => item.max.saturating_mul(max).min(FACTOR_UNBOUNDED - 1),
                    Err(_) => return false,
                },
            };
            out.push(FactorItem {
                set: item.set,
                min: item.min.saturating_mul(min),
                max,
            });
            true
        }
        _ => false,
    }
}

fn class_factor_item(class: &CharClass) -> FactorItem {
    let ascii = super::bytecode::ascii_class_masks(class).0;
    if class_may_match_non_ascii(class) {
        // One character is one to four UTF-8 bytes.
        FactorItem {
            set: [ascii[0], ascii[1], u64::MAX, u64::MAX],
            min: 1,
            max: 4,
        }
    } else {
        FactorItem {
            set: [ascii[0], ascii[1], 0, 0],
            min: 1,
            max: 1,
        }
    }
}

fn class_may_match_non_ascii(class: &CharClass) -> bool {
    // Intersections only narrow the first union, so the union alone gives a
    // conservative answer.
    class.negated
        || class.atoms.iter().any(|atom| match atom {
            ClassAtom::Char(ch) => !ch.is_ascii(),
            ClassAtom::Range(_, end) => !end.is_ascii(),
            ClassAtom::Nested(nested) => class_may_match_non_ascii(nested),
            ClassAtom::Perl(_) | ClassAtom::Posix { .. } | ClassAtom::Unicode { .. } => true,
        })
}

/// Splits a mandatory item run into greedy-exact segments and keeps the most
/// selective one. Any contiguous piece of a mandatory run is itself mandatory,
/// and so is a variable item's minimum count at either end of a piece.
fn finish_factor_run(run: Vec<FactorItem>, best: &mut Option<RequiredFactor>) {
    let mut segment: Vec<FactorItem> = Vec::new();
    let mut items = run.into_iter().peekable();
    while let Some(mut item) = items.next() {
        if segment.is_empty() {
            item.max = item.min;
        }
        if item.min == 0 && item.max == 0 {
            continue;
        }
        let greedy_exact = !item.is_variable()
            || items
                .peek()
                .is_some_and(|next| next.min > 0 && item.disjoint(next));
        if greedy_exact {
            segment.push(item);
            continue;
        }
        if item.min > 0 {
            item.max = item.min;
            segment.push(item);
        }
        offer_factor(std::mem::take(&mut segment), best);
    }
    offer_factor(segment, best);
}

fn offer_factor(mut items: Vec<FactorItem>, best: &mut Option<RequiredFactor>) {
    if let Some(last) = items.last_mut() {
        last.max = last.min;
    }
    while items.last().is_some_and(|item| item.min == 0) {
        items.pop();
    }
    // Runs of fixed single bytes are literals, which the literal extractor
    // already finds with substring search.
    if items.len() < 2
        || items
            .iter()
            .all(|item| !item.is_variable() && item.set_len() == 1)
    {
        return;
    }
    let candidate = RequiredFactor::new(items);
    let score = candidate.score();
    if score >= 2 && best.as_ref().is_none_or(|best| score > best.score()) {
        *best = Some(candidate);
    }
}

pub fn required_literals(ast: &Ast) -> RequiredLiterals<'_> {
    if let Some(literal) = exact_literal(ast).filter(|literal| !literal.is_empty()) {
        return RequiredLiterals::One(literal);
    }
    match ast {
        Ast::Concat(nodes) => sequence_required_literals(nodes),
        Ast::Alternation(branches) => alternation_required_literals(branches),
        Ast::Group { child, .. } | Ast::Flags { child, .. } => required_literals(child),
        Ast::Look {
            kind: LookKind::Ahead,
            child,
        } => required_literals(child),
        Ast::Repeat { node, min, .. } if *min > 0 => required_literals(node),
        Ast::Class(class) => class_required_literals(class),
        _ => RequiredLiterals::None,
    }
}

fn sequence_required_literals(nodes: &[Ast]) -> RequiredLiterals<'_> {
    let mut best = RequiredLiterals::None;
    let mut run = String::new();
    for node in nodes {
        let run_len = run.len();
        if append_exact_literal(node, &mut run) {
            continue;
        }
        run.truncate(run_len);
        if !run.is_empty() {
            best = choose_more_selective(
                best,
                RequiredLiterals::One(Cow::Owned(std::mem::take(&mut run))),
            );
        }
        let candidate = required_literals(node);
        best = choose_more_selective(best, candidate);
    }
    if !run.is_empty() {
        best = choose_more_selective(best, RequiredLiterals::One(Cow::Owned(run)));
    }
    best
}

/// The exact string `ast` matches, if it is one. A single literal (possibly
/// grouped) is borrowed; only concatenations build a new string.
fn exact_literal(ast: &Ast) -> Option<Cow<'_, str>> {
    match ast {
        Ast::Empty => Some(Cow::Borrowed("")),
        Ast::Literal(literal) => Some(Cow::Borrowed(literal)),
        Ast::Concat(nodes) => {
            if !nodes.iter().all(is_exact_literal) {
                return None;
            }
            let mut out = String::new();
            for node in nodes {
                append_exact_literal(node, &mut out);
            }
            Some(Cow::Owned(out))
        }
        Ast::Group { child, .. } | Ast::Flags { child, .. } => exact_literal(child),
        _ => None,
    }
}

fn is_exact_literal(ast: &Ast) -> bool {
    match ast {
        Ast::Empty | Ast::Literal(_) => true,
        Ast::Concat(nodes) => nodes.iter().all(is_exact_literal),
        Ast::Group { child, .. } | Ast::Flags { child, .. } => is_exact_literal(child),
        _ => false,
    }
}

/// Appends the exact string `ast` matches; on `false` the caller discards
/// whatever was appended.
fn append_exact_literal(ast: &Ast, out: &mut String) -> bool {
    match ast {
        Ast::Empty => true,
        Ast::Literal(literal) => {
            out.push_str(literal);
            true
        }
        Ast::Concat(nodes) => nodes.iter().all(|node| append_exact_literal(node, out)),
        Ast::Group { child, .. } | Ast::Flags { child, .. } => append_exact_literal(child, out),
        _ => false,
    }
}

fn alternation_required_literals(branches: &[Ast]) -> RequiredLiterals<'_> {
    let mut literals = Vec::new();
    for branch in branches {
        match required_literals(branch) {
            RequiredLiterals::One(literal) => literals.push(literal),
            RequiredLiterals::Any(mut branch_literals) => literals.append(&mut branch_literals),
            RequiredLiterals::None => return RequiredLiterals::None,
        }
    }
    literals.sort();
    literals.dedup();
    RequiredLiterals::Any(literals)
}

fn choose_more_selective<'a>(
    left: RequiredLiterals<'a>,
    right: RequiredLiterals<'a>,
) -> RequiredLiterals<'a> {
    if left.is_empty() {
        return right;
    }
    if right.is_empty() {
        return left;
    }
    let left_len = max_literal_len(&left);
    let right_len = max_literal_len(&right);
    if right_len > left_len {
        return right;
    }
    if right_len < left_len {
        return left;
    }
    if literal_cardinality(&right) < literal_cardinality(&left) {
        right
    } else {
        left
    }
}

fn max_literal_len(literals: &RequiredLiterals) -> usize {
    match literals {
        RequiredLiterals::None => 0,
        RequiredLiterals::One(literal) => literal.len(),
        RequiredLiterals::Any(literals) => literals
            .iter()
            .map(|literal| literal.len())
            .max()
            .unwrap_or(0),
    }
}

fn literal_cardinality(literals: &RequiredLiterals) -> usize {
    match literals {
        RequiredLiterals::None => usize::MAX,
        RequiredLiterals::One(_) => 1,
        RequiredLiterals::Any(literals) => literals.len(),
    }
}

fn class_required_literals(class: &CharClass) -> RequiredLiterals<'_> {
    if class.negated || class.atoms.is_empty() {
        return RequiredLiterals::None;
    }
    // Every intersection result is a subset of the first union. Its finite
    // literals therefore remain a safe (possibly broader) prefilter set.
    let mut literals = Vec::new();
    for atom in &class.atoms {
        match atom {
            ClassAtom::Char(ch) => literals.push(Cow::Owned(ch.to_string())),
            ClassAtom::Range(..)
            | ClassAtom::Perl(_)
            | ClassAtom::Posix { .. }
            | ClassAtom::Unicode { .. }
            | ClassAtom::Nested(_) => return RequiredLiterals::None,
        }
    }
    literals.sort();
    literals.dedup();
    match literals.len() {
        0 => RequiredLiterals::None,
        1 => RequiredLiterals::One(literals.remove(0)),
        _ => RequiredLiterals::Any(literals),
    }
}

fn literal_prefix(pattern: &str) -> Option<String> {
    let mut literal = String::new();
    let mut escaped = false;
    for ch in pattern.chars() {
        if escaped {
            if ch.is_ascii_alphanumeric() {
                return None;
            }
            literal.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '(' | ')' | '[' | ']' | '{' | '}' | '|' | '?' | '*' | '+' | '.' | '^' | '$' => break,
            ch => literal.push(ch),
        }
    }
    (!literal.is_empty()).then_some(literal)
}

/// Native multi-literal leftmost search used by pattern sets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralSet {
    literals: Vec<String>,
    trie: Vec<LiteralTrieNode>,
    empty_pattern: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct LiteralTrieNode {
    edges: Vec<(u8, usize)>,
    terminal_patterns: Vec<usize>,
}

impl LiteralSet {
    pub(crate) fn retained_heap_bytes(&self) -> usize {
        let mut bytes = self
            .literals
            .capacity()
            .saturating_mul(std::mem::size_of::<String>())
            .saturating_add(
                self.trie
                    .capacity()
                    .saturating_mul(std::mem::size_of::<LiteralTrieNode>()),
            );
        for literal in &self.literals {
            bytes = bytes.saturating_add(literal.capacity());
        }
        for node in &self.trie {
            bytes = bytes
                .saturating_add(
                    node.edges
                        .capacity()
                        .saturating_mul(std::mem::size_of::<(u8, usize)>()),
                )
                .saturating_add(
                    node.terminal_patterns
                        .capacity()
                        .saturating_mul(std::mem::size_of::<usize>()),
                );
        }
        bytes
    }

    pub fn new(literals: Vec<String>) -> Self {
        let mut trie = vec![LiteralTrieNode::default()];
        let mut empty_pattern = None;
        for (pattern, literal) in literals.iter().enumerate() {
            if literal.is_empty() {
                empty_pattern =
                    Some(empty_pattern.map_or(pattern, |best: usize| best.min(pattern)));
                continue;
            }
            let mut node = 0usize;
            for byte in literal.bytes() {
                let next = trie[node]
                    .edges
                    .iter()
                    .find_map(|(edge, next)| (*edge == byte).then_some(*next));
                node = if let Some(next) = next {
                    next
                } else {
                    let next = trie.len();
                    trie.push(LiteralTrieNode::default());
                    trie[node].edges.push((byte, next));
                    next
                };
            }
            trie[node].terminal_patterns.push(pattern);
        }
        Self {
            literals,
            trie,
            empty_pattern,
        }
    }

    pub fn literals(&self) -> &[String] {
        &self.literals
    }

    /// Leftmost match; on equal start offset the lowest pattern index wins.
    pub fn find(&self, haystack: &str, from: usize) -> Option<(usize, usize, usize)> {
        if !haystack.is_char_boundary(from) {
            return None;
        }
        for start in haystack[from..]
            .char_indices()
            .map(|(offset, _)| from + offset)
            .chain(std::iter::once(haystack.len()))
        {
            let mut best = self.empty_pattern.map(|pattern| (pattern, start));
            let mut node = 0usize;
            let mut end = start;
            while let Some(byte) = haystack.as_bytes().get(end) {
                let Some(next) = self.trie[node]
                    .edges
                    .iter()
                    .find_map(|(edge, next)| (*edge == *byte).then_some(*next))
                else {
                    break;
                };
                node = next;
                end += 1;
                for pattern in &self.trie[node].terminal_patterns {
                    if best.is_none_or(|(best, _)| *pattern < best) {
                        best = Some((*pattern, end));
                    }
                }
            }
            if let Some((pattern, end)) = best {
                return Some((pattern, start, end));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::regex::ast::parse;

    #[test]
    fn required_factor_rejects_subscripts_for_empty_array_declarators() {
        let parsed = parse(r"(\w+)\s*(\[) *(])\s*(=)");
        let prefilter = parsed.prefilter();
        assert!(
            matches!(prefilter, Prefilter::Factor { .. }),
            "{prefilter:?}"
        );
        assert!(!prefilter.may_match("values[index] = values[index] * 2;", 0));
        assert!(prefilter.may_match("int values[ ] = {1};", 0));
        assert!(prefilter.may_match("int values[] = {1};", 0));
        assert!(!prefilter.may_match("int values[] = {1};", 11));
        assert_eq!(prefilter.next_occurrence("a[i] = b[i]", 0), None);
        assert!(prefilter.next_occurrence("a[i] b[  ] =", 0).is_some());
    }

    #[test]
    fn required_factor_is_not_derived_under_case_folding() {
        for pattern in [r"(?i)\[ *]=", r"(?i:\[ *])=", r"x(?i)\[ *]"] {
            let parsed = parse(pattern);
            assert!(
                !matches!(parsed.prefilter(), Prefilter::Factor { .. }),
                "{pattern}: {:?}",
                parsed.prefilter()
            );
        }
    }

    #[test]
    fn required_factor_stops_where_greedy_verification_would_be_inexact() {
        // `a*` followed by `a` would need backtracking; neither piece of the
        // run is more selective than the literal extractor's answer.
        assert_eq!(required_factor(&parse("xa*a").ast), None);
        // A variable item followed by an optional one also ends the run.
        assert_eq!(required_factor(&parse("x *y?z").ast), None);
        assert!(required_factor(&parse(r"x *\(").ast).is_some());
    }

    /// Every start where the pattern matches must remain viable under its
    /// prefilter. The reference is the bytecode VM, which never consults the
    /// prefilter.
    #[test]
    fn required_factor_prefilter_never_rejects_a_matching_start() {
        use crate::engine::regex::AnchorContext;
        use crate::engine::regex::backtrack::StepBudget;
        use crate::engine::regex::bytecode::{BytecodeScratch, Program};

        let patterns = [
            r"(\[) *(])\s*=",
            r"x *\(",
            r"xa*a",
            r"x[ab]*b",
            r"[a-z]+ *\(",
            r"\w+ *\(",
            r"é+ ?x",
            r"[^\]]* *\]x",
            r"(?:ab)+c d",
            r"a{2,3} b",
            r"a(?=b *c)",
            r"a*+a b",
            r"(?<=x)y *z",
            r"b\s*+=\s*+c",
            r"[ab]{2} *c",
            r"\[\s*\]",
            r"(?:\[ *]|\( *\))=",
            r"(a)\1 *b",
            r"(?<n>a) *\k<n>",
            r"(\( *\)|\g<1>x) *=",
            r"𝒳 *[^a]",
            r"[^x] +\t",
            r"\h+ *\]",
        ];
        let alphabet = [
            "a", "b", "c", "x", "y", "z", " ", " ", "[", "]", "(", ")", "=", "é", "𝒳", "\t", "\n",
        ];
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let mut matched_starts = 0usize;
        for pattern in patterns {
            let parsed = parse(pattern);
            let program =
                Program::compile_captures(&parsed, &(0..=parsed.capture_count).collect::<Vec<_>>())
                    .unwrap_or_else(|error| panic!("{pattern}: {error:?}"));
            let prefilter = Prefilter::from_regex(&parsed);
            let mut scratch = BytecodeScratch::default();
            for _ in 0..20_000 {
                let len = (next() % 14) as usize;
                let line = (0..len)
                    .map(|_| alphabet[(next() % alphabet.len() as u64) as usize])
                    .collect::<String>();
                for start in (0..=line.len()).filter(|index| line.is_char_boundary(*index)) {
                    let mut budget = StepBudget::new(1_000_000);
                    let matched = program
                        .execute(
                            &line,
                            start,
                            AnchorContext::line_start(),
                            &mut budget,
                            &mut scratch,
                        )
                        .expect("budget");
                    if matched.is_some() {
                        matched_starts += 1;
                        assert!(
                            prefilter.may_match(&line, start),
                            "{pattern} matches {line:?} at {start} but {prefilter:?} rejects it"
                        );
                    }
                }
            }
        }
        assert!(matched_starts > 15_000, "{matched_starts}");
    }

    #[test]
    fn extracts_safe_literal_prefix() {
        assert_eq!(required_literal("foo|bar"), Some("foo".to_owned()));
        assert_eq!(required_literal(r"\w+"), None);
    }

    #[test]
    fn extracts_alternation_literals() {
        let parsed = parse("foo|bar");
        assert_eq!(
            required_literals(&parsed.ast),
            RequiredLiterals::Any(vec!["bar".into(), "foo".into()])
        );
    }

    #[test]
    fn extracts_positive_lookahead_literals() {
        let parsed = parse(r"(?<=return)\s*(?=(<)\s*([A-Za-z]+))");
        assert_eq!(
            required_literals(&parsed.ast),
            RequiredLiterals::One("<".into())
        );

        let parsed = parse(r"(?<!\\)(?=;)");
        assert_eq!(
            required_literals(&parsed.ast),
            RequiredLiterals::One(";".into())
        );

        let parsed = parse(r"(?<=return)");
        assert_eq!(required_literals(&parsed.ast), RequiredLiterals::None);
    }

    #[test]
    fn extracts_positive_class_literals() {
        let parsed = parse(r"(?=[;)])(?<!\\)");
        assert_eq!(
            required_literals(&parsed.ast),
            RequiredLiterals::Any(vec![")".into(), ";".into()])
        );

        let parsed = parse(r"(?=[A-Z])");
        assert_eq!(required_literals(&parsed.ast), RequiredLiterals::None);
    }

    #[test]
    fn enables_ascii_literal_prefilter_for_case_insensitive_patterns() {
        let parsed = parse(r"(?i)foo");
        let prefilter = Prefilter::from_regex(&parsed);
        assert!(prefilter.is_enabled());
        assert!(prefilter.may_match("xxFOO", 0));
        assert!(!prefilter.may_match("xxbar", 0));

        let parsed = parse(r"(?i)k");
        let prefilter = Prefilter::from_regex(&parsed);
        assert!(prefilter.may_match("K", 0));
        assert_eq!(prefilter.next_occurrence("xxK", 0), Some(2));

        let parsed = parse(r"(?i)café");
        assert!(!Prefilter::from_regex(&parsed).is_enabled());

        let parsed = parse(r"foo");
        assert!(Prefilter::from_regex(&parsed).is_enabled());
    }

    #[test]
    fn prefilter_uses_byte_scan_for_single_byte() {
        let parsed = parse("x+");
        let prefilter = Prefilter::from_pattern(&parsed.ast);
        assert!(prefilter.may_match("abcx", 0));
        assert!(!prefilter.may_match("abc", 0));
    }

    #[test]
    fn byte_set_prefilter_finds_first_of_many_bytes() {
        let parsed = parse("a|e|i|o|u");
        let prefilter = Prefilter::from_pattern(&parsed.ast);
        assert_eq!(prefilter.next_occurrence("xxxyz o", 0), Some(6));
        assert_eq!(prefilter.next_occurrence("xxxyz o", 6), Some(6));
        assert_eq!(prefilter.next_occurrence("xxxyz o", 7), None);
        assert!(!prefilter.may_match("bcdfg", 0));
    }

    #[test]
    fn ignore_ascii_case_search_finds_first_folded_needle() {
        assert_eq!(find_ignore_ascii_case("xxSELECT", "select"), Some(2));
        assert_eq!(find_ignore_ascii_case("Select", "SELECT"), Some(0));
        assert_eq!(find_ignore_ascii_case("nope", "SELECT"), None);
        assert_eq!(find_ignore_ascii_case("ssssSELECT", "select"), Some(4));
    }

    #[test]
    fn multi_literal_finder_preserves_leftmost_and_failure_outputs() {
        let literals = [
            "bc", "abcd", "suffix", "hers", "his", "she", "he", "keyword",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        let finder = MultiLiteralFinder::new(&literals, false);
        // `bc` ends before `abcd`, but the longer literal starts earlier.
        assert_eq!(finder.find(b"zabcd"), Some(1));
        // `he` is reached through the failure link after scanning `she`.
        assert_eq!(finder.find(b"ushers"), Some(1));
        assert_eq!(finder.find(b"nothing"), None);
    }

    #[test]
    fn multi_literal_finder_matches_naive_leftmost_search() {
        // Small alphabet forces shared prefixes, deep failure chains, wide
        // fanouts (binary-searched edges), and overlapping outputs.
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = |bound: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % bound
        };
        for round in 0..200 {
            let alphabet: &[u8] = if round % 3 == 0 {
                b"ab"
            } else if round % 3 == 1 {
                b"abcd-"
            } else {
                b"abcdefghijklmnopqrstuvwxyz0123456789"
            };
            let count = 1 + next(40) as usize;
            let mut literals = (0..count)
                .map(|_| {
                    let len = 1 + next(6) as usize;
                    (0..len)
                        .map(|_| alphabet[next(alphabet.len() as u64) as usize] as char)
                        .collect::<String>()
                })
                .collect::<Vec<_>>();
            if round % 3 == 2 {
                // A wide interior fanout exercises binary-searched edges.
                literals.extend(alphabet.iter().map(|byte| format!("a{}", *byte as char)));
            }
            let finder = MultiLiteralFinder::new(&literals, false);
            for _ in 0..20 {
                let len = next(24) as usize;
                let haystack = (0..len)
                    .map(|_| alphabet[next(alphabet.len() as u64) as usize])
                    .collect::<Vec<_>>();
                let naive = (0..=haystack.len()).find(|&start| {
                    literals
                        .iter()
                        .any(|literal| haystack[start..].starts_with(literal.as_bytes()))
                });
                assert_eq!(
                    finder.find(&haystack),
                    naive,
                    "literals {literals:?} haystack {:?}",
                    String::from_utf8_lossy(&haystack)
                );
            }
        }
    }

    #[test]
    fn high_process_global_slots_use_a_bounded_collision_safe_memo() {
        let keyword = parse("keyword");
        let keyword = Prefilter::from_pattern(&keyword.ast);
        let missing = parse("missing");
        let missing = Prefilter::from_pattern(&missing.ast);
        let mut cursors = PrefilterCursors::default();
        let line = "keyword";
        let high_slot = 1_000_000;
        let colliding_slot = high_slot + MAX_PREFILTER_CURSOR_SLOTS as u32;
        cursors.begin_line(line);

        assert!(cursors.may_match(high_slot, &keyword, line, 0));
        assert!(!cursors.may_match(colliding_slot, &missing, line, 0));
        assert!(cursors.may_match(high_slot, &keyword, line, 0));
        assert!(cursors.slots.is_empty());
        assert!(!cursors.overflow_slots.is_empty());
        assert!(cursors.overflow_slots.len() <= MAX_PREFILTER_CURSOR_SLOTS);
    }

    #[test]
    fn explicit_line_boundary_invalidates_reused_string_storage() {
        let parsed = parse("keyword");
        let prefilter = Prefilter::from_pattern(&parsed.ast);
        let mut cursors = PrefilterCursors::default();
        let mut line = String::from("no-match");
        cursors.begin_line(&line);
        assert!(!cursors.may_match(7, &prefilter, &line, 0));

        line.clear();
        line.push_str("keyword!");
        cursors.begin_line(&line);
        assert!(cursors.may_match(7, &prefilter, &line, 0));
    }

    #[test]
    fn large_any_prefilter_uses_compiled_finder_without_changing_answers() {
        let parsed = parse(concat!(
            "alpha_long|beta_long|gamma_long|delta_long|epsilon_long|zeta_long|theta_long|keyword_long|",
            "iota_long|kappa_long|lambda_long|mu_long_value|nu_long_value|xi_long_value|omicron_long|pi_long_value",
        ));
        let prefilter = Prefilter::from_pattern(&parsed.ast);
        let Prefilter::Any { finder, .. } = &prefilter else {
            panic!("expected Any prefilter");
        };
        assert!(finder.is_some());
        assert_eq!(
            prefilter.next_occurrence("xx keyword_long alpha", 0),
            Some(3)
        );
        assert!(prefilter.may_match("xx keyword_long alpha", 3));
        assert!(!prefilter.may_match("xx keyword_long alpha", 4));
        assert!(!prefilter.may_match("unrelated", 0));
    }

    #[test]
    fn case_insensitive_finder_matches_per_literal_search() {
        let literals: Vec<String> = [
            "abstract",
            "accept",
            "accepting",
            "add",
            "add-corresponding",
            "Alias",
            "SELECT",
            "sKip",
            "kind",
            "he",
            "she",
            "hers",
            "Z_9",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        let prefilter = Prefilter::from_required(
            RequiredLiterals::Any(literals.iter().cloned().map(Cow::Owned).collect()),
            true,
        );
        let Prefilter::Any {
            finder,
            mixed_width_fold_mask,
            ..
        } = &prefilter
        else {
            panic!("expected Any prefilter");
        };
        assert!(finder.is_some());
        let reference = |slice: &str| {
            literals
                .iter()
                .filter_map(|literal| find_ignore_ascii_case(slice, literal))
                .chain(first_ascii_case_fold_candidate(
                    slice,
                    *mixed_width_fold_mask,
                ))
                .min()
        };
        for text in [
            "",
            "nothing here",
            "  ADD-CORRESPONDING x",
            "xaccEPTing",
            "uSHErs",
            "ſkip and \u{212a}ind",
            "Ä add é SELECT",
            "z_9 Z_9",
            "ACCEPT",
            "aDd",
        ] {
            for from in (0..=text.len()).filter(|from| text.is_char_boundary(*from)) {
                let expected = reference(&text[from..]).map(|pos| from + pos);
                assert_eq!(
                    prefilter.next_occurrence(text, from),
                    expected,
                    "{text:?} from {from}"
                );
                assert_eq!(
                    prefilter.may_match(text, from),
                    expected.is_some(),
                    "{text:?} from {from}"
                );
            }
        }
    }

    #[test]
    fn literal_set_leftmost_lowest_index() {
        let set = LiteralSet::new(vec!["bb".into(), "b".into(), "a".into()]);
        assert_eq!(set.find("abb", 0), Some((2, 0, 1)));
        assert_eq!(set.find("abb", 1), Some((0, 1, 3)));
    }

    #[test]
    fn literal_trie_preserves_empty_prefix_and_utf8_order() {
        let set = LiteralSet::new(vec!["éx".into(), "é".into(), "".into()]);
        assert_eq!(set.find("zéx", 1), Some((0, 1, 4)));

        let set = LiteralSet::new(vec!["".into(), "é".into()]);
        assert_eq!(set.find("é", 0), Some((0, 0, 0)));
        assert_eq!(set.find("é", 1), None);
    }
}
