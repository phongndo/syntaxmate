# Competitive benchmarks

Two tracks answer different questions:

- **Engine:** Syntaxmate versus `vscode-textmate`, with identical grammar assets
  and source fixtures. Setup is excluded; normalized UTF-8 scope-stream digests
  must match.
- **End to end:** Syntaxmate, Shiki, and Syntect using each product's bundled
  syntax, theme, highlighting, and HTML API. This measures the product
  experience, not identical grammar behavior.

## Reference evidence

[results-2026-08-04.json](results-2026-08-04.json) contains the historical
reference environment, versions, language sets, per-corpus p50/p95 latencies,
aggregates, and output digests. It measures that revision on that machine, not
the current checkout. [Report tests](../../tools/test_competitive_benchmarks.py)
check its accounting, completeness, pins, and digest consistency.

Benchmark dependencies are pinned separately in [package.json](package.json)
and the [Syntect driver](syntect-driver/Cargo.toml), with their lockfiles. These
pins are independent of the development correctness oracle.

## Reproduce

Run from the repository root with Cargo and Node available:

```sh
python3 tools/run-competitive-benchmarks.py \
  --samples 7 --minimum-time-ms 100 --include-samples \
  --out target/competitive-benchmarks.json
```

The runner installs pinned benchmark dependencies, builds optimized drivers,
rotates engine order across separate processes, and calibrates warm iterations.
It validates source-byte accounting, completion, sample counts, output
determinism, and engine scope-digest equality before writing the report.
`--include-samples` retains raw records for review. Native product profiling
also supports ANSI via `profile-product --renderer ansi`; the comparative
product report uses HTML.

## Interpretation

- Process-cold includes process launch and runtime startup. Steady/replay timing
  excludes setup.
- Steady matching disables Syntaxmate's line-result cache. Unchanged-document
  replay permits it; the direct `vscode-textmate` driver has no equivalent
  cache. That replay speedup is not raw regex-engine speed.
- Applications can add caching above any of these libraries.
- Syntect's Sublime grammars differ from TextMate JSON grammars. The product
  track cannot establish scope or HTML equivalence.
- Different products emit different output sizes and token counts. Counts are
  accounting checks, not quality scores.
- Inspect per-corpus results and regressions before describing an aggregate.
  Results apply to the recorded machine, revisions, and workloads only.
