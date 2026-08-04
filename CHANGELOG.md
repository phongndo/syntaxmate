# Changelog

## Unreleased

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
