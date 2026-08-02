#![no_main]

use libfuzzer_sys::fuzz_target;
use syntaxmate::{GrammarRegistry, Tokenizer, TokenizerOptions};

fuzz_target!(|data: &[u8]| {
    let Some(split) = data.iter().position(|byte| *byte == 0) else {
        return;
    };
    let Ok(grammar) = std::str::from_utf8(&data[..split]) else {
        return;
    };
    let Ok(source) = std::str::from_utf8(&data[split + 1..]) else {
        return;
    };
    let mut registry = GrammarRegistry::new();
    let Ok(root) = registry.add_json(grammar) else {
        return;
    };
    let options = TokenizerOptions {
        max_line_bytes: 16 * 1024,
        line_cache_entries: 64,
    };
    if let Ok(mut tokenizer) = Tokenizer::new(&registry, root, options) {
        let document = tokenizer.tokenize(source);
        let source_lines = source.split('\n').collect::<Vec<_>>();
        assert_eq!(document.lines().len(), source_lines.len());
        for (line, text) in document.lines().iter().zip(&source_lines) {
            for span in line.spans() {
                let range = span.range();
                assert!(range.start <= range.end);
                assert!(range.end <= text.len());
                assert!(text.is_char_boundary(range.start));
                assert!(text.is_char_boundary(range.end));
                for _ in line.scope_names(span.scope_stack()) {}
            }
        }

        let mut state = tokenizer.initial_state();
        let mut replay_state = tokenizer.initial_state();
        for (line_index, text) in source_lines.iter().enumerate() {
            let incremental = tokenizer.tokenize_line(text, &mut state).unwrap();
            let replay = tokenizer.tokenize_line(text, &mut replay_state).unwrap();
            // Degradation reports execution-budget use and can improve after
            // matcher caches warm up. Semantic tokens and continuation state
            // must still replay identically.
            assert_eq!(incremental.tokens(), replay.tokens());
            assert_eq!(state.depth(), replay_state.depth());

            let complete = &document.lines()[line_index];
            let complete_scopes = complete
                .spans()
                .iter()
                .map(|span| {
                    (
                        span.range(),
                        complete
                            .scope_names(span.scope_stack())
                            .map(str::to_owned)
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>();
            let incremental_scopes = incremental
                .tokens()
                .iter()
                .map(|token| {
                    (
                        token.range(),
                        token.scopes().map(str::to_owned).collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(complete_scopes, incremental_scopes);
        }

        if source_lines.len() > 1 {
            let selector = data.last().copied().unwrap_or(0) as usize;
            let start = selector % source_lines.len();
            let end = (start + 1 + selector / source_lines.len()).min(source_lines.len());
            let mut checkpoints = tokenizer.checkpoints(1 + selector % 8);
            let viewport = tokenizer
                .tokenize_viewport(source, start..end, &mut checkpoints)
                .unwrap();
            assert_eq!(viewport.lines().len(), end - start);
            for (actual, expected) in viewport.lines().iter().zip(&document.lines()[start..end]) {
                let actual = actual
                    .spans()
                    .iter()
                    .map(|span| {
                        (
                            span.range(),
                            actual
                                .scope_names(span.scope_stack())
                                .map(str::to_owned)
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect::<Vec<_>>();
                let expected = expected
                    .spans()
                    .iter()
                    .map(|span| {
                        (
                            span.range(),
                            expected
                                .scope_names(span.scope_stack())
                                .map(str::to_owned)
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect::<Vec<_>>();
                assert_eq!(actual, expected);
            }
        }
    }
});
