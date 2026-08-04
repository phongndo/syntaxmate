# Public API design and stability

Syntaxmate's public API is layered so downstream applications can stop at exact
TextMate scopes or opt into styling and rendering. The tokenizer does not
require a theme.

## Stable conceptual layers

1. `GrammarRegistry`, `PreparedLanguage`, and `Tokenizer` compile or retain
   grammars and emit exact scopes.
2. `Theme` maps exact scope stacks to generic RGB styles and font modifiers.
3. `render_html` and `render_ansi` render an already styled document.
4. `Highlighter` and `HighlightSession` are bundled-asset convenience APIs.

The low-level grammar compiler, regex VM, numeric rule IDs, and mutable caches
are deliberately private. `PreparedLanguage` is the explicit caller-owned
sharing boundary: derived tokenizers share immutable grammar/regex preparation
but never continuation state, line caches, dynamic regexes, or source-specific
candidate-set bindings. Its public statistics expose count and charged-byte
ceilings; artifacts that exceed those ceilings stay tokenizer-local. Public
state and checkpoint values are opaque and may only be reused with their
originating tokenizer.

## Asset independence

`Tokenizer` and `GrammarRegistry` remain usable with
`default-features = false`. `Highlighter::tokenize` is independent of themes,
and `Highlighter::session_with_theme` accepts a caller-supplied theme when
`bundled-themes` is disabled. Bundled themes are convenience data, not part of
the tokenization contract.

## Compatibility policy

Before `1.0`, breaking changes are allowed only when called out in the
changelog and justified by downstream feedback. After the first crates.io
release, CI compares the public API against the latest published version with
`cargo-semver-checks`.

The following are compatibility commitments:

- UTF-8 byte ranges always end on character boundaries;
- exact ordered scope names remain observable;
- resource exhaustion is reported through `HighlightStatus`;
- feature flags are additive;
- release builds perform no filesystem, network, or environment access.

Regex bytecode, cache layout, bundle encoding, and diagnostic output are not
stable interfaces.

## Review checklist

A new public item must have a downstream use case, rustdoc, focused tests,
feature-matrix coverage, and an ownership/error model that does not require
access to private engine internals. Prefer extending opaque types over exposing
implementation structures.
