# Basic Nushell fixture: λ 漢字 and astral 🦀 in a comment.
let title = `TextMate basic fixture`
let raw_note = r#'Raw "quotes" stay literal.
Second line keeps λ and 🚀 untouched.'#

def greet [name: string, --loud(-l)] -> string {
  let text = $"Hello, ($name)! λ"
  if $loud { $text | str upcase } else { $text }
}

let numbers = [42 0xff 0o755 0b1010 3.14 2kb 250ms]
let profile = {name: "Ada", active: true, note: 'compiler pioneer'}
let people = [[name, score];
  ["Ada", 98]
  ["Grace", 95]
  ["Lin", 79]
]
let adjust = {|score: int| $score + 1 }
let passing = $people
  | where score >= 80
  | each {|row| {name: $row.name, score: (do $adjust $row.score)} }

for row in $passing {
  print $"($row.name): ($row.score) — ($title)"
}

greet $profile.name --loud
