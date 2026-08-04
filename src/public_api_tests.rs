use crate::{
    Catalog, Error, GrammarRegistry, HighlightStatus, Highlighter, HtmlOptions, PreparedLanguage,
    Theme, Tokenizer, TokenizerOptions, render_html, style_document,
};

#[test]
fn public_runtime_types_are_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Highlighter>();
    assert_send::<PreparedLanguage>();
    assert_send::<Tokenizer>();
    assert_send::<crate::TokenizerState>();
    assert_send::<Theme>();
}

#[test]
fn prepared_language_is_an_explicit_shared_immutable_boundary() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<PreparedLanguage>();

    let prepared = PreparedLanguage::for_bundled_language("rust").unwrap();
    let initial = prepared.stats();
    assert!(initial.grammar_count() >= 1);
    assert!(initial.compiled_pattern_count() <= initial.static_pattern_capacity());
    assert!(initial.static_pattern_retained_bytes() <= initial.static_pattern_byte_capacity());
    assert!(initial.static_candidate_count() <= initial.static_candidate_capacity());
    assert!(initial.static_candidate_retained_bytes() <= initial.static_candidate_byte_capacity());

    let mut first = prepared.tokenizer(TokenizerOptions::default());
    let mut second = Tokenizer::from_prepared(&prepared, TokenizerOptions::default());
    assert_eq!(
        first.tokenize("fn first() {}"),
        second.tokenize("fn first() {}")
    );
}

#[test]
fn every_bundled_language_fits_the_preparation_bounds() {
    for language in Catalog::bundled().languages() {
        let prepared = PreparedLanguage::for_bundled_language(&language)
            .unwrap_or_else(|error| panic!("failed to prepare {language}: {error}"));
        let stats = prepared.stats();
        assert!(stats.compiled_pattern_count() <= stats.static_pattern_capacity());
        assert!(stats.static_pattern_retained_bytes() <= stats.static_pattern_byte_capacity());
        assert!(stats.static_candidate_count() <= stats.static_candidate_capacity());
        assert!(stats.static_candidate_retained_bytes() <= stats.static_candidate_byte_capacity());
    }
}

#[test]
fn batteries_included_api_detects_tokenizes_and_styles() {
    let catalog = Catalog::bundled();
    assert_eq!(catalog.detect_path("src/main.rs").as_deref(), Some("rust"));
    assert!(catalog.languages().len() >= 264);
    assert_eq!(
        catalog.themes(),
        vec![
            "github-dark",
            "github-dark-high-contrast",
            "github-light",
            "github-light-high-contrast",
        ]
    );

    let mut highlighter = Highlighter::bundled().unwrap();
    let document = highlighter
        .highlight("rust", "fn main() {}", "github-dark")
        .unwrap();
    assert_eq!(document.status(), HighlightStatus::Complete);
    assert_eq!(document.lines().len(), 1);
    assert!(!document.lines()[0].spans().is_empty());
}

#[test]
fn custom_grammar_and_theme_work_without_product_types() {
    let mut registry = GrammarRegistry::new();
    let root = registry
        .add_json(
            r#"{
                "scopeName": "source.demo",
                "patterns": [{"match": "\\b(todo|done)\\b", "name": "keyword.demo"}]
            }"#,
        )
        .unwrap();
    let mut tokenizer = Tokenizer::new(&registry, root, TokenizerOptions::default()).unwrap();
    let document = tokenizer.tokenize("todo then done");
    assert_eq!(document.status(), HighlightStatus::Complete);
    assert!(document.lines()[0].spans().iter().any(|span| {
        document.lines()[0]
            .scope_names(span.scope_stack())
            .any(|scope| scope == "keyword.demo")
    }));

    let theme = Theme::from_json(
        r##"{
            "name": "Demo",
            "tokenColors": [{
                "scope": "keyword",
                "settings": {"foreground": "#ff0000", "fontStyle": "bold"}
            }]
        }"##,
    )
    .unwrap();
    assert_eq!(theme.name(), "Demo");

    let highlighted = style_document(document, &theme);
    assert!(highlighted.lines()[0].spans().iter().any(|span| {
        span.style()
            .foreground
            .is_some_and(|color| color.red == 255 && color.green == 0 && color.blue == 0)
    }));
    let html = render_html("todo then done", &highlighted, &HtmlOptions::default()).unwrap();
    assert!(html.as_str().contains("color:#ff0000"));
}

#[test]
fn incremental_output_matches_complete_document_scopes() {
    let source = "fn main() {\n    let value = \"text\";\n}";
    let mut highlighter = Highlighter::bundled().unwrap();
    let complete = highlighter.tokenize("rust", source).unwrap();
    let mut session = highlighter.session("rust", "github-dark").unwrap();

    for (line_index, text) in source.lines().enumerate() {
        let incremental = session.highlight_line(text).unwrap();
        assert_eq!(incremental.status(), HighlightStatus::Complete);
        let incremental_scopes = incremental
            .spans()
            .iter()
            .map(|span| {
                (
                    span.range(),
                    span.scopes().map(str::to_owned).collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        let complete_scopes = complete.lines()[line_index]
            .spans()
            .iter()
            .map(|span| {
                (
                    span.range(),
                    complete.lines()[line_index]
                        .scope_names(span.scope_stack())
                        .map(str::to_owned)
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(incremental_scopes, complete_scopes);
    }
}

#[test]
fn incremental_session_accepts_a_custom_theme() {
    let theme = Theme::from_json(
        r##"{
            "name": "Custom",
            "tokenColors": [{
                "scope": "keyword",
                "settings": {"foreground": "#112233"}
            }]
        }"##,
    )
    .unwrap();
    let highlighter = Highlighter::bundled().unwrap();
    let mut session = highlighter.session_with_theme("rust", &theme).unwrap();
    let line = session.highlight_line("fn main() {}").unwrap();
    assert_eq!(line.status(), HighlightStatus::Complete);
    assert!(line.spans().iter().any(|span| {
        span.style().foreground
            == Some(crate::RgbColor {
                red: 0x11,
                green: 0x22,
                blue: 0x33,
            })
    }));
}

#[test]
fn viewport_output_matches_complete_document_slice() {
    let source = "fn first() {}\nfn second() {\n    let value = 2;\n}\n";
    let mut full_tokenizer =
        Tokenizer::for_bundled_language("rust", TokenizerOptions::default()).unwrap();
    let full = full_tokenizer.tokenize(source);

    let mut viewport_tokenizer =
        Tokenizer::for_bundled_language("rust", TokenizerOptions::default()).unwrap();
    let mut checkpoints = viewport_tokenizer.checkpoints(2);
    let viewport = viewport_tokenizer
        .tokenize_viewport(source, 1..4, &mut checkpoints)
        .unwrap();
    assert_eq!(viewport.lines().len(), 3);

    for (actual, expected) in viewport.lines().iter().zip(&full.lines()[1..4]) {
        let actual_scopes = actual
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
        let expected_scopes = expected
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
        assert_eq!(actual_scopes, expected_scopes);
    }
}

#[test]
fn tokenizer_state_cannot_cross_tokenizer_instances() {
    let grammar = r#"{"scopeName":"source.demo","patterns":[]}"#;
    let mut registry = GrammarRegistry::new();
    let root = registry.add_json(grammar).unwrap();
    let first = Tokenizer::new(&registry, root, TokenizerOptions::default()).unwrap();
    let mut second = Tokenizer::new(&registry, root, TokenizerOptions::default()).unwrap();
    let mut state = first.initial_state();

    assert_eq!(
        second.tokenize_line("text", &mut state).unwrap_err(),
        Error::StateMismatch
    );
}
