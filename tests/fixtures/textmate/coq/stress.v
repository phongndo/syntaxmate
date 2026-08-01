From Coq Require Import List String Arith Bool.
Require Export Coq.Init.Nat.
Import ListNotations.
Export ListNotations.
Open Scope string_scope.
Close Scope nat_scope.
Open Scope nat_scope.
(* Coq TextMate stress fixture: BMP λ 東京; astral 🚀 𝌆. *)

Delimit Scope fixture_scope with fixture.
Undelimit Scope fixture_scope.
Bind Scope nat_scope with nat.
Section GrammarStress.
Module Type FixtureSignature.
  Parameter item : Type.
  Parameter default_item : item.
End FixtureSignature.
Module FixtureModule.
  Definition module_value : nat := 7.
End FixtureModule.
Include FixtureModule.

Universe u v.
Generalizable All Variables.
Variable A : Type@{u}.
Variables B C : Type.
Parameter seed : nat.
Parameters left right : A.
Parameter Inline inline_seed : nat.
Axiom identity_axiom : forall x : A, x = x.
Axioms first_axiom : True.
Conjecture sample_conjecture : Prop.
Conjectures another_conjecture : Prop.
Hypothesis positive_seed : seed >= 0.
Hypotheses stable_left : left = left.
Context (value : A).
Context {implicit_value : B}.
Context `{named_context : C}.
Context (λ 東京 : nat).
Variable 𝒜 : Type.

Definition decimal_number : nat := 1_000.
Definition floating_notation := 3.1415.
Definition hexadecimal_number := 0xCAFE.
Definition ordinary_string := "café λ 東京 🚀 𝌆".
Definition quoted_string := "say ""hello""".
Local Definition local_identity (x : A) := x.
Program Definition programmed_identity (x : A) : A := x.
Definition 𝒜_identity (x : 𝒜) : 𝒜 := x.
Example arithmetic_example : 2 + 2 = 4.
Proof. reflexivity. Qed.

Fixpoint countdown (n : nat) : nat :=
  match n with
  | O => O
  | S next => countdown next
  end.

CoFixpoint repeat_true : Stream bool :=
  Cons true repeat_true.

Function predecessor (n : nat) : nat :=
  match n with
  | O => O
  | S next => next
  end.

Let Fixpoint local_countdown (n : nat) : nat :=
  match n with O => O | S next => local_countdown next end.
Let CoFixpoint local_stream : Stream nat := Cons 0 local_stream.
Equations doubled (n : nat) : nat :=
doubled n := n + n.

Inductive traffic_light : Type :=
| Red
| Amber
| Green.

Variant response (T : Type) : Type :=
| Accepted (payload : T)
| Rejected.

CoInductive signal : Type :=
| Tick : signal -> signal.

Record point : Type := {
  point_x : nat;
  point_y : nat
}.

Structure wrapper : Type := {
  wrapped : A
}.

Class Default (T : Type) := {
  default : T
}.
Instance nat_default : Default nat := { default := 0 }.

Definition constructor_values :=
  (True, False, tt, true, false, Some 1, None, nil, cons 1 nil,
   pair 1 2, inl 1, inr 2, Eq, Lt, Gt, id, ex, all, unique).
Definition wildcard_match (candidate : option nat) :=
  match candidate with Some n => n | _ => 0 end.

Theorem theorem_identity : forall n : nat, n = n.
Proof. intros n. reflexivity. Qed.
Lemma lemma_identity : ∀ n : nat, n = n.
Proof. intro n; reflexivity. Defined.
Remark remark_true : True.
Proof. constructor. Save saved_remark.
Fact fact_true : True.
Proof. exact I. Qed.
Corollary corollary_true : True.
Proof. trivial. Qed.
Property property_true : True.
Proof. easy. Qed.
Proposition proposition_true : True.
Proof. now constructor. Qed.
Goal exists n : nat, n <= n.
Proof. exists 0; reflexivity. Qed.

Definition gallina_controls (flag : bool) (input : option nat) : nat :=
  let base := if flag then 1 else 0 in
  match input return nat with
  | Some n => base + n
  | None => base
  end.
Definition lambda_value := fun x : nat => x.
Definition unicode_lambda := λ x : nat, x.
Definition fixed_value := fix loop (n : nat) :=
  match n with O => O | S next => loop next end.
Definition logical_types : Prop :=
  forall n : nat, exists m : nat,
    n = m ∨ n ≠ m ∧ n <= m ↔ n >= m.
Check (nat * bool + option nat + list nat + unit + sum nat nat).
Check (prod nat nat * comparison * Empty_set).
Eval compute in countdown 4.
Compute predecessor 9.
Search (_ = _).
About countdown.
Locate "+".
Print All countdown.

Ltac introduce_and_finish :=
  intros; try reflexivity; repeat constructor; auto.
Ltac branching tactic :=
  first [ assumption | eassumption | trivial | easy ].
Ltac rewrite_everywhere lemma :=
  progress rewrite lemma at *; simpl; now auto.
Ltac2 mutable rec modern_tactic () := ().

Lemma tactic_vocabulary (P Q : Prop) : P -> Q -> P /\ Q.
Proof.
  intros P_proof Q_proof.
  split; [ exact P_proof | exact Q_proof ].
  try solve [ assumption ].
  first [ reflexivity | idtac "fallback" ].
Qed.

Hint Constructors traffic_light : fixture.
Hint Resolve theorem_identity : fixture.
Hint Rewrite theorem_identity : fixture.
Hint Mode Default + : typeclass_instances.
Create HintDb fixture.
Arguments Some {A} _.
Implicit Types x y : nat.
Reserved Notation "x === y" (at level 70).
Notation "x === y" := (x = y) (at level 70).
Infix "⊕" := plus (at level 50).
Canonical nat_default.
Existing Class Default.
Existing Instance nat_default.
Typeclasses Opaque Default.
Typeclasses Transparent Default.
Set Printing Universes.
Unset Printing Universes.
Remove Printing Let.

Obligation Tactic := intros.
Next Obligation.
Solve All Obligations.
Show Proof.
Show Existentials.
Focus 1.
Unfocus.
Unshelve.
Time Check countdown.
Timeout 1 Check countdown.
Fail Check missing_name.
Redirect "fixture.log" Print countdown.

Lemma admitted_branch : False.
Proof. admit. Admitted.

(* Outer comment exercises recursive state.
   (* Nested comment contains quoted marker text and λ 東京. *)
   Operators inside comments: -> ↔ ∧ ∨ ≠ ≤ ≥.
   Astral offsets remain visible here: 🚀 and 𝌆.
*)
Definition after_nested_comment : string :=
  "closed multiline fixture: λ 東京 🚀 𝌆".
End GrammarStress.
