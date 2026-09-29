#![cfg(feature = "bundled-grammars")]

use syntaxmate_boundary::{
    Engine, ErrorKind, HtmlOptions, NO_COLOR, OffsetUnit, ThemeHandle, TokenBuffer, TokenOptions,
};

const SOURCE: &str = "// café ☕\nconst s = \"😀 𝒳\";\r\nlet t = 1;\n";

fn tokens(unit: OffsetUnit) -> TokenBuffer {
    let engine = Engine::bundled().unwrap();
    let theme = ThemeHandle::bundled("github-dark").unwrap();
    let options = TokenOptions {
        unit,
        include_scopes: true,
    };
    engine
        .tokens("javascript", SOURCE, &theme, options)
        .unwrap()
}

fn slices(buffer: &TokenBuffer) -> Vec<String> {
    let utf16: Vec<u16> = SOURCE.encode_utf16().collect();
    let chars: Vec<char> = SOURCE.chars().collect();
    (0..buffer.token_starts.len())
        .map(|i| {
            let start = buffer.token_starts[i] as usize;
            let end = start + buffer.token_lengths[i] as usize;
            match buffer.unit {
                OffsetUnit::Utf8 => SOURCE[start..end].to_owned(),
                OffsetUnit::Utf16 => String::from_utf16(&utf16[start..end]).unwrap(),
                OffsetUnit::CodePoint => chars[start..end].iter().collect(),
            }
        })
        .collect()
}

#[test]
fn every_unit_selects_the_same_text() {
    let utf8 = tokens(OffsetUnit::Utf8);
    assert!(!utf8.token_starts.is_empty());
    for unit in [OffsetUnit::Utf16, OffsetUnit::CodePoint] {
        let other = tokens(unit);
        assert_eq!(slices(&utf8), slices(&other), "{unit:?}");
        assert_eq!(utf8.token_styles, other.token_styles);
        assert_eq!(utf8.line_token_ranges, other.line_token_ranges);
    }
}

#[test]
fn line_layout_matches_newline_split() {
    let buffer = tokens(OffsetUnit::CodePoint);
    assert_eq!(buffer.line_starts.len(), SOURCE.split('\n').count());
    assert_eq!(buffer.line_token_ranges.len(), buffer.line_starts.len() + 1);
    assert_eq!(buffer.line_starts[1], "// café ☕\n".chars().count() as u32);
    assert_eq!(buffer.token_scopes.len(), buffer.token_starts.len());
    assert!(
        buffer
            .scope_stacks
            .iter()
            .flatten()
            .any(|scope| scope.starts_with("string."))
    );
}

#[test]
fn session_matches_document_per_line() {
    let engine = Engine::bundled().unwrap();
    let theme = ThemeHandle::bundled("github-dark").unwrap();
    let options = TokenOptions {
        unit: OffsetUnit::Utf16,
        include_scopes: false,
    };
    let document = engine
        .tokens("javascript", SOURCE, &theme, options)
        .unwrap();
    let mut session = engine.session("javascript", &theme, options).unwrap();
    for (index, line) in SOURCE.split('\n').enumerate() {
        let buffer = session.line(line).unwrap();
        let range = document.line_token_ranges[index] as usize
            ..document.line_token_ranges[index + 1] as usize;
        let start = document.line_starts[index];
        let starts: Vec<u32> = document.token_starts[range.clone()]
            .iter()
            .map(|offset| offset - start)
            .collect();
        assert_eq!(buffer.token_starts, starts);
        assert_eq!(buffer.token_lengths, document.token_lengths[range.clone()]);
        let styles = |b: &TokenBuffer, ids: &[u32]| {
            ids.iter()
                .map(|id| b.styles[*id as usize])
                .collect::<Vec<_>>()
        };
        assert_eq!(
            styles(&buffer, &buffer.token_styles),
            styles(&document, &document.token_styles[range])
        );
    }
}

#[test]
fn custom_theme_styles_pack() {
    let engine = Engine::bundled().unwrap();
    let theme = ThemeHandle::from_json(
        r##"{"name":"t","colors":{"editor.foreground":"#010203"},
        "tokenColors":[{"scope":"keyword","settings":{"foreground":"#112233","fontStyle":"bold italic"}}]}"##,
    )
    .unwrap();
    let buffer = engine
        .tokens("rust", "fn x() {}", &theme, TokenOptions::default())
        .unwrap();
    assert_eq!(buffer.default_style.foreground, 0x010203);
    assert_eq!(buffer.default_style.background, NO_COLOR);
    let keyword = buffer.styles[buffer.token_styles[0] as usize];
    assert_eq!((keyword.foreground, keyword.modifiers), (0x112233, 1 | 2));
    let html = engine
        .html("rust", "fn x() {}", &theme, &HtmlOptions::default())
        .unwrap();
    assert!(html.complete && html.text.contains("#112233"));
}

#[test]
fn errors_have_stable_kinds() {
    let engine = Engine::bundled().unwrap();
    let theme = ThemeHandle::bundled("github-dark").unwrap();
    let kind = |r: Result<TokenBuffer, syntaxmate_boundary::BoundaryError>| r.unwrap_err().kind;
    assert_eq!(
        kind(engine.tokens("nope", "", &theme, TokenOptions::default())),
        ErrorKind::UnknownLanguage
    );
    assert_eq!(
        ThemeHandle::bundled("nope").unwrap_err().kind,
        ErrorKind::UnknownTheme
    );
    assert_eq!(
        ThemeHandle::from_json("{").unwrap_err().kind,
        ErrorKind::InvalidTheme
    );
    assert_eq!(
        Engine::from_bundle(b"junk").unwrap_err().kind,
        ErrorKind::InvalidBundle
    );
    let mut session = engine
        .session("rust", &theme, TokenOptions::default())
        .unwrap();
    assert_eq!(kind(session.line("a\nb")), ErrorKind::InvalidInput);
}

#[test]
fn catalog_queries() {
    let engine = Engine::bundled().unwrap();
    assert!(engine.languages().iter().any(|l| l == "rust"));
    assert!(engine.themes().iter().any(|t| t == "github-dark"));
    assert_eq!(engine.canonical_language("py").as_deref(), Some("python"));
    assert_eq!(engine.detect(Some("x.rs"), "").as_deref(), Some("rust"));
}

#[test]
fn conformance_fixtures_are_current() {
    let status = std::process::Command::new(env!("CARGO"))
        .args([
            "run",
            "-q",
            "-p",
            "syntaxmate-boundary",
            "--example",
            "conformance",
            "--",
            "--check",
        ])
        .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/.."))
        .status()
        .unwrap();
    assert!(status.success(), "run the conformance example with --write");
}

#[test]
fn offset_edges_preserve_text_scopes_and_session_continuation() {
    let engine = Engine::bundled().unwrap();
    let theme = ThemeHandle::bundled("github-dark").unwrap();
    let sources = [
        (String::new(), true),
        ("\n\r\n\n".to_owned(), true),
        (
            "// 😀 e\u{301} \0\r\nconst s = `𝒳 ${1 + 2} café`;\n\n// fin\r\n".to_owned(),
            true,
        ),
        ("/* 😀\0 e\u{301}\r\n𝒳 */ let x = '<&>';\n".to_owned(), true),
        // This overlong line degrades, then the next line recovers normally.
        (
            format!("// {}\nlet recovered = 1;\n", "😀e\u{301}".repeat(2000)),
            false,
        ),
    ];
    for (source, expected_complete) in sources {
        let mut expected = None;
        for unit in [OffsetUnit::Utf8, OffsetUnit::Utf16, OffsetUnit::CodePoint] {
            let options = TokenOptions {
                unit,
                include_scopes: true,
            };
            let document = engine
                .tokens("javascript", &source, &theme, options)
                .unwrap();
            let unpack = |text: &str, buffer: &TokenBuffer, range: std::ops::Range<usize>| {
                let utf16: Vec<u16> = text.encode_utf16().collect();
                let chars: Vec<char> = text.chars().collect();
                range
                    .map(|i| {
                        let start = buffer.token_starts[i] as usize;
                        let end = start + buffer.token_lengths[i] as usize;
                        let text = match unit {
                            OffsetUnit::Utf8 => text[start..end].to_owned(),
                            OffsetUnit::Utf16 => String::from_utf16(&utf16[start..end]).unwrap(),
                            OffsetUnit::CodePoint => chars[start..end].iter().collect(),
                        };
                        (
                            text,
                            buffer.styles[buffer.token_styles[i] as usize],
                            buffer.scope_stacks[buffer.token_scopes[i] as usize].clone(),
                        )
                    })
                    .collect::<Vec<_>>()
            };
            let resolved = unpack(&source, &document, 0..document.token_starts.len());
            if let Some(expected) = &expected {
                assert_eq!(
                    &resolved, expected,
                    "offset conversion changed output: {unit:?}"
                );
            } else {
                expected = Some(resolved);
            }
            let mut session = engine.session("javascript", &theme, options).unwrap();
            // Reset keeps caches, but must restore the same continuation output.
            for _ in 0..2 {
                session.reset();
                let mut complete = true;
                let mut start = 0;
                for (i, line) in source.split('\n').enumerate() {
                    assert_eq!(document.line_starts[i], start);
                    let actual = session.line(line).unwrap();
                    complete &= actual.complete;
                    if !expected_complete && i > 0 {
                        assert!(actual.complete, "line after oversized input must recover");
                    }
                    let range = document.line_token_ranges[i] as usize
                        ..document.line_token_ranges[i + 1] as usize;
                    assert_eq!(
                        unpack(line, &actual, 0..actual.token_starts.len()),
                        unpack(&source, &document, range),
                        "session line {i}, {unit:?}"
                    );
                    start += match unit {
                        OffsetUnit::Utf8 => line.len(),
                        OffsetUnit::Utf16 => line.encode_utf16().count(),
                        OffsetUnit::CodePoint => line.chars().count(),
                    } as u32
                        + 1;
                }
                assert_eq!(complete, document.complete);
            }
            assert_eq!(document.complete, expected_complete);
        }
    }
}
