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

The string table and scope-ID table are uncompressed. The runtime borrows their
embedded bytes, including the string offset index, without allocating per string
or decompressing metadata at first use. The reader validates table bounds,
UTF-8, string boundaries, and scope IDs before exposing them. Tool parsing of
non-static input owns each table in one buffer. Repository-walk skeletons also
remain uncompressed so the runtime can borrow their embedded bytes.

The private v4 format was redefined before release to remove metadata compression,
prioritizing cold-start latency and retained heap over bundle and binary size.

The bundle uses independently compressed compiled grammars and records each
grammar's dependency closure, so a tokenizer decodes a closure member only when
it needs that grammar. Its format is private; version and validation rules live
in the [container decoder](../src/grammars/bundle.rs), the
[closure analysis](../src/engine/grammar_closure.rs), and the
[grammar IR codec](../src/engine/grammar_ir.rs). Changing any of them requires
regenerating the bundle. Custom grammars use the JSON
compiler. Release builds consume committed assets without running Node or
fetching upstream sources.

## Custom and subset bundles

Build subsets from a checkout using the [bundle builder](../tools/build-bundle.rs):

```sh
cargo run --locked --bin syntaxmate-bundle --features bundle-tools -- \
  --languages rust,toml,json --out grammars-subset.bundle
```

`--languages` accepts comma-separated public IDs or aliases. The output exposes
only those languages, and includes their transitive dependencies, private
grammars, and corresponding license records. Ordering and duplicate arguments
do not change the output. With `--languages`, the default output is
`grammars-subset.bundle`; without it, the builder updates the complete embedded
bundle. `--check` compares the selected output with a deterministic rebuild.
Build from the same Syntaxmate version that consumes the bundle.

Disable `bundled-grammars` to omit the embedded bundle entirely:

```toml
syntaxmate = { version = "0.2", default-features = false }
```

```rust,ignore
use syntaxmate::{Catalog, Highlighter};

let catalog = Catalog::from_static(include_bytes!("grammars-subset.bundle"))?;
let highlighter = Highlighter::new(&catalog);
let tokens = highlighter.tokenize("rust", "fn main() {}")?;
```

`Catalog::from_static` borrows uncompressed tables from static storage;
`Catalog::from_bytes(&bytes)` copies retained data and lets the caller release
its buffer. Both validate grammar IR before returning. A separate `Vec`/`Arc`
constructor would not avoid the decoder's per-section ownership, so the borrowed
slice constructor covers runtime-loaded bundles without another ownership API.
Use `Catalog::licenses()` to retrieve the included notices for distribution.
Custom JSON grammars through `GrammarRegistry` remain available too.

Catalog queries borrow metadata; keep the catalog alive while using their
results. `language(id_or_alias)` exposes IDs, aliases, suffixes, basenames, and
root scopes. See [`Catalog::detect`](../src/catalog.rs) for combined filename
and bounded first-line detection, including precedence and supported modelines.

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
