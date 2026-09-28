use syntaxmate::{GrammarRegistry, Tokenizer, TokenizerOptions};

#[test]
fn cached_lines_do_not_leave_stale_regex_cursors_in_reused_input_buffers() {
    let mut registry = GrammarRegistry::new();
    let root = registry
        .add_json(
            r#"{
        "scopeName": "source.reused",
        "patterns": [{"match":"(?=z)z", "name":"keyword.z"}]
    }"#,
        )
        .unwrap();
    let mut cached = Tokenizer::new(&registry, root, TokenizerOptions::default()).unwrap();
    let mut input = String::with_capacity(8);
    for source in ["aa", "aa", "az", "az", "za", "aa"] {
        input.clear();
        input.push_str(source);
        let actual = cached
            .tokenize_line(&input, &mut cached.initial_state())
            .unwrap();
        let mut fresh = Tokenizer::new(
            &registry,
            root,
            TokenizerOptions {
                line_cache_entries: 0,
                ..TokenizerOptions::default()
            },
        )
        .unwrap();
        let expected = fresh
            .tokenize_line(source, &mut fresh.initial_state())
            .unwrap();
        assert!(actual.status().is_complete());
        assert_eq!(actual.tokens(), expected.tokens(), "{source}");
        assert_eq!(
            actual
                .tokens()
                .iter()
                .any(|token| token.scopes().any(|scope| scope == "keyword.z")),
            source.contains('z')
        );
    }
}

fn line_scopes_complete(
    source: &str,
    grammar: &str,
) -> Vec<Vec<(std::ops::Range<usize>, Vec<String>)>> {
    let mut registry = GrammarRegistry::new();
    let root = registry.add_json(grammar).expect("grammar");
    let mut tokenizer = Tokenizer::new(
        &registry,
        root,
        TokenizerOptions {
            max_line_bytes: 16 * 1024,
            line_cache_entries: 64,
        },
    )
    .expect("tokenizer");
    tokenizer
        .tokenize(source)
        .lines()
        .iter()
        .map(|line| {
            line.tokens()
                .iter()
                .map(|span| (span.range(), span.scopes().map(str::to_owned).collect()))
                .collect()
        })
        .collect()
}

fn line_scopes_incremental(
    source: &str,
    grammar: &str,
) -> Vec<Vec<(std::ops::Range<usize>, Vec<String>)>> {
    let mut registry = GrammarRegistry::new();
    let root = registry.add_json(grammar).expect("grammar");
    let mut tokenizer = Tokenizer::new(
        &registry,
        root,
        TokenizerOptions {
            max_line_bytes: 16 * 1024,
            line_cache_entries: 64,
        },
    )
    .expect("tokenizer");
    let mut state = tokenizer.initial_state();
    source
        .split('\n')
        .map(|line| {
            let tokenized = tokenizer.tokenize_line(line, &mut state).expect("line");
            tokenized
                .tokens()
                .iter()
                .map(|token| (token.range(), token.scopes().map(str::to_owned).collect()))
                .collect()
        })
        .collect()
}

#[test]
fn final_line_without_trailing_newline_can_differ_from_tokenize_line() {
    // Lookahead class includes `\s`, so the synthetic newline that
    // `tokenize_line` always appends changes the match at EOF.
    // The fuzz harness therefore skips complete↔incremental parity for the
    // final line when the source has no trailing newline.
    let grammar = r#"{
        "scopeName": "source.seed.eol",
        "patterns": [
            {
                "match": "(?<word>[[:alpha:]]+)(?=\\s)",
                "name": "meta.word"
            }
        ]
    }"#;
    let source = "nabarbaal";

    let complete = line_scopes_complete(source, grammar);
    let incremental = line_scopes_incremental(source, grammar);
    assert_ne!(
        complete, incremental,
        "complete tokenize and tokenize_line intentionally disagree without a trailing newline"
    );
}

#[test]
fn incremental_text_start_anchor_matches_complete_on_later_lines() {
    // Scheduled fuzzing found `tokenize_line` rematching `\A` on every line
    // because the public incremental API always passed line_index 0.
    let grammar = r#"{
        "scopeName": "source.seed",
        "patterns": [
            {
                "match": "\\A(let|fn)\\b",
                "name": "[port.function.seed"
            }
        ]
    }"#;
    let source = "BLOCK{\nfn  key: λ,";

    let complete = line_scopes_complete(source, grammar);
    let incremental = line_scopes_incremental(source, grammar);
    assert_eq!(complete, incremental);
    assert!(
        complete[1]
            .iter()
            .all(|(_, scopes)| scopes.iter().all(|scope| scope != "[port.function.seed")),
        "\\A must not match the second line: {complete:#?}"
    );
}
