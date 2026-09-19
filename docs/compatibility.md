# TextMate compatibility

Syntaxmate implements JSON TextMate grammars and TextMate color themes. The
reference is `vscode-textmate` with `vscode-oniguruma`, pinned in the development
[oracle package](../tools/golden-oracle/package.json). The
[language ledger](language-status.md) records the generated catalog validation
status. Corpus parity is a regression contract, not proof of every Oniguruma
expression or every possible grammar.

## Scopes and lines

Supported grammar constructs include match, begin/end, begin/while, captures,
dynamic end patterns, repositories, local/external includes, `$self`, `$base`,
injections, and cross-line continuation. Exact ordered scope stacks are public
output; themes are optional.

Public ranges are UTF-8 byte offsets on character boundaries, relative to their
logical line. The JavaScript oracle uses UTF-16 offsets, which the tests convert
before comparison. Incremental calls accept one logical line without its newline
terminator and apply TextMate line-termination semantics internally. Consult
[Tokenizer rustdoc](https://docs.rs/syntaxmate/latest/syntaxmate/struct.Tokenizer.html)
for state ownership and replay contracts.

Regex and tokenizer work is bounded. `HighlightStatus::Degraded` means a limit
prevented complete highlighting. Check status when complete output is required;
a successful return alone does not establish completeness.

## Themes and editor comparisons

Themes resolve ordered scope stacks to foreground/background colors and font
modifiers. Tokenized output can be styled without rerunning the tokenizer.
Caller-supplied themes work without the `bundled-themes` feature; see the
[custom-theme example](../examples/custom_theme.rs).

VS Code can overlay semantic tokens, bracket-pair colors, inlay hints, and other
editor decorations. Disable those when comparing TextMate output. Compare exact
scopes first, then resolved styles; a screenshot alone cannot identify which
layer differs. Theme reference checks and reports are described in the
[oracle guide](../tools/golden-oracle/README.md#themes).

## Known boundaries

The bundled final-output fixtures have no allowlisted divergences. Scanner-level
replay has a separate [difference ledger](../benchmarks/textmate/regex-execution-differences.json):
in some failed alternatives, the oracle retains dormant captures as empty
end-of-line ranges where Syntaxmate reports absent captures. The audited scanner
winners/full-match ranges agree. New or stale ledger entries fail the replay
check; passing final-output fixtures does not prove every low-level difference
unobservable. [Custom-grammar regressions](../tests/engine_regressions.rs) probe
capture interpolation, retokenization, and dynamic continuation separately.

Language-server semantic tokens, plist grammars, Shiki transformers, and stable
serialization of tokenizer state are outside the contract. Syntaxmate is not a
drop-in API replacement for Shiki or Syntect; Syntect uses Sublime syntax
instead of the same TextMate grammar format. [Competitive benchmarks](../benchmarks/competitors/README.md)
separate equivalent-grammar measurements from product comparisons.
