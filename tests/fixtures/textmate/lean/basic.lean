import Std

/-! A compact Lean 4 fixture: café, λ, 東京, 🚀, and 𝌆. -/
namespace TextMate.Basic

-- Ordinary line comment with `code` and a closed /- nested -/ marker.
/-- A documented structure with a Unicode field. -/
structure Greeting where
  café : String
  count : Nat := 0

inductive Mood where | calm | excited deriving Repr, DecidableEq

@[simp] def twice (n : Nat) : Nat := n + n
infixl:65 " ⊕ " => Nat.add

syntax:max "greet!(" term ")" : term
macro_rules | `(greet!($name)) => `(s!"Hello, {$name}! 東京 🚀 𝌆")

def literals : Nat × Float × Char × Char := (0x2A, 6.02e23, 'λ', '\n')
def message : String := greet!("café")

/- Outer block comment
   /- nested block comment with "strings" and 123 -/
   closes on this line. -/
theorem twice_eq (n : Nat) : twice n = n ⊕ n := by simp [twice]

#check Greeting.mk
#eval message
end TextMate.Basic
