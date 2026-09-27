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
let highlighter = Highlighter::bundled()?;
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
resolved RGB colors and fixed CSS properties. Default foreground and background
colors appear once on `<pre>`. Runs omit those inherited colors and need no span
unless they have font modifiers or `include_scopes` needs a `data-scopes`
attribute. Adjacent runs with identical output merge within each logical line;
with `include_scopes`, their scope attributes must also match. Font modifiers
stay on runs: putting underline or strikethrough on an ancestor would prevent
tokens from clearing those decorations.

Set `include_wrapper: false` to embed spans in an existing container. In this
mode, runs retain their full colors, including theme defaults, so embedding does
not require matching container styles. For options without a structured
document, use `Highlighter::highlight_html_with_options`.

### CSS classes

Set `class_prefix` to emit classes instead of inline styles. Generate the
stylesheet once from the same theme, using the same prefix:

```rust
use syntaxmate::{Highlighter, HtmlOptions, Theme, html_stylesheet, render_html};

let source = "fn main() {}";
let theme = Theme::bundled("github-dark")?;
let highlighter = Highlighter::bundled()?;
let document = highlighter.highlight_with_theme("rust", source, &theme)?;
let options = HtmlOptions {
    class_prefix: Some("code".to_owned()),
    ..HtmlOptions::default()
};
let css = html_stylesheet(&theme, "code");
let html = render_html(source, &document, &options)?;
assert!(!html.as_str().contains("style="));
assert!(css.contains(".sm-code-"));
# Ok::<(), syntaxmate::Error>(())
```

Include the CSS in your page or an external stylesheet. Classes describe
resolved color properties and font modifiers, so combinations inherited from
multiple TextMate rules work without converting scope selectors into CSS.
Scope and theme names never enter class names. The prefix is encoded into a
safe CSS identifier; see [`html_stylesheet`](https://docs.rs/syntaxmate/latest/syntaxmate/fn.html_stylesheet.html)
for the naming scheme. The existing `class` option still adds a class to `<pre>`.

## ANSI

```rust
use syntaxmate::Highlighter;

let highlighter = Highlighter::bundled()?;
let ansi = highlighter.highlight_ansi("rust", "let answer = 42;", "github-dark")?;
assert!(ansi.status().is_complete());
print!("{}", ansi.as_str());
# Ok::<(), syntaxmate::Error>(())
```

The renderer emits 24-bit SGR colors and font modifiers. By default it preserves
the terminal background, except for token backgrounds that differ from the
theme default. Set `AnsiOptions::include_default_background` to paint the theme
background too. Equal adjacent styles share SGR sequences; styles reset at
logical line boundaries and at the end of output.

Source control characters
are sanitized by default, including ESC; horizontal tabs and logical line breaks
are preserved. Disable `AnsiOptions::sanitize_control_characters` only for trusted
source. Use `render_ansi` for an existing structured document or
`Highlighter::highlight_ansi_with_options` for a single-call path.

## Streaming

`render_html_to` and `render_ansi_to` accept a `fmt::Write` sink and return
`Result<HighlightStatus>`. The String-returning renderers delegate to these
functions. A sink can forward chunks or count bytes without retaining the
complete rendered string:

```rust
use std::fmt;
use syntaxmate::{Highlighter, HtmlOptions, render_html_to};

struct ByteCount(usize);
impl fmt::Write for ByteCount {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.0 += text.len();
        Ok(())
    }
}

let source = "fn main() {}";
let highlighter = Highlighter::bundled()?;
let document = highlighter.highlight("rust", source, "github-dark")?;
let mut sink = ByteCount(0);
let status = render_html_to(source, &document, &HtmlOptions::default(), &mut sink)?;
assert!(status.is_complete());
assert!(sink.0 > source.len());
# Ok::<(), syntaxmate::Error>(())
```

The document is still retained; streaming avoids allocating the complete
rendered output. Source validation finishes before the first write. A writer
failure returns `Error::Render` and may leave partial output, including an active
ANSI style. Renderers do not flush or roll back the sink.

## Source/document pairing

Pass the exact source used to produce the `HighlightedDocument`. The standalone
renderers reject invalid UTF-8 ranges and mismatched logical line counts, but do
not retain the original text and cannot detect every source mismatch. Different
text with compatible ranges can render successfully with incorrect highlighting.
The `Highlighter` convenience methods avoid this pairing problem by highlighting
and rendering the same source in one call.
