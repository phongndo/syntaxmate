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
    let css = html_stylesheet(&theme, "example");
    let mut output = String::new();
    let status = render_html_to(source, &document, &options, &mut output)?;
    assert!(status.is_complete());
    println!("<style>{css}</style>\n{output}");
    Ok(())
}
