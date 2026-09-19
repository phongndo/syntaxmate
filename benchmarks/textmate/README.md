# Engine measurement and guardrails

Run commands from the repository root. Use identical source and grammar inputs
for engine comparisons. Record commit IDs, toolchain, machine, corpus hashes,
and timing boundaries. Compare release builds in alternating-order, separate
processes; report sample distributions rather than a single best run.

Require complete output and matching scope/range or rendered-byte digests before
calling a change a speedup. Separate construction, first use, warm matching,
and cached line-result replay. Report regressions and correctness tradeoffs;
raising a budget or removing a slow corpus does not establish an improvement.

## Engine timing

```sh
cargo build --release --locked --example profile-cold
target/release/examples/profile-cold \
  --mode process-cold --assets assets/grammars/languages \
  --scope source.rust tests/fixtures/textmate/rust/stress.rs 1
```

For an identical-assets comparison with the pinned development oracle:

```sh
npm ci --prefix tools/golden-oracle
python3 tools/compare-textmate-performance.py \
  --iterations 5 --out target/textmate-performance/comparison.json
```

This focused smoke comparison excludes driver setup. The separate-process
first/steady/replay engine track and end-to-end product comparison are described
in [competitive benchmarks](../competitors/README.md). Syntect uses different
grammars and belongs only in the product comparison.

## Allocation and retained memory

```sh
cargo run --release --locked --example profile-alloc -- \
  --json --no-line-cache rust tests/fixtures/textmate/rust/stress.rs
cargo build --release --locked --example profile-alloc
python3 tools/check-allocation-performance.py \
  --write-report target/textmate-performance/allocation-report.json
```

[profile-alloc](../../examples/profile-alloc.rs) measures construction,
whole-document and incremental first/warm phases, styling, and prepared-language
creation/reuse. It reports allocation/reallocation calls, cumulative bytes,
boundary retention, peak additional live bytes, completion, and output digests.
Default warm runs can reuse cached line tokens; `--no-line-cache` forces matching
again. Digest work stays outside timed API intervals.

The counting allocator changes costs. Use uninstrumented engine/product drivers
for latency claims and report allocation measurements separately.
[profile-prepared](../../examples/profile-prepared.rs) compares independent
sessions in `direct`, `prepared-total`, and `prepared-reuse` modes, also with a
counting allocator.

[allocation-policy.json](allocation-policy.json) pins corpora and reviewed
per-phase memory/call ceilings. The checker rejects stale inputs, degraded
output, digest drift, and ceiling breaches. Raising a ceiling requires new
profile evidence and review. Elapsed time is informational on shared runners.

## Catalog performance

```sh
python3 tools/build-textmate-corpora.py --check
python3 tools/check-textmate-catalog-performance.py
```

The default command applies reference-machine floors; `--ci` selects the
separate shared-runner policy. After an intentional reference measurement:

```sh
python3 tools/check-textmate-catalog-performance.py --iterations 3 --write-report
python3 tools/generate-language-status.py
python3 tools/check-language-docs.py --write
```

This replaces [catalog-performance.json](catalog-performance.json); review it
before committing. Policy and corpus identity are defined in
[validation-policy.json](validation-policy.json) and [corpora.toml](corpora.toml).
`core-repeated` and `catalog-repeated` are fixed comparison inputs. Only
`representative-markdown` deliberately follows current project documentation;
its generated manifest records the changed input hash.

## Prior experiments

Revisit rejected approaches only with a different design and fresh evidence:

- Independent per-pattern next-match memoization lost the unified scanner's
  lazy grammar-order search.
- Position-only recursive subroutines lost observable captures.
- Fixed-byte case-folded lookbehind bounds missed Unicode folds across UTF-8
  widths. The correctness repair has a measured SDBL cost; preserve the
  regression cases while investigating it.
- Larger execution budgets traded latency for fewer misses without repairing
  the underlying execution cost.

Historical measurements and additional rejected experiments remain in the
[0.1.3 performance roadmap](https://github.com/phongndo/syntaxmate/blob/v0.1.3/docs/performance-roadmap.md)
and [correctness pass report](https://github.com/phongndo/syntaxmate/blob/v0.1.3/docs/correctness-performance-pass.md).
They describe those revisions, not current performance. Keep exploratory
profiles and raw logs under `target/`; publish durable evidence with the relevant
change rather than adding another current-status document.
