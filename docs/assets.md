# Asset provenance and updates

Bundled grammars and themes are source code from independent upstream projects.
Every asset must remain reproducible, reviewable, and legally attributable. The
default theme catalog is intentionally limited to GitHub dark/light themes and
their high-contrast variants; applications can load any compatible custom
TextMate theme at runtime.

## Source of truth

- `assets/grammars/SOURCE.toml` records grammar source pins and transformations.
- `assets/grammars/licenses.json` records per-asset SPDX and source metadata.
- `assets/themes/SOURCE.toml` records the theme source revision and checksums.
- `THIRD_PARTY_LICENSES.md` is the human-readable release notice.
- `assets/grammars.bundle` is the committed deterministic runtime artifact.

Release builds consume committed files only. They never download assets or run
Node.

## Grammar bundle format and compatibility

`assets/grammars.bundle` is an `MRKB` little-endian sectioned container. Format
version 2 contains sorted string/scope metadata, public language records,
individually compressed grammar blobs, and license records. Grammar blobs with
flag `1` contain `CGIR` version 1: a deterministic compiled-grammar encoding
with an insertion-ordered grammar string table, lexical supplemental strings,
unsigned varints, tagged rule bodies/references, and checked IDs. Deflate remains
per grammar so the runtime can decode only a selected external-include closure.

The bundle builder parses and compiles vendored JSON, writes `CGIR`, and includes
the compiler/codec sources in the generated source hash. The runtime rejects
unknown `MRKB` or `CGIR` versions and malformed/truncated records rather than
attempting a best-effort interpretation. Custom caller-supplied grammars still
use the JSON grammar compiler; only committed bundled grammars use binary IR.

Regenerate and verify the artifact with:

```sh
cargo run --locked --bin syntaxmate-bundle --features bundle-tools --
cargo run --locked --bin syntaxmate-bundle --features bundle-tools -- --check
```

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
