//! Rust-native TextMate grammar engine.

pub mod cache;
pub mod checkpoint;
pub mod counters;
pub mod grammar;
pub(crate) mod grammar_ir;
pub(crate) mod hashing;
pub mod line;
pub mod regex;
pub mod scopes;
pub mod state;
pub mod tokenizer;

#[cfg(test)]
mod closure_parity_tests;

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::{Error, Result, grammars};
use grammar::{CompiledGrammar, RuleBody, RuleRef};
use tokenizer::GrammarSet;

pub(crate) fn load_grammar_set(language: &str) -> Result<(GrammarSet, state::GrammarId)> {
    let mut grammars = GrammarSet::new();
    let mut root = None;
    let bundle = crate::grammars::embedded_bundle();
    let root_blob = bundle.grammar_blob_for_language(language).ok_or_else(|| {
        Error::Grammar(format!("bundled TextMate grammar `{language}` is missing"))
    })?;
    let root_scope = root_blob.scope_name.clone();
    for grammar in compiled_grammar_closure(bundle, &root_scope)? {
        let is_root = grammar.scope_name == root_scope;
        let grammar_id = grammars.add(grammar);
        if is_root {
            root = Some(grammar_id);
        }
    }

    // Community grammars occasionally retain optional repository includes
    // supplied only by a host editor extension. The tokenizer skips those
    // references rather than disabling the complete bundled backend.
    let root = root.ok_or_else(|| {
        Error::Grammar(format!("bundled TextMate grammar `{language}` is missing"))
    })?;
    Ok((grammars, root))
}

/// Decode exactly the compiled external-include closure of one root.
///
/// `CompiledGrammar` retains the complete include graph, so dependency
/// discovery can walk it directly without parsing bundled JSON or compiling
/// grammar rules at runtime.
fn compiled_grammar_closure(
    bundle: &grammars::bundle::Bundle,
    root_scope: &str,
) -> Result<Vec<CompiledGrammar>> {
    let scope_indexes = bundle
        .grammar_blobs
        .iter()
        .enumerate()
        .map(|(index, blob)| (blob.scope_name.as_str(), index))
        .collect::<HashMap<_, _>>();
    let Some(&root_index) = scope_indexes.get(root_scope) else {
        return Ok(Vec::new());
    };
    let mut walk = DependencyWalk {
        scope_indexes: &scope_indexes,
        root_index,
        pending: vec![(root_index, None)],
    };
    let mut selected = vec![false; bundle.grammar_blobs.len()];
    let mut inspected = HashSet::new();
    let mut compiled = vec![None::<CompiledGrammar>; bundle.grammar_blobs.len()];

    while let Some((index, repository)) = walk.pending.pop() {
        selected[index] = true;
        if !inspected.insert((index, repository.clone())) {
            continue;
        }
        if compiled[index].is_none() {
            let blob = &bundle.grammar_blobs[index];
            compiled[index] = Some(blob.compiled_grammar(state::GrammarId(0)).map_err(
                |error| {
                    Error::Grammar(format!(
                        "failed to decode bundled TextMate grammar `{}`: {error:?}",
                        blob.language
                    ))
                },
            )?);
        }
        let grammar = compiled[index]
            .as_ref()
            .expect("selected grammar compiled before dependency inspection");
        walk.collect_dependencies(grammar, index, repository.as_deref());
    }

    let mut closure = Vec::new();
    for (index, is_selected) in selected.into_iter().enumerate() {
        if !is_selected {
            continue;
        }
        let mut grammar = compiled[index]
            .take()
            .expect("selected grammar compiled during dependency discovery");
        grammar.id = state::GrammarId(
            u16::try_from(closure.len()).expect("grammar closure fits in GrammarId"),
        );
        closure.push(grammar);
    }
    Ok(closure)
}

/// Pending `(bundle grammar index, repository)` inclusions for one closure.
/// Unknown external scopes are optional host-provided includes and are
/// skipped when they are discovered.
struct DependencyWalk<'a> {
    scope_indexes: &'a HashMap<&'a str, usize>,
    root_index: usize,
    pending: Vec<(usize, Option<Arc<str>>)>,
}

/// Rules and repository entries already visited while inspecting one grammar.
struct VisitedRules<'g> {
    rules: Vec<bool>,
    repositories: HashSet<&'g str>,
}

impl DependencyWalk<'_> {
    fn collect_dependencies(
        &mut self,
        grammar: &CompiledGrammar,
        grammar_index: usize,
        repository_rule: Option<&str>,
    ) {
        let mut visited = VisitedRules {
            rules: vec![false; grammar.rules.len()],
            repositories: HashSet::new(),
        };
        if let Some(name) = repository_rule {
            self.collect_repository(grammar, grammar_index, name, &mut visited);
            return;
        }

        self.collect_rule_refs(grammar, grammar_index, &grammar.top_level, &mut visited);
        // Inline injections belong only to the root. Dependencies can themselves
        // define injections, but loading those grammars as includes must not
        // activate or expand the unrelated injection rules.
        if grammar_index == self.root_index {
            for injection in &grammar.injections {
                self.collect_rule_refs(grammar, grammar_index, &injection.patterns, &mut visited);
            }
        }
    }

    fn collect_rule_refs<'g>(
        &mut self,
        grammar: &'g CompiledGrammar,
        grammar_index: usize,
        refs: &'g [RuleRef],
        visited: &mut VisitedRules<'g>,
    ) {
        for rule_ref in refs {
            self.collect_rule_ref(grammar, grammar_index, rule_ref, visited);
        }
    }

    fn collect_rule_ref<'g>(
        &mut self,
        grammar: &'g CompiledGrammar,
        grammar_index: usize,
        rule_ref: &'g RuleRef,
        visited: &mut VisitedRules<'g>,
    ) {
        match rule_ref {
            RuleRef::Rule(rule_id) => {
                let Some(seen) = visited.rules.get_mut(rule_id.0 as usize) else {
                    return;
                };
                if std::mem::replace(seen, true) {
                    return;
                }
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
                self.collect_rule_refs(grammar, grammar_index, patterns, visited);
            }
            RuleRef::Repository(name) => {
                self.collect_repository(grammar, grammar_index, name, visited);
            }
            RuleRef::SelfRef => self.pending.push((grammar_index, None)),
            RuleRef::BaseRef => self.pending.push((self.root_index, None)),
            RuleRef::External { scope, repository } => {
                if let Some(&index) = grammar
                    .scope(*scope)
                    .and_then(|scope| self.scope_indexes.get(scope))
                {
                    self.pending.push((index, repository.clone()));
                }
            }
        }
    }

    fn collect_repository<'g>(
        &mut self,
        grammar: &'g CompiledGrammar,
        grammar_index: usize,
        name: &str,
        visited: &mut VisitedRules<'g>,
    ) {
        // vscode-textmate's dependency processor walks the grammar's
        // top-level repository, but does not expand repositories declared
        // inside an include-only rule. The compiler gives those lexical
        // overlays a collision-free internal name; following them here
        // would load large unrelated closures (notably every fenced
        // language reachable from Wikitext) and change the established
        // bundled-closure contract.
        if name.starts_with("$mark.local.") {
            return;
        }
        let Some((name, rule_ref)) = grammar.repository.get_key_value(name) else {
            return;
        };
        if !visited.repositories.insert(name) {
            return;
        }
        self.collect_rule_ref(grammar, grammar_index, rule_ref, visited);
    }
}
