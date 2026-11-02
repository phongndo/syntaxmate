use std::collections::HashMap;

use crate::theme::FontModifiers;
use crate::{
    Error, HtmlOptions, RenderErrorKind, RgbColor, Style, Theme, TokenizedDocument, render_html,
    render_html_themes,
};

const THEMES: [&str; 4] = [
    "github-light",
    "github-dark",
    "github-dark-high-contrast",
    "github-light-high-contrast",
];

fn names(index: usize) -> &'static str {
    ["first", "dark", "dark-hc", "light-hc"][index]
}

/// Declarations from one inline `style` attribute.
fn declarations(style: &str) -> HashMap<String, String> {
    style
        .split(';')
        .filter_map(|declaration| declaration.split_once(':'))
        .map(|(property, value)| (property.to_owned(), value.to_owned()))
        .collect()
}

fn style_attribute(tag: &str) -> &str {
    tag.split_once("style=\"").map_or("", |(_, rest)| {
        rest.split_once('"').expect("closed style").0
    })
}

fn parse_hex(value: &str) -> RgbColor {
    let value = value.strip_prefix('#').expect("hex color");
    let byte = |index: usize| u8::from_str_radix(&value[index..index + 2], 16).unwrap();
    RgbColor {
        red: byte(0),
        green: byte(2),
        blue: byte(4),
    }
}

/// Emulates CSS inheritance for flat multi-theme output: each source byte gets
/// its computed style under every theme.
fn computed_styles(html: &str, theme_count: usize, wrapper: bool) -> Vec<(char, Vec<Style>)> {
    let mut rest = html;
    let mut root = HashMap::new();
    if wrapper {
        let (tag, body) = rest.split_once("><code>").expect("wrapper");
        root = declarations(style_attribute(tag));
        rest = body.strip_suffix("</code></pre>").expect("wrapper end");
    }
    let mut span: Option<HashMap<String, String>> = None;
    let mut characters = Vec::new();
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("<span") {
            let (tag, body) = after.split_once('>').expect("span start");
            span = Some(declarations(style_attribute(tag)));
            rest = body;
            continue;
        }
        if let Some(after) = rest.strip_prefix("</span>") {
            span = None;
            rest = after;
            continue;
        }
        let (character, length) = [
            ("&lt;", '<'),
            ("&gt;", '>'),
            ("&quot;", '"'),
            ("&#39;", '\''),
            ("&amp;", '&'),
        ]
        .into_iter()
        .find(|(entity, _)| rest.starts_with(entity))
        .map_or_else(
            || {
                let character = rest.chars().next().unwrap();
                (character, character.len_utf8())
            },
            |(entity, character)| (character, entity.len()),
        );
        rest = &rest[length..];
        let lookup = |property: &str| {
            span.as_ref()
                .and_then(|span| span.get(property))
                .or_else(|| root.get(property))
                .cloned()
        };
        let mut styles = Vec::new();
        for index in 0..theme_count {
            let (color, background, weight, font_style, decoration) = if index == 0 {
                (
                    "color".to_owned(),
                    "background-color".to_owned(),
                    "font-weight".to_owned(),
                    "font-style".to_owned(),
                    "text-decoration".to_owned(),
                )
            } else {
                let name = names(index);
                (
                    format!("--sm-{name}"),
                    format!("--sm-{name}-bg"),
                    format!("--sm-{name}-fw"),
                    format!("--sm-{name}-fs"),
                    format!("--sm-{name}-td"),
                )
            };
            // The first theme's font properties live only on runs.
            let font = |property: &str| {
                if index == 0 {
                    span.as_ref().and_then(|span| span.get(property)).cloned()
                } else {
                    lookup(property)
                }
            };
            let mut modifiers = FontModifiers::empty();
            if font(&weight).as_deref() == Some("bold") {
                modifiers.insert(FontModifiers::BOLD);
            }
            if font(&font_style).as_deref() == Some("italic") {
                modifiers.insert(FontModifiers::ITALIC);
            }
            let decoration = font(&decoration).unwrap_or_default();
            if decoration.contains("underline") {
                modifiers.insert(FontModifiers::UNDERLINED);
            }
            if decoration.contains("line-through") {
                modifiers.insert(FontModifiers::CROSSED_OUT);
            }
            styles.push(Style {
                foreground: lookup(&color).map(|value| parse_hex(&value)),
                background: lookup(&background).map(|value| parse_hex(&value)),
                modifiers,
            });
        }
        characters.push((character, styles));
    }
    characters
}

/// The exact per-character style under one theme, with inherited defaults.
fn expected_styles(
    source: &str,
    document: &TokenizedDocument,
    theme: &Theme,
) -> Vec<(char, Style)> {
    let defaults = theme.default_style();
    let mut expected = Vec::new();
    let mut lines = source.split_inclusive('\n');
    for line in document.lines() {
        let text = lines.next().unwrap_or("");
        let content = text.trim_end_matches('\n').trim_end_matches('\r');
        for token in line.tokens() {
            let mut style = theme.resolve(token.scopes());
            style.foreground = style.foreground.or(defaults.foreground);
            style.background = style.background.or(defaults.background);
            for character in content[token.range()].chars() {
                expected.push((character, style));
            }
        }
        for character in text[content.len()..].chars() {
            expected.push((
                character,
                Style {
                    modifiers: FontModifiers::empty(),
                    ..defaults
                },
            ));
        }
    }
    expected
}

fn fixture(language: &str, file: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/textmate/{language}/{file}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

#[test]
fn every_theme_resolves_exactly_through_inherited_variables() {
    let themes = THEMES.map(|name| Theme::bundled(name).unwrap());
    let named = themes
        .iter()
        .enumerate()
        .map(|(index, theme)| (names(index), theme))
        .collect::<Vec<_>>();
    let highlighter = crate::Highlighter::bundled().unwrap();
    for (language, file) in [
        ("rust", "stress.rs"),
        ("tsx", "stress.tsx"),
        ("markdown", "stress.md"),
        ("css", "stress.css"),
    ] {
        let source = fixture(language, file).replace('\n', "\r\n");
        let document = highlighter.tokenize(language, &source).unwrap();
        for include_wrapper in [true, false] {
            let options = HtmlOptions {
                include_wrapper,
                ..HtmlOptions::default()
            };
            let html = render_html_themes(&source, &document, &named, &options).unwrap();
            let computed = computed_styles(html.as_str(), themes.len(), include_wrapper);
            for (index, theme) in themes.iter().enumerate() {
                let mut expected = expected_styles(&source, &document, theme);
                let mut actual = computed
                    .iter()
                    .map(|(character, styles)| (*character, styles[index]))
                    .collect::<Vec<_>>();
                if !include_wrapper {
                    // Without a wrapper, text outside runs (terminators) has no
                    // styles; the embedding container supplies them.
                    for ((character, actual), (_, expected)) in actual.iter_mut().zip(&mut expected)
                    {
                        if *character == '\n' || *character == '\r' {
                            *actual = Style::default();
                            *expected = Style::default();
                        }
                    }
                }
                assert_eq!(actual.len(), expected.len(), "{language} {index}");
                if let Some(position) = actual.iter().zip(&expected).position(|(a, b)| a != b) {
                    panic!(
                        "{language} theme {} wrapper={include_wrapper} char {position}: {:?} != {:?}",
                        THEMES[index], actual[position], expected[position]
                    );
                }
            }
        }
    }
}

#[test]
fn a_single_theme_matches_inline_html() {
    let source = "fn main() {\r\n    let x = \"<y>\";\r\n}\n";
    let highlighter = crate::Highlighter::bundled().unwrap();
    let theme = Theme::bundled("github-dark").unwrap();
    let tokens = highlighter.tokenize("rust", source).unwrap();
    let styled = highlighter
        .highlight_with_theme("rust", source, &theme)
        .unwrap();
    for options in [
        HtmlOptions::default(),
        HtmlOptions {
            include_wrapper: false,
            include_scopes: true,
            ..HtmlOptions::default()
        },
    ] {
        let single = render_html(source, &styled, &options).unwrap();
        let multi = render_html_themes(source, &tokens, &[("dark", &theme)], &options).unwrap();
        assert_eq!(multi.as_str(), single.as_str());
    }
}

#[test]
fn invalid_theme_names_and_class_mode_are_rejected() {
    let source = "x";
    let highlighter = crate::Highlighter::bundled().unwrap();
    let document = highlighter.tokenize("rust", source).unwrap();
    let theme = Theme::bundled("github-dark").unwrap();
    let options = HtmlOptions::default();
    let class_mode = HtmlOptions {
        class_prefix: Some("code".to_owned()),
        ..HtmlOptions::default()
    };
    for (themes, options) in [
        (vec![], &options),
        (vec![("", &theme)], &options),
        (vec![("a;b", &theme)], &options),
        (vec![("a", &theme), ("a", &theme)], &options),
        (
            vec![("a", &theme), ("dark", &theme), ("dark-bg", &theme)],
            &options,
        ),
        (vec![("a", &theme)], &class_mode),
    ] {
        let error = render_html_themes(source, &document, &themes, options).unwrap_err();
        assert!(
            matches!(&error, Error::Render(error) if error.kind() == RenderErrorKind::InvalidOptions),
            "{error:?}"
        );
    }
    let mismatch = render_html_themes("a\nb", &document, &[("a", &theme)], &options).unwrap_err();
    assert!(
        matches!(mismatch, Error::Render(error) if error.kind() == RenderErrorKind::SourceMismatch)
    );
}
