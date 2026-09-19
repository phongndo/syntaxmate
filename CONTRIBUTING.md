# Contributing

Participation follows the [Code of Conduct](CODE_OF_CONDUCT.md). Report security
issues through [private vulnerability reporting](SECURITY.md).

## Setup and checks

Use `nix develop` for the environment defined in [flake.nix](flake.nix), or
install the toolchain in [rust-toolchain.toml](rust-toolchain.toml). The minimum
supported Rust version is in [Cargo.toml](Cargo.toml). Node is needed for asset
and oracle tooling, not normal library builds or tests; see the
[oracle setup](tools/golden-oracle/README.md).

Run from the repository root:

```sh
cargo fmt --all --check
cargo test --all-features --locked public_api_tests
cargo test --all-features --locked render::tests
cargo test --doc --all-features --locked
python3 tools/check-docs.py
python3 tools/check-language-docs.py --check
```

Choose additional checks for the affected code. [CI](.github/workflows/ci.yml)
is the executable definition of the complete matrix, including feature
combinations, MSRV, golden shards, generated files, performance, and packaged
consumers. To enable the optional local checks in [hk.pkl](hk.pkl), run
`hk install` with hk installed.

## Finding the right evidence

- API or ownership changes: start with the [architecture map](docs/architecture.md)
  and nearby public API tests.
- Scope changes: add a minimal [oracle fixture](tests/fixtures/textmate/README.md)
  and review the exact ordered scope change. Generate golden JSONL from the
  pinned oracle; do not hand-edit it or normalize away a mismatch.
- Grammar/theme updates: follow [asset maintenance](docs/assets.md).
- Regex or theme behavior: use the [oracle tools](tools/golden-oracle/README.md).
- Performance changes: follow the [measurement procedure](benchmarks/textmate/README.md).
- Releases and compatibility commitments: see [releasing](docs/releasing.md).

Explain the general downstream use case in a pull request. Document public API
changes in rustdoc and user-visible behavior changes in the changelog. Keep
private engine details behind the public boundary and cover relevant feature
combinations, including custom assets without default features.

Parsers and renderers accept untrusted input. Include malformed-input and
injection coverage when changing those boundaries. Fuzz targets and scheduled
commands live in [fuzz/](fuzz/) and the [fuzz workflow](.github/workflows/fuzz.yml).

## Documentation maintenance

Keep API contracts beside their implementation, runnable examples in doctests
or `examples/`, and prose for usage, rationale, or procedures that need explanation.
Link to manifests, policies, reports, and CI instead of copying changing
versions, counts, cache limits, command matrices, or benchmark tables.

Update or delete affected guidance in the same change as the code. Completed
plans and investigation transcripts belong in Git history or linked issues/PRs,
not among current instructions. Keep generated documents generator-owned.
The README and rendering-guide Rust examples run as doctests; the documentation
checks validate local link targets and generated language/scale summaries.
