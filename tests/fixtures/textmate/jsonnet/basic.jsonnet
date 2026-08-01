// Compact Jsonnet grammar fixture: BMP café λ 雪 and astral 🚀 𝄞.
local greet(name) = "Hello, " + name;
local data = { visible: true, hidden:: null, forced::: 6.02E+23, merged+: .5 };
{
  title: "Mark café λ 雪 🚀 𝄞",
  escaped: "astral 🚀 then newline: \n, tab: \t, lambda: \u03BB",
  single: 'slash: \/, quote: \', backslash: \\, BMP λ',
  illegalEscape: "astral 🚀 then bad: \q",
  integer: 42,
  scientific: -12E-3,
  fraction: 17.25e+2,
  flags: [true, false, null],
  message: greet("世界 🚀"),
  size: std.length([1, 2, 3]),
  mapper: function(x) x * 2,
  choice: if data.visible && !false then self.title else error "missing",
  imported: importstr "banner.txt",
  note: |||
    Triple text keeps café and λ.
    Astral symbols stay literal: 🚀 𝄞.
  |||,
  // A slash comment after a triple string.
  # Hash comments receive the grammar's block-comment scope.
  /* A block comment spans lines.
     It also carries BMP 東京 and astral 😀.
  */
  fields: data,
}
