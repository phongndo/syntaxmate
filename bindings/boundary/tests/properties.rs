//! Randomized invariants over adversarial text: mixed-width characters, `\r`,
//! empty lines, BOMs, and unbalanced delimiters across several grammars.

#![cfg(feature = "bundled-grammars")]

use syntaxmate::{Highlighter, Theme};
use syntaxmate_boundary::{
    Engine, HtmlOptions, OffsetUnit, PackedStyle, ThemeHandle, TokenBuffer, TokenOptions,
};

const LANGUAGES: &[&str] = &[
    "rust",
    "javascript",
    "python",
    "markdown",
    "html",
    "json",
    "cpp",
    "yaml",
    "shellscript",
];

const PIECES: &[&str] = &[
    "fn ", "let ", "def ", "const ", "x", "_1", "0x1F", "=", "(", ")", "{", "}", "[", "]", "<a>",
    "</a>", "\"", "'", "`", "${", "\\", "//", "/*", "*/", "#", "# ", "```", "- ", ": ", ";", ",",
    " ", "  ", "\t", "é", "ß", "日本", "☕", "😀", "𝒳", "🦀", "\u{200d}", "\u{feff}", "\r", "\n",
    "\r\n", "\n\n",
];

/// Reads a numeric override, e.g. `PROPERTY_SEED=7 PROPERTY_CASES=5000 cargo test --release`.
fn env_or(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |value| value.parse().unwrap())
}

/// xorshift64*: deterministic, dependency-free. The seed must be nonzero.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn random_source(rng: &mut Rng) -> String {
    let len = rng.below(60);
    let mut source: String = (0..len).map(|_| PIECES[rng.below(PIECES.len())]).collect();
    if rng.below(8) == 0 {
        source.insert(0, '\u{feff}');
    }
    source
}

/// Converts a UTF-8 byte offset within `text` to `unit`.
fn convert(text: &str, byte: usize, unit: OffsetUnit) -> u32 {
    let prefix = &text[..byte];
    (match unit {
        OffsetUnit::Utf8 => prefix.len(),
        OffsetUnit::Utf16 => prefix.encode_utf16().count(),
        OffsetUnit::CodePoint => prefix.chars().count(),
    }) as u32
}

fn slice(source: &str, start: u32, len: u32, unit: OffsetUnit) -> String {
    let (start, end) = (start as usize, (start + len) as usize);
    match unit {
        OffsetUnit::Utf8 => source[start..end].to_owned(),
        OffsetUnit::Utf16 => {
            String::from_utf16(&source.encode_utf16().collect::<Vec<_>>()[start..end]).unwrap()
        }
        OffsetUnit::CodePoint => source.chars().skip(start).take(end - start).collect(),
    }
}

fn line_tokens(buffer: &TokenBuffer, line: usize) -> std::ops::Range<usize> {
    buffer.line_token_ranges[line] as usize..buffer.line_token_ranges[line + 1] as usize
}

/// Checks one whole-document buffer against the raw engine and the source.
fn check_document(
    context: &str,
    source: &str,
    buffer: &TokenBuffer,
    raw: &syntaxmate::HighlightedDocument,
    theme: &Theme,
) {
    let unit = buffer.unit;
    let lines: Vec<&str> = source.split('\n').collect();
    assert_eq!(buffer.line_starts.len(), lines.len(), "{context}");
    assert_eq!(raw.lines().len(), lines.len(), "{context}");
    assert_eq!(buffer.line_token_ranges.len(), lines.len() + 1, "{context}");
    assert_eq!(buffer.line_token_ranges[0], 0, "{context}");
    let count = buffer.token_starts.len();
    assert_eq!(buffer.line_token_ranges[lines.len()] as usize, count);
    assert_eq!(buffer.token_lengths.len(), count, "{context}");
    assert_eq!(buffer.token_styles.len(), count, "{context}");
    assert!(buffer.complete, "{context}");

    let scoped = !buffer.token_scopes.is_empty() || !buffer.scope_stacks.is_empty();
    if scoped {
        assert_eq!(buffer.token_scopes.len(), count, "{context}");
    }
    assert_distinct(&buffer.styles, context);
    if scoped {
        assert_distinct(&buffer.scope_stacks, context);
    }

    let mut line_byte = 0;
    for (index, line) in lines.iter().enumerate() {
        let start = buffer.line_starts[index];
        assert_eq!(
            start,
            convert(source, line_byte, unit),
            "{context} line {index}"
        );
        let end = start + convert(line, line.len(), unit);
        let range = line_tokens(buffer, index);
        assert!(range.start <= range.end, "{context} line {index}");
        let expected = raw.lines()[index].tokens();
        assert_eq!(range.len(), expected.len(), "{context} line {index}");
        let mut previous_end = start;
        for (token, raw_token) in range.zip(expected) {
            let (token_start, len) = (buffer.token_starts[token], buffer.token_lengths[token]);
            assert!(len > 0, "{context} token {token} is empty");
            assert!(
                token_start >= previous_end,
                "{context} token {token} overlaps"
            );
            assert!(
                token_start + len <= end,
                "{context} token {token} leaves line"
            );
            previous_end = token_start + len;

            let bytes = raw_token.range();
            assert_eq!(
                slice(source, token_start, len, unit),
                line[bytes.clone()],
                "{context} token {token}"
            );
            let style = buffer.styles[buffer.token_styles[token] as usize];
            assert_eq!(
                style,
                PackedStyle::from(raw_token.style()),
                "{context} token {token}"
            );
            if scoped {
                let stack = &buffer.scope_stacks[buffer.token_scopes[token] as usize];
                let names: Vec<&str> = raw_token.scopes().collect();
                assert_eq!(stack, &names, "{context} token {token}");
                assert_eq!(style, PackedStyle::from(theme.resolve_scope_names(&names)));
            }
        }
        line_byte += line.len() + 1;
    }
}

fn assert_distinct<T: PartialEq + std::fmt::Debug>(items: &[T], context: &str) {
    for (index, item) in items.iter().enumerate() {
        assert!(
            !items[..index].contains(item),
            "{context}: duplicate entry {item:?}"
        );
    }
}

/// Checks that a session's per-line buffers describe the document's lines.
fn check_session(context: &str, document: &TokenBuffer, line: usize, buffer: &TokenBuffer) {
    assert!(buffer.complete, "{context}");
    assert_eq!(buffer.line_starts, [0], "{context}");
    assert_eq!(
        buffer.line_token_ranges,
        [0, buffer.token_starts.len() as u32],
        "{context}"
    );
    let range = line_tokens(document, line);
    let base = document.line_starts[line];
    let relative: Vec<u32> = document.token_starts[range.clone()]
        .iter()
        .map(|start| start - base)
        .collect();
    assert_eq!(buffer.token_starts, relative, "{context}");
    assert_eq!(
        buffer.token_lengths,
        document.token_lengths[range.clone()],
        "{context}"
    );
    let styles = |b: &TokenBuffer, ids: &[u32]| -> Vec<PackedStyle> {
        ids.iter().map(|id| b.styles[*id as usize]).collect()
    };
    assert_eq!(
        styles(buffer, &buffer.token_styles),
        styles(document, &document.token_styles[range.clone()]),
        "{context}"
    );
    let stacks = |b: &TokenBuffer, ids: &[u32]| -> Vec<Vec<String>> {
        ids.iter()
            .map(|id| b.scope_stacks[*id as usize].clone())
            .collect()
    };
    assert_eq!(
        stacks(buffer, &buffer.token_scopes),
        stacks(document, &document.token_scopes[range]),
        "{context}"
    );
}

#[test]
fn random_documents_keep_offset_invariants() {
    let engine = Engine::bundled().unwrap();
    let highlighter = Highlighter::bundled().unwrap();
    let handle = ThemeHandle::bundled("github-dark").unwrap();
    let theme = Theme::bundled("github-dark").unwrap();
    let mut rng = Rng(env_or("PROPERTY_SEED", 0x5eed_cafe_f00d_d00d));
    for iteration in 0..env_or("PROPERTY_CASES", 400) as usize {
        let language = LANGUAGES[iteration % LANGUAGES.len()];
        let source = random_source(&mut rng);
        let raw = highlighter
            .highlight_with_theme(language, &source, &theme)
            .unwrap();
        for unit in [OffsetUnit::Utf8, OffsetUnit::Utf16, OffsetUnit::CodePoint] {
            let include_scopes = rng.below(2) == 0;
            let options = TokenOptions {
                unit,
                include_scopes,
            };
            let context = format!("{language} {unit:?} scopes={include_scopes} {source:?}");
            let document = engine.tokens(language, &source, &handle, options).unwrap();
            check_document(&context, &source, &document, &raw, &theme);

            let full = engine
                .tokens(
                    language,
                    &source,
                    &handle,
                    TokenOptions {
                        unit,
                        include_scopes: true,
                    },
                )
                .unwrap();
            let mut session = engine
                .session(
                    language,
                    &handle,
                    TokenOptions {
                        unit,
                        include_scopes: true,
                    },
                )
                .unwrap();
            // Sessions treat every line as newline-terminated, so a document's
            // final, unterminated line may legitimately differ (e.g. a trailing
            // shell `\`); compare only the terminated lines.
            let lines: Vec<&str> = source.split('\n').collect();
            for (index, line) in lines[..lines.len() - 1].iter().enumerate() {
                let buffer = session.line(line).unwrap();
                check_session(&format!("{context} line {index}"), &full, index, &buffer);
            }
        }
    }
}

#[test]
fn session_reset_replays_identically() {
    let engine = Engine::bundled().unwrap();
    let handle = ThemeHandle::bundled("github-dark").unwrap();
    let options = TokenOptions {
        unit: OffsetUnit::Utf16,
        include_scopes: true,
    };
    let source = "/* open 😀\nstill ☕ */ let x = `a${\n1}`;\r\n";
    let mut session = engine.session("javascript", &handle, options).unwrap();
    let first: Vec<TokenBuffer> = source
        .split('\n')
        .map(|line| session.line(line).unwrap())
        .collect();
    session.reset();
    let second: Vec<TokenBuffer> = source
        .split('\n')
        .map(|line| session.line(line).unwrap())
        .collect();
    assert_eq!(first, second);
}

#[test]
fn oversized_lines_report_degraded_output() {
    let engine = Engine::bundled().unwrap();
    let handle = ThemeHandle::bundled("github-dark").unwrap();
    // Longer than the engine's default 8 KiB per-line limit.
    let long = format!("let s = \"{}\";", "é".repeat(8 * 1024));
    let source = format!("fn a() {{}}\n{long}\nfn b() {{}}\n");
    let options = TokenOptions {
        unit: OffsetUnit::Utf16,
        include_scopes: true,
    };
    let buffer = engine.tokens("rust", &source, &handle, options).unwrap();
    assert!(!buffer.complete);
    assert_eq!(buffer.line_starts.len(), 4);
    let end = buffer.token_starts.len();
    for token in 0..end {
        let start = buffer.token_starts[token];
        let text = slice(
            &source,
            start,
            buffer.token_lengths[token],
            OffsetUnit::Utf16,
        );
        assert!(!text.is_empty() && !text.contains('\n'));
    }
    let html = engine
        .html("rust", &source, &handle, &HtmlOptions::default())
        .unwrap();
    assert!(!html.complete);
    let ansi = engine
        .ansi("rust", &source, &handle, &Default::default())
        .unwrap();
    assert!(!ansi.complete);
    let mut session = engine.session("rust", &handle, options).unwrap();
    assert!(session.line("fn a() {}").unwrap().complete);
    assert!(!session.line(&long).unwrap().complete);
}
