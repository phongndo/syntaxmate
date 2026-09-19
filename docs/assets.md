# Asset provenance and updates

Bundled grammars and themes come from independent upstream projects. Preserve
immutable source revisions, licenses, checksums, and documented transformations
so updates remain reviewable and reproducible.

## Authoritative inputs

- Grammars: [source pins](../assets/grammars/SOURCE.toml),
  [licenses](../assets/grammars/licenses.json),
  [public/private coverage](../assets/grammars/coverage.toml), and
  [path metadata](../assets/grammars/language-metadata.json).
- Themes: [source pins](../assets/themes/SOURCE.toml) and
  [licenses](../assets/themes/licenses.json).
- Attribution: [third-party notices](../THIRD_PARTY_LICENSES.md).
- Runtime bundle: [assets/grammars.bundle](../assets/grammars.bundle), generated
  by the [bundle builder](../tools/build-bundle.rs).

The bundle uses independently compressed compiled grammars so a caller loads
only the selected language's dependency closure. Its format is private; version
and validation rules live in the [container decoder](../src/grammars/bundle.rs)
and [grammar IR codec](../src/engine/grammar_ir.rs). Custom grammars use the JSON
compiler. Release builds consume committed assets without running Node or
fetching upstream sources.

## Updating assets

1. Review the upstream change and license, then update the appropriate source
   manifest with an immutable revision and any transformation.
2. Regenerate the affected assets using the vendor tools. Install the
   [pinned oracle dependencies](../tools/golden-oracle/README.md) first.
3. For grammar changes, regenerate detection metadata and review collisions in
   [catalog.rs](../src/grammars/catalog.rs). Preserve deliberately unresolved
   optional includes recorded in the source manifest: resolving an extra
   dependency can change output relative to the reference host.
4. Regenerate the grammar bundle:

   ```sh
   cargo run --locked --bin syntaxmate-bundle --features bundle-tools --
   cargo run --locked --bin syntaxmate-bundle --features bundle-tools -- --check
   ```

5. Regenerate the affected [scope fixtures](../tests/fixtures/textmate/README.md)
   and [theme goldens](../tools/golden-oracle/README.md#themes), then review exact
   scope/style diffs. Edit source inputs, not generated JSONL or bundle bytes.
6. Record user-visible output changes in the changelog. Run the generated-file,
   golden, provenance, performance, and package checks in
   [CI](../.github/workflows/ci.yml). A source-package pin update requires review
   of the full affected import, not just one grammar.

## Adding a public language

Use a real, licensed grammar with distinct detection and tested scope behavior.
Framework names or new versions of an existing language do not by themselves
justify another public ID. Add aliases/extensions/basenames and explicit
collision ownership; dependency-only grammars stay private.

Add basic/stress fixtures and regex conformance cases for any newly introduced
constructs. Promotion requires exact oracle parity, complete native output,
resolved required dependencies, and a passing reference performance sweep.
The enforced contract lives in
[validation-policy.json](../benchmarks/textmate/validation-policy.json) and
[textmate_validation.py](../tools/textmate_validation.py).

After intentional membership changes, review the independent count/identity
locks and [golden scale policy](../tools/textmate-golden-scale-policy.json),
record an explicit promotion date in the
[promotion ledger](../benchmarks/textmate/language-promotions.json), rebuild
corpora, and persist a [reference sweep](../benchmarks/textmate/README.md#catalog-performance).
Then regenerate [language status](language-status.md) and its summary snippets.
Regeneration must not conceal a missing fixture, lost language, or new divergence.
