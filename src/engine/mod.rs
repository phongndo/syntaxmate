//! Rust-native TextMate grammar engine.

pub mod cache;
pub mod checkpoint;
pub mod counters;
pub mod grammar;
pub(crate) mod hashing;
pub mod line;
pub mod regex;
pub mod scopes;
pub mod state;
pub mod tokenizer;

#[cfg(test)]
mod closure_parity_tests;

use std::collections::{BTreeSet, HashMap, HashSet};

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

/// Decode, parse, and compile exactly the external-include closure of one root.
///
/// `CompiledGrammar` retains the complete include graph, so dependency
/// discovery can walk it directly. The previous path first parsed every JSON
/// blob into `serde_json::Value` for discovery and then parsed the same bytes
/// again for compilation. Embedded-heavy grammars paid that duplicate work on
/// their first visible highlight.
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
    let mut pending = vec![(root_scope.to_owned(), None::<String>)];
    let mut selected = vec![false; bundle.grammar_blobs.len()];
    let mut inspected = HashSet::new();
    let mut compiled = vec![None::<CompiledGrammar>; bundle.grammar_blobs.len()];

    while let Some((scope, repository)) = pending.pop() {
        let Some(&index) = scope_indexes.get(scope.as_str()) else {
            continue;
        };
        selected[index] = true;
        if !inspected.insert((index, repository.clone())) {
            continue;
        }
        if compiled[index].is_none() {
            let blob = &bundle.grammar_blobs[index];
            let bytes = blob.decoded_bytes().map_err(|error| {
                Error::Grammar(format!(
                    "failed to decode bundled TextMate grammar `{}`: {error:?}",
                    blob.language
                ))
            })?;
            let source = std::str::from_utf8(&bytes).map_err(|_| {
                Error::Grammar(format!(
                    "bundled TextMate grammar `{}` is not UTF-8",
                    blob.language
                ))
            })?;
            compiled[index] = Some(
                grammar::load_dev_grammar_from_str(state::GrammarId(0), source).map_err(
                    |error| {
                        Error::Grammar(format!(
                            "failed to load bundled TextMate grammar `{}`: {error}",
                            blob.language
                        ))
                    },
                )?,
            );
        }
        let grammar = compiled[index]
            .as_ref()
            .expect("selected grammar compiled before dependency inspection");
        collect_compiled_dependencies(grammar, root_scope, repository.as_deref(), &mut pending);
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

fn collect_compiled_dependencies(
    grammar: &CompiledGrammar,
    root_scope: &str,
    repository_rule: Option<&str>,
    pending: &mut Vec<(String, Option<String>)>,
) {
    let mut visited_rules = BTreeSet::new();
    let mut visited_repositories = BTreeSet::new();
    if let Some(name) = repository_rule {
        collect_compiled_rule_ref(
            grammar,
            &RuleRef::Repository(name.to_owned()),
            root_scope,
            pending,
            &mut visited_rules,
            &mut visited_repositories,
        );
        return;
    }

    collect_compiled_rule_refs(
        grammar,
        &grammar.top_level,
        root_scope,
        pending,
        &mut visited_rules,
        &mut visited_repositories,
    );
    // Inline injections belong only to the root. Dependencies can themselves
    // define injections, but loading those grammars as includes must not
    // activate or expand the unrelated injection rules.
    if grammar.scope_name == root_scope {
        for injection in &grammar.injections {
            collect_compiled_rule_refs(
                grammar,
                &injection.patterns,
                root_scope,
                pending,
                &mut visited_rules,
                &mut visited_repositories,
            );
        }
    }
}

fn collect_compiled_rule_refs(
    grammar: &CompiledGrammar,
    refs: &[RuleRef],
    root_scope: &str,
    pending: &mut Vec<(String, Option<String>)>,
    visited_rules: &mut BTreeSet<state::RuleId>,
    visited_repositories: &mut BTreeSet<String>,
) {
    for rule_ref in refs {
        collect_compiled_rule_ref(
            grammar,
            rule_ref,
            root_scope,
            pending,
            visited_rules,
            visited_repositories,
        );
    }
}

fn collect_compiled_rule_ref(
    grammar: &CompiledGrammar,
    rule_ref: &RuleRef,
    root_scope: &str,
    pending: &mut Vec<(String, Option<String>)>,
    visited_rules: &mut BTreeSet<state::RuleId>,
    visited_repositories: &mut BTreeSet<String>,
) {
    match rule_ref {
        RuleRef::Rule(rule_id) => {
            if !visited_rules.insert(*rule_id) {
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
            collect_compiled_rule_refs(
                grammar,
                patterns,
                root_scope,
                pending,
                visited_rules,
                visited_repositories,
            );
        }
        RuleRef::Repository(name) => {
            // vscode-textmate's dependency processor walks the grammar's
            // top-level repository, but does not expand repositories declared
            // inside an include-only rule. The compiler gives those lexical
            // overlays a collision-free internal name; following them here
            // would load large unrelated closures (notably every fenced
            // language reachable from Wikitext) and change the established
            // bundled-closure contract.
            if name.starts_with("$mark.local.") || !visited_repositories.insert(name.clone()) {
                return;
            }
            if let Some(rule_ref) = grammar.repository.get(name) {
                collect_compiled_rule_ref(
                    grammar,
                    rule_ref,
                    root_scope,
                    pending,
                    visited_rules,
                    visited_repositories,
                );
            }
        }
        RuleRef::SelfRef => pending.push((grammar.scope_name.clone(), None)),
        RuleRef::BaseRef => pending.push((root_scope.to_owned(), None)),
        RuleRef::External { scope, repository } => {
            if let Some(scope) = grammar.scope(*scope) {
                pending.push((scope.to_owned(), repository.clone()));
            }
        }
    }
}
