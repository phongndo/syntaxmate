use syntaxmate::Highlighter;

fn main() -> syntaxmate::Result<()> {
    let source = "fn main() { println!(\"hello\"); }";
    let mut highlighter = Highlighter::bundled()?;
    let document = highlighter.highlight("rust", source, "github-dark")?;

    for line in document.lines() {
        for span in line.tokens() {
            let scopes = span.scopes().collect::<Vec<_>>();
            println!("{:?} {:?} {scopes:?}", span.range(), span.style());
        }
    }
    Ok(())
}
