# Development oracle

[package.json](package.json) and [package-lock.json](package-lock.json) pin the
TextMate tokenizer, Oniguruma WASM, and grammar/theme source packages. These are
development dependencies, excluded from the released crate. Run commands from
the repository root; use the Node version configured in
[CI](../../.github/workflows/ci.yml) or enter `nix develop`.

```sh
npm ci --prefix tools/golden-oracle
```

For bundled scope fixtures, use the
[fixture workflow](../../tests/fixtures/textmate/README.md). The commands below
need Cargo as well as the installed oracle.

## Regex and custom grammars

```sh
node tools/regex-conformance.mjs --out target/regex-conformance.json
node tools/regex-execution-parity.mjs --max-executions 512 --out target/regex-execution-parity.json
node tools/fuzz-regex-conformance.mjs --seed 1 --cases 256 --out target/regex-differential-fuzz.json
node tools/generate-engine-regressions.mjs --check
cargo test --all-features --locked engine_regressions
```

Conformance checks explicit pattern cases. Execution replay samples real scanner
calls and compares winners, ranges, and captures; its
[difference ledger](../../benchmarks/textmate/regex-execution-differences.json)
rejects new and stale exceptions. Differential mutation expands the proving set
with a reproducible seed. None of these establishes universal regex equivalence.

[Custom-grammar cases](../../tests/fixtures/engine-regressions/cases.json) isolate
behavior that final bundled output can hide, including capture interpolation,
retokenization, dynamic continuation, and Unicode lookbehind. Regenerate their
scope goldens with `node tools/generate-engine-regressions.mjs` after reviewing
source-case changes.

## Themes

```sh
cargo test --all-features --locked theme_golden::
tools/check-textmate-parity.sh
```

The [parity script](../check-textmate-parity.sh) checks provenance, vendored
assets, style goldens, selector conformance, and catalog scope-stack replay.
It does not replace the Rust tests. Current report data lives in
[theme-parity.json](../../benchmarks/textmate/theme-parity.json); avoid copying
its token counts or pass status into prose.

The focused LaTeX reproduction is
[hw2-theme.tex](../../tests/fixtures/textmate/latex/hw2-theme.tex). Historical
extraction mismatches are frozen in
[latex-baseline-mismatches.json](../../benchmarks/textmate/latex-baseline-mismatches.json),
not a current exception list. Editor comparisons exclude semantic highlighting
and decorations; see [compatibility](../../docs/compatibility.md).

Inspect a standalone scope stack with:

```sh
printf '%s\n' '["source.rust","keyword.control.rust"]' | \
  cargo run --quiet --example theme-resolve -- github-dark-high-contrast
```

For changing source pins and regenerating assets, follow
[asset maintenance](../../docs/assets.md). Review lockfile and scope/style diffs
together when changing oracle versions.
