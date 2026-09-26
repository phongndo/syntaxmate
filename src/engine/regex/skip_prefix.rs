//! Skip-prefix gate for separator-prefixed patterns.
//!
//! Many C-family grammar rules are shaped `<separator> <token>`, where the
//! separator is the comment-or-whitespace alternation (or a plain `\s*+`)
//! and the token decides the rule: `<sep>(#)\s*pragma`, `<sep>(?<!\w)this`,
//! `\s*+(?<!\w)(?:unsigned|signed|...)`. Such patterns are start-nullable,
//! so the ordered candidate scan attempts them at every position — and the
//! attempt re-consumes the same whitespace run for every candidate before
//! failing on the token.
//!
//! This gate walks the pattern's leading edge through groups, alternations,
//! optional elements, and zero-width assertions. Skip elements (whitespace
//! runs and the separator) are stepped over; the first consuming element on
//! every path is a token whose possible first bytes are recorded. The scan
//! then skips the anchored attempt whenever no token can start at the
//! position itself, at the end of the shared whitespace run, or through a
//! `/*` comment path (gated by a per-line block-comment check). Leading `^`,
//! `\A`, `\G`, and single-character lookbehinds that every path passes also
//! yield a start condition checked before the token bytes.
//!
//! Gates are over-approximations: `decide` may allow a position with no real
//! match, but must never skip one that has any.

use super::AnchorContext;
use super::ast::{AnchorKind, Ast, ClassAtom, LookKind, ParsedRegex, PerlClassKind};
use super::backtrack::{
    StartByteSet, class_start_bytes, expand_case_insensitive_start_bytes,
    is_cpp_space_comment_separator, is_perl_class, strip_nonsemantic_group,
};

use std::sync::Arc;

const ASCII_WHITESPACE: [u8; 6] = [b' ', b'\t', b'\n', b'\r', 0x0b, 0x0c];

/// Shared so every candidate set holding the pattern stores one pointer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SkipGate(Arc<SkipGateParts>);

#[derive(Debug, PartialEq, Eq)]
struct SkipGateParts {
    /// Zero-width condition every match must satisfy at its start.
    start: Option<StartAssert>,
    rest: Option<RestGate>,
}

/// Disjunction of facts about a match start position, derived from leading
/// `^` / `\A` / `\G` anchors, single-character lookbehinds, and tokens that
/// must start exactly at the position.
#[derive(Debug, Clone, PartialEq, Eq)]
struct StartAssert {
    /// `^` / `\A`: only byte offset 0.
    line_start: bool,
    /// `\G`: only the continuation offset, when the context allows it.
    continuation: bool,
    /// Lookbehind: the previous byte (always a whole ASCII character) is in
    /// this set.
    prev_bytes: StartByteSet,
    /// The byte at the position is in this set.
    cur_bytes: StartByteSet,
}

impl StartAssert {
    fn never() -> Self {
        Self {
            line_start: false,
            continuation: false,
            prev_bytes: StartByteSet::empty(),
            cur_bytes: StartByteSet::empty(),
        }
    }

    fn line_start() -> Self {
        Self {
            line_start: true,
            ..Self::never()
        }
    }

    fn continuation() -> Self {
        Self {
            continuation: true,
            ..Self::never()
        }
    }

    fn prev_bytes(prev_bytes: StartByteSet) -> Self {
        Self {
            prev_bytes,
            ..Self::never()
        }
    }

    fn cur_bytes(cur_bytes: StartByteSet) -> Self {
        Self {
            cur_bytes,
            ..Self::never()
        }
    }

    fn only_cur_bytes(&self) -> bool {
        !self.line_start && !self.continuation && self.prev_bytes.is_empty()
    }

    /// Either condition may hold; `None` is "no constraint".
    fn union(left: Option<Self>, right: Option<Self>) -> Option<Self> {
        let (mut left, right) = (left?, right?);
        left.line_start |= right.line_start;
        left.continuation |= right.continuation;
        left.prev_bytes.extend(&right.prev_bytes);
        left.cur_bytes.extend(&right.cur_bytes);
        Some(left)
    }

    fn allows(&self, bytes: &[u8], start: usize, ctx: AnchorContext) -> bool {
        (self.line_start && start == 0)
            || (self.continuation && ctx.allow_g && ctx.g_pos == start)
            || start
                .checked_sub(1)
                .and_then(|prev| bytes.get(prev))
                .is_some_and(|byte| self.prev_bytes.contains(*byte))
            || bytes
                .get(start)
                .is_some_and(|byte| self.cur_bytes.contains(*byte))
    }
}

/// First-token byte gate behind a whitespace/comment skip prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RestGate {
    rest_bytes: StartByteSet,
    /// A token may start at the scan position itself.
    allow_empty: bool,
    /// A whitespace-consuming skip path exists, so a token may start at the
    /// end of the whitespace run.
    allow_whitespace: bool,
    /// A `/*` comment skip path exists; positions on lines containing `/*`
    /// are never gated.
    allow_comment: bool,
}

/// Per-`find` lazily computed line state shared by every gated candidate.
#[derive(Default)]
pub(crate) struct SkipGateLineState {
    whitespace_run: Option<(usize, usize)>,
}

/// Outcome of the cheap byte checks; the block-comment lookup is deferred to
/// the caller so it can be cached per line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SkipGateDecision {
    Allow,
    Skip,
    /// Only a `/*` comment path could still allow a match here.
    NeedsCommentCheck,
}

impl SkipGate {
    #[cfg(test)]
    pub(crate) fn analyze(parsed: &ParsedRegex) -> Option<Self> {
        parsed.analysis().skip_gate().cloned()
    }

    /// `start_restricted` reports that the candidate scan already requires
    /// the pattern's first byte, making a current-byte-only condition
    /// redundant.
    pub(crate) fn analyze_with_effective_flags(
        parsed: &ParsedRegex,
        uniform_flags: Option<super::ast::RegexFlags>,
        has_case_insensitive_scope: bool,
        start_restricted: bool,
    ) -> Option<Self> {
        let mut walk = PrefixWalk {
            bytes: StartByteSet::empty(),
            unbounded: false,
            allow_empty: false,
            allow_whitespace: false,
            allow_comment: false,
            stopped: None,
            case_folding: has_case_insensitive_scope
                || parsed.flags.case_insensitive
                || uniform_flags.is_some_and(|flags| flags.case_insensitive),
        };
        let continuing = walk.visit(&parsed.ast, PathState::START);
        let all_stopped = continuing.is_none();
        let start = match (walk.stopped.take(), continuing) {
            (Some(stopped), Some(path)) => StartAssert::union(stopped, path.assert),
            (Some(stopped), None) => stopped,
            (None, Some(path)) => path.assert,
            (None, None) => None,
        }
        .filter(|start| !(start_restricted && start.only_cur_bytes()));
        let rest = if all_stopped {
            walk.rest_gate(parsed, uniform_flags, has_case_insensitive_scope)
        } else {
            None
        };
        (start.is_some() || rest.is_some()).then(|| Self(Arc::new(SkipGateParts { start, rest })))
    }

    /// Whether an anchored attempt at `start` can possibly match, using only
    /// the cheap byte checks.
    pub(crate) fn decide(
        &self,
        line: &str,
        start: usize,
        ctx: AnchorContext,
        state: &mut SkipGateLineState,
    ) -> SkipGateDecision {
        if let Some(assert) = &self.0.start
            && !assert.allows(line.as_bytes(), start, ctx)
        {
            return SkipGateDecision::Skip;
        }
        match &self.0.rest {
            Some(rest) => rest.decide(line, start, state),
            None => SkipGateDecision::Allow,
        }
    }
}

impl RestGate {
    fn decide(&self, line: &str, start: usize, state: &mut SkipGateLineState) -> SkipGateDecision {
        let bytes = line.as_bytes();
        if self.allow_empty
            && bytes
                .get(start)
                .is_some_and(|byte| self.rest_bytes.contains(*byte))
        {
            return SkipGateDecision::Allow;
        }
        if self.allow_whitespace {
            match whitespace_run_end(bytes, start, &mut state.whitespace_run) {
                Some(end) => {
                    if end > start
                        && bytes
                            .get(end)
                            .is_some_and(|byte| self.rest_bytes.contains(*byte))
                    {
                        return SkipGateDecision::Allow;
                    }
                }
                // Non-ASCII whitespace in the run: give up gating here.
                None => return SkipGateDecision::Allow,
            }
        }
        if self.allow_comment {
            SkipGateDecision::NeedsCommentCheck
        } else {
            SkipGateDecision::Skip
        }
    }
}

/// State of one family of match paths that has not yet consumed a token
/// (a character outside the whitespace/comment skip prefix).
#[derive(Debug, Clone)]
struct PathState {
    /// Some path may still be at the scan position (nothing consumed).
    at_start: bool,
    /// Every path is still at the scan position.
    pristine: bool,
    /// A start condition every path has passed; `None` when unconstrained.
    assert: Option<StartAssert>,
}

impl PathState {
    const START: Self = Self {
        at_start: true,
        pristine: true,
        assert: None,
    };

    fn merge(left: Option<Self>, right: Option<Self>) -> Option<Self> {
        match (left, right) {
            (Some(left), Some(right)) => Some(Self {
                at_start: left.at_start || right.at_start,
                pristine: left.pristine && right.pristine,
                assert: StartAssert::union(left.assert, right.assert),
            }),
            (path, None) | (None, path) => path,
        }
    }

    /// Conjoins a newly passed condition. Only one conjunct is kept; the
    /// earlier one is as valid as the new one.
    fn assume(mut self, assert: StartAssert) -> Self {
        if self.assert.is_none() {
            self.assert = Some(assert);
        }
        self
    }

    fn consumed(self, nullable: bool) -> Self {
        Self {
            at_start: self.at_start && nullable,
            pristine: false,
            ..self
        }
    }
}

/// Walks the leading edge of a pattern: skip elements (whitespace runs, the
/// C-family comment separator, zero-width assertions) are stepped over, and
/// the first consuming element on each path contributes its first bytes as
/// a token. Every union is an over-approximation of where a token can start.
#[derive(Clone)]
struct PrefixWalk {
    bytes: StartByteSet,
    /// A token's first byte is unknown (dot, `\w`, backreference, ...).
    unbounded: bool,
    allow_empty: bool,
    allow_whitespace: bool,
    allow_comment: bool,
    /// Union of the start conditions of every path that reached a token;
    /// `None` until one does.
    stopped: Option<Option<StartAssert>>,
    /// Some scope folds case. Start conditions (lookbehind and current
    /// bytes) are then not derived, and class ranges are not trusted.
    case_folding: bool,
}

impl PrefixWalk {
    /// Records a token reached by `path`; the path stops here.
    fn token(&mut self, path: PathState, bytes: Option<&StartByteSet>) -> Option<PathState> {
        match bytes {
            Some(bytes) => self.bytes.extend(bytes),
            None => self.unbounded = true,
        }
        self.allow_empty |= path.at_start;
        let mut assert = path.assert;
        if assert.is_none()
            && path.pristine
            && !self.case_folding
            && let Some(bytes) = bytes
        {
            assert = Some(StartAssert::cur_bytes(bytes.clone()));
        }
        self.stopped = Some(match self.stopped.take() {
            Some(stopped) => StartAssert::union(stopped, assert),
            None => assert,
        });
        None
    }

    /// Returns the merged state of the paths that pass through `ast` without
    /// consuming a token, or `None` when every path consumes one.
    fn visit(&mut self, ast: &Ast, path: PathState) -> Option<PathState> {
        match classify_skip_element(ast) {
            Some(SkipElement::Separator) => {
                self.allow_whitespace = true;
                self.allow_comment = true;
                return Some(path.consumed(true));
            }
            Some(SkipElement::Whitespace { nullable }) => {
                self.allow_whitespace = true;
                return Some(path.consumed(nullable));
            }
            None => {}
        }
        match ast {
            Ast::Empty => Some(path),
            // `^` and `\A` only hold at offset 0, so passing one anywhere
            // pins the match start there too.
            Ast::Anchor(AnchorKind::LineStart | AnchorKind::TextStart) => {
                Some(path.assume(StartAssert::line_start()))
            }
            Ast::Anchor(AnchorKind::Continuation) if path.pristine => {
                Some(path.assume(StartAssert::continuation()))
            }
            Ast::Anchor(_) => Some(path),
            Ast::Look {
                kind: LookKind::Behind,
                child,
            } if path.pristine && !self.case_folding => match behind_assert(child) {
                Some(assert) => Some(path.assume(assert)),
                None => Some(path),
            },
            // The lookahead body must match at this position, so when every
            // path through it reaches a known token, that token gates this
            // path too. Unknown tokens keep the lookahead transparent.
            Ast::Look {
                kind: LookKind::Ahead,
                child,
            } => {
                let mut inner = self.clone();
                if inner.visit(child, path.clone()).is_none()
                    && (self.unbounded || !inner.unbounded)
                {
                    *self = inner;
                    None
                } else {
                    Some(path)
                }
            }
            Ast::Look { .. } => Some(path),
            Ast::Literal(literal) => match literal.chars().next() {
                None => Some(path),
                Some(ch) if ch.is_ascii() => {
                    let mut bytes = StartByteSet::empty();
                    bytes.insert(ch as u8);
                    self.token(path, Some(&bytes))
                }
                Some(_) => self.token(path, None),
            },
            Ast::Class(class) => self.token(path, class_start_bytes(class).as_ref()),
            Ast::Dot | Ast::Grapheme => self.token(path, None),
            Ast::Concat(nodes) => {
                let mut path = path;
                for node in nodes {
                    path = self.visit(node, path)?;
                }
                Some(path)
            }
            Ast::Alternation(branches) => {
                let mut merged = None;
                for branch in branches {
                    let branch = self.visit(branch, path.clone());
                    merged = PathState::merge(merged, branch);
                }
                merged
            }
            Ast::Repeat { node, min, max, .. } => {
                if *max == Some(0) {
                    return Some(path);
                }
                // Later iterations start from a state no earlier than the
                // first one's, and every recorded fact is a union, so one
                // visit of the body covers them.
                let through = self.visit(node, path.clone());
                if *min == 0 {
                    PathState::merge(through, Some(path))
                } else {
                    through
                }
            }
            Ast::Group { child, .. } | Ast::Flags { child, .. } => self.visit(child, path),
            // Opaque constructs may consume anything or nothing.
            Ast::Backref(_)
            | Ast::Conditional { .. }
            | Ast::Subroutine(_)
            | Ast::Unsupported(_) => {
                self.unbounded = true;
                Some(path.consumed(true))
            }
        }
    }

    fn rest_gate(
        self,
        parsed: &ParsedRegex,
        uniform_flags: Option<super::ast::RegexFlags>,
        has_case_insensitive_scope: bool,
    ) -> Option<RestGate> {
        if self.unbounded || self.bytes.is_empty() || !(self.allow_whitespace || self.allow_comment)
        {
            return None;
        }
        // Mirror the fallback matcher's start-byte policy: bail out on mixed
        // case-insensitive scopes, expand ASCII case pairs (plus non-ASCII
        // lead bytes) when the effective flags fold case.
        if has_case_insensitive_scope && uniform_flags.is_none() {
            return None;
        }
        let mut rest_bytes = self.bytes;
        if uniform_flags.unwrap_or(parsed.flags).case_insensitive {
            expand_case_insensitive_start_bytes(&mut rest_bytes);
        }
        // The whitespace-run shortcut assumes no token can begin inside the
        // run.
        if ASCII_WHITESPACE
            .iter()
            .any(|byte| rest_bytes.contains(*byte))
        {
            return None;
        }
        Some(RestGate {
            rest_bytes,
            allow_empty: self.allow_empty,
            allow_whitespace: self.allow_whitespace,
            allow_comment: self.allow_comment,
        })
    }
}

/// Condition a positive lookbehind with this child forces on the match
/// start: a line start (for `^` / `\A` branches) or an ASCII previous byte.
/// `None` when it cannot be expressed that way.
fn behind_assert(child: &Ast) -> Option<StartAssert> {
    match child {
        Ast::Anchor(AnchorKind::LineStart | AnchorKind::TextStart) => {
            Some(StartAssert::line_start())
        }
        Ast::Group { child, .. } => behind_assert(child),
        Ast::Alternation(branches) => branches
            .iter()
            .map(behind_assert)
            .reduce(StartAssert::union)
            .flatten(),
        Ast::Concat(nodes) => behind_assert(nodes.last()?),
        Ast::Repeat { node, min, .. } if *min >= 1 => behind_assert(node),
        Ast::Literal(literal) => {
            let last = literal.chars().next_back()?;
            last.is_ascii().then(|| {
                let mut bytes = StartByteSet::empty();
                bytes.insert(last as u8);
                StartAssert::prev_bytes(bytes)
            })
        }
        Ast::Class(class) => {
            let bytes = class_start_bytes(class)?;
            // Only whole ASCII characters: a non-ASCII previous character
            // ends in a continuation byte, not its lead byte.
            (0x80..=0xff)
                .all(|byte| !bytes.contains(byte))
                .then(|| StartAssert::prev_bytes(bytes))
        }
        _ => None,
    }
}

enum SkipElement {
    /// The exact C-family comment-or-whitespace separator alternation.
    Separator,
    /// A pure `\s` repeat (or single `\s`); `nullable` when it can match
    /// empty.
    Whitespace { nullable: bool },
}

fn classify_skip_element(ast: &Ast) -> Option<SkipElement> {
    let stripped = strip_flags(strip_nonsemantic_group(ast));
    if let Ast::Alternation(branches) = stripped
        && is_cpp_space_comment_separator(branches)
    {
        return Some(SkipElement::Separator);
    }
    if is_perl_class(strip_flags(stripped), PerlClassKind::Space) {
        return Some(SkipElement::Whitespace { nullable: false });
    }
    if let Ast::Repeat { node, min, max, .. } = stripped
        && max.is_none_or(|max| max >= *min)
        && is_perl_class(
            strip_flags(strip_nonsemantic_group(node)),
            PerlClassKind::Space,
        )
    {
        return Some(SkipElement::Whitespace {
            nullable: *min == 0,
        });
    }
    None
}

fn strip_flags(ast: &Ast) -> &Ast {
    let mut ast = ast;
    loop {
        match ast {
            Ast::Flags { child, .. } => ast = strip_nonsemantic_group(child),
            _ => return ast,
        }
    }
}

/// End of the ASCII whitespace run starting at `start`. Returns `None` when
/// the run hits a non-ASCII byte (Unicode whitespace such as NBSP could
/// extend it, so the caller must not gate).
fn whitespace_run_end(
    bytes: &[u8],
    start: usize,
    memo: &mut Option<(usize, usize)>,
) -> Option<usize> {
    if let Some((memo_start, memo_end)) = *memo
        && start >= memo_start
        && start < memo_end
    {
        return Some(memo_end);
    }
    let mut end = start;
    while let Some(byte) = bytes.get(end) {
        if matches!(*byte, b'\t' | b'\n' | 0x0b | 0x0c | b'\r' | b' ') {
            end += 1;
        } else if !byte.is_ascii() {
            return None;
        } else {
            break;
        }
    }
    if end > start {
        *memo = Some((start, end));
    }
    Some(end)
}

#[cfg(test)]
mod tests {
    use super::super::ast::parse;
    use super::*;

    const SEPARATOR: &str =
        r"((?:\s*+/\*(?:[^*]++|\*+(?!/))*+\*/\s*+)+|\s++|(?<=\W)|(?=\W)|^|\n?$|\A|\Z)";

    fn gate(pattern: &str) -> Option<SkipGate> {
        SkipGate::analyze(&parse(pattern))
    }

    fn allows(pattern: &str, line: &str, start: usize) -> bool {
        match gate(pattern).expect("pattern should have a gate").decide(
            line,
            start,
            AnchorContext::default(),
            &mut SkipGateLineState::default(),
        ) {
            SkipGateDecision::Allow => true,
            SkipGateDecision::Skip => false,
            SkipGateDecision::NeedsCommentCheck => {
                memchr::memmem::find(line.as_bytes(), b"/*").is_some()
            }
        }
    }

    #[test]
    fn separator_prefixed_keyword_gates_on_token_byte() {
        let pattern = format!("{SEPARATOR}((?<!\\w)this(?!\\w))");
        assert!(allows(&pattern, "this", 0));
        assert!(allows(&pattern, "  this", 0));
        assert!(allows(&pattern, "x  this", 1));
        assert!(!allows(&pattern, "  that_", 7));
        assert!(!allows(&pattern, "  #define", 0));
        // A block comment can hide the token, so the line is not gated.
        assert!(allows(&pattern, "/* c */ this", 0));
        assert!(allows(&pattern, "  /* c */ x", 0));
    }

    #[test]
    fn whitespace_prefixed_type_set_gates_on_first_letters() {
        let pattern = r"\s*+(?<!\w)(?:(unsigned|signed|double)(?!\w))";
        assert!(allows(pattern, "  unsigned x", 0));
        assert!(allows(pattern, "signed", 0));
        assert!(!allows(pattern, "  (cast)", 0));
        assert!(!allows(pattern, "  12345", 1));
    }

    #[test]
    fn mandatory_whitespace_requires_the_run() {
        let pattern = r"\s++(#)";
        // Empty separator is impossible, so `#` at the position itself is
        // not enough.
        assert!(!allows(pattern, "#x", 0));
        assert!(allows(pattern, "  #x", 0));
    }

    #[test]
    fn non_ascii_whitespace_disables_the_gate() {
        let pattern = format!("{SEPARATOR}(#)");
        // U+00A0 no-break space: byte scan must give up, not misjudge.
        assert!(allows(&pattern, " \u{a0} #", 0));
    }

    #[test]
    fn patterns_without_skip_shape_have_no_gate() {
        assert!(gate(r"[A-Za-z_]\w*").is_none());
        assert!(gate(r"(?<!\w)this").is_none());
        assert!(gate(r"\s*+\S+").is_none(), "rest may start with anything");
        assert!(
            gate(r"\s*+ ?#").is_none(),
            "rest starting with whitespace defeats the run shortcut"
        );
    }

    #[test]
    fn optional_prefix_elements_contribute_token_bytes() {
        // C++ declaration shape: optional attribute, separator, keywords.
        let pattern = format!(r"(\s*+(\[\[.*?]])?{SEPARATOR}(?:unsigned|long)\b");
        assert!(allows(&pattern, "  unsigned x", 0));
        assert!(allows(&pattern, "[[x]] long", 0));
        assert!(allows(&pattern, "  [[x]] long", 0));
        assert!(!allows(&pattern, "  (x)", 0));
        assert!(!allows(&pattern, "  signed", 1));
    }

    #[test]
    fn leading_anchors_and_lookbehinds_gate_start_positions() {
        let decide = |pattern: &str, line: &str, start: usize, ctx: AnchorContext| {
            gate(pattern).expect("pattern should have a gate").decide(
                line,
                start,
                ctx,
                &mut SkipGateLineState::default(),
            )
        };
        let line_start = AnchorContext::line_start();
        let pattern = format!(r"^({SEPARATOR}(#)\s*define)\b");
        assert_eq!(
            decide(&pattern, "#define", 0, line_start),
            SkipGateDecision::Allow
        );
        assert_eq!(
            decide(&pattern, "  #define", 1, line_start),
            SkipGateDecision::Skip
        );

        let pattern = r"(?:(?:^|\G|(?<=[;}]))|(?<=>|\*/))\s*+\w+";
        assert_eq!(
            decide(pattern, "a;b", 2, line_start),
            SkipGateDecision::Allow
        );
        assert_eq!(
            decide(pattern, "a*/b", 3, line_start),
            SkipGateDecision::Allow
        );
        assert_eq!(
            decide(pattern, "a,b", 2, line_start),
            SkipGateDecision::Skip
        );
        assert_eq!(
            decide(pattern, "a,b", 2, AnchorContext::continuation(2)),
            SkipGateDecision::Allow
        );

        // A lookbehind after a possibly consuming element does not describe
        // the start position.
        assert!(gate(r"\s?(?<=[ \t])x").is_some_and(|gate| gate.0.start.is_none()));
        // `^` pins the start even after a nullable consuming element.
        assert!(gate(r"\s*^x").is_some_and(|gate| gate.0.start.is_some()));
        // Case folding disables lookbehind byte sets.
        assert!(gate(r"(?i)(?<=k)\s+y").is_some_and(|gate| gate.0.start.is_none()));
    }

    #[test]
    fn case_insensitive_rest_bytes_cover_both_cases() {
        let gate = gate(r"(?i)\s*+(select|insert)\b").expect("gate");
        let mut state = SkipGateLineState::default();
        assert_eq!(
            gate.decide("  SELECT", 0, AnchorContext::default(), &mut state),
            SkipGateDecision::Allow
        );
        assert_eq!(
            gate.decide("  select", 0, AnchorContext::default(), &mut state),
            SkipGateDecision::Allow
        );
        let mut state = SkipGateLineState::default();
        assert_eq!(
            gate.decide("  update", 0, AnchorContext::default(), &mut state),
            SkipGateDecision::Skip
        );
    }
}
