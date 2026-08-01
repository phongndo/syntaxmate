# TextMate Stress – café λ🚀

Paragraph with `inline code`, **strong _nested emphasis_**, and a
[link](https://example.com/search?q=%CE%BB).

> Block quote with nested punctuation:
> - item one
> - item two with `code`

```rust
/* outer markdown rust fence comment
   /* nested rust comment */
   still outer
*/
fn main() {
    println!("hello λ🚀");
}
```

```js
const route = /^\/api\/[\p{Letter}-]+$/u;
const quotient = 42 / 7 / 2;
const template = `multi-line ${route.test("/api/café")}`;
```

~~~markdown
Nested-looking fence text kept inside a tilde fence:
```text
not the outer fence terminator
```
~~~

## Inline forms and punctuation

A realistic sentence escapes \*literal stars\*, \[brackets\], and a backslash \\
while preserving entities such as &amp;, &copy;, &#955;, and &#x1F680;.

Visit <https://example.org/docs?q=markdown> or write to
<maintainer@example.org>. Mix *gentle emphasis*, **clear importance**,
***both at once***, ~~retired wording~~, and `printf("café 🚀")` safely.

Use ``code containing a ` backtick`` beside H~2~O-like prose, and keep
underscores in snake_case_words from becoming accidental decoration.

Setext section with Unicode 雪 and 𝄞
====================================

The renderer should retain smart-looking punctuation—en dashes, “quotes,”
ellipses… and astral symbols 😀 𐐷—without changing source offsets.

Lists and checkpoints
---------------------

1. Install the parser.
2. Run the fixture suite.
   1. Compare token scopes.
   2. Confirm the final rule stack.
3. Publish the report.

- Fruit
  - café-grown beans
  - yuzu and pear
- Tools
  * parser
  * highlighter
    + snapshot viewer

- [x] Preserve the original sample.
- [ ] Review reference links.
- [ ] Verify emoji widths 🚀.

> ### Quoted release note
>
> The migration keeps **existing behavior** and documents `--dry-run`.
>
> 1. Back up the configuration.
> 2. Apply the update.
>    - If validation fails, restore the backup.
>
> > A nested reviewer adds: “Check café paths and 東京 labels.”
>
> The quote closes before ordinary prose resumes.

Links, media, and notes
-----------------------

Read the [syntax guide][guide], open the [project home][], or inspect
[an inline destination](https://example.net/a_(b) "Balanced title").

![A telescope pointed at a violet nebula](https://example.org/nebula.png "Night sky")

The compact form [guide] is useful, as is this footnote-like marker[^offsets].
Another sentence carries a conventional note.[^unicode]

[^offsets]: This fixture treats footnote syntax as realistic Markdown text.
    Continuation text mentions byte offsets and code points.
[^unicode]: BMP λ and astral 🚀 should both remain intact.

[guide]: https://example.org/guide "Syntax guide"
[project home]: https://example.org/
[badge]: https://example.org/badge.svg "passing"
[ci]: https://example.org/actions

---

### Compact comparison table

| Construct | Example | Expected state |
|:----------|:--------|---------------:|
| Entity | `&amp;` | root |
| Unicode | café 雪 🚀 | root |
| Escape | `\*plain\*` | root |
| Link | `[guide][guide]` | root |

### Embedded JSON

```json
{
  "name": "mark",
  "enabled": true,
  "glyphs": ["λ", "雪", "🚀"],
  "limits": { "minimum": 1, "maximum": 200 }
}
```

### Embedded CSS

```css
:root {
  --accent: #7c3aed;
  --label: "café 🚀";
}
.card:hover > .title {
  color: var(--accent);
  font-weight: 700;
}
```

### Embedded shell session

```bash
#!/usr/bin/env bash
set -euo pipefail
name="café"
printf 'checking %s %s\n' "$name" "🚀"
for file in README.md docs/guide.md; do
  test -f "$file" && echo "$file"
done
```

### Embedded Python

```python
from pathlib import Path
labels = ["café", "東京", "🚀"]
for index, label in enumerate(labels, start=1):
    print(f"{index}: {label}")
exists = Path("README.md").exists()
print({"readme": exists, "count": len(labels)})
```

### Uninterpreted fenced text

```text
Literal Markdown stays literal here: **not strong** and [not a link](nowhere).
A shorter fence marker like `` does not terminate this block.
Unicode remains ordinary text: naïve, Ω, 𝄞, and 🛰️.
```

HTML and code blocks
--------------------

<!-- A closed HTML comment can mention <tags>, **Markdown**, and café 🚀. -->

<section class="release" data-version="3">
  <h3>Rendered release summary</h3>
  <p>Status: <strong>stable</strong>; locale: 日本語; orbit: 🛰️.</p>
  <details>
    <summary>Compatibility</summary>
    <p>Links remain available at <a href="https://example.org">example.org</a>.</p>
  </details>
</section>

The following legacy snippet uses four-space indentation rather than fences:

    $ mark check README.md
    checking headings, links, and entities
    result = {"ok": true, "warnings": 0}

Paragraphs after indented code return to ordinary Markdown. A hard break follows\
this clause, while the next sentence uses a normal soft wrap.

Final text confirms all comments, HTML elements, links, emphasis spans, code
spans, and fenced blocks above are deliberately closed at end of file.
