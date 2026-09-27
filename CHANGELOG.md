# Changelog

## Unreleased

- Prepare the breaking 0.2 API: unify document and incremental token/line types,
  expose allocation-free `Scopes` views, and consolidate theme construction and
  resolution on `Theme`. Document output now reports completion per line.
- Hide scope storage and cache instrumentation, remove redundant aliases and
  catalog free functions, and require rustdoc for every public item.

- Implement `PartialEq`, `Eq`, and `Hash` for `TokenizerState`, allowing editors
  to stop incremental re-highlighting when continuation states converge. Keep
  embedded base-grammar context distinct when reusing static frame identities.
- Borrow uncompressed bundle string and scope tables to reduce cold-start latency
  and retained heap, trading a larger bundle for no metadata decompression.
  Custom-grammar-only builds no longer depend on `miniz_oxide`.
- Reduce HTML output by inheriting default colors from the wrapper and merging
  equal adjacent runs. Keep full colors when rendering without a wrapper.
  ANSI output now omits the theme default background unless
  `AnsiOptions::include_default_background` is set.
- Add class-based HTML through `HtmlOptions::class_prefix` and `html_stylesheet`,
  plus `render_html_to` and `render_ansi_to` for `fmt::Write` sinks. Option struct
  literals should use `..Default::default()` or supply the new fields.
- Restore allocation guardrails for prepared tokenizers and bundled construction
  by dropping construction-only rule templates and borrowing embedded repository
  skeleton bytes.
- Reduce first-use tokenization costs for complex and embedded grammars through
  lower-allocation regex parsing, deferred matcher construction, and reuse within
  each tokenizer. Refresh the [catalog reference measurements](benchmarks/textmate/catalog-performance.json).
- Honor `TokenizerOptions::line_cache_entries = 0` by disabling line-result
  caching completely.
- Keep the verified MSRV at Rust 1.88 while using a newer development toolchain.
- Slim the published crate by omitting raw grammar sources and checkout-only
  development targets; retain bundled assets and all license notices.
- Document only user-facing features on docs.rs, with feature availability labels.
- Refresh bundled grammars from `@shikijs/langs` 3.23.0 to 4.4.3. Highlighting
  changes for 52 upstream-updated grammars, including a rewritten C++ grammar
  whose declarations, calls, and attributes scope differently, and `coq` now
  uses the upstream `source.rocq` root scope. Seven new Shiki grammars (`ahk`,
  `ahk2`, `chapel`, `nsis`, `org`, `rbs`, `smithy`) are vendored as private
  assets pending promotion; the public catalog is unchanged at 264 languages.
- Update the reference oracle to `vscode-textmate` 9.3.2, matching current
  VS Code; `vscode-oniguruma` stays at 1.7.0 because VS Code still ships it.
- Support Oniguruma `\p{XIDS}`/`\p{XIDC}` (XID_Start/XID_Continue, with loose
  property-name matching), used by the updated Typst grammar.
- Fix fallback regex search skipping matches of nullable patterns whose empty
  branch is guarded by a lookbehind, such as `x|(?<=T)`. Patterns whose every
  branch starts with `^`, `\A`, or `\G` (for example `(^|\G)`) are now tried
  only at those anchors, and each search reuses one matcher scratch.
- Match vscode-textmate for captures outside a match: skip empty captures, stop
  at the first capture that starts after the match, and resume scanning at the
  match end instead of after lookahead captures. Captures past the match end
  now nest like captures inside it, including after an empty match such as
  `(?=((a)b))`.
- Pop the enclosing rule after a match rule that does not advance, as
  vscode-textmate does. The bundled GraphQL grammar no longer stays inside a
  type block for the rest of the document.
- Stop re-entering a begin rule that matched without advancing at the same
  position, and drop or format deeply nested tokenizer states without
  recursion. A self-including zero-width `begin` no longer overflows the
  stack.
- Fold case for bracketed classes as Oniguruma does: under `(?i)` a character
  matches when one of its case variants is in the class, with intersections,
  nested classes, and properties evaluated first. `(?i)[^a-{]` now matches
  `` ` ``, `(?i)[A-Z&&a-z]` no longer matches `a`, and `(?i)[\p{Lu}]` matches
  `a`, while `(?i)\p{Lu}` outside brackets still does not.
- Fix several Oniguruma incompatibilities in the fallback matcher:
  - `\k<name>` and `(?(<name>)…)` consider every group sharing the name;
  - `a{1,2}+` repeats the interval instead of being possessive;
  - recursive subroutine calls keep their caller's loop counts;
  - subroutine capture replay no longer panics after backtracking into a
    returned call, and recursion that never consumes input, such as
    `(\g<1>)?`, fails instead of overflowing the stack;
  - Unicode case-insensitive keyword sets containing characters such as `θ`,
    `ϑ`, and `ϴ` no longer miss matches.

### Migrating from 0.1

This is a clean break for 0.2; removed names have no deprecated aliases.
Both document and incremental lines expose `tokens()`, and each token exposes
`range()` and `scopes()`. Styled tokens also expose `style()`. Scope iteration
borrows the token and allocates nothing; cloned tokens keep their scopes alive.
Line `status()` describes that line, while document `status()` covers the whole
operation (including any checkpoint replay).

| Old API | 0.2 replacement |
| --- | --- |
| `DocumentLine` | `TokenizedLine` |
| `TokenSpan`, `ScopedToken` | `Token` |
| `HighlightedSpan`, `IncrementalHighlightedSpan` | `HighlightedToken` |
| `IncrementalHighlightedLine` | `HighlightedLine` |
| `ResolvedSyntaxStyle` | `Style` |
| `SyntaxModifiers` | `FontModifiers` |
| `ScopeStackRef` | `ScopeStackId` |
| `ScopeTable`, `HighlightScopeTable`, `ScopeAtomId` | Private storage; use `token.scopes()` |
| `line.spans()` | `line.tokens()` |
| `line.scope_names(token.scope_stack())`, `line.scope_table()` | `token.scopes()` |
| `token.scope_stack() -> ScopeStackRef` | `Option<ScopeStackId>`; `Some` for document tokens, `None` for incremental tokens; keys are comparable only within the same document |
| `TextMateTheme` | `Theme`; `from_json`, `from_rules`, and `bundled` return crate `Result` / `Error` |
| `Theme::resolve(table, stack)` | `Theme::resolve(token.scopes())`; standalone names use `resolve_scope_names(&[&str])` |
| `TextMateTheme::resolve_style`, `resolve_with_match` | `Theme::resolve_style(scopes)`, `resolve_with_match(scopes)`, requiring `diagnostics` |
| `ResolvedThemeStyle`, `ThemeMatch`, `ThemeSelectorScore` | Same names, available only with `diagnostics` |
| `style_cache_stats`, `memory_bytes` | Private instrumentation; no public replacement |
| `DEFAULT_LINE_CACHE_ENTRIES`, `DEFAULT_MAX_LINE_BYTES` | `TokenizerOptions::default()`; defaults are documented there |
| `canonical_language(language)` | `Catalog::bundled().canonical_language(language)` |
| `detect_language_from_path(path)` | `Catalog::bundled().detect_path(path)` |
| `available_languages()` | `Catalog::bundled().languages()` |

`Theme` also exposes `default_style()` and `color(name)`. Theme selector
inspection is diagnostic output and remains outside the stable API contract.

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
