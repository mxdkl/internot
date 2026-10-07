import Mathlib

/-!
# Cell world, areas (math §17)

Lemma M: any injective matching of a block's demand into its supply pairs
at least `Σ_a max(0, D_a − S_a)` demand ranks across labels, since label
`a`'s demand matched within label `a` lands injectively in its supply.
The labelled block matching (`procedural_core::matching`) attains it.
-/

namespace CellWorldAreas

open Finset

theorem cross_lower {α β L : Type*} [Fintype α] [Fintype β] [Fintype L] [DecidableEq L]
    (f : α → β) (hf : Function.Injective f) (lD : α → L) (lS : β → L) :
    ∑ a, ((univ.filter (fun x => lD x = a)).card - (univ.filter (fun y => lS y = a)).card)
      ≤ (univ.filter (fun x => lS (f x) ≠ lD x)).card := by
  classical
  -- The crossed demand, split by its own label.
  have hsplit : (univ.filter (fun x => lS (f x) ≠ lD x)).card
      = ∑ a, ((univ.filter (fun x => lD x = a)).filter (fun x => ¬ lS (f x) = a)).card := by
    rw [card_eq_sum_card_fiberwise (f := lD) (t := univ) (by simp)]
    apply sum_congr rfl
    intro a _
    congr 1
    ext x
    simp only [mem_filter, mem_univ, true_and]
    constructor
    · rintro ⟨h1, h2⟩
      exact ⟨h2, h2 ▸ h1⟩
    · rintro ⟨h1, h2⟩
      exact ⟨h1 ▸ h2, h1⟩
  rw [hsplit]
  apply sum_le_sum
  intro a _
  -- Label `a`'s demand matched within `a` lands injectively in its supply.
  have hloc : ((univ.filter (fun x => lD x = a)).filter (fun x => lS (f x) = a)).card
      ≤ (univ.filter (fun y => lS y = a)).card := by
    apply card_le_card_of_injOn f
    · intro x hx
      simp only [coe_filter, mem_filter, mem_univ, true_and, Set.mem_ofPred_eq] at hx ⊢
      exact hx.2
    · intro x _ y _ h
      exact hf h
  have hpart := card_filter_add_card_filter_not (s := univ.filter (fun x => lD x = a)) (fun x => lS (f x) = a)
  omega

end CellWorldAreas

#print axioms CellWorldAreas.cross_lower
