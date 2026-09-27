use std::error::Error as _;

use crate::{
    Error, GrammarErrorKind, GrammarLimits, GrammarRegistry, GrammarResource, PreparedLanguage,
    Theme, ThemeErrorKind, Tokenizer, TokenizerOptions,
};

fn grammar_error(error: Error) -> crate::GrammarError {
    assert_eq!(error, error.clone());
    let Error::Grammar(error) = error else {
        panic!("expected grammar error: {error}");
    };
    error
}

fn has_source<T: std::error::Error + 'static>(error: &dyn std::error::Error) -> bool {
    let mut source = error.source();
    while let Some(error) = source {
        if error.is::<T>() {
            return true;
        }
        source = error.source();
    }
    false
}

#[test]
fn json_positions_and_original_causes_survive_cloning() {
    let json = "{\n  \"scopeName\": }";
    let expected = serde_json::from_str::<serde_json::Value>(json).unwrap_err();
    let error = GrammarRegistry::new().add_json(json).unwrap_err();
    assert!(has_source::<serde_json::Error>(&error.clone()));
    let error = grammar_error(error);
    assert_eq!(error.scope_name(), None);
    let GrammarErrorKind::InvalidJson(json) = error.kind() else {
        panic!("{error}");
    };
    assert_eq!(
        (json.line(), json.column()),
        (expected.line(), expected.column())
    );

    let error = Theme::from_json("{\n\"name\": }").unwrap_err();
    assert_eq!(error, error.clone());
    assert!(has_source::<serde_json::Error>(&error));
    let Error::Theme(error) = error else { panic!() };
    let ThemeErrorKind::InvalidJson(json) = error.kind() else {
        panic!()
    };
    assert_eq!((json.line(), json.column()), (2, 9));

    let first = GrammarRegistry::new().add_json("{").unwrap_err();
    let same = GrammarRegistry::new().add_json("{").unwrap_err();
    let other = GrammarRegistry::new().add_json("[").unwrap_err();
    assert_eq!(first, same);
    assert_ne!(first, other);
}

#[test]
fn missing_includes_report_origin_scope_and_target() {
    for (include, target_scope, key, add_target) in [
        ("#missing", None, Some("missing"), false),
        ("source.other", Some("source.other"), None, false),
        (
            "source.other#entry",
            Some("source.other"),
            Some("entry"),
            false,
        ),
        (
            "source.other#entry",
            Some("source.other"),
            Some("entry"),
            true,
        ),
    ] {
        let mut registry = GrammarRegistry::new();
        registry
            .add_json(
                &serde_json::json!({
                    "scopeName": "source.origin", "patterns": [{"include": include}]
                })
                .to_string(),
            )
            .unwrap();
        if add_target {
            registry
                .add_json(r#"{"scopeName":"source.other","patterns":[]}"#)
                .unwrap();
        }
        let error = grammar_error(registry.validate().unwrap_err());
        assert_eq!(error.scope_name(), Some("source.origin"));
        let GrammarErrorKind::MissingInclude(target) = error.kind() else {
            panic!("{error}")
        };
        assert_eq!(target.scope_name(), target_scope);
        assert_eq!(target.repository_key(), key);
    }
}

#[test]
fn regex_validation_is_explicit_and_positions_count_scalars() {
    for (pattern, position) in [("é)", 1), ("(日本", 3)] {
        let mut registry = GrammarRegistry::new();
        let root = registry
            .add_json(
                &serde_json::json!({
                    "scopeName": "source.regex", "patterns": [{"match": pattern}]
                })
                .to_string(),
            )
            .unwrap();
        registry.validate().unwrap();
        Tokenizer::new(&registry, root, TokenizerOptions::default()).unwrap();
        let error = grammar_error(registry.validate_regexes().unwrap_err());
        assert_eq!(error.scope_name(), Some("source.regex"));
        let GrammarErrorKind::InvalidRegex(regex) = error.kind() else {
            panic!("{error}")
        };
        assert_eq!(regex.pattern(), pattern);
        assert_eq!(regex.position(), position);
        assert!(!regex.message().is_empty());
    }
    let mut registry = GrammarRegistry::new();
    registry
        .add_json(r#"{"scopeName":"source.valid","patterns":[{"match":"(?<=a)b"}]}"#)
        .unwrap();
    registry.validate_regexes().unwrap();
}

#[test]
fn grammar_limits_report_configured_and_attempted_values() {
    let mut registry = GrammarRegistry::with_limits(GrammarLimits {
        max_grammar_bytes: 4,
        max_grammars: 1,
    });
    let error = grammar_error(registry.add_json("12345").unwrap_err());
    let GrammarErrorKind::LimitExceeded(limit) = error.kind() else {
        panic!()
    };
    assert_eq!(limit.resource(), GrammarResource::GrammarBytes);
    assert_eq!((limit.limit(), limit.actual()), (4, 5));

    let mut registry = GrammarRegistry::with_limits(GrammarLimits {
        max_grammar_bytes: 1024,
        max_grammars: 1,
    });
    let grammar = r#"{"scopeName":"source.test","patterns":[]}"#;
    registry.add_json(grammar).unwrap();
    let error = grammar_error(registry.add_json(grammar).unwrap_err());
    let GrammarErrorKind::LimitExceeded(limit) = error.kind() else {
        panic!()
    };
    assert_eq!(limit.resource(), GrammarResource::GrammarCount);
    assert_eq!((limit.limit(), limit.actual()), (1, 2));
    assert_eq!(registry.grammar_count(), 1);
}

#[test]
fn foreign_grammar_ids_are_matchable() {
    let mut first = GrammarRegistry::new();
    let root = first
        .add_json(r#"{"scopeName":"source.test","patterns":[]}"#)
        .unwrap();
    let second = GrammarRegistry::new();
    for error in [
        Tokenizer::new(&second, root, TokenizerOptions::default()).unwrap_err(),
        PreparedLanguage::new(&second, root).unwrap_err(),
    ] {
        assert_eq!(
            grammar_error(error).kind(),
            &GrammarErrorKind::ForeignGrammarId
        );
    }
}

#[test]
fn invalid_theme_colors_preserve_untrusted_values() {
    for value in [
        "#ééé",
        "#ggg",
        "red",
        "\"><script>alert(1)</script>",
        "#fff0",
    ] {
        let json = serde_json::json!({"colors": {"editor.background": value}}).to_string();
        let error = Theme::from_json(&json).unwrap_err();
        let Error::Theme(error) = error else { panic!() };
        assert_eq!(
            error.kind(),
            &ThemeErrorKind::InvalidColor(value.to_owned())
        );
        assert!(error.source().is_none());
    }
    let error =
        Theme::from_json(r#"{"tokenColors":[{"settings":{"fontStyle":"blink"}}]}"#).unwrap_err();
    let Error::Theme(error) = error else { panic!() };
    assert_eq!(error.kind(), &ThemeErrorKind::InvalidRule);
}

#[cfg(feature = "html")]
#[test]
fn render_validation_precedes_writing_and_writer_cause_is_preserved() {
    use crate::{HtmlOptions, RenderErrorKind, render_html_to, style_document};
    let mut registry = GrammarRegistry::new();
    let root = registry
        .add_json(r#"{"scopeName":"source.test","patterns":[]}"#)
        .unwrap();
    let mut tokenizer = Tokenizer::new(&registry, root, TokenizerOptions::default()).unwrap();
    let theme = Theme::from_json("{}").unwrap();
    let document = style_document(tokenizer.tokenize("abc"), &theme);
    let mut output = String::new();
    for source in ["abc\ndef", "é"] {
        let error =
            render_html_to(source, &document, &HtmlOptions::default(), &mut output).unwrap_err();
        let Error::Render(error) = error else {
            panic!()
        };
        assert_eq!(error.kind(), RenderErrorKind::SourceMismatch);
        assert!(error.source().is_none());
        assert!(output.is_empty());
    }
    struct Fails;
    impl std::fmt::Write for Fails {
        fn write_str(&mut self, _: &str) -> std::fmt::Result {
            Err(std::fmt::Error)
        }
    }
    let error = render_html_to("abc", &document, &HtmlOptions::default(), &mut Fails).unwrap_err();
    assert!(has_source::<std::fmt::Error>(&error));
    let Error::Render(error) = error else {
        panic!()
    };
    assert_eq!(error.kind(), RenderErrorKind::Writer);
}

#[cfg(feature = "diagnostics")]
#[test]
fn diagnostic_causes_are_matchable() {
    use crate::{
        DiagnosticErrorKind,
        diagnostics::{RegexAnchorContext, RegexEngine, match_regex},
    };
    for (engine, pattern, from, budget, kind) in [
        (
            RegexEngine::Fallback,
            "a",
            99,
            100,
            DiagnosticErrorKind::InvalidStart,
        ),
        (
            RegexEngine::Fallback,
            "(a+)+b",
            0,
            0,
            DiagnosticErrorKind::BudgetExceeded,
        ),
    ] {
        let error = match_regex(
            pattern,
            "aaaa",
            from,
            RegexAnchorContext::default(),
            engine,
            budget,
        )
        .unwrap_err();
        let Error::Diagnostic(error) = error else {
            panic!()
        };
        assert_eq!(error.kind(), kind);
        assert_eq!(error.pattern(), pattern);
        match kind {
            DiagnosticErrorKind::InvalidStart => assert_eq!(error.position(), Some(from)),
            DiagnosticErrorKind::BudgetExceeded => assert!(error.steps().is_some()),
            _ => {}
        }
    }
}

#[test]
fn bundle_kinds_hide_private_codec_types() {
    use crate::{BundleError, BundleErrorKind};
    for kind in [
        BundleErrorKind::EmptyCatalog,
        BundleErrorKind::MissingGrammar,
        BundleErrorKind::Decode,
    ] {
        let error = Error::Bundle(BundleError::new(
            kind,
            Some("test".into()),
            "invalid bundle".into(),
        ));
        assert_eq!(error, error.clone());
        assert_eq!(error.to_string(), "invalid bundle");
        let Error::Bundle(error) = error else {
            panic!()
        };
        assert_eq!(error.kind(), kind);
        assert_eq!(error.language(), Some("test"));
    }
}

#[test]
fn private_validation_failures_map_without_exposing_engine_types() {
    let error = crate::engine::grammar::GrammarValidationError::new(
        "source.invalid",
        "patterns[0]",
        "rule",
        "unknown rule id 99",
    );
    let error = grammar_error(super::grammar_load_error(
        crate::engine::grammar::GrammarLoadError::Validation {
            path: None,
            source: Box::new(error),
        },
    ));
    assert_eq!(error.scope_name(), Some("source.invalid"));
    assert!(matches!(
        error.kind(),
        GrammarErrorKind::InvalidReference(_)
    ));
    assert!(error.to_string().contains("unknown rule id 99"));
    assert!(error.source().is_none());
}

#[cfg(feature = "diagnostics")]
#[test]
fn private_matcher_build_failures_keep_pattern_context() {
    let error = crate::DiagnosticError::build("(?bad)", "cannot build matcher".into());
    assert_eq!(error.kind(), crate::DiagnosticErrorKind::MatcherBuild);
    assert_eq!(error.pattern(), "(?bad)");
    assert_eq!(error.to_string(), "cannot build matcher");
}

#[cfg(feature = "bundled-grammars")]
#[test]
fn missing_bundle_grammar_maps_at_the_boundary() {
    let error = crate::engine::load_grammar_set("missing-language").unwrap_err();
    let Error::Bundle(error) = error else {
        panic!()
    };
    assert_eq!(error.kind(), crate::BundleErrorKind::MissingGrammar);
    assert_eq!(error.language(), Some("missing-language"));
}

#[test]
fn preparation_limits_keep_the_known_root_scope() {
    // Enough pending references to exceed preparation's bounded graph walk.
    // Loading remains permissive; validation of this bound is preparation-only.
    let patterns = vec![r##"{"include":"#empty"}"##; 300_000].join(",");
    let json = format!(
        r#"{{"scopeName":"source.large","patterns":[{patterns}],"repository":{{"empty":{{"patterns":[]}}}}}}"#
    );
    let mut registry = GrammarRegistry::with_limits(GrammarLimits {
        max_grammar_bytes: json.len(),
        max_grammars: 1,
    });
    let root = registry.add_json(&json).unwrap();
    let error = grammar_error(PreparedLanguage::new(&registry, root).unwrap_err());
    assert_eq!(error.scope_name(), Some("source.large"));
    assert!(matches!(
        error.kind(),
        GrammarErrorKind::PreparationLimit(_)
    ));
}

// Payloads are boxed so `Result<T>` stays small on hot paths; keep `Error`
// no larger than its largest inline payload (`String`) plus the discriminant.
#[cfg(target_pointer_width = "64")]
#[test]
fn error_stays_small() {
    assert!(std::mem::size_of::<Error>() <= 32);
}
