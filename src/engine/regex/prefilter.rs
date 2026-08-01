use std::collections::VecDeque;

use super::ast::{
    Ast, CharClass, ClassAtom, LookKind, ParsedRegex, has_case_insensitive_scope,
    uniform_effective_flags,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequiredLiterals {
    None,
    One(String),
    Any(Vec<String>),
}

impl RequiredLiterals {
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
        literals: Vec<String>,
        ascii_case_insensitive: bool,
        finder: Option<MultiLiteralFinder>,
    },
}

impl Prefilter {
    pub fn from_regex(parsed: &ParsedRegex) -> Self {
        if let Some(flags) = uniform_effective_flags(&parsed.ast)
            && has_case_insensitive_scope(&parsed.ast)
        {
            return Self::from_required(required_literals(&parsed.ast), flags.case_insensitive);
        }
        if let Ast::Flags { flags, child } = &parsed.ast
            && !has_flag_scope(child)
        {
            // TextMate grammars commonly spell a whole pattern as
            // `(?i:...)`. Treat that as a uniform case policy instead of
            // disabling required-literal filtering for the scoped flag node.
            return Self::from_required(required_literals(child), flags.case_insensitive);
        }
        if parsed.flags.case_insensitive {
            Self::from_case_insensitive_pattern(&parsed.ast)
        } else if has_case_insensitive_scope(&parsed.ast) {
            // A single prefilter cannot safely apply one case-folding policy
            // to a pattern containing scoped flags. False negatives would
            // change TextMate rule selection, so leave filtering disabled.
            Self::None
        } else {
            Self::from_pattern(&parsed.ast)
        }
    }

    pub fn from_pattern(ast: &Ast) -> Self {
        Self::from_required(required_literals(ast), false)
    }

    fn from_required(required: RequiredLiterals, ascii_case_insensitive: bool) -> Self {
        if ascii_case_insensitive {
            let literals = match required {
                RequiredLiterals::None => return Self::None,
                RequiredLiterals::One(literal) => vec![literal],
                RequiredLiterals::Any(literals) => literals,
            };
            if literals.is_empty()
                || literals
                    .iter()
                    .any(|literal| literal.is_empty() || !literal.is_ascii())
            {
                return Self::None;
            }
            return Self::Any {
                literals,
                ascii_case_insensitive: true,
                finder: None,
            };
        }
        match required {
            RequiredLiterals::None => Self::None,
            RequiredLiterals::One(literal) => prefilter_one(literal),
            RequiredLiterals::Any(literals) if literals.is_empty() => Self::None,
            RequiredLiterals::Any(literals) if literals.len() == 1 => {
                prefilter_one(literals.into_iter().next().expect("one literal"))
            }
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
            RequiredLiterals::Any(literals) => Self::Any {
                finder: MultiLiteralFinder::for_literals(&literals),
                literals,
                ascii_case_insensitive: false,
            },
        }
    }

    fn from_case_insensitive_pattern(ast: &Ast) -> Self {
        Self::from_required(required_literals(ast), true)
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
            Self::Any {
                literals,
                ascii_case_insensitive: false,
                finder,
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
                ..
            } => literals
                .iter()
                .any(|literal| contains_ignore_ascii_case(slice, literal)),
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
            Self::Any {
                literals,
                ascii_case_insensitive: false,
                finder,
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
                ..
            } => literals
                .iter()
                .filter_map(|literal| find_ignore_ascii_case(slice, literal))
                .min()
                .map(|pos| from + pos),
        }
    }

    pub fn is_enabled(&self) -> bool {
        !matches!(self, Self::None)
    }

    pub fn literals(&self) -> &[String] {
        match self {
            Self::Any { literals, .. } => literals,
            Self::None | Self::Byte(_) | Self::ByteSet { .. } | Self::Literal(_) => &[],
        }
    }
}

fn has_flag_scope(ast: &Ast) -> bool {
    match ast {
        Ast::Flags { .. } => true,
        Ast::Concat(nodes) | Ast::Alternation(nodes) => nodes.iter().any(has_flag_scope),
        Ast::Conditional {
            matched, unmatched, ..
        } => has_flag_scope(matched) || has_flag_scope(unmatched),
        Ast::Repeat { node, .. }
        | Ast::Group { child: node, .. }
        | Ast::Look { child: node, .. } => has_flag_scope(node),
        _ => false,
    }
}

/// Compact failure-linked trie for large case-sensitive required-literal
/// sets. Small sets retain the standard library's highly tuned two-way
/// search; the trie is reserved for cases where rebuilding and running one
/// searcher per alternative dominates (notably C/C++ keyword inventories).
#[derive(Debug, Clone, PartialEq, Eq)]
#[doc(hidden)]
pub struct MultiLiteralFinder {
    nodes: Vec<FinderNode>,
    /// Every input byte probes the root at least once. A dense root table
    /// avoids a linear scan over the large first-byte fanout while keeping
    /// deeper, usually tiny transition sets compact.
    root_edges: Box<[u32; 256]>,
    max_literal_len: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct FinderNode {
    edges: Vec<(u8, u32)>,
    failure: u32,
    /// Longest literal ending in this state or one of its failure states.
    /// The longest output has the earliest start for a fixed end position.
    output_len: usize,
}

impl MultiLiteralFinder {
    fn for_literals(literals: &[String]) -> Option<Self> {
        let total_bytes = literals.iter().map(String::len).sum::<usize>();
        (literals.len() >= multi_literal_min_literals()
            && total_bytes >= multi_literal_min_total_bytes())
        .then(|| Self::new(literals))
    }

    fn new(literals: &[String]) -> Self {
        let mut nodes = vec![FinderNode::default()];
        let mut max_literal_len = 0usize;
        for literal in literals {
            debug_assert!(!literal.is_empty());
            max_literal_len = max_literal_len.max(literal.len());
            let mut state = 0usize;
            for byte in literal.bytes() {
                let next = edge(&nodes[state], byte);
                state = if let Some(next) = next {
                    next as usize
                } else {
                    let next = u32::try_from(nodes.len()).expect("prefilter trie exceeds u32");
                    nodes.push(FinderNode::default());
                    nodes[state].edges.push((byte, next));
                    next as usize
                };
            }
            nodes[state].output_len = nodes[state].output_len.max(literal.len());
        }

        let mut queue = VecDeque::new();
        let root_children = nodes[0]
            .edges
            .iter()
            .map(|(_, child)| *child)
            .collect::<Vec<_>>();
        for child in root_children {
            queue.push_back(child);
        }
        while let Some(state) = queue.pop_front() {
            let transitions = nodes[state as usize].edges.clone();
            for (byte, child) in transitions {
                let mut failure = nodes[state as usize].failure;
                while failure != 0 && edge(&nodes[failure as usize], byte).is_none() {
                    failure = nodes[failure as usize].failure;
                }
                if let Some(next) = edge(&nodes[failure as usize], byte)
                    && next != child
                {
                    failure = next;
                }
                nodes[child as usize].failure = failure;
                nodes[child as usize].output_len = nodes[child as usize]
                    .output_len
                    .max(nodes[failure as usize].output_len);
                queue.push_back(child);
            }
        }
        let mut root_edges = Box::new([u32::MAX; 256]);
        for (byte, child) in &nodes[0].edges {
            root_edges[*byte as usize] = *child;
        }
        Self {
            nodes,
            root_edges,
            max_literal_len,
        }
    }

    /// Returns the leftmost literal start. Scanning may stop once the maximum
    /// literal length proves that no future match can begin earlier.
    fn find(&self, haystack: &[u8]) -> Option<usize> {
        let mut state = 0u32;
        let mut best = None;
        for (index, byte) in haystack.iter().copied().enumerate() {
            while state != 0 && self.transition(state, byte).is_none() {
                state = self.nodes[state as usize].failure;
            }
            state = self.transition(state, byte).unwrap_or(0);
            let output_len = self.nodes[state as usize].output_len;
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

    fn transition(&self, state: u32, byte: u8) -> Option<u32> {
        if state == 0 {
            let next = self.root_edges[byte as usize];
            (next != u32::MAX).then_some(next)
        } else {
            edge(&self.nodes[state as usize], byte)
        }
    }
}

fn multi_literal_min_literals() -> usize {
    4
}

fn multi_literal_min_total_bytes() -> usize {
    32
}

fn edge(node: &FinderNode, byte: u8) -> Option<u32> {
    node.edges
        .iter()
        .find_map(|(candidate, next)| (*candidate == byte).then_some(*next))
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

fn find_byte_set(haystack: &[u8], bytes: &[u8], bitmap: &[u64; 4]) -> Option<usize> {
    match bytes {
        [] => None,
        [byte] => memchr::memchr(*byte, haystack),
        [a, b] => memchr::memchr2(*a, *b, haystack),
        [a, b, c] => memchr::memchr3(*a, *b, *c, haystack),
        _ => haystack
            .iter()
            .position(|byte| bitmap[*byte as usize >> 6] & (1u64 << (*byte & 63)) != 0),
    }
}

fn find_literal(haystack: &str, needle: &str) -> Option<usize> {
    memchr::memmem::find(haystack.as_bytes(), needle.as_bytes())
}

fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    find_ignore_ascii_case(haystack, needle).is_some()
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
    hay.windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle))
}

/// Scan-local memo of required-literal occurrences, keyed by compiled-pattern
/// slot. Anchored pattern-set attempts ask the same rejection question at
/// monotonically increasing positions; remembering the next occurrence turns
/// the per-position tail search into an O(1) check. Rejection-only: a stale or
/// missing entry falls back to a fresh search, so false negatives are
/// impossible by construction.
#[derive(Debug, Clone, Default)]
pub(crate) struct PrefilterCursors {
    line_ptr: usize,
    line_len: usize,
    generation: u64,
    slots: Vec<CursorSlot>,
}

#[derive(Debug, Clone, Copy, Default)]
struct CursorSlot {
    generation: u64,
    searched_from: usize,
    next_occurrence: Option<usize>,
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
        let slot = slot as usize;
        let (line_ptr, line_len) = (line.as_ptr() as usize, line.len());
        if self.line_ptr != line_ptr || self.line_len != line_len {
            self.line_ptr = line_ptr;
            self.line_len = line_len;
            self.generation = self.generation.wrapping_add(1);
        }
        if slot >= self.slots.len() {
            self.slots.resize(slot + 1, CursorSlot::default());
        }
        // Generation zero marks never-written slots; never treat it as bound.
        if self.generation == 0 {
            self.generation = 1;
        }
        let generation = self.generation;
        let entry = &mut self.slots[slot];
        let stale = entry.generation != generation || entry.searched_from > start;
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
}

pub fn required_literal(pattern: &str) -> Option<String> {
    let parsed = super::ast::parse(pattern);
    match required_literals(&parsed.ast) {
        RequiredLiterals::One(literal) => Some(literal),
        RequiredLiterals::Any(literals) => literals.into_iter().max_by_key(|literal| literal.len()),
        RequiredLiterals::None => literal_prefix(pattern),
    }
}

pub fn required_literals(ast: &Ast) -> RequiredLiterals {
    if let Some(literal) = exact_literal(ast).filter(|literal| !literal.is_empty()) {
        return RequiredLiterals::One(literal);
    }
    match ast {
        Ast::Literal(literal) if !literal.is_empty() => RequiredLiterals::One(literal.clone()),
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

fn sequence_required_literals(nodes: &[Ast]) -> RequiredLiterals {
    let mut best = RequiredLiterals::None;
    let mut run = String::new();
    for node in nodes {
        if let Some(literal) = exact_literal(node) {
            run.push_str(&literal);
            continue;
        }
        if !run.is_empty() {
            best = choose_more_selective(best, RequiredLiterals::One(std::mem::take(&mut run)));
        }
        let candidate = required_literals(node);
        best = choose_more_selective(best, candidate);
    }
    if !run.is_empty() {
        best = choose_more_selective(best, RequiredLiterals::One(run));
    }
    best
}

fn exact_literal(ast: &Ast) -> Option<String> {
    match ast {
        Ast::Empty => Some(String::new()),
        Ast::Literal(literal) => Some(literal.clone()),
        Ast::Concat(nodes) => {
            let mut out = String::new();
            for node in nodes {
                out.push_str(&exact_literal(node)?);
            }
            Some(out)
        }
        Ast::Group { child, .. } | Ast::Flags { child, .. } => exact_literal(child),
        _ => None,
    }
}

fn alternation_required_literals(branches: &[Ast]) -> RequiredLiterals {
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

fn choose_more_selective(left: RequiredLiterals, right: RequiredLiterals) -> RequiredLiterals {
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
        RequiredLiterals::Any(literals) => literals.iter().map(String::len).max().unwrap_or(0),
    }
}

fn literal_cardinality(literals: &RequiredLiterals) -> usize {
    match literals {
        RequiredLiterals::None => usize::MAX,
        RequiredLiterals::One(_) => 1,
        RequiredLiterals::Any(literals) => literals.len(),
    }
}

fn class_required_literals(class: &CharClass) -> RequiredLiterals {
    if class.negated || class.atoms.is_empty() {
        return RequiredLiterals::None;
    }
    // Every intersection result is a subset of the first union. Its finite
    // literals therefore remain a safe (possibly broader) prefilter set.
    let mut literals = Vec::new();
    for atom in &class.atoms {
        match atom {
            ClassAtom::Char(ch) => literals.push(ch.to_string()),
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
    fn extracts_safe_literal_prefix() {
        assert_eq!(required_literal("foo|bar"), Some("foo".to_owned()));
        assert_eq!(required_literal(r"\w+"), None);
    }

    #[test]
    fn extracts_alternation_literals() {
        let parsed = parse("foo|bar");
        assert_eq!(
            required_literals(&parsed.ast),
            RequiredLiterals::Any(vec!["bar".to_owned(), "foo".to_owned()])
        );
    }

    #[test]
    fn extracts_positive_lookahead_literals() {
        let parsed = parse(r"(?<=return)\s*(?=(<)\s*([A-Za-z]+))");
        assert_eq!(
            required_literals(&parsed.ast),
            RequiredLiterals::One("<".to_owned())
        );

        let parsed = parse(r"(?<!\\)(?=;)");
        assert_eq!(
            required_literals(&parsed.ast),
            RequiredLiterals::One(";".to_owned())
        );

        let parsed = parse(r"(?<=return)");
        assert_eq!(required_literals(&parsed.ast), RequiredLiterals::None);
    }

    #[test]
    fn extracts_positive_class_literals() {
        let parsed = parse(r"(?=[;)])(?<!\\)");
        assert_eq!(
            required_literals(&parsed.ast),
            RequiredLiterals::Any(vec![")".to_owned(), ";".to_owned()])
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
    fn multi_literal_finder_preserves_leftmost_and_failure_outputs() {
        let literals = [
            "bc", "abcd", "suffix", "hers", "his", "she", "he", "keyword",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        let finder = MultiLiteralFinder::new(&literals);
        // `bc` ends before `abcd`, but the longer literal starts earlier.
        assert_eq!(finder.find(b"zabcd"), Some(1));
        // `he` is reached through the failure link after scanning `she`.
        assert_eq!(finder.find(b"ushers"), Some(1));
        assert_eq!(finder.find(b"nothing"), None);
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
