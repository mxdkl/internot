import Mathlib

/-!
# Cell world, lookup speed (math §18)

Lemma K (chain keys) and the emptiness identity of §18.2.

* A chain step that keeps a key keeps it along the whole chain.
* If every candidate father of a child has the chain key of the child's
  mother, children whose mothers' keys differ share no candidate father.
* The residue classes mod `K` partition `[0, p)`, so the eligible immigrant
  mothers of both kinds sum to the arrived couples, singles and women who
  don't partner here.
-/

namespace CellWorldSpeed

/-- Fact 1: a step that keeps the key keeps it after any number of steps. -/
theorem key_along_chain {C κ : Type*} (key : C → κ) (next : C → C)
    (h : ∀ c, key (next c) = key c) (c : C) (n : ℕ) :
    key (next^[n] c) = key c := by
  induction n generalizing c with
  | zero => rfl
  | succ n ih => rw [Function.iterate_succ_apply, ih, h]

/-- Lemma K: candidate fathers carry their child's mother's chain key, so
different keys mean disjoint candidate sets. -/
theorem lemma_K {P κ : Type*} (keyOf : P → κ) (cands : P → Set P) (mkey : P → κ)
    (hK : ∀ x, ∀ f ∈ cands x, keyOf f = mkey x) (w h : P) (hne : mkey w ≠ mkey h) :
    Disjoint (cands w) (cands h) := by
  rw [Set.disjoint_left]
  intro f hw hh
  exact hne ((hK w f hw).symm.trans (hK h f hh))

/-- Lemma K, father and daughter: a man whose own key differs from the
daughter's mother's is none of her candidate fathers. -/
theorem lemma_K_father {P κ : Type*} (keyOf : P → κ) (cands : P → Set P) (mkey : P → κ)
    (hK : ∀ x, ∀ f ∈ cands x, keyOf f = mkey x) (w h : P) (hne : keyOf h ≠ mkey w) :
    h ∉ cands w := fun hm => hne (hK w h hm)

/-- The residue classes mod `K` partition `[0, p)`: their counts sum to `p`. -/
theorem residue_counts_sum (K p : ℕ) (hK : 0 < K) :
    ∑ r ∈ Finset.range K, ((Finset.range p).filter (fun j => j % K = r)).card = p := by
  rw [← Finset.card_eq_sum_card_fiberwise (f := fun j => j % K)]
  · simp
  · intro j _
    simpa using Nat.mod_lt j hK

/-- §18.2: with married ≤ arrived in every class, the mothers of both kinds
(`p_ac + Σ married` and `p_nw + Σ (arrived − married)`) sum to
`p_ac + p_nw + Σ arrived`. -/
theorem mothers_sum (K : ℕ) (pac pnw : ℕ) (arrived married : ℕ → ℕ)
    (hle : ∀ c, married c ≤ arrived c) :
    (pac + ∑ c ∈ Finset.range K, married c) + (pnw + ∑ c ∈ Finset.range K, (arrived c - married c))
      = pac + pnw + ∑ c ∈ Finset.range K, arrived c := by
  have h : ∑ c ∈ Finset.range K, married c + ∑ c ∈ Finset.range K, (arrived c - married c)
      = ∑ c ∈ Finset.range K, arrived c := by
    rw [← Finset.sum_add_distrib]
    exact Finset.sum_congr rfl (fun c _ => Nat.add_sub_cancel' (hle c))
  omega

/-- §18.6: the pair decision is symmetric in its two couples. -/
theorem pair_decision_symmetric (rkk rmm rkm rmk : Bool) :
    ((rkk || rmm) && !rkm && !rmk) = ((rmm || rkk) && !rmk && !rkm) := by
  cases rkk <;> cases rmm <;> cases rkm <;> cases rmk <;> rfl

end CellWorldSpeed

#print axioms CellWorldSpeed.lemma_K
#print axioms CellWorldSpeed.residue_counts_sum
#print axioms CellWorldSpeed.mothers_sum
