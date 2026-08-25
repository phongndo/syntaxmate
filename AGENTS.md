# AGENTS.md

## Cursor Cloud specific instructions

`syntaxmate` is a **Rust library crate** (a TextMate-grammar syntax highlighter),
not a long-running service. "Running the app" means running one of the
`examples/` binaries, which highlight source code to HTML/ANSI/token output.

- Toolchain is pinned to Rust `1.88` via `rust-toolchain.toml` and is
  auto-installed by `rustup` on the first `cargo` invocation. Do not switch to
  the VM default toolchain (`1.83`); it is too old for `edition = "2024"`.
- The update script runs `cargo fetch --locked` to pre-download dependencies.
  First build after that still compiles from source.
- Standard build/lint/test/package commands live in `CONTRIBUTING.md`; use those
  rather than inventing new ones. Common ones:
  - Build: `cargo build --all-features --locked`
  - Lint: `cargo fmt --all --check` and
    `cargo clippy --all-targets --all-features --locked -- -D warnings`
  - Test: `cargo test --all-features --locked` (~1 min; ~415 tests)
  - No-default-features check: `cargo check --lib --no-default-features --locked`
- Examples are gated behind feature flags (see the `[[example]]` blocks in
  `Cargo.toml` for each example's `required-features`). For example:
  `cargo run --example html --features "bundled-grammars bundled-themes html"`.
  The `basic` example runs with default features.
- The `syntaxmate-bundle` binary regenerates/validates the grammar bundle:
  `cargo run --bin syntaxmate-bundle --features bundle-tools --locked -- --check`.
- Node/Python tooling under `tools/` is only needed when regenerating the
  pinned `vscode-textmate` oracle or language docs (Node 24 recommended for
  oracle regen; the VM ships Node 22). These are not required to build, test,
  or run the library.
- Git hooks use [`hk`](https://hk.jdx.dev/) (`hk.pkl`): pre-commit/pre-push run
  the language-docs and competitive-benchmark checks. `hk` is not installed by
  default; install it only if you touch those globs.
