# Rendering

HTML and ANSI output preserve `HighlightStatus`. Check it when complete
highlighting is required. The examples below use default Cargo features and
are run by `cargo test --doc --all-features --locked`.

## HTML

Use `Highlighter::highlight_html` for default output. If you need a structured
document as well, render it separately:

```rust
use syntaxmate::{Highlighter, HtmlOptions, render_html};

let source = "const message = '<safe>';";
let mut highlighter = Highlighter::bundled()?;
let document = highlighter.highlight("typescript", source, "github-dark")?;
let html = render_html(source, &document, &HtmlOptions {
    include_scopes: true,
    ..HtmlOptions::default()
})?;
assert!(html.status().is_complete());
assert!(html.as_str().contains("&lt;safe&gt;"));
# Ok::<(), syntaxmate::Error>(())
```

Source text, wrapper classes, and scope attributes are escaped. Styles use
resolved RGB colors and fixed CSS properties. Set `include_wrapper: false` to
embed spans in an existing container. For options without a structured document,
use `Highlighter::highlight_html_with_options`.

## ANSI

```rust
use syntaxmate::Highlighter;

let mut highlighter = Highlighter::bundled()?;
let ansi = highlighter.highlight_ansi("rust", "let answer = 42;", "github-dark")?;
assert!(ansi.status().is_complete());
print!("{}", ansi.as_str());
# Ok::<(), syntaxmate::Error>(())
```

The renderer emits 24-bit SGR colors and font modifiers. Source control characters
are sanitized by default, including ESC; horizontal tabs and logical line breaks
are preserved. Disable `AnsiOptions::sanitize_control_characters` only for trusted
source. Use `render_ansi` for an existing structured document or
`Highlighter::highlight_ansi_with_options` for a single-call path.

## Source/document pairing

Pass the exact source used to produce the `HighlightedDocument`. The standalone
renderers reject invalid UTF-8 ranges and mismatched logical line counts, but do
not retain the original text and cannot detect every source mismatch. Different
text with compatible ranges can render successfully with incorrect highlighting.
The `Highlighter` convenience methods avoid this pairing problem by highlighting
and rendering the same source in one call.
