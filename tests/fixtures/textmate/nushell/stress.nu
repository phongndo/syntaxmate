# Stress fixture for Nushell grammar states; BMP λ 漢字 and astral 🦀 🪐 are intentional.

module toolkit {
  export const release = "2026.07"
  export const default_limit = 25
  export alias top-five = first 5

  export def greet [
    name: string
    --punctuation(-p): string = "!"
  ] -> string {
    $"Hello, ($name)($punctuation)"
  }

  export def summarize [
    rows: list<record<name: string, score: int>>
  ] -> record<count: int, best: int> {
    {
      count: ($rows | length)
      best: ($rows | get score | math max)
    }
  }

  export def normalize [value: any] -> string {
    $value | into string | str trim | str downcase
  }
}

module application {
  export use toolkit [greet summarize]

  export def banner [name: string] -> string {
    greet $name --punctuation "!!"
  }
}

use toolkit [greet summarize normalize]
use application banner

extern "demo-tool" [
  input?: path
  --count(-n): int
  --format(-f): string = "nuon"
  --verbose(-v)
  ...rest: string
]

def "score label" [score: int, --strict(-s)] -> string {
  match $score {
    90..100 => "excellent"
    75..<90 => "passing"
    _ => (if $strict { "rejected" } else { "review" })
  }
}

def select-active [
  rows: list<record<name: string, score: int, active: bool>>
  threshold: int = 80
  --columns(-c): list<string> = [name score]
] -> table {
  $rows
    | where active == true
    | filter {|row| $row.score >= $threshold }
    | select ...$columns
}

def inspect-kind []: [string -> record, list<any> -> record] {
  {kind: ($in | describe), value: $in}
}

def --wrapped proxy [command: string, ...arguments: string] {
  ^$command ...$arguments
}

alias newest = sort-by modified --reverse
const language = "Nushell"
let user = {name: "Ada", "display name": "Ada Lovelace"}
mut counter = 0
mut words = [alpha beta]

# Numeric forms, units, dates, and byte-stream literals.
let radial_values = [0xff 0o755 0b101101]
let quantities = [1_000 -42 3.1415 6.02e23 4kb 250ms 2day]
let observed_at = 2026-07-12T14:30:00Z
let bytes = 0x[de ad be ef 00 ff]
let constants = [true false null]

# Every quoting mode has content that can challenge a stateful tokenizer.
let single = 'single quoted λ text'
let double = "double quoted 漢字 with\nnewline,\ttab, \"quote\", and \\ slash"
let command_name = `name with spaces`
let raw = r###'Raw text keeps '#, "quotes", ($interpolation), λ, and 🪐.
This second raw line deliberately contains ## and a closing-looking '# token.
Only the matching delimiter on the next line closes it.'###
let multiline = "A double-quoted string may cross a line.
Its second line contains an astral symbol 🎛️ and closes here."
let salutation = $"($language) says hello to ($user.name)"
let possessive = $'User ($user."display name") explores strings'

# Lists, records, tables, computed keys, spreads, and nested cell paths.
let dynamic_key = "favorite color"
let base_profile = {
  name: $user.name
  "display name": $user."display name"
  address: {city: "London", coordinates: [51.5072 -0.1276]}
}
let profile = {
  ...$base_profile
  $"($dynamic_key)": "blue"
  tags: [math poetry computing]
}
let head = [zero one]
let tail = [two three]
let combined = [...$head ...$tail]
let city = $profile.address.city
let display = $profile."display name"
let matrix = [[name, score, active];
  ["Ada", 98, true]
  ["Grace", 95, true]
  ["Edsger", 88, false]
  ["Lin", 79, true]
]

# Inclusive, stepped, and exclusive ranges.
let inclusive = 1..5
let stepped = 2..2..10
let exclusive = 0..<5
let expanded_ranges = [$inclusive $stepped $exclusive]

# Symbolic, word, membership, regex, and bitwise operators.
let arithmetic = (((10 + 2) * 3 - 4) / 2)
let floor_and_power = [(17 // 3) (2 ** 8) (17 mod 5)]
let comparisons = (10 >= 5) and (3 != 4) and not (1 > 9)
let membership = ("nu" in [nu sh]) and ("fish" not-in [nu sh])
let boundaries = ("nushell" starts-with "nu") and ("fixture.nu" ends-with ".nu")
let patterns = ("abc42" =~ '[a-z]+\d+') and ("abc" !~ '^\d+$')
let wildcard = ("report.nu" like "*.nu") and ("notes.txt" not-like "*.nu")
let containment = ([a b c] has b) and ([a b c] not-has z)
let bits = ((0b1100 bit-and 0b1010) bit-or (1 bit-shl 3))
let alternatives = (true xor false) or (false and true)
let concatenated = ([1 2] ++ [3 4])
$counter += 1
$words ++= [gamma]

# Closures capture variables and carry typed parameter lists.
let prefix = "item"
let decorate = {|text: string, index: int| $"($prefix)-($index): ($text)" }
let decorated = $words | enumerate | each {|entry|
  do $decorate $entry.item $entry.index
}
let scaled = {|value: int, factor: int| $value * $factor }
let answer = do $scaled 7 6

# Pipelines exercise row conditions, filters, projection, sorting, and reduction.
let active = select-active $matrix 80 --columns [name score]
let ranked = $matrix
  | where score >= 75
  | sort-by score --reverse
  | each {|row| $row | upsert label (score label $row.score) }
let total = $ranked
  | get score
  | reduce --fold 0 {|score, sum| $sum + $score }
let grouped = $ranked | group-by active
let first_two = $ranked | take 2 | select name label

# Branching constructs keep nested braces, closures, and commands balanced.
let verdict = if $total > 300 {
  "large"
} else if $total == 300 {
  "exact"
} else {
  "small"
}

let status = match $counter {
  0 => {state: "idle", code: 200}
  1..10 => {state: "working", code: 202}
  _ => {state: "unknown", code: 500}
}

let parsed = try {
  "123" | into int
} catch {|error|
  $"parse failed: ($error.msg)"
}

for row in $first_two {
  print $"($row.name) => ($row.label)"
}

mut cursor = 0
while $cursor < 3 {
  $cursor += 1
  if $cursor == 2 { continue }
  print $"cursor=($cursor)"
}

mut attempts = 0
loop {
  $attempts += 1
  if $attempts < 2 { continue }
  break
}

# Environment assignment, internal variables, custom commands, and final values.
DEMO_MODE=fixture echo $env.DEMO_MODE
let nu_version = $nu.version
let normalized = normalize "  MIXED Case  "
let greeting = greet $display --punctuation "?"
let app_banner = banner "TextMate"
let summary = summarize ($matrix | select name score)
let kind = [λ 🦀] | inspect-kind

{
  greeting: $greeting
  banner: $app_banner
  normalized: $normalized
  verdict: $verdict
  status: $status
  parsed: $parsed
  total: $total
  answer: $answer
  summary: $summary
  kind: $kind
  city: $city
  observed: $observed_at
  bytes: $bytes
  constants: $constants
  ranges: $expanded_ranges
  grouped: $grouped
  decorated: $decorated
  raw: $raw
  multiline: $multiline
  salutation: $salutation
  possessive: $possessive
  command: $command_name
  radial: $radial_values
  quantities: $quantities
  arithmetic: $arithmetic
  operators: [$floor_and_power $comparisons $membership $boundaries $patterns]
  extras: [$wildcard $containment $bits $alternatives $concatenated]
}
