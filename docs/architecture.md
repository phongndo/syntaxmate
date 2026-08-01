# Architecture

Syntaxmate is one public crate with layered private internals. The public API
is intentionally smaller than the engine implementation so parser and regex
optimizations can evolve without forcing downstream migrations.

## Layers

1. **Catalog and assets** — a deterministic compressed grammar bundle, embedded
   themes, aliases, path metadata, and third-party provenance.
2. **Grammar compiler** — parses JSON TextMate rules, validates IDs and include
   graphs, and compiles immutable grammar structures.
3. **Regex engine** — routes regular patterns through scanners/automata and
   advanced Oniguruma constructs through a budgeted native backtracker.
4. **Tokenizer** — applies TextMate grammar ordering, captures, injections,
   continuation stacks, line caches, and viewport checkpoints.
5. **Scope and theme engine** — interns exact scope stacks and resolves TextMate
   selectors to generic RGB colors and font modifiers.
6. **Facade and renderers** — provides bundled/custom highlighters plus escaped
   HTML and terminal-safe ANSI output.

## Ownership and isolation

Mutable grammar, matcher, frame, state, and line caches are owned by a
`Tokenizer` or `Highlighter`. Dropping that owner reclaims the state; separate
instances do not communicate through hidden mutable global caches. Process-wide
initialization is limited to immutable embedded bundle/theme data.

`TokenizerState` and `CheckpointTable` are opaque and tied to the tokenizer that
created them. This prevents accidental state reuse across grammar sets while
allowing callers to clone state within one session.

## Compatibility boundary

The observable contract is:

- UTF-8 byte ranges on character boundaries;
- exact ordered TextMate scope stacks;
- resolved generic styles;
- deterministic catalog/detection metadata;
- explicit complete/degraded status.

Regex bytecode, grammar rule IDs, cache layout, compiled patterns, and coarse
internal syntax classes are not public API.

## Runtime access

The release library performs no filesystem, network, process-environment, or
Node access. Custom assets are supplied as strings by the caller; bundled assets
are compiled into the crate. Development tooling owns vendoring, checksums,
oracle generation, and corpus production.
