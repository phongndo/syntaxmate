# Architecture and design boundaries

Syntaxmate keeps a small public facade over private engine internals so grammar
and regex optimizations do not force downstream API migrations. Applications
consume exact scopes or generic styles; editor configuration, worker queues,
and UI-framework types belong in downstream adapters.

## Where to look

| Task | Entry point |
| --- | --- |
| Public API and feature gates | [crate facade](../src/lib.rs), [Cargo features](../Cargo.toml) |
| Custom grammars, prepared languages, state and checkpoints | [tokenizer API](../src/tokenizer.rs) |
| Bundled highlighting, sessions and styling | [highlighter](../src/highlighter.rs) |
| Detection and provenance | [catalog](../src/catalog.rs), [grammar registry](../src/grammars/registry.rs) |
| Grammar compilation and bundle encoding | [grammar compiler](../src/engine/grammar.rs), [IR codec](../src/engine/grammar_ir.rs), [bundle builder](../tools/build-bundle.rs) |
| Matching and continuation | [regex engine](../src/engine/regex/), [tokenizer engine](../src/engine/tokenizer.rs) |
| Theme selectors and output | [themes](../src/theme/mod.rs), [renderers](../src/render.rs) |

## Ownership

Mutable continuation state and source-dependent caches belong to a tokenizer
or highlighting session. Independent instances must not affect one another's
output. `TokenizerState` and `CheckpointTable` are tied to their originating
tokenizer; cloning a state does not make it transferable to another tokenizer.

`PreparedLanguage` is the explicit sharing boundary for repeated independent
tokenizers. It retains bounded grammar and static matcher preparation, while
derived tokenizers keep their own mutable state. Read its rustdoc and statistics
API for retention semantics; numeric cache ceilings live with the implementation.
This avoids hidden process-global retention and lets the caller choose the
lifetime of reusable work.

## Public contract

Exact ordered scopes, UTF-8 byte ranges, resolved styles, detection metadata,
and completion status are observable. Regex bytecode, rule IDs, cache layout,
bundle encoding, and diagnostics are implementation details. Keep new API
items tied to a downstream use case, with rustdoc and tests at the public
boundary. Tokenization must remain independent of themes; custom assets must
remain usable without bundled assets.

The release library accepts custom assets as strings and performs no filesystem,
network, or process-environment access. Committed bundled assets keep normal
builds independent of Node and upstream availability. Development tools own
asset import, compilation, and oracle regeneration.

For output limitations see [compatibility](compatibility.md); for changes to
assets see [asset maintenance](assets.md); for version commitments see
[release policy](releasing.md#version-policy). The extraction source is recorded
in [EXTRACTION.md](../EXTRACTION.md).
