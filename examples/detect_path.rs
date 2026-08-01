use syntaxmate::{Catalog, Highlighter};

fn main() -> syntaxmate::Result<()> {
    let path = "src/main.rs";
    let source = "fn main() {}";
    let language = Catalog::bundled()
        .detect_path(path)
        .ok_or_else(|| syntaxmate::Error::UnknownLanguage(path.to_owned()))?;

    let mut highlighter = Highlighter::bundled()?;
    let document = highlighter.highlight_path(path, source, "github-dark")?;
    println!("{language}: {} highlighted line(s)", document.lines().len());
    Ok(())
}
