# Quality contract

Syntaxmate treats output compatibility, bounded execution, package isolation,
and supply-chain provenance as release requirements rather than best-effort
checks.

## Merge gates

- formatting and Clippy with warnings denied;
- rustdoc with warnings denied;
- MSRV and current-stable builds;
- default, no-default, all-features, and feature-powerset checks;
- public API semver checks against the latest crates.io release after bootstrap;
- unit, public API, incremental-state replay, checkpoint, renderer, and doctests;
- strict sharded oracle parity for every public language;
- actual scanner-execution replay against `vscode-oniguruma`;
- grammar-regex construct coverage and deterministic differential mutation;
- deterministic grammar bundle and generated documentation;
- Linux, macOS, and Windows public API tests;
- dependency advisory/license/source policy;
- packaged default and custom-assets downstream consumers;
- package size and catalog performance floors.

Coverage is published for the focused library suite. Scheduled workflows run
longer fuzz campaigns and static security analysis.

## Compatibility evidence

The pinned JavaScript oracle is development-only. Checked-in goldens preserve
its exact ordered scope stacks after UTF-16-to-UTF-8 offset conversion. An empty
final-output divergence allowlist and stale-exception detection prevent silent
normalization of known mismatches.

Final tokens can conceal lower-level matcher differences. Inspired by
[Shiki's record-and-replay comparison](https://github.com/shikijs/shiki/blob/main/packages/engine-javascript/test/compare.test.ts)
of its JavaScript and Oniguruma engines,
`regex-execution-parity.mjs` records real scanner calls made while highlighting
the stress fixtures for all 31 core regression assets and replays a balanced,
deterministic sample through Syntaxmate. Every core language contributes to the
sample. Exact winners, ranges, and captures are required.
The narrowly documented dormant-capture differences live in
`benchmarks/textmate/regex-execution-differences.json`; new differences and
stale exceptions both fail CI, following the known-failure baseline discipline
used by [Syntect's dual regex-backend syntax tests](https://github.com/trishume/syntect/blob/master/.github/workflows/CI.yml).

The focused regex proving corpus must represent every advanced construct and
variant inventoried from bundled grammars. Deterministic mutations add hostile
Unicode and surrounding text. Every committed oracle fixture is also replayed
twice from identical incremental state, ensuring cache history cannot change
output or continuation state.

Theme goldens validate scope matching, colors, alpha compositing, and font
modifiers. Generated catalog documentation locks public, validated, oracle, and
stress-corpus counts.

## Performance evidence

Machine-sensitive throughput uses stable generated corpora and exact input
hashes. Deterministic engine counters remain the preferred merge signal for
micro-optimizations; reference-machine measurements guard whole-catalog
throughput, bundle size, and theme cache behavior. The validation policy keeps
the reference-machine floor separate from conservative per-language and
aggregate floors calibrated for variable GitHub-hosted runners.

Use the allocation profiler to compare construction, first/warm whole-document
runs, incremental tokenization, and incremental highlighting on one corpus:

```sh
cargo run --release --example profile-alloc -- rust path/to/source.rs
```

It reports allocation/reallocation calls, cumulative allocated bytes, retained
bytes at the phase boundary, elapsed time, and allocations per KiB.

## Release evidence

A release tag is required to match `Cargo.toml` and a non-Unreleased changelog
section. The release workflow reuses full CI, packages the crate, records a
SHA-256 checksum, publishes through crates.io OIDC trusted publishing, attests
the archive provenance, and creates a GitHub release.

The first crates.io publication must be bootstrapped manually because a trusted
publisher can only be configured after the crate exists. Later releases should
not use long-lived registry tokens.

## Reporting regressions

A useful correctness report includes the Syntaxmate version, bundle version,
language ID, theme, minimal source, expected scopes/style, actual scopes/style,
and whether status was `Complete` or `Degraded`. Security-sensitive cases
follow `SECURITY.md` instead of a public issue.
