use std::{collections::HashSet, fmt, sync::Arc};

use super::ast::{AnchorKind, Ast, ClassAtom, LookKind, PerlClassKind, RegexFlags};
use super::backtrack::{FallbackMatcher, is_line_end_position};
use super::prefilter::LiteralSet;
use super::scanner::Scanner;
use super::translate::{AnchorStrategy, Translation, translate};
use super::{AnchorContext, MatchResult, Matcher, is_unicode_word_char};
use crate::engine::hashing::{FastMap, fast_map};

/// Build error for the native fast-path matcher. Kept as a distinct type so
/// call sites that previously handled regex-automata compile failures continue
/// to compile against a Display/Error value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomataBuildError {
    message: String,
}

impl AutomataBuildError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for AutomataBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AutomataBuildError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralMatcher {
    literal: String,
}

impl LiteralMatcher {
    pub fn new(literal: impl Into<String>) -> Self {
        Self {
            literal: literal.into(),
        }
    }
}

impl Matcher for LiteralMatcher {
    fn find(&self, line: &str, from: usize, _ctx: AnchorContext) -> Option<MatchResult> {
        if !line.is_char_boundary(from) {
            return None;
        }
        let haystack = line.get(from..)?;
        let offset = haystack.find(&self.literal)?;
        let start = from + offset;
        let end = start + self.literal.len();
        Some(MatchResult {
            start,
            end,
            captures: vec![Some(start..end)],
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimpleMatcher {
    Literal(String),
    LineStartLiteral(String),
    FileStartLiteral(String),
    ContinuationLiteral(String),
}

impl SimpleMatcher {
    pub fn from_pattern(pattern: &str) -> Self {
        if let Some(rest) = pattern.strip_prefix(r"\A") {
            Self::FileStartLiteral(unescape_literal(rest))
        } else if let Some(rest) = pattern.strip_prefix(r"\G") {
            Self::ContinuationLiteral(unescape_literal(rest))
        } else if let Some(rest) = pattern.strip_prefix('^') {
            Self::LineStartLiteral(unescape_literal(rest))
        } else {
            Self::Literal(unescape_literal(pattern))
        }
    }

    /// Build a simple matcher from a parsed AST when the pattern is a pure
    /// literal (optionally with a single leading anchor). Returns `None` when
    /// the pattern needs the general VM (classes, quantifiers, groups, …).
    pub fn try_from_translation(translation: &Translation) -> Option<Self> {
        if translation.parsed.capture_count > 0 {
            return None;
        }
        let flags = translation.parsed.flags;
        if flags.case_insensitive || flags.multi_line || flags.dot_matches_new_line {
            return None;
        }
        if matches!(translation.anchor_strategy, AnchorStrategy::Fallback) {
            return None;
        }
        let literal = pure_literal_body(&translation.parsed.ast)?;
        Some(match translation.anchor_strategy {
            AnchorStrategy::None => Self::Literal(literal),
            AnchorStrategy::LineStartGuard => Self::LineStartLiteral(literal),
            AnchorStrategy::TextStartGuard => Self::FileStartLiteral(literal),
            AnchorStrategy::ContinuationGuard => Self::ContinuationLiteral(literal),
            AnchorStrategy::Fallback => return None,
        })
    }

    fn match_at(&self, line: &str, start: usize) -> Option<MatchResult> {
        let literal = match self {
            Self::Literal(literal)
            | Self::LineStartLiteral(literal)
            | Self::FileStartLiteral(literal)
            | Self::ContinuationLiteral(literal) => literal,
        };
        let end = start.checked_add(literal.len())?;
        (line.get(start..end)? == literal).then(|| MatchResult {
            start,
            end,
            captures: vec![Some(start..end)],
        })
    }

    fn unanchored_literal(&self) -> Option<&str> {
        match self {
            Self::Literal(literal) => Some(literal),
            Self::LineStartLiteral(_)
            | Self::FileStartLiteral(_)
            | Self::ContinuationLiteral(_) => None,
        }
    }
}

impl Matcher for SimpleMatcher {
    fn find(&self, line: &str, from: usize, ctx: AnchorContext) -> Option<MatchResult> {
        if !line.is_char_boundary(from) {
            return None;
        }
        match self {
            Self::Literal(literal) => {
                let haystack = line.get(from..)?;
                let offset = haystack.find(literal)?;
                self.match_at(line, from + offset)
            }
            Self::LineStartLiteral(_) => {
                if from == 0 {
                    self.match_at(line, 0)
                } else {
                    None
                }
            }
            Self::FileStartLiteral(_) => {
                if ctx.allow_a && from == 0 {
                    self.match_at(line, 0)
                } else {
                    None
                }
            }
            Self::ContinuationLiteral(_) => {
                if ctx.allow_g && ctx.g_pos >= from && line.is_char_boundary(ctx.g_pos) {
                    self.match_at(line, ctx.g_pos)
                } else {
                    None
                }
            }
        }
    }
}

/// Pure-literal body of an AST that is at most `anchor? · literal*`.
fn pure_literal_body(ast: &Ast) -> Option<String> {
    match ast {
        Ast::Empty => Some(String::new()),
        Ast::Literal(literal) => Some(literal.clone()),
        Ast::Concat(nodes) => {
            let mut out = String::new();
            let mut saw_non_anchor = false;
            for node in nodes {
                match node {
                    Ast::Empty => {}
                    Ast::Literal(literal) => {
                        saw_non_anchor = true;
                        out.push_str(literal);
                    }
                    Ast::Anchor(kind)
                        if !saw_non_anchor
                            && matches!(
                                kind,
                                AnchorKind::LineStart
                                    | AnchorKind::TextStart
                                    | AnchorKind::Continuation
                            ) => {}
                    _ => return None,
                }
            }
            Some(out)
        }
        Ast::Anchor(AnchorKind::LineStart | AnchorKind::TextStart | AnchorKind::Continuation) => {
            Some(String::new())
        }
        _ => None,
    }
}

#[derive(Debug, Clone)]
enum NativeEngine {
    Simple(SimpleMatcher),
    WordSet(WordSetMatcher),
    SymbolSet(SymbolSetMatcher),
    DelimitedFields(DelimitedFieldsMatcher),
    NixUri(NixUriMatcher),
    /// General native VM for DFA-routable (and best-effort) patterns.
    Vm(FallbackMatcher),
}

#[derive(Debug, Clone)]
struct SymbolSetMatcher {
    buckets: Vec<SymbolBucket>,
    start_bytes: Vec<u8>,
    start_bitmap: [u64; 4],
    left: Separator,
    right: Separator,
    leading_word_boundary: bool,
    trailing_word_boundary: bool,
    capture_group: Option<u32>,
    capture_count: usize,
}

#[derive(Debug, Clone)]
struct SymbolBucket {
    len: usize,
    symbols: FastMap<Vec<u8>, usize>,
}

#[derive(Debug, Clone, Default)]
struct Separator {
    chars: Vec<char>,
    whitespace: bool,
    line_boundary: bool,
}

impl SymbolSetMatcher {
    fn try_from_translation(translation: &Translation) -> Option<Self> {
        if !matches!(
            translation.anchor_strategy,
            AnchorStrategy::Fallback | AnchorStrategy::None
        ) || translation.parsed.flags != RegexFlags::default()
        {
            return None;
        }
        let Ast::Concat(nodes) = &translation.parsed.ast else {
            return None;
        };
        let nodes = nodes
            .iter()
            .filter(|node| !matches!(node, Ast::Empty))
            .collect::<Vec<_>>();
        let mut cursor = 0;
        let leading_word_boundary = nodes
            .get(cursor)
            .is_some_and(|node| matches!(node, Ast::Anchor(AnchorKind::WordBoundary)));
        cursor += usize::from(leading_word_boundary);
        let left = separator_look(nodes.get(cursor).copied()?, LookKind::Behind)?;
        cursor += 1;
        let body = nodes.get(cursor).copied()?;
        let (capture_group, child) = match body {
            Ast::Group {
                index,
                name: None,
                child,
            } => (*index, child.as_ref()),
            Ast::Alternation(_) => (None, body),
            _ if translation.parsed.capture_count == 0 => (None, body),
            _ => return None,
        };
        cursor += 1;
        let right = separator_look(nodes.get(cursor).copied()?, LookKind::Ahead)?;
        cursor += 1;
        let trailing_word_boundary = nodes
            .get(cursor)
            .is_some_and(|node| matches!(node, Ast::Anchor(AnchorKind::WordBoundary)));
        cursor += usize::from(trailing_word_boundary);
        if cursor != nodes.len() {
            return None;
        }

        let variants = symbol_variants(child, 131_072)?;
        if variants.len() < 32 || variants.iter().any(String::is_empty) {
            return None;
        }
        let mut buckets = Vec::<SymbolBucket>::new();
        let mut start_bitmap = [0u64; 4];
        for (order, symbol) in variants.into_iter().enumerate() {
            let bytes = symbol.into_bytes();
            if let Some(first) = bytes.first().copied() {
                start_bitmap[first as usize >> 6] |= 1u64 << (first & 63);
            }
            let len = bytes.len();
            match buckets.binary_search_by_key(&len, |bucket| bucket.len) {
                Ok(index) => {
                    buckets[index].symbols.entry(bytes).or_insert(order);
                }
                Err(index) => {
                    let mut symbols = fast_map();
                    symbols.insert(bytes, order);
                    buckets.insert(index, SymbolBucket { len, symbols });
                }
            }
        }
        Some(Self {
            buckets,
            start_bytes: (0u8..=u8::MAX)
                .filter(|byte| start_bitmap[*byte as usize >> 6] & (1u64 << (*byte & 63)) != 0)
                .collect(),
            start_bitmap,
            left,
            right,
            leading_word_boundary,
            trailing_word_boundary,
            capture_group,
            capture_count: translation.parsed.capture_count as usize + 1,
        })
    }

    fn match_at(&self, line: &str, start: usize) -> Option<MatchResult> {
        if !line.is_char_boundary(start)
            || !self.left.matches_before(line, start)
            || (self.leading_word_boundary && !word_boundary_at(line, start))
        {
            return None;
        }
        let bytes = line.as_bytes();
        let mut selected = None;
        for bucket in &self.buckets {
            let end = start.checked_add(bucket.len)?;
            let Some(candidate) = bytes.get(start..end) else {
                continue;
            };
            let Some(&order) = bucket.symbols.get(candidate) else {
                continue;
            };
            if line.is_char_boundary(end)
                && self.right.matches_after(line, end)
                && (!self.trailing_word_boundary || word_boundary_at(line, end))
                && selected.is_none_or(|(best, _)| order < best)
            {
                selected = Some((order, end));
            }
        }
        let (_, end) = selected?;
        let mut captures = vec![None; self.capture_count];
        captures[0] = Some(start..end);
        if let Some(capture_group) = self.capture_group {
            captures[capture_group as usize] = Some(start..end);
        }
        Some(MatchResult {
            start,
            end,
            captures,
        })
    }

    fn find(&self, line: &str, from: usize) -> Option<MatchResult> {
        if !line.is_char_boundary(from) {
            return None;
        }
        let bytes = line.as_bytes();
        let mut start = from;
        while start < bytes.len() {
            let offset = bytes[start..].iter().position(|byte| {
                self.start_bitmap[*byte as usize >> 6] & (1u64 << (*byte & 63)) != 0
            })?;
            start += offset;
            if let Some(result) = self.match_at(line, start) {
                return Some(result);
            }
            start += 1;
            while start < bytes.len() && !line.is_char_boundary(start) {
                start += 1;
            }
        }
        None
    }
}

impl Separator {
    fn matches_before(&self, line: &str, position: usize) -> bool {
        (position == 0 && self.line_boundary)
            || previous_char(line, position).is_some_and(|ch| self.matches_char(ch))
    }

    fn matches_after(&self, line: &str, position: usize) -> bool {
        (self.line_boundary && is_line_end_position(line, position))
            || char_at(line, position).is_some_and(|(ch, _)| self.matches_char(ch))
    }

    fn matches_char(&self, ch: char) -> bool {
        self.chars.contains(&ch) || (self.whitespace && ch.is_whitespace())
    }
}

fn separator_look(ast: &Ast, expected: LookKind) -> Option<Separator> {
    let Ast::Look { kind, child } = ast else {
        return None;
    };
    if *kind != expected {
        return None;
    }
    let branches = match child.as_ref() {
        Ast::Alternation(branches) => branches.as_slice(),
        branch => std::slice::from_ref(branch),
    };
    let mut separator = Separator::default();
    for branch in branches {
        match branch {
            Ast::Anchor(
                AnchorKind::LineStart
                | AnchorKind::TextStart
                | AnchorKind::LineEnd
                | AnchorKind::TextEnd
                | AnchorKind::TextEndOrFinalNewline,
            ) => separator.line_boundary = true,
            Ast::Class(class) if !class.negated && class.intersections.is_empty() => {
                for atom in &class.atoms {
                    match atom {
                        ClassAtom::Char(ch) => separator.chars.push(*ch),
                        ClassAtom::Perl(PerlClassKind::Space) => separator.whitespace = true,
                        _ => return None,
                    }
                }
            }
            _ => return None,
        }
    }
    Some(separator)
}

fn symbol_variants(ast: &Ast, limit: usize) -> Option<Vec<String>> {
    match ast {
        Ast::Empty => Some(vec![String::new()]),
        Ast::Literal(literal) => Some(vec![literal.clone()]),
        Ast::Group {
            child, name: None, ..
        }
        | Ast::Flags { child, .. } => symbol_variants(child, limit),
        Ast::Class(class) if !class.negated && class.intersections.is_empty() => class
            .atoms
            .iter()
            .map(|atom| match atom {
                ClassAtom::Char(ch) => Some(ch.to_string()),
                ClassAtom::Range(start, end) if start.is_ascii() && end.is_ascii() => Some(
                    ((*start as u8)..=(*end as u8))
                        .map(char::from)
                        .collect::<String>(),
                ),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
            .map(|parts| parts.concat().chars().map(|ch| ch.to_string()).collect()),
        Ast::Alternation(branches) => {
            let mut variants = Vec::new();
            for branch in branches {
                variants.extend(symbol_variants(branch, limit)?);
                if variants.len() > limit {
                    return None;
                }
            }
            Some(variants)
        }
        Ast::Concat(nodes) => {
            let mut variants = vec![String::new()];
            for node in nodes {
                let part = symbol_variants(node, limit)?;
                if variants.len().saturating_mul(part.len()) > limit {
                    return None;
                }
                let mut next = Vec::with_capacity(variants.len() * part.len());
                for prefix in &variants {
                    for suffix in &part {
                        let mut combined = prefix.clone();
                        combined.push_str(suffix);
                        next.push(combined);
                    }
                }
                variants = next;
            }
            Some(variants)
        }
        Ast::Repeat {
            node,
            min,
            max: Some(max),
            possessive: false,
            atomic: false,
            ..
        } if max.saturating_sub(*min) <= 2 && *max <= 3 => {
            let atoms = symbol_variants(node, limit)?;
            let mut variants = Vec::new();
            for count in *min..=*max {
                let mut repeated = vec![String::new()];
                for _ in 0..count {
                    if repeated.len().saturating_mul(atoms.len()) > limit {
                        return None;
                    }
                    repeated = repeated
                        .iter()
                        .flat_map(|prefix| {
                            atoms.iter().map(move |suffix| {
                                let mut value = prefix.clone();
                                value.push_str(suffix);
                                value
                            })
                        })
                        .collect();
                }
                variants.extend(repeated);
            }
            (variants.len() <= limit).then_some(variants)
        }
        _ => None,
    }
}

fn word_boundary_at(line: &str, position: usize) -> bool {
    previous_char(line, position).is_some_and(is_word_char)
        != char_at(line, position).is_some_and(|(ch, _)| is_word_char(ch))
}

/// Specialized matcher for fixed-width delimiter grammars such as TSV's
/// `([^\t]*\t?)([^\t]*\t?)...`.  A chain of nullable greedy fields has an
/// exponential number of equivalent VM paths when later fields may also be
/// empty.  Oniguruma handles that shape efficiently, but exploring those
/// paths in the general bounded VM can consume the whole per-call budget.
///
/// Recognizing the AST (rather than a particular source spelling) makes this
/// reusable for CSV-like TextMate grammars while preserving each field's
/// capture, including its optional trailing delimiter.
#[derive(Debug, Clone, Copy)]
struct DelimitedFieldsMatcher {
    delimiter: u8,
    fields: usize,
    capture_count: usize,
}

impl DelimitedFieldsMatcher {
    fn try_from_translation(translation: &Translation) -> Option<Self> {
        if !matches!(translation.anchor_strategy, AnchorStrategy::None)
            || translation.parsed.flags != RegexFlags::default()
        {
            return None;
        }
        let Ast::Concat(fields) = &translation.parsed.ast else {
            return None;
        };
        if fields.len() < 2 {
            return None;
        }
        let mut delimiter = None;
        for (offset, field) in fields.iter().enumerate() {
            let Ast::Group {
                index: Some(index),
                name: None,
                child,
            } = field
            else {
                return None;
            };
            if usize::try_from(*index).ok()? != offset + 1 {
                return None;
            }
            let Ast::Concat(parts) = child.as_ref() else {
                return None;
            };
            let [body, suffix] = parts.as_slice() else {
                return None;
            };
            let Ast::Repeat {
                node: body,
                min: 0,
                max: None,
                greedy: true,
                possessive: false,
                atomic: false,
            } = body
            else {
                return None;
            };
            let Ast::Class(class) = body.as_ref() else {
                return None;
            };
            let [ClassAtom::Char(excluded)] = class.atoms.as_slice() else {
                return None;
            };
            if !class.negated || !class.intersections.is_empty() || !excluded.is_ascii() {
                return None;
            }
            let Ast::Repeat {
                node: suffix,
                min: 0,
                max: Some(1),
                greedy: true,
                possessive: false,
                atomic: false,
            } = suffix
            else {
                return None;
            };
            let Ast::Literal(literal) = suffix.as_ref() else {
                return None;
            };
            let bytes = literal.as_bytes();
            if bytes.len() != 1 || bytes[0] != *excluded as u8 {
                return None;
            }
            if delimiter
                .replace(bytes[0])
                .is_some_and(|seen| seen != bytes[0])
            {
                return None;
            }
        }
        Some(Self {
            delimiter: delimiter?,
            fields: fields.len(),
            capture_count: translation.parsed.capture_count as usize + 1,
        })
    }

    fn match_at(&self, line: &str, start: usize) -> Option<MatchResult> {
        if !line.is_char_boundary(start) {
            return None;
        }
        let bytes = line.as_bytes();
        let mut position = start;
        let mut captures = vec![None; self.capture_count];
        for capture in captures.iter_mut().take(self.fields + 1).skip(1) {
            let field_start = position;
            position += memchr::memchr(self.delimiter, &bytes[position..])
                .map_or(bytes.len() - position, |offset| offset + 1);
            *capture = Some(field_start..position);
        }
        captures[0] = Some(start..position);
        Some(MatchResult {
            start,
            end: position,
            captures,
        })
    }

    fn find(&self, line: &str, from: usize) -> Option<MatchResult> {
        // Every recognized field is nullable, so the leftmost match is always
        // exactly `from`.
        self.match_at(line, from)
    }
}

/// Native matcher for the Nix unquoted URI production
/// `([A-Za-z][-+.0-9A-Za-z]*:[!$-'*-:=?-Z_a-z~]+)`. The generic VM is
/// correct but hot on Nix files because every identifier-like token probes for
/// a following `:`. This keeps the exact ASCII byte language and capture shape
/// while reducing each negative probe to a tight byte scan.
#[derive(Debug, Clone, Copy)]
struct NixUriMatcher;

impl NixUriMatcher {
    const PATTERN: &'static str = "([A-Za-z][-+.0-9A-Za-z]*:[!$-'*-:=?-Z_a-z~]+)";

    fn try_from_translation(translation: &Translation) -> Option<Self> {
        (translation.pattern == Self::PATTERN).then_some(Self)
    }

    fn find(&self, line: &str, from: usize) -> Option<MatchResult> {
        let bytes = line.as_bytes();
        let mut pos = from;
        while pos < bytes.len() {
            let found = memchr::memchr2(b':', b'<', &bytes[pos..])
                .map(|offset| pos + offset)
                .unwrap_or(bytes.len());
            // A colon is required. If the next potentially interesting byte is
            // not a colon, continue after it (notably over Nix `<spath>`s).
            if found >= bytes.len() {
                return None;
            }
            if bytes[found] != b':' {
                pos = found + 1;
                continue;
            }
            let mut run_start = found;
            while run_start > from && is_nix_uri_scheme_byte(bytes[run_start - 1]) {
                run_start -= 1;
            }
            for (start, byte) in bytes.iter().enumerate().take(found).skip(run_start) {
                if is_ascii_alpha(*byte)
                    && line.is_char_boundary(start)
                    && let Some(result) = self.match_at(line, start)
                {
                    return Some(result);
                }
            }
            pos = found + 1;
        }
        None
    }

    fn match_at(&self, line: &str, start: usize) -> Option<MatchResult> {
        let bytes = line.as_bytes();
        let first = *bytes.get(start)?;
        if !is_ascii_alpha(first) || !line.is_char_boundary(start) {
            return None;
        }
        let mut pos = start + 1;
        while pos < bytes.len() && is_nix_uri_scheme_byte(bytes[pos]) {
            pos += 1;
        }
        if bytes.get(pos).copied() != Some(b':') {
            return None;
        }
        pos += 1;
        let body_start = pos;
        while pos < bytes.len() && is_nix_uri_body_byte(bytes[pos]) {
            pos += 1;
        }
        if pos == body_start || !line.is_char_boundary(pos) {
            return None;
        }
        Some(MatchResult {
            start,
            end: pos,
            captures: vec![Some(start..pos), Some(start..pos)],
        })
    }
}

fn is_ascii_alpha(byte: u8) -> bool {
    byte.is_ascii_alphabetic()
}

fn is_nix_uri_scheme_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'+' | b'.')
}

fn is_nix_uri_body_byte(byte: u8) -> bool {
    matches!(byte, b'!' | b'$'..=b'\'' | b'*'..=b':' | b'=' | b'?'..=b'Z' | b'_' | b'a'..=b'z' | b'~')
}

/// Specialized matcher for the common keyword-list spelling
/// `\b(?i)(foo|bar|...)\b` / `(?i:\b(?:foo|bar|...)\b)`, plus the SQL
/// function-list suffix `\s*\(`.
#[derive(Debug, Clone)]
struct WordSetMatcher {
    buckets: Vec<WordBucket>,
    case_insensitive: bool,
    suffix: WordSetSuffix,
    start_bytes: Vec<u8>,
    capture_group: Option<u32>,
    capture_count: usize,
}

#[derive(Debug, Clone)]
struct WordBucket {
    len: usize,
    words: HashSet<Vec<u8>>,
}

#[derive(Debug, Clone)]
struct WordSetSpec {
    words: Vec<String>,
    case_insensitive: bool,
    suffix: WordSetSuffix,
    capture_group: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WordSetSuffix {
    None,
    OpenParenAfterOptionalSpace,
}

impl WordSetMatcher {
    fn try_from_translation(translation: &Translation) -> Option<Self> {
        if !matches!(translation.anchor_strategy, AnchorStrategy::None) {
            return None;
        }
        let spec = word_set_spec(&translation.parsed.ast, translation.parsed.flags)?;
        if spec.words.len() < word_set_min_words() {
            return None;
        }
        Some(Self::new(
            spec.words,
            spec.case_insensitive,
            spec.suffix,
            spec.capture_group,
            translation.parsed.capture_count as usize + 1,
        ))
    }

    fn new(
        words: Vec<String>,
        case_insensitive: bool,
        suffix: WordSetSuffix,
        capture_group: Option<u32>,
        capture_count: usize,
    ) -> Self {
        let mut buckets = Vec::<WordBucket>::new();
        let mut start_bitmap = [0u64; 4];
        for word in words {
            debug_assert!(!word.is_empty());
            debug_assert!(word.bytes().all(is_ascii_word_byte));
            let mut bytes = word.into_bytes();
            if case_insensitive {
                bytes.make_ascii_lowercase();
            }
            if let Some(&first) = bytes.first() {
                if case_insensitive && first.is_ascii_alphabetic() {
                    let lower = first.to_ascii_lowercase();
                    let upper = first.to_ascii_uppercase();
                    start_bitmap[lower as usize >> 6] |= 1u64 << (lower & 63);
                    start_bitmap[upper as usize >> 6] |= 1u64 << (upper & 63);
                } else {
                    start_bitmap[first as usize >> 6] |= 1u64 << (first & 63);
                }
            }
            let len = bytes.len();
            match buckets.binary_search_by_key(&len, |bucket| bucket.len) {
                Ok(index) => {
                    buckets[index].words.insert(bytes);
                }
                Err(index) => {
                    let mut set = HashSet::new();
                    set.insert(bytes);
                    buckets.insert(index, WordBucket { len, words: set });
                }
            }
        }
        let start_bytes = (0u8..=u8::MAX)
            .filter(|byte| start_bitmap[*byte as usize >> 6] & (1u64 << (*byte & 63)) != 0)
            .collect();
        Self {
            buckets,
            case_insensitive,
            suffix,
            start_bytes,
            capture_group,
            capture_count,
        }
    }

    fn find(&self, line: &str, from: usize) -> Option<MatchResult> {
        if !line.is_char_boundary(from) {
            return None;
        }
        let bytes = line.as_bytes();
        let mut position = from;
        while position < bytes.len() {
            let offset = bytes[position..]
                .iter()
                .position(|byte| is_ascii_word_byte(*byte))?;
            position += offset;
            if previous_char(line, position).is_some_and(is_word_char) {
                position = ascii_word_end(bytes, position);
                continue;
            }
            let end = ascii_word_end(bytes, position);
            if char_at(line, end).is_some_and(|(ch, _)| is_word_char(ch)) {
                position = end;
                continue;
            }
            if self.contains(&bytes[position..end])
                && let Some(match_end) = self.suffix_end(line, end)
            {
                return Some(self.match_result(position, end, match_end));
            }
            position = end;
        }
        None
    }

    fn match_at(&self, line: &str, start: usize) -> Option<MatchResult> {
        if start > line.len() || !line.is_char_boundary(start) {
            return None;
        }
        let bytes = line.as_bytes();
        let first = *bytes.get(start)?;
        if !is_ascii_word_byte(first) || previous_char(line, start).is_some_and(is_word_char) {
            return None;
        }
        let end = ascii_word_end(bytes, start);
        if char_at(line, end).is_some_and(|(ch, _)| is_word_char(ch)) {
            return None;
        }
        if !self.contains(&bytes[start..end]) {
            return None;
        }
        let match_end = self.suffix_end(line, end)?;
        Some(self.match_result(start, end, match_end))
    }

    fn contains(&self, token: &[u8]) -> bool {
        let Ok(index) = self
            .buckets
            .binary_search_by_key(&token.len(), |bucket| bucket.len)
        else {
            return false;
        };
        if self.case_insensitive {
            let mut folded = token.to_vec();
            folded.make_ascii_lowercase();
            self.buckets[index].words.contains(&folded)
        } else {
            self.buckets[index].words.contains(token)
        }
    }

    fn suffix_end(&self, line: &str, word_end: usize) -> Option<usize> {
        match self.suffix {
            WordSetSuffix::None => Some(word_end),
            WordSetSuffix::OpenParenAfterOptionalSpace => {
                let mut position = word_end;
                while let Some((ch, next)) = char_at(line, position) {
                    if !ch.is_whitespace() {
                        break;
                    }
                    position = next;
                }
                (line.as_bytes().get(position) == Some(&b'(')).then_some(position + 1)
            }
        }
    }

    fn match_result(&self, start: usize, word_end: usize, match_end: usize) -> MatchResult {
        let mut captures = vec![None; self.capture_count];
        captures[0] = Some(start..match_end);
        if let Some(group) = self.capture_group
            && let Some(slot) = captures.get_mut(group as usize)
        {
            *slot = Some(start..word_end);
        }
        MatchResult {
            start,
            end: match_end,
            captures,
        }
    }
}

fn word_set_min_words() -> usize {
    4
}

fn word_set_spec(ast: &Ast, flags: RegexFlags) -> Option<WordSetSpec> {
    match ast {
        Ast::Flags {
            flags: scoped_flags,
            child,
        } => word_set_spec(child, *scoped_flags),
        Ast::Concat(nodes) => {
            // A bare option switch scopes the remainder of its enclosing
            // subexpression. `\b(?i)(foo|bar)\b` therefore has a default
            // boundary prefix followed by one insensitive remainder node.
            if let [
                prefix,
                Ast::Flags {
                    flags: scoped,
                    child,
                },
            ] = nodes.as_slice()
                && let Ast::Concat(remainder) = child.as_ref()
            {
                let mut flattened = Vec::with_capacity(remainder.len() + 1);
                flattened.push(prefix.clone());
                flattened.extend(remainder.iter().cloned());
                return word_set_spec_from_concat(&flattened, *scoped);
            }
            word_set_spec_from_concat(nodes, flags)
        }
        _ => None,
    }
}

fn word_set_spec_from_concat(nodes: &[Ast], flags: RegexFlags) -> Option<WordSetSpec> {
    let significant = nodes
        .iter()
        .filter(|node| !matches!(node, Ast::Empty))
        .collect::<Vec<_>>();
    if significant.len() < 3 {
        return None;
    }
    let mut effective_flags = snapshot_flags(significant[0]).unwrap_or(flags);
    if !matches!(
        unwrap_snapshot(significant[0]),
        Ast::Anchor(AnchorKind::WordBoundary)
    ) {
        return None;
    }
    if !matches!(
        unwrap_snapshot(significant[2]),
        Ast::Anchor(AnchorKind::WordBoundary)
    ) {
        return None;
    }
    let mut body = unwrap_snapshot(significant[1]);
    if let Ast::Flags {
        flags: scoped_flags,
        child,
    } = body
    {
        effective_flags = *scoped_flags;
        body = child.as_ref();
    } else if let Some(scoped_flags) = uniform_snapshot_flags(body) {
        if snapshot_flags(significant[0]).is_some() && effective_flags != scoped_flags {
            return None;
        }
        effective_flags = scoped_flags;
    } else if contains_snapshot(body) {
        return None;
    }
    let suffix = word_set_suffix(&significant[3..])?;
    let (alternation, capture_group) = match body {
        Ast::Group {
            index,
            child,
            name: None,
        } => (child.as_ref(), *index),
        Ast::Alternation(_) => (body, None),
        _ => return None,
    };
    let Ast::Alternation(branches) = alternation else {
        return None;
    };
    let mut words = Vec::with_capacity(branches.len());
    for branch in branches {
        let variants = word_variants(branch)?;
        for word in variants {
            if word.is_empty() || !word.bytes().all(is_ascii_word_byte) {
                return None;
            }
            words.push(word);
        }
    }
    Some(WordSetSpec {
        words,
        case_insensitive: effective_flags.case_insensitive,
        suffix,
        capture_group,
    })
}

fn uniform_snapshot_flags(ast: &Ast) -> Option<RegexFlags> {
    match ast {
        Ast::Flags { flags, .. } => Some(*flags),
        Ast::Alternation(nodes) | Ast::Concat(nodes) => {
            let mut flags = None;
            for node in nodes {
                let node_flags = uniform_snapshot_flags(node)?;
                if flags.is_some_and(|flags| flags != node_flags) {
                    return None;
                }
                flags = Some(node_flags);
            }
            flags
        }
        Ast::Group { child, .. } => uniform_snapshot_flags(child),
        _ => None,
    }
}

fn snapshot_flags(ast: &Ast) -> Option<RegexFlags> {
    let Ast::Flags { flags, .. } = ast else {
        return None;
    };
    Some(*flags)
}

fn unwrap_snapshot(ast: &Ast) -> &Ast {
    if let Ast::Flags { child, .. } = ast {
        child
    } else {
        ast
    }
}

fn contains_snapshot(ast: &Ast) -> bool {
    match ast {
        Ast::Flags { .. } => true,
        Ast::Concat(nodes) | Ast::Alternation(nodes) => nodes.iter().any(contains_snapshot),
        Ast::Repeat { node, .. }
        | Ast::Group { child: node, .. }
        | Ast::Look { child: node, .. } => contains_snapshot(node),
        Ast::Conditional {
            matched, unmatched, ..
        } => contains_snapshot(matched) || contains_snapshot(unmatched),
        _ => false,
    }
}

fn word_set_suffix(nodes: &[&Ast]) -> Option<WordSetSuffix> {
    let nodes = nodes
        .iter()
        .map(|node| unwrap_snapshot(node))
        .collect::<Vec<_>>();
    match nodes.as_slice() {
        [] => Some(WordSetSuffix::None),
        [
            Ast::Repeat {
                node,
                min: 0,
                max: None,
                possessive: false,
                ..
            },
            Ast::Literal(literal),
        ] if literal == "(" && is_perl_space_class(unwrap_snapshot(node)) => {
            Some(WordSetSuffix::OpenParenAfterOptionalSpace)
        }
        _ => None,
    }
}

fn is_perl_space_class(ast: &Ast) -> bool {
    let Ast::Class(class) = ast else {
        return false;
    };
    !class.negated
        && class.intersections.is_empty()
        && matches!(
            class.atoms.as_slice(),
            [ClassAtom::Perl(PerlClassKind::Space)]
        )
}

fn word_variants(ast: &Ast) -> Option<Vec<String>> {
    match ast {
        Ast::Empty => Some(vec![String::new()]),
        Ast::Literal(literal) => Some(vec![literal.clone()]),
        Ast::Group {
            child, name: None, ..
        } => word_variants(child),
        Ast::Flags { child, .. } => word_variants(child),
        Ast::Concat(nodes) => {
            let mut variants = vec![String::new()];
            for node in nodes {
                let part = word_variants(node)?;
                if variants.len().saturating_mul(part.len()) > 8 {
                    return None;
                }
                let mut next = Vec::with_capacity(variants.len() * part.len());
                for prefix in &variants {
                    for suffix in &part {
                        let mut combined = prefix.clone();
                        combined.push_str(suffix);
                        next.push(combined);
                    }
                }
                variants = next;
            }
            Some(variants)
        }
        Ast::Repeat {
            node,
            min: 0,
            max: Some(1),
            possessive: false,
            atomic: false,
            ..
        } => {
            let mut variants = vec![String::new()];
            variants.extend(word_variants(node)?);
            Some(variants)
        }
        _ => None,
    }
}

fn is_ascii_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn ascii_word_end(bytes: &[u8], start: usize) -> usize {
    let mut end = start;
    while bytes.get(end).is_some_and(|byte| is_ascii_word_byte(*byte)) {
        end += 1;
    }
    end
}

fn char_at(line: &str, pos: usize) -> Option<(char, usize)> {
    let ch = line.get(pos..)?.chars().next()?;
    Some((ch, pos + ch.len_utf8()))
}

fn previous_char(line: &str, pos: usize) -> Option<char> {
    line.get(..pos)?.chars().next_back()
}

fn is_word_char(ch: char) -> bool {
    is_unicode_word_char(ch)
}

/// Native "fast path" matcher. Previously backed by regex-automata; now uses
/// literal specializations plus the AST fallback VM
/// with a required-literal prefilter.
#[derive(Debug, Clone)]
pub struct AutomataMatcher {
    engine: NativeEngine,
    translation: Translation,
    generic_prefilter: bool,
}

impl AutomataMatcher {
    pub fn new(pattern: &str) -> Result<Self, AutomataBuildError> {
        let translation = translate(pattern);
        Self::from_translation(translation)
    }

    pub fn from_translation(translation: Translation) -> Result<Self, AutomataBuildError> {
        let engine = specialized_engine(&translation).unwrap_or_else(|| {
            // Match against the original Oniguruma pattern so anchors/classes
            // keep AST semantics (the translated spelling is diagnostic-only).
            NativeEngine::Vm(FallbackMatcher::from_parsed(
                Arc::clone(&translation.parsed),
                super::backtrack::DEFAULT_STEP_BUDGET,
            ))
        });
        // Native specializations already carry an exact literal/start-byte
        // search, so only VM-backed matchers consult the shared generic
        // prefilter retained by the immutable analysis.
        let generic_prefilter = matches!(&engine, NativeEngine::Vm(_));
        if !generic_prefilter {
            translation.parsed.initialize_analysis();
        }
        Ok(Self {
            engine,
            translation,
            generic_prefilter,
        })
    }

    /// Build only when a bounded native specialization recognizes the AST.
    /// This lets fallback-routed patterns (notably fixed-width separator
    /// lookarounds around huge symbol inventories) bypass the general VM
    /// without relabeling every fallback expression as an automata matcher.
    pub(crate) fn from_specialized_translation(translation: Translation) -> Option<Self> {
        let engine = specialized_engine(&translation)?;
        translation.parsed.initialize_analysis();
        Some(Self {
            engine,
            translation,
            generic_prefilter: false,
        })
    }

    pub fn translation(&self) -> &Translation {
        &self.translation
    }

    pub fn is_simple(&self) -> bool {
        matches!(self.engine, NativeEngine::Simple(_))
    }

    pub fn is_word_set(&self) -> bool {
        matches!(self.engine, NativeEngine::WordSet(_))
    }

    pub(crate) fn unanchored_literal(&self) -> Option<&str> {
        match &self.engine {
            NativeEngine::Simple(matcher) => matcher.unanchored_literal(),
            NativeEngine::WordSet(_) => None,
            NativeEngine::SymbolSet(_) => None,
            NativeEngine::DelimitedFields(_) => None,
            NativeEngine::NixUri(_) => None,
            NativeEngine::Vm(_) => None,
        }
    }

    pub(crate) fn restricted_start_bytes(&self) -> Option<Vec<u8>> {
        match &self.engine {
            NativeEngine::Simple(matcher) => matcher
                .unanchored_literal()
                .and_then(|literal| literal.as_bytes().first().copied())
                .map(|byte| vec![byte]),
            NativeEngine::WordSet(matcher) => Some(matcher.start_bytes.clone()),
            NativeEngine::SymbolSet(matcher) => Some(matcher.start_bytes.clone()),
            NativeEngine::DelimitedFields(_) => None,
            NativeEngine::NixUri(_) => Some((b'A'..=b'Z').chain(b'a'..=b'z').collect()),
            NativeEngine::Vm(matcher) => matcher.restricted_start_bytes(),
        }
    }

    pub fn prefilter_may_match(&self, line: &str, from: usize) -> Option<bool> {
        let prefilter = self
            .generic_prefilter
            .then(|| self.translation.parsed.prefilter())?;
        prefilter
            .is_enabled()
            .then(|| prefilter.may_match(line, from))
    }

    pub(crate) fn find_report_for_selection(
        &self,
        line: &str,
        from: usize,
        ctx: AnchorContext,
    ) -> Result<(Option<MatchResult>, Option<usize>), super::FallbackError> {
        match &self.engine {
            NativeEngine::Simple(matcher) => Ok((matcher.find(line, from, ctx), None)),
            NativeEngine::WordSet(matcher) => Ok((matcher.find(line, from), None)),
            NativeEngine::SymbolSet(matcher) => Ok((matcher.find(line, from), None)),
            NativeEngine::DelimitedFields(matcher) => Ok((matcher.find(line, from), None)),
            NativeEngine::NixUri(matcher) => Ok((matcher.find(line, from), None)),
            NativeEngine::Vm(matcher) => {
                if !anchor_permits_search(self.translation.anchor_strategy, from, ctx, line) {
                    return Ok((None, None));
                }
                matcher
                    .try_find_for_selection(line, from, ctx)
                    .map(|report| (report.result, Some(report.steps)))
            }
        }
    }

    pub(crate) fn find_report_at(
        &self,
        line: &str,
        start: usize,
        ctx: AnchorContext,
    ) -> Result<(Option<MatchResult>, Option<usize>), super::FallbackError> {
        if !anchor_permits_at(self.translation.anchor_strategy, start, ctx, line) {
            return Ok((None, None));
        }
        match &self.engine {
            NativeEngine::Simple(matcher) => Ok((matcher.match_at(line, start), None)),
            NativeEngine::WordSet(matcher) => Ok((matcher.match_at(line, start), None)),
            NativeEngine::SymbolSet(matcher) => Ok((matcher.match_at(line, start), None)),
            NativeEngine::DelimitedFields(matcher) => Ok((matcher.match_at(line, start), None)),
            NativeEngine::NixUri(matcher) => Ok((matcher.match_at(line, start), None)),
            NativeEngine::Vm(matcher) => matcher
                .try_find_at(line, start, ctx)
                .map(|report| (report.result, Some(report.steps))),
        }
    }

    pub(crate) fn find_at_without_captures_with_scratch(
        &self,
        line: &str,
        start: usize,
        ctx: AnchorContext,
        scratch: &mut super::bytecode::BytecodeScratch,
    ) -> Result<Option<MatchResult>, super::FallbackError> {
        if !anchor_permits_at(self.translation.anchor_strategy, start, ctx, line) {
            return Ok(None);
        }
        match &self.engine {
            NativeEngine::Simple(matcher) => Ok(matcher.match_at(line, start)),
            NativeEngine::WordSet(matcher) => Ok(matcher.match_at(line, start)),
            NativeEngine::SymbolSet(matcher) => Ok(matcher.match_at(line, start)),
            NativeEngine::DelimitedFields(matcher) => Ok(matcher.match_at(line, start)),
            NativeEngine::NixUri(matcher) => Ok(matcher.match_at(line, start)),
            NativeEngine::Vm(matcher) => matcher
                .try_find_at_without_captures_with_scratch(line, start, ctx, scratch)
                .map(|report| report.result),
        }
    }
}

fn specialized_engine(translation: &Translation) -> Option<NativeEngine> {
    if let Some(simple) = SimpleMatcher::try_from_translation(translation) {
        Some(NativeEngine::Simple(simple))
    } else if let Some(word_set) = WordSetMatcher::try_from_translation(translation) {
        Some(NativeEngine::WordSet(word_set))
    } else if let Some(symbol_set) = SymbolSetMatcher::try_from_translation(translation) {
        Some(NativeEngine::SymbolSet(symbol_set))
    } else if let Some(fields) = DelimitedFieldsMatcher::try_from_translation(translation) {
        Some(NativeEngine::DelimitedFields(fields))
    } else {
        NixUriMatcher::try_from_translation(translation).map(NativeEngine::NixUri)
    }
}

impl Matcher for AutomataMatcher {
    fn find(&self, line: &str, from: usize, ctx: AnchorContext) -> Option<MatchResult> {
        match &self.engine {
            NativeEngine::Simple(matcher) => matcher.find(line, from, ctx),
            NativeEngine::WordSet(matcher) => matcher.find(line, from),
            NativeEngine::SymbolSet(matcher) => matcher.find(line, from),
            NativeEngine::DelimitedFields(matcher) => matcher.find(line, from),
            NativeEngine::NixUri(matcher) => matcher.find(line, from),
            NativeEngine::Vm(matcher) => {
                // Anchor strategies previously enforced by regex-automata Input
                // spans. The VM also re-checks anchors, but bail early when the
                // strategy makes a match impossible.
                if !anchor_permits_search(self.translation.anchor_strategy, from, ctx, line) {
                    return None;
                }
                // FallbackMatcher applies its own prefilter; skip a redundant
                // outer check for the Vm path to avoid double work.
                matcher.find(line, from, ctx)
            }
        }
    }
}

fn anchor_permits_search(
    strategy: AnchorStrategy,
    from: usize,
    ctx: AnchorContext,
    line: &str,
) -> bool {
    match strategy {
        AnchorStrategy::None => true,
        AnchorStrategy::TextStartGuard => ctx.allow_a && from == 0,
        AnchorStrategy::LineStartGuard => from == 0,
        AnchorStrategy::ContinuationGuard => {
            ctx.allow_g && ctx.g_pos >= from && line.is_char_boundary(ctx.g_pos)
        }
        // Non-leading anchors are handled inside the VM.
        AnchorStrategy::Fallback => true,
    }
}

fn anchor_permits_at(
    strategy: AnchorStrategy,
    start: usize,
    ctx: AnchorContext,
    line: &str,
) -> bool {
    match strategy {
        AnchorStrategy::ContinuationGuard => {
            ctx.allow_g && ctx.g_pos == start && line.is_char_boundary(start)
        }
        _ => anchor_permits_search(strategy, start, ctx, line),
    }
}

#[derive(Debug, Clone, Copy)]
enum PatternEntry {
    Literal,
    Matcher,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrontierBuildPolicy {
    Auto,
    #[cfg(test)]
    Force,
}

#[derive(Debug, Clone)]
struct CandidateFrontier {
    scanner: Scanner,
    opaque_unrestricted_entries: Vec<usize>,
    opaque_start_byte_entries: StartByteBuckets,
    opaque_start_prefilter: Option<StartBytePrefilter>,
}

/// Compact immutable byte → candidate-index adjacency table.
///
/// Candidate sets used to retain 256 independent `Vec` headers (6 KiB on a
/// 64-bit target) plus one allocation for every populated byte. A tokenizer
/// can retain thousands of candidate sets, so the empty headers alone could
/// dominate engine memory. One offset table and one contiguous index slice
/// preserve ordered lookup while substantially improving locality.
#[derive(Debug, Clone)]
struct StartByteBuckets {
    offsets: [u32; 257],
    entries: Box<[usize]>,
}

impl StartByteBuckets {
    fn retained_heap_bytes(&self) -> usize {
        self.entries
            .len()
            .saturating_mul(std::mem::size_of::<usize>())
    }

    fn from_patterns(patterns: &[Arc<super::CompiledPattern>]) -> (Self, Vec<usize>) {
        let mut counts = [0u32; 256];
        let mut unrestricted = Vec::new();
        for (index, pattern) in patterns.iter().enumerate() {
            if !for_each_pattern_start_byte(pattern, |byte| {
                counts[byte as usize] = counts[byte as usize]
                    .checked_add(1)
                    .expect("candidate bucket count fits in u32");
            }) {
                unrestricted.push(index);
            }
        }

        let mut offsets = [0u32; 257];
        for byte in 0..256 {
            offsets[byte + 1] = offsets[byte]
                .checked_add(counts[byte])
                .expect("candidate bucket offsets fit in u32");
        }
        let mut cursors: [u32; 256] = offsets[..256]
            .try_into()
            .expect("candidate offset prefix has 256 entries");
        let mut entries = vec![0usize; offsets[256] as usize];
        for (index, pattern) in patterns.iter().enumerate() {
            for_each_pattern_start_byte(pattern, |byte| {
                let cursor = &mut cursors[byte as usize];
                entries[*cursor as usize] = index;
                *cursor += 1;
            });
        }
        (
            Self {
                offsets,
                entries: entries.into_boxed_slice(),
            },
            unrestricted,
        )
    }

    fn from_vecs(buckets: Vec<Vec<usize>>) -> Self {
        debug_assert_eq!(buckets.len(), usize::from(u8::MAX) + 1);
        let mut offsets = [0u32; 257];
        let total = buckets.iter().map(Vec::len).sum();
        let mut entries = Vec::with_capacity(total);
        for (byte, bucket) in buckets.into_iter().enumerate() {
            offsets[byte] = u32::try_from(entries.len()).expect("candidate bucket fits in u32");
            entries.extend(bucket);
        }
        offsets[256] = u32::try_from(entries.len()).expect("candidate bucket fits in u32");
        Self {
            offsets,
            entries: entries.into_boxed_slice(),
        }
    }

    #[inline]
    fn get(&self, byte: u8) -> &[usize] {
        let index = byte as usize;
        let start = self.offsets[index] as usize;
        let end = self.offsets[index + 1] as usize;
        &self.entries[start..end]
    }
}

fn for_each_pattern_start_byte(
    pattern: &super::CompiledPattern,
    mut visit: impl FnMut(u8),
) -> bool {
    if let Some(literal) = pattern.unanchored_literal() {
        if let Some(byte) = literal.as_bytes().first().copied() {
            visit(byte);
            return true;
        }
        return false;
    }
    let Some(bytes) = pattern.restricted_start_bytes() else {
        return false;
    };
    for byte in bytes {
        visit(*byte);
    }
    true
}

#[derive(Debug, Clone)]
struct StartBytePrefilter {
    bytes: Vec<u8>,
    bitmap: [u64; 4],
}

/// Native multi-pattern matcher. Leftmost match wins; on equal start offsets
/// the lowest pattern index wins (regex-automata compatible).
#[derive(Debug, Clone)]
pub struct PatternSetMatcher {
    compiled: Arc<[Arc<super::CompiledPattern>]>,
    entries: Vec<PatternEntry>,
    unrestricted_entries: Vec<usize>,
    start_byte_entries: StartByteBuckets,
    start_prefilter: Option<StartBytePrefilter>,
    /// Per-entry word-context start-class masks (see `start_class`); an
    /// entry is skipped at scan positions whose class bit is not set.
    start_class_masks: Vec<u8>,
    /// Per-entry separator skip gates (see `skip_prefix`); `None` entries
    /// are always attempted.
    skip_gates: Vec<Option<super::skip_prefix::SkipGate>>,
    /// Present when every pattern is a pure unanchored literal.
    literal_set: Option<LiteralSet>,
    /// Shared ordered NFA for candidate sets wholly inside the regular subset.
    scanner: Option<Scanner>,
    /// Exact mixed frontier: regular candidates are scanned together, and the
    /// opaque candidates are searched only as far as the regular bound they can
    /// still beat. Candidate indexes are always the original grammar order.
    frontier: Option<CandidateFrontier>,
    #[cfg(test)]
    force_regular_replay_failure: Option<usize>,
}

impl PatternSetMatcher {
    /// Heap allocations uniquely retained by the selector. The shared
    /// compiled-pattern Arc slice is charged by the owning candidate blueprint.
    pub(crate) fn retained_heap_bytes(&self) -> usize {
        let entry_bytes = self
            .entries
            .capacity()
            .saturating_mul(std::mem::size_of::<PatternEntry>());
        let unrestricted_bytes = self
            .unrestricted_entries
            .capacity()
            .saturating_mul(std::mem::size_of::<usize>());
        let mask_bytes = self
            .start_class_masks
            .capacity()
            .saturating_mul(std::mem::size_of::<u8>());
        let skip_gate_size = std::mem::size_of::<Option<super::skip_prefix::SkipGate>>();
        let skip_gate_bytes = self.skip_gates.capacity().saturating_mul(skip_gate_size);
        let mut bytes = entry_bytes
            .saturating_add(unrestricted_bytes)
            .saturating_add(self.start_byte_entries.retained_heap_bytes())
            .saturating_add(mask_bytes)
            .saturating_add(skip_gate_bytes);
        if let Some(prefilter) = &self.start_prefilter {
            bytes = bytes.saturating_add(prefilter.retained_heap_bytes());
        }
        if let Some(literal_set) = &self.literal_set {
            bytes = bytes.saturating_add(literal_set.retained_heap_bytes());
        }
        if let Some(scanner) = &self.scanner {
            bytes = bytes.saturating_add(scanner.retained_heap_bytes());
        }
        if let Some(frontier) = &self.frontier {
            bytes = bytes.saturating_add(frontier.retained_heap_bytes());
        }
        bytes
    }

    pub fn new(patterns: &[String]) -> Result<Self, AutomataBuildError> {
        let patterns = patterns
            .iter()
            .map(|pattern| Arc::new(super::CompiledPattern::new(pattern)))
            .collect::<Vec<_>>();
        Ok(Self::from_compiled(&patterns))
    }

    /// Builds the ordered set from already-compiled patterns. The tokenizer
    /// uses this path so constructing a candidate set never reparses regexes.
    pub fn from_compiled(patterns: &[Arc<super::CompiledPattern>]) -> Self {
        Self::from_shared_compiled(Arc::from(patterns))
    }

    pub(crate) fn from_shared_compiled(patterns: Arc<[Arc<super::CompiledPattern>]>) -> Self {
        Self::from_compiled_with_frontier_policy(patterns, FrontierBuildPolicy::Auto)
    }

    fn from_compiled_with_frontier_policy(
        patterns: Arc<[Arc<super::CompiledPattern>]>,
        frontier_policy: FrontierBuildPolicy,
    ) -> Self {
        let mut entries = Vec::with_capacity(patterns.len());
        let mut all_literals = Vec::with_capacity(patterns.len());
        let mut start_class_masks = Vec::with_capacity(patterns.len());
        let mut skip_gates = Vec::with_capacity(patterns.len());
        let masks_enabled = start_class_gate_enabled();
        let gates_enabled = skip_gate_enabled();
        let mut literals_only = true;
        let (start_byte_entries, unrestricted_entries) = StartByteBuckets::from_patterns(&patterns);
        for pattern in patterns.iter() {
            start_class_masks.push(if masks_enabled {
                pattern.start_class_mask()
            } else {
                super::start_class::START_CLASS_ALL
            });
            skip_gates.push(if gates_enabled {
                pattern.skip_gate().cloned()
            } else {
                None
            });
            if let Some(literal) = pattern.unanchored_literal() {
                let literal = literal.to_owned();
                if literals_only {
                    all_literals.push(literal.clone());
                }
                entries.push(PatternEntry::Literal);
            } else {
                literals_only = false;
                all_literals.clear();
                entries.push(PatternEntry::Matcher);
            }
        }
        let literal_set = if literals_only && !entries.is_empty() {
            Some(LiteralSet::new(all_literals))
        } else {
            None
        };
        let start_prefilter =
            StartBytePrefilter::from_buckets(&unrestricted_entries, &start_byte_entries);
        let (scanner, frontier) = if !literals_only && entries.len() > 1 {
            if frontier_explicitly_disabled() {
                (
                    Scanner::compile_with_hints(patterns.iter().enumerate().map(
                        |(index, pattern)| {
                            (index, pattern.parsed(), pattern.restricted_start_bytes())
                        },
                    ))
                    .ok(),
                    None,
                )
            } else {
                let regular = patterns
                    .iter()
                    .filter(|pattern| pattern.analysis().scanner_supported())
                    .count();
                let opaque = entries.len() - regular;
                if opaque == 0 {
                    (
                        Scanner::compile_with_hints(patterns.iter().enumerate().map(
                            |(index, pattern)| {
                                (index, pattern.parsed(), pattern.restricted_start_bytes())
                            },
                        ))
                        .ok(),
                        None,
                    )
                } else if regular != 0
                    && frontier_enabled(frontier_policy, entries.len(), regular, opaque)
                {
                    let (partial, failures) = Scanner::compile_partial_with_hints(
                        patterns.iter().enumerate().map(|(index, pattern)| {
                            (index, pattern.parsed(), pattern.restricted_start_bytes())
                        }),
                    );
                    let frontier = if partial.entry_count() != 0 {
                        let opaque_entries = failures
                            .into_iter()
                            .map(|failure| failure.pattern)
                            .collect::<Vec<_>>();
                        Some(CandidateFrontier::new(partial, &opaque_entries, &patterns))
                    } else {
                        None
                    };
                    (None, frontier)
                } else {
                    (None, None)
                }
            }
        } else {
            (None, None)
        };

        Self {
            compiled: patterns,
            entries,
            unrestricted_entries,
            start_byte_entries,
            start_prefilter,
            start_class_masks,
            skip_gates,
            literal_set,
            scanner,
            frontier,
            #[cfg(test)]
            force_regular_replay_failure: None,
        }
    }

    #[cfg(test)]
    fn from_compiled_with_forced_frontier(patterns: &[Arc<super::CompiledPattern>]) -> Self {
        Self::from_compiled_with_frontier_policy(Arc::from(patterns), FrontierBuildPolicy::Force)
    }

    #[cfg(test)]
    fn force_regular_replay_failure_for(&mut self, pattern: usize) {
        self.force_regular_replay_failure = Some(pattern);
    }

    pub fn find(&self, line: &str, from: usize) -> Option<(usize, MatchResult)> {
        self.find_with_context(line, from, AnchorContext::default())
    }

    pub fn find_with_context(
        &self,
        line: &str,
        from: usize,
        ctx: AnchorContext,
    ) -> Option<(usize, MatchResult)> {
        self.find_with_context_and_scratch(
            line,
            from,
            ctx,
            &mut super::bytecode::BytecodeScratch::default(),
        )
        .0
    }

    pub(crate) fn find_with_context_and_scratch(
        &self,
        line: &str,
        from: usize,
        ctx: AnchorContext,
        scratch: &mut super::bytecode::BytecodeScratch,
    ) -> (Option<(usize, MatchResult)>, bool) {
        if !line.is_char_boundary(from) {
            return (None, false);
        }
        if let Some(set) = &self.literal_set {
            let found = set.find(line, from).map(|(idx, start, end)| {
                (
                    idx,
                    MatchResult {
                        start,
                        end,
                        captures: vec![Some(start..end)],
                    },
                )
            });
            return (found, false);
        }
        if let Some(frontier) = &self.frontier {
            return self.find_with_frontier(line, from, ctx, scratch, frontier);
        }
        if scanner_candidate_enabled()
            && let Some((result, budget_killed)) = self.find_with_scanner(line, from, ctx, scratch)
        {
            return (result, budget_killed);
        }

        self.find_reference_with_buckets(
            line,
            from,
            ctx,
            scratch,
            &self.unrestricted_entries,
            &self.start_byte_entries,
            self.start_prefilter.as_ref(),
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn find_reference_with_buckets(
        &self,
        line: &str,
        from: usize,
        ctx: AnchorContext,
        scratch: &mut super::bytecode::BytecodeScratch,
        unrestricted_entries: &[usize],
        start_byte_entries: &StartByteBuckets,
        start_prefilter: Option<&StartBytePrefilter>,
        bound: Option<(usize, usize)>,
    ) -> (Option<(usize, MatchResult)>, bool) {
        let ascii_line = scratch.line_is_ascii(line);
        let mut skip_state = super::skip_prefix::SkipGateLineState::default();
        let mut line_has_comment: Option<bool> = None;
        let mut budget_killed = false;
        let start = match start_prefilter {
            Some(prefilter) => prefilter.next_start(line, from),
            None => Some(from),
        };
        let Some(mut start) = start else {
            return (None, false);
        };
        loop {
            if bound.is_some_and(|(bound_start, _)| start > bound_start) {
                break;
            }
            let position_class = position_class_bit(line.as_bytes(), start);
            let restricted = line
                .as_bytes()
                .get(start)
                .map_or(&[][..], |byte| start_byte_entries.get(*byte));
            let mut unrestricted_index = 0usize;
            let mut restricted_index = 0usize;
            while unrestricted_index < unrestricted_entries.len()
                || restricted_index < restricted.len()
            {
                let unrestricted = unrestricted_entries.get(unrestricted_index).copied();
                let restricted = restricted.get(restricted_index).copied();
                let idx = match (unrestricted, restricted) {
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
                };
                if self.start_class_masks[idx] & position_class == 0 {
                    continue;
                }
                if let Some(gate) = &self.skip_gates[idx] {
                    match gate.decide(line, start, &mut skip_state) {
                        super::skip_prefix::SkipGateDecision::Allow => {}
                        super::skip_prefix::SkipGateDecision::Skip => continue,
                        super::skip_prefix::SkipGateDecision::NeedsCommentCheck => {
                            let has_comment = *line_has_comment
                                .get_or_insert_with(|| scratch.line_has_block_comment(line));
                            if !has_comment {
                                continue;
                            }
                        }
                    }
                }
                if bound.is_some_and(|(bound_start, bound_index)| {
                    start == bound_start && idx >= bound_index
                }) {
                    continue;
                }
                let (result, killed) = self.match_entry_at(idx, line, start, ctx, scratch);
                budget_killed |= killed;
                if let Some(result) = result {
                    return (Some((idx, result)), budget_killed);
                }
            }
            if start == line.len() {
                break;
            }
            let next = start
                + if ascii_line {
                    1
                } else {
                    line[start..]
                        .chars()
                        .next()
                        .expect("start is before line end")
                        .len_utf8()
                };
            start = match start_prefilter {
                Some(prefilter) => prefilter.next_start(line, next),
                None => Some(next),
            }
            .unwrap_or(line.len());
        }
        (None, budget_killed)
    }

    fn find_with_frontier(
        &self,
        line: &str,
        from: usize,
        ctx: AnchorContext,
        scratch: &mut super::bytecode::BytecodeScratch,
        frontier: &CandidateFrontier,
    ) -> (Option<(usize, MatchResult)>, bool) {
        let regular = frontier.scanner.find(line, from, ctx, scratch.scanner());
        let mut budget_killed = false;
        let opaque = if let Some(selected) = regular {
            let (found, killed) = self.find_reference_with_buckets(
                line,
                from,
                ctx,
                scratch,
                &frontier.opaque_unrestricted_entries,
                &frontier.opaque_start_byte_entries,
                frontier.opaque_start_prefilter.as_ref(),
                Some((selected.start, selected.pattern)),
            );
            budget_killed |= killed;
            found
        } else {
            let (found, killed) = self.find_reference_with_buckets(
                line,
                from,
                ctx,
                scratch,
                &frontier.opaque_unrestricted_entries,
                &frontier.opaque_start_byte_entries,
                frontier.opaque_start_prefilter.as_ref(),
                None,
            );
            budget_killed |= killed;
            found
        };
        if let Some(opaque) = opaque {
            return (Some(opaque), budget_killed);
        }

        let Some(selected) = regular else {
            return (None, budget_killed);
        };
        let replay = if selected.pattern < self.entries.len() {
            let (result, killed) =
                self.match_entry_at(selected.pattern, line, selected.start, ctx, scratch);
            budget_killed |= killed;
            #[cfg(test)]
            {
                if self.force_regular_replay_failure == Some(selected.pattern) {
                    None
                } else {
                    result
                }
            }
            #[cfg(not(test))]
            {
                result
            }
        } else {
            None
        };
        match replay {
            Some(result)
                if result.start == selected.start
                    && result.end <= line.len()
                    && line.is_char_boundary(result.end) =>
            {
                (Some((selected.pattern, result)), budget_killed)
            }
            _ => {
                // The frontier is a selector only. If exact replay of the
                // selected regular candidate fails (or a future scanner bug
                // picks a non-authoritative endpoint), fall back to the known
                // exact reference traversal instead of reporting no match.
                let (found, killed) = self.find_reference_with_buckets(
                    line,
                    from,
                    ctx,
                    scratch,
                    &self.unrestricted_entries,
                    &self.start_byte_entries,
                    self.start_prefilter.as_ref(),
                    None,
                );
                (found, budget_killed | killed)
            }
        }
    }

    fn find_with_scanner(
        &self,
        line: &str,
        from: usize,
        ctx: AnchorContext,
        scratch: &mut super::bytecode::BytecodeScratch,
    ) -> Option<(Option<(usize, MatchResult)>, bool)> {
        let scanner = self.scanner.as_ref()?;
        let Some(selected) = scanner.find(line, from, ctx, scratch.scanner()) else {
            return Some((None, false));
        };
        let (result, budget_killed) =
            self.match_entry_at(selected.pattern, line, selected.start, ctx, scratch);
        Some((
            result.map(|result| (selected.pattern, result)),
            budget_killed,
        ))
    }

    /// Returns the winning entry plus whether any candidate attempt was cut
    /// short by its execution budget. A budget kill means the scan is not
    /// exact: callers must treat the result as degraded rather than complete.
    fn match_entry_at(
        &self,
        index: usize,
        line: &str,
        start: usize,
        ctx: AnchorContext,
        scratch: &mut super::bytecode::BytecodeScratch,
    ) -> (Option<MatchResult>, bool) {
        let Some(entry) = self.entries.get(index) else {
            return (None, false);
        };
        let Some(pattern) = self.compiled.get(index) else {
            return (None, false);
        };
        match entry {
            PatternEntry::Literal => (
                match_literal_at(
                    line,
                    start,
                    pattern
                        .unanchored_literal()
                        .expect("literal entry retains literal pattern"),
                ),
                false,
            ),
            PatternEntry::Matcher => match pattern
                .matcher()
                .find_at_without_captures_with_scratch(line, start, ctx, scratch)
            {
                Ok(result) => (result, false),
                Err(super::FallbackError::BudgetExceeded { .. }) => (None, true),
                Err(_) => (None, false),
            },
        }
    }

    pub fn patterns(&self) -> impl ExactSizeIterator<Item = &str> {
        self.compiled
            .iter()
            .map(|pattern| pattern.translated_pattern())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl CandidateFrontier {
    fn retained_heap_bytes(&self) -> usize {
        let mut bytes = self
            .scanner
            .retained_heap_bytes()
            .saturating_add(
                self.opaque_unrestricted_entries
                    .capacity()
                    .saturating_mul(std::mem::size_of::<usize>()),
            )
            .saturating_add(self.opaque_start_byte_entries.retained_heap_bytes());
        if let Some(prefilter) = &self.opaque_start_prefilter {
            bytes = bytes.saturating_add(prefilter.retained_heap_bytes());
        }
        bytes
    }

    fn new(
        scanner: Scanner,
        opaque_entries: &[usize],
        patterns: &[Arc<super::CompiledPattern>],
    ) -> Self {
        let mut opaque_unrestricted_entries = Vec::new();
        let mut opaque_start_byte_entries = (0..=u8::MAX)
            .map(|_| Vec::<usize>::new())
            .collect::<Vec<_>>();
        for &index in opaque_entries {
            if let Some(bytes) = patterns
                .get(index)
                .and_then(|pattern| pattern.restricted_start_bytes())
                .filter(|bytes| !bytes.is_empty())
            {
                for byte in bytes {
                    opaque_start_byte_entries[*byte as usize].push(index);
                }
            } else {
                opaque_unrestricted_entries.push(index);
            }
        }
        for bucket in &mut opaque_start_byte_entries {
            bucket.sort_unstable();
            bucket.dedup();
        }
        opaque_unrestricted_entries.sort_unstable();
        opaque_unrestricted_entries.dedup();
        let opaque_start_byte_entries = StartByteBuckets::from_vecs(opaque_start_byte_entries);
        let opaque_start_prefilter = StartBytePrefilter::from_buckets(
            &opaque_unrestricted_entries,
            &opaque_start_byte_entries,
        );
        Self {
            scanner,
            opaque_unrestricted_entries,
            opaque_start_byte_entries,
            opaque_start_prefilter,
        }
    }
}

impl StartBytePrefilter {
    fn retained_heap_bytes(&self) -> usize {
        self.bytes
            .capacity()
            .saturating_mul(std::mem::size_of::<u8>())
    }

    fn from_buckets(
        unrestricted_entries: &[usize],
        start_byte_entries: &StartByteBuckets,
    ) -> Option<Self> {
        if !unrestricted_entries.is_empty() {
            return None;
        }
        let mut bytes = Vec::new();
        let mut bitmap = [0u64; 4];
        for byte in 0u8..=u8::MAX {
            if !start_byte_entries.get(byte).is_empty() {
                bytes.push(byte);
                bitmap[byte as usize >> 6] |= 1u64 << (byte & 63);
            }
        }
        (!bytes.is_empty()).then_some(Self { bytes, bitmap })
    }

    fn next_start(&self, line: &str, from: usize) -> Option<usize> {
        if from > line.len() {
            return None;
        }
        let mut base = from;
        loop {
            let slice = line.as_bytes().get(base..)?;
            let offset = find_start_byte(slice, &self.bytes, &self.bitmap)?;
            let position = base + offset;
            if line.is_char_boundary(position) {
                return Some(position);
            }
            base = position.saturating_add(1);
        }
    }
}

fn find_start_byte(haystack: &[u8], bytes: &[u8], bitmap: &[u64; 4]) -> Option<usize> {
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

fn frontier_enabled(
    policy: FrontierBuildPolicy,
    total: usize,
    regular: usize,
    opaque: usize,
) -> bool {
    match policy {
        #[cfg(test)]
        FrontierBuildPolicy::Force => return true,
        FrontierBuildPolicy::Auto => {}
    }
    // Measured guardrails: relaxing these to admit opaque-majority sets
    // (e.g. TypeScript root sets run 22 regular / 63 opaque) regressed
    // steady-state throughput by 14-48% because the NFA selection pass runs
    // ahead of an still-unbounded opaque reference scan. Only revisit with a
    // design that also bounds the opaque traversal.
    total >= frontier_min_total()
        && regular >= frontier_min_regular()
        && opaque <= frontier_max_opaque()
        && regular >= opaque.saturating_mul(frontier_min_regular_per_opaque())
}

fn frontier_explicitly_disabled() -> bool {
    false
}

fn frontier_min_total() -> usize {
    10
}

fn frontier_min_regular() -> usize {
    8
}

fn frontier_max_opaque() -> usize {
    4
}

fn frontier_min_regular_per_opaque() -> usize {
    4
}

/// Word-context class bit for a scan position: `1 << (prev_word * 2 +
/// cur_word)`, with line edges counting as non-word. Any non-ASCII neighbor
/// returns all classes so masks are only authoritative over ASCII text.
#[inline]
fn position_class_bit(bytes: &[u8], position: usize) -> u8 {
    let prev = position.checked_sub(1).and_then(|prev| bytes.get(prev));
    let cur = bytes.get(position);
    if prev.is_some_and(|byte| !byte.is_ascii()) || cur.is_some_and(|byte| !byte.is_ascii()) {
        return super::start_class::START_CLASS_ALL;
    }
    let prev_word = prev.copied().is_some_and(is_ascii_word_byte);
    let cur_word = cur.copied().is_some_and(is_ascii_word_byte);
    1 << ((u8::from(prev_word) << 1) | u8::from(cur_word))
}

fn start_class_gate_enabled() -> bool {
    true
}

fn skip_gate_enabled() -> bool {
    true
}

fn scanner_candidate_enabled() -> bool {
    true
}

fn match_literal_at(line: &str, start: usize, literal: &str) -> Option<MatchResult> {
    let end = start.checked_add(literal.len())?;
    (line.as_bytes().get(start..end)? == literal.as_bytes()).then(|| MatchResult {
        start,
        end,
        captures: vec![Some(start..end)],
    })
}

fn unescape_literal(pattern: &str) -> String {
    let mut output = String::new();
    let mut escaped = false;
    for ch in pattern.chars() {
        if escaped {
            output.push(match ch {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                other => other,
            });
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else {
            output.push(ch);
        }
    }
    if escaped {
        output.push('\\');
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::regex::CompiledPattern;

    fn ctx() -> AnchorContext {
        AnchorContext {
            allow_a: false,
            allow_g: false,
            g_pos: 0,
        }
    }

    #[test]
    fn simple_literal_finds_unanchored_match() {
        let matcher = SimpleMatcher::from_pattern("foo");
        assert_eq!(matcher.find("xxfoo", 0, ctx()).unwrap().start, 2);
    }

    #[test]
    fn case_insensitive_word_set_matches_keyword_tokens() {
        let matcher =
            AutomataMatcher::new(r"\b(?i)(select|from|where|abort_after_wait)\b").unwrap();
        assert!(matcher.is_word_set());
        let result = matcher
            .find(
                "xx SELECT abort_after_waiting abort_after_wait",
                0,
                AnchorContext::default(),
            )
            .unwrap();
        assert_eq!(result.start..result.end, 3..9);
        assert_eq!(result.captures[1], Some(3..9));
        let result = matcher
            .find(
                "xx abort_after_waiting abort_after_wait",
                0,
                AnchorContext::default(),
            )
            .unwrap();
        assert_eq!(result.start..result.end, 23..39);
        assert!(
            matcher
                .find("éSELECT", 0, AnchorContext::default())
                .is_none()
        );
        assert!(
            matcher
                .find("SELECTé", 0, AnchorContext::default())
                .is_none()
        );
    }

    #[test]
    fn scoped_word_set_supports_noncapturing_inventory() {
        let matcher = AutomataMatcher::new(r"(?i:\b(?:select|from|where|join)\b)").unwrap();
        assert!(matcher.is_word_set());
        let result = matcher
            .find("xx JOIN", 0, AnchorContext::default())
            .unwrap();
        assert_eq!(result.start..result.end, 3..7);
        assert_eq!(result.captures, vec![Some(3..7)]);
    }

    #[test]
    fn word_set_allows_sql_function_call_suffix() {
        let matcher = AutomataMatcher::new(r"(?i)\b(abs|acos|asin|atan)\b\s*\(").unwrap();
        assert!(matcher.is_word_set());
        let result = matcher
            .find("xx ACOS (value)", 0, AnchorContext::default())
            .unwrap();
        assert_eq!(result.start..result.end, 3..9);
        assert_eq!(result.captures[1], Some(3..7));
        assert!(
            matcher
                .find("xx ACOS value", 0, AnchorContext::default())
                .is_none()
        );
    }

    #[test]
    fn word_set_rejects_inventories_with_multiword_branches() {
        let matcher = AutomataMatcher::new(concat!(
            r"\b(?i:define|select|distinct|reduced|from|named|construct|ask|describe|where|",
            r"graph|having|bind|as|filter|optional|union|order|by|group|limit|offset|values|",
            r"insert data|delete data|with|delete|insert|clear|silent|default|all|create|drop|",
            r"copy|move|add|to|using|service|not exists|exists|not in|in|minus|load)\b"
        ))
        .unwrap();
        assert!(!matcher.is_word_set());
        for phrase in ["NOT IN", "NOT EXISTS", "INSERT DATA", "DELETE DATA"] {
            let matched = matcher.find(phrase, 0, ctx()).expect(phrase);
            assert_eq!(matched.start..matched.end, 0..phrase.len());
        }
    }

    #[test]
    fn standalone_inline_flags_apply_from_their_position_only() {
        let matcher = AutomataMatcher::new(
            r"(?:(?i)\b(as|by|or|and|over|where|output|outputnew)|(?-i)\b(NOT|true|false))\b",
        )
        .unwrap();
        for sample in ["AS", "By", "outputNEW", "NOT", "true"] {
            assert!(matcher.find(sample, 0, ctx()).is_some(), "{sample}");
        }
        for sample in ["not", "TRUE", "False"] {
            assert!(matcher.find(sample, 0, ctx()).is_none(), "{sample}");
        }
    }

    #[test]
    fn bare_inline_flags_scope_the_remaining_subexpression() {
        let matcher = AutomataMatcher::new(r"a(?i)b|c").unwrap();
        for sample in ["ab", "aB", "ac", "aC"] {
            let matched = matcher.find(sample, 0, ctx()).expect(sample);
            assert_eq!(matched.start..matched.end, 0..sample.len());
        }
        for sample in ["c", "C"] {
            assert!(matcher.find(sample, 0, ctx()).is_none(), "{sample}");
        }

        let matcher = AutomataMatcher::new(r"(?:(?i)a|b)c").unwrap();
        for sample in ["ac", "Ac", "bc", "Bc"] {
            let matched = matcher.find(sample, 0, ctx()).expect(sample);
            assert_eq!(matched.start..matched.end, 0..sample.len());
        }
        for sample in ["aC", "BC"] {
            assert!(matcher.find(sample, 0, ctx()).is_none(), "{sample}");
        }

        let matcher = AutomataMatcher::new(r"(?i:foo(?-i:bar))").unwrap();
        assert!(matcher.find("FOObar", 0, ctx()).is_some());
        assert!(matcher.find("FOOBAR", 0, ctx()).is_none());

        let matcher = AutomataMatcher::new(r"(?i)(?-i:foo)").unwrap();
        assert!(matcher.find("foo", 0, ctx()).is_some());
        assert!(matcher.find("FOO", 0, ctx()).is_none());
    }

    #[test]
    fn start_byte_prefilter_jumps_to_candidate_bytes() {
        let mut buckets = (0..=u8::MAX)
            .map(|_| Vec::<usize>::new())
            .collect::<Vec<_>>();
        buckets[b'x' as usize].push(0);
        buckets["é".as_bytes()[0] as usize].push(1);
        let buckets = StartByteBuckets::from_vecs(buckets);
        let prefilter = StartBytePrefilter::from_buckets(&[], &buckets).unwrap();
        assert_eq!(prefilter.next_start("abcx", 0), Some(3));
        assert_eq!(prefilter.next_start("abc", 0), None);
        assert_eq!(prefilter.next_start("aéx", 0), Some(1));
        assert!(StartBytePrefilter::from_buckets(&[0], &buckets).is_none());
    }

    #[test]
    fn line_anchor_does_not_match_after_resume() {
        let matcher = SimpleMatcher::from_pattern("^foo");
        assert!(matcher.find("foo", 1, ctx()).is_none());
        assert_eq!(matcher.find("foo", 0, ctx()).unwrap().start, 0);
    }

    #[test]
    fn file_and_g_anchors_use_context() {
        let file = SimpleMatcher::from_pattern(r"\Afoo");
        assert!(file.find("foo", 0, ctx()).is_none());
        assert!(
            file.find(
                "foo",
                0,
                AnchorContext {
                    allow_a: true,
                    allow_g: false,
                    g_pos: 0,
                },
            )
            .is_some()
        );

        let g = SimpleMatcher::from_pattern(r"\Gfoo");
        assert!(
            g.find(
                "xxfoo",
                0,
                AnchorContext {
                    allow_a: false,
                    allow_g: true,
                    g_pos: 2,
                },
            )
            .is_some()
        );
    }

    #[test]
    fn automata_returns_capture_spans() {
        let matcher = AutomataMatcher::new(r"foo(\d+)").unwrap();
        let result = matcher.find("xxfoo123", 0, ctx()).unwrap();
        assert_eq!(result.start..result.end, 2..8);
        assert_eq!(result.captures[1], Some(5..8));
    }

    #[test]
    fn automata_anchor_context_for_resume() {
        let matcher = AutomataMatcher::new("^foo").unwrap();
        assert!(matcher.find("foo", 1, ctx()).is_none());
        assert!(matcher.find("foo", 0, ctx()).is_some());

        let matcher = AutomataMatcher::new(r"\Gfoo").unwrap();
        assert!(
            matcher
                .find(
                    "xxfoo",
                    0,
                    AnchorContext {
                        allow_a: false,
                        allow_g: true,
                        g_pos: 2,
                    }
                )
                .is_some()
        );
    }

    #[test]
    fn pure_literal_uses_simple_engine() {
        let matcher = AutomataMatcher::new("foo").unwrap();
        assert!(matcher.is_simple());
        assert_eq!(matcher.find("xxfoo", 0, ctx()).unwrap().start, 2);
    }

    #[test]
    fn start_class_gate_is_selection_neutral() {
        // Differential: the word-context gate must never change the selected
        // candidate or match span, only skip provably impossible attempts.
        let patterns: Vec<String> = [
            r"(?<!\w)this(?!\w)",
            r"\bwhile\b",
            r"[A-Za-z_]\w*",
            r"\b(\h{7}|\h{10})\b",
            r"\b[0-9]+\b",
            r"(?<![\w$])if\b",
            r"[[:punct:]]+",
            r"\s*+(?:(?<=\W)|(?=\W)|^|\n?$)#",
            r"((?:\s*+/\*(?:[^*]++|\*+(?!/))*+\*/\s*+)+|\s++|(?<=\W)|(?=\W)|^|\n?$|\A|\Z)((?<!\w)template(?!\w))",
            r"\s*+(?<!\w)(?:(unsigned|signed|double)(?!\w))",
            r"\s++(#)\s*define",
            r"\{",
            r"^\s*#\s*define",
            r"=",
            r"\G\w+",
            r"_x",
            r"x_",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        let texts = [
            "this that_this this_",
            "while awhile while_ WHILE while",
            "a_b 7f3a91c deadbeef42 12345 x",
            "if $if aif if",
            "#define FOO_BAR(x) x##y",
            "int template_arg = 0; /* c */ template <class T>",
            "  /* x */ template",
            "\ttemplate/* */unsigned",
            "  \u{a0} template",
            "   #define A 1",
            "foo{bar}_baz{}",
            "__x_ _x x_ =_= a=b",
            "  # trailing",
            "é_word wordé _é 7é",
            "",
            "_",
            "= =",
        ];
        let compiled: Vec<_> = patterns
            .iter()
            .map(|pattern| Arc::new(super::super::CompiledPattern::new(pattern)))
            .collect();
        let gated = PatternSetMatcher::from_compiled(&compiled);
        let mut ungated = gated.clone();
        ungated
            .start_class_masks
            .iter_mut()
            .for_each(|mask| *mask = super::super::start_class::START_CLASS_ALL);
        ungated.skip_gates.iter_mut().for_each(|gate| *gate = None);
        assert!(
            gated.skip_gates.iter().any(Option::is_some),
            "expected at least one skip-gated pattern in the differential set"
        );
        for text in texts {
            for from in 0..=text.len() {
                if !text.is_char_boundary(from) {
                    continue;
                }
                for ctx in [
                    AnchorContext::line_start(),
                    AnchorContext::start_of_file(),
                    AnchorContext::continuation(from),
                ] {
                    let got = gated.find_with_context(text, from, ctx);
                    let expected = ungated.find_with_context(text, from, ctx);
                    assert_eq!(
                        got.as_ref()
                            .map(|(idx, result)| (*idx, result.start, result.end)),
                        expected
                            .as_ref()
                            .map(|(idx, result)| (*idx, result.start, result.end)),
                        "text={text:?} from={from}"
                    );
                }
            }
        }
    }

    #[test]
    fn pattern_set_leftmost_lowest_index() {
        let patterns = vec!["bb".into(), "b".into(), "a".into()];
        let set = PatternSetMatcher::new(&patterns).unwrap();
        let (idx, result) = set.find("abb", 0).unwrap();
        assert_eq!(idx, 2);
        assert_eq!(result.start..result.end, 0..1);

        let (idx, result) = set.find("abb", 1).unwrap();
        assert_eq!(idx, 0);
        assert_eq!(result.start..result.end, 1..3);
    }

    #[test]
    fn pattern_set_handles_regex_entries() {
        let patterns = vec![r"\d+".into(), "foo".into()];
        let set = PatternSetMatcher::new(&patterns).unwrap();
        let (idx, result) = set.find("xfoo9", 0).unwrap();
        assert_eq!(idx, 1);
        assert_eq!(result.start..result.end, 1..4);
        let (idx, result) = set.find("x9foo", 0).unwrap();
        assert_eq!(idx, 0);
        assert_eq!(result.start..result.end, 1..2);
    }

    #[test]
    fn pattern_set_replays_continuation_anchor_only_at_g_position() {
        let patterns = vec![r"\Gfoo".into(), "nomatch".into()];
        let set = PatternSetMatcher::new(&patterns).unwrap();
        let (idx, result) = set
            .find_with_context("foo xxfoo", 0, AnchorContext::continuation(6))
            .unwrap();
        assert_eq!(idx, 0);
        assert_eq!(result.start..result.end, 6..9);
    }

    #[test]
    fn pattern_set_reports_budget_kills_instead_of_silently_dropping() {
        // Regression: a candidate whose VM attempt exhausts its step budget
        // used to be indistinguishable from a plain no-match, so lines could
        // report Complete while matches were silently dropped.
        let patterns = vec!["zzz".into(), r"(a|aa)+b(?=x)".into()];
        let set = PatternSetMatcher::new(&patterns).unwrap();
        let adversarial = format!("{}c", "a".repeat(40));
        let (_, budget_killed) = set.find_with_context_and_scratch(
            &adversarial,
            0,
            AnchorContext::line_start(),
            &mut super::super::bytecode::BytecodeScratch::default(),
        );
        assert!(
            budget_killed,
            "exponential candidate must surface its budget kill"
        );

        // Benign inputs on the same set stay exact and unflagged.
        let (found, budget_killed) = set.find_with_context_and_scratch(
            "zzz here",
            0,
            AnchorContext::line_start(),
            &mut super::super::bytecode::BytecodeScratch::default(),
        );
        assert!(found.is_some());
        assert!(!budget_killed);
    }

    fn forced_frontier(patterns: &[&str]) -> PatternSetMatcher {
        let compiled = patterns
            .iter()
            .map(|pattern| Arc::new(CompiledPattern::new(pattern)))
            .collect::<Vec<_>>();
        PatternSetMatcher::from_compiled_with_forced_frontier(&compiled)
    }

    #[test]
    fn partial_frontier_opaque_can_beat_regular_bound() {
        let set = forced_frontier(&["a", "(?=x)x"]);
        let (idx, result) = set.find("x a", 0).unwrap();
        assert_eq!(idx, 1);
        assert_eq!(result.start..result.end, 0..1);
    }

    #[test]
    fn partial_frontier_preserves_equal_start_grammar_order() {
        let set = forced_frontier(&["(?=a)a", "a"]);
        let (idx, result) = set.find("a", 0).unwrap();
        assert_eq!(idx, 0);
        assert_eq!(result.start..result.end, 0..1);

        let set = forced_frontier(&["a", "(?=a)a"]);
        let (idx, result) = set.find("a", 0).unwrap();
        assert_eq!(idx, 0);
        assert_eq!(result.start..result.end, 0..1);
    }

    #[test]
    fn partial_frontier_ignores_opaque_after_regular_bound() {
        let set = forced_frontier(&["a", "(?=z)z"]);
        let (idx, result) = set.find("a z", 0).unwrap();
        assert_eq!(idx, 0);
        assert_eq!(result.start..result.end, 0..1);
    }

    #[test]
    fn partial_frontier_falls_back_when_regular_replay_fails() {
        let mut set = forced_frontier(&["a", "(?=z)z"]);
        set.force_regular_replay_failure_for(0);
        let (idx, result) = set.find("a z", 0).unwrap();
        assert_eq!(idx, 0);
        assert_eq!(result.start..result.end, 0..1);
    }

    #[test]
    fn partial_frontier_honors_continuation_anchor() {
        let set = forced_frontier(&[r"\Gfoo", "(?=q)q"]);
        let (idx, result) = set
            .find_with_context("xxfoo z", 0, AnchorContext::continuation(2))
            .unwrap();
        assert_eq!(idx, 0);
        assert_eq!(result.start..result.end, 2..5);
        assert!(
            set.find_with_context("xxfoo z", 0, AnchorContext::continuation(3))
                .is_none()
        );
    }

    #[test]
    fn partial_frontier_selection_still_defers_captures_to_replay() {
        let set = forced_frontier(&["(a)", "(?=z)z"]);
        let (idx, result) = set.find("a", 0).unwrap();
        assert_eq!(idx, 0);
        assert_eq!(result.start..result.end, 0..1);
        assert!(
            result.captures.is_empty(),
            "candidate selection should not eagerly materialize captures"
        );
    }

    #[test]
    fn partial_frontier_matches_reference_search_on_mixed_sets() {
        let cases = [
            (
                vec!["a+", "(?=a)a", "b"],
                vec![
                    ("", 0, AnchorContext::default()),
                    ("baaa", 0, AnchorContext::default()),
                    ("xaa", 0, AnchorContext::default()),
                    ("xaa", 1, AnchorContext::default()),
                ],
            ),
            (
                vec![r"\Gfoo", "(?<=x)y", "foo"],
                vec![
                    ("xxfoo y", 0, AnchorContext::continuation(2)),
                    ("xy foo", 0, AnchorContext::continuation(0)),
                    ("xy foo", 2, AnchorContext::continuation(2)),
                ],
            ),
            (
                vec!["[A-Z]+", "(?=z)z", "end$"],
                vec![
                    ("aaZ z", 0, AnchorContext::default()),
                    ("end", 0, AnchorContext::default()),
                    ("lower", 0, AnchorContext::default()),
                ],
            ),
        ];

        for (patterns, probes) in cases {
            let set = forced_frontier(&patterns);
            for (line, from, ctx) in probes {
                let actual = set.find_with_context(line, from, ctx).map(summarize_match);
                let mut scratch = super::super::bytecode::BytecodeScratch::default();
                let expected = set
                    .find_reference_with_buckets(
                        line,
                        from,
                        ctx,
                        &mut scratch,
                        &set.unrestricted_entries,
                        &set.start_byte_entries,
                        set.start_prefilter.as_ref(),
                        None,
                    )
                    .0
                    .map(summarize_match);
                assert_eq!(actual, expected, "patterns={patterns:?} line={line:?}");
            }
        }
    }

    #[test]
    fn nix_uri_specialization_matches_ascii_uri_shape() {
        let matcher = AutomataMatcher::new(NixUriMatcher::PATTERN).unwrap();
        let result = matcher
            .find(
                "xx 1abc:def <nixpkgs> mailto:user",
                0,
                AnchorContext::default(),
            )
            .unwrap();
        assert_eq!(result.start..result.end, 4..11);
        assert_eq!(result.captures, vec![Some(4..11), Some(4..11)]);
        assert!(
            matcher
                .find("<nixpkgs>", 0, AnchorContext::default())
                .is_none()
        );
        assert_eq!(
            matcher
                .find("prefix mailto:user", 7, AnchorContext::default())
                .unwrap()
                .start,
            7
        );
    }

    #[test]
    fn delimited_fields_specialization_preserves_captures_and_empty_fields() {
        let matcher = AutomataMatcher::new(r"([^\t]*\t?)([^\t]*\t?)([^\t]*\t?)").unwrap();
        assert!(matches!(matcher.engine, NativeEngine::DelimitedFields(_)));

        let result = matcher.find("alpha\t\tγamma\n", 0, ctx()).unwrap();
        assert_eq!(result.start..result.end, 0.."alpha\t\tγamma\n".len());
        assert_eq!(
            result.captures,
            vec![
                Some(0.."alpha\t\tγamma\n".len()),
                Some(0..6),
                Some(6..7),
                Some(7.."alpha\t\tγamma\n".len()),
            ]
        );
    }

    #[test]
    fn delimited_fields_specialization_stops_after_declared_columns() {
        let matcher = AutomataMatcher::new(r"([^,]*,?)([^,]*,?)").unwrap();
        let result = matcher.find("a,b,c", 0, ctx()).unwrap();
        assert_eq!(result.start..result.end, 0..4);
        assert_eq!(result.captures, vec![Some(0..4), Some(0..2), Some(2..4)]);
    }

    #[test]
    fn separator_bounded_symbol_set_preserves_capture_and_boundaries() {
        let symbols = (0..40)
            .map(|index| format!("dashboard-command-{index}"))
            .collect::<Vec<_>>()
            .join("|");
        let pattern = format!(r"(?<=[()\s]|^)({symbols})(?=[()\s]|$)");
        let matcher = AutomataMatcher::new(&pattern).unwrap();
        assert!(matches!(matcher.engine, NativeEngine::SymbolSet(_)));
        assert_eq!(
            matcher.prefilter_may_match("unrelated text", 0),
            None,
            "the exact symbol-set search must not build a redundant generic prefilter"
        );

        let line = "(dashboard-command-31) dashboard-command-310";
        let result = matcher.find(line, 0, ctx()).unwrap();
        assert_eq!(result.start..result.end, 1..21);
        assert_eq!(result.captures, vec![Some(1..21), Some(1..21)]);
        assert!(matcher.find(line, 21, ctx()).is_none());
    }

    #[test]
    fn separator_bounded_noncapturing_symbol_set_uses_direct_alternation() {
        let symbols = (0..40)
            .map(|index| format!("dashboard-face-{index}"))
            .collect::<Vec<_>>()
            .join("|");
        let pattern = format!(r"\b(?<=[()\s]|^)(?:{symbols})(?=[()\s]|$)\b");
        let matcher = AutomataMatcher::new(&pattern).unwrap();
        assert!(matches!(matcher.engine, NativeEngine::SymbolSet(_)));
        let result = matcher.find(" dashboard-face-7 ", 0, ctx()).unwrap();
        assert_eq!(result.start..result.end, 1..17);
        assert_eq!(result.captures, vec![Some(1..17)]);
    }

    fn summarize_match((idx, result): (usize, MatchResult)) -> (usize, usize, usize) {
        (idx, result.start, result.end)
    }
}
