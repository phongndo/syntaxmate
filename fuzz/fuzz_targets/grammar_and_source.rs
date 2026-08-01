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
        for (line, text) in document.lines().iter().zip(source_lines) {
            for span in line.spans() {
                let range = span.range();
                assert!(range.start <= range.end);
                assert!(range.end <= text.len());
                assert!(text.is_char_boundary(range.start));
                assert!(text.is_char_boundary(range.end));
                for _ in line.scope_names(span.scope_stack()) {}
            }
        }
    }
});
