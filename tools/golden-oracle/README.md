# Golden oracle dependencies

Development-only package used by the golden generators, regex conformance and
execution-replay tools, theme parity checks, and grammar/theme vendor scripts in
`tools/`. Versions are pinned exactly (no ranges) so oracle output and vendored
asset imports stay reproducible.

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

This compares a focused set of patterns against `vscode-oniguruma` by driving
the `syntaxmate` `regex-parse` example. Deterministic mutation expands that
proving set while retaining a reproducible seed:

```sh
node tools/fuzz-regex-conformance.mjs --seed 1 --cases 256
```

Replay a deterministic sample of scanner calls observed during real TextMate
tokenization:

```sh
node tools/regex-execution-parity.mjs --max-executions 512
```

All three commands require a working `cargo` toolchain and are development-only.

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
| `github-vscode-themes` | `6.3.4` | Pinned source for GitHub themes |
| `vscode-textmate` | `9.2.0` | TextMate line tokenizer reference |
| `vscode-oniguruma` | `1.7.0` | Oniguruma WASM used by the reference |

Bump source or oracle pins deliberately, reinstall with the lockfile, then
regenerate the affected assets and goldens and review the diff.
