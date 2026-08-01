use syntaxmate::Highlighter;

fn main() -> syntaxmate::Result<()> {
    let source = "fn main() { println!(\"<hello>\"); }";
    let mut highlighter = Highlighter::bundled()?;
    let output = highlighter.highlight_html("rust", source, "github-dark")?;
    assert!(output.status().is_complete());
    println!("{}", output.as_str());
    Ok(())
}
