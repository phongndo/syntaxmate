#![allow(clippy::single_range_in_vec_init)]

use std::ops::Range;

use crate::engine::tokenizer::{GrammarSet, TextMateTokenizer, TokenizedLine, TokenizerState};

fn tokenizer(grammar: &str) -> TextMateTokenizer {
    TextMateTokenizer::from_grammar(grammar).expect("distilled grammar should load")
}

fn range_of(line: &str, text: &str) -> Range<usize> {
    let start = line
        .find(text)
        .unwrap_or_else(|| panic!("{text:?} should occur in {line:?}"));
    start..start + text.len()
}

fn scope_ranges(tokenized: &TokenizedLine, scope: &str) -> Vec<Range<usize>> {
    let mut ranges: Vec<Range<usize>> = Vec::new();
    for token in tokenized
        .tokens
        .iter()
        .filter(|token| token.scopes.iter().any(|candidate| candidate == scope))
    {
        if let Some(previous) = ranges.last_mut()
            && previous.end == token.range.start
        {
            previous.end = token.range.end;
        } else {
            ranges.push(token.range.clone());
        }
    }
    ranges
}

fn assert_scope_ranges(
    line: &str,
    tokenized: &TokenizedLine,
    scope: &str,
    expected: &[Range<usize>],
) {
    assert_eq!(
        scope_ranges(tokenized, scope),
        expected,
        "unexpected ranges for {scope:?} in {line:?}; tokens: {:#?}",
        tokenized.tokens
    );
}

#[test]
fn typescript_satisfies_keeps_lookbehind_and_begin_captures_precise() {
    // Distilled from TypeScript's `#expression-operators` rule. In
    // particular, keep its fixed lookbehinds: a property named `satisfies`
    // must not be mistaken for the modern type-checking operator.
    let grammar = r##"{
        "scopeName": "source.ts.quality",
        "patterns": [{
            "begin": "(?<![$_[:alnum:]])(?:(?<=\\.\\.\\.)|(?<!\\.))(?:(as)|(satisfies))\\s+",
            "beginCaptures": {
                "1": {"name": "keyword.control.as.ts"},
                "2": {"name": "keyword.control.satisfies.ts"}
            },
            "end": "(?=;|$)",
            "contentName": "meta.type.annotation.ts",
            "patterns": [
                {"match": "(?<![$_[:alnum:]])(readonly|keyof|typeof)(?![$_[:alnum:]])", "name": "storage.modifier.type.ts"},
                {"match": "[$_[:alpha:]][$_[:alnum:]]*", "name": "support.type.ts"}
            ]
        }]
    }"##;
    let line = "obj.satisfies; const cfg = value satisfies readonly Config;";
    let mut tokenizer = tokenizer(grammar);
    let tokenized = tokenizer.tokenize_line_scopes(line, TokenizerState::default());

    let operator_start = range_of(line, "satisfies readonly").start;
    assert_scope_ranges(
        line,
        &tokenized,
        "keyword.control.satisfies.ts",
        &[operator_start..operator_start + "satisfies".len()],
    );
    assert_scope_ranges(
        line,
        &tokenized,
        "storage.modifier.type.ts",
        &[range_of(line, "readonly")],
    );
    assert_scope_ranges(
        line,
        &tokenized,
        "support.type.ts",
        &[range_of(line, "Config")],
    );
    assert!(tokenized.state.is_initial());
}

#[test]
fn python_async_def_and_debug_fstring_preserve_sparse_captures() {
    // The f-string begin/end and capture numbering are distilled from the
    // Python asset's `fstring-fnorm-quoted-single-line` repository rule.
    // The body adds the debug `=` and conversion syntax introduced in modern
    // Python while retaining the quote backreference used by the real rule.
    let grammar = r##"{
        "scopeName": "source.python.quality",
        "patterns": [
            {
                "match": "^\\s*(?:(async)\\s+)?(def)\\s+([_[:alpha:]]\\w*)(?=\\s*\\()",
                "captures": {
                    "1": {"name": "storage.type.function.async.python"},
                    "2": {"name": "storage.type.function.python"},
                    "3": {"name": "entity.name.function.python"}
                }
            },
            {
                "begin": "\\b([Ff])([BUbu])?(([\"']))",
                "beginCaptures": {
                    "1": {"name": "storage.type.string.python"},
                    "2": {"name": "invalid.illegal.prefix.python"},
                    "3": {"name": "punctuation.definition.string.begin.python"}
                },
                "end": "(\\3)|((?<!\\\\)\\n)",
                "endCaptures": {
                    "1": {"name": "punctuation.definition.string.end.python"},
                    "2": {"name": "invalid.illegal.newline.python"}
                },
                "name": "meta.fstring.python",
                "patterns": [{
                    "match": "(\\{)([_[:alpha:]]\\w*)(=)(![rsa])?(\\})",
                    "captures": {
                        "1": {"name": "punctuation.definition.interpolation.begin.python"},
                        "2": {"name": "variable.other.readwrite.python"},
                        "3": {"name": "keyword.operator.debug-conversion.python"},
                        "4": {"name": "storage.type.format-conversion.python"},
                        "5": {"name": "punctuation.definition.interpolation.end.python"}
                    }
                }]
            }
        ]
    }"##;
    let mut tokenizer = tokenizer(grammar);

    let declaration = "async def render(value):";
    let tokenized = tokenizer.tokenize_line_scopes(declaration, TokenizerState::default());
    for (scope, text) in [
        ("storage.type.function.async.python", "async"),
        ("storage.type.function.python", "def"),
        ("entity.name.function.python", "render"),
    ] {
        assert_scope_ranges(
            declaration,
            &tokenized,
            scope,
            &[range_of(declaration, text)],
        );
    }

    let expression = "return f\"{value=!r} isn't empty\" + tail";
    let tokenized = tokenizer.tokenize_line_scopes(expression, TokenizerState::default());
    for (scope, text) in [
        ("storage.type.string.python", "f"),
        ("variable.other.readwrite.python", "value"),
        ("keyword.operator.debug-conversion.python", "="),
        ("storage.type.format-conversion.python", "!r"),
    ] {
        assert_scope_ranges(expression, &tokenized, scope, &[range_of(expression, text)]);
    }
    assert_scope_ranges(
        expression,
        &tokenized,
        "punctuation.definition.string.begin.python",
        &[range_of(expression, "\"")],
    );
    let closing_quote = expression.rfind('"').expect("closing f-string quote");
    assert_scope_ranges(
        expression,
        &tokenized,
        "punctuation.definition.string.end.python",
        &[closing_quote..closing_quote + 1],
    );
    assert!(tokenized.state.is_initial());
}

#[test]
fn rust_raw_string_uses_hash_capture_for_its_dynamic_end() {
    // Distilled from Rust's `#strings` raw-string rule. A quote followed by
    // too few hashes is content, not the end delimiter.
    let grammar = r##"{
        "scopeName": "source.rust.quality",
        "patterns": [
            {
                "begin": "\\b(b?r)(#*)(\")",
                "beginCaptures": {
                    "1": {"name": "storage.type.string.raw.rust"},
                    "2": {"name": "punctuation.definition.string.raw.rust"},
                    "3": {"name": "punctuation.definition.string.rust"}
                },
                "end": "(\")(\\2)",
                "endCaptures": {
                    "1": {"name": "punctuation.definition.string.rust"},
                    "2": {"name": "punctuation.definition.string.raw.rust"}
                },
                "name": "string.quoted.double.raw.rust"
            },
            {"match": "\\b(async|move)\\b", "name": "keyword.other.rust"}
        ]
    }"##;
    let line = "let text = r###\"a premature \"## stays raw\"###; async move || {};";
    let mut tokenizer = tokenizer(grammar);
    let tokenized = tokenizer.tokenize_line_scopes(line, TokenizerState::default());

    let raw_start = line.find("r###").unwrap();
    let raw_end = line.find("\"###;").unwrap() + "\"###".len();
    assert_scope_ranges(
        line,
        &tokenized,
        "string.quoted.double.raw.rust",
        &[raw_start..raw_end],
    );
    let opening_hashes = raw_start + 1..raw_start + 4;
    let closing_hashes = raw_end - 3..raw_end;
    assert_scope_ranges(
        line,
        &tokenized,
        "punctuation.definition.string.raw.rust",
        &[opening_hashes, closing_hashes],
    );
    assert_scope_ranges(
        line,
        &tokenized,
        "keyword.other.rust",
        &[range_of(line, "async"), range_of(line, "move")],
    );
    assert!(tokenized.state.is_initial());
}

#[test]
fn cpp_recursive_template_capture_survives_possessive_spacing() {
    // Distilled from the C++ scope-resolution rules, which combine recursive
    // template subroutines, possessive repeats/spacers, and sparse captures.
    // The keyword capture mirrors the modern entries in `#misc_keywords`.
    let grammar = r##"{
        "scopeName": "source.cpp.quality",
        "patterns": [
            {
                "match": "(?<!\\w)(concept|requires|module)(?!\\w)",
                "captures": {"1": {"name": "keyword.other.$1.cpp"}}
            },
            {
                "match": "([A-Za-z_]\\w*)\\s*+((?<template><(?:[^<>]++|\\g<template>)*>)\\s*+)?(::)([A-Za-z_]\\w*)",
                "captures": {
                    "1": {"name": "entity.name.scope-resolution.cpp"},
                    "3": {"name": "meta.template.arguments.cpp"},
                    "4": {"name": "punctuation.separator.scope-resolution.cpp"},
                    "5": {"name": "entity.name.type.cpp"}
                }
            }
        ]
    }"##;
    let line = "concept Nested = requires vector<pair<int, long>>::iterator;";
    let mut tokenizer = tokenizer(grammar);
    let tokenized = tokenizer.tokenize_line_scopes(line, TokenizerState::default());

    assert_scope_ranges(
        line,
        &tokenized,
        "keyword.other.concept.cpp",
        &[range_of(line, "concept")],
    );
    assert_scope_ranges(
        line,
        &tokenized,
        "keyword.other.requires.cpp",
        &[range_of(line, "requires")],
    );
    assert_scope_ranges(
        line,
        &tokenized,
        "entity.name.scope-resolution.cpp",
        &[range_of(line, "vector")],
    );
    assert_scope_ranges(
        line,
        &tokenized,
        "meta.template.arguments.cpp",
        &[range_of(line, "<pair<int, long>>")],
    );
    assert_scope_ranges(
        line,
        &tokenized,
        "punctuation.separator.scope-resolution.cpp",
        &[range_of(line, "::")],
    );
    assert_scope_ranges(
        line,
        &tokenized,
        "entity.name.type.cpp",
        &[range_of(line, "iterator")],
    );
}

#[test]
fn markdown_fence_backreference_embeds_the_registered_typescript_grammar() {
    // Distilled from Markdown's `fenced_code_block_ts`: preserve its capture
    // numbering, language attributes, exact fence backreference, and external
    // `source.ts` include without loading the very large production grammars.
    let markdown = r##"{
        "scopeName": "text.html.markdown.quality",
        "patterns": [{
            "begin": "(^|\\G)(\\s*)(`{3,}|~{3,})\\s*(?i:(typescript|ts)((\\s+|[,:?{])[^`]*)?$)",
            "beginCaptures": {
                "3": {"name": "punctuation.definition.markdown"},
                "4": {"name": "fenced_code.block.language.markdown"},
                "5": {"name": "fenced_code.block.language.attributes.markdown"}
            },
            "end": "(^|\\G)(\\2|\\s{0,3})(\\3)\\s*$",
            "endCaptures": {
                "3": {"name": "punctuation.definition.markdown"}
            },
            "name": "markup.fenced_code.block.markdown",
            "contentName": "meta.embedded.block.typescript",
            "patterns": [{"include": "source.ts"}]
        }]
    }"##;
    let typescript = r##"{
        "scopeName": "source.ts",
        "patterns": [{
            "match": "(?<![$_[:alnum:]])(satisfies)(?![$_[:alnum:]])",
            "captures": {"1": {"name": "keyword.control.satisfies.ts"}}
        }]
    }"##;
    let mut grammars = GrammarSet::new();
    let root = grammars.load_and_add(markdown).unwrap();
    grammars.load_and_add(typescript).unwrap();
    grammars.validate_include_graph().unwrap();
    let mut tokenizer = TextMateTokenizer::new(grammars, root);

    let opening = "~~~~ts {.focused}";
    let opened = tokenizer.tokenize_line_scopes(opening, TokenizerState::default());
    assert_scope_ranges(
        opening,
        &opened,
        "punctuation.definition.markdown",
        &[range_of(opening, "~~~~")],
    );
    assert_scope_ranges(
        opening,
        &opened,
        "fenced_code.block.language.markdown",
        &[range_of(opening, "ts")],
    );
    assert_scope_ranges(
        opening,
        &opened,
        "fenced_code.block.language.attributes.markdown",
        &[range_of(opening, " {.focused}")],
    );
    assert_eq!(opened.state.depth(), 1);

    let body = "const cfg = value satisfies Shape;";
    let embedded = tokenizer.tokenize_line_scopes(body, opened.state);
    assert_scope_ranges(
        body,
        &embedded,
        "keyword.control.satisfies.ts",
        &[range_of(body, "satisfies")],
    );
    assert_scope_ranges(
        body,
        &embedded,
        "meta.embedded.block.typescript",
        &[0..body.len()],
    );

    let short_fence = "~~~";
    let still_open = tokenizer.tokenize_line_scopes(short_fence, embedded.state);
    assert_eq!(
        still_open.state.depth(),
        1,
        "a shorter fence must not close"
    );

    let closing = "~~~~";
    let closed = tokenizer.tokenize_line_scopes(closing, still_open.state);
    assert_scope_ranges(
        closing,
        &closed,
        "punctuation.definition.markdown",
        &[0..closing.len()],
    );
    assert!(closed.state.is_initial());
}
