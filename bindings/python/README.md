# syntaxmate for Python

Python bindings for [Syntaxmate](../../README.md), a fast Rust syntax
highlighter powered by TextMate grammars. Highlight to HTML, 24-bit ANSI, or
flat token arrays, with the full grammar catalog and GitHub themes embedded.

- One `abi3` wheel per platform supports CPython 3.11 and newer. Free-threaded
  builds (such as 3.14t) cannot load `abi3` extensions.
- Highlighting releases the GIL; a `Highlighter` is safe to share across threads.
- Token offsets index Python `str` values directly (Unicode code points).

## Install

Wheels are not published yet. Build one from a checkout as described in
[Build from source](#build-from-source), then install it:

```sh
pip install bindings/target/wheels/syntaxmate-*.whl
```

## Usage

```python
import syntaxmate

hl = syntaxmate.Highlighter()  # reuse one instance; it caches prepared grammars

html = hl.html('fn main() { println!("<hi>"); }', "rust", "github-dark")
print(hl.ansi("print('hi')", "python"))  # the theme defaults to github-dark

hl.languages()                      # canonical IDs, e.g. "python", "rust"
hl.themes()                         # bundled theme names
hl.canonical_language("py")         # "python"
hl.detect(path="setup.py")          # "python"
hl.detect("#!/bin/sh\necho hi\n")   # "shellscript", from the first line
```

### Themes and CSS classes

`theme` accepts a bundled theme name or a `Theme`:

```python
from syntaxmate import Theme

dark = Theme.bundled("github-dark")
custom = Theme.from_json({"name": "Mine", "tokenColors": [...]})  # also str or bytes

# Class mode: render once, switch themes by swapping stylesheets.
html = hl.html(code, "rust", class_prefix="sm")
css = dark.stylesheet("sm")
```

HTML options (keyword-only) are `include_wrapper`, `wrapper_class`,
`include_scopes` (adds `data-scopes`), and `class_prefix`. ANSI options are
`colors`, `sanitize_control_characters` (on by default so untrusted source
cannot inject escape sequences), and `include_default_background`. See
[rendering](../../docs/rendering.md) for their exact behavior.

### Tokens

`tokens()` returns flat arrays instead of one Python object per token. Each
array is a read-only `memoryview` of unsigned 32-bit integers, so it indexes
like a list, converts with `.tolist()`, and works with `numpy.frombuffer`.

```python
code = 'let s = "😀";\nfn f() {}'
tokens = hl.tokens(code, "rust", include_scopes=True)

for i in range(len(tokens)):
    start, length = tokens.starts[i], tokens.lengths[i]
    style = tokens.styles[tokens.style_ids[i]]  # Style: foreground, bold, ...
    print(code[start : start + length], hex(style.foreground or 0))

# Convenience iteration builds a Token per item.
for token in tokens:
    print(token.start, token.end, token.style, token.scopes)
```

- Lines split on `\n` only; `line_starts[l]` is the offset of line `l`, and its
  tokens are `line_token_ranges[l]` up to `line_token_ranges[l + 1]`.
- `styles` holds the distinct styles; colors are `0xRRGGBB` integers or `None`,
  and `modifiers` is a bitset of `BOLD`, `ITALIC`, `UNDERLINE`, `STRIKETHROUGH`.
- `scope_ids` and `scope_stacks` are filled only with `include_scopes=True`.
- `complete` is false if tokenization stopped at a resource limit.
- `unit="utf8"` or `unit="utf16"` reports offsets in bytes or UTF-16 code units.

### Incremental sessions

```python
session = hl.session("python", "github-dark")
for line in source.split("\n"):
    tokens = session.line(line)  # offsets are relative to the line
session.reset()                  # back to the start-of-document state
```

A session carries state between lines, so feed lines in order. Calls on one
session from several threads are serialized.

### Errors

Failures raise a `SyntaxmateError` subclass whose `kind` attribute is a stable
category string:

| Exception | `kind` | Also a |
| --- | --- | --- |
| `UnknownLanguageError` | `"unknown_language"` | `LookupError` |
| `UnknownThemeError` | `"unknown_theme"` | `LookupError` |
| `InvalidGrammarError` | `"invalid_grammar"` | `ValueError` |
| `InvalidThemeError` | `"invalid_theme"` | `ValueError` |
| `InvalidBundleError` | `"invalid_bundle"` | `ValueError` |
| `InvalidInputError` | `"invalid_input"` | `ValueError` |
| `RenderError` | `"render"` | |
| `InternalError` | `"internal"` | |

These are the same categories as the JavaScript `kind` values and C status
codes, spelled in Python style. `InternalError` includes Rust panics caught at
the boundary. After such a panic, a
session raises `InternalError` until `reset()` is called.

Text arguments must be encodable as UTF-8: a `str` containing a lone surrogate
such as `"\ud800"` raises the standard `UnicodeEncodeError`, a `ValueError`.

### Subset bundles

To ship fewer grammars, build a bundle with the
[subset-bundle command](../../docs/assets.md#custom-and-subset-bundles) and load it:

```python
hl = syntaxmate.Highlighter.from_bundle(open("grammars-subset.bundle", "rb").read())
```

A wheel built with `--no-default-features` omits the embedded catalog, so only
`from_bundle` works.

## Build from source

Run from `bindings/python` with Rust 1.88 or newer and
[maturin](https://www.maturin.rs/) 1.9.4 or newer (for example
`pip install "maturin>=1.9.4"` in a virtualenv). Inside this repository,
`rust-toolchain.toml` selects the pinned Rust version. The sdist does not
include that file, so `pip install` of the sdist uses your default toolchain;
with rustup, make sure one is set (`rustup default stable`). On the
repository's Nix setup, `nix develop` provides Rust and Python.

```sh
maturin build --release --strip                        # wheel in bindings/target/wheels
maturin build --release --strip --no-default-features  # without embedded grammars
maturin develop --release                              # install into the active venv
python -m pytest                                       # needs pytest
```

Measured on Linux x86-64 (2026-09-29, Rust 1.98.1, stripped release build): the
default wheel is 2.9 MB (4.6 MB extension), and the wheel without embedded
grammars is 1.0 MB.

Wheels carry the license and provenance records of the bundled themes and
grammars under `*.dist-info/licenses/third-party/`. `third-party/` holds
symlinks into `assets/` because maturin rejects `../` paths in `license-files`;
the sdist stores their contents.

The tests include the shared [conformance fixtures](../conformance/README.md):
every case must match the reference HTML, class-mode HTML, ANSI, token buffers
in all three offset units, scopes, and session output exactly.

## Benchmark

[`bench/bench_html.py`](bench/bench_html.py) compares HTML throughput with
Pygments on the stress fixtures used by the
[competitive benchmarks](../../benchmarks/competitors/README.md). It needs this
package and Pygments installed in the same interpreter:

```sh
python bench/bench_html.py --samples 5 --json /tmp/bench.json
```

Pygments uses its own lexers and emits different HTML, so this compares
products, not identical output. Syntaxmate's `replay` mode benefits from its
line-result cache on an unchanged document; `steady` rotates document variants
to defeat that cache.
