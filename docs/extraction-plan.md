# Syntaxmate extraction plan

Status: implementation in progress. The independent repository and package are
bootstrapped; crates.io publication and Mark's registry-dependency migration
remain release-gated follow-up work.

Syntaxmate will be an independent, batteries-included Rust syntax-highlighting
library built around Mark's Rust-native TextMate implementation. It will have
its own repository, package, CI, releases, documentation, and compatibility
policy. Mark will be an ordinary downstream crates.io consumer.

The public crate name is **`syntaxmate`**.

## Decisions

These decisions are fixed for the extraction unless implementation evidence
requires an explicit revision to this document.

1. **Syntaxmate is independent of Mark.** It has no `mark-*` dependencies,
   `MARK_*` environment variables, Mark configuration types, workspace-relative
   assets, or privileged integration API.
2. **There is one normal dependency for users.** The `syntaxmate` crate includes
   the engine, language detection, bundled TextMate grammars, TextMate theme
   parsing, and bundled themes. Internal modules and Cargo features provide
   modularity without requiring users to assemble an engine and asset pack.
3. **Batteries are included by default.** A default build supports custom and
   bundled grammars/themes. `default-features = false` provides a smaller
   custom-assets-only build.
4. **Mark uses the public API.** The final Mark manifest contains a normal
   versioned registry dependency, not a path, Git, workspace, or permanent
   `[patch]` dependency.
5. **TextMate scopes are the primary output.** Mark's coarse `SyntaxClass`, TUI
   styles, worker queues, configuration, and storage remain Mark concerns.
6. **Correctness remains oracle-driven.** The pinned `vscode-textmate` and
   `vscode-oniguruma` development oracle, checked-in goldens, grammar corpus,
   and performance policies move with the engine.
7. **The release crate is offline and Node-free.** Node remains a development
   tool for asset import and oracle regeneration only.
8. **The first release is `0.1.0`.** Its public API is intentionally narrow;
   implementation modules are private so engine optimization does not become a
   semver break.

## Product scope

Syntaxmate should serve the same broad role as Syntect or Shiki for projects
that want TextMate grammars and themes:

- complete-source highlighting;
- incremental line highlighting with continuation state;
- checkpointed or viewport highlighting for editors;
- exact TextMate scope stacks;
- generic resolved colors and font modifiers;
- built-in language aliases and path detection;
- custom JSON grammar and theme loading;
- a curated, versioned grammar and theme catalog;
- safe degradation under explicit work and memory limits;
- diagnostics for compatibility and performance investigations.

Initial non-goals are semantic tokens, parser-based highlighting, network asset
installation, Mark or Ratatui integration, asynchronous APIs, plist grammar
input, stable serialization of tokenizer state, and `no_std` support. ANSI and
HTML render helpers can follow the extraction; the initial output is generic
styled spans rather than framework-specific rendering.

## Repository and package layout

The independent Syntaxmate repository should begin with one publishable crate:

```text
syntaxmate/
  Cargo.toml
  README.md
  CHANGELOG.md
  LICENSE
  THIRD_PARTY_LICENSES.md
  src/
    lib.rs                 # documented facade and re-exports
    highlighter.rs         # batteries-included whole-document API
    session.rs             # incremental line API
    catalog.rs             # language IDs, aliases, and path detection
    grammar/               # private grammar model and JSON compiler
    regex/                 # private native TextMate regex implementation
    tokenizer/             # private stack machine and caches
    scopes.rs              # public result-owned scope table
    theme/                 # public theme types; private selector machinery
    assets/                # public bundled-catalog access
    diagnostics.rs         # feature-gated counters and reports
  assets/
    grammars.bundle        # deterministic, compressed, committed artifact
    themes.bundle          # deterministic, compressed, committed artifact
  examples/
    basic.rs
    detect_path.rs
    incremental.rs
    custom_grammar.rs
    custom_theme.rs
  tests/                   # focused public-API tests suitable for packaging
  test-data/               # large repository-only oracle corpus
  tools/                   # asset generation and pinned Node oracle
  benches/                 # engine, catalog, and theme benchmarks
  docs/                    # compatibility, assets, security, and performance
```

The published package must include only files required to compile, document,
and demonstrate the crate. The large oracle corpus and generation tools remain
in the repository but are excluded from the `.crate` archive.

Proposed feature policy:

```toml
[features]
default = ["bundled-grammars", "bundled-themes"]
bundled-grammars = []
bundled-themes = []
diagnostics = []
```

Features must be additive. Every default/no-default/all-features combination is
compiled in CI. Features for ANSI or HTML output should be added only after the
core API is stable enough to avoid making renderer choices part of extraction.

## Public API target

The API should have an easy layer and an advanced layer. Names below are a
contract sketch, not a requirement to preserve current Mark names.

### Easy whole-document API

```rust
use syntaxmate::Highlighter;

let highlighter = Highlighter::bundled()?;
let document = highlighter.highlight("rust", source, "github-dark")?;

for line in document.lines() {
    for span in line.spans() {
        println!("{:?} {:?}", span.range(), span.style());
    }
}
```

Path detection should be equally direct:

```rust
let document = highlighter.highlight_path(
    "src/main.rs",
    source,
    "github-dark",
)?;
```

### Incremental API

```rust
let mut session = highlighter.session("rust", "github-dark")?;
for line in source.lines() {
    let highlighted = session.highlight_line(line)?;
}
```

The line contract must state whether the caller supplies line terminators.
Syntaxmate should expose one unambiguous normal API and keep any raw
terminator-aware operation advanced or private.

### Advanced API

Advanced consumers need access to neutral abstractions, not implementation
internals:

- `Catalog` and `CatalogBuilder`;
- opaque `LanguageId`, `GrammarId`, and `ThemeId` values;
- `Tokenizer` and cloneable opaque `TokenizerState`;
- `TokenizerOptions` and resource limits;
- `TokenizedLine`, `HighlightedLine`, and `HighlightedDocument`;
- `ScopeStackId` and a result-owned `ScopeTable`;
- `TextMateTheme`, `ResolvedStyle`, `RgbColor`, and modifiers;
- typed grammar, theme, catalog, and tokenization errors;
- optional diagnostics and counters.

Regex ASTs, VM/DFA types, compiled rules, caches, raw numeric IDs, internal
state IDs, and bundle section structures are not public API. Internal tests may
exercise private modules; integration tests must not force implementation
visibility.

### Output and failure semantics

- Ranges are UTF-8 byte ranges and are always valid boundaries.
- Per-line ranges are explicitly documented as line-relative; document helpers
  may additionally expose source-relative offsets.
- Scope stacks are interned in a result-owned table so tokens do not allocate a
  `Vec<String>` each.
- A tokenizer state is tied to the engine/catalog identity that created it.
  Reusing it with an unrelated tokenizer must be impossible or return an error.
- Invalid user grammar/theme data returns typed errors and must not panic.
- Budget exhaustion remains recoverable, but degradation is observable through
  a status or diagnostic value rather than silently appearing as successful
  complete highlighting.
- Error enums are `#[non_exhaustive]`, implement `std::error::Error`, and retain
  useful source/path/scope context.

## Ownership split

### Move to Syntaxmate

- `crates/mark-syntax/src/engine/**`;
- the generic scope table, token, line fingerprint, highlighted-line/document,
  and tokenizer-limit concepts currently mixed into `types.rs`;
- generic TextMate JSON theme parsing and selector resolution from
  `theme/mod.rs`;
- grammar bundle parsing, catalog metadata, language aliases, extensions,
  basenames, dependency closure loading, and license records;
- `assets/grammars/` and transferable TextMate theme assets;
- `crates/mark-syntax/tests/fixtures/textmate/` and the golden harness;
- TextMate oracle, asset-vendoring, conformance, corpus, and status tools;
- TextMate-specific examples, benchmarks, performance evidence, and
  compatibility documentation.

### Keep in Mark

- syntax enable/disable configuration and `syntax.json` storage;
- config paths, migration behavior, and CLI commands;
- TUI worker queues, priorities, prefetching, and file/hunk caches;
- source extraction from Git revisions and diff line mapping;
- Mark-specific size, queue, worker, and notification settings;
- `SyntaxClass` fallback classification if Mark still needs it;
- Ratatui style conversion and full-TUI theme integration;
- Mark's adapter types and user-facing syntax settings.

After migration, `mark-syntax` becomes a relatively small product adapter over
Syntaxmate. It may preserve Mark's existing API while converting Syntaxmate
scope/style output into Mark rendering data.

## Required engine decoupling

The extraction is not complete after copying files. Syntaxmate must remove the
following product assumptions.

### Errors and options

- Replace `mark_core::{MarkError, MarkResult}` with Syntaxmate-owned errors.
- Split engine limits from Mark's queue/cache/worker `SyntaxLimits`.
- Put grammar size, rule count, include depth, regex work, line work, and cache
  limits in documented neutral option types.

### Environment and global state

- Remove all `MARK_TEXTMATE_*` reads from the library.
- Convert useful tuning switches into builder options or feature-gated
  diagnostics.
- Move process-global grammar, pattern, frame, and state caches behind an owned,
  cloneable engine handle with bounded storage.
- Make independent engine instances deterministic and isolated.
- Preserve sharing across Mark syntax workers by cloning an `Arc`-backed public
  engine/highlighter handle, not by relying on hidden global state.

### Mark output coupling

- Remove `SyntaxClass` from tokenizer output. Offer exact scopes and, if useful,
  a generic opt-in scope classifier utility.
- Replace `HighlightedText` internals that encode Mark theme-cache assumptions
  with neutral scope and style result types.
- Keep the efficient interned scope representation and theme-generation cache,
  but expose them through neutral names and behavior.

### Assets and builds

- Do not traverse parent directories from `build.rs`.
- Generate deterministic compressed bundles ahead of publication and commit
  them in the Syntaxmate repository.
- Verify generated bundles against source assets and checksums in CI.
- Make normal `cargo build` offline, deterministic, and independent of Node.
- Expose crate, catalog, grammar-source, and bundle versions/hashes for bug
  reports and reproducible output.

## Migration phases

### Phase 0: freeze and baseline

1. Record the exact Mark commit used as the extraction baseline.
2. Run and archive the full golden, theme, conformance, catalog, package-size,
   and performance baselines.
3. Inventory source files, generated artifacts, upstream pins, transformations,
   and third-party licenses.
4. Announce a short engine/asset freeze while repository ownership moves. Any
   urgent fix lands in Syntaxmate first and is backported deliberately.

Exit gate: the baseline can be reproduced from a clean Mark checkout.

### Phase 1: bootstrap the independent repository

1. Create the Syntaxmate repository, preferably preserving relevant Git history
   with a filtered-history import; otherwise record the source commit and
   attribution explicitly.
2. Copy the engine, generic themes, grammar assets, tests, tools, benchmarks,
   and documentation without intentional behavior changes.
3. Make the copied crate compile independently before redesigning APIs.
4. Move the large fixture corpus outside the publishable package contents.

Exit gate: Syntaxmate's internal test suite produces byte-identical scope and
style streams to the extraction baseline.

### Phase 2: establish the neutral core

1. Add Syntaxmate-owned errors and engine options.
2. Introduce an owned bounded engine/cache context.
3. Remove Mark environment variables, paths, settings, and output types.
4. Make all implementation modules private and expose the narrow advanced API.
5. Add state ownership validation and observable degradation.
6. Keep a temporary internal compatibility adapter only while tests migrate.

Exit gate: `rg -i '\bmark(_|-|::)'` over release source and public docs finds no
product coupling except historical attribution.

### Phase 3: build the batteries-included facade

1. Add `Highlighter::bundled`, `highlight`, `highlight_path`, and incremental
   session APIs.
2. Integrate language detection, grammar dependency closure loading, theme
   resolution, and styled-span output.
3. Support custom grammar/theme registries through builders.
4. Add basic, detection, incremental, custom grammar, and custom theme examples.
5. Test default/no-default/all-feature builds.

Exit gate: a new external project can highlight a detected file with a bundled
theme using only `syntaxmate = "0.1"` and the documented easy API.

### Phase 4: harden correctness and resource behavior

1. Retarget all golden and theme parity tests to the public API where practical.
2. Prove full-document, line-incremental, and checkpoint/viewport equivalence.
3. Add property tests for ordered non-overlapping UTF-8 ranges and valid scope
   references.
4. Add fuzz targets for grammar JSON, regex parsing/matching, tokenizer input,
   bundle parsing, and theme parsing/selection.
5. Audit every user-controlled path for panic, unbounded allocation, recursion,
   and work limits.
6. Document the supported Oniguruma/TextMate surface and known limitations.

Exit gate: malformed public input is error-returning or safely degraded, fuzz
runs are clean, and committed fixtures have no unreviewed divergence.

### Phase 5: package and release Syntaxmate

1. Complete package metadata, README, rustdoc, changelog, MSRV policy, security
   policy, and third-party notices.
2. Add stable/latest and MSRV checks, default-feature matrix checks, and native
   Linux/macOS/Windows coverage.
3. Run `cargo package`, unpack the resulting `.crate`, and build it in a clean
   directory without repository files.
4. Build separate external fixture projects against the packaged artifact for
   the easy, advanced, default-features-disabled, and custom-assets APIs.
5. Verify the compressed package and bundled binary-size budgets.
6. Publish `syntaxmate 0.1.0` only after the exact release commit passes all
   gates.

Exit gate: `cargo add syntaxmate` works in a clean project and requires no
undeclared files, tools, or network access during compilation.

### Phase 6: migrate Mark as a downstream consumer

1. Add `syntaxmate = "0.1"` to Mark. Temporary local development may use
   `[patch.crates-io]`, but no patch is committed in the completed migration.
2. Implement the small `mark-syntax` adapter using only documented public API.
3. During the transition, run old and new engines over the committed corpus and
   compare exact scopes, styles, degradation, and relevant counters.
4. Re-run Mark's TUI performance, memory, worker concurrency, theme, and
   full-file tests.
5. Remove duplicated engine code, grammar/theme assets, build logic, oracle
   tooling, and large fixture corpus from Mark.
6. Update Mark documentation and generated language status to report the
   Syntaxmate crate version and catalog bundle hash.

Exit gate: Mark's manifest uses the registry release, Mark contains no private
copy of the engine/assets, and Mark passes its complete CI and performance
suite.

### Phase 7: establish independent maintenance

1. Move grammar/theme updates, parity issues, and engine optimization work to
   Syntaxmate.
2. Add `cargo-semver-checks` after the first public release.
3. Define release semantics: API-compatible engine fixes may be patches;
   catalog refreshes and intended highlighting changes are minor releases.
4. Update Mark through normal dependency-update pull requests with reviewed
   output, docs, and performance changes.
5. Invite at least one non-Mark pilot integration before considering a 1.0 API.

## Quality gates

### Correctness

- Pinned `vscode-textmate` and `vscode-oniguruma` oracle versions.
- Exact scope-stack and theme-style goldens for the committed catalog corpus.
- No silent `stoppedEarly` or budget degradation in promoted fixtures.
- Full-source, incremental, and viewport/checkpoint equivalence.
- Unicode, astral-plane, combining-mark, CRLF, empty-file, final-empty-line,
  long-line, injection, external include, capture, dynamic-end, and
  begin/while coverage.

### Safety and resilience

- No unsafe code in release engine modules unless separately justified,
  reviewed, and tested.
- No panic on malformed public grammar, theme, source, or bundle input.
- Bounded regex fallback, include depth, tokenizer steps, dynamic matcher
  growth, scope/state interning, and caches.
- Poisoned synchronization primitives recover or return errors rather than
  turning user input into a process-wide failure.
- Fuzz corpus and regression seeds are retained.

### Performance

Track at least:

- package and embedded bundle size;
- compile time and release binary contribution;
- cold grammar closure load and first-line latency;
- warm whole-document throughput;
- incremental line latency and checkpoint replay;
- allocations per line/token and scope-table memory;
- shared-cache behavior under parallel tokenizers;
- worst-case fallback-regex work and degradation.

The initial extraction must preserve Mark's measured baseline before pursuing
new optimization. Machine-sensitive thresholds remain scheduled/reference-run
checks; deterministic counters and output checks remain merge gates.

### Publication

- Complete rustdoc for every public item; no broken intra-doc links or warnings.
- Explicit package contents; the current large golden corpus is not shipped.
- All dependency versions and licenses are publishable.
- `THIRD_PARTY_LICENSES.md` covers every bundled grammar and theme source.
- The packaged artifact builds offline in isolation.
- The crate description makes the compatibility boundary clear and does not
  imply affiliation with TextMate, VS Code, Syntect, or Shiki.

## Mark integration contract

Mark must be treated exactly like another application:

```toml
[dependencies]
syntaxmate = "0.1"
```

Mark may choose features explicitly, but it receives no private Cargo feature,
friend API, internal module visibility, or release ordering exception. A change
needed only by Mark must still be justified as a coherent public API useful to
other applications, or remain in Mark's adapter.

Mark's lockfile pins the exact resolved Syntaxmate release. Dependency updates
are reviewed like other externally observable syntax changes. Mark should
include the Syntaxmate crate version and bundle hash in syntax diagnostics so
users can file reproducible reports against the correct project.

## Extraction definition of done

The project is extracted when all of the following are true:

- `syntaxmate 0.1.0` is independently published and documented;
- a clean third-party project can highlight bundled and custom languages;
- Syntaxmate owns the engine, generic theme implementation, catalog, assets,
  oracle, conformance tests, and performance policy;
- Syntaxmate release source contains no Mark dependencies or product behavior;
- Mark depends on the crates.io release using only public API;
- Mark no longer contains a duplicate engine, grammar/theme bundle, or oracle
  corpus;
- Mark's exact syntax output and accepted performance baseline are preserved;
- package, license, MSRV, cross-platform, fuzz, and semver gates are active;
- future Syntaxmate and Mark releases can occur independently.
