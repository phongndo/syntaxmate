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

- [ ] Add a caller-owned `PreparedLanguage` that can construct independent
      tokenizers while retaining immutable parsed regexes, repository contexts,
      compiled patterns, and static candidate descriptors.
- [ ] Keep hidden process-global mutable caches out of the runtime.
- [ ] Bound and report retained memory.

### 3. Dense repository-context tables

- [ ] Replace `(GrammarId, RuleId)` hash lookups with grammar-indexed, lazily
      allocated dense rule tables.
- [ ] Intern repository names used by traversal and cycle detection.
- [ ] Preserve first-lazy-binding compatibility with `vscode-textmate`.

### 4. Single-pass regex analysis

- [ ] Compute start bytes, effective flags, start class, skip prefix, required
      literals, and capture/backreference metadata in one immutable analysis.
- [ ] Reuse that analysis in matcher, scanner, prefilter, and bytecode setup.

### 5. Compact regex bytecode

- [ ] Establish size assertions for instructions and backtrack frames.
- [ ] Evaluate `u32` program counters, packed flags/operands, and improved frame
      locality.
- [ ] Add only byte-exact, measured superinstructions.

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
