# Performance roadmap

This roadmap turns profile evidence into independently reviewable changes. Each
item must preserve exact TextMate scope streams, bounded execution, deterministic
assets, and the public UTF-8 byte-range contract.

## Measurement contract

Before marking an item complete:

1. Compare an optimized release build with the commit immediately before the
   item using alternating-order, separate-process samples.
2. Report construction plus first and warm whole-document, incremental, and
   incremental-highlighting phases where relevant.
3. Report allocation calls, cumulative allocated bytes, boundary and peak
   retained bytes, and elapsed API time with `examples/profile-alloc.rs`.
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
without separate byte-exact evidence. Benchmark token counts and the complete
golden scope streams were unchanged. The raw median report is
`target/profile-item5-comparison.json`.

### 6. Reusable-buffer and sink APIs

- [x] Add optional `tokenize_line_into` and callback/sink APIs.
- [x] Add direct compact-token HTML/ANSI rendering paths.
- [x] Keep existing owned-output APIs unchanged.

Result: `tokenize_line_into` and `highlight_line_into` reuse caller-owned
buffers, while `tokenize_line_with` and `highlight_line_with` deliver owned
scope-backed values without an output collection. Against the corresponding
owned APIs, seven alternating release samples removed 194 to 244 allocation
calls and 17 to 92 KiB cumulatively for token lines, and 346 to 484 calls and 39
to 219 KiB for styled lines. Depending on corpus size, that is a 0.1% to 1.9%
call and 0.1% to 4.9% byte reduction for token sinks, and 0.2% to 3.5% and 0.3%
to 10.8% for styled sinks. Callback retained bytes were unchanged; reusable
buffers retained only their final 0.75 to 4.5 KiB capacity. Most sink elapsed
medians were neutral or improved; regressions were at most 0.274 ms.

The bundled `Highlighter` HTML/ANSI conveniences now render the engine's compact
scope-stack spans directly; standalone structured-document renderers and all
owned token APIs are unchanged. Warm HTML allocation calls fell 6.1% to 24.4%
and cumulative bytes fell 11.2% to 16.2%. Warm ANSI calls fell 70.5% to 92.1%,
cumulative bytes fell 45.8% to 59.3%, and elapsed time improved 23.1% to 49.5%.
First ANSI rendering also reduced calls 1.0% to 36.2%, bytes 0.5% to 18.3%, and
elapsed time 0.7% to 16.3%. First HTML rendering reduced calls 0.1% to 1.3%
and bytes 0.2% to 3.7%, with elapsed time effectively neutral. Rendered byte lengths, tokenizer item counts, and complete
golden scope streams were unchanged. Existing owned phases remained
allocation- and byte-identical except one fewer Markdown construction call.
The raw median report is `target/profile-item6-comparison.json`.

### 7. Incremental theme cache

- [x] Carry internal scope-stack identity through incremental highlighting.
- [x] Cache `ScopeStackId -> ResolvedSyntaxStyle` per session with a hard bound.

Result: incremental sinks now carry the tokenizer's stable private
`ScopeStackId` into `HighlightSession`, which caches resolved styles in a dense
8,192-slot per-session table. Higher IDs are resolved without retention, so
pathological dynamic scope generation cannot grow the cache beyond the fixed
bound.

Across seven alternating release samples, a second incremental highlighting
pass with the cache populated improved elapsed time by 14.0% to 59.8% across
owned, reusable-buffer, and callback APIs. First-pass reusable/callback elapsed
time improved 3.4% to 3.6% for C++, 14.3% to 15.4% for Rust, and 0.9% to 1.7%
for HTML; Markdown's -0.8% callback and +1.4% buffer medians were treated as
neutral. The owned first pass improved 0.9%, 3.2%, 15.8%, and 1.2% for Markdown,
C++, Rust, and HTML, respectively. It now fills its final span collection
directly, removing 151 to 239 allocation calls and 8.7 to 58.9 KiB of cumulative
allocation depending on the corpus.

Initial cache population added one allocation and six to nine reallocations to
the sink paths, retaining 2.25 to 18 KiB and adding 4.5 to 36 KiB cumulatively.
Once populated, reusable-buffer and callback allocations, cumulative bytes, and
retained bytes were unchanged while style-resolution time fell. All unrelated
`profile-alloc` phases remained allocation- and byte-identical. Item counts,
range/scope/style digests, and complete golden scope streams were unchanged.
The raw median report is `target/profile-item7-comparison.json`.

### 8. Performance guardrails

- [x] Add warm incremental replay and warm incremental highlighting phases.
- [x] Add peak retained bytes and token/scope-stream digests.
- [x] Define reviewed CI allocation ceilings and corpus percentile reporting.

Result: `profile-alloc` now resets incremental continuation state while
retaining tokenizer and theme caches, then measures true warm tokenization and
highlighting replays. Its human and versioned JSON outputs report allocation
and reallocation calls, cumulative and boundary-retained bytes, peak additional
live bytes, API elapsed time, completeness, item counts, and stable token-range
and exact scope-stream digests. Digest work stays outside the timed API
intervals, and first/warm output mismatches fail the profiler.

The CI allocation policy fixes four representative Markdown, C++, Rust, and
HTML stress corpora by path, byte count, and SHA-256. All 12 phases have reviewed
per-corpus ceilings for allocation calls, cumulative allocated bytes, and peak
live bytes, calibrated with approximately 5% headroom plus small fixed
allowances. The checker rejects
stale inputs, malformed accounting, degraded output, digest drift, and ceiling
breaches. It also emits nearest-rank p50/p95 allocation-call, cumulative-byte,
and peak-live-byte densities per KiB; elapsed time remains informational on
shared runners. The checked policy is
`benchmarks/textmate/allocation-policy.json`, and ad-hoc reports can be written
to `target/textmate-performance/allocation-report.json`. The initial four-corpus
CI replay passes every ceiling with exact first/warm token and scope digests.

### 9. Winner-capture and output allocation cleanup

- [x] Let candidate selection synthesize capture group zero from the winning
      span and replay only live nonzero groups.
- [x] Copy bytecode capture slots directly into the final group layout and
      recycle final capture vectors in a hard-bounded tokenizer-owned pool.
- [x] Compact candidate adjacency indexes to `u32` and reuse scope-stack
      resolution storage.
- [x] Replace generic color formatting in hot HTML and ANSI writers with
      byte-exact direct encoders.

Result: eleven alternating release samples reduced steady engine time by 0.25%
for Markdown, 1.65% for C++, 4.68% for Rust, and 1.05% for HTML. First-document
allocation calls fell 0.95% to 10.55% and cumulative bytes fell 1.00% to 4.77%.
Incremental-first and highlighting-first calls fell 0.67% to 11.72%, with
0.93% to 5.37% fewer bytes. Prepared reuse showed the largest reductions:
8.69% to 28.31% fewer calls and 4.89% to 13.40% fewer bytes.

The capture pool retains at most 16 vectors of at most 1,024 slots (384 KiB of
payload on 64-bit targets). Actual prepared-reuse peak changes ranged from
-24.1 KiB to +22.7 KiB across the four corpora, while first-document,
incremental, highlighting, and prepared-first peaks all declined. Scope-stack
resolution now reuses one ID scratch vector and constructs final `Arc` slices
directly, removing up to 5.18% of prepared-reuse calls in the scope-output
phases without changing ownership at the public boundary.

Direct two-digit hexadecimal and decimal-byte color writers improved warm HTML
end-to-end time by 2.55% to 13.72% and ANSI time by 1.60% to 12.12% across the
same corpora. Every before/after token, scope, HTML, and ANSI digest matched;
the strict complete-catalog golden suite remained exact. Seventy-two improved
allocation ceilings were tightened without weakening any existing ceiling.
Raw reports are under `target/perf-work/`, including
`final-engine-comparison.json`, `final-alloc.json`, and
`final-product-comparison.json`.

The remaining C++ steady-state profile is dominated by regex execution: 55.9%
of top-of-stack samples were in the bytecode/fallback VM, 8.9% in substring
prefilters, and 4.1% in allocation. Candidate traversal is the next distinct
cost center, but its start-class and skip gates reject most expensive attempts;
changes there must preserve that filtering advantage.

## Experiments not to repeat unchanged

The engine history already records neutral, slower, or incompatible attempts,
including independent per-pattern next-match memoization, the linear-only
bytecode slice, position-only recursive subroutines, larger execution budgets,
and several start-gate variants. This iteration also rejected a case-folded
word-set hash (no representative allocation reduction), fixed-count scan and
repeat-start bytecode fusions (mixed 0.5% to 2.0% regressions), direct-index
start-class classification (C++ regressed 1.8%), and bulk HTML/ANSI ordinary-run
scans (up to 3.7% and 2.5% regressions). Revisit them only with a materially
different design and new parity evidence.
