# Binding conformance fixtures

`cases.json` holds shared inputs; `expected.json` holds the reference output
from `syntaxmate-boundary`. Every binding runs each case and must match it
exactly, which catches offset-unit and style-encoding drift.

A case with `"theme": null` uses `customTheme` from `cases.json`. Expected
output per case:

- `html`: default HTML options; `htmlClasses`: class prefix `sm`.
- `ansi`: default ANSI options.
- `tokens.utf8`, `tokens.utf16`, `tokens.codePoint`: whole-document token
  buffers with `includeScopes` false.
- `scopes`: the UTF-8 buffer's `tokenScopes` and `scopeStacks` with scopes on.
- `session`: one UTF-16 buffer per `\n`-separated line from a single session.

Styles are `[foreground, background, modifiers]`, where colors are
`0xRRGGBB` integers or `4294967295` for none.

Regenerate after an intentional engine change:

```sh
cd bindings && cargo run -p syntaxmate-boundary --example conformance -- --write
```

`--check` (the default) fails if `expected.json` is stale.
