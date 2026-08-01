= Typst Stress Fixture <top>

Unicode inventory: café λ 東京 🚀 𝌆.
Markup has *bold text*, _italic text_, and *nested _emphasis_ safely*.
Escaped forms: \# \$ \[ \] \{ \} \_ \` \~ \u{03bb}.
Spacing uses a nonbreaking~space and an explicit linebreak\
Soft hyphen -? plus en dash --, em dash ---, and ellipsis ....
Visit https://example.test/a/b?q=typst&lang=en and return to @top.
Symbols include :rocket: :arrow.r: :alpha: and inline `let x = 1`.
Math is closed: $sum_(i=1)^n i = (n(n+1))/2$.

== Lists and references <lists>

- First bullet with *strong café*.
- Second bullet with _東京_ and @details.
  - Indented bullet with `raw 🚀`.
1. Numbered item one.
2. Numbered item two.
+ Auto-numbered item.
/ Term: a description-list value.
/ Unicode: λ and 𝌆 remain text.

=== Details <details>

The heading, labels <inline-label>, and references @inline-label are scoped.
// A line comment closes at its newline: { [ $ #
The URL https://typst.app/docs/reference/markup/ is not a comment.

/* Outer multiline comment begins.
   It contains /* nested comment café λ */ and resumes.
   Delimiters #let [ ] { } and astral 🚀 𝌆 stay comments.
   The outer comment closes here. */

#let title = "Grammar-driven café λ 東京 🚀 𝌆"
#let count = 7
#let ratio = 6.022e23
#let absent = none
#let automatic = auto
#let enabled = true
#let disabled = false
#let escaped = "quote: \" slash: \\ newline: \n tab: \t unicode: \u{1f680}"

#set text(size: 10.5pt)
#set page(width: 210mm, height: 29.7cm)
#set par(justify: true, leading: 1.2em)
#show heading: it => [#text(weight: "bold", it.body)]
#show link: underline

Numeric constants in markup interpolation:
- length #12pt, #2.54cm, #0.5in, #1.2em, and #8mm.
- angle #90deg and #3.14159rad.
- percent #62.5% and fraction #1fr.
- integers #0, #42, #1000 and float #3.5.

#let add(a, b: 2) = a + b
#let compare(a, b) = a == b or a != b and not false
#let relation(x) = x < 10 and x <= 10 and x > 0 and x >= 0
#let assignment = {
  let value = 1
  value += 2
  value -= 1
  value *= 3
  value /= 2
  value
}

#let choose(value) = {
  if value > 10 {
    [large: #value]
  } else if value == 10 {
    [exactly ten]
  } else {
    [small: #value]
  }
}

#let classify(value) = {
  let label = if value in (1, 2, 3) { "small" } else { "other" }
  return (value, label)
}

#let render(items) = {
  let output = []
  for item in items {
    output += [Item *#item*; ]
  }
  output
}

#let bounded(limit) = {
  let index = 0
  while index < limit {
    index += 1
    if index == 2 { continue }
    if index > 4 { break }
  }
  index
}

#let callback(value, apply: it => it) = apply(value)
#let spread-demo(values) = (0, ..values, 99)
#let dictionary = (name: "café", city: "東京", launched: true)
#let selected = dictionary.name

Function calls and argument scopes:
#add(4, b: 8)
#compare("λ", "λ")
#relation(9)
#choose(12)
#classify(2)
#render(("alpha", "beta", "🚀"))
#bounded(8)
#callback("𝌆", apply: value => [value=#value])
#spread-demo((1, 2, 3))

=== Content blocks

#let card(name, body) = [
  *#name*
  #body
]

#card("Multiline", [
  This content block spans lines.
  It mixes _markup_, $x^2 + y^2$, and @top.
  #if enabled [Enabled 🚀] else [Disabled]
])

#if enabled [
  Conditional content starts here.
  #for word in ("café", "λ", "東京", "𝌆") [
    - word: #word
  ]
] else [
  This branch is still lexically closed.
]

#for row in (1, 2, 3) [
  Row #row: #calc.sum(row, count)
]

#while false [This loop body is content.]
#break
#continue
#return

=== Imports and flow

#import "helpers.typ": helper as renamed
#include "chapter.typ"
#export title
#as
#in

#let module-code = {
  import "data.typ": dataset
  include "fragment.typ"
  export dataset
  let result = helper(dataset)
  // Code comment with https://example.test is closed here.
  return result
}

=== More markup

Plain #title and dotted #dictionary.city interpolate variables.
Line one ends with a hard break\
Line two contains \= escaped punctuation and :checkmark:.
Nested styles: *bold with _italic and `raw` inside_*.
Closed inline code: `#let fake = [not code]`.
Closed math one: $integral_0^1 x^2 dif x = 1/3$.
Closed math two: $vec(a) dot vec(b) <= norm(a) norm(b)$.

/* Final multiline state test:
   nested /* comments /* can */ nest */ correctly,
   and all levels close before EOF. */

==== Closure check <closure>

Everything references @closure, prints #title, and ends in ordinary markup.
Final Unicode: café λ 東京 🚀 𝌆.
