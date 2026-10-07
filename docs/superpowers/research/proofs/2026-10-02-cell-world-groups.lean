import Mathlib

/-!
# Cell world, groups (math §16.1)

Lemma C: a rounded count `⌊a + x⌋ − ⌊a⌋` is at most `x + 1`, so segments
whose densities sum below `A − K` give integer counts summing below `A`.

Lemma R: splitting a nondecreasing integer cumulative `f` into
`⌊f·r/N⌋` and `f − ⌊f·r/N⌋` (with `r ≤ N`) keeps both parts nondecreasing.
-/

namespace CellWorldGroups

/-- One rounded step is at most its density plus one. -/
theorem step_le (a x : ℝ) : ((⌊a + x⌋ - ⌊a⌋ : ℤ) : ℝ) ≤ x + 1 := by
  have h1 := Int.floor_le (a + x)
  have h2 := Int.lt_floor_add_one a
  push_cast
  linarith

/-- Lemma C: `K` segments whose densities sum below `A − K` give integer
counts summing below `A`. -/
theorem layout_fits {K : ℕ} (a x : Fin K → ℝ) (A : ℤ)
    (hsum : ∑ i, x i < (A : ℝ) - K) :
    ∑ i, (⌊a i + x i⌋ - ⌊a i⌋) < A := by
  have h : ((∑ i, (⌊a i + x i⌋ - ⌊a i⌋) : ℤ) : ℝ) ≤ ∑ i, (x i + 1) := by
    push_cast
    apply Finset.sum_le_sum
    intro i _
    have := step_le (a i) (x i)
    push_cast at this
    linarith
  rw [Finset.sum_add_distrib] at h
  simp only [Finset.sum_const, Finset.card_univ, Fintype.card_fin, nsmul_eq_mul, mul_one] at h
  have hr : ((∑ i, (⌊a i + x i⌋ - ⌊a i⌋) : ℤ) : ℝ) < (A : ℝ) := by linarith
  exact_mod_cast hr

/-- The kept part of a split cumulative is nondecreasing. -/
theorem kept_mono {f g r N : ℕ} (h : f ≤ g) : f * r / N ≤ g * r / N :=
  Nat.div_le_div_right (Nat.mul_le_mul_right r h)

/-- The moved part `f − ⌊f·r/N⌋` is nondecreasing when `r ≤ N`. -/
theorem moved_mono {f g r N : ℕ} (hN : 0 < N) (hr : r ≤ N) (h : f ≤ g) :
    f - f * r / N ≤ g - g * r / N := by
  have h1 : g * r ≤ f * r + (g - f) * N := by
    have e : g * r = f * r + (g - f) * r := by
      rw [← Nat.add_mul, Nat.add_sub_cancel' h]
    rw [e]
    exact Nat.add_le_add_left (Nat.mul_le_mul_left _ hr) _
  have h2 : g * r / N ≤ f * r / N + (g - f) := by
    calc g * r / N ≤ (f * r + (g - f) * N) / N := Nat.div_le_div_right h1
      _ = f * r / N + (g - f) := Nat.add_mul_div_right _ _ hN
  have hk : f * r / N ≤ f := by
    calc f * r / N ≤ f * N / N := Nat.div_le_div_right (Nat.mul_le_mul_left _ hr)
      _ = f := Nat.mul_div_cancel _ hN
  generalize f * r / N = p at *
  generalize g * r / N = q at *
  omega

/-- The two parts add back to the whole (the arrivals' total is kept). -/
theorem split_total {f r N : ℕ} (hN : 0 < N) (hr : r ≤ N) :
    f * r / N + (f - f * r / N) = f := by
  have hk : f * r / N ≤ f := by
    calc f * r / N ≤ f * N / N := Nat.div_le_div_right (Nat.mul_le_mul_left _ hr)
      _ = f := Nat.mul_div_cancel _ hN
  omega

end CellWorldGroups

#print axioms CellWorldGroups.layout_fits
#print axioms CellWorldGroups.moved_mono
#print axioms CellWorldGroups.split_total
