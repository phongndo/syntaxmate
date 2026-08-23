use std::sync::Arc;

use super::ast::{AnchorKind, Ast, LookKind, ParsedRegex, parse};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Dfa,
    Fallback { reasons: Vec<&'static str> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorStrategy {
    None,
    TextStartGuard,
    LineStartGuard,
    ContinuationGuard,
    Fallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Translation {
    pub pattern: String,
    pub route: Route,
    pub anchor_strategy: AnchorStrategy,
    pub parsed: Arc<ParsedRegex>,
}

pub fn route(parsed: &ParsedRegex) -> Route {
    let reasons = fallback_reasons(parsed);
    if reasons.is_empty() {
        Route::Dfa
    } else {
        Route::Fallback { reasons }
    }
}

pub fn translate(pattern: &str) -> Translation {
    let parsed = Arc::new(parse(pattern));
    let mut reasons = fallback_reasons(&parsed);
    let (anchor_strategy, stripped) = anchor_strategy_and_stripped(pattern, &parsed);
    if anchor_strategy == AnchorStrategy::ContinuationGuard {
        reasons.retain(|reason| *reason != "\\G");
    }
    if anchor_strategy == AnchorStrategy::Fallback {
        reasons.push("anchor-context");
    }
    // Native AST matching does not need a rust-regex compile probe. Keep the
    // Oniguruma→Rust spelling normalization for diagnostics and tooling.
    let translated = normalize_oniguruma_for_rust_regex(stripped);
    let route = if reasons.is_empty() {
        Route::Dfa
    } else {
        Route::Fallback { reasons }
    };
    Translation {
        pattern: translated,
        route,
        anchor_strategy,
        parsed,
    }
}

fn fallback_reasons(parsed: &ParsedRegex) -> Vec<&'static str> {
    parsed.features.reasons()
}

fn anchor_strategy_and_stripped<'a>(
    pattern: &'a str,
    parsed: &ParsedRegex,
) -> (AnchorStrategy, &'a str) {
    // A leading anchor only licenses restricted search positions when every
    // top-level alternation branch provably starts at that anchor. Mixed
    // patterns such as `^#|//` or `\Gfoo|bar` have unanchored branches that
    // must remain searchable from any offset, so they fall back to the VM.
    if let Some(rest) = pattern.strip_prefix(r"\A")
        && every_branch_starts_with_anchor(&parsed.ast, AnchorKind::TextStart)
    {
        return (AnchorStrategy::TextStartGuard, rest);
    }
    if let Some(rest) = pattern.strip_prefix(r"\G")
        && every_branch_starts_with_anchor(&parsed.ast, AnchorKind::Continuation)
    {
        // A leading \G can be implemented by an anchored search at ctx.g_pos.
        return (AnchorStrategy::ContinuationGuard, rest);
    }
    if let Some(rest) = pattern.strip_prefix('^')
        && every_branch_starts_with_anchor(&parsed.ast, AnchorKind::LineStart)
    {
        return (AnchorStrategy::LineStartGuard, rest);
    }
    if parsed.features.anchor_g || parsed.features.anchor_a || parsed.features.line_anchor {
        // Non-leading anchors remain correct in the fallback VM. The D/Pike path
        // intentionally avoids ambiguous resume semantics for them.
        return (AnchorStrategy::Fallback, pattern);
    }
    (AnchorStrategy::None, pattern)
}

/// Whether the whole expression can only match at the given anchor position:
/// true when there is a single branch starting with the anchor, or when every
/// alternation branch does. Groups, inline flag scopes, and zero-width
/// lookarounds are transparent; anything else (notably empty branches and
/// leading literals) is conservatively rejected.
fn every_branch_starts_with_anchor(ast: &Ast, anchor: AnchorKind) -> bool {
    match ast {
        Ast::Alternation(branches) => branches
            .iter()
            .all(|branch| branch_starts_with_anchor(branch, anchor)),
        ast => branch_starts_with_anchor(ast, anchor),
    }
}

fn branch_starts_with_anchor(node: &Ast, anchor: AnchorKind) -> bool {
    match node {
        Ast::Anchor(kind) => *kind == anchor,
        Ast::Alternation(branches) => branches
            .iter()
            .all(|branch| branch_starts_with_anchor(branch, anchor)),
        Ast::Group { child, .. } | Ast::Flags { child, .. } => {
            branch_starts_with_anchor(child, anchor)
        }
        // A positive lookahead inherits the current position, so an anchor at
        // the start of every asserted branch constrains the whole match.
        // Negative assertions and lookbehinds do not: `(?!^x)y` commonly
        // matches away from line start, while `(?<=^x)y` starts after `x`.
        Ast::Look {
            kind: LookKind::Ahead,
            child,
        } => branch_starts_with_anchor(child, anchor),
        Ast::Concat(nodes) => nodes
            .first()
            .is_some_and(|first| branch_starts_with_anchor(first, anchor)),
        _ => false,
    }
}

pub fn normalize_oniguruma_for_rust_regex(pattern: &str) -> String {
    let mut out = String::with_capacity(pattern.len());
    let mut chars = pattern.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        let Some(next) = chars.next() else {
            out.push('\\');
            break;
        };
        match next {
            // Oniguruma/Onigmo \h is a hex digit class (not PCRE horizontal space).
            'h' => out.push_str(r"[0-9A-Fa-f]"),
            'H' => out.push_str(r"[^0-9A-Fa-f]"),
            // Oniguruma \R is any line break. Syntaxmate tokenizes one line at a time,
            // but keep the full spelling for conformance tests and fixture tools.
            'R' => out.push_str(r"(?:\r\n|[\n\v\f\r\u{85}\u{2028}\u{2029}])"),
            // Rust regex recognizes \z, not Oniguruma's before-final-newline \Z.
            // Lines passed to the tokenizer do not include final newlines, so the
            // stricter end anchor is the right deterministic lowering here.
            'Z' => out.push_str(r"\z"),
            other => {
                out.push('\\');
                out.push(other);
            }
        }
    }
    out
}

pub fn is_ast_translatable(ast: &Ast) -> bool {
    match ast {
        Ast::Backref(_)
        | Ast::Conditional { .. }
        | Ast::Subroutine(_)
        | Ast::Look { .. }
        | Ast::Unsupported(_) => false,
        Ast::Repeat {
            node, possessive, ..
        } => !*possessive && is_ast_translatable(node),
        Ast::Concat(nodes) | Ast::Alternation(nodes) => nodes.iter().all(is_ast_translatable),
        Ast::Group { child, .. } | Ast::Flags { child, .. } => is_ast_translatable(child),
        Ast::Empty | Ast::Literal(_) | Ast::Dot | Ast::Class(_) | Ast::Anchor(_) => true,
        Ast::Grapheme => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_plain_regex_to_dfa() {
        let translated = translate(r"\bfoo\w+");
        assert_eq!(translated.route, Route::Dfa);
    }

    #[test]
    fn routes_lookaround_to_fallback() {
        let translated = translate(r"foo(?=bar)");
        assert!(matches!(translated.route, Route::Fallback { .. }));
    }

    #[test]
    fn lowers_hex_digit_class() {
        assert_eq!(normalize_oniguruma_for_rust_regex(r"\h+"), r"[0-9A-Fa-f]+");
    }

    #[test]
    fn chooses_anchor_strategy() {
        assert_eq!(
            translate(r"\Afoo").anchor_strategy,
            AnchorStrategy::TextStartGuard
        );
        assert_eq!(
            translate(r"\Gfoo").anchor_strategy,
            AnchorStrategy::ContinuationGuard
        );
        assert_eq!(
            translate("^foo").anchor_strategy,
            AnchorStrategy::LineStartGuard
        );
    }

    #[test]
    fn leading_g_is_dfa_routable() {
        let translated = translate(r"\Gfoo");
        assert_eq!(translated.route, Route::Dfa);
        assert_eq!(
            translated.anchor_strategy,
            AnchorStrategy::ContinuationGuard
        );
    }

    #[test]
    fn mixed_anchor_alternations_fall_back() {
        // Unanchored branches must stay searchable from any offset.
        assert_eq!(
            translate(r"^#|//").anchor_strategy,
            AnchorStrategy::Fallback
        );
        assert_eq!(
            translate(r"\Gfoo|bar").anchor_strategy,
            AnchorStrategy::Fallback
        );
        assert_eq!(
            translate(r"\Afoo|bar").anchor_strategy,
            AnchorStrategy::Fallback
        );
    }

    #[test]
    fn empty_branch_blocks_anchor_guard() {
        // `\G|(,)`: the empty branch matches at any position.
        assert_eq!(
            translate(r"\G|(,)").anchor_strategy,
            AnchorStrategy::Fallback
        );
    }

    #[test]
    fn fully_anchored_alternations_keep_guard() {
        assert_eq!(
            translate(r"^foo|^bar").anchor_strategy,
            AnchorStrategy::LineStartGuard
        );
        assert_eq!(
            translate(r"\Gfoo|\Gbar").anchor_strategy,
            AnchorStrategy::ContinuationGuard
        );
        assert_eq!(
            translate(r"^foo|(?=^bar)bar").anchor_strategy,
            AnchorStrategy::LineStartGuard
        );
        assert_eq!(
            translate(r"^foo|(?:^bar|^baz)").anchor_strategy,
            AnchorStrategy::LineStartGuard
        );
    }

    #[test]
    fn negative_assertions_and_lookbehinds_do_not_license_anchor_guards() {
        for pattern in [
            r"^foo|(?!^bar)bar",
            r"^foo|(?<=^bar)baz",
            r"^foo|(?<!^bar)baz",
        ] {
            assert_eq!(
                translate(pattern).anchor_strategy,
                AnchorStrategy::Fallback,
                "{pattern:?}"
            );
        }

        // The explicit automata entry point uses the strategy as a search
        // gate even when its native VM handles a fallback construct.
        use super::super::{AnchorContext, AutomataMatcher, Matcher};
        let matcher = AutomataMatcher::new(r"^foo|(?!^foo)bar").unwrap();
        let matched = matcher
            .find("xx bar", 3, AnchorContext::line_start())
            .expect("negative assertion branch remains searchable after resume");
        assert_eq!(matched.start..matched.end, 3..6);
    }

    #[test]
    fn anchored_prefix_with_nullable_tail_keeps_guard() {
        // The anchor is the first element of the only branch; every match
        // still starts at the anchored position.
        assert_eq!(
            translate(r"^\s*foo").anchor_strategy,
            AnchorStrategy::LineStartGuard
        );
        assert_eq!(
            translate(r"^(?:foo|bar)").anchor_strategy,
            AnchorStrategy::LineStartGuard
        );
    }

    #[test]
    fn flag_prefixed_anchor_stays_conservative() {
        // The source does not start with the anchor, so no prefix stripping
        // applies; the VM handles the anchor itself.
        assert_eq!(
            translate(r"(?i)^foo").anchor_strategy,
            AnchorStrategy::Fallback
        );
    }

    #[test]
    fn mixed_anchor_pattern_matches_after_resume_offset() {
        // Regression: `^#|//` used to be locked to line-start searches, so a
        // scan resumed past an intervening token could never see `//`.
        use super::super::{AnchorContext, FallbackMatcher, Matcher, RegexMatcher};
        let pattern = r"^#|//";
        let line = "  // note";
        let ctx = AnchorContext {
            allow_a: true,
            allow_g: false,
            g_pos: 0,
        };
        let reference = FallbackMatcher::new(pattern)
            .find(line, 2, ctx)
            .expect("fallback engine finds the unanchored branch");
        assert_eq!(reference.start..reference.end, 2..4);
        let matcher = RegexMatcher::new(pattern);
        let result = matcher.find(line, 2, ctx);
        assert_eq!(
            result.map(|matched| matched.start..matched.end),
            Some(2..4),
            "auto-routed engine must agree with the fallback engine"
        );
    }
}
