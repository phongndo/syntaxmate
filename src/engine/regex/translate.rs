use std::sync::Arc;

use super::ast::{Ast, ParsedRegex, parse};

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
    if let Some(rest) = pattern.strip_prefix(r"\A") {
        return (AnchorStrategy::TextStartGuard, rest);
    }
    if let Some(rest) = pattern.strip_prefix(r"\G") {
        // A leading \G can be implemented by an anchored search at ctx.g_pos.
        return (AnchorStrategy::ContinuationGuard, rest);
    }
    if let Some(rest) = pattern.strip_prefix('^') {
        return (AnchorStrategy::LineStartGuard, rest);
    }
    if parsed.features.anchor_g || parsed.features.anchor_a || parsed.features.line_anchor {
        // Non-leading anchors remain correct in the fallback VM. The D/Pike path
        // intentionally avoids ambiguous resume semantics for them.
        return (AnchorStrategy::Fallback, pattern);
    }
    (AnchorStrategy::None, pattern)
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
}
