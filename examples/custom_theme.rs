use syntaxmate::{Highlighter, Theme};

fn main() -> syntaxmate::Result<()> {
    let theme = Theme::from_json(
        r##"{
        "name": "Demo",
        "colors": {"editor.foreground": "#d0d0d0", "editor.background": "#101010"},
        "tokenColors": [{"scope": "keyword", "settings": {"foreground": "#ff6600"}}]
    }"##,
    )?;
    let mut highlighter = Highlighter::bundled()?;
    let document = highlighter.highlight_with_theme("rust", "fn main() {}", &theme)?;
    println!("{} highlighted line(s)", document.lines().len());
    Ok(())
}
