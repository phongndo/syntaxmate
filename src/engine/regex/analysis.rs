//! Immutable, shared metadata derived from one parsed regex.
//!
//! Matcher construction used to rediscover the same properties independently
//! in the fallback matcher, prefilter, candidate scanner, start-class gate,
//! skip-prefix gate, and capture bytecode setup. `RegexAnalysis` is the single
//! ownership boundary for those answers. Consumers may build their own compact
//! runtime tables from it, but they do not walk the AST again to classify the
//! pattern.

use super::ast::{Ast, Backref, ParsedRegex, RegexFlags};
use super::backtrack::{StartByteSet, expand_case_insensitive_start_bytes, first_start_bytes};
use super::prefilter::{Prefilter, required_literals};
use super::skip_prefix::SkipGate;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CaptureAnalysis {
    referenced_groups: Box<[u32]>,
    capture_bytecode_supported: bool,
    position_only_eligible: bool,
    selection_requires_captures: bool,
}

impl CaptureAnalysis {
    pub(crate) fn referenced_groups(&self) -> &[u32] {
        &self.referenced_groups
    }

    pub(crate) fn capture_bytecode_supported(&self) -> bool {
        self.capture_bytecode_supported
    }

    pub(crate) fn position_only_eligible(&self) -> bool {
        self.position_only_eligible
    }

    pub(crate) fn selection_requires_captures(&self) -> bool {
        self.selection_requires_captures
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegexAnalysis {
    uniform_effective_flags: Option<RegexFlags>,
    has_case_insensitive_scope: bool,
    prefilter_case_insensitive: Option<bool>,
    prefilter: OnceLock<Prefilter>,
    start_bytes: Option<StartByteSet>,
    start_nullable: bool,
    start_class_mask: u8,
    skip_gate: Option<SkipGate>,
    capture: CaptureAnalysis,
    scanner_supported: bool,
    bytecode_beneficial: bool,
}

impl RegexAnalysis {
    pub(crate) fn new(parsed: &ParsedRegex) -> Self {
        // Effective flags are shared by start-byte, required-literal, and
        // skip-prefix analysis. Compute them once before deriving those views.
        let flags = analyze_effective_flags(&parsed.ast);
        let uniform_effective_flags = flags.uniform;
        let has_case_insensitive_scope = flags.has_case_insensitive_scope;
        let prefilter_case_insensitive = prefilter_case_policy(parsed, &flags);

        let (start_bytes, start_nullable) =
            analyze_start_bytes(parsed, uniform_effective_flags, has_case_insensitive_scope);
        let capture = analyze_captures(parsed);
        let scanner_supported = super::scanner::Scanner::supports(parsed);
        let bytecode_beneficial = super::bytecode::Program::is_beneficial(parsed);
        let skip_gate = SkipGate::analyze_with_effective_flags(
            parsed,
            uniform_effective_flags,
            has_case_insensitive_scope,
        );

        Self {
            uniform_effective_flags,
            has_case_insensitive_scope,
            prefilter_case_insensitive,
            prefilter: OnceLock::new(),
            start_bytes,
            start_nullable,
            start_class_mask: super::start_class::start_class_mask(parsed),
            skip_gate,
            capture,
            scanner_supported,
            bytecode_beneficial,
        }
    }

    pub(crate) fn uniform_effective_flags(&self) -> Option<RegexFlags> {
        self.uniform_effective_flags
    }

    pub(crate) fn has_case_insensitive_scope(&self) -> bool {
        self.has_case_insensitive_scope
    }

    pub(crate) fn prefilter<'a>(&'a self, parsed: &ParsedRegex) -> &'a Prefilter {
        self.prefilter.get_or_init(|| {
            let Some(case_fold) = self.prefilter_case_insensitive else {
                return Prefilter::None;
            };
            Prefilter::from_required(required_literals(&parsed.ast), case_fold)
        })
    }

    pub(crate) fn start_bytes(&self) -> Option<&StartByteSet> {
        self.start_bytes.as_ref()
    }

    pub(crate) fn start_nullable(&self) -> bool {
        self.start_nullable
    }

    pub(crate) fn start_class_mask(&self) -> u8 {
        self.start_class_mask
    }

    pub(crate) fn skip_gate(&self) -> Option<&SkipGate> {
        self.skip_gate.as_ref()
    }

    pub(crate) fn capture(&self) -> &CaptureAnalysis {
        &self.capture
    }

    pub(crate) fn scanner_supported(&self) -> bool {
        self.scanner_supported
    }

    pub(crate) fn bytecode_beneficial(&self) -> bool {
        self.bytecode_beneficial
    }
}

fn analyze_start_bytes(
    parsed: &ParsedRegex,
    uniform_flags: Option<RegexFlags>,
    has_case_insensitive_scope: bool,
) -> (Option<StartByteSet>, bool) {
    if has_case_insensitive_scope && uniform_flags.is_none() {
        return (None, false);
    }
    match first_start_bytes(&parsed.ast) {
        Some(mut info) if !info.bytes.is_empty() => {
            if uniform_flags.unwrap_or(parsed.flags).case_insensitive {
                expand_case_insensitive_start_bytes(&mut info.bytes);
            }
            if info.bytes.len() < 128 {
                (Some(info.bytes), info.nullable)
            } else {
                (None, info.nullable)
            }
        }
        Some(info) => (None, info.nullable),
        None => (None, false),
    }
}

fn prefilter_case_policy(parsed: &ParsedRegex, flags: &EffectiveFlagsAnalysis) -> Option<bool> {
    if let Some(uniform) = flags.uniform
        && flags.has_case_insensitive_scope
    {
        return Some(uniform.case_insensitive);
    }
    if let Some(root_flags) = flags.root_flags_without_nested_scope {
        return Some(root_flags.case_insensitive);
    }
    if parsed.flags.case_insensitive {
        Some(true)
    } else if flags.has_case_insensitive_scope {
        None
    } else {
        Some(false)
    }
}

#[derive(Clone, Copy)]
struct EffectiveFlagsAnalysis {
    uniform: Option<RegexFlags>,
    has_case_insensitive_scope: bool,
    root_flags_without_nested_scope: Option<RegexFlags>,
}

#[derive(Clone, Copy)]
struct FlagNodeAnalysis {
    uniform: Result<Option<RegexFlags>, ()>,
    has_case_insensitive_scope: bool,
    has_flag_scope: bool,
}

fn analyze_effective_flags(ast: &Ast) -> EffectiveFlagsAnalysis {
    fn combine(nodes: impl IntoIterator<Item = FlagNodeAnalysis>) -> FlagNodeAnalysis {
        let mut uniform = None;
        let mut mixed = false;
        let mut has_case_insensitive_scope = false;
        let mut has_flag_scope = false;
        for node in nodes {
            has_case_insensitive_scope |= node.has_case_insensitive_scope;
            has_flag_scope |= node.has_flag_scope;
            match node.uniform {
                Ok(Some(node_flags)) => {
                    mixed |= uniform.is_some_and(|flags| flags != node_flags);
                    uniform = Some(node_flags);
                }
                Ok(None) => {}
                Err(()) => mixed = true,
            }
        }
        FlagNodeAnalysis {
            uniform: if mixed { Err(()) } else { Ok(uniform) },
            has_case_insensitive_scope,
            has_flag_scope,
        }
    }

    fn visit(ast: &Ast, inherited: RegexFlags) -> FlagNodeAnalysis {
        match ast {
            Ast::Empty => FlagNodeAnalysis {
                uniform: Ok(None),
                has_case_insensitive_scope: false,
                has_flag_scope: false,
            },
            Ast::Flags { flags, child } => {
                let child = visit(child, *flags);
                FlagNodeAnalysis {
                    uniform: child.uniform,
                    has_case_insensitive_scope: flags.case_insensitive
                        || child.has_case_insensitive_scope,
                    has_flag_scope: true,
                }
            }
            Ast::Concat(nodes) | Ast::Alternation(nodes) => {
                combine(nodes.iter().map(|node| visit(node, inherited)))
            }
            Ast::Conditional {
                matched, unmatched, ..
            } => combine([visit(matched, inherited), visit(unmatched, inherited)]),
            Ast::Repeat { node, .. }
            | Ast::Group { child: node, .. }
            | Ast::Look { child: node, .. } => visit(node, inherited),
            _ => FlagNodeAnalysis {
                uniform: Ok(Some(inherited)),
                has_case_insensitive_scope: false,
                has_flag_scope: false,
            },
        }
    }

    let (root, root_flags_without_nested_scope) = match ast {
        Ast::Flags { flags, child } => {
            let child = visit(child, *flags);
            let root_flags = (!child.has_flag_scope).then_some(*flags);
            (
                FlagNodeAnalysis {
                    uniform: child.uniform,
                    has_case_insensitive_scope: flags.case_insensitive
                        || child.has_case_insensitive_scope,
                    has_flag_scope: true,
                },
                root_flags,
            )
        }
        _ => (visit(ast, RegexFlags::default()), None),
    };
    EffectiveFlagsAnalysis {
        uniform: root.uniform.ok().flatten(),
        has_case_insensitive_scope: root.has_case_insensitive_scope,
        root_flags_without_nested_scope,
    }
}

fn analyze_captures(parsed: &ParsedRegex) -> CaptureAnalysis {
    let mut referenced_groups = Vec::new();
    collect_referenced_groups(&parsed.ast, parsed, &mut referenced_groups);
    referenced_groups.sort_unstable();
    referenced_groups.dedup();
    let features = &parsed.features;
    CaptureAnalysis {
        referenced_groups: referenced_groups.into_boxed_slice(),
        capture_bytecode_supported: capture_ast_supported(&parsed.ast),
        position_only_eligible: parsed.capture_count > 0
            && !features.backreference
            && !features.subroutine
            && !features.possessive_or_atomic
            && !features.conditional
            && !features.unsupported_escape,
        selection_requires_captures: features.backreference
            || features.conditional
            || features.subroutine,
    }
}

fn capture_ast_supported(ast: &Ast) -> bool {
    match ast {
        Ast::Grapheme | Ast::Unsupported(_) => false,
        Ast::Repeat { node, .. }
        | Ast::Group { child: node, .. }
        | Ast::Look { child: node, .. }
        | Ast::Flags { child: node, .. } => capture_ast_supported(node),
        Ast::Concat(nodes) | Ast::Alternation(nodes) => nodes.iter().all(capture_ast_supported),
        Ast::Conditional {
            matched, unmatched, ..
        } => capture_ast_supported(matched) && capture_ast_supported(unmatched),
        Ast::Empty
        | Ast::Literal(_)
        | Ast::Dot
        | Ast::Class(_)
        | Ast::Anchor(_)
        | Ast::Backref(_)
        | Ast::Subroutine(_) => true,
    }
}

fn collect_referenced_groups(ast: &Ast, parsed: &ParsedRegex, groups: &mut Vec<u32>) {
    match ast {
        Ast::Backref(backref) => push_backref_group(backref, parsed, groups),
        Ast::Concat(nodes) | Ast::Alternation(nodes) => {
            for node in nodes {
                collect_referenced_groups(node, parsed, groups);
            }
        }
        Ast::Repeat { node, .. }
        | Ast::Group { child: node, .. }
        | Ast::Look { child: node, .. }
        | Ast::Flags { child: node, .. } => collect_referenced_groups(node, parsed, groups),
        Ast::Conditional {
            condition,
            matched,
            unmatched,
        } => {
            push_backref_group(condition, parsed, groups);
            collect_referenced_groups(matched, parsed, groups);
            collect_referenced_groups(unmatched, parsed, groups);
        }
        Ast::Empty
        | Ast::Literal(_)
        | Ast::Dot
        | Ast::Grapheme
        | Ast::Class(_)
        | Ast::Anchor(_)
        | Ast::Subroutine(_)
        | Ast::Unsupported(_) => {}
    }
}

fn push_backref_group(backref: &Backref, parsed: &ParsedRegex, groups: &mut Vec<u32>) {
    match backref {
        Backref::Number(group) => groups.push(*group),
        Backref::Name(name) => {
            if let Some(group) = parsed.named_captures.get(name) {
                groups.push(*group);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::regex::ast::parse;

    #[test]
    fn analysis_collects_shared_matcher_metadata() {
        let parsed = parse(r"(?i:\s*+(?<word>select|insert)\s+\k<word>)");
        let analysis = parsed.analysis();

        assert_eq!(
            analysis.uniform_effective_flags(),
            Some(RegexFlags {
                case_insensitive: true,
                ..RegexFlags::default()
            })
        );
        assert!(analysis.has_case_insensitive_scope());
        assert_eq!(analysis.capture().referenced_groups(), &[1]);
        assert!(analysis.capture().selection_requires_captures());
        assert!(analysis.start_class_mask() != 0);
    }

    #[test]
    fn parsed_regex_caches_one_analysis_without_changing_equality() {
        let parsed = parse(r"(?i:foo|bar)");
        let equal = parse(r"(?i:foo|bar)");
        assert_eq!(parsed, equal);

        let first = parsed.analysis();
        let second = parsed.analysis();
        assert!(std::ptr::eq(first, second));
        assert!(std::ptr::eq(parsed.prefilter(), parsed.prefilter()));
        assert_eq!(parsed, equal, "cache initialization is not regex identity");
    }

    #[test]
    fn mixed_case_scopes_disable_shared_byte_and_literal_gates() {
        let parsed = parse(r"(?i:foo)(?-i:bar)");
        let analysis = parsed.analysis();

        assert!(analysis.uniform_effective_flags().is_none());
        assert!(analysis.start_bytes().is_none());
        assert!(!analysis.prefilter(&parsed).is_enabled());
    }
}
