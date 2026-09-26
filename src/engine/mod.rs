//! Rust-native TextMate grammar engine.

pub mod cache;
pub mod checkpoint;
pub mod counters;
pub mod grammar;
pub(crate) mod grammar_closure;
pub(crate) mod grammar_ir;
pub(crate) mod hashing;
pub mod line;
pub mod regex;
pub mod scopes;
pub mod state;
pub mod tokenizer;

#[cfg(test)]
mod closure_parity_tests;

use crate::{Error, Result};
use tokenizer::{GrammarSet, LazyGrammar};

/// Build the bundled grammar set for `language` from its recorded closure.
///
/// Only the root is decoded here; other members decode on first access. The
/// recorded member traits let the repository-context walk skip members that
/// cannot bind a non-empty context, and recorded availability proofs answer
/// include-availability checks for members the tokenizer has not entered.
pub(crate) fn load_grammar_set(language: &str) -> Result<(GrammarSet, state::GrammarId)> {
    let bundle = crate::grammars::embedded_bundle();
    let missing = || Error::Grammar(format!("bundled TextMate grammar `{language}` is missing"));
    let root_index = bundle
        .grammar_blob_index_for_language(language)
        .ok_or_else(missing)?;
    let root_blob = &bundle.grammar_blobs[root_index];
    let mut grammars = GrammarSet::new();
    let mut root = None;
    let root_graph = bundle.grammar_graphs.get(root_index).ok_or_else(missing)?;
    for member in &root_graph.closure {
        let blob = &bundle.grammar_blobs[member.blob as usize];
        let grammar_id = grammars.add_lazy(LazyGrammar {
            blob,
            traits: member.traits,
            top_level_availability: bundle.grammar_graphs[member.blob as usize]
                .top_level_availability
                .as_deref(),
            repository_walk_skeleton: bundle.grammar_graphs[member.blob as usize]
                .repository_walk_skeleton
                .as_deref(),
        });
        if blob.scope_name == root_blob.scope_name {
            root = Some(grammar_id);
        }
    }

    // Community grammars occasionally retain optional repository includes
    // supplied only by a host editor extension. The tokenizer skips those
    // references rather than disabling the complete bundled backend.
    let root = root.ok_or_else(missing)?;
    if grammars.grammar(root).is_none() {
        let error = root_blob.compiled_grammar(root).err();
        return Err(Error::Grammar(format!(
            "failed to decode bundled TextMate grammar `{}`: {error:?}",
            root_blob.language
        )));
    }
    Ok((grammars, root))
}

/// Decode exactly the compiled external-include closure of one root.
#[cfg(test)]
fn compiled_grammar_closure(
    bundle: &crate::grammars::bundle::Bundle,
    root_scope: &str,
) -> Result<Vec<grammar::CompiledGrammar>> {
    let mut grammars = decode_bundle_grammars(bundle);
    let root = grammars
        .iter()
        .rposition(|grammar| grammar.scope_name == root_scope)
        .ok_or_else(|| Error::Grammar(format!("missing root `{root_scope}`")))?;
    let members = grammar_closure::dependency_closure(&grammars, root);
    let mut closure = Vec::with_capacity(members.len());
    for index in members.into_iter().rev() {
        closure.push(grammars.swap_remove(index));
    }
    closure.reverse();
    for (id, grammar) in closure.iter_mut().enumerate() {
        grammar.id = state::GrammarId(u16::try_from(id).expect("closure fits in GrammarId"));
    }
    Ok(closure)
}

#[cfg(test)]
fn decode_bundle_grammars(
    bundle: &crate::grammars::bundle::Bundle,
) -> Vec<grammar::CompiledGrammar> {
    bundle
        .grammar_blobs
        .iter()
        .map(|blob| {
            blob.compiled_grammar(state::GrammarId(0))
                .unwrap_or_else(|error| panic!("{}: {error:?}", blob.language))
        })
        .collect()
}
