# TextMate compatibility

Syntaxmate implements JSON TextMate grammars and TextMate color themes. Its
reference behavior is the pinned `vscode-textmate` and `vscode-oniguruma`
development oracle under `tools/golden-oracle`.

The bundled catalog is validated with exact scope-stack goldens over basic and
stress fixtures. This is a strong regression contract, not proof of every
Oniguruma expression or every possible grammar.

## Supported grammar behavior

The engine supports match, begin/end, begin/while, captures, dynamic end
patterns, local and external includes, `$self`, `$base`, repositories,
injections, selector priority, and continuation state across lines. Its native
regex implementation routes regular patterns through optimized scanners and
uses a budgeted backtracker for constructs such as lookaround,
backreferences, and recursive subroutines.

## Execution-level differences

The final bundled-corpus scope contract has no allowlisted divergences. Direct
replay of real `vscode-oniguruma` scanner calls currently documents a narrower
difference: in some failed alternatives, the oracle retains dormant captures as
empty end-of-line ranges while Syntaxmate reports those nonparticipating
captures as absent. Scanner winners and full-match ranges agree, and the
committed fixtures do not consume those dormant captures. The exact audited
set is locked in `benchmarks/textmate/regex-execution-differences.json`; any new
or stale entry fails CI.

## Indexing and lines

Public Rust ranges are UTF-8 byte offsets on valid character boundaries.
TextMate's JavaScript reference emits UTF-16 offsets; oracle tests convert them
before comparison. Whole-document spans are relative to their logical line.
The normal incremental API accepts one logical line without its newline
terminator and applies TextMate line-termination semantics internally.

## Degradation

Regex and tokenizer work is bounded. When a limit is exhausted, Syntaxmate
continues with safe plain-scope output and reports `HighlightStatus::Degraded`.
Callers that require complete output must check the status.

## Project positioning

Syntaxmate is not a drop-in API replacement for Shiki or Syntect. It targets a
related use case with a different runtime and compatibility boundary:

| | Syntaxmate | Shiki | Syntect |
| --- | --- | --- | --- |
| Primary runtime | Native Rust | JavaScript/TypeScript with WebAssembly regex | Native Rust |
| Main grammar contract | JSON TextMate, checked against `vscode-textmate` | JSON TextMate | Sublime syntax definitions and themes |
| Bundled experience | Grammars, four accessible themes, detection, HTML, and ANSI in one crate | Broad language/theme bundles and web integrations | Default syntax/theme sets plus Rust rendering helpers |
| Incremental state | Opaque line state and viewport checkpoints | Tokenization/highlighter APIs | Parse state and line highlighting |
| Safety boundary | Explicit work limits and `Degraded` status | Host/runtime dependent | Regex-backend dependent |

Choose Shiki when its JavaScript ecosystem, transformers, or framework
integrations are the priority. Choose Syntect when Sublime syntax compatibility
or its mature Rust ecosystem is required. Choose Syntaxmate when a Rust-native,
TextMate-JSON-compatible engine with exact scope output and explicit degradation
is the better fit.

## Not included

Syntaxmate does not implement language-server semantic tokens. Editors such as
VS Code may visually differ when semantic highlighting overlays TextMate
scopes. Plist grammar input, Shiki transformer compatibility, and stable
serialization of continuation state are not currently supported.
