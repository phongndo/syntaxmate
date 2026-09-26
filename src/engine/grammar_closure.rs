//! External-include analysis over compiled grammars.
//!
//! The bundle builder runs this analysis once per root grammar and records the
//! result, so bundled tokenizers can resolve their grammar closure without
//! decoding every member. The module depends only on the compiled grammar
//! model because the builder includes it by path.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use super::grammar::{CaptureSpec, CompiledGrammar, GrammarMetadata, RuleBody, RuleRef};
use super::state::RuleId;

/// Closure-member traits recorded by the bundle builder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClosureMemberTraits {
    /// The member declares rule-local repositories, or reaches a member that
    /// does through external includes. Only these members can receive a
    /// non-empty repository context, so only these must be walked eagerly.
    pub repository_contexts: bool,
    /// The member, or a member it reaches through external includes, contains
    /// `$base`. Under a non-root base such a walk can enter another grammar's
    /// top level.
    pub base_reference: bool,
    /// The member registers standalone injections through `injectTo`.
    pub injects: bool,
}

/// One step of a top-level include-availability proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AvailabilityStep {
    Rule(RuleId),
    Repository(String),
}

/// Longest availability proof recorded for one grammar.
pub const MAX_AVAILABILITY_STEPS: usize = 32;

/// First-reference chain proving that `grammar`'s top level has an available
/// rule under any `$base`.
///
/// The tokenizer asks whether an included top level has an available rule
/// before admitting a container. When every step's first reference is a rule
/// or repository entry of this grammar and the chain ends at a match rule or
/// an empty rule list, that search succeeds on the first reference at every
/// level, independently of `$base`, and only the recorded nodes are visited.
/// Grammars with rule-local repositories are excluded because their contexts
/// can rename repository references.
pub fn top_level_availability_chain(grammar: &CompiledGrammar) -> Option<Vec<AvailabilityStep>> {
    if grammar
        .rules
        .iter()
        .any(|rule| !rule.local_repository.is_empty())
    {
        return None;
    }
    let mut steps = Vec::new();
    let mut refs: &[RuleRef] = &grammar.top_level;
    while let Some(first) = refs.first() {
        if steps.len() >= MAX_AVAILABILITY_STEPS {
            return None;
        }
        refs = match first {
            RuleRef::Rule(rule_id) => {
                let rule = grammar.rule(*rule_id)?;
                if steps.contains(&AvailabilityStep::Rule(*rule_id)) {
                    return None;
                }
                steps.push(AvailabilityStep::Rule(*rule_id));
                match &rule.body {
                    RuleBody::Match { .. } => return Some(steps),
                    RuleBody::BeginEnd { patterns, .. }
                    | RuleBody::BeginWhile { patterns, .. }
                    | RuleBody::IncludeOnly { patterns } => patterns,
                }
            }
            RuleRef::Repository(name) => {
                let target = grammar.repository.get(name)?;
                let step = AvailabilityStep::Repository(name.clone());
                if steps.contains(&step) {
                    return None;
                }
                steps.push(step);
                std::slice::from_ref(target)
            }
            RuleRef::SelfRef | RuleRef::BaseRef | RuleRef::External { .. } => return None,
        };
    }
    Some(steps)
}

/// Indexes of the grammars in `root`'s external-include closure, ascending.
///
/// This is vscode-textmate's dependency processor: it follows rule patterns
/// and repository entries but not capture-only includes or rule-local
/// repositories, and only the root contributes its inline injections.
pub fn dependency_closure(grammars: &[CompiledGrammar], root: usize) -> Vec<usize> {
    let scope_indexes = grammars
        .iter()
        .enumerate()
        .map(|(index, grammar)| (grammar.scope_name.as_str(), index))
        .collect::<HashMap<_, _>>();
    let root_scope = grammars[root].scope_name.as_str();
    let mut pending = vec![(root_scope.to_owned(), None::<String>)];
    let mut selected = vec![false; grammars.len()];
    let mut inspected = HashSet::new();
    while let Some((scope, repository)) = pending.pop() {
        let Some(&index) = scope_indexes.get(scope.as_str()) else {
            continue;
        };
        selected[index] = true;
        if !inspected.insert((index, repository.clone())) {
            continue;
        }
        collect_dependencies(
            &grammars[index],
            root_scope,
            repository.as_deref(),
            &mut pending,
        );
    }
    selected
        .iter()
        .enumerate()
        .filter_map(|(index, selected)| selected.then_some(index))
        .collect()
}

/// Traits of each closure member, in `members` order.
///
/// External edges are over-approximated with every external include in the
/// grammar, including capture patterns, so a trait is never missed.
pub fn closure_member_traits(
    grammars: &[CompiledGrammar],
    members: &[usize],
) -> Vec<ClosureMemberTraits> {
    // Later duplicates win, matching the runtime scope table.
    let positions = members
        .iter()
        .enumerate()
        .map(|(position, &index)| (grammars[index].scope_name.as_str(), position))
        .collect::<HashMap<_, _>>();
    let mut edges = Vec::with_capacity(members.len());
    let mut traits = Vec::with_capacity(members.len());
    for &index in members {
        let grammar = &grammars[index];
        let mut targets = BTreeSet::new();
        let mut base_reference = false;
        for_each_rule_ref(grammar, |rule_ref| match rule_ref {
            RuleRef::BaseRef => base_reference = true,
            RuleRef::External { scope, .. } => {
                if let Some(&position) = grammar.scope(*scope).and_then(|s| positions.get(s)) {
                    targets.insert(position);
                }
            }
            RuleRef::Rule(_) | RuleRef::Repository(_) | RuleRef::SelfRef => {}
        });
        edges.push(targets.into_iter().collect::<Vec<_>>());
        traits.push(ClosureMemberTraits {
            repository_contexts: grammar
                .rules
                .iter()
                .any(|rule| !rule.local_repository.is_empty()),
            base_reference,
            injects: !grammar.metadata.inject_to.is_empty(),
        });
    }
    loop {
        let mut changed = false;
        for position in 0..traits.len() {
            for &target in &edges[position] {
                let target_traits = traits[target];
                let current = &mut traits[position];
                if target_traits.repository_contexts && !current.repository_contexts {
                    current.repository_contexts = true;
                    changed = true;
                }
                if target_traits.base_reference && !current.base_reference {
                    current.base_reference = true;
                    changed = true;
                }
            }
        }
        if !changed {
            return traits;
        }
    }
}

/// The part of `grammar` the repository-context walk reads.
///
/// That walk follows rule, repository, capture, and external references and
/// applies rule-local repositories; it never reads regexes, scope names, or
/// metadata. The skeleton blanks those while keeping every ID valid (pattern
/// and scope tables keep their lengths, and scopes named by external includes
/// keep their text), so it encodes far smaller than the full grammar.
pub fn repository_walk_skeleton(grammar: &CompiledGrammar) -> CompiledGrammar {
    let mut external_scopes = BTreeSet::new();
    for_each_rule_ref(grammar, |rule_ref| {
        if let RuleRef::External { scope, .. } = rule_ref {
            external_scopes.insert(scope.0 as usize);
        }
    });
    let empty: Arc<str> = Arc::from("");
    let captures = |captures: &Arc<CaptureSpec>| {
        let mut captures = captures.as_ref().clone();
        captures
            .entries
            .retain(|_, entry| !entry.patterns.is_empty());
        for entry in captures.entries.values_mut() {
            entry.name = None;
        }
        Arc::new(captures)
    };
    let mut skeleton = grammar.clone();
    skeleton.metadata = GrammarMetadata::default();
    skeleton.string_names.clear();
    skeleton.patterns.iter_mut().for_each(String::clear);
    for (index, scope) in skeleton.scope_names.iter_mut().enumerate() {
        if !external_scopes.contains(&index) {
            *scope = Arc::clone(&empty);
        }
    }
    for rule in &mut skeleton.rules {
        match &mut rule.body {
            RuleBody::Match {
                captures: match_captures,
                name,
                ..
            } => {
                *name = None;
                *match_captures = captures(match_captures);
            }
            RuleBody::BeginEnd {
                begin_captures,
                end_captures,
                name,
                content_name,
                ..
            } => {
                *name = None;
                *content_name = None;
                *begin_captures = captures(begin_captures);
                *end_captures = captures(end_captures);
            }
            RuleBody::BeginWhile {
                begin_captures,
                while_captures,
                name,
                content_name,
                ..
            } => {
                *name = None;
                *content_name = None;
                *begin_captures = captures(begin_captures);
                *while_captures = captures(while_captures);
            }
            RuleBody::IncludeOnly { .. } => {}
        }
    }
    skeleton
}

pub(crate) fn for_each_rule_ref(grammar: &CompiledGrammar, mut visit: impl FnMut(&RuleRef)) {
    let mut refs = |rule_refs: &[RuleRef]| rule_refs.iter().for_each(&mut visit);
    refs(&grammar.top_level);
    for rule_ref in grammar.repository.values() {
        refs(std::slice::from_ref(rule_ref));
    }
    for injection in &grammar.injections {
        refs(&injection.patterns);
    }
    for rule in &grammar.rules {
        let (patterns, captures): (&[RuleRef], [_; 2]) = match &rule.body {
            RuleBody::Match { captures, .. } => (&[], [Some(captures), None]),
            RuleBody::BeginEnd {
                begin_captures,
                end_captures,
                patterns,
                ..
            } => (patterns, [Some(begin_captures), Some(end_captures)]),
            RuleBody::BeginWhile {
                begin_captures,
                while_captures,
                patterns,
                ..
            } => (patterns, [Some(begin_captures), Some(while_captures)]),
            RuleBody::IncludeOnly { patterns } => (patterns, [None, None]),
        };
        refs(patterns);
        for captures in captures.into_iter().flatten() {
            for entry in captures.entries.values() {
                refs(&entry.patterns);
            }
        }
    }
}

fn collect_dependencies(
    grammar: &CompiledGrammar,
    root_scope: &str,
    repository_rule: Option<&str>,
    pending: &mut Vec<(String, Option<String>)>,
) {
    let mut walk = DependencyWalk {
        grammar,
        root_scope,
        pending,
        visited_rules: BTreeSet::new(),
        visited_repositories: BTreeSet::new(),
    };
    if let Some(name) = repository_rule {
        walk.rule_ref(&RuleRef::Repository(name.to_owned()));
        return;
    }
    walk.rule_refs(&grammar.top_level);
    // Inline injections belong only to the root. Dependencies can themselves
    // define injections, but loading those grammars as includes must not
    // activate or expand the unrelated injection rules.
    if grammar.scope_name == root_scope {
        for injection in &grammar.injections {
            walk.rule_refs(&injection.patterns);
        }
    }
}

struct DependencyWalk<'a, 'p> {
    grammar: &'a CompiledGrammar,
    root_scope: &'a str,
    pending: &'p mut Vec<(String, Option<String>)>,
    visited_rules: BTreeSet<RuleId>,
    visited_repositories: BTreeSet<String>,
}

impl DependencyWalk<'_, '_> {
    fn rule_refs(&mut self, refs: &[RuleRef]) {
        for rule_ref in refs {
            self.rule_ref(rule_ref);
        }
    }

    fn rule_ref(&mut self, rule_ref: &RuleRef) {
        match rule_ref {
            RuleRef::Rule(rule_id) => {
                if !self.visited_rules.insert(*rule_id) {
                    return;
                }
                let grammar = self.grammar;
                let Some(rule) = grammar.rule(*rule_id) else {
                    return;
                };
                let patterns = match &rule.body {
                    RuleBody::BeginEnd { patterns, .. }
                    | RuleBody::BeginWhile { patterns, .. }
                    | RuleBody::IncludeOnly { patterns } => patterns,
                    // Match captures are retokenization rules. vscode-textmate's
                    // dependency processor does not follow capture-only includes.
                    RuleBody::Match { .. } => return,
                };
                self.rule_refs(patterns);
            }
            RuleRef::Repository(name) => {
                // vscode-textmate's dependency processor walks the grammar's
                // top-level repository, but does not expand repositories
                // declared inside an include-only rule. The compiler gives
                // those lexical overlays a collision-free internal name;
                // following them here would load large unrelated closures
                // (notably every fenced language reachable from Wikitext) and
                // change the established bundled-closure contract.
                if name.starts_with("$mark.local.")
                    || !self.visited_repositories.insert(name.clone())
                {
                    return;
                }
                let grammar = self.grammar;
                if let Some(rule_ref) = grammar.repository.get(name) {
                    self.rule_ref(rule_ref);
                }
            }
            RuleRef::SelfRef => self.pending.push((self.grammar.scope_name.clone(), None)),
            RuleRef::BaseRef => self.pending.push((self.root_scope.to_owned(), None)),
            RuleRef::External { scope, repository } => {
                if let Some(scope) = self.grammar.scope(*scope) {
                    self.pending.push((scope.to_owned(), repository.clone()));
                }
            }
        }
    }
}
