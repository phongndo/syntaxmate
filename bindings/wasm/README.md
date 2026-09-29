# syntaxmate for JavaScript

TextMate syntax highlighting to HTML, ANSI, or flat token arrays, compiled from
the Rust [Syntaxmate](../../README.md) engine to WebAssembly. It runs in Node
and in browsers as an ES module.

Grammars are not compiled into the `.wasm`. The package ships the full catalog
as a separate `grammars.bundle` file, which you load at runtime. That keeps the
WebAssembly module small and lets you ship a smaller subset bundle instead.

## Install

```sh
npm install syntaxmate
```

This package is not yet published; see [Build](#build) to produce it locally.

## Node

Importing the package instantiates WebAssembly synchronously.

```js
import { createHighlighter } from 'syntaxmate'

const highlighter = await createHighlighter() // reads the bundled grammars.bundle
const html = highlighter.html('fn main() {}', { lang: 'rust', theme: 'github-dark' })
```

For fully synchronous setup, use `Highlighter.fromBundle(loadBundleSync())`.
Pass a path to `loadBundle`, `loadBundleSync`, or `createHighlighter({ bundle })`
to load a different bundle.

## Browsers and bundlers

The browser entry point must be initialized once. `createHighlighter()` fetches
the `.wasm` and `grammars.bundle` that sit beside the module, using
`new URL(..., import.meta.url)`, which Vite, webpack 5, and plain ES modules
understand.

```js
import { createHighlighter } from 'syntaxmate'

const highlighter = await createHighlighter()
```

To serve the files elsewhere, pass their locations or bytes:

```js
import { createHighlighter } from 'syntaxmate'

const highlighter = await createHighlighter({
  wasm: '/assets/syntaxmate.wasm',
  bundle: '/assets/grammars.bundle',
})
```

The files are exported as `syntaxmate/syntaxmate.wasm` and
`syntaxmate/grammars.bundle`. Lower-level `init(wasm)` and `loadBundle(url)`
are also exported. The full bundle is about 2.5 MB; ship a subset when you
need only a few languages.

## API

The TypeScript declarations in [lib/core.d.ts](lib/core.d.ts) are the reference.

- `Highlighter.fromBundle(bytes)`: decodes a grammar bundle.
  - `html(code, { lang, theme, includeWrapper, class, includeScopes, classPrefix })`
  - `ansi(code, { lang, theme, colors, sanitizeControlCharacters, includeDefaultBackground })`
  - `tokens(code, { lang, theme, includeScopes })` returns a `TokenBuffer`.
  - `session({ lang, theme, includeScopes })` returns a `Session`.
  - `languages()`, `themes()`, `canonicalLanguage(idOrAlias)`, and
    `detect({ path, source })`.
- `Theme.bundled(name)` and `Theme.fromJson(jsonStringOrObject)`, with `name`,
  `defaultStyle`, and `stylesheet(classPrefix)`. `theme` options accept a
  `Theme` or a bundled theme name; names are resolved once per highlighter.
- `Session.line(text)` highlights the next line, passed without its `\n`, and
  returns a one-line `TokenBuffer` with line-relative offsets. `reset()` returns
  to the start-of-document state.
- `version()` returns the engine version.

### Tokens

`tokens()` returns flat `Uint32Array`s rather than one object per token.
Offsets and lengths are UTF-16 code units, so they index JavaScript strings
directly. Lines split on `\n` only.

```js
const t = highlighter.tokens(code, { lang: 'ts', theme: 'github-light' })
for (let line = 0; line < t.lineCount; line++) {
  for (let i = t.lineTokenRanges[line]; i < t.lineTokenRanges[line + 1]; i++) {
    const text = code.slice(t.tokenStarts[i], t.tokenStarts[i] + t.tokenLengths[i])
    const { foreground, modifiers } = t.styles[t.tokenStyles[i]]
  }
}
```

Styles are deduplicated in `styles`. Colors are `0xRRGGBB` integers, or `null`
when unset; `modifiers` is a bitset of `BOLD`, `ITALIC`, `UNDERLINE`, and
`STRIKETHROUGH`. Text not covered by a token uses `defaultStyle`. With
`includeScopes`, `tokenScopes[i]` indexes `scopeStacks`, which lists scopes
outermost first. Iterating a `TokenBuffer` yields token objects for convenience;
that allocates per token.

### Class-mode HTML

With `classPrefix`, HTML carries theme-independent scope classes instead of
inline styles. Switch themes by swapping CSS, without highlighting again:

```js
const html = highlighter.html(code, { lang: 'rust', theme: 'github-dark', classPrefix: 'sm' })
const css = Theme.bundled('github-light').stylesheet('sm')
```

### Errors

Engine failures throw `SyntaxmateError`, an `Error` with a stable `kind`
(`UnknownLanguage`, `UnknownTheme`, `InvalidGrammar`, `InvalidTheme`,
`InvalidBundle`, `InvalidInput`, `Render`, or `Internal`) and the matching
numeric `code` shared with the other bindings. Missing required options throw
`TypeError`.

A Rust panic aborts the WebAssembly instance and surfaces as a
`WebAssembly.RuntimeError`; discard the module after one.

### Memory

`Highlighter`, `Theme`, and `Session` own WebAssembly memory. It is reclaimed
at garbage collection, but you can release it deterministically with `free()`
or a `using` declaration. `free()` is idempotent, and later use throws.
`TokenBuffer`s are plain JavaScript and need no freeing.

```js
{
  using session = highlighter.session({ lang: 'python', theme: 'github-dark' })
  for (const line of lines) render(session.line(line))
}
```

## Subset bundles

Build a smaller bundle with the `syntaxmate-bundle` tool described in
[asset docs](../../docs/assets.md#custom-and-subset-bundles), using the same
Syntaxmate version, then pass its bytes, path, or URL as `bundle`.

## Build

The package builds with `wasm-bindgen-cli`, not `wasm-pack`, so the output
layout is explicit. From the repository root, the `wasm` Nix shell provides the
pinned toolchain with the `wasm32-unknown-unknown` target, a matching
`wasm-bindgen-cli`, `binaryen`, and Node:

```sh
nix develop .#wasm -c sh -c 'cd bindings/wasm && npm run build && npm test'
```

Without Nix, install the target (`rustup target add wasm32-unknown-unknown`),
`wasm-bindgen-cli` at the `wasm-bindgen` version in
[Cargo.toml](Cargo.toml), and optionally `wasm-opt`.

`npm run build` runs [scripts/build.mjs](scripts/build.mjs). It compiles with
the `wasm-release` profile, generates `dist/` with `wasm-bindgen --target web`,
optimizes with `wasm-opt -O3` when available, and copies `grammars.bundle` and
license files into the package. `npm pack` then produces the tarball.

`npm test` runs the API tests and the shared
[conformance cases](../conformance/README.md) with `node --test`.

## Benchmark

[bench/bench.mjs](bench/bench.mjs) compares HTML output against the pinned Shiki
in [benchmarks/competitors](../../benchmarks/competitors/README.md), on the
same stress fixtures and theme:

```sh
npm ci --prefix ../../benchmarks/competitors --ignore-scripts
npm run bench -- --samples 7 --out ../target/wasm-bench.json
```

Each sample runs in a fresh process. `cold` covers import, setup, and the first
call. `steady` rotates through 16 copies of each fixture that differ only in
trailing spaces, so Syntaxmate's per-line result cache cannot hit. `replay`
repeats one document, which that cache serves; Shiki has no equivalent. Results
apply only to the machine and revisions measured.

## License

MIT. The grammars and themes in `grammars.bundle` come from upstream projects
under their own licenses; the built package carries their source pins, license
records, and notices in `third-party/`, copied from the repository's
[third-party asset records](../../THIRD_PARTY_LICENSES.md).
