= Typst Fixture <intro>

Unicode café λ 東京 🚀 𝌆, *bold*, _italic_, and `raw`.
- Visit https://example.test/path?q=typst and see @intro.
1. Escapes: \# \[ \] \_ \u{03bb}, dashes -- ---, dots ..., tie~here.
/ Term: description with $sum_(i=1)^n i$ and :rocket:.

/* Multiline comment with nested /* inner 東京 */
   closes before code 🚀. */
#let greet(name, excited: true) = {
  let mark = if excited { "!" } else { "." }
  return "Hello, " + name + mark
}
#set text(size: 11pt)
#show heading: it => [*#it*]
#greet("café λ", excited: false)
#if true [Content block with #greet("東京 🚀 𝌆")]
#for item in ("one", "two") [#item ]
