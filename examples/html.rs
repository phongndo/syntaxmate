use syntaxmate::{Highlighter, HtmlOptions, Theme, html_stylesheet, render_html_to};

fn main() -> syntaxmate::Result<()> {
    let source = "fn main() { println!(\"<hello>\"); }";
    let highlighter = Highlighter::bundled()?;
    let theme = Theme::bundled("github-dark")?;
    let document = highlighter.highlight_with_theme("rust", source, &theme)?;
    let options = HtmlOptions {
        class_prefix: Some("example".to_owned()),
        ..HtmlOptions::default()
    };
    let dark_css = html_stylesheet(&theme, "example");
    let light_css = html_stylesheet(&Theme::bundled("github-light")?, "example");
    let mut output = String::new();
    let status = render_html_to(source, &document, &options, &mut output)?;
    assert!(status.is_complete());
    // Both stylesheets target the same HTML; the browser chooses the theme.
    println!(
        "<style>{light_css}@media (prefers-color-scheme: dark){{{dark_css}}}</style>\n{output}"
    );
    Ok(())
}
