use std::{fs, ops::Range, path::Path};

use serde::Deserialize;
use serde_json::Value;

use crate::{
    GrammarRegistry, HighlightStatus, PreparedLanguage, TokenizedDocument, TokenizerOptions,
};

type ScopeLines = Vec<Vec<(Range<usize>, Vec<String>)>>;

#[derive(Deserialize)]
struct Case {
    name: String,
    grammars: Vec<Value>,
}

#[derive(Deserialize)]
struct Golden {
    name: String,
    source: String,
    lines: Vec<Vec<Span>>,
}

#[derive(Deserialize)]
struct Span {
    start: usize,
    end: usize,
    scopes: Vec<String>,
}

fn scopes(document: &TokenizedDocument) -> ScopeLines {
    document
        .lines()
        .iter()
        .map(|line| {
            line.spans()
                .iter()
                .map(|span| {
                    (
                        span.range(),
                        line.scope_names(span.scope_stack())
                            .map(str::to_owned)
                            .collect(),
                    )
                })
                .collect()
        })
        .collect()
}

#[test]
fn custom_oracle_regressions_across_sessions_sinks_and_checkpoints() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/engine-regressions");
    let cases: Vec<Case> =
        serde_json::from_str(&fs::read_to_string(directory.join("cases.json")).unwrap()).unwrap();
    let goldens = fs::read_to_string(directory.join("scopes.golden.jsonl")).unwrap();
    for golden in goldens
        .lines()
        .map(|line| serde_json::from_str::<Golden>(line).unwrap())
    {
        let case = cases.iter().find(|case| case.name == golden.name).unwrap();
        let mut registry = GrammarRegistry::new();
        let mut root = None;
        for grammar in &case.grammars {
            let id = registry.add_json(&grammar.to_string()).unwrap();
            root.get_or_insert(id);
        }
        let root = root.unwrap();
        // vscode-textmate emits one token per produce call; public spans
        // merge adjacent tokens with the same scopes.
        let expected: ScopeLines = golden
            .lines
            .into_iter()
            .map(|line| {
                let mut spans: Vec<(Range<usize>, Vec<String>)> = Vec::new();
                for span in line {
                    if let Some((range, scopes)) = spans.last_mut()
                        && range.end == span.start
                        && *scopes == span.scopes
                    {
                        range.end = span.end;
                    } else {
                        spans.push((span.start..span.end, span.scopes));
                    }
                }
                spans
            })
            .collect();
        let prepared = PreparedLanguage::new(&registry, root).unwrap();
        let mut tokenizer = prepared.tokenizer(TokenizerOptions::default());
        let mut buffered = prepared.tokenizer(TokenizerOptions::default());
        let mut callback = prepared.tokenizer(TokenizerOptions::default());
        for _ in 0..2 {
            let document = tokenizer.tokenize(&golden.source);
            assert_eq!(document.status(), HighlightStatus::Complete);
            assert_eq!(
                scopes(&document),
                expected,
                "{} {:?}",
                golden.name,
                golden.source
            );
            let mut state = tokenizer.initial_state();
            let mut buffer_state = buffered.initial_state();
            let mut callback_state = callback.initial_state();
            let mut buffer = Vec::new();
            for (index, line) in golden.source.split('\n').enumerate() {
                let owned = tokenizer.tokenize_line(line, &mut state).unwrap();
                assert_eq!(owned.status(), HighlightStatus::Complete);
                assert_eq!(
                    buffered
                        .tokenize_line_into(line, &mut buffer_state, &mut buffer)
                        .unwrap(),
                    owned.status()
                );
                let mut delivered = Vec::new();
                assert_eq!(
                    callback
                        .tokenize_line_with(line, &mut callback_state, |token| delivered
                            .push(token))
                        .unwrap(),
                    owned.status()
                );
                assert_eq!(owned.tokens(), buffer);
                assert_eq!(owned.tokens(), delivered);
                assert_eq!(
                    owned
                        .tokens()
                        .iter()
                        .map(|token| (token.range(), token.scopes().map(str::to_owned).collect()))
                        .collect::<Vec<_>>(),
                    expected[index]
                );
                assert_eq!(state.depth(), buffer_state.depth());
                assert_eq!(state.depth(), callback_state.depth());
            }
        }
        let mut checkpoints = tokenizer.checkpoints(1);
        tokenizer
            .tokenize_viewport(&golden.source, 0..expected.len(), &mut checkpoints)
            .unwrap();
        for start in 0..expected.len() {
            let replay = tokenizer
                .tokenize_viewport(&golden.source, start..expected.len(), &mut checkpoints)
                .unwrap();
            assert_eq!(replay.status(), HighlightStatus::Complete);
            assert_eq!(scopes(&replay), expected[start..]);
        }
        // An edit before every populated checkpoint must replay from the
        // invalidation boundary, not retain the old continuation stack.
        let edited = format!("plain\n{}", golden.source);
        checkpoints.invalidate_from(0);
        let replay = tokenizer
            .tokenize_viewport(&edited, 0..expected.len() + 1, &mut checkpoints)
            .unwrap();
        let fresh = prepared
            .tokenizer(TokenizerOptions::default())
            .tokenize(&edited);
        assert_eq!(replay.status(), fresh.status());
        assert_eq!(scopes(&replay), scopes(&fresh));
    }
}

#[test]
fn deep_tokenizer_states_format_and_drop_without_recursing() {
    // Each line enters one more frame, the shape nested input produces.
    let grammar = r##"{"scopeName":"source.deep","patterns":[{"include":"#r"}],"repository":{"r":{"begin":"(?=(zz))","end":"b","name":"meta.r","patterns":[{"include":"#r"}]}}}"##;
    let mut registry = GrammarRegistry::new();
    let root = registry.add_json(grammar).unwrap();
    let prepared = PreparedLanguage::new(&registry, root).unwrap();
    let mut tokenizer = prepared.tokenizer(TokenizerOptions::default());
    let mut state = tokenizer.initial_state();
    for _ in 0..10_000 {
        tokenizer.tokenize_line("zz", &mut state).unwrap();
    }
    // Recursing once per frame through 10,000 frames overflows a 64 KiB
    // stack. Every owner of the frames drops on that thread.
    std::thread::Builder::new()
        .stack_size(64 * 1024)
        .spawn(move || {
            assert!(format!("{state:?}").len() > 10_000);
            drop((state, tokenizer, prepared, registry));
        })
        .unwrap()
        .join()
        .unwrap();
}
