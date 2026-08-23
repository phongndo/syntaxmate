# Changelog

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
