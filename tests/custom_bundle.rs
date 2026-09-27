use syntaxmate::{Catalog, Highlighter, PreparedLanguage, Theme, TokenizerOptions};

// Generated with syntaxmate-bundle --languages json; includes its license.
const JSON: &[u8] = include_bytes!("fixtures/bundles/json.bundle");

#[test]
fn custom_bundle_works_without_bundled_assets() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<Catalog>();
    send_sync::<Highlighter>();
    send_sync::<PreparedLanguage>();

    let catalog = Catalog::from_static(JSON).unwrap();
    assert_eq!(catalog.languages(), ["json"]);
    let metadata = catalog.language("JSON").unwrap();
    assert_eq!(metadata.id(), "json");
    assert_eq!(metadata.root_scope(), "source.json");
    assert!(
        metadata
            .extensions()
            .iter()
            .any(|extension| extension == "json")
    );
    assert!(catalog.language("rust").is_none());
    assert_eq!(catalog.bundle_summary().grammar_count, 1);
    assert_eq!(catalog.licenses().len(), 1);
    assert_eq!(catalog.detect_path("data.JSON"), Some("json"));
    assert_eq!(catalog.detect(None, "// -*- mode: json -*-"), Some("json"));
    assert_eq!(catalog.detect(None, "// vim: set ft=json:"), Some("json"));

    let highlighter = Highlighter::new(&catalog);
    let theme = Theme::from_json(r#"{"name":"custom","tokenColors":[]}"#).unwrap();
    let source = r#"{"a": "<script>&\"", "b": true}"#;
    let expected = highlighter
        .highlight_with_theme("json", source, &theme)
        .unwrap();
    assert!(expected.status().is_complete());
    #[cfg(feature = "html")]
    {
        let rendered =
            syntaxmate::render_html(source, &expected, &syntaxmate::HtmlOptions::default())
                .unwrap();
        assert!(!rendered.as_str().contains("<script>"));
        assert!(rendered.as_str().contains("&lt;script&gt;"));
    }
    let mut session = highlighter.session_with_theme("json", &theme).unwrap();
    let first = session.highlight_line(source).unwrap();
    session.reset();
    let second = session.highlight_line(source).unwrap();
    assert_eq!(first, second);
    for (a, b) in first.tokens().iter().zip(second.tokens()) {
        assert!(a.scope_stack().is_some());
        assert_eq!(a.scope_stack(), b.scope_stack());
    }
    for a in first.tokens() {
        for b in first.tokens() {
            if !a.scopes().eq(b.scopes()) {
                assert_ne!(a.scope_stack(), b.scope_stack());
            }
        }
    }
    let prepared = PreparedLanguage::from_catalog(&catalog, "json").unwrap();
    let mut tokenizer = prepared.tokenizer(TokenizerOptions::default());
    let mut state = tokenizer.initial_state();
    let line = tokenizer.tokenize_line(source, &mut state).unwrap();
    assert!(
        line.tokens()
            .iter()
            .all(|token| token.scope_stack().is_some())
    );
    drop(catalog);
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let highlighter = highlighter.clone();
            let theme = &theme;
            let expected = &expected;
            scope.spawn(move || {
                for _ in 0..10 {
                    assert_eq!(
                        &highlighter
                            .highlight_with_theme("json", source, theme)
                            .unwrap(),
                        expected
                    );
                }
            });
        }
    });
    let temporary = JSON.to_vec();
    let owned = Catalog::from_bytes(&temporary).unwrap();
    drop(temporary);
    assert_eq!(owned.languages(), ["json"]);
    assert!(
        Highlighter::new(&owned)
            .tokenize("json", source)
            .unwrap()
            .status()
            .is_complete()
    );
}

#[test]
fn malformed_bundles_return_errors() {
    for end in 0..JSON.len() {
        assert!(
            Catalog::from_bytes(&JSON[..end]).is_err(),
            "truncated at {end}"
        );
    }
    for bytes in [b"".as_slice(), b"MRKB", b"<script>alert(1)</script>"] {
        match Catalog::from_bytes(bytes) {
            Err(syntaxmate::Error::Bundle(error)) => {
                assert_eq!(error.kind(), syntaxmate::BundleErrorKind::Invalid);
            }
            other => panic!("expected an invalid-bundle error, got {other:?}"),
        }
    }
    // Forge counts and cross-references in otherwise well-formed containers.
    for section_id in [5u32, 6, 7] {
        let offset = section_offset(JSON, section_id);
        let mut bad = JSON.to_vec();
        bad[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Catalog::from_bytes(&bad).is_err());
    }
    let languages = section_offset(JSON, 5);
    for field in [12, 16] {
        let mut bad = JSON.to_vec();
        bad[languages + field..languages + field + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Catalog::from_bytes(&bad).is_err());
    }
    let mut huge = JSON.to_vec();
    let blobs = section_offset(JSON, 6);
    huge[blobs + 20..blobs + 24].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(Catalog::from_bytes(&huge).is_err());
}

fn section_offset(bytes: &[u8], id: u32) -> usize {
    let count = u16::from_le_bytes(bytes[6..8].try_into().unwrap());
    for index in 0..usize::from(count) {
        let start = 32 + index * 24;
        if u32::from_le_bytes(bytes[start..start + 4].try_into().unwrap()) == id {
            return u64::from_le_bytes(bytes[start + 8..start + 16].try_into().unwrap()) as usize;
        }
    }
    panic!("missing section")
}
