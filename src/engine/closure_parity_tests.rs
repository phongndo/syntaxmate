use std::collections::{BTreeSet, HashMap};

use serde_json::Value;

use super::{compiled_grammar_closure, grammars};

#[test]
fn compiled_dependency_walk_matches_representative_json_contracts() {
    let bundle = grammars::embedded_bundle();
    // Embedded-heavy roots exercise broad external closures; Wikitext also
    // guards the local-repository boundary that must not expand into every
    // fenced language in the catalog.
    for language_id in ["asciidoc", "markdown", "mdx", "php", "wikitext", "yaml"] {
        let language = bundle
            .languages
            .iter()
            .find(|language| language.canonical == language_id)
            .expect("representative language is bundled");
        let actual = compiled_grammar_closure(bundle, &language.scope_name)
            .unwrap_or_else(|error| panic!("{}: {error}", language.canonical))
            .into_iter()
            .map(|grammar| grammar.scope_name)
            .collect::<Vec<_>>();
        let expected = reference_closure(bundle, &language.scope_name);
        assert_eq!(actual, expected, "{}", language.canonical);
    }
}

fn reference_closure(bundle: &grammars::bundle::Bundle, root_scope: &str) -> Vec<String> {
    let mut pending = vec![(root_scope.to_owned(), None::<String>)];
    let mut selected = BTreeSet::new();
    let mut inspected = BTreeSet::new();
    let mut decoded = HashMap::new();
    while let Some((scope, repository)) = pending.pop() {
        let Some((index, blob)) = bundle
            .grammar_blobs
            .iter()
            .enumerate()
            .find(|(_, blob)| blob.scope_name == scope)
        else {
            continue;
        };
        selected.insert(index);
        if !inspected.insert((index, repository.clone())) {
            continue;
        }
        let json = decoded.entry(index).or_insert_with(|| {
            serde_json::from_slice::<Value>(&blob.decoded_bytes().unwrap()).unwrap()
        });
        collect_external_scopes(
            json,
            &blob.scope_name,
            root_scope,
            repository.as_deref(),
            bundle,
            &mut pending,
        );
    }
    selected
        .into_iter()
        .map(|index| bundle.grammar_blobs[index].scope_name.clone())
        .collect()
}

fn collect_external_scopes(
    grammar: &Value,
    grammar_scope: &str,
    root_scope: &str,
    repository_rule: Option<&str>,
    bundle: &grammars::bundle::Bundle,
    pending: &mut Vec<(String, Option<String>)>,
) {
    let Some(object) = grammar.as_object() else {
        return;
    };
    let repository = object.get("repository").and_then(Value::as_object);
    let mut visited_local = BTreeSet::new();
    if let Some(name) = repository_rule {
        if let Some(rule) = repository.and_then(|repository| repository.get(name)) {
            collect_rule_dependencies(
                rule,
                grammar_scope,
                root_scope,
                repository,
                bundle,
                pending,
                &mut visited_local,
            );
        }
    } else {
        if let Some(patterns) = object.get("patterns") {
            collect_pattern_dependencies(
                patterns,
                grammar_scope,
                root_scope,
                repository,
                bundle,
                pending,
                &mut visited_local,
            );
        }
        if grammar_scope == root_scope
            && let Some(injections) = object.get("injections").and_then(Value::as_object)
        {
            for rule in injections.values() {
                collect_rule_dependencies(
                    rule,
                    grammar_scope,
                    root_scope,
                    repository,
                    bundle,
                    pending,
                    &mut visited_local,
                );
            }
        }
    }
}

fn collect_pattern_dependencies(
    patterns: &Value,
    grammar_scope: &str,
    root_scope: &str,
    repository: Option<&serde_json::Map<String, Value>>,
    bundle: &grammars::bundle::Bundle,
    pending: &mut Vec<(String, Option<String>)>,
    visited_local: &mut BTreeSet<String>,
) {
    let Some(patterns) = patterns.as_array() else {
        return;
    };
    for rule in patterns {
        collect_rule_dependencies(
            rule,
            grammar_scope,
            root_scope,
            repository,
            bundle,
            pending,
            visited_local,
        );
    }
}

fn collect_rule_dependencies(
    rule: &Value,
    grammar_scope: &str,
    root_scope: &str,
    repository: Option<&serde_json::Map<String, Value>>,
    bundle: &grammars::bundle::Bundle,
    pending: &mut Vec<(String, Option<String>)>,
    visited_local: &mut BTreeSet<String>,
) {
    let Some(rule) = rule.as_object() else {
        return;
    };
    if let Some(include) = rule.get("include").and_then(Value::as_str) {
        if let Some(name) = include.strip_prefix('#') {
            if visited_local.insert(name.to_owned())
                && let Some(local) = repository.and_then(|repository| repository.get(name))
            {
                collect_rule_dependencies(
                    local,
                    grammar_scope,
                    root_scope,
                    repository,
                    bundle,
                    pending,
                    visited_local,
                );
            }
        } else if include == "$self" {
            pending.push((grammar_scope.to_owned(), None));
        } else if include == "$base" {
            pending.push((root_scope.to_owned(), None));
        } else {
            let (scope, repository) = include
                .split_once('#')
                .map_or((include, None), |(scope, repository)| {
                    (scope, Some(repository.to_owned()))
                });
            if bundle.grammar_blob_for_scope(scope).is_some() {
                pending.push((scope.to_owned(), repository));
            }
        }
        return;
    }
    if let Some(patterns) = rule.get("patterns") {
        collect_pattern_dependencies(
            patterns,
            grammar_scope,
            root_scope,
            repository,
            bundle,
            pending,
            visited_local,
        );
    }
}
