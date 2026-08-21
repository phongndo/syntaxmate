use syntaxmate::{GrammarRegistry, Tokenizer, TokenizerOptions};

/// Regression test for the scheduled fuzzing crash in run 32446712843: a
/// grammar pattern whose regex contains a numeric backreference to a group
/// that does not exist (`\3` with a single capture group) panicked with an
/// index-out-of-bounds while replaying captures. Input is the exact libFuzzer
/// artifact from the CI run.
#[test]
fn backreference_to_missing_group_does_not_panic() {
    let grammar = r#"{"scopeName":"source.seed.multiline","patterns":[{"begin":"(?<tag>[A-Z])+\\{","bzginCaptures":{"1":{"name":"entity.name.tag.seed"}},"end":"\\}","name":"meta.block.seed","patterns":[{"match":"(?<=:)\\3*(?<value>[^,}]+)","captures":{"1":{"name":"string.unquoted.seed"}}}]}]}"#;
    let source = "BLOCK{\n patterns\"\n  emoji: \u{1F680}\n}\n";

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

    let document = tokenizer.tokenize(source);
    assert_eq!(document.lines().len(), source.split('\n').count());
    for line in document.lines() {
        for span in line.spans() {
            assert!(span.range().start <= span.range().end);
        }
    }

    let mut state = tokenizer.initial_state();
    for text in source.split('\n') {
        tokenizer.tokenize_line(text, &mut state).expect("line");
    }
}
