// A broad Gleam tokenizer stress corpus; Unicode: 東京 λ 🚀 𝌆
import gleam/bool
import gleam/dict as dictionary
import gleam/int
import gleam/io
import gleam/list
import gleam/option.{None, Some}
import gleam/result
import gleam/string

pub type Priority {
  Low
  Normal
  High
  Urgent(code: Int)
}

pub type Delivery {
  Pending
  Sent(at: String, attempts: Int)
  Failed(reason: String)
}

pub type Envelope(a) {
  Envelope(id: Int, payload: a, priority: Priority, delivery: Delivery)
}

opaque type Token {
  Token(value: String)
}

pub const default_limit = 1_024
const permission_mask = 0b1111_0000
const unix_mode = 0o755
const color_code = 0xFF_80_20
const tiny_ratio = 6.02e-23
const exact_ratio = 12.500

pub fn new_token(value: String) -> Token {
  Token(value: value)
}

pub fn token_text(token: Token) -> String {
  let Token(value: value) = token
  value
}

pub fn priority_rank(priority: Priority) -> Int {
  case priority {
    Low -> 0
    Normal -> 1
    High -> 2
    Urgent(code: code) if code > 99 -> 4
    Urgent(..) -> 3
  }
}

pub fn make_envelope(id: Int, payload: a) -> Envelope(a) {
  Envelope(
    id: id,
    payload: payload,
    priority: Normal,
    delivery: Pending,
  )
}

pub fn mark_sent(envelope: Envelope(a), when: String) -> Envelope(a) {
  Envelope(
    ..envelope,
    delivery: Sent(at: when, attempts: 1),
  )
}

pub fn bump_attempts(envelope: Envelope(a)) -> Envelope(a) {
  case envelope.delivery {
    Sent(at: timestamp, attempts: count) -> Envelope(
      ..envelope,
      delivery: Sent(at: timestamp, attempts: count + 1),
    )
    Pending | Failed(..) -> envelope
  }
}

fn numeric_operators(left: Int, right: Int) -> Int {
  let added = left + right
  let subtracted = added - 3
  let multiplied = subtracted * 4
  let divided = multiplied / 2
  let remainder = divided % 7
  remainder
}

fn float_operators(left: Float, right: Float) -> Float {
  let added = left +. right
  let subtracted = added -. 1.25
  let multiplied = subtracted *. 4.0
  let divided = multiplied /. 2.0
  divided
}

fn compare_numbers(one: Int, two: Int, x: Float, y: Float) -> Bool {
  let integer_order = one < two && two <= 100 && two >= one && two > 0
  let float_order = x <. y && x <=. y && y >=. x && y >. 0.0
  integer_order == float_order || one != two
}

fn join_labels(prefix: String, suffix: String) -> String {
  prefix <> ":" <> suffix
}

fn unicode_banner() -> String {
  "東京 / λ / 🚀 / 𝌆 / escaped quote: \" / slash: \\"
}

fn classify(priority: Priority) -> String {
  case priority {
    Low -> "low"
    Normal -> "normal"
    High -> "high"
    Urgent(code: code) if code >= 500 -> "critical"
    Urgent(code: _) -> "urgent"
  }
}

fn guarded_delivery(delivery: Delivery) -> String {
  case delivery {
    Pending -> "waiting"
    Sent(at: at, attempts: tries) if tries == 1 -> "first:" <> at
    Sent(at: at, attempts: tries) if tries > 1 -> int.to_string(tries) <> at
    Failed(reason: message) -> "failed:" <> message
  }
}

fn transform(values: List(Int)) -> List(Int) {
  values
  |> list.filter(fn(value) { value >= 0 })
  |> list.map(fn(value) { value * value })
  |> list.reverse
}

fn nested_calls(values: List(String)) -> String {
  string.join(list.map(values, fn(item) { string.uppercase(item) }), ",")
}

fn dictionary_lookup(entries: Dict(String, Int), key: String) -> Int {
  entries
  |> dictionary.get(key)
  |> result.unwrap(0)
}

fn parse_positive(text: String) -> Result(Int, String) {
  use number <- result.try(int.parse(text))
  if number > 0 {
    Ok(number)
  } else {
    Error("not positive")
  }
}

fn require_value(value: Option(a)) -> a {
  case value {
    Some(inner) -> inner
    None -> panic as "missing value"
  }
}

fn assertions(actual: Int, expected: Int) -> Int {
  assert actual == expected
  assert Ok(actual) = Ok(expected)
  actual
}

fn placeholders(_unused: String, _also_unused: Int) -> Nil {
  let _discarded = "tokenized as unused"
  let _ = _discarded
  Nil
}

fn work_in_progress(flag: Bool) -> String {
  if flag {
    todo as "implement 東京 route"
  } else {
    panic as "launch 🚀 aborted"
  }
}

fn callback_factory(offset: Int) -> fn(Int) -> Int {
  fn(value) { value + offset }
}

fn logical_matrix(a: Bool, b: Bool) -> Bool {
  let conjunction = a && b
  let disjunction = a || b
  bool.exclusive_or(conjunction, disjunction)
}

fn namespaced_calls(message: String) {
  io.println(message)
  string.length(message)
  int.to_string(42)
}

fn pipeline_echo(value: String) -> String {
  value
  |> string.trim
  |> echo
  |> string.lowercase
}

fn assignment_and_arrows(input: Result(Int, String)) -> Int {
  use value <- result.try(input)
  let doubled = value * 2
  doubled
}

fn multiline_constructor(payload: String) -> Envelope(String) {
  Envelope(
    id: 99,
    payload: payload,
    priority: Urgent(code: 503),
    delivery: Failed(reason: "retry later"),
  )
}

// End state keeps comments, upper-case entities, and 0xAB_CD visible.
pub fn main() {
  let envelope = multiline_constructor(unicode_banner())
  let summary = guarded_delivery(envelope.delivery)
  io.println(summary)
}
