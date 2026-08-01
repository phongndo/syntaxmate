# Golden oracle dependencies

Development-only package used by `../golden-dump.mjs`, `../generate-goldens.mjs`,
`../regex-conformance.mjs`, `../vendor-shiki-grammars.mjs`, and
`../vendor-textmate-themes.mjs`, and `../vscode-grammar-behavior-audit.mjs`.
Versions are
pinned exactly (no ranges) so oracle output and vendored grammar imports stay
reproducible.

These dependencies are **not** used by release builds and are intentionally kept
out of the Rust workspace. Install only when regenerating goldens, running regex
conformance, or checking the Shiki grammar vendor snapshot.

## Install

```sh
npm install --prefix tools/golden-oracle
```

## Regenerate TextMate goldens

From the repository root, with the pinned grammar assets under
`assets/grammars/`:

```sh
# all cases in the manifest
node tools/generate-goldens.mjs

# one language id (matches [[case]].language)
node tools/generate-goldens.mjs --case rust
node tools/generate-goldens.mjs --case java

# fail if committed goldens differ (CI-friendly)
node tools/generate-goldens.mjs --check
```

Ad-hoc single file:

```sh
node tools/golden-dump.mjs \
  --language rust \
  --scope source.rust \
  --grammar assets/grammars/languages/rust.tmLanguage.json \
  --file tests/fixtures/textmate/rust/basic.rs \
  --out tests/fixtures/textmate/rust/basic.golden.jsonl
```

## Regex conformance helper

```sh
node tools/regex-conformance.mjs
# optional: --out target/regex-conformance-phase2.json
```

This compares a small set of patterns against `vscode-oniguruma` by driving the
`syntaxmate` `regex-parse` example. It requires a working `cargo` toolchain and
is also dev-only.

## Shiki grammar vendor check

```sh
node tools/vendor-shiki-grammars.mjs --check
```

This verifies `assets/grammars/languages/`, `coverage.toml`,
`coverage.full-shiki.toml`, and `licenses.json` against the pinned
`@shikijs/langs` package installed here.

## Pins

| Package | Version | Role |
| --- | --- | --- |
| `@shikijs/langs` | `3.23.0` | Pinned source for vendored TextMate grammars |
| `@shikijs/themes` | `3.23.0` | Pinned source for named community themes |
| `github-vscode-themes` | `6.3.4` | Pinned source for GitHub themes |
| `vscode-textmate` | `9.2.0` | TextMate line tokenizer reference |
| `vscode-oniguruma` | `1.7.0` | Oniguruma WASM used by the reference |

Bump both together, reinstall with the lockfile, then regenerate goldens and
review the diff.
