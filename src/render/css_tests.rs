//! Emulate the emitted CSS subset against the actual HTML tree, independently
//! of TextMate matching. Every text character is compared with inline resolution.
use super::*;
use crate::{Catalog, Highlighter, theme::RgbColor};

#[derive(Clone, Default)]
struct Computed {
    style: Style,
    weight: bool,
    italic: bool,
    decoration: u8,
}

struct CssRule {
    root: String,
    scopes: Vec<String>,
    declarations: Vec<(String, String)>,
}

fn parse_css(css: &str, prefix: &str) -> Vec<CssRule> {
    css.lines()
        .filter_map(|line| {
            let (selector, declarations) = line.split_once('{').unwrap();
            if selector.ends_with(" span:not([class])") {
                assert_eq!(declarations, format!("font-weight:var(--{prefix}-weight);font-style:var(--{prefix}-style);text-decoration:var(--{prefix}-decoration);}}"));
                return None;
            }
            let selector = selector.strip_prefix(":where(.").unwrap().strip_suffix(')').unwrap();
            let mut parts = selector.split(' ');
            let root = parts.next().unwrap().to_owned();
            let scopes = parts.map(|part| part.strip_prefix("[class|=\"").unwrap().strip_suffix("\"]").unwrap().to_owned()).collect();
            let declarations = declarations.strip_suffix('}').unwrap().split(';').filter(|s| !s.is_empty()).map(|s| {
                let (name, value) = s.split_once(':').unwrap();
                (name.to_owned(), value.to_owned())
            }).collect();
            Some(CssRule { root, scopes, declarations })
        })
        .collect()
}

fn attribute_matches(class: &str, value: &str) -> bool {
    class == value
        || class
            .strip_prefix(value)
            .is_some_and(|tail| tail.starts_with('-'))
}

fn rule_matches(rule: &CssRule, ancestors: &[String], class: &str) -> bool {
    if rule.scopes.is_empty() {
        return class.split_whitespace().any(|c| c == rule.root);
    }
    if !attribute_matches(class, rule.scopes.last().unwrap()) {
        return false;
    }
    let mut ancestors = ancestors.iter().rev();
    for scope in rule.scopes[..rule.scopes.len() - 1].iter().rev() {
        if !ancestors.any(|class| attribute_matches(class, scope)) {
            return false;
        }
    }
    ancestors.any(|class| class.split_whitespace().any(|c| c == rule.root))
}

fn color(value: &str) -> RgbColor {
    assert_eq!(value.len(), 7);
    assert!(value.starts_with('#'));
    RgbColor {
        red: u8::from_str_radix(&value[1..3], 16).unwrap(),
        green: u8::from_str_radix(&value[3..5], 16).unwrap(),
        blue: u8::from_str_radix(&value[5..7], 16).unwrap(),
    }
}

fn apply(computed: &mut Computed, name: &str, value: &str, prefix: &str) {
    match name {
        "color" => computed.style.foreground = Some(color(value)),
        // Transparent child backgrounds show the nearest painted ancestor.
        "background-color" => computed.style.background = Some(color(value)),
        _ if name == format!("--{prefix}-weight") => computed.weight = value == "bold",
        _ if name == format!("--{prefix}-style") => computed.italic = value == "italic",
        _ if name == format!("--{prefix}-decoration") => {
            computed.decoration = match value {
                "none" => 0,
                "underline" => 4,
                "line-through" => 8,
                "underline line-through" => 12,
                _ => panic!("unexpected decoration: {value}"),
            };
        }
        _ => panic!("unexpected declaration: {name}"),
    }
}

fn decode_text(mut text: &str) -> String {
    let mut decoded = String::new();
    while !text.is_empty() {
        if text.starts_with('&') {
            let end = text.find(';').unwrap();
            decoded.push(match &text[..=end] {
                "&amp;" => '&',
                "&lt;" => '<',
                "&gt;" => '>',
                "&quot;" => '"',
                "&#39;" => '\'',
                entity => panic!("unexpected text entity: {entity}"),
            });
            text = &text[end + 1..];
        } else {
            let c = text.chars().next().unwrap();
            decoded.push(c);
            text = &text[c.len_utf8()..];
        }
    }
    decoded
}

fn css_characters(mut html: &str, css: &str, prefix: &str) -> Vec<(char, Style)> {
    let rules = parse_css(css, prefix);
    let modifiers: [FontModifiers; 16] =
        std::array::from_fn(|bits| modifiers_from_bits(bits as u8));
    let mut classes = Vec::new();
    let mut computed = vec![Computed::default()];
    let mut characters = Vec::new();
    while !html.is_empty() {
        if html.starts_with('<') {
            let end = html.find('>').unwrap();
            let tag = &html[1..end];
            if tag.starts_with('/') {
                classes.pop().unwrap();
                computed.pop().unwrap();
            } else {
                let class = tag
                    .split_once("class=\"")
                    .map(|(_, rest)| rest.split('"').next().unwrap())
                    .unwrap_or("");
                let mut node = computed.last().unwrap().clone();
                // All scope/root selectors have zero specificity: later wins.
                for rule in &rules {
                    if rule_matches(rule, &classes, class) {
                        for (name, value) in &rule.declarations {
                            apply(&mut node, name, value, prefix);
                        }
                    }
                }
                if class.is_empty() && tag.split_whitespace().next() == Some("span") {
                    let bits =
                        u8::from(node.weight) | (u8::from(node.italic) << 1) | node.decoration;
                    node.style.modifiers = modifiers[usize::from(bits)];
                }
                classes.push(class.to_owned());
                computed.push(node);
            }
            html = &html[end + 1..];
        } else {
            let end = html.find('<').unwrap_or(html.len());
            characters.extend(
                decode_text(&html[..end])
                    .chars()
                    .map(|c| (c, computed.last().unwrap().style)),
            );
            html = &html[end..];
        }
    }
    assert!(classes.is_empty());
    characters
}

fn modifiers_from_bits(bits: u8) -> FontModifiers {
    // Resolve only a fixed fontStyle declaration, never scopes or ranking.
    let style = [
        (1, "bold"),
        (2, "italic"),
        (4, "underline"),
        (8, "strikethrough"),
    ]
    .into_iter()
    .filter(|(bit, _)| bits & bit != 0)
    .map(|(_, name)| name)
    .collect::<Vec<_>>()
    .join(" ");
    Theme::from_json(
        &serde_json::json!({"tokenColors":[{"settings":{"fontStyle":style}}]}).to_string(),
    )
    .unwrap()
    .default_style()
    .modifiers
}

fn inline_characters(source: &str, doc: &HighlightedDocument) -> Vec<(char, Style)> {
    let mut chars = Vec::new();
    for (index, (chunk, line)) in crate::engine::line::LineChunks::new(source)
        .zip(doc.lines())
        .enumerate()
    {
        if index != 0 {
            chars.push((
                '\n',
                Style {
                    modifiers: FontModifiers::empty(),
                    ..doc.default_style
                },
            ));
        }
        let mut cursor = 0;
        for token in line.tokens() {
            let range = token.range();
            chars.extend(
                chunk.text[cursor..range.start]
                    .chars()
                    .map(|c| (c, doc.default_style)),
            );
            chars.extend(
                chunk.text[range.clone()]
                    .chars()
                    .map(|c| (c, token.style())),
            );
            cursor = range.end;
        }
        chars.extend(chunk.text[cursor..].chars().map(|c| (c, doc.default_style)));
    }
    chars
}

#[test]
fn bundled_css_fidelity_and_size() {
    let options = HtmlOptions {
        class_prefix: Some("code".to_owned()),
        ..HtmlOptions::default()
    };
    let mut highlighter = Highlighter::bundled().unwrap();
    let mut previous_html = std::collections::HashMap::new();
    for theme_name in Catalog::bundled().themes() {
        let theme = Theme::bundled(theme_name).unwrap();
        let css = html_stylesheet(&theme, "code");
        let mut total = 0;
        let mut matched = 0;
        for (language, extension) in [
            ("rust", "rs"),
            ("tsx", "tsx"),
            ("html", "html"),
            ("markdown", "md"),
            ("php", "php"),
            ("css", "css"),
        ] {
            let path = format!(
                "{}/tests/fixtures/textmate/{language}/stress.{extension}",
                env!("CARGO_MANIFEST_DIR")
            );
            let source = std::fs::read_to_string(path).unwrap();
            let doc = highlighter
                .highlight_with_theme(language, &source, &theme)
                .unwrap();
            assert!(doc.status().is_complete());
            let html = render_html(&source, &doc, &options).unwrap().into_string();
            let inline = render_html(&source, &doc, &HtmlOptions::default()).unwrap();
            let expected = inline_characters(&source, &doc);
            let actual = css_characters(&html, &css, "sm-code");
            assert_eq!(
                actual.iter().map(|(c, _)| *c).collect::<String>(),
                expected.iter().map(|(c, _)| *c).collect::<String>()
            );
            let matches = actual.iter().zip(&expected).filter(|(a, b)| a == b).count();
            let fidelity = matches as f64 / expected.len() as f64;
            println!(
                "{theme_name}/{language}: {matches}/{} ({:.4}%), inline={} scope={} ratio={:.3}",
                expected.len(),
                fidelity * 100.0,
                inline.as_str().len(),
                html.len(),
                html.len() as f64 / inline.as_str().len() as f64
            );
            assert!(fidelity >= 0.99, "{theme_name}/{language}: {fidelity:.4}");
            if let Some(previous) = previous_html.insert(language, html.clone()) {
                assert_eq!(previous, html, "HTML changed with theme for {language}");
            }
            total += expected.len();
            matched += matches;
        }
        println!(
            "{theme_name}: {matched}/{total} ({:.4}%), stylesheet={} bytes",
            matched as f64 / total as f64 * 100.0,
            css.len()
        );
    }
}

fn assert_scope_style(theme: &Theme, scopes: &[&str]) -> Style {
    let options = HtmlOptions {
        class_prefix: Some("test".to_owned()),
        ..HtmlOptions::default()
    };
    let mut html = String::new();
    write_html_start(theme.default_style(), &options, Some("sm-test"), &mut html).unwrap();
    render_html_line(
        "<&λ",
        std::iter::once((0..4, Style::default(), scopes.iter().copied())),
        theme.default_style(),
        &options,
        Some("sm-test"),
        &mut html,
    )
    .unwrap();
    write_html_end(&options, &mut html).unwrap();
    let actual = css_characters(&html, &html_stylesheet(theme, "test"), "sm-test");
    let expected = theme.resolve_scope_names(scopes);
    assert_eq!(
        actual,
        vec![('<', expected), ('&', expected), ('λ', expected)],
        "{scopes:?}: {html}"
    );
    expected
}

#[test]
fn css_cascade_preserves_rank_inheritance_and_decoration_resets() {
    let theme = Theme::from_json(r##"{"tokenColors":[
        {"settings":{"foreground":"#111111","background":"#222222","fontStyle":"underline bold italic strikethrough"}},
        {"scope":"keyword.control","settings":{"foreground":"#333333"}},
        {"scope":"source.rust meta.function keyword","settings":{"foreground":"#444444"}},
        {"scope":"source.rust keyword","settings":{"background":"#555555"}},
        {"scope":"source.rust meta.function keyword","settings":{"background":"#666666"}},
        {"scope":"source.rust meta.function keyword","settings":{"background":"#777777"}},
        {"scope":"reset, punctuation","settings":{"fontStyle":""}},
        {"scope":"inner","settings":{"fontStyle":"italic","foreground":"#888888"}}
    ]}"##).unwrap();
    let style = assert_scope_style(
        &theme,
        &["source.rust", "meta.function", "keyword.control.rust"],
    );
    assert_eq!(style.foreground, Some(color("#333333")));
    assert_eq!(style.background, Some(color("#777777")));
    for reset in ["reset", "punctuation.definition"] {
        let style = assert_scope_style(
            &theme,
            &[
                "source.rust",
                "meta.function",
                "keyword.control.rust",
                reset,
            ],
        );
        assert!(style.modifiers.is_empty());
        let style = assert_scope_style(
            &theme,
            &[
                "source.rust",
                "meta.function",
                "keyword.control.rust",
                reset,
                "inner",
            ],
        );
        assert_eq!(style.modifiers, FontModifiers::ITALIC);
    }
    assert_scope_style(&theme, &[]);
    assert_scope_style(
        &theme,
        &["source.rust", "meta.function", "keyword.control", "keyword"],
    );
}

#[test]
fn scope_matching_preserves_atom_order_boundaries_and_literal_punctuation() {
    let theme = Theme::from_json(
        r##"{"tokenColors":[
        {"scope":"a.b","settings":{"foreground":"#111111"}},
        {"scope":"a-b","settings":{"foreground":"#222222"}},
        {"scope":"b.a","settings":{"foreground":"#333333"}},
        {"scope":"a_2eb","settings":{"foreground":"#444444"}},
        {"scope":"a:b","settings":{"foreground":"#555555"}},
        {"scope":"a a.b","settings":{"background":"#666666"}}
    ]}"##,
    )
    .unwrap();
    for scopes in [
        vec!["a.b"],
        vec!["a.b.c"],
        vec!["a.bc"],
        vec!["b.a"],
        vec!["a-b"],
        vec!["a_2eb"],
        vec!["a:b"],
        vec!["a", "a.b"],
        vec!["a.b", "a"],
        vec!["a", "", "a.b"],
        vec!["a.λ\"</style>"],
        vec!["a\0b"],
    ] {
        assert_scope_style(&theme, &scopes);
    }
}

#[test]
fn css_skips_unsupported_selectors_without_dropping_supported_list_members() {
    let theme = Theme::from_json(r##"{"tokenColors":[
        {"scope":"source - comment, source -comment, keyword, source > string, * , L:source, R:source","settings":{"foreground":"#112233"}}
    ]}"##).unwrap();
    let css = html_stylesheet(&theme, "");
    assert_eq!(css.matches("[class|=").count(), 1);
    assert!(css.contains("[class|=\"sm--s-keyword\"]"));
    assert!(!css.contains("comment"));
    assert!(!css.contains("string"));
    let malformed = r##"{"tokenColors":[{"scope":"x\"}</style><script>","settings":{"foreground":"#112233"}}]}"##;
    assert!(Theme::from_json(malformed).is_err());
}
