# TextMate theme parity status

Syntaxmate bundles four accessible themes from `github-vscode-themes@6.3.4`:

| Theme ID | Selector engine | Status |
|---|---|---|
| `github-dark` | Exact TextMate scopes | Enabled |
| `github-dark-high-contrast` | Exact TextMate scopes | Enabled |
| `github-light` | Exact TextMate scopes | Enabled |
| `github-light-high-contrast` | Exact TextMate scopes | Enabled |

Arbitrary caller-provided TextMate themes remain supported through
`Theme::from_json`, including in builds without the `bundled-themes` feature.
Tokenization is theme-independent.

Semantic highlighting is disabled for parity comparisons. Raw tokenizer scope
goldens and resolved-style oracle goldens are checked independently.

The committed catalog report at `benchmarks/textmate/theme-parity.json` covers
546 golden files, 466,480 tokens, and 60,901 unique scope stacks with no GitHub
Dark High Contrast style mismatches. `tools/theme-selector-conformance.mjs`
also checks 1,040 theme-derived selector cases across the four bundled themes
and 4,000 deterministically generated custom-theme cases against pinned
`vscode-textmate`.

Run the complete local theme contract with:

```sh
npm ci --prefix tools/golden-oracle
tools/check-textmate-parity.sh
```

Theme provenance, source revisions, licenses, and checksums are recorded in
`assets/themes/SOURCE.toml` and `assets/themes/licenses.json`.
