use std::fs;

use crate::{
    HighlightScopeTable, Highlighter,
    theme::{RgbColor, SyntaxModifiers, github_dark_high_contrast},
};
use serde::Deserialize;

const FIXTURE: &str = "tests/fixtures/textmate/latex/hw2-theme.tex";
const SCOPE_GOLDEN: &str = "tests/fixtures/textmate/latex/hw2-theme.golden.jsonl";
const THEME_GOLDEN: &str = "tests/fixtures/textmate/latex/hw2-theme.theme.golden.jsonl";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoldenLine {
    line: String,
    tokens: Vec<GoldenToken>,
    stopped_early: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoldenToken {
    start_index: usize,
    end_index: usize,
    scopes: Vec<String>,
    #[serde(default)]
    style: Option<GoldenStyle>,
}

#[derive(Debug, Deserialize)]
struct GoldenStyle {
    foreground: Option<String>,
    background: Option<String>,
    modifiers: Vec<String>,
}

#[test]
fn latex_regression_scope_stacks_match_vscode_textmate() {
    let source = fs::read_to_string(repo_path(FIXTURE)).unwrap();
    let records = read_golden(SCOPE_GOLDEN);
    let mut highlighter = Highlighter::bundled().unwrap();
    let highlighted = highlighter.tokenize("latex", &source).unwrap();
    assert_eq!(highlighted.lines().len(), records.len());

    for (line_index, (line, record)) in highlighted.lines().iter().zip(&records).enumerate() {
        assert!(!record.stopped_early);
        let expected = record
            .tokens
            .iter()
            .filter_map(|token| {
                let start = utf16_to_utf8(&record.line, token.start_index)?;
                let utf16_len = record.line.encode_utf16().count();
                let end = utf16_to_utf8(&record.line, token.end_index.min(utf16_len))?
                    .min(record.line.len());
                (start < end).then_some((start, end, token.scopes.as_slice()))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            line.spans().len(),
            expected.len(),
            "line {} actual={:?} expected={:?}",
            line_index + 1,
            line.spans()
                .iter()
                .map(|span| (
                    span.range().start,
                    span.range().end,
                    line.scope_names(span.scope_stack()).collect::<Vec<_>>()
                ))
                .collect::<Vec<_>>(),
            expected
        );
        for (span, (start, end, scopes)) in line.spans().iter().zip(expected) {
            assert_eq!(span.range(), start..end);
            assert_eq!(
                line.scope_names(span.scope_stack()).collect::<Vec<_>>(),
                scopes.iter().map(String::as_str).collect::<Vec<_>>(),
                "line {} bytes {start}..{end}",
                line_index + 1
            );
        }
    }
}

#[test]
fn latex_regression_styles_match_vscode_textmate() {
    let theme = github_dark_high_contrast();
    for (line_index, record) in read_golden(THEME_GOLDEN).into_iter().enumerate() {
        assert!(!record.stopped_early);
        for token in record.tokens {
            let expected = token.style.expect("theme oracle style");
            let names = token.scopes.iter().map(String::as_str).collect::<Vec<_>>();
            let (table, stack) = HighlightScopeTable::from_scope_names(&names);
            let actual = theme.resolve(&table, stack);
            assert_eq!(
                actual.foreground,
                expected.foreground.as_deref().map(parse_color),
                "line {} scopes {:?}",
                line_index + 1,
                token.scopes
            );
            assert_eq!(
                actual.background,
                expected.background.as_deref().map(parse_color),
                "line {} scopes {:?}",
                line_index + 1,
                token.scopes
            );
            assert_eq!(
                modifier_names(actual.modifiers),
                expected.modifiers,
                "line {} scopes {:?}",
                line_index + 1,
                token.scopes
            );
        }
    }
}

fn read_golden(path: &str) -> Vec<GoldenLine> {
    fs::read_to_string(repo_path(path))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn repo_path(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path)
}

fn utf16_to_utf8(text: &str, utf16: usize) -> Option<usize> {
    if utf16 == 0 {
        return Some(0);
    }
    let mut units = 0;
    for (byte, character) in text.char_indices() {
        if units == utf16 {
            return Some(byte);
        }
        units += character.len_utf16();
        if units > utf16 {
            return None;
        }
    }
    (units == utf16).then_some(text.len())
}

fn parse_color(value: &str) -> RgbColor {
    let value = value.strip_prefix('#').unwrap();
    RgbColor {
        red: u8::from_str_radix(&value[0..2], 16).unwrap(),
        green: u8::from_str_radix(&value[2..4], 16).unwrap(),
        blue: u8::from_str_radix(&value[4..6], 16).unwrap(),
    }
}

fn modifier_names(modifiers: SyntaxModifiers) -> Vec<String> {
    [
        (SyntaxModifiers::ITALIC, "italic"),
        (SyntaxModifiers::BOLD, "bold"),
        (SyntaxModifiers::UNDERLINED, "underline"),
        (SyntaxModifiers::CROSSED_OUT, "strikethrough"),
    ]
    .into_iter()
    .filter(|(modifier, _)| modifiers.contains(*modifier))
    .map(|(_, name)| name.to_owned())
    .collect()
}
