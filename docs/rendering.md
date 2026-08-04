# Rendering

Syntaxmate exposes structured highlighted spans first. The built-in HTML and
ANSI renderers are dependency-free convenience layers over the same output.
Both preserve `HighlightStatus`; callers must still reject or disclose
`Degraded` output when completeness is required.

## HTML

```rust
use syntaxmate::{Highlighter, HtmlOptions, render_html};

let source = "const message = '<safe>';";
let mut highlighter = Highlighter::bundled()?;
let document = highlighter.highlight("typescript", source, "github-dark")?;
let html = render_html(
    source,
    &document,
    &HtmlOptions {
        include_scopes: true,
        ..HtmlOptions::default()
    },
)?;
assert!(html.as_str().contains("&lt;safe&gt;"));
# Ok::<(), syntaxmate::Error>(())
```

The default wrapper is `<pre class="syntaxmate"><code>`. Source text, wrapper
classes, and `data-scopes` values are escaped. Styles contain only resolved RGB
colors and fixed CSS properties. Set `include_wrapper` to `false` when embedding
spans into an existing code container.

`Highlighter::highlight_html` combines highlighting and default rendering.
`Highlighter::highlight_html_with_options` provides the same compact-token
path with caller options. These convenience methods render directly from the
engine's compact scope-stack spans instead of first allocating public token and
styled-document copies. `render_html` remains available unchanged when a
structured `HighlightedDocument` is already needed.

## ANSI

```rust
use syntaxmate::Highlighter;

let mut highlighter = Highlighter::bundled()?;
let ansi = highlighter.highlight_ansi("rust", "let answer = 42;", "github-dark")?;
print!("{}", ansi.as_str());
# Ok::<(), syntaxmate::Error>(())
```

The ANSI renderer emits 24-bit SGR foreground/background colors and bold,
italic, underline, and strikethrough modifiers. It resets style at line
boundaries and at the end of styled runs. `Highlighter::highlight_ansi` and
`highlight_ansi_with_options` use the direct compact-token path; `render_ansi`
continues to render an existing structured document.

`AnsiOptions::sanitize_control_characters` defaults to `true`. This replaces
source control characters—including ESC—with visible control pictures, so
untrusted source cannot inject terminal commands. Disable sanitization only
when exact byte display is more important and the source is trusted.

## Source/document pairing

The standalone renderers validate every UTF-8 byte range and logical line count
before slicing. Pass the exact source used to produce the `HighlightedDocument`.
Supplying a document from different source returns `Error::Render`. The direct
`Highlighter` methods consume compact output produced from the source in the
same call and retain debug assertions for the engine's range invariants.
