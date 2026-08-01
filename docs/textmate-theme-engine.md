# TextMate theme highlighting

Syntaxmate preserves each token's complete TextMate scope stack and resolves
it against theme selectors. Changing a theme therefore does not retokenize
source files.

All named built-in themes use vendored TextMate rules. Package-backed themes
come from `github-vscode-themes@6.3.4` and `@shikijs/themes@3.23.0`; additional
families use their pinned upstream editor palettes or VS Code themes. Sources,
licenses, revisions, adaptations, and checksums are recorded in
`assets/themes/SOURCE.toml`. Release builds embed the JSON and do
not need Node, VS Code, network access, or files outside the binary.

## Comparison contract

Reference comparisons use `vscode-textmate@9.2.0` and
`vscode-oniguruma@1.7.0`, as pinned under `tools/golden-oracle`, with VS Code
semantic highlighting disabled. TextMate parity covers foreground,
background, bold, italic, underline, and strikethrough; it does not cover
language-server semantic tokens.

## Diff composition

Styles are composed in this order:

1. TextMate foreground and font modifiers;
2. token background on unchanged/context rows;
3. diff row background on additions and deletions;
4. inline-diff background and bold emphasis;
5. coarse user syntax-color overrides;
6. scope-aware `[[syntax_rules]]` overrides.

With transparent backgrounds enabled, token backgrounds are omitted. `system`,
ANSI, and user-provided Base16 schemes remain intentionally class-based because
they are terminal palettes rather than VS Code TextMate themes. Every named RGB
theme uses exact scope selectors.

Scope-aware overrides use the same selector parser and precedence rules:

```toml
[[syntax_rules]]
scope = "support.function"
foreground = "#91cbff"

[[syntax_rules]]
scope = "entity.name.function"
foreground = "#dbb7ff"
font_style = "bold"
```

Syntaxmate resolves themes against exact TextMate scope stacks. Historical
coarse-class comparison was an extraction-time validation aid and is not a
runtime mode or environment switch.

VS Code comparisons must disable semantic highlighting, bracket-pair
colorization, inlay hints, and editor link decorations. Those layers are not
TextMate theme output.

Run the complete non-CI parity contract locally with:

```sh
tools/check-textmate-parity.sh
```

Set `VSCODE_SOURCE=/path/to/vscode-1.128` to include direct canonical checks
against the pinned VS Code checkout.

The regression input
`tests/fixtures/textmate/latex/hw2-theme.tex` includes the
LaTeX scopes that originally exposed the lossy class-based rendering path.
