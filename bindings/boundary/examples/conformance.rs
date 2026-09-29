//! Writes or checks `bindings/conformance/expected.json`.

use serde_json::{Value, json};
use syntaxmate_boundary::{
    AnsiOptions, Engine, HtmlOptions, OffsetUnit, ThemeHandle, TokenBuffer, TokenOptions,
};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../conformance");

fn main() {
    let write = std::env::args().any(|arg| arg == "--write");
    let cases: Value =
        serde_json::from_str(&std::fs::read_to_string(format!("{DIR}/cases.json")).unwrap())
            .unwrap();
    let engine = Engine::bundled().unwrap();
    let custom = ThemeHandle::from_json(&cases["customTheme"].to_string()).unwrap();
    let mut out = serde_json::Map::new();
    for case in cases["cases"].as_array().unwrap() {
        let language = case["language"].as_str().unwrap();
        let source = case["source"].as_str().unwrap();
        let theme = match case["theme"].as_str() {
            Some(name) => ThemeHandle::bundled(name).unwrap(),
            None => custom.clone(),
        };
        let classes = HtmlOptions {
            class_prefix: Some("sm".to_owned()),
            ..HtmlOptions::default()
        };
        let tokens = |unit, include_scopes| {
            let options = TokenOptions {
                unit,
                include_scopes,
            };
            engine.tokens(language, source, &theme, options).unwrap()
        };
        let scoped = tokens(OffsetUnit::Utf8, true);
        let mut session = engine
            .session(
                language,
                &theme,
                TokenOptions {
                    unit: OffsetUnit::Utf16,
                    include_scopes: false,
                },
            )
            .unwrap();
        let session_lines: Vec<Value> = source
            .split('\n')
            .map(|line| buffer(&session.line(line).unwrap()))
            .collect();
        out.insert(
            case["name"].as_str().unwrap().to_owned(),
            json!({
                "html": engine.html(language, source, &theme, &HtmlOptions::default()).unwrap().text,
                "htmlClasses": engine.html(language, source, &theme, &classes).unwrap().text,
                "ansi": engine.ansi(language, source, &theme, &AnsiOptions::default()).unwrap().text,
                "tokens": {
                    "utf8": buffer(&tokens(OffsetUnit::Utf8, false)),
                    "utf16": buffer(&tokens(OffsetUnit::Utf16, false)),
                    "codePoint": buffer(&tokens(OffsetUnit::CodePoint, false)),
                },
                "scopes": { "tokenScopes": scoped.token_scopes, "scopeStacks": scoped.scope_stacks },
                "session": session_lines,
            }),
        );
    }
    let text = serde_json::to_string_pretty(&Value::Object(out)).unwrap() + "\n";
    let path = format!("{DIR}/expected.json");
    if write {
        std::fs::write(&path, text).unwrap();
    } else if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
        eprintln!("{path} is stale; rerun with --write");
        std::process::exit(1);
    }
}

fn buffer(buffer: &TokenBuffer) -> Value {
    let style =
        |s: &syntaxmate_boundary::PackedStyle| json!([s.foreground, s.background, s.modifiers]);
    json!({
        "complete": buffer.complete,
        "lineStarts": buffer.line_starts,
        "lineTokenRanges": buffer.line_token_ranges,
        "tokenStarts": buffer.token_starts,
        "tokenLengths": buffer.token_lengths,
        "tokenStyles": buffer.token_styles,
        "styles": buffer.styles.iter().map(style).collect::<Vec<_>>(),
        "defaultStyle": style(&buffer.default_style),
    })
}
