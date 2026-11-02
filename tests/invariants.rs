//! Output invariants checked against every bundled language fixture.
//!
//! Golden tests pin exact scopes for known inputs. These checks instead hold
//! every bundled grammar to properties that must hold for any input: renderers
//! neither lose nor invent source text, tokens tile each line, incremental and
//! document APIs agree, CRLF input tokenizes like LF, and highlighting one
//! input cannot change the result for the next.
//!
//! Run with `cargo test --release --test invariants -- --nocapture` to print
//! the per-invariant pass counts.

use std::fmt::Write as _;

use syntaxmate::{
    AnsiOptions, HighlightedDocument, HighlightedLine, Highlighter, HtmlOptions, Theme, Tokenizer,
    TokenizerOptions, render_html,
};

const THEME: &str = "github-dark";

struct Case {
    language: String,
    fixture: String,
    source: String,
}

fn cases() -> Vec<Case> {
    let root = env!("CARGO_MANIFEST_DIR");
    let manifest = std::fs::read_to_string(format!("{root}/tests/fixtures/textmate/cases.toml"))
        .expect("read cases manifest");
    let manifest: toml::Table = toml::from_str(&manifest).expect("parse cases manifest");
    let catalog = syntaxmate::Catalog::bundled();
    let mut cases = Vec::new();
    for case in manifest["case"].as_array().expect("case array") {
        let language = case["language"].as_str().expect("language");
        let fixture = case["fixture"].as_str().expect("fixture");
        if catalog.language(language).is_none() {
            continue;
        }
        let source = std::fs::read_to_string(format!("{root}/{fixture}")).expect("read fixture");
        cases.push(Case {
            language: language.to_owned(),
            fixture: fixture.to_owned(),
            source,
        });
    }
    assert!(cases.len() > 400, "expected the full fixture catalog");
    cases
}

type Line = Vec<(std::ops::Range<usize>, Vec<String>)>;

fn line_tokens(line: &HighlightedLine) -> Line {
    line.tokens()
        .iter()
        .map(|token| (token.range(), token.scopes().map(str::to_owned).collect()))
        .collect()
}

fn document_tokens(document: &HighlightedDocument) -> Vec<Line> {
    document.lines().iter().map(line_tokens).collect()
}

/// Logical line texts as the document APIs define them.
fn logical_lines(source: &str) -> Vec<&str> {
    source
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect()
}

/// Recovers the text content of rendered HTML.
fn html_text(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        text.push_str(&rest[..start]);
        let end = rest[start..].find('>').expect("closed tag") + start;
        rest = &rest[end + 1..];
    }
    text.push_str(rest);
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

/// Removes SGR escape sequences from ANSI output.
fn ansi_text(ansi: &str) -> String {
    let mut text = String::with_capacity(ansi.len());
    let mut chars = ansi.chars();
    while let Some(character) = chars.next() {
        if character == '\x1b' {
            assert_eq!(chars.next(), Some('['));
            for terminator in chars.by_ref() {
                if terminator == 'm' {
                    break;
                }
            }
        } else {
            text.push(character);
        }
    }
    text
}

/// A deterministic shuffle of the fixture's own characters (xorshift64).
fn scrambled(source: &str, seed: u64) -> String {
    let mut characters = source.chars().collect::<Vec<_>>();
    let mut state = seed | 1;
    for index in (1..characters.len()).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        characters.swap(index, (state % (index as u64 + 1)) as usize);
    }
    characters.into_iter().collect()
}

type Check = fn(&Highlighter, &Case) -> Result<(), String>;

/// Tokens are non-empty, ordered, on character boundaries, and tile each line.
fn tokens_tile_lines(highlighter: &Highlighter, case: &Case) -> Result<(), String> {
    let document = highlight(highlighter, case, &case.source)?;
    let lines = logical_lines(&case.source);
    if lines.len() != document.lines().len() {
        return Err(format!(
            "{} source lines, {} document lines",
            lines.len(),
            document.lines().len()
        ));
    }
    for (index, (text, line)) in lines.iter().zip(document.lines()).enumerate() {
        let mut cursor = 0;
        for token in line.tokens() {
            let range = token.range();
            if range.start != cursor || range.end <= range.start {
                return Err(format!("line {index}: token {range:?} after {cursor}"));
            }
            if !text.is_char_boundary(range.start) || !text.is_char_boundary(range.end) {
                return Err(format!("line {index}: token {range:?} splits a character"));
            }
            cursor = range.end;
        }
        if cursor != text.len() {
            return Err(format!(
                "line {index}: tokens end at {cursor} of {}",
                text.len()
            ));
        }
    }
    Ok(())
}

/// Inline HTML, class HTML, and ANSI output contain exactly the source text.
fn renderers_preserve_text(highlighter: &Highlighter, case: &Case) -> Result<(), String> {
    let inline = highlighter
        .highlight_html(&case.language, &case.source, THEME)
        .map_err(|error| error.to_string())?;
    if html_text(inline.as_str()) != case.source {
        return Err("inline HTML text differs from source".to_owned());
    }
    let classes = HtmlOptions {
        class_prefix: Some("t".to_owned()),
        ..HtmlOptions::default()
    };
    let classes = highlighter
        .highlight_html_with_options(&case.language, &case.source, THEME, &classes)
        .map_err(|error| error.to_string())?;
    if html_text(classes.as_str()) != case.source {
        return Err("class HTML text differs from source".to_owned());
    }
    let ansi = AnsiOptions {
        sanitize_control_characters: false,
        ..AnsiOptions::default()
    };
    let ansi = highlighter
        .highlight_ansi_with_options(&case.language, &case.source, THEME, &ansi)
        .map_err(|error| error.to_string())?;
    if ansi_text(ansi.as_str()) != case.source {
        return Err("ANSI text differs from source".to_owned());
    }
    Ok(())
}

/// The single-call renderer and rendering a structured document agree.
fn html_paths_agree(highlighter: &Highlighter, case: &Case) -> Result<(), String> {
    let options = HtmlOptions {
        include_scopes: true,
        ..HtmlOptions::default()
    };
    let direct = highlighter
        .highlight_html_with_options(&case.language, &case.source, THEME, &options)
        .map_err(|error| error.to_string())?;
    let document = highlight(highlighter, case, &case.source)?;
    let rendered =
        render_html(&case.source, &document, &options).map_err(|error| error.to_string())?;
    if direct.as_str() != rendered.as_str() {
        return Err("highlight_html and render_html differ".to_owned());
    }
    Ok(())
}

/// Highlighting line by line through a session matches the whole document.
fn session_matches_document(highlighter: &Highlighter, case: &Case) -> Result<(), String> {
    let document = document_tokens(&highlight(highlighter, case, &case.source)?);
    let mut session = highlighter
        .session(&case.language, THEME)
        .map_err(|error| error.to_string())?;
    for (index, text) in logical_lines(&case.source).into_iter().enumerate() {
        let line = session
            .highlight_line(text)
            .map_err(|error| error.to_string())?;
        if document.get(index) != Some(&line_tokens(&line)) {
            return Err(format!("line {index} differs"));
        }
    }
    Ok(())
}

/// CRLF line endings tokenize exactly like LF line endings, and renderers
/// keep the original terminators.
fn crlf_matches_lf(highlighter: &Highlighter, case: &Case) -> Result<(), String> {
    let lf = case.source.replace("\r\n", "\n");
    let crlf = lf.replace('\n', "\r\n");
    let lf_tokens = document_tokens(&highlight(highlighter, case, &lf)?);
    let crlf_tokens = document_tokens(&highlight(highlighter, case, &crlf)?);
    if lf_tokens.len() != crlf_tokens.len() {
        return Err("different line counts".to_owned());
    }
    if let Some(index) = lf_tokens.iter().zip(&crlf_tokens).position(|(a, b)| a != b) {
        return Err(format!("line {index} differs"));
    }
    let html = highlighter
        .highlight_html(&case.language, &crlf, THEME)
        .map_err(|error| error.to_string())?;
    if html_text(html.as_str()) != crlf {
        return Err("CRLF HTML text differs from source".to_owned());
    }
    let ansi = AnsiOptions {
        sanitize_control_characters: false,
        ..AnsiOptions::default()
    };
    let ansi = highlighter
        .highlight_ansi_with_options(&case.language, &crlf, THEME, &ansi)
        .map_err(|error| error.to_string())?;
    if ansi_text(ansi.as_str()) != crlf {
        return Err("CRLF ANSI text differs from source".to_owned());
    }
    Ok(())
}

/// Earlier inputs to a shared highlighter cannot change later output.
fn no_state_leaks(highlighter: &Highlighter, case: &Case) -> Result<(), String> {
    let fresh = Highlighter::bundled().map_err(|error| error.to_string())?;
    let expected = document_tokens(&highlight(&fresh, case, &case.source)?);
    for seed in [0x9e37_79b9, 0x2545_f491] {
        let _ = highlight(highlighter, case, &scrambled(&case.source, seed))?;
        let actual = document_tokens(&highlight(highlighter, case, &case.source)?);
        if actual != expected {
            return Err(format!("output changed after scrambled input {seed:#x}"));
        }
    }
    Ok(())
}

/// A missing final newline changes no line's tokens.
fn final_newline_is_optional(highlighter: &Highlighter, case: &Case) -> Result<(), String> {
    let terminated = format!("{}\n", case.source.trim_end_matches('\n'));
    let unterminated = &terminated[..terminated.len() - 1];
    let with = document_tokens(&highlight(highlighter, case, &terminated)?);
    let without = document_tokens(&highlight(highlighter, case, unterminated)?);
    // The terminated document has one extra, empty final line.
    if with.len() != without.len() + 1 || with.last().is_some_and(|line| !line.is_empty()) {
        return Err("unexpected line structure".to_owned());
    }
    match with.iter().zip(&without).position(|(a, b)| a != b) {
        Some(index) => Err(format!("line {index} differs")),
        None => Ok(()),
    }
}

/// Viewports replayed from checkpoints match the same lines of a full document.
fn viewports_match_document(_: &Highlighter, case: &Case) -> Result<(), String> {
    let mut tokenizer =
        Tokenizer::for_bundled_language(&case.language, TokenizerOptions::default())
            .map_err(|error| error.to_string())?;
    let document = tokenizer.tokenize(&case.source);
    let expected = document
        .lines()
        .iter()
        .map(|line| {
            line.tokens()
                .iter()
                .map(|token| (token.range(), token.scopes().map(str::to_owned).collect()))
                .collect::<Line>()
        })
        .collect::<Vec<_>>();
    let mut checkpoints = tokenizer.checkpoints(8);
    let count = expected.len();
    // Visit later windows first so most windows replay from a checkpoint.
    let windows = [count / 2..count, 0..count.min(5), count / 3..count / 3 + 7];
    for window in windows {
        let window = window.start.min(count)..window.end.min(count);
        if window.is_empty() {
            continue;
        }
        let viewport = tokenizer
            .tokenize_viewport(&case.source, window.clone(), &mut checkpoints)
            .map_err(|error| error.to_string())?;
        for (offset, line) in viewport.lines().iter().enumerate() {
            let tokens = line
                .tokens()
                .iter()
                .map(|token| (token.range(), token.scopes().map(str::to_owned).collect()))
                .collect::<Line>();
            if tokens != expected[window.start + offset] {
                return Err(format!(
                    "viewport {window:?} line {} differs",
                    window.start + offset
                ));
            }
        }
    }
    Ok(())
}

/// Cached, ancestor-resumed style resolution equals resolving each full
/// scope list from scratch, for light and dark themes.
fn cached_styles_match_full_resolution(
    highlighter: &Highlighter,
    case: &Case,
) -> Result<(), String> {
    let document = highlighter
        .tokenize(&case.language, &case.source)
        .map_err(|error| error.to_string())?;
    for name in ["github-light", "github-dark"] {
        let theme = Theme::bundled(name).map_err(|error| error.to_string())?;
        for (index, line) in document.lines().iter().enumerate() {
            for token in line.tokens() {
                let names = token.scopes().collect::<Vec<_>>();
                if theme.resolve(token.scopes()) != theme.resolve_scope_names(&names) {
                    return Err(format!("{name} line {index} token {:?}", token.range()));
                }
            }
        }
    }
    Ok(())
}

fn highlight(
    highlighter: &Highlighter,
    case: &Case,
    source: &str,
) -> Result<HighlightedDocument, String> {
    highlighter
        .highlight(&case.language, source, THEME)
        .map_err(|error| error.to_string())
}

/// Runs one invariant over every fixture and reports all failures at once.
fn check_every_fixture(name: &str, check: Check) {
    let cases = cases();
    let highlighter = Highlighter::bundled().expect("bundled highlighter");
    let failures = cases
        .iter()
        .filter_map(|case| {
            check(&highlighter, case)
                .err()
                .map(|message| format!("{}: {message}", case.fixture))
        })
        .collect::<Vec<_>>();
    let mut report = format!(
        "{name}: {}/{} passed\n",
        cases.len() - failures.len(),
        cases.len()
    );
    for failure in failures.iter().take(5) {
        let _ = writeln!(report, "  {failure}");
    }
    println!("{report}");
    assert!(failures.is_empty(), "{report}");
}

macro_rules! invariant_tests {
    ($($name:ident),* $(,)?) => {
        $(
            #[test]
            fn $name() {
                check_every_fixture(stringify!($name), super::$name);
            }
        )*
    };
}

mod every_bundled_fixture {
    invariant_tests!(
        tokens_tile_lines,
        renderers_preserve_text,
        html_paths_agree,
        session_matches_document,
        crlf_matches_lf,
        no_state_leaks,
        final_newline_is_optional,
        viewports_match_document,
        cached_styles_match_full_resolution,
    );

    use super::check_every_fixture;
}
