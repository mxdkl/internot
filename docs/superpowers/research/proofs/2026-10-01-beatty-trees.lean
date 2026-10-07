import Mathlib

/-!
# Beatty trees: the closed-form inversions

`procedural_core::lattice::BeattyTree`: a node with left share `p/Q`
(`Q = 2³²`) and offset `t < Q` puts position `i` on its left iff
`L(i+1) − L(i) = 1`, where `L(n) = ⌊(n·p + t)/Q⌋` counts the left members
among the first `n`. These are the facts the counts and selects rely on.
-/

namespace Internot.Beatty

/-- Left members among a node's first `n` positions. -/
def L (Q p t n : ℕ) : ℕ := (n * p + t) / Q

/-- No position before the first: `L(0) = 0`. -/
theorem L_zero {Q p t : ℕ} (ht : t < Q) : L Q p t 0 = 0 := by
  simp [L, Nat.div_eq_of_lt ht]

/-- Each position adds at most one left member (`p ≤ Q`): membership is the
step `L(i+1) − L(i) ∈ {0, 1}`, and `L(n)` counts the members among the
first `n` by telescoping. -/
theorem L_step {Q p t : ℕ} (hQ : 0 < Q) (hp : p ≤ Q) (n : ℕ) :
    L Q p t n ≤ L Q p t (n + 1) ∧ L Q p t (n + 1) ≤ L Q p t n + 1 := by
  unfold L
  constructor
  · apply Nat.div_le_div_right
    nlinarith
  · calc ((n + 1) * p + t) / Q ≤ ((n * p + t) + Q) / Q := by
          apply Nat.div_le_div_right
          nlinarith
      _ = (n * p + t) / Q + 1 := Nat.add_div_right _ hQ

/-- **Left select.** The `j`-th left member (from 0) is position `m* − 1`,
with `m* = ⌈((j+1)·Q − t)/p⌉`: `L(m)` reaches `j + 1` exactly from `m*` on. -/
theorem select_left {Q p t : ℕ} (hQ : 0 < Q) (hp : 0 < p) (ht : t < Q) (j m : ℕ) :
    j + 1 ≤ L Q p t m ↔ ((j + 1) * Q - t + p - 1) / p ≤ m := by
  unfold L
  rw [Nat.le_div_iff_mul_le hQ, Nat.div_le_iff_le_mul_add_pred hp]
  have hq : t ≤ (j + 1) * Q := by nlinarith
  constructor
  · intro h
    have : (j + 1) * Q - t ≤ m * p := by omega
    have e : (j + 1) * Q - t + p - 1 = ((j + 1) * Q - t) + (p - 1) := by omega
    rw [e, mul_comm p m]
    omega
  · intro h
    have e : (j + 1) * Q - t + p - 1 = ((j + 1) * Q - t) + (p - 1) := by omega
    rw [e, mul_comm p m] at h
    omega

/-- **Right select.** The `j`-th right member (from 0) is position
`⌊(j·Q + t)/(Q − p)⌋`: the right members among the first `m` reach `j + 1`
exactly when `m` passes it (a Beatty sequence's complement is one too). -/
theorem select_right {Q p t : ℕ} (hQ : 0 < Q) (hp : p < Q) (ht : t < Q) (j m : ℕ) :
    j + 1 ≤ m - L Q p t m ↔ (j * Q + t) / (Q - p) < m := by
  unfold L
  rw [Nat.div_lt_iff_lt_mul (by omega)]
  -- Name `X = ⌊(m·p + t)/Q⌋`; all we need of it is `X < k ↔ m·p + t < k·Q`.
  have key : ∀ k, (m * p + t) / Q < k ↔ m * p + t < k * Q := fun k =>
    Nat.div_lt_iff_lt_mul hQ
  have hLm : (m * p + t) / Q ≤ m := by
    have : (m * p + t) / Q < m + 1 := (key (m + 1)).mpr (by nlinarith)
    omega
  have hmq : m * (Q - p) = m * Q - m * p := Nat.mul_sub m Q p
  have hpQ : m * p ≤ m * Q := Nat.mul_le_mul_left m hp.le
  generalize hX : (m * p + t) / Q = X at key hLm ⊢
  constructor
  · intro h
    -- `X ≤ m − j − 1 < m − j`, so `m·p + t < (m − j)·Q`.
    have hj : j + 1 ≤ m := by omega
    have h2 : m * p + t < (m - j) * Q := (key (m - j)).mp (by omega)
    have hmj : (m - j) * Q = m * Q - j * Q := Nat.sub_mul m j Q
    have hjQ : j * Q ≤ m * Q := Nat.mul_le_mul_right Q (by omega)
    omega
  · intro h
    -- `j·Q + t < m·Q − m·p`, so `m·p + t < (m − j)·Q` and `X < m − j`.
    have hjm : j < m := by
      by_contra hc
      have : m * Q ≤ j * Q := Nat.mul_le_mul_right Q (by omega)
      omega
    have hmj : (m - j) * Q = m * Q - j * Q := Nat.sub_mul m j Q
    have hjQ : j * Q ≤ m * Q := Nat.mul_le_mul_right Q hjm.le
    have h2 : m * p + t < (m - j) * Q := by omega
    have h1 : X < m - j := (key (m - j)).mpr h2
    omega

end Internot.Beatty
