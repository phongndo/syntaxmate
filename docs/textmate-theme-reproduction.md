# TextMate theme regression reproduction

The focused regression fixture is
`tests/fixtures/textmate/latex/hw2-theme.tex`. Its raw scope golden and
resolved-style golden are generated with `vscode-textmate@9.2.0`,
`vscode-oniguruma@1.7.0`, the pinned LaTeX grammar, and GitHub Dark High
Contrast `6.3.4`. Semantic highlighting is disabled.

The frozen extraction-era mismatch examples are recorded in
`benchmarks/textmate/latex-baseline-mismatches.json`. Current exact-scope and
resolved-style goldens have no mismatches.

Reproduce the Rust and oracle checks with:

```sh
cargo test --locked theme_golden::
npm ci --prefix tools/golden-oracle
node tools/generate-theme-goldens.mjs --check
node tools/theme-selector-conformance.mjs
node tools/theme-catalog-parity.mjs --check
```

For a standalone scope stack, pipe a JSON array of scope names to the
`theme-resolve` development example:

```sh
printf '%s\n' '["source.rust","keyword.control.rust"]' | \
  cargo run --quiet --example theme-resolve -- github-dark-high-contrast
```
