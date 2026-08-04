# Syntaxmate

[![Crates.io](https://img.shields.io/crates/v/syntaxmate.svg)](https://crates.io/crates/syntaxmate)
[![Documentation](https://docs.rs/syntaxmate/badge.svg)](https://docs.rs/syntaxmate)
[![CI](https://github.com/phongndo/syntaxmate/actions/workflows/ci.yml/badge.svg)](https://github.com/phongndo/syntaxmate/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A fast, Rust-native syntax highlighter powered by TextMate grammars.

264 validated languages, four GitHub themes, incremental highlighting, safe
HTML and ANSI output, and no native Oniguruma dependency.

[**Documentation**](https://docs.rs/syntaxmate) ·
[**Examples**](examples) ·
[**Languages**](docs/language-status.md) ·
[**Compatibility**](docs/compatibility.md)

## Usage

```toml
[dependencies]
syntaxmate = "0.1"
```

```rust
use syntaxmate::Highlighter;

let mut highlighter = Highlighter::bundled()?;
let output = highlighter.highlight_html(
    "rust",
    "fn main() { println!(\"<hello>\"); }",
    "github-dark",
)?;

assert!(output.status().is_complete());
println!("{}", output.as_str());
# Ok::<(), syntaxmate::Error>(())
```

## License

[MIT](LICENSE)
