# Performance roadmap

This roadmap turns profile evidence into independently reviewable changes. Each
item must preserve exact TextMate scope streams, bounded execution, deterministic
assets, and the public UTF-8 byte-range contract.

## Measurement contract

Before marking an item complete:

1. Compare an optimized release build with the commit immediately before the
   item using alternating-order, separate-process samples.
2. Report construction, first and warm whole-document tokenization,
   incremental tokenization, and incremental highlighting where relevant.
3. Report allocation calls, cumulative allocated bytes, retained bytes, and
   elapsed time with `examples/profile-alloc.rs`.
4. Confirm identical token counts and scope-stream digests on benchmark inputs.
5. Run formatting, Clippy, all-feature tests, the complete TextMate golden suite,
   generated-asset checks, and package checks.
6. Keep a change only when representative corpora improve without a material
   regression elsewhere.

Raw ad-hoc reports belong under `target/`; committed catalog performance remains
under `benchmarks/textmate/`.

## Work items

### 1. Compiled grammar IR in the bundle

- [x] Store a versioned deterministic binary representation of
      `CompiledGrammar` in each grammar blob.
- [x] Decode the IR directly for bundled grammars while retaining JSON
      compilation for custom grammars.
- [x] Reject stale bundle versions and malformed/truncated IR.
- [x] Regenerate `assets/grammars.bundle` and document its format/version.
- [x] Measure bundle size, cold construction, first tokenization, and allocation
      changes on embedded-heavy and core grammars.

Result: seven alternating release samples reduced median process-cold time by
1.63x for Markdown, 1.12x for C++, and 1.08x for Rust and HTML. Construction
allocation calls fell by 38.6%, 30.3%, 1.6%, and 11.4%, respectively; cumulative
construction bytes fell by 63.6%, 51.1%, 0.6%, and 29.4%. First/warm,
incremental, and highlighting allocation counts remained identical. Benchmark
token counts and the complete golden scope streams were unchanged. The compact
raw grammar payload shrank 38.5%, while independently compressed blobs increased
the complete committed bundle from 2,048,812 to 2,274,428 bytes (+11.0%) and
changed retained construction bytes by -4.0% to +6.2% depending on closure size.
The raw report is `target/profile-item1-comparison.json`.

### 2. Explicit prepared-language API

- [x] Add a caller-owned `PreparedLanguage` that can construct independent
      tokenizers while retaining immutable parsed regexes, repository contexts,
      compiled patterns, and static candidate descriptors.
- [x] Keep hidden process-global mutable caches out of the runtime.
- [x] Bound and report retained memory.

Result: for one preparation plus four independent first-tokenization sessions,
seven alternating release samples improved Markdown by 2.52x, C++ by 1.75x,
Rust by 1.31x, and HTML by 1.96x. Reusing an already prepared value improved
the session-only phase by 5.23x, 1.95x, 1.89x, and 2.12x, respectively. Total
allocation calls fell 64.1%, 57.9%, 26.3%, and 57.8%; cumulative bytes fell
66.7%, 60.2%, 31.4%, and 57.3%. Process-retained totals were 21.48, 21.97,
4.16, and 13.60 MiB, explicitly owned by the prepared value plus the embedded
bundle. Static regex retention is bounded by each closure's exact slot count
and a 64 MiB conservative byte charge. Reusable static candidate descriptors
have a 1,024-entry ceiling and share a 12 MiB charge with scanners and canonical
injection outcomes. The direct path stayed within 2.3% elapsed time and 0.4%
allocation calls, with exact token/scope-digest parity and unchanged warm
allocation counts. Raw reports are `target/profile-item2-comparison.json` and
`target/profile-item2-alloc.json`.

### 3. Dense repository-context tables

- [x] Replace `(GrammarId, RuleId)` hash lookups with grammar-indexed, lazily
      allocated dense rule tables.
- [x] Intern repository names used by traversal and cycle detection.
- [x] Preserve first-lazy-binding compatibility with `vscode-textmate`.

Result: seven alternating release samples reduced construction allocation calls
by 12.2% for Markdown, 6.0% for C++, 0.5% for Rust, and 2.7% for HTML;
preparation calls fell by 36.5%, 11.7%, 5.5%, and 6.3%, respectively.
Construction elapsed time improved 6.2%, 4.5%, 1.6%, and 2.6%, while
preparation improved 5.3%, 6.6%, 8.7%, and 4.3%. First, incremental, and
highlighting calls also fell 0.4% to 0.7%; warm tokenization allocations were
unchanged. Cumulative and retained bytes declined across all four corpora. One
Rust first-tokenization median (+1.7%), one HTML prepared-reuse median (+0.9%),
and the sub-0.2 ms warm medians were within process noise; the other measured
elapsed phases improved. Benchmark token counts and the complete golden scope
streams were unchanged.
The raw median report is `target/profile-item3-comparison.json`.

### 4. Single-pass regex analysis

- [x] Compute start bytes, effective flags, start class, skip prefix, required
      literals, and capture/backreference metadata in one immutable analysis.
- [x] Reuse that analysis in matcher, scanner, prefilter, and bytecode setup.

Result: seven alternating release samples reduced first-tokenization allocation
calls by 0.9% for Markdown, 0.2% for C++, 1.8% for Rust, and 0.2% for HTML;
incremental and highlighting calls fell by 0.2% to 1.7%. First-tokenization
elapsed time improved 2.7%, 2.9%, 3.6%, and 2.3%, respectively, and prepared
first-tokenization improved 1.1% to 2.6%. Warm allocation counts and tokenizer
construction allocations were unchanged. Preparation calls were effectively
flat for Markdown and HTML and fell 0.2% for C++ and 3.2% for Rust. Sharing the
immutable metadata increased retained bytes by 1.3 to 30.9 KiB depending on the
phase and corpus (at most 0.9% in tokenization and 2.2% in the small Rust
preparation), within the existing conservative prepared-pattern charge.
Benchmark token counts and the complete golden scope streams were unchanged.
The raw median report is `target/profile-item4-comparison.json`.

### 5. Compact regex bytecode

- [x] Establish size assertions for instructions and backtrack frames.
- [x] Evaluate `u32` program counters, packed flags/operands, and improved frame
      locality.
- [x] Keep superinstructions limited to byte-exact, measured forms; no new
      fusion met the bar for this item.

Result: `u32` program counters and VM slots, packed regex flags and repeat
bounds, and compact arena marks reduced each instruction from 56 to 24 bytes,
the hot backtrack frame from 56 to 32 bytes, assertion frames from 120 to 64
bytes, call frames from 16 to 8 bytes, and repeat state from 24 to 16 bytes.
Compile-time and unit-test assertions protect those 64-bit layouts, and
oversized operands reject bytecode compilation rather than truncating, leaving
the bounded recursive fallback available.

Across seven alternating release samples, first-tokenization cumulative bytes
fell 8.6% for Markdown, 12.0% for C++, 4.8% for Rust, and 7.9% for HTML;
retained bytes fell 14.3%, 14.7%, 7.3%, and 13.8%, respectively. Incremental
cumulative bytes fell 4.8% to 11.9% and retained bytes fell 7.9% to 17.3%;
prepared first-tokenization showed comparable 6.4% to 14.0% cumulative and 9.5%
to 16.8% retained reductions. Allocation-call counts and warm cumulative bytes
were unchanged. First, incremental, and highlighting elapsed medians improved
across all four corpora by 1.0% to 4.8%; prepared reuse improved 3.1% to 8.0%.
Prepared first-tokenization improved 0.8% to 5.1% outside Markdown's +0.061 ms
(+0.7%) median. Construction and preparation do not materialize lazy bytecode
and remained allocation-identical; their elapsed changes and the sub-0.2 ms
warm phases were treated as process noise. No new superinstruction was added
without separate byte-exact evidence. Benchmark token counts and the complete golden
scope streams were unchanged. The raw median report is
`target/profile-item5-comparison.json`.

### 6. Reusable-buffer and sink APIs

- [ ] Add optional `tokenize_line_into` and callback/sink APIs.
- [ ] Add direct compact-token HTML/ANSI rendering paths.
- [ ] Keep existing owned-output APIs unchanged.

### 7. Incremental theme cache

- [ ] Carry internal scope-stack identity through incremental highlighting.
- [ ] Cache `ScopeStackId -> ResolvedSyntaxStyle` per session with a hard bound.

### 8. Performance guardrails

- [ ] Add warm incremental replay and warm incremental highlighting phases.
- [ ] Add peak retained bytes and token/scope-stream digests.
- [ ] Define reviewed CI allocation ceilings and corpus percentile reporting.

## Experiments not to repeat unchanged

The engine history already records neutral, slower, or incompatible attempts,
including independent per-pattern next-match memoization, the linear-only
bytecode slice, position-only recursive subroutines, larger execution budgets,
and several start-gate variants. Revisit them only with a materially different
design and new parity evidence.
