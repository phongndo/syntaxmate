# Syntaxmate

[![Crates.io](https://img.shields.io/crates/v/syntaxmate.svg)](https://crates.io/crates/syntaxmate)
[![Documentation](https://docs.rs/syntaxmate/badge.svg)](https://docs.rs/syntaxmate)
[![CI](https://github.com/phongndo/syntaxmate/actions/workflows/ci.yml/badge.svg)](https://github.com/phongndo/syntaxmate/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/phongndo/syntaxmate/graph/badge.svg)](https://codecov.io/gh/phongndo/syntaxmate)
[![MSRV 1.88](https://img.shields.io/badge/MSRV-1.88-blue.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A Rust-native TextMate syntax highlighter with the batteries included: 264
validated languages, curated themes, exact scope stacks, incremental state,
path detection, safe HTML/ANSI output, and no native Oniguruma dependency.

```toml
[dependencies]
syntaxmate = "0.1"
```

## Highlight in three lines

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

`highlight_html` escapes source and attributes. `highlight_ansi` emits 24-bit
terminal colors and sanitizes source control characters by default.

## Structured highlighting

Use structured spans when rendering into an editor, TUI, or custom format:

```rust
use syntaxmate::Highlighter;

let mut highlighter = Highlighter::bundled()?;
let source = "fn main() { println!(\"hello\"); }";
let document = highlighter.highlight("rust", source, "github-dark")?;

for line in document.lines() {
    for span in line.spans() {
        let scopes = line.scope_names(span.scope_stack()).collect::<Vec<_>>();
        println!("{:?} {:?} {scopes:?}", span.range(), span.style());
    }
}
# Ok::<(), syntaxmate::Error>(())
```

Automatic path detection and incremental sessions use the same catalog:

```rust
use syntaxmate::Highlighter;

let highlighter = Highlighter::bundled()?;
let mut session = highlighter.session("rust", "github-dark")?;
for line in ["fn main() {", "    println!(\"hello\");", "}"] {
    let output = session.highlight_line(line)?;
    assert!(output.status().is_complete());
}
# Ok::<(), syntaxmate::Error>(())
```

See the runnable [`examples`](examples), the [rendering guide](docs/rendering.md),
and the API documentation on [docs.rs](https://docs.rs/syntaxmate).

## Custom assets

Disable bundled assets when an application supplies its own grammars and
themes:

```toml
syntaxmate = { version = "0.1", default-features = false }
```

Use `GrammarRegistry`, `Tokenizer`, and `Theme::from_json` for that path. Add
any externally included grammars you want resolved. Missing optional includes
are ignored like `vscode-textmate`; call `GrammarRegistry::validate` when you
require a strict, closed include graph.

## Feature flags

| Feature | Default | Purpose |
| --- | :---: | --- |
| `bundled-grammars` | yes | Validated grammar catalog and path metadata |
| `bundled-themes` | yes | Curated TextMate color themes |
| `html` | yes | Escaped, dependency-free HTML renderer |
| `ansi` | yes | 24-bit ANSI renderer with terminal-injection protection |
| `diagnostics` | no | Counters and regex conformance diagnostics |
| `bundle-tools` | no | Deterministic catalog bundle generator binary |

Features are additive. Release builds are offline and do not require Node.
Node packages under `tools/golden-oracle` are development-only.

## Compatibility and trust

Syntaxmate checks every public language against pinned `vscode-textmate` and
`vscode-oniguruma` output. The checked-in contract currently covers 544 oracle
cases across all 264 language IDs with no divergence allowlist. Public ranges
are UTF-8 byte offsets; language-server semantic tokens are outside scope.

Regex and tokenizer work is bounded. Callers that require complete output must
check `HighlightStatus`; safe fallback output is marked `Degraded` when a
limit is exhausted. The release library performs no filesystem, network, or
environment-variable access.

- [Architecture](docs/architecture.md)
- [Asset provenance and updates](docs/assets.md)
- [TextMate compatibility](docs/compatibility.md)
- [Quality contract](docs/quality.md)
- [Security policy](SECURITY.md)
- [Support policy](SUPPORT.md)

Syntaxmate combines Shiki-like batteries-included TextMate assets with a
Syntect-like native Rust API, while using its own bounded regex engine and
oracle-driven compatibility contract. See the [project comparison](docs/compatibility.md#project-positioning)
for precise scope and non-goals.

Syntaxmate originated in the syntax engine developed for
[Mark](https://github.com/phongndo/mark). It is not affiliated with TextMate,
Microsoft, Syntect, or Shiki.
