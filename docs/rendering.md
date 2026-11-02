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

Set `class_prefix` to render theme-independent scope classes. Render once and
switch themes by replacing the stylesheet, keeping the same prefix:

```rust
use syntaxmate::{Highlighter, HtmlOptions, Theme, html_stylesheet, render_html};

let source = "fn main() {}";
let highlighter = Highlighter::bundled()?;
let document = highlighter.highlight("rust", source, "github-dark")?;
let options = HtmlOptions {
    class_prefix: Some("code".to_owned()),
    ..HtmlOptions::default()
};
let html = render_html(source, &document, &options)?;
let dark_css = html_stylesheet(&Theme::bundled("github-dark")?, "code");
let light_css = html_stylesheet(&Theme::bundled("github-light")?, "code");
assert!(!html.as_str().contains("style="));
assert_ne!(dark_css, light_css);
# Ok::<(), syntaxmate::Error>(())
```

Include one stylesheet in your page, or use media queries to select between
them. Scope classes contain no theme colors. Nested spans preserve the ordered
scope stack, and adjacent tokens share their outer spans within each line.
One encoded class per scope preserves atom order and repeated atoms without
repeating the prefix for every atom. CSS attribute prefix matching also avoids
emitting a separate class for every dotted prefix of a scope. For example,
`keyword.control.rust` becomes `sm-code-s-keyword-control-rust`, and a theme's
`keyword.control` selector matches it with `[class|="sm-code-s-keyword-control"]`.
Literal hyphens and other punctuation are encoded so they cannot masquerade as
scope separators or inject HTML/CSS. See
[`html_stylesheet`](https://docs.rs/syntaxmate/latest/syntaxmate/fn.html_stylesheet.html)
for the exact encoding and supported selector subset.

The wrapper receives `sm-code-root` plus the existing `class` option. When
`include_wrapper` is false, add `sm-code-root` to your own container to supply
defaults and anchor the selectors. Keep generated scope spans' classes intact;
adding classes to them would break the attribute matching. Plain inner spans
apply font modifiers to text, so nested scopes can clear underline and
strikethrough without an ancestor continuing to decorate them.

Parent selectors become CSS descendants, and comma lists become equivalent
separate rules. Unsupported selectors are skipped as documented in the API.
Zero-specificity `:where()` rules follow TextMate rank order; CSS inheritance
then applies scope levels from outer to inner. This is an approximation for
custom themes, especially unsupported selectors or interfering page styles.
Class output requires CSS custom properties and `:where()` support, and is
larger than the default inline output because it retains the scope structure.
Use inline output when exact resolution or compact HTML matters more than
switching stylesheets.

The regression measurement parses generated HTML and CSS and emulates the
emitted cascade, comparing foreground, effective background, and all font
modifiers for each Unicode character with inline resolution. It covers every
bundled theme and the Rust, TSX, HTML, Markdown, PHP, and CSS stress fixtures,
and prints fidelity and uncompressed byte sizes:

```sh
cargo test --all-features --locked bundled_css_fidelity_and_size -- --nocapture
```

### Multiple themes

Use `render_html_themes` to render light and dark (or any number of) themes in
one pass, with each theme resolved exactly. The first theme renders as normal
inline styles. Each additional theme adds inherited custom properties per run:
`--sm-{name}` (color), `--sm-{name}-bg` (background), and `--sm-{name}-fw`,
`-fs`, and `-td` (weight, style, and decoration):

```rust
use syntaxmate::{Highlighter, HtmlOptions, Theme, render_html_themes};

let source = "fn main() {}";
let highlighter = Highlighter::bundled()?;
let document = highlighter.tokenize("rust", source)?;
let light = Theme::bundled("github-light")?;
let dark = Theme::bundled("github-dark")?;
let html = render_html_themes(
    source,
    &document,
    &[("light", &light), ("dark", &dark)],
    &HtmlOptions::default(),
)?;
assert!(html.as_str().contains("--sm-dark:#"));
# Ok::<(), syntaxmate::Error>(())
```

Switch themes with CSS alone, here following the system color scheme:

```css
@media (prefers-color-scheme: dark) {
  .syntaxmate,
  .syntaxmate span {
    color: var(--sm-dark) !important;
    background-color: var(--sm-dark-bg) !important;
    font-weight: var(--sm-dark-fw) !important;
    font-style: var(--sm-dark-fs) !important;
    text-decoration: var(--sm-dark-td) !important;
  }
}
```

With the wrapper, runs omit values equal to a theme's defaults and inherit
them from `<pre>`, which also resets each theme's font properties. Without the
wrapper, every run carries every value. Unlike class mode, no stylesheet
approximates the theme selectors, so custom themes render exactly. The output
grows with each extra theme and does not support `class_prefix`.

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
