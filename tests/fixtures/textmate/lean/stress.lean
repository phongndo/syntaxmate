import Std
import Std.Data.HashMap
import Std.Tactic

/-!
# Lean 4 TextMate stress fixture

This file is intentionally broad but reviewable. Unicode samples: café, λ,
東京, 🚀, and the non-BMP symbol 𝌆. Every multiline construct is closed.
-/

namespace TextMate.Stress

universe u v

variable {α : Type u} {β : Type v}
variable [BEq α] [Hashable α]

section Foundations

/-- `identity` documents a polymorphic definition. -/
@[inline] def identity (x : α) : α := x

abbrev Predicate (α : Type u) := α → Prop

opaque secretNumber : Nat

private def hiddenGreeting : String := "café in 東京"

protected def Nat.triple (n : Nat) : Nat := n + n + n

theorem identity_eq (x : α) : identity x = x := by
  rfl

lemma compose_identity (f : α → β) (x : α) : identity (f x) = f x := by
  simp [identity]

example (p : Prop) (h : p) : p := by
  exact h

#check identity
#print Predicate
#reduce identity 42
#eval hiddenGreeting

end Foundations

/- A regular block comment begins here.
   /- Nested level one.
      /- Nested level two: λ, 0xCAFE, and "not a string". -/
      Level one closes as well. -/
   The outer comment ends here. -/

/-- A two-dimensional point with generated instances. -/
structure Point (α : Type u) where
  x : α
  y : α
deriving Repr, DecidableEq

structure ColoredPoint extends Point Nat where
  color : String := "blue"
deriving Repr

class HasOrigin (α : Type u) where
  origin : α

instance pointNatOrigin : HasOrigin (Point Nat) where
  origin := ⟨0, 0⟩

inductive TrafficLight where
  | red
  | amber
  | green
deriving DecidableEq, Inhabited

deriving instance Repr for TrafficLight

inductive Tree (α : Type u) where
  | leaf (value : α)
  | branch (left right : Tree α)
deriving Repr

namespace Tree

def size : Tree α → Nat
  | leaf _ => 1
  | branch left right => size left + size right

def map (f : α → β) : Tree α → Tree β
  | leaf value => leaf (f value)
  | branch left right => branch (map f left) (map f right)

@[simp] theorem size_map (f : α → β) (tree : Tree α) :
    size (map f tree) = size tree := by
  induction tree with
  | leaf value => rfl
  | branch left right left_ih right_ih =>
      simp [map, size, left_ih, right_ih]

end Tree

mutual
  inductive Even : Nat → Prop where
    | zero : Even 0
    | step : Odd n → Even (n + 1)
  inductive Odd : Nat → Prop where
    | step : Even n → Odd (n + 1)
end

axiom launchReady : Prop

theorem admittedLaunch : launchReady := by
  admit

def «東京の値» : Nat := 7

prefix:max "⁺" => Nat.succ
postfix:max "‼" => Nat.factorial
infixl:65 " ⊞ " => Nat.add
infixr:67 " ** " => Nat.pow
notation "originPoint" => (HasOrigin.origin : Point Nat)

def operatorDemo (n : Nat) : Nat := ⁺n ⊞ (n ** 2)

def naturals : List Nat := [0, 1, 42, 1_000_000, 0xff, 0b1010]
def decimals : List Float := [0.0, 3.14159, 6.02e23, 1.0e-9]
def truthValues : Bool × Bool := (true, false)
def characters : List Char := ['a', 'λ', '\n', '\x41', '\u6771']

def escaped : String := "quote: \" slash: \\ newline: \n tab: \t"
def unicodeText : String := "café — λ — 東京 — 🚀 — 𝌆"
def interpolated (name : String) (count : Nat) : String :=
  s!"Hello, {name}; count = {count + 1}; rocket = 🚀"

def tracedValue (n : Nat) : Nat :=
  dbg_trace "traced {n} from café"
  n + 1

declare_syntax_cat paintColor
syntax "red" : paintColor
syntax "green" : paintColor
syntax:max "paint(" paintColor ")" : term

macro_rules
  | `(paint(red)) => `("#ff0000")
  | `(paint(green)) => `("#00ff00")

syntax:max "twice(" term ")" : term
macro_rules
  | `(twice($value)) => `($value + $value)

syntax (name := repeatExact) "repeat_exact " term : tactic
macro_rules
  | `(tactic| repeat_exact $proof) => `(tactic| first | exact $proof | assumption)

theorem quotationDemo : twice(21) = 42 := by
  repeat_exact rfl

def quotedSyntax : String := paint(green)

-- The antiquotations above exercise term and tactic quotations.
macro "unless " condition:term " then " body:term : term =>
  `(if !$condition then $body else ())

def guardedPrint (quiet : Bool) : IO Unit := do
  unless quiet then println! "launch 🚀"

attribute [simp] identity
attribute [local instance] pointNatOrigin

section Tactics

variable (a b c : Nat)

theorem add_zero_demo : a + 0 = a := by
  simp

theorem and_swap (p q : Prop) : p ∧ q → q ∧ p := by
  intro h
  constructor
  · exact h.right
  · exact h.left

theorem calc_demo : a + (b + c) = a + b + c := by
  calc
    a + (b + c) = (a + b) + c := by omega
    _ = a + b + c := rfl

theorem match_demo (light : TrafficLight) : String := by
  cases light with
  | red => exact "stop"
  | amber => exact "wait"
  | green => exact "go"

theorem have_demo (h : a = b) : a + 1 = b + 1 := by
  have congruent := congrArg (fun n => n + 1) h
  simpa using congruent

end Tactics

noncomputable def chooseDefault [Inhabited α] : α := Classical.choice inferInstance

partial def countdown : Nat → IO Unit
  | 0 => println! "liftoff: 🚀"
  | n + 1 => do
      println! s!"T-minus {n + 1}"
      countdown n

set_option pp.universes true in
#check @identity

#synth HasOrigin (Point Nat)
#eval operatorDemo 4
#eval interpolated "東京" 4
#check (fun x : Nat => x ⊞ 1)

/- Final multiline comment keeps state across lines.
   It contains punctuation: @[simp], `(quoted $syntax), and '𝌆'.
   It is deliberately and visibly closed. -/

end TextMate.Stress
