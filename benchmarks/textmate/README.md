# TextMate engine benchmarks

The benchmark modes deliberately separate grammar/parser setup from full-file
highlighting and never serialize tokens inside the timed interval.

## Native Syntaxmate

```sh
cargo build --release -p syntaxmate --example profile-cold
target/release/examples/profile-cold \
  --mode process-cold \
  --assets assets/grammars/languages \
  --scope source.rust \
  tests/fixtures/textmate/rust/stress.rs 1
```

Run the command in a fresh process for each process-cold sample. The driver
loads only the requested grammar's transitive external-include closure.

## Allocation and live-memory guardrails

`profile-alloc` exposes a stable JSON protocol for first/warm whole-document,
incremental replay, incremental highlighting, and prepared-language phases. It
records allocation calls, cumulative bytes, boundary retention, peak additional
live bytes, and token/scope-stream digests without serializing output in the
timed API intervals.

```sh
cargo build --release --example profile-alloc --locked
python3 tools/check-allocation-performance.py \
  --write-report target/textmate-performance/allocation-report.json
```

The checker validates the four fixed corpus paths, sizes, and SHA-256 digests in
`allocation-policy.json`, enforces every phase's allocation-call,
cumulative-byte, and peak-live-byte ceilings, and reports nearest-rank p50/p95
values per KiB. Elapsed time is
reported but not gated on variable shared CI runners.

## Pinned standalone vscode-textmate

```sh
npm install --prefix tools/golden-oracle
node tools/textmate-bench.mjs \
  --mode process-cold \
  --assets assets/grammars/languages \
  --scope source.rust \
  --file tests/fixtures/textmate/rust/stress.rs \
  --iterations 1 --json
```

Use `--mode same-driver --iterations 3` for repeated passes after one setup.

## Reproducible engine comparison

Compare Syntaxmate and the pinned VS Code implementation on identical grammar
assets and source fixtures, excluding each driver's setup phase:

```sh
npm ci --prefix tools/golden-oracle
RUSTUP_TOOLCHAIN=1.88.0 python3 tools/compare-textmate-performance.py \
  --iterations 5 \
  --out target/textmate-performance/comparison.json
```

The committed reference result is `engine-comparison.json`; use a `target/`
path for ad-hoc runs and replace the reference only after reviewing the complete
environment and per-language output. The report records raw elapsed time,
processed bytes, token counts, and the runtime environment. Token counts may
differ and are never treated as a speed score. Syntect uses Sublime syntax
definitions rather than the same TextMate JSON assets, so it is intentionally
excluded from this like-for-like report; any separate Syntect measurement must
be labeled as a different-grammar comparison.

## Quality oracle

```sh
node tools/golden-dump.mjs \
  --assets assets/grammars/languages \
  --scope text.html.markdown \
  --file benchmarks/textmate/corpora/markdown-embedded-private.md \
  --out /tmp/oracle.jsonl

SYNTAXMATE_STRICT=1 cargo test --all-features \
  textmate_golden::manifest_golden_cases_match_or_are_allowlisted
```

Report throughput together with each engine's emitted segment/token count;
token counts differ between engines and are not directly comparable.
