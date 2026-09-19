# TextMate golden fixtures

Source fixtures and generated reference output for
[the Rust harness](../../textmate_golden.rs), included as library tests by
[src/lib.rs](../../../src/lib.rs). The
[language ledger](../../../docs/language-status.md) is the per-language report.
Run commands below from the repository root.

<!-- BEGIN GENERATED: language-counts -->
The manifest has **544 cases** covering **264 public language IDs**. Of **264 supported IDs**, **264 are validated** and **264 are in the stress corpus** (`catalog-repeated`). See the [generated ledger](../../../docs/language-status.md) for individual results.
<!-- END GENERATED: language-counts -->

## Inputs and evidence

- [cases.toml](cases.toml): generated fixture/grammar/embedding manifest.
- [cases.config.json](cases.config.json): asset mappings and exceptional cases.
- [divergences.toml](divergences.toml): reviewed output exceptions. Stale entries
  fail the harness; an exception prevents full language validation.
- Language directories: source inputs and generator-owned `*.golden.jsonl`.

The harness compares exact ordered scope stacks after converting oracle UTF-16
positions to UTF-8, plus internal coarse classifications. It separately checks
budget degradation and deterministic incremental replay. Basic/stress size and
promotion requirements live in the
[validation policy](../../../benchmarks/textmate/validation-policy.json).
A smoke case alone is not full validation.

## Run the harness

```sh
cargo test --all-features --locked --lib textmate_golden::
```

CI shards only the manifest-wide loops. The selected shard count and measured
scale decision are in the [scale policy](../../../tools/textmate-golden-scale-policy.json);
[CI](../../../.github/workflows/ci.yml) runs every shard. To reproduce one shard:

```sh
SYNTAXMATE_SHARD_INDEX=0 SYNTAXMATE_SHARD_TOTAL=4 \
  cargo test --all-features --locked --lib textmate_golden::manifest_golden_cases_
```

Both variables are required together; `INDEX` is zero-based and less than
`TOTAL`. All cases for a public language stay together, including aliases such
as fixture ID `bash` for public ID `shellscript`. Unset both variables for a
complete local run.

<!-- BEGIN GENERATED: golden-scale-policy -->
Static gate: measure at **124 manifest cases**, after **1 warmup** and **5 timed runs**. Keep the suite unsharded at p95 ≤ **60 s**; above that, choose a reviewed count of at most **8 stable language-ID shards** whose maximum p95 is ≤ **45 s**. Final scale is at least **528 cases** for **264 public IDs**. Use nearest-rank p95 on **local development machine (L0.6 baseline)**. Current decision: **544 cases** measured at **62.41 s p95**, above the **60 s** trigger, so CI runs **4 shards**. Measured per-shard p95 (2026-07-14): shard 0 = 17.11 s, shard 1 = 21.76 s, shard 2 = 8.57 s, shard 3 = 13.88 s; maximum **21.76 s** ≤ the **45 s** shard target.
<!-- END GENERATED: golden-scale-policy -->

## Update fixtures

Install the [pinned oracle](../../../tools/golden-oracle/README.md), edit source
fixtures, then regenerate and inspect the exact scope diff:

```sh
node tools/generate-textmate-cases.mjs
node tools/generate-goldens.mjs --case rust
node tools/generate-textmate-cases.mjs --check
node tools/generate-goldens.mjs --check
```

Omit `--case` to regenerate all goldens. Do not hand-edit golden JSONL. Use
`cases.config.json` for asset remaps or non-convention regression fixtures.

For membership or performance changes, complete the
[asset promotion procedure](../../../docs/assets.md#adding-a-public-language)
before regenerating the ledger and summary:

```sh
python3 tools/generate-language-status.py
python3 tools/check-language-docs.py --write
python3 tools/check-language-docs.py --check
```

## Triage

```sh
node tools/triage-language.mjs rust --golden
```

`--golden` compares against committed reference output; omit it to generate a
fresh temporary oracle. Triage also checks degradation/budget counters and
stress throughput. Use `--kind stress` to focus a fixture kind or `--keep-temp`
to retain diagnostics. For ad-hoc reference output use
`node tools/golden-dump.mjs --help` and write outside committed fixture paths.

## Non-obvious fixture choices

- `zig/stress.zig` retains pre-0.11 async syntax because the pinned grammar has
  rules for it. These fixtures test grammar coverage, not compiler acceptance;
  replacing those lines with modern Zig would drop that coverage.
- The pinned `bird2` grammar's nested `#blocks` rule consumes the brace that
  would end `meta.function-definition.bird`. Function scopes therefore leak to
  end-of-file in the reference too. Definitions stay in `bird2/basic.bird2`:
  including them in repeated stress input changes every later continuation
  state and defeats line caching. Revisit this split when the upstream rule
  changes.
