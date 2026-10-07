import Mathlib

/-!
# The monotone world (`thinking/claude/004`–`008`)

The lemmas `internot_society::mono` rests on.

* **Monotone slots:** in a class whose death times rise with the slot,
  the slots dead by `t` are a prefix.
* **Proportional owner:** child `j` of a block of `c` children over `m`
  mothers belongs to mother `⌊j·m/c⌋`; the children of mothers `[i₀, i₁)`
  are exactly `[⌈i₀·c/m⌉, ⌈i₁·c/m⌉)`, one interval (so "born by t" over a
  run of mothers is one subtraction).
* **Rotated phases:** if `h` is monotone with values below `Q`, the slots
  whose rotated phase `(h ρ + β) mod Q` is at most `p` are the slots with
  `h ρ ≤ p − β` (when `β ≤ p`) together with those with
  `Q − β ≤ h ρ ≤ Q − β + p`: two intervals of a monotone sequence.
-/

namespace MonoWorld

/-- Monotone slots: the slots `ρ < n` with `T ρ ≤ t` are those below the
first slot past `t`. -/
theorem dead_prefix (T : ℕ → ℕ) (_hT : Monotone T) (t k : ℕ) (hk : ∀ ρ, ρ < k ↔ T ρ ≤ t)
    (n : ℕ) : ((Finset.range n).filter (fun ρ => T ρ ≤ t)).card = min k n := by
  have : (Finset.range n).filter (fun ρ => T ρ ≤ t) = Finset.range (min k n) := by
    ext ρ
    simp only [Finset.mem_filter, Finset.mem_range, lt_min_iff]
    rw [← hk]
    tauto
  rw [this, Finset.card_range]

/-- Proportional owner: `⌊j·m/c⌋ < i ↔ j < ⌈i·c/m⌉` (for `c, m > 0`), with
the ceiling as `(i*c + m - 1)/m`. -/
theorem owner_lt_iff (j i c m : ℕ) (hm : 0 < m) (hc : 0 < c) :
    j * m / c < i ↔ j < (i * c + m - 1) / m := by
  rw [Nat.div_lt_iff_lt_mul hc]
  constructor
  · intro h
    -- j·m < i·c, so (j+1)·m ≤ i·c + m − 1.
    have : (j + 1) * m ≤ i * c + m - 1 := by
      have : j * m + 1 ≤ i * c := by linarith [Nat.mul_comm c i]
      rw [Nat.add_mul]; omega
    exact Nat.lt_of_lt_of_le (Nat.lt_succ_self j) ((Nat.le_div_iff_mul_le hm).mpr this)
  · intro h
    have h2 : (j + 1) * m ≤ i * c + m - 1 := (Nat.le_div_iff_mul_le hm).mp h
    rw [Nat.add_mul] at h2
    have : j * m < i * c := by omega
    linarith [Nat.mul_comm c i]

/-- The children of mothers `[i₀, i₁)` are the `j` with
`⌈i₀·c/m⌉ ≤ j < ⌈i₁·c/m⌉`: membership of `⌊j·m/c⌋` in an interval is
membership of `j` in an interval. -/
theorem owner_mem_Ico (j i₀ i₁ c m : ℕ) (hm : 0 < m) (hc : 0 < c) :
    (i₀ ≤ j * m / c ∧ j * m / c < i₁) ↔
      ((i₀ * c + m - 1) / m ≤ j ∧ j < (i₁ * c + m - 1) / m) := by
  have k0 := owner_lt_iff j i₀ c m hm hc
  have k1 := owner_lt_iff j i₁ c m hm hc
  constructor
  · rintro ⟨h0, h1⟩
    refine ⟨?_, k1.mp h1⟩
    by_contra hh
    exact absurd (k0.mpr (by omega)) (by omega)
  · rintro ⟨h0, h1⟩
    refine ⟨?_, k1.mpr h1⟩
    by_contra hh
    exact absurd (k0.mp (by omega)) (by omega)

/-- Rotated phases: for `h ρ < Q` and `β < Q`, `(h ρ + β) % Q ≤ p` iff
`h ρ + β ≤ p` or `Q ≤ h ρ + β ≤ Q + p` (two intervals of `h ρ`). -/
theorem rotated_le_iff (x β Q p : ℕ) (hx : x < Q) (hb : β < Q) :
    (x + β) % Q ≤ p ↔ x + β ≤ p ∨ (Q ≤ x + β ∧ x + β ≤ Q + p) := by
  rcases lt_or_ge (x + β) Q with h | h
  · rw [Nat.mod_eq_of_lt h]
    constructor
    · intro hp; exact Or.inl hp
    · rintro (hp | ⟨hq, _⟩)
      · exact hp
      · omega
  · have h2 : x + β < 2 * Q := by omega
    have : (x + β) % Q = x + β - Q := by
      rw [Nat.mod_eq_sub_mod h, Nat.mod_eq_of_lt (by omega)]
    rw [this]
    omega

/-- Capped blocks: with no more children than mothers (`c ≤ m`), the
proportional owner gives every mother at most one child: the children of
mother `i` are `[⌈i·c/m⌉, ⌈(i+1)·c/m⌉)`, at most one. -/
theorem one_child_per_mother (i c m : ℕ) (hm : 0 < m) (hcm : c ≤ m) :
    ((i + 1) * c + m - 1) / m ≤ (i * c + m - 1) / m + 1 := by
  have h1 : (i + 1) * c + m - 1 ≤ (i * c + m - 1) + m := by
    rw [Nat.add_mul, one_mul]; omega
  calc ((i + 1) * c + m - 1) / m ≤ ((i * c + m - 1) + m) / m := Nat.div_le_div_right h1
    _ = (i * c + m - 1) / m + 1 := Nat.add_div_right _ hm

end MonoWorld

#print axioms MonoWorld.owner_mem_Ico
#print axioms MonoWorld.rotated_le_iff
#print axioms MonoWorld.dead_prefix
#print axioms MonoWorld.one_child_per_mother
