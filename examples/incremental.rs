use syntaxmate::Highlighter;

fn main() -> syntaxmate::Result<()> {
    let highlighter = Highlighter::bundled()?;
    let mut session = highlighter.session("rust", "github-dark")?;

    for line in ["fn main() {", "    println!(\"hello\");", "}"] {
        let highlighted = session.highlight_line(line)?;
        println!("{} span(s)", highlighted.spans().len());
    }
    Ok(())
}
