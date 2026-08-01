use syntaxmate::{GrammarRegistry, Tokenizer, TokenizerOptions};

fn main() -> syntaxmate::Result<()> {
    let grammar = r#"{
        "scopeName": "source.demo",
        "patterns": [{"match": "\\b(todo|done)\\b", "name": "keyword.demo"}]
    }"#;
    let mut registry = GrammarRegistry::new();
    let root = registry.add_json(grammar)?;
    let mut tokenizer = Tokenizer::new(&registry, root, TokenizerOptions::default())?;
    let document = tokenizer.tokenize("todo then done");
    println!("{} tokenized line(s)", document.lines().len());
    Ok(())
}
