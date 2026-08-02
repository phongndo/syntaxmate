# TextMate theme highlighting

Syntaxmate preserves each token's complete TextMate scope stack and resolves
that stack against TextMate theme selectors. Changing a theme does not
retokenize source files.

## Theme catalog

The default `bundled-themes` feature embeds four themes:

- `github-dark`
- `github-dark-high-contrast`
- `github-light`
- `github-light-high-contrast`

All four come from the pinned `github-vscode-themes@6.3.4` release. Provenance,
license text, revisions, and checksums are recorded under `assets/themes/`.
Release builds do not require Node, network access, or external asset files.

Custom TextMate themes are first-class styling inputs:

```rust
use syntaxmate::{Highlighter, Theme};

let theme = Theme::from_json(r##"{
  "name": "Custom",
  "tokenColors": [{
    "scope": "keyword.control",
    "settings": { "foreground": "#ff9492", "fontStyle": "bold" }
  }]
}"##)?;
let mut highlighter = Highlighter::bundled()?;
let document = highlighter.highlight_with_theme("rust", "fn main() {}", &theme)?;
# Ok::<(), syntaxmate::Error>(())
```

`Theme::from_json`, `Theme::resolve_scope_names`, and custom-theme highlighting
remain available when `bundled-themes` is disabled. Incremental callers use
`Highlighter::session_with_theme`.

## Selector and style contract

Selectors use TextMate parent/target matching and rule-order precedence.
Resolved styles can provide foreground, background, bold, italic, underline,
and strikethrough. Transparent colors are composited against the theme's opaque
editor background using the same contract as the pinned oracle.

Reference comparisons use `vscode-textmate@9.2.0` and
`vscode-oniguruma@1.7.0`, with semantic highlighting disabled. Language-server
semantic tokens, bracket-pair colorization, inlay hints, and editor decorations
are outside this contract.

Theme goldens, catalog-wide scope-stack replay, generated selector cases, and
deterministic custom-theme cases are separate checks. See
[`theme-parity-status.md`](theme-parity-status.md) and run:

```sh
tools/check-textmate-parity.sh
```
