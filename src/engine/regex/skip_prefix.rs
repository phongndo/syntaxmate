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
//! This gate statically splits the pattern into skip elements and the rest,
//! records the rest's possible first bytes, and lets the scan skip the
//! anchored attempt whenever the rest cannot start at the position itself
//! (zero-width separator), at the end of the shared whitespace run
//! (whitespace branches), or through a `/*` comment path (gated by a
//! per-line block-comment check).
//!
//! Gates are over-approximations: `allows` may return true for a position
//! with no real match, but must never return false for one that has any.

use super::ast::{
    Ast, ParsedRegex, PerlClassKind, has_case_insensitive_scope, uniform_effective_flags,
};
use super::backtrack::{
    StartByteSet, concat_start_bytes, expand_case_insensitive_start_bytes,
    is_cpp_space_comment_separator, is_perl_class, strip_nonsemantic_group,
};

const ASCII_WHITESPACE: [u8; 6] = [b' ', b'\t', b'\n', b'\r', 0x0b, 0x0c];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SkipGate {
    rest_bytes: StartByteSet,
    /// The skip prefix can match empty, so the rest may start at the scan
    /// position itself.
    allow_empty: bool,
    /// A whitespace-consuming skip path exists, so the rest may start at the
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
    pub(crate) fn analyze(parsed: &ParsedRegex) -> Option<Self> {
        let mut ast = &parsed.ast;
        while let Ast::Flags { child, .. } = ast {
            ast = child;
        }
        let Ast::Concat(nodes) = ast else {
            return None;
        };
        let mut allow_empty = true;
        let mut allow_whitespace = false;
        let mut allow_comment = false;
        let mut index = 0;
        while index < nodes.len() {
            match classify_skip_element(&nodes[index]) {
                Some(SkipElement::Separator) => {
                    allow_whitespace = true;
                    allow_comment = true;
                }
                Some(SkipElement::Whitespace { nullable }) => {
                    allow_whitespace = true;
                    allow_empty &= nullable;
                }
                Some(SkipElement::ZeroWidth) => {}
                None => break,
            }
            index += 1;
        }
        if index == 0 || !(allow_whitespace || allow_comment) {
            return None;
        }
        let rest = &nodes[index..];
        if rest.is_empty() {
            return None;
        }
        // Mirror the fallback matcher's start-byte policy: bail out on mixed
        // case-insensitive scopes, expand ASCII case pairs (plus non-ASCII
        // lead bytes) when the effective flags fold case.
        let uniform_flags = uniform_effective_flags(&parsed.ast);
        if has_case_insensitive_scope(&parsed.ast) && uniform_flags.is_none() {
            return None;
        }
        let info = concat_start_bytes(rest)?;
        if info.nullable || info.bytes.is_empty() {
            return None;
        }
        let mut rest_bytes = info.bytes;
        if uniform_flags.unwrap_or(parsed.flags).case_insensitive {
            expand_case_insensitive_start_bytes(&mut rest_bytes);
        }
        // The whitespace-run shortcut assumes the rest cannot begin inside
        // the run.
        if ASCII_WHITESPACE
            .iter()
            .any(|byte| rest_bytes.contains(*byte))
        {
            return None;
        }
        Some(Self {
            rest_bytes,
            allow_empty,
            allow_whitespace,
            allow_comment,
        })
    }

    /// Whether an anchored attempt at `start` can possibly match, using only
    /// the cheap byte checks.
    pub(crate) fn decide(
        &self,
        line: &str,
        start: usize,
        state: &mut SkipGateLineState,
    ) -> SkipGateDecision {
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

enum SkipElement {
    /// The exact C-family comment-or-whitespace separator alternation.
    Separator,
    /// A pure `\s` repeat (or single `\s`); `nullable` when it can match
    /// empty.
    Whitespace { nullable: bool },
    /// Consumes nothing (anchors, lookarounds).
    ZeroWidth,
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
    consumes_nothing(stripped).then_some(SkipElement::ZeroWidth)
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

/// True when the node can never consume input.
fn consumes_nothing(ast: &Ast) -> bool {
    match ast {
        Ast::Empty | Ast::Anchor(_) | Ast::Look { .. } => true,
        Ast::Literal(literal) => literal.is_empty(),
        Ast::Group { child, .. } | Ast::Flags { child, .. } => consumes_nothing(child),
        Ast::Concat(nodes) => nodes.iter().all(consumes_nothing),
        Ast::Alternation(branches) => !branches.is_empty() && branches.iter().all(consumes_nothing),
        Ast::Repeat { node, max, .. } => *max == Some(0) || consumes_nothing(node),
        _ => false,
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
        if ASCII_WHITESPACE.contains(byte) {
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
    fn case_insensitive_rest_bytes_cover_both_cases() {
        let gate = gate(r"(?i)\s*+(select|insert)\b").expect("gate");
        let mut state = SkipGateLineState::default();
        assert_eq!(
            gate.decide("  SELECT", 0, &mut state),
            SkipGateDecision::Allow
        );
        assert_eq!(
            gate.decide("  select", 0, &mut state),
            SkipGateDecision::Allow
        );
        let mut state = SkipGateLineState::default();
        assert_eq!(
            gate.decide("  update", 0, &mut state),
            SkipGateDecision::Skip
        );
    }
}
