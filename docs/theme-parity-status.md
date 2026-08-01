# TextMate theme parity status

| Theme | Selector engine | Asset | Status |
|---|---|---|---|
| GitHub Dark/Light and high-contrast variants | Exact TextMate scopes | `github-vscode-themes@6.3.4` | Enabled |
| Catppuccin Latte/Frappé/Macchiato/Mocha | Exact TextMate scopes | `@shikijs/themes@3.23.0` | Enabled |
| Gruvbox Dark/Light | Exact TextMate scopes | `@shikijs/themes@3.23.0` | Enabled |
| Tokyo Night | Exact TextMate scopes | `@shikijs/themes@3.23.0` | Enabled |
| System, ANSI, user Base16 | Coarse terminal palette | User/terminal-defined | Intentional fallback |

Semantic highlighting is disabled for parity comparisons. Raw tokenizer scope
goldens and resolved-style oracle goldens are checked independently.

The catalog report at `benchmarks/textmate/theme-parity.json` currently covers
526 committed fixtures, 453,228 tokens, and 59,847 unique scope stacks with no
GitHub Dark High Contrast style mismatches. In addition, 7,894 generated
theme-derived selector cases and 4,000 seeded randomized cases across all 11
named themes are checked directly against pinned `vscode-textmate` by the local
conformance tool.

`benchmarks/textmate/grammar-provenance-audit.json` gives every public and
private grammar an explicit reference, canonical hash, root scope, and external
dependencies. `tools/check-grammar-provenance.py --verify-vscode` also compares
the two pinned VS Code built-in dependency grammars directly with VS Code
`fc3def6774c76082adf699d366f31a557ce5573f`.

The broader source audit in
`benchmarks/textmate/vscode-grammar-differences.json` matches root scopes
against that complete VS Code checkout. Of 63 shared scopes, eight assets are
canonically identical to VS Code and 55 are source-different. The differential
behavior report in `benchmarks/textmate/vscode-grammar-behavior.json` tokenizes
all committed fixtures with both asset sets; all 55 produce identical scope
stacks and GitHub Dark High Contrast styles. Dart, Handlebars, PHP, R,
reStructuredText, and YAML now use the pinned VS Code assets directly. Pug uses
the same asset with unavailable Sass/Stylus includes preserved as unavailable,
matching VS Code's built-in registry behavior.
Regenerate or check the audit with:

```sh
python3 tools/audit-vscode-grammars.py \
  --vscode-checkout /path/to/vscode-1.128 --check
node tools/vscode-grammar-behavior-audit.mjs \
  --vscode-checkout /path/to/vscode-1.128 --check
```
