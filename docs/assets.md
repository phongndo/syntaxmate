# Asset provenance and updates

Bundled grammars and themes are source code from independent upstream projects.
Every asset must remain reproducible, reviewable, and legally attributable.

## Source of truth

- `assets/grammars/SOURCE.toml` records grammar source pins and transformations.
- `assets/grammars/licenses.json` records per-asset SPDX and source metadata.
- `assets/themes/SOURCE.toml` records theme sources, revisions, adaptations, and
  checksums.
- `THIRD_PARTY_LICENSES.md` is the human-readable release notice.
- `assets/grammars.bundle` is the committed deterministic runtime artifact.

Release builds consume committed files only. They never download assets or run
Node.

## Updating a grammar or theme

1. Pin an immutable upstream revision and verify its license.
2. Apply the smallest documented transformation needed by the runtime format.
3. Update source, checksum, and license records.
4. Regenerate the grammar bundle when grammar input changes.
5. Regenerate oracle/theme goldens with the pinned development tools.
6. Review scope/style changes and update the changelog.
7. Run generated-file, full golden, performance, package, and license gates.

Do not hand-edit generated golden JSONL or the compressed bundle.

## Catalog promotion

A public language requires aliases/path metadata, basic and stress fixtures,
exact oracle parity, no unresolved required dependency, no budget degradation,
provenance, and the catalog performance floor. The machine-readable counts and
promotion ledger are documented in `docs/language-status.md` and
`benchmarks/textmate/validation-policy.json`.

Optional external TextMate includes may be absent, matching the reference host
behavior. Required bundled dependencies must be present in the private grammar
closure even when they are not public language IDs.
