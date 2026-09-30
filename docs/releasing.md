# Releasing Syntaxmate

Release a reviewed, green commit on `main`. Downstream applications consume
published versions through the public API and do not control release order.

## Preparing a release

1. Add a dated `X.Y.Z` section to [CHANGELOG.md](../CHANGELOG.md), including
   migration notes and intentional scope/style changes.
2. Update the version in [Cargo.toml](../Cargo.toml) and the bindings (see
   [Language bindings](#language-bindings)), then run `cargo update --workspace`
   for `Cargo.lock`, `bindings/Cargo.lock`, and `fuzz/Cargo.lock`.
3. Review asset provenance and generated-file freshness using the
   [asset procedure](assets.md). Require the complete [CI matrix](../.github/workflows/ci.yml)
   to pass, including packaged consumers and performance policies.
4. Run `cargo publish --dry-run --locked` from a clean checkout.
5. Merge to `main`, wait for required checks, then create and push an annotated
   `vX.Y.Z` tag at that exact commit.

The [release workflow](../.github/workflows/release.yml) re-runs CI and verifies
the tag, package version, and changelog heading. It packages, checksums, publishes,
attests, and creates the GitHub release. The workflow rejects an `Unreleased`
heading; maintainers must review the date and release notes themselves.

## Published package

The [Cargo include list](../Cargo.toml) ships the compiled grammar bundle, bundled
themes, their licenses and provenance notices, library sources, documentation,
and the self-contained HTML/ANSI examples. Raw grammar JSON, test fixtures and
targets, profiling examples, and the bundle-builder executable are checkout-only.
Cargo removes excluded targets from the published manifest; the full test suite
and bundle regeneration require a Git checkout.

The [package CI job](../.github/workflows/ci.yml) verifies the archive, checks its
contents and retained licenses, builds its examples and documentation, and runs
separate default-feature and custom-asset consumers. Review `cargo package --list
--locked` and run `cargo package --locked` when changing the include list.

The docs.rs feature list in [Cargo.toml](../Cargo.toml) explicitly selects the
user-facing features. It excludes `bundle-tools` and `diagnostics`: both support
checkout maintenance and expose implementation details outside the stable public
contract. Nightly rustdoc labels feature-gated items through `doc(auto_cfg)`;
ordinary stable documentation builds do not require unstable compiler features.

## Language bindings

The Python and JavaScript bindings share the crate's version. Bump
[bindings/Cargo.toml](../bindings/Cargo.toml),
[pyproject.toml](../bindings/python/pyproject.toml), and
[package.json](../bindings/wasm/package.json) with [Cargo.toml](../Cargo.toml),
along with the version check in the Python tests.

The same tag triggers the [bindings release workflow](../.github/workflows/release-bindings.yml).
It runs the Python and JavaScript binding workflows at the tagged commit. It
then publishes the manylinux, macOS, and Windows `abi3` wheels and the sdist to
PyPI, and the packed tarball to npm. Versions already on a registry are skipped.
Run it manually with a ref to retry, or to publish bindings for an already
released version. The C and C++ binding ships only as source in the tagged
repository.

## Publishing credentials

The workflow uses crates.io OIDC trusted publishing through the `crates-io`
GitHub environment. The publisher configuration must match the repository,
workflow, and environment. Protect that environment with reviewers and tag-only
deployment rules; verify repository protections in GitHub rather than assuming
this file configures them. Routine releases do not require a stored registry token.

The bindings use the same arrangement: PyPI trusted publishing through the
`pypi` environment and npm trusted publishing through the `npm` environment,
both for `release-bindings.yml`. npm accepts a trusted publisher only for an
existing package, so the first npm version must be published manually.

## Version policy

Patch releases preserve the public API and normally contain engine correctness,
safety, or documentation fixes. Catalog refreshes and intentional highlighting
changes use a minor release with the upstream pin and output impact recorded.
Breaking API changes require a minor release during 0.x and a major release
after 1.0, with migration notes.

The minimum supported Rust version is declared in [Cargo.toml](../Cargo.toml)
and checked by the [MSRV CI job](../.github/workflows/ci.yml) against `Cargo.lock`,
with default features, no default features, and all development targets/features.
Rust 1.88 is required by let chains and `slice::as_chunks` in the library; the
locked dependencies do not raise that floor. The pinned development toolchain
may be newer. Recheck the MSRV when updating dependencies.

MSRV increases require a minor release and release notes. Feature flags remain
additive within a release line. Diagnostic output, bundle encoding, and private
engine representations are not stable interfaces.
