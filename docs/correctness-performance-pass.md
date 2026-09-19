# Correctness-first compilation and status pass

## Baseline and method

Baseline: `c6db4db7b6873f9da786ff3cb777520cf8763d86`, in a detached worktree.
The measured candidate was subsequently committed as
`8abe394ce6890e3a063d4022b488287912b8b9bd`. Measurements preceded that commit;
release dependencies were unchanged. Source identities, driver hashes,
commands, raw samples, profiles, and red/green logs are in
`target/correctness-pass/` (ignored, machine-local artifacts).

Measurements use Rust/Cargo 1.88.0, the normal release profile (thin LTO,
`codegen-units=1`, panic abort), System allocation, NixOS x86_64, Linux 6.18.48,
and a Ryzen 9 9950X. Node 24.19.0 runs the unchanged, lockfile-pinned
TextMate/Oniguruma oracle. Baseline and candidate have separate build directories.
The final comparison uses eleven alternating-order, separate-process samples,
pinned to logical CPU 8. It records median, minimum, maximum, and sample standard
deviation. Construction, digest computation, and drop costs are not silently
moved across the existing profilers' API timing boundaries.

The four allocation-policy corpus paths and hashes are unchanged: Markdown,
C++, Rust, and HTML stress fixtures, including embedded-language workloads. All returned outputs must be
complete and have identical counts and exact scope/range or rendered-byte
digests before timing comparisons are accepted. Lifecycle allocation runs use
the counting allocator; elapsed lifecycle results come from the same driver
with that allocator disabled. `--no-line-cache` additionally distinguishes warm
matching from cached line-token replay. No whole-document memoization was added.
The existing engine/product profilers provide independent engine and HTML/ANSI
cold, steady, and replay measurements.

## Fresh evidence and selection

The new C++ steady `perf` sample attributed 38.34% of self samples to bytecode
`execute_inner`, 14.16% to `backtrack_or_resolve`, 11.02% to candidate traversal,
and about 8.3% to substring/trie prefilters. The cold sample exposed additional
allocation and compilation costs: `_int_malloc` 6.26%, `malloc_consolidate`
4.18%, literal-trie compilation 3.88%, AST cloning 1.93%, and AST dropping 1.61%.
These are fresh profiles, not the previous roadmap's measurements.

Three runtime experiments were removed before the final candidate:

- Inline empty-backtrack/assertion fast exit: C++ median regressed about 2.9%.
- Suppress repeat undo records without suspended work: negligible allocation
  benefit and no convincing representative timing gain.
- Store a prepared `memchr::memmem::Finder` for literal prefilters: mixed results,
  not a reliable end-to-end gain.

The retained optimization instead borrows subroutine AST definitions during
compilation and uses `Cow<str>` for exact literal inventories. Plain literals
and nested groups borrow; concatenations still own their constructed strings.
Final instructions/tries own their data and retain no AST references. A unit
test verifies literal inventory borrowing. There is no instruction fusion,
execution shortcut, new retained cache, or public runtime API expansion.

## Reproduced defects

| Defect | Minimal observation and fix | Durable evidence |
| --- | --- | --- |
| Cached degraded output becomes complete | On the baseline, `(a+)+(?=$)` against thirteen `a`s followed by `!` is degraded on the first call and complete on identical replay. Store status with the cached line and restore it on hits. | Public API cold/warm, independent-session, owned/buffer/callback, viewport, styling, HTML/ANSI tests; fuzz replay assertion and seed. The regression uses a longer alternation case to cover related budget paths. |
| Nested budget failures are discarded | Exhaustion during capture retokenization or a `while` test can leave the outer call complete. Accumulate degradation throughout the logical line, including bounded-loop exhaustion. | Three status regressions fail against the isolated baseline and pass with the fix. Later safe calls still report complete. |
| Folded lookbehind assumes fixed UTF-8 width | `(?<=(?i:k))x` must find byte range `3..4` in `Kx`; the baseline misses it. Carry effective scoped flags into both width analyses, use conservative scalar byte bounds for folded literals, and disable the recursive case-sensitive shortcut when folding applies. | Six added Oniguruma conformance cases plus bytecode/recursive tests for reverse-width folds, captures, negation, bounded repeats, and an astral prefix. |
| Literal scope names are normalized | TextMate preserves `.entity..name.`, `.`, tabs, and empty atoms between repeated literal spaces. The baseline strips/splits them. Preserve rule scope atoms; strip leading dots only from captured replacement text. | Generated custom-grammar goldens, including `meta.$1` with `..word..`, ensure capture interpolation and literal punctuation remain distinct. Bundled Make goldens protect `.PHONY` interpolation. |

`tests/fixtures/engine-regressions/` contains nine pinned-oracle documents,
generated by `tools/generate-engine-regressions.mjs`, never hand-edited.
They exercise includes, left injections and equal-position candidates, dynamic
end/while patterns, participating and dormant captures, CRLF, empty/final lines,
and Unicode. The regression compares entire ordered scope streams through
whole-document cold/warm calls, independent prepared sessions, owned/reusable/
callback incremental APIs, checkpoints, and edits before populated checkpoints.
Generated-asset CI now checks this additional oracle output.

The frozen differential run (`seed=20260925`, 4,096 cases) has 4,090 matches and
the same six named-conditional dormant-capture differences before and after.
They are not counted as passes. The small public fixtures probe whether those
differences affect capture retokenization, scope substitution, and dynamic
end/while continuation; those fixtures match TextMate after the scope-name fix.
This does **not** prove every ledger entry unobservable. Neither the execution
difference ledger nor any output allowlist was widened.

## Final measurements

All timings below are medians in milliseconds, baseline → candidate. The final
run contains 1,232 checked records and 140 timing summaries.

| Corpus | Engine first | Engine steady | HTML cold | ANSI cold |
| --- | ---: | ---: | ---: | ---: |
| Markdown | 8.9207 → 8.8208 | 1.5136 → 1.5324 | 20.3858 → 19.3811 | 20.0725 → 19.2910 |
| C++ | 15.4157 → 13.6886 | 3.9933 → 3.9824 | 19.4716 → 17.6841 | 19.6181 → 17.5532 |
| Rust | 1.2804 → 1.2821 | 0.5152 → 0.5181 | 1.7488 → 1.7384 | 1.7383 → 1.7135 |
| HTML | 11.4830 → 11.2953 | 2.4045 → 2.4271 | 14.5462 → 13.9548 | 14.5702 → 13.7659 |

C++ engine first improves 11.2%; HTML/ANSI cold output improves 9.2%/10.5%.
The steady engine changes (−0.3% to +1.2%) are not a material VM speedup.
The complete uninstrumented, line-cache-disabled lifecycle is:

| Phase | Markdown | C++ | Rust | HTML |
| --- | ---: | ---: | ---: | ---: |
| construct | 11.0537 → 11.0122 | 2.9849 → 2.9849 | 1.4442 → 1.4381 | 2.3245 → 2.3306 |
| tokenize-first | 10.3246 → 10.0855 | 17.5044 → 15.6618 | 1.2592 → 1.2294 | 12.7230 → 12.5692 |
| tokenize-warm | 1.6101 → 1.6209 | 4.1378 → 4.1172 | 0.6002 → 0.5875 | 2.5706 → 2.5297 |
| incremental-first | 10.2738 → 10.0839 | 17.7064 → 15.7707 | 1.1511 → 1.1254 | 12.7429 → 12.5511 |
| incremental-warm | 1.6214 → 1.6165 | 4.1408 → 4.0549 | 0.5945 → 0.5776 | 2.5621 → 2.5262 |
| highlight-lines-first | 10.4471 → 10.2997 | 18.3234 → 16.3724 | 1.1860 → 1.1518 | 13.1812 → 12.8830 |
| highlight-lines-warm | 1.6577 → 1.6444 | 4.2683 → 4.2637 | 0.6054 → 0.5862 | 2.6040 → 2.5478 |
| prepare-language | 15.2963 → 15.2772 | 3.4117 → 3.4123 | 0.3310 → 0.3257 | 1.8864 → 1.9052 |
| prepared-new | 0.0094 → 0.0106 | 0.0083 → 0.0097 | 0.0065 → 0.0066 | 0.0068 → 0.0085 |
| prepared-first | 10.0046 → 9.7885 | 16.3420 → 14.3622 | 0.9577 → 0.9286 | 11.8738 → 11.6425 |
| prepared-new-warm | 0.0117 → 0.0127 | 0.1485 → 0.1558 | 0.0048 → 0.0060 | 0.0065 → 0.0067 |
| prepared-reuse | 2.0692 → 2.0239 | 6.7662 → 6.6289 | 0.6503 → 0.6316 | 4.1478 → 4.1180 |

Some tiny prepared-tokenizer construction phases regress by 1–2 µs; for example,
HTML `prepared-new` rises 25.7% (6.76 → 8.50 µs). C++ warmed construction rises
7.28 µs with overlapping sample ranges. These are reported rather than called
wins; the complete prepared-first/reuse lifecycles improve. Cached-token warm
replay is a separate, much cheaper workload:

| Cached phase | Markdown | C++ | Rust | HTML |
| --- | ---: | ---: | ---: | ---: |
| tokenize-warm | 0.0576 → 0.0582 | 0.1014 → 0.1007 | 0.0461 → 0.0455 | 0.1107 → 0.1101 |
| incremental-warm | 0.0388 → 0.0410 | 0.0726 → 0.0787 | 0.0474 → 0.0474 | 0.0681 → 0.0713 |
| highlight-lines-warm | 0.0422 → 0.0427 | 0.0868 → 0.0875 | 0.0553 → 0.0557 | 0.0833 → 0.0830 |

### Allocation and retention

These counts come from separate instrumented runs, not the elapsed-time driver.
First whole-document phases with default line caching:

| Corpus | Allocation/reallocation calls | Cumulative bytes | Boundary-retained bytes, both | Peak additional live bytes, both |
| --- | ---: | ---: | ---: | ---: |
| Markdown | 152,866 → 138,778 | 17,259,847 → 17,079,968 | 8,518,724 | 8,528,036 |
| C++ | 234,941 → 163,849 | 34,024,847 → 31,116,550 | 17,384,432 | 17,398,352 |
| Rust | 9,897 → 9,518 | 1,456,883 → 1,453,228 | 766,889 | 770,409 |
| HTML | 166,086 → 150,573 | 19,856,284 → 19,777,802 | 10,315,483 | 10,338,619 |

C++ removes 71,092 calls (30.3%) and 2,908,297 cumulative bytes (8.5%).
Incremental-first and highlighting-first remove the same number of calls/bytes
as whole-document first for each corpus. Prepared phases remove:

| Phase; calls / bytes saved | Markdown | C++ | Rust | HTML |
| --- | ---: | ---: | ---: | ---: |
| prepare-language | 3 / 78 | 22 / 775 | 57 / 1,503 | 4 / 174 |
| prepared-first | 14,085 / 179,801 | 71,070 / 2,907,522 | 322 / 2,152 | 15,509 / 78,308 |
| prepared-reuse | 433 / 13,270 | 351 / 12,615 | 37 / 1,141 | 541 / 15,854 |

Construction, warmed token/incremental/highlighting phases, and new-tokenizer
allocation medians are unchanged. The no-line-cache runs show the same
per-phase savings. Median boundary retention and peak additional live bytes are
unchanged in every measured phase and cache mode; Markdown construction has
an occasional 1,088-byte peak variation in both builds. Reallocation counts
are unchanged. The existing allocation-policy ceilings pass without alteration.

### Catalog and downstream

All 264 `catalog-repeated` languages passed completeness and exact scope-digest
comparison in three alternating sample pairs. Summed per-language first-call
medians are 1,121.562 → 1,116.787 ms (−0.43%): essentially neutral, not a claimed
catalog-wide speedup. The normal CI catalog throughput floor also passes.

The catalog scan did expose a real SDBL cost, not hidden by the neutral
aggregate. Eleven focused pairs give first-call 7.4675 → 7.9626 ms (+6.6%) and
uncached steady 55.3136 → 58.7632 ms (+6.2%). Nix and Erlang follow-up changes
are below 1%. An isolated, deliberately incorrect ablation kept the compiler
borrowing and other fixes but restored fixed-byte folded-literal bounds. Its
SDBL times returned to 7.4821/54.4634 ms, versus the correct candidate's
7.9546/58.6638 ms in that run. This attributes the cost to the correctness
repair, not the allocation optimization. The broken ablation is **not shipped**.
The pass therefore does not meet a blanket no-regressions performance claim:
recovering that lookbehind overhead without missing Unicode matches remains
open. No budget was raised or workload dropped to conceal it.

The isolated Mark snapshot is commit
`56c2735558ebdf2a6158ff9d5c68d5fde3672b49`. It uses the real `mark-syntax`
adapter and MiMalloc, with identical baseline/candidate dependency patches only
inside ignored driver directories. First-call medians are Markdown 19.6019 →
19.2853 ms, C++ 18.1194 → 17.4590 ms (−3.6%), Rust 2.7794 → 2.7362 ms, and HTML
13.9902 → 13.8149 ms. C++ sample ranges are 17.8043–19.0022 versus
17.0862–17.6163 ms. Warm adapter changes range from −2.6% to +2.4%, with no
material improvement claimed. Canonical scope/range/class digests match;
identical native options independently verify completeness outside timed calls.
This is an adapter benchmark, not a full Mark TUI benchmark. The user's dirty
Mark working tree was not modified.

## Safety boundaries

Execution-step, fallback, recursion, line-length, prepared-pattern, candidate,
scanner, theme, and capture-pool ceilings are unchanged. No pool was added or
enlarged, no mutable process-global cache was introduced, and no unsafe code or
native runtime dependency was added. Borrowed compiler inputs cannot outlive
compilation; compiled programs keep their existing owned representation.
UTF-8 boundaries and full capture/winner checks remain in place. Cache hits now
preserve resource-failure status rather than upgrading partial work to complete.
Bundled grammar sources, grammar bundle, oracle versions, corpus manifests,
allocation ceilings, and difference ledgers are unchanged.

## Verification and limitations

Executed successfully on Linux:

- Rust 1.88 formatting, Clippy (`--all-targets --all-features --locked -- -D
  warnings`), rustdoc with warnings denied, no-default library/fuzz-manifest
  checks, and all-target/all-feature MSRV checks.
- Default and all-feature tests: 402 library tests each, plus the applicable
  integration tests and doctests. All four exact strict golden shards ran with
  `SYNTAXMATE_SHARD_TOTAL=4`, indexes 0–3, and the complete
  `textmate_golden::manifest_golden_cases_` filter (not only scope parity).
- Feature powerset (`--depth 2 --exclude-features bundle-tools`), custom-theme
  session without bundled themes, bundle freshness and size, `cargo deny`,
  catalog throughput, allocation ceilings, and performance-reporting tests.
- All generated-workflow commands: 100/100 regex conformance cases, 512-scanner
  execution replay with no new/stale exception, construct coverage, seed-1
  256-case differential run, case/golden/custom-regression generation, theme
  goldens/selectors/catalog/vendor assets, corpora, language status, and docs.
- Nightly 1.100.0 (2026-09-03) fuzz smoke, seed 20260925, 45-second requested
  budgets: 277,790 grammar/source and 6,740,677 theme runs; both exit zero.
  Fuzzer-generated corpus entries/artifacts stay under the ignored target tree.
- Rust 1.98 all-target/all-feature and no-default builds and public API tests.
  Semver comparison against published 0.1.2 passes on Rust 1.95 with
  cargo-semver-checks 0.47.0 (196 checks pass, 56 inapplicable checks skip).

Packaging passed with the exact `cargo package --locked` command in a clean,
isolated candidate snapshot. Both packaged consumers then ran successfully:
default bundled highlighting/escaped HTML, and no-default custom grammar/theme
HTML. The original dirty working tree correctly refuses plain packaging;
`--allow-dirty` was also verified there. The clean snapshot avoids changing or
committing the user's working tree merely to satisfy Cargo's cleanliness gate.

Limitations of the original local verification:

- Local **Rust 1.98.0 Clippy was not green**: it emitted the same five diagnostics
  on the untouched baseline and candidate (generic trie `Default`, lazy range
  fallback, and three constant-chunk test loops). These unrelated baseline
  diagnostics were not suppressed. Rust 1.88 Clippy passes.
- cargo-semver-checks 0.47.0 cannot parse Rust 1.98's rustdoc JSON v60; the
  successful compatibility run uses Rust 1.95's supported format instead.
- The frozen 4,096-case differential run retains six failures, described above;
  it is not presented as an all-green fuzz run.
- macOS/Windows, hosted coverage/security publishing, and the long scheduled
  fuzz campaigns were not run locally. The Mark TUI and user-modified Mark
  workloads were not benchmarked.

### Hosted CI follow-up (2026-09-19)

The same candidate commit subsequently passed the complete
[hosted CI run](https://github.com/phongndo/syntaxmate/actions/runs/35414983686),
including quality/Clippy, MSRV, feature powersets, semver, all four golden shards,
generated assets, performance, packaged consumers, all three operating systems,
and coverage. The PR-only dependency-review job was inapplicable and skipped.
[CodeQL](https://github.com/phongndo/syntaxmate/actions/runs/35414983692) also
passed. Hosted quality used Rust **1.98.1**, not the local 1.98.0 toolchain.
No lint suppression or CI gate change was needed.

This establishes a green hosted CI matrix for the measured code; the local
1.98.0/tool-version diagnostics above remain recorded as historical evidence.
The longer fuzz campaigns, six frozen differential mismatches, and SDBL
performance tradeoff remain as documented. Zero performance regressions are
not claimed.

## Reproduction and artifact map

Within `nix develop`, the relevant public entry points are:

```sh
cargo test --all-features --locked
cargo test --all-features --locked engine_regressions
node tools/generate-engine-regressions.mjs --check
node tools/regex-conformance.mjs --out target/regex-conformance.json
node tools/regex-execution-parity.mjs --max-executions 512 --out target/regex-execution-parity.json
cargo run --release --example profile-alloc -- --json --no-line-cache cpp tests/fixtures/textmate/cpp/stress.cpp
python3 tools/check-allocation-performance.py --write-report target/allocation-report.json
```

Machine-local artifacts under `target/correctness-pass/`:

- `environment.txt`, `driver-hashes.txt`, source identities and final patch:
  environment and candidate/baseline provenance.
- `baseline-debug.perf`, `profile.txt`, `baseline-cold.perf`, `profile-cold.txt`:
  initial profile evidence; experiment reports/logs record rejected attempts.
- `baseline-exact-red.log`, `baseline-test-overlay.patch`, `lookbehind-red.log`,
  `probes.json`, `final-focused.log`: minimized defects and regression evidence.
- `final-compare.py`, `final-raw.json`, `final-summary.json`: accepted final
  timing and allocation samples, not the earlier `lifecycle-*` exploratory run.
- `catalog-compare.py`, `catalog-raw.json`, `catalog-summary.json`,
  `catalog-followup-*`, `width-ablation-*`: broader comparisons, completeness/
  digest checks, and the explicitly non-shipping lookbehind cost attribution.
- `mark-identity.txt`, isolated `mark-snapshot/`, `mark-{baseline,candidate}/`:
  downstream adapter source and builds. No override was left in Mark.
- `verify.py`, `*-results.json`, individual check logs, package-consumer logs:
  command-by-command outcomes.
- `fuzz-baseline.json`, `fuzz-candidate-frozen.json`, `frozen-fuzz.log`,
  `fuzz-*.log`, `fuzz-exits.txt`: preserved successes and unresolved differences.

These measurements establish a compilation/allocation improvement, not a
material steady-state VM speedup. Future execution work should start with new
profiles and exact winner/capture evidence, not revive the rejected fast paths
unchanged. Longer fuzz campaigns and non-Linux runners remain separate CI work.
