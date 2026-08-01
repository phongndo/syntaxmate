# TextMate theme engine performance

Reference measurement: Apple Silicon/macOS, 2026-07-13, release profile, Rust
stress fixture, 20 process-cold iterations. Baseline is repository `HEAD`
before scope-stack transport; candidate is the exact-theme implementation.

| Measurement | Baseline | Exact engine | Change |
|---|---:|---:|---:|
| Process-cold highlighting throughput | 5.00 MB/s | 4.82 MB/s | -3.6% |
| Peak RSS | 88.39 MB | 88.31 MB | -0.1% |
| Release `mark` binary | 6,000,336 bytes | 6,598,592 bytes | +10.0% |

Command:

```sh
cargo run -p syntaxmate --release --example profile-cold -- \
  --mode process-cold \
  --assets assets/grammars/languages \
  --scope source.rust \
  tests/fixtures/textmate/rust/stress.rs 20
```

The resolved-style cache is stored per shared scope table. After the first
resolution of a stack/theme pair, rendering performs no string or heap
allocation for theme matching. Candidate rules are indexed by the first scope
component rather than scanning the complete theme. Scope-table and style-cache
bytes are included in syntax cache accounting.

The larger `syntax-large-rust` TUI corpus measures cold render, repeated random
viewport render, cache behavior, and RSS. Exact rendering changed the p50 of
the per-sample random-scroll maximum from 252 µs to 261 µs (+3.6%), and peak
RSS by +5.9%. Instrumented warm resolution recorded 863,772 hits and 30 misses
(99.9965%). The scope table occupied 2,388 bytes for 32 unique stacks in the
cached viewport sources. All values and policy gates are recorded in
`benchmarks/textmate/theme-performance.json`; run
`python3 tools/check-theme-performance.py` for the local gate.

Theme-cache counters are compiled only with the `diagnostics` feature, so
default production rendering does not pay an atomic-counter cost per segment.

The 200-iteration line-cold run measured 11.37 MB/s before scope transport and
10.52 MB/s after it (-7.48%). Exact output contains 2,125 segments versus 1,954
coarse segments (+8.75%) because differently scoped adjacent text can no longer
be merged. The report therefore records an explicit 10% local
warm-tokenization exception. The measured cost remains below the additional
exact output volume and preserves required token boundaries rather than
concealing them.
