#![cfg(feature = "bundled-grammars")]

use syntaxmate::{HighlightStatus, Highlighter, RgbColor, Theme};

#[test]
fn custom_theme_incremental_session_needs_no_bundled_themes() {
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
            == Some(RgbColor {
                red: 0x11,
                green: 0x22,
                blue: 0x33,
            })
    }));
}
