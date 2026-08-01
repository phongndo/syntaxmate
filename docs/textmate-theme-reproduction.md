# TextMate theme reproduction

The frozen regression is
`tests/fixtures/textmate/latex/hw2-theme.tex`. Its raw scope
golden and resolved-style golden are generated with `vscode-textmate@9.2.0`,
`vscode-oniguruma@1.7.0`, the LaTeX grammar pinned in the grammar manifest, and
GitHub Dark High Contrast `6.3.4`. Semantic highlighting is disabled.

Before exact scope transport, punctuation nested inside `support.function`,
`constant.character` values, and markup modifiers were collapsed into coarse
classes. The frozen baseline examples and their expected selectors are in
`benchmarks/textmate/latex-baseline-mismatches.json`. The current scope and
resolved-style goldens have zero mismatches.

Inspect any token without a screenshot:

```sh
mark syntax inspect \
  tests/fixtures/textmate/latex/hw2-theme.tex \
  --line 10 --theme github-dark-high-contrast
```

The diagnostic reports byte/UTF-16 ranges, complete scopes, selector score,
rule order, colors, modifiers, configured coarse/scope overrides, and final
foreground. Diff overlays are absent in this source-oriented command and are
reported as such.
