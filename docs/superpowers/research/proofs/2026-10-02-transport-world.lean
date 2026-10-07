import Mathlib

/-!
# The transport world: the lemmas its exactness rests on

`research/2026-10-02-transport-world.md`. Seats, births and classes are rational
Beatty sequences: over `len` positions with `t` members and offset `τ < len`,
the members among the first `n` are `⌊(n·t + τ)/len⌋`.

1. `rb_total`: the sequence has exactly `t` members.
2. `rb_step`: if `t ≤ len`, each position holds at most one member.
3. `rb_complement`: the non-members form a rational Beatty sequence too, with
   `len − t` members and offset `len − 1 − τ`. Selecting the `r`-th
   non-member is then the same closed form as selecting a member.
4. `block_feasible`, `class_feasible`: a block rounded as `⌊h·L + u⌋` with
   `h ≤ 1` never exceeds the `L` eligible couples it is placed on (and a
   class's share of a survival-weighted count never exceeds the class).
5. `repair_involution`: exchanging the husbands of a pair of seats, under a
   condition symmetric in the pair, is an involution: both partners agree.
-/

namespace Transport

/-- Two naturals summing to `(n + 1)·len − 1` have floors summing to `n`. -/
theorem floor_pair (a b n len : ℕ) (hl : 0 < len) (h : a + b + 1 = (n + 1) * len) :
    a / len + b / len = n := by
  have ha := Nat.div_add_mod a len
  have hb := Nat.div_add_mod b len
  have ra := Nat.mod_lt a hl
  have rb := Nat.mod_lt b hl
  -- (q₁ + q₂)·len + r₁ + r₂ + 1 = (n + 1)·len with r₁ + r₂ + 1 ≤ 2·len − 1.
  have key : (a / len + b / len) * len + (a % len + b % len + 1) = (n + 1) * len := by
    nlinarith [ha, hb, h]
  have hr : a % len + b % len + 1 = len := by
    have h1 : (a / len + b / len) * len < (n + 1) * len := by omega
    have h2 : (n + 1) * len < (a / len + b / len + 2) * len := by nlinarith
    have lt1 : a / len + b / len < n + 1 := by
      by_contra hc; push Not at hc; nlinarith
    have lt2 : n + 1 < a / len + b / len + 2 := by
      by_contra hc; push Not at hc; nlinarith
    have : a / len + b / len = n := by omega
    subst this; nlinarith
  nlinarith

/-- A rational Beatty sequence has exactly `t` members over its `len` positions. -/
theorem rb_total (t len τ : ℕ) (hl : 0 < len) (hτ : τ < len) :
    (len * t + τ) / len = t := by
  rw [Nat.add_comm, Nat.add_mul_div_left _ _ hl, Nat.div_eq_of_lt hτ, Nat.zero_add]

/-- If `t ≤ len`, each position holds at most one member. -/
theorem rb_step (n t len τ : ℕ) (hl : 0 < len) (ht : t ≤ len) :
    ((n + 1) * t + τ) / len ≤ (n * t + τ) / len + 1 := by
  have : (n + 1) * t + τ ≤ (n * t + τ) + len := by nlinarith
  calc ((n + 1) * t + τ) / len ≤ ((n * t + τ) + len) / len := Nat.div_le_div_right this
    _ = (n * t + τ) / len + 1 := by rw [Nat.add_div_right _ hl]

/-- The complement of a rational Beatty sequence is the rational Beatty
sequence with `len − t` members and offset `len − 1 − τ`. -/
theorem rb_complement (n t len τ : ℕ) (hl : 0 < len) (ht : t ≤ len) (hτ : τ < len) :
    n - (n * t + τ) / len = (n * (len - t) + (len - 1 - τ)) / len := by
  have h := floor_pair (n * t + τ) (n * (len - t) + (len - 1 - τ)) n len hl (by
    have e1 : n * (len - t) + n * t = n * len := by
      rw [← Nat.mul_add, Nat.sub_add_cancel ht]
    have e2 : (len - 1 - τ) + τ + 1 = len := by omega
    nlinarith [e1, e2])
  generalize (n * t + τ) / len = X at h ⊢
  generalize (n * (len - t) + (len - 1 - τ)) / len = Y at h ⊢
  omega

/-- A block rounded over its line's eligible couples never exceeds them. -/
theorem block_feasible (h u : ℝ) (L : ℕ) (h0 : 0 ≤ h) (h1 : h ≤ 1) (u0 : 0 ≤ u) (u1 : u < 1) :
    ⌊h * L + u⌋ ≤ (L : ℤ) := by
  rw [Int.floor_le_iff]
  have : h * L ≤ L := by
    have hL : (0 : ℝ) ≤ L := Nat.cast_nonneg L
    nlinarith
  push_cast; linarith

/-- With classes: a class's births `⌊h·e·E_φ/E + u⌋`, from a survival-weighted
eligible count `e ≤ E`, never exceed the class's `E_φ` eligible couples. -/
theorem class_feasible (h e u : ℝ) (E Eφ : ℕ) (hE : 0 < E) (h0 : 0 ≤ h) (h1 : h ≤ 1)
    (e0 : 0 ≤ e) (e1 : e ≤ E) (u0 : 0 ≤ u) (u1 : u < 1) :
    ⌊h * e * Eφ / E + u⌋ ≤ (Eφ : ℤ) := by
  have hE' : (0 : ℝ) < E := by exact_mod_cast hE
  have hr : 0 ≤ e / E ∧ e / E ≤ 1 := ⟨div_nonneg e0 hE'.le, (div_le_one hE').mpr e1⟩
  have : h * e * Eφ / E = (h * (e / E)) * Eφ := by field_simp
  rw [this]
  exact block_feasible (h * (e / E)) u Eφ (mul_nonneg h0 hr.1)
    (by nlinarith [hr.1, hr.2]) u0 u1

/-- Kin repair: seats pair by an involution `mate`; a pair exchanges its
husbands iff a condition symmetric in the pair holds. The map from a seat to
the seat whose default husband it gets is an involution. -/
theorem repair_involution {α : Type*} (mate : α → α) (hm : ∀ p, mate (mate p) = p)
    (swap : α → Prop) [DecidablePred swap] (hs : ∀ p, swap (mate p) ↔ swap p) (p : α) :
    let f := fun q => if swap q then mate q else q
    f (f p) = p := by
  intro f
  by_cases h : swap p
  · have : swap (mate p) := (hs p).mpr h
    simp [f, h, this, hm]
  · simp [f, h]

end Transport

#print axioms Transport.rb_complement
#print axioms Transport.class_feasible
#print axioms Transport.repair_involution
