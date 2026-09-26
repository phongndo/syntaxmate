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
use super::prefilter::{Prefilter, required_factor, required_literals};
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
    scanner_end_exact: bool,
    bytecode_beneficial: bool,
    instruction_capacity_hint: usize,
}

impl RegexAnalysis {
    pub(crate) fn new(parsed: &ParsedRegex) -> Self {
        // One structural walk derives every whole-tree property; separate
        // walks each paid the pointer-chasing cost of the full AST. Effective
        // flags are then shared by start-byte, required-literal, and
        // skip-prefix analysis.
        let summary = summarize_ast(parsed);
        let flags = summary.flags;
        let uniform_effective_flags = flags.uniform;
        let has_case_insensitive_scope = flags.has_case_insensitive_scope;
        let prefilter_case_insensitive = prefilter_case_policy(parsed, &flags);

        let (start_bytes, start_nullable) =
            analyze_start_bytes(parsed, uniform_effective_flags, has_case_insensitive_scope);
        let capture = analyze_captures(
            parsed,
            summary.referenced_groups,
            summary.capture_bytecode_supported,
        );
        let scanner_supported = super::scanner::Scanner::supports(parsed);
        let scanner_end_exact = super::scanner::match_end_is_exact(parsed);
        let bytecode_beneficial =
            super::bytecode::Program::is_beneficial_fanout(summary.ordered_fanout);
        let skip_gate = SkipGate::analyze_with_effective_flags(
            parsed,
            uniform_effective_flags,
            has_case_insensitive_scope,
            start_bytes.is_some() && !start_nullable,
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
            scanner_end_exact,
            bytecode_beneficial,
            instruction_capacity_hint: summary.instruction_capacity_hint,
        }
    }

    /// Instruction count estimate for bytecode compilation, derived here so
    /// lazy compilation does not walk a cold AST just to size its arena.
    pub(crate) fn instruction_capacity_hint(&self) -> usize {
        self.instruction_capacity_hint
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
            let literals = required_literals(&parsed.ast);
            // Byte-class runs are only derived for wholly case-sensitive
            // patterns; case folding can map ASCII to non-ASCII characters.
            if !case_fold
                && !self.has_case_insensitive_scope
                && !parsed.flags.case_insensitive
                && let Some(factor) = required_factor(&parsed.ast)
            {
                return Prefilter::Factor {
                    factor,
                    literals: Box::new(Prefilter::from_required(literals, false)),
                };
            }
            Prefilter::from_required(literals, case_fold)
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

    /// See [`super::scanner::match_end_is_exact`].
    pub(crate) fn scanner_end_exact(&self) -> bool {
        self.scanner_end_exact
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
    // With mixed case scopes, fold the whole set: case-insensitive expansion
    // only adds candidates, so it stays a superset for case-sensitive parts.
    let case_insensitive = match uniform_flags {
        Some(flags) => flags.case_insensitive,
        None => has_case_insensitive_scope || parsed.flags.case_insensitive,
    };
    match first_start_bytes(&parsed.ast) {
        Some(mut info) if !info.bytes.is_empty() => {
            if case_insensitive {
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
    // Mixed case scopes search every required literal case-insensitively:
    // that finds a superset of the case-sensitive occurrences, so the
    // rejection gate stays free of false negatives.
    Some(parsed.flags.case_insensitive || flags.has_case_insensitive_scope)
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

impl FlagNodeAnalysis {
    const EMPTY: Self = Self {
        uniform: Ok(None),
        has_case_insensitive_scope: false,
        has_flag_scope: false,
    };

    fn leaf(inherited: RegexFlags) -> Self {
        Self {
            uniform: Ok(Some(inherited)),
            has_case_insensitive_scope: false,
            has_flag_scope: false,
        }
    }

    fn scoped(flags: RegexFlags, child: Self) -> Self {
        Self {
            uniform: child.uniform,
            has_case_insensitive_scope: flags.case_insensitive || child.has_case_insensitive_scope,
            has_flag_scope: true,
        }
    }
}

/// Combines sibling flag analyses: the union is uniform only when every
/// non-empty sibling reports the same effective flags.
struct FlagCombiner {
    uniform: Option<RegexFlags>,
    mixed: bool,
    has_case_insensitive_scope: bool,
    has_flag_scope: bool,
}

impl FlagCombiner {
    fn new() -> Self {
        Self {
            uniform: None,
            mixed: false,
            has_case_insensitive_scope: false,
            has_flag_scope: false,
        }
    }

    fn push(&mut self, node: FlagNodeAnalysis) {
        self.has_case_insensitive_scope |= node.has_case_insensitive_scope;
        self.has_flag_scope |= node.has_flag_scope;
        match node.uniform {
            Ok(Some(node_flags)) => {
                self.mixed |= self.uniform.is_some_and(|flags| flags != node_flags);
                self.uniform = Some(node_flags);
            }
            Ok(None) => {}
            Err(()) => self.mixed = true,
        }
    }

    fn finish(self) -> FlagNodeAnalysis {
        FlagNodeAnalysis {
            uniform: if self.mixed {
                Err(())
            } else {
                Ok(self.uniform)
            },
            has_case_insensitive_scope: self.has_case_insensitive_scope,
            has_flag_scope: self.has_flag_scope,
        }
    }
}

/// Whole-tree facts gathered by one traversal of a parsed regex.
struct AstSummary {
    flags: EffectiveFlagsAnalysis,
    /// Valid backreference and conditional-test groups, unsorted.
    referenced_groups: Vec<u32>,
    /// No grapheme or unsupported node occurs anywhere in the tree.
    capture_bytecode_supported: bool,
    /// Ordered choice points: extra alternation branches, repeats of
    /// non-trivial nodes, and conditionals.
    ordered_fanout: usize,
    instruction_capacity_hint: usize,
}

struct SummaryWalk<'a> {
    parsed: &'a ParsedRegex,
    referenced_groups: Vec<u32>,
    capture_bytecode_supported: bool,
    ordered_fanout: usize,
    instruction_capacity_hint: usize,
}

impl SummaryWalk<'_> {
    fn add_instructions(&mut self, count: usize) {
        self.instruction_capacity_hint = self.instruction_capacity_hint.saturating_add(count);
    }

    fn add_fanout(&mut self, count: usize) {
        self.ordered_fanout = self.ordered_fanout.saturating_add(count);
    }

    fn leaf(&mut self, inherited: RegexFlags) -> FlagNodeAnalysis {
        self.add_instructions(1);
        FlagNodeAnalysis::leaf(inherited)
    }

    fn visit(&mut self, ast: &Ast, inherited: RegexFlags) -> FlagNodeAnalysis {
        match ast {
            Ast::Empty => FlagNodeAnalysis::EMPTY,
            Ast::Flags { flags, child } => {
                let child = self.visit(child, *flags);
                FlagNodeAnalysis::scoped(*flags, child)
            }
            Ast::Concat(nodes) => {
                let mut combined = FlagCombiner::new();
                for node in nodes {
                    combined.push(self.visit(node, inherited));
                }
                combined.finish()
            }
            Ast::Alternation(branches) => {
                let extra = branches.len().saturating_sub(1);
                self.add_fanout(extra);
                self.add_instructions(extra.saturating_mul(2));
                let mut combined = FlagCombiner::new();
                for branch in branches {
                    combined.push(self.visit(branch, inherited));
                }
                combined.finish()
            }
            Ast::Conditional {
                condition,
                matched,
                unmatched,
            } => {
                push_backref_group(condition, self.parsed, &mut self.referenced_groups);
                self.add_fanout(1);
                self.add_instructions(1);
                let mut combined = FlagCombiner::new();
                combined.push(self.visit(matched, inherited));
                combined.push(self.visit(unmatched, inherited));
                combined.finish()
            }
            Ast::Repeat { node, .. } => {
                self.add_fanout(usize::from(!matches!(
                    node.as_ref(),
                    Ast::Literal(_) | Ast::Class(_) | Ast::Dot
                )));
                self.add_instructions(3);
                self.visit(node, inherited)
            }
            Ast::Group { child, .. } => self.visit(child, inherited),
            Ast::Look { child, .. } => {
                self.add_instructions(2);
                self.visit(child, inherited)
            }
            Ast::Backref(backref) => {
                push_backref_group(backref, self.parsed, &mut self.referenced_groups);
                self.leaf(inherited)
            }
            Ast::Grapheme | Ast::Unsupported(_) => {
                self.capture_bytecode_supported = false;
                self.leaf(inherited)
            }
            Ast::Literal(_) | Ast::Dot | Ast::Class(_) | Ast::Anchor(_) | Ast::Subroutine(_) => {
                self.leaf(inherited)
            }
        }
    }
}

fn summarize_ast(parsed: &ParsedRegex) -> AstSummary {
    let mut walk = SummaryWalk {
        parsed,
        referenced_groups: Vec::new(),
        capture_bytecode_supported: true,
        ordered_fanout: 0,
        instruction_capacity_hint: 0,
    };
    let (root, root_flags_without_nested_scope) = match &parsed.ast {
        Ast::Flags { flags, child } => {
            let child = walk.visit(child, *flags);
            let root_flags = (!child.has_flag_scope).then_some(*flags);
            (FlagNodeAnalysis::scoped(*flags, child), root_flags)
        }
        ast => (walk.visit(ast, RegexFlags::default()), None),
    };
    AstSummary {
        flags: EffectiveFlagsAnalysis {
            uniform: root.uniform.ok().flatten(),
            has_case_insensitive_scope: root.has_case_insensitive_scope,
            root_flags_without_nested_scope,
        },
        referenced_groups: walk.referenced_groups,
        capture_bytecode_supported: walk.capture_bytecode_supported,
        ordered_fanout: walk.ordered_fanout,
        instruction_capacity_hint: walk.instruction_capacity_hint,
    }
}

fn analyze_captures(
    parsed: &ParsedRegex,
    mut referenced_groups: Vec<u32>,
    capture_bytecode_supported: bool,
) -> CaptureAnalysis {
    referenced_groups.sort_unstable();
    referenced_groups.dedup();
    let features = &parsed.features;
    CaptureAnalysis {
        referenced_groups: referenced_groups.into_boxed_slice(),
        capture_bytecode_supported,
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

fn push_backref_group(backref: &Backref, parsed: &ParsedRegex, groups: &mut Vec<u32>) {
    match backref {
        Backref::Number(group) => {
            if *group != 0 && *group <= parsed.capture_count {
                groups.push(*group);
            }
        }
        Backref::Name(name) => {
            if let Some(shared) = parsed.duplicate_names.get(name) {
                groups.extend_from_slice(shared);
            } else if let Some(group) = parsed.named_captures.get(name) {
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
    fn out_of_range_numeric_backrefs_are_not_collected() {
        let parsed = parse(r"(?<=:)\3*(?<value>[^,}]+)");
        assert_eq!(parsed.capture_count, 1);
        assert!(parsed.analysis().capture().referenced_groups().is_empty());

        let parsed = parse(r"(a)(b)(c)\4(x)(y)\6");
        assert_eq!(parsed.capture_count, 5);
        assert_eq!(parsed.analysis().capture().referenced_groups(), &[4]);
    }

    #[test]
    fn mixed_case_scopes_use_case_folded_byte_and_literal_gates() {
        let parsed = parse(r"(?i:foo)(?-i:bar)");
        let analysis = parsed.analysis();

        assert!(analysis.uniform_effective_flags().is_none());
        let start_bytes = analysis.start_bytes().expect("folded start bytes");
        for byte in [b'f', b'F', 0xc5, 0xe2] {
            assert!(start_bytes.contains(byte), "{byte:#x}");
        }
        assert!(!start_bytes.contains(b'b'));
        let prefilter = analysis.prefilter(&parsed);
        assert!(prefilter.is_enabled());
        for line in ["FOObar", "xfOobar", "fooBAR"] {
            assert!(prefilter.may_match(line, 0), "{line:?}");
        }
        assert!(!prefilter.may_match("fo obar", 0));

        // A case-sensitive literal behind a case-insensitive scope keeps a
        // (folded) gate instead of none.
        let parsed = parse(r"(?<!@)@@(?i)\b(error|rowcount)");
        let analysis = parsed.analysis();
        assert!(
            analysis
                .start_bytes()
                .is_some_and(|bytes| bytes.contains(b'@'))
        );
        assert!(!analysis.prefilter(&parsed).may_match("select 1", 0));
        assert!(analysis.prefilter(&parsed).may_match("x @@ERROR", 0));
    }
}
