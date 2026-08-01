# Releasing Syntaxmate

Syntaxmate releases are independent of downstream applications. Published
artifacts must come from a reviewed, green commit on `main`.

## One-time setup

The first `0.1.0` publication must be performed manually with a narrowly scoped,
short-lived crates.io token because trusted publishing is configured on an
existing crate. Immediately afterward:

1. add a crates.io trusted publisher for repository `phongndo/syntaxmate`,
   workflow `release.yml`, and GitHub environment `crates-io`;
2. configure the protected `crates-io` environment with required reviewer(s)
   and tag-only deployment rules;
3. enable GitHub private vulnerability reporting, branch protection, required
   CI/security checks, signed commits/tags where practical, and tag protection;
4. remove the bootstrap crates.io token.

Later releases use OIDC short-lived credentials and do not require a stored
registry secret.

## Preparing a release

1. Update `CHANGELOG.md`: replace `Unreleased` with the ISO release date and add
   migration notes for any public API or intentional output change.
2. Update the package version and regenerate `Cargo.lock`.
3. Regenerate and check `assets/grammars.bundle` and generated documentation.
4. Confirm third-party source pins, checksums, SPDX records, and notices.
5. Run formatting, Clippy, feature powersets, tests, strict golden shards,
   oracle checks, docs, MSRV, package consumers, performance, and security
   policy checks.
6. Run `cargo publish --dry-run --locked` from a clean checkout.
7. Merge the release commit to `main` and wait for required checks.
8. Create and push an annotated `vX.Y.Z` tag pointing at that exact commit.

The tag workflow verifies that the tag, manifest, and dated changelog agree. It
reuses full CI, packages the crate, records a SHA-256 checksum, authenticates to
crates.io through OIDC, publishes, attests provenance for the crate archive,
and creates a GitHub release with changelog notes and artifacts.

## Version policy

Patch releases preserve the public API and normally contain engine correctness,
safety, or documentation fixes. Catalog refreshes and intentional highlighting
changes use a minor release and include the upstream pin and output impact in
the changelog. Breaking API changes require a major release after 1.0; during
0.x they require a minor release and migration notes.

The MSRV may increase only in a minor release and must be called out in release
notes. Feature flags remain additive within a release line.

Downstream projects update through ordinary dependency pull requests. No
project receives an unpublished feature, path override, or friend API.
