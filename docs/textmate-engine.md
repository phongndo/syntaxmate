# In-house TextMate engine

Status document for Syntaxmate's native TextMate engine. The engine, grammar
bundle, oracle harness, and full public catalog are maintained in this
repository. The checked-in fixture corpus passes exact scope-stack parity
without divergence allowlists.

The historical extraction boundary and completed ownership transfer are
recorded in [`extraction-plan.md`](extraction-plan.md).

## Goals

- One syntax engine, one grammar catalog, one source of truth for detection.
- Correctness measured against a pinned `vscode-textmate` + `vscode-oniguruma`
  oracle (dev-only Node tooling), not against the old hybrid.
- Coverage is an **asset** problem: add or drop real grammars; never map a
  language onto another language's hand-written lexer.
- Release builds stay offline and **Node-free**. Node is only for regenerating
  goldens / regex conformance during development.

## Architecture

```text
src/
  engine/
    grammar.rs      # rule model (match / begin-end / begin-while, includes, injections)
    regex/          # ordered scanner + budgeted backtracking fallback
    tokenizer.rs    # line-oriented TextMate stack machine and owned caches
    state.rs        # interned parser state
    checkpoint.rs   # viewport replay checkpoints
    line.rs         # line splitting / UTF-8 boundary helpers
  grammars/
    bundle.rs       # deterministic embedded grammar bundle
    registry.rs     # bundled catalog loading and dependency closure
  tokenizer.rs      # custom registry, exact scopes, state, and viewport API
  highlighter.rs    # bundled and custom-theme highlighting API
  catalog.rs        # aliases, extensions, basenames, licenses, and themes
  theme/            # TextMate selector and style resolution

assets/grammars/           # committed TextMate JSON (full public catalog + private deps)
  SOURCE.toml                 # pin: @shikijs/langs@3.23.0
  licenses.json               # per-grammar license manifest
  coverage.toml               # active public/private grammar coverage manifest

tools/                        # dev-only Node oracle (not linked into the binary)
  golden-oracle/              # pinned vscode-textmate@9.2.0, vscode-oniguruma@1.7.0
  golden-dump.mjs             # single-file oracle dumper
  generate-goldens.mjs        # regenerate all cases from cases.toml
  regex-conformance.mjs       # small Oniguruma vs syntaxmate pattern checks
  grammar-stats.mjs           # pattern feature inventory over vendored JSON

tests/
  textmate_golden.rs          # exact + coarse parity harness + property tests
  fixtures/textmate/          # sources, goldens, cases.toml, divergences.toml
```

### Matching strategy (hybrid, Rust-native)

1. Parse and classify grammar patterns into the engine's ordered regular scanner when
   possible, with literal/start-byte/required-literal filtering.
2. Route lookaround / backreferences / `\G`-heavy patterns to an in-house
   backtracker with a hard step budget.
3. Scan all candidates together by input position and grammar order, then
   replay only the winner when captures are required.
4. Keep the VM's common zero/one-state result inline, allocate only for real
   backtracking fanout, and reuse an unchanged candidate set within a line.
5. On budget kill: degrade the affected range, keep the line moving (no hang).

There is no production regex crate or Oniguruma dependency. Hot rules in
TypeScript/C++/Ruby still exercise the fallback path, so fallback cost is a
first-class concern, not a rare escape hatch.

### Public API shape

Syntaxmate exposes engine-independent, exact-scope types rather than product or
renderer-specific classes:

- `Catalog`, `GrammarRegistry`, `Tokenizer`, and `TokenizerState`;
- `TokenizedDocument`, `TokenizedLine`, `TokenSpan`, and scope-table references;
- `CheckpointTable` for bounded viewport replay;
- `Highlighter`, `HighlightSession`, and caller-supplied `Theme` values;
- optional escaped HTML and terminal-safe ANSI rendering.

Regex/compiler internals and cache identities remain private.

## Public language catalog

<!-- BEGIN GENERATED: language-counts -->
Completed generated coverage: **264 supported public language IDs**, **264 validated**, **264 oracle-covered**, and **264 in the catalog stress corpus**. The locked quality contract is 264/264 validated; the deterministic validation policy locks all four counts and the exact catalog identity (SHA-256 of the sorted public-ID list), so regeneration cannot make a lost public-ID basic/stress contract look complete or swap one language for another. See [`language-status.md`](language-status.md) for the generated ledger.
<!-- END GENERATED: language-counts -->

The native catalog is the full pinned Shiki language set plus audited external
grammars vendored under `assets/grammars/languages/`. The active public IDs
are listed in `assets/grammars/coverage.toml`; dependency-only blobs such as
`twig-source`, `yaml-1.2`, and `yaml-embedded` remain hidden from user-facing
language selection. Planned catalog batches are tracked in
[`future-language-support.md`](future-language-support.md).

The original core regression set remains covered by fixtures and includes:

| Language id | Grammar asset | Root scope |
| --- | --- | --- |
| `bash` | `shellscript.tmLanguage.json` | `source.shell` |
| `c` | `c.tmLanguage.json` | `source.c` |
| `cpp` | `cpp.tmLanguage.json` | `source.cpp` |
| `csharp` | `csharp.tmLanguage.json` | `source.cs` |
| `css` | `css.tmLanguage.json` | `source.css` |
| `docker` | `docker.tmLanguage.json` | `source.dockerfile` |
| `go` | `go.tmLanguage.json` | `source.go` |
| `html` | `html.tmLanguage.json` | `text.html.basic` |
| `java` | `java.tmLanguage.json` | `source.java` |
| `javascript` | `javascript.tmLanguage.json` | `source.js` |
| `json` | `json.tmLanguage.json` | `source.json` |
| `jsx` | `jsx.tmLanguage.json` | `source.js.jsx` |
| `kotlin` | `kotlin.tmLanguage.json` | `source.kotlin` |
| `lua` | `lua.tmLanguage.json` | `source.lua` |
| `make` | `make.tmLanguage.json` | `source.makefile` |
| `markdown` | `markdown.tmLanguage.json` | `text.html.markdown` |
| `nix` | `nix.tmLanguage.json` | `source.nix` |
| `php` | `php.tmLanguage.json` | `source.php` |
| `powershell` | `powershell.tmLanguage.json` | `source.powershell` |
| `python` | `python.tmLanguage.json` | `source.python` |
| `ruby` | `ruby.tmLanguage.json` | `source.ruby` |
| `rust` | `rust.tmLanguage.json` | `source.rust` |
| `scss` | `scss.tmLanguage.json` | `source.css.scss` |
| `sql` | `sql.tmLanguage.json` | `source.sql` |
| `swift` | `swift.tmLanguage.json` | `source.swift` |
| `terraform` | `terraform.tmLanguage.json` | `source.hcl.terraform` |
| `toml` | `toml.tmLanguage.json` | `source.toml` |
| `tsx` | `tsx.tmLanguage.json` | `source.tsx` |
| `typescript` | `typescript.tmLanguage.json` | `source.ts` |
| `yaml` | `yaml.tmLanguage.json` | `source.yaml` |

Grammar pin: `@shikijs/langs@3.23.0` (see `assets/grammars/SOURCE.toml`).

The first-class extended fixtures include `zig`, `llvm`, `riscv`, `mipsasm`,
`odin`, `asm`, `mojo`, `ocaml`, and `mlir`; adding more coverage is an asset and
fixture decision, not an engine fork.

Path detection reads the checked-in
`assets/grammars/language-metadata.json` contract, generated
deterministically from the pinned registration aliases and grammar
`fileTypes`, then merges the curated mappings in `catalog.rs`. The contract
covers all 264 public IDs (253 Shiki IDs plus 11 additional public grammars).
Tests compare every alias, extension, and basename exactly with the built
catalog and require all
collisions to have explicit precedence or suppression; ambiguous entries are
never skipped. Generic `.conf` files use `apache`, `.v` files use `verilog`,
and `.js` files use `javascript`. Losing generated extensions are omitted,
while language IDs themselves (such as `bird2`) remain available for explicit
selection.

### Adding or updating a grammar (checklist)

1. Vendor the compact JSON from the pinned package via
   `tools/vendor-shiki-grammars.mjs` (or `[[additional_sources]]` in
   `SOURCE.toml` for non-Shiki grammars like `mlir`), stable key order.
2. `licenses.json` entry from the package's per-grammar license metadata.
3. `coverage.toml` public entry; regenerate `language-metadata.json`, then add
   any intentional detection collision ownership to `catalog.rs`.
4. `tools/grammar-stats.mjs` inventory first — any regex construct not in
   `tools/regex-conformance.mjs`'s proving set gets a conformance case
   **before** the grammar lands.
5. Fixtures + oracle goldens (`stoppedEarly: false`), exact + coarse parity,
   `divergences.toml` stays empty, zero degraded lines.
6. Perf: process-cold stress must meet the per-language floor in
   `benchmarks/textmate/validation-policy.json`; a floor breach gets a counters
   audit + tracked issue, never a silent merge. Add a sweep corpus member so
   the aggregate tracks the language forever.

The catalog performance floor is a **local** quality check — shared CI runners
are too variable for absolute MB/s gates. Run it on a quiescent development
machine before promoting a language or landing engine changes:

```sh
python3 tools/check-textmate-catalog-performance.py
```

After an intentional reference run, persist and publish the measurements with:

```sh
python3 tools/check-textmate-catalog-performance.py --write-report
python3 tools/generate-language-status.py
```

The first command atomically writes
`benchmarks/textmate/catalog-performance.json`. The status generator rejects a
report whose catalog membership, corpus digest, or policy floor is stale. New
validated languages also need an explicit ISO date in
`benchmarks/textmate/language-promotions.json`; dates are never inferred from
the current day. Every current entry is 2026-07-14 because all 264 promotions
were re-locked in that final completed batch, not because the generator assigned
a global rollout date.

Updating the `@shikijs/langs` pin is a full-catalog operation, not an isolated
asset bump: update `SOURCE.toml` and the pinned oracle lockfile, run
`tools/vendor-shiki-grammars.mjs`, regenerate `cases.toml` and every oracle
golden, rebuild the catalog corpora/status ledger, then run the complete golden,
counter, conformance-gap, latency, size, and per-language performance gates.
Grammar and golden diffs are reviewed together; a pin update does not land with
allowlisted or silently stale fixture output.

## Dependency policy

| Layer | Allowed | Forbidden in release |
| --- | --- | --- |
| Product binary (`syntaxmate` / CLI) | Pure Rust; versioned compiled `grammars.bundle`; caller-supplied custom JSON | Node, npm, network at build/runtime for highlighting |
| Dev oracle (`tools/golden-oracle`) | Pinned `vscode-textmate@9.2.0`, `vscode-oniguruma@1.7.0` | Version ranges (`^`/`~`); resolving packages from unrelated trees |
| Tests | Checked-in `.golden.jsonl`, optional Node for regen | Requiring Node in default `cargo test` |

Install oracle deps only when regenerating goldens:

```sh
npm install --prefix tools/golden-oracle
```

The package is `"private": true` and lives outside the Rust workspace.

## Phases 0–5

### Phase 0 — baseline, inventory, guardrails

**Goal.** Freeze measurable behavior and know what the engine must replace.

**Deliverables.**

- Grammar source pin (`SOURCE.toml`) and license manifest.
- Pattern-feature inventory (`tools/grammar-stats.mjs` over vendored JSON).
- Coverage keep/drop notes (`coverage.toml`; active full catalog).
- Golden-token oracle tools and fixture corpus.
- A divergence file that must stay empty while committed fixtures are exact.

**Current state.** Full-catalog assets are vendored. Oracle tools, a checked-in
oracle corpus, and a separate catalog-wide literal smoke/budget gate exist.
Production still does not depend on Node. Current counts are generated in the
public-catalog section rather than repeated here.

### Phase 1 — grammar model and dev loader

**Goal.** Represent enough TextMate semantics to run the tokenizer on raw JSON.

**Deliverables.**

- Rule model: `match`, `begin`/`end`, `begin`/`while`, includes (`$self`,
  `$base`, `#repo`, external scopes), captures, `name`/`contentName`,
  injections / selectors.
- Dev loader for raw `tmLanguage.json` (used by tests and examples).

**Current state.** In-tree under `engine/grammar.rs` and related modules; core
fixture grammars deserialize. Full injection fidelity is still a parity work
item (see limitations).

### Phase 2 — regex translation and matching

**Goal.** Correct hybrid matching with a budgeted fallback.

**Deliverables.**

- Regex AST + feature classification (DFA vs fallback).
- DFA/PikeVM path for supported patterns; fallback VM for lookaround/backrefs.
- Anchor-context handling (`^` / `\A` / `\G` depend on resume position).
- Prefilters (required literals).
- Conformance helper: `tools/regex-conformance.mjs`.

**Current state.** Native regular and fallback matchers, ordered pattern-set
selection, anchor context, prefilters, and budget kills are implemented. The
conformance script covers every inventoried bundled-grammar construct and
variant. Deterministic mutation and sampled replay of real
`vscode-oniguruma` scanner executions extend that proving set; the full
Oniguruma surface remains larger than the committed corpus.

### Phase 3 — tokenizer and exact scope transport

**Goal.** Line tokenization that can pass golden fixtures.

**Deliverables.**

- Stack machine with first-match-wins, while-continuation, captures, zero-width
  guards.
- Cross-grammar includes and injection candidate flattening.
- Exact scope-stack interning and public token/document conversion.
- Exact golden and incremental-state replay harness (`textmate_golden.rs`).

**Current state.** Tokenizer + harness land; all basic, stress, and smoke cases
are exact gates. `divergences.toml` contains no exceptions.

### Phase 4 — grammar compiler and embedded bundle

**Goal.** Deterministic embedded `bundle.bin` with lazy per-language decode.

**Deliverables.**

- `grammar-compile` / `build.rs` pipeline → `MRKB` bundle.
- Registry APIs: available / canonical / detect-from-path.
- License table and bundle version stamp.
- No network/Node in release builds.

**Current state.** `MRKB` version 2 stores individually compressed, versioned
compiled-grammar IR. The runtime decodes only the selected grammar and its
embedding dependencies without parsing bundled JSON or compiling rules. The dev
loader remains the custom caller-supplied JSON path.

### Phase 5 — performance layer

**Goal.** Meet interactive latency without reintroducing hand lexers.

**Deliverables.**

- Interned `StateId`, line cache, checkpoints, candidate/prefilter caches.
- Per-line degradation budgets.
- Instrumentation counters and before/after measurements.

**Current state.** State interning, bounded line/candidate caches, checkpoints,
budgets, feature-gated counters, and lazy per-language tokenizer instances are
implemented behind Syntaxmate's public highlighter and tokenizer APIs.

Current throughput measurements and policies are recorded under
[`benchmarks/textmate/`](../benchmarks/textmate/). The like-for-like engine
comparison uses identical grammar and source assets and excludes setup time.

**Retained optimizations** (each validated with alternating-order,
separate-process A/B runs, paired medians, and byte-exact scope streams):
candidate-index results with deferred ownership, per-line unchanged-state
candidate reuse, pre-sized VM fanout, inline zero/one VM states,
parse-time-resolved subroutine target paths (~41% on libc++), anchored winner
capture replay, capture-observability gating (position-only selection VM for
lookahead/lookbehind with winner-only capture replay), the ordered
alternation/repetition bytecode path with the deterministic C-family
comment-or-space separator instruction, compact backreference capture
layouts, reduced token/capture/stack cloning, and a 512 steps-per-byte
source-wide fallback allowance (128 was too low for valid complex grammars).
`profile-cold` configures production syntax limits so its line-cache behavior
matches the runtime.

Later baseline additions include the word-context start-class gate
(per-pattern AST-derived masks over previous/current word-ness), the
skip-prefix gate for separator-prefixed rules, cached per-line block-comment
checks, and atom-derived ASCII class bitmaps in the bytecode compiler. In
Syntaxmate these proven optimizations are deterministic engine behavior rather
than environment switches. Compiled-pattern, frame, candidate, and dynamic-rule
caches are tokenizer-owned. Immutable repository-binding walks are shared by
clones of a grammar registry, while only immutable bundled assets are
initialized globally. `profile-cold --mode process-cold` constructs a fresh
tokenizer per iteration.

**Reverted experiments — do not retry as-is** (measured deltas in the git
history of this file):

- Per-candidate next-match memoization: independent per-pattern searches were
  2.14× slower than the unified ordered grammar-order scan; any future memo
  must preserve the unified scan's laziness.
- Linear-only bytecode slice (literal/class/dot/anchor/group): neutral-to-
  slower; bytecode must cover ordered alternation/repetition where recursive
  fanout dominates.
- Position-only recursive subroutines: ~30% faster on libc++ but increased
  mismatched C++ lines 1,488 → 1,804; subroutine calls stay capture-aware.
- Routing atomic/possessive patterns through the position VM: 0.8–1.7%
  regressions.
- 10× per-match/per-line budgets: 1.4× slower for only ~5% fewer mismatches —
  quality needs cheaper execution, not bigger budgets.
- Also neutral/slower: leading-word-boundary gates, mandatory-prefix checks,
  compact line-cache keys, possessive-repeat specialization, iterator-based
  lookbehind positions, ASCII class branches, source line pre-counting,
  routing capture-free regexes through the position VM.

### Ongoing validation

The fixture corpus and conformance tools continue to broaden coverage beyond
the original proving set. The unavailable-backend shim has already been removed
from the production path. Performance gates are enforced by the benchmark
policies and CI scripts under `benchmarks/textmate/` and `scripts/ci/`.

## Reproducible oracle commands

```sh
# 1. Install pinned Node oracle (once per machine / lockfile change)
npm install --prefix tools/golden-oracle

# 2. Regenerate every case in the manifest
node tools/generate-goldens.mjs

# 3. Check committed goldens without rewriting
node tools/generate-goldens.mjs --check

# 4. One language id
node tools/generate-goldens.mjs --case typescript
node tools/generate-goldens.mjs --case java

# 5. Ad-hoc dump
node tools/golden-dump.mjs \
  --language rust \
  --scope source.rust \
  --grammar assets/grammars/languages/rust.tmLanguage.json \
  --file tests/fixtures/textmate/rust/basic.rs \
  --out tests/fixtures/textmate/rust/basic.golden.jsonl

# 6. Pattern feature inventory over vendored grammars
node tools/grammar-stats.mjs assets/grammars/languages

# 7. Small regex conformance report (needs cargo + oracle install)
node tools/regex-conformance.mjs
# → target/regex-conformance-phase2.json

# 8. Engine golden harness (when the crate builds)
cargo test -p syntaxmate --lib textmate_golden::manifest_golden_cases_ --locked
```

Oracle record shape (one JSON object per source line):

- `language`, `scopeName`, `file`, `lineNumber`, `line`
- `tokens[]`: UTF-16 `startIndex` / `endIndex` + full `scopes` stack
- `ruleStack` (debug string), `ruleStackHash` (sha256 of that string)
- `stoppedEarly` (must be false for committed goldens; dumper uses time limit 0)

## Fixture corpus policy

Every manifest case is an exact scope-stack + coarse-class gate. `basic` and
`stress` together satisfy the fixture portion of validation; `smoke` alone is
oracle coverage, and additional named cases remain regression cases. The
generated per-language kinds and current counts live in
[`language-status.md`](language-status.md).

Embedded grammars in the manifest (non-exhaustive): markdown→rust/js,
html→js/css, scss→css, php→html/js/css/sql, cpp→cpp-macro.

`bash` is the historical fixture name for the public `shellscript` grammar ID.
There are no remaining validation-only public IDs: every public ID has both
basic and stress exact-contract membership and belongs to the catalog sweep.
`validation-policy.json` locks that completed membership independently of the
generated case, corpus, and status outputs.

## Golden harness scale policy

L0.6 is a policy gate first; CI does not benchmark the full golden suite on
every change. Build the test binary once, run one untimed warmup, then record
wall-clock timings for the configured number of
`cargo test -p syntaxmate --lib textmate_golden::manifest_golden_cases_ --locked` runs on the
documented reference runner. Commit the reviewed timing decision to
`tools/textmate-golden-scale-policy.json` rather than silently changing CI.

<!-- BEGIN GENERATED: golden-scale-policy -->
Static gate: measure at **124 manifest cases**, after **1 warmup** and **5 timed runs**. Keep the suite unsharded at p95 ≤ **60 s**; above that, choose a reviewed count of at most **8 stable language-ID shards** whose maximum p95 is ≤ **45 s**. Final scale is at least **528 cases** for **264 public IDs**. Use nearest-rank p95 on **local development machine (L0.6 baseline)**. Current decision: **544 cases** measured at **62.41 s p95**, above the **60 s** trigger, so CI runs **4 shards**. Measured per-shard p95 (2026-07-14): shard 0 = 17.11 s, shard 1 = 21.76 s, shard 2 = 8.57 s, shard 3 = 13.88 s; maximum **21.76 s** ≤ the **45 s** shard target.
<!-- END GENERATED: golden-scale-policy -->

If sharding is required, assign all cases for one public language to the same
shard: interpret `SHA-256(language_id)` as an unsigned big-endian integer and
take it modulo the shard count. The language ID is derived from the root
grammar asset (`bash` therefore uses its public `shellscript` ID). Candidate
count timings can be recorded when they are available. CI must run every shard;
the unfiltered `--test textmate_golden` command remains the full local/release
gate, and degradation/budget assertions remain enabled in each shard. The
status freshness check validates the decision rules and any recorded result
without running the suite or inventing a measurement itself.

The shard harness uses a zero-based index and requires both variables:

```sh
# One CI shard (index 0 of 4)
SYNTAXMATE_SHARD_INDEX=0 SYNTAXMATE_SHARD_TOTAL=4 \
  cargo test -p syntaxmate --lib textmate_golden::manifest_golden_cases_ --locked

# Full local/release gate (no shard variables)
cargo test -p syntaxmate --lib textmate_golden::manifest_golden_cases_ --locked
```

Partial, malformed, zero-total, and out-of-range configurations fail loudly.
Run `python3 tools/check-language-docs.py --write` after regenerating
`docs/language-status.md`; CI uses `python3 tools/check-language-docs.py
--check`.

The hard completion checks are `node tools/generate-textmate-cases.mjs
--check`, `python3 tools/build-textmate-corpora.py --check`, and `python3
tools/generate-language-status.py --check`. Regenerating all three cannot
normalize a missing public-ID basic/stress pair into a passing state because
each check reads the independent expected counts in `validation-policy.json`.

Do not hand-edit `*.golden.jsonl`. Update sources, then regenerate.

## Known current limitations (honest)

### VS Code screenshots are not a TextMate-only oracle

VS Code normally overlays language-server semantic tokens on TextMate tokens.
For Rust, rust-analyzer can therefore color parameters, local declarations,
fields, and inferred types differently even when the underlying TextMate scope
stream is identical. Inline type hints in a screenshot are another indication
that semantic analysis is active. Syntaxmate promises parity with pinned
`vscode-textmate`, not rust-analyzer semantic-token parity.

For example, the `execute_inner` function in
`engine/regex/bytecode.rs`—including its parameters and `let mut pc` /
`let mut position` declarations—was compared token-by-token and has the exact
same TextMate scopes in Syntaxmate and `vscode-textmate`. A visual difference there
comes from semantic-token overlays and/or theme scope-color mappings, not a
native tokenizer divergence. Visual investigations must first run VS Code with
semantic highlighting disabled, then compare the full scope stacks before
changing tokenizer behavior.

1. **Corpus parity is not universal proof.** The checked-in fixtures are 100%
   exact, but more adversarial and real-world fixtures are still needed.
2. **Broader Oniguruma conformance.** Recursive subroutines, nested classes,
   dynamic ends, lookaround captures, anchors, and backreferences are covered
   by the proving set; the full Oniguruma surface is larger.
3. **Hot fallback path.** Lookaround-heavy rules (especially TS/C++/Ruby) still
   rely on the budgeted backtracker; translator widening may be required if
   fallback dominates hot matches.
4. **Performance evidence remains workload-specific.** Shared-runner and
   reference-machine policies are separate, and comparisons with
   `vscode-textmate` use identical assets. Results are not generalized to
   engines that consume different grammar formats.
5. **Catalog breadth.** The product catalog now follows the full pinned Shiki
   import plus MLIR. Full Shiki parity still relies on smoke/stress fixtures and
   the budget guard; adversarial coverage remains incremental.
6. **Oracle is Node-only.** Correctness regen requires Node ≥ 20 and the pinned
   packages under `tools/golden-oracle`. CI that only runs `cargo test` does not
   re-derive goldens unless a separate job runs `generate-goldens.mjs --check`.
7. **UTF-16 vs UTF-8.** Oracle offsets are UTF-16 (JS); the harness converts to
   UTF-8 byte ranges. Non-ASCII fixtures exist to catch boundary mistakes; any
   new fixture with astral-plane characters should stay in the corpus.
8. **No silent oracle truncation.** `golden-dump.mjs` passes time limit `0` so
    long minified lines are not stopped early by vscode-textmate's wall clock.
    Committed goldens must keep `stoppedEarly: false`.

## Related paths

- Fixtures: `tests/fixtures/textmate/`
- Harness: `tests/textmate_golden.rs`
- Oracle: `tools/golden-dump.mjs`, `tools/generate-goldens.mjs`, `tools/golden-oracle/`
- Assets: `assets/grammars/`
- Engine: `src/engine/`
- Performance evidence: `benchmarks/textmate/`
