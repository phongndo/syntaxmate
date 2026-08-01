use syntaxmate::Highlighter;

fn main() -> syntaxmate::Result<()> {
    let source = "fn main() { println!(\"hello\"); }";
    let mut highlighter = Highlighter::bundled()?;
    let document = highlighter.highlight("rust", source, "github-dark")?;

    for line in document.lines() {
        for span in line.spans() {
            let scopes = line.scope_names(span.scope_stack()).collect::<Vec<_>>();
            println!("{:?} {:?} {scopes:?}", span.range(), span.style());
        }
    }
    Ok(())
}
