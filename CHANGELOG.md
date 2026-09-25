# Changelog

## Unreleased

- Cut construction and first-use allocation: borrow embedded bundle payloads
  and scope names from static data, share interned grammar patterns and
  repository names, store capture specs as sorted vectors, and skip the
  diagnostics-only translated regex spelling and parser scratch vectors.
- Store regex literal-alternation tries in an exactly sized sorted layout
  (12-byte nodes, one byte per edge) instead of per-node edge vectors.
- Public APIs, outputs, and the Rust 1.88 MSRV are unchanged.
  `PreparedLanguage` statistics report smaller charged bytes for the smaller
  structures. The measured SDBL steady workload is about 0.7% slower
  (within one standard deviation).

## 0.1.3 - 2026-09-19

- Reduce temporary bytecode-compilation allocations by borrowing subroutine AST
  definitions and exact literal inventories, without retaining AST references.
- Preserve degraded status on line-cache replay and report regex-budget
  exhaustion inside capture retokenization and `while` conditions.
- Fix case-insensitive lookbehind across different UTF-8 character widths in
  both bytecode and recursive matchers.
- Preserve literal scope punctuation and space-separated empty scope atoms;
  strip leading dots only from interpolated capture text, matching TextMate.
- Add generated custom-grammar oracle regressions and a `profile-alloc
  --no-line-cache` mode that separates warm execution from cached-token replay.
- Public APIs and the Rust 1.88 MSRV are unchanged. Output changes correct
  previously non-oracle scope/capture behavior; cached budget failures now
  consistently report `Degraded` instead of incorrectly reporting `Complete`.
- Cold-start gains are strongest for C++; this is not a general steady-state
  speedup. Correct Unicode lookbehind costs approximately 6% on the measured
  SDBL workload. See the [measurements and known limitations](https://github.com/phongndo/syntaxmate/blob/v0.1.3/docs/correctness-performance-pass.md).

## 0.1.2 - 2026-08-23

- Add reproducible engine and end-to-end competitive benchmarks against current
  pinned vscode-textmate, Shiki, and Syntect releases.
- Reduce tokenizer capture and scope-output allocations with deferred group-zero
  synthesis, direct compact-capture output, bounded buffer reuse, and compact
  candidate indexes; speed up HTML and ANSI color serialization.

## 0.1.1 - 2026-08-04

- Fix participating capture ranges in the optimized Nix expression-end
  lookahead so they match `vscode-oniguruma`.
- Expand scanner-execution parity from four fixtures to a balanced sample over
  all 31 core regression assets.
- Speed up tokenizer construction with dense O(1) rule lookup, shared immutable
  repository walks, and copy-on-write grammar registry clones.
- Reduce cold and incremental allocation pressure by directly decoding grammar
  unions, sharing interned scopes and capture specs, and compacting regex tries.
- Store deterministic versioned compiled-grammar IR in the bundled asset,
  removing runtime JSON parsing and rule compilation for bundled languages.
- Add caller-owned `PreparedLanguage` snapshots for constructing independent
  tokenizers while reusing count- and byte-bounded immutable grammar and regex
  preparation.
- Add reusable incremental token/span buffers, callback sinks, and direct
  compact-token HTML/ANSI rendering paths while retaining the owned APIs.
- Cache incremental theme resolution by private tokenizer scope-stack identity
  with a fixed 8,192-slot per-session bound.
- Add resettable warm incremental profiling, peak live-byte and output-digest
  reporting, plus reviewed allocation and retention ceilings in CI.

## 0.1.0 - 2026-08-01

- Extract the Rust-native TextMate grammar, tokenizer, regex, catalog, and theme
  engines from Mark.
- Add batteries-included whole-document and incremental highlighting APIs.
- Bundle the validated 264-language catalog and four GitHub dark/light themes,
  including high-contrast variants; custom themes remain supported.
- Add escaped HTML and terminal-safe 24-bit ANSI renderers.
- Add feature-powerset, coverage, scheduled and differential fuzzing, static
  analysis, comparative benchmarks, secure OIDC release automation, dependency
  updates, and community templates.
