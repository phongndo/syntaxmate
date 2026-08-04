# Contributing

Thank you for improving Syntaxmate. By participating, you agree to the
[Code of Conduct](CODE_OF_CONDUCT.md).

## Development setup

Install Rust 1.88 or newer. The checked-in toolchain file selects the normal
stable toolchain. Node 24 is needed only when regenerating the pinned
`vscode-textmate` oracle.

The repository includes an [hk](https://hk.jdx.dev/) configuration for fast
local consistency checks. With `hk` installed, enable it once with `hk install`
(or the recommended global `hk install --global`).

Start with focused checks while iterating:

```sh
cargo fmt --all
cargo test --all-features public_api_tests
cargo test --all-features render::tests
```

Before submitting a pull request, run:

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo check --lib --no-default-features --locked
python3 tools/check-language-docs.py --check
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --locked
cargo test --all-features --locked
cargo run --bin syntaxmate-bundle --features bundle-tools --locked -- --check
cargo package --locked
```

CI additionally checks Rust 1.88, feature powersets, three operating systems,
downstream package consumers, coverage, generated files, whole-catalog
performance, and four strict oracle shards.

## Pull requests

- Keep changes focused and explain the downstream use case.
- Add public API documentation, tests, and a changelog entry for user-visible
  behavior.
- Preserve `default-features = false` unless the change intentionally requires
  a documented feature.
- Prefer conventional commit prefixes such as `feat:`, `fix:`, `perf:`,
  `docs:`, `test:`, and `chore:`.
- Do not expose regex bytecode, grammar rule IDs, caches, or other engine
  internals to solve one application's integration problem.
- Never add filesystem, network, or process-environment access to the release
  library.

## Compatibility changes

TextMate output changes require a fixture demonstrating the behavior and review
against the pinned oracle. Do not hand-edit `*.golden.jsonl`. Update a source
fixture or pinned asset, regenerate, and review the exact scope-stack change.
Unused divergence exceptions fail the suite; new exceptions require explicit
justification and prevent a language from being considered validated.

Oracle regeneration uses the lockfile exactly:

```sh
npm ci --prefix tools/golden-oracle
node tools/generate-textmate-cases.mjs --check
node tools/generate-goldens.mjs --check
node tools/generate-theme-goldens.mjs --check
```

## Grammar and theme assets

Every asset update requires an immutable upstream revision, source URL, license
record, checksum, and deterministic generated output. Follow
[`docs/assets.md`](docs/assets.md). Include the affected language/theme IDs and
the scope/style impact in the pull request.

## Security and fuzzing

Treat all grammar, theme, source, HTML, and terminal input as untrusted. New
parsers or renderers need malformed-input and injection tests. Run fuzz targets
with nightly Rust when changing those boundaries:

```sh
cargo install cargo-fuzz
cargo +nightly fuzz run grammar_and_source
cargo +nightly fuzz run theme_json
```

Report suspected vulnerabilities privately according to
[`SECURITY.md`](SECURITY.md).
