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
