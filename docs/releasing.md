# Releasing Syntaxmate

Release a reviewed, green commit on `main`. Downstream applications consume
published versions through the public API and do not control release order.

## Preparing a release

1. Add a dated `X.Y.Z` section to [CHANGELOG.md](../CHANGELOG.md), including
   migration notes and intentional scope/style changes.
2. Update the version in [Cargo.toml](../Cargo.toml) and regenerate `Cargo.lock`.
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

## Publishing credentials

The workflow uses crates.io OIDC trusted publishing through the `crates-io`
GitHub environment. The publisher configuration must match the repository,
workflow, and environment. Protect that environment with reviewers and tag-only
deployment rules; verify repository protections in GitHub rather than assuming
this file configures them. Routine releases do not require a stored registry token.

## Version policy

Patch releases preserve the public API and normally contain engine correctness,
safety, or documentation fixes. Catalog refreshes and intentional highlighting
changes use a minor release with the upstream pin and output impact recorded.
Breaking API changes require a minor release during 0.x and a major release
after 1.0, with migration notes.

MSRV increases require a minor release and release notes. Feature flags remain
additive within a release line. Diagnostic output, bundle encoding, and private
engine representations are not stable interfaces.
