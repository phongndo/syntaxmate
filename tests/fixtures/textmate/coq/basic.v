From Coq Require Import List String.
Import ListNotations.
Open Scope string_scope.

(* Basic TextMate fixture: BMP λ 東京; astral 🚀 𝌆. *)
Section Basic.
Variable A : Type.
Context (x : A).

Inductive maybe_λ : Type :=
| Nothing
| Just (value : A).

Definition choose (flag : bool) : maybe_λ :=
  if flag then Just x else Nothing.
Definition answer : nat := 42.
Definition message := "λ 東京 🚀 𝌆".

Lemma choose_true : choose true = Just x.
Proof.
  unfold choose; reflexivity.
Qed.
End Basic.
