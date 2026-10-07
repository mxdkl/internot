import Mathlib

/-!
# The pure world's counting lemmas

Machine-checked facts behind `internot_society::pure` and
`procedural_core::lattice` (research note `2026-10-01-pure-world.md`).

1. **Systematic rounding** (`B k = ⌊N·C k + u⌋`, `0 ≤ u < 1`): every boundary
   and every count is within 1 of its expectation, the ends are exact, and
   counts telescope, so a block's members split exactly over categories.
2. **The queue offset bound**: summed over blocks, the boundaries' total
   error is below the number of blocks, so women's and men's cumulative
   seats differ by less than the blocks on each side (plus any difference
   in expectation): the men's queue stays bounded.
3. **The Kronecker counting identity**: whether a phase `v mod m` is below
   `d` is a difference of two floors, so a count over a range is a
   difference of two floor sums.
4. **Rotations and mirrors keep category counts**: why the re-partnering
   lines balance exactly (men read their group's pattern rotated) and why
   mirrored layers have equal sizes on both sides.
-/

namespace Internot

/-! ## 1. Systematic rounding -/

/-- A boundary's rounding error is less than 1. -/
theorem boundary_close (N C u : ℝ) (hu0 : 0 ≤ u) (hu1 : u < 1) :
    |(⌊N * C + u⌋ : ℝ) - N * C| < 1 := by
  have h1 := Int.floor_le (N * C + u)
  have h2 := Int.lt_floor_add_one (N * C + u)
  rw [abs_lt]
  constructor <;> linarith

/-- Boundaries are monotone in the cumulative share (for `N ≥ 0`). -/
theorem boundary_mono {N C C' u : ℝ} (hN : 0 ≤ N) (h : C ≤ C') :
    ⌊N * C + u⌋ ≤ ⌊N * C' + u⌋ := by
  apply Int.floor_mono
  nlinarith

/-- The first boundary (`C = 0`) is `0` and the last (`C = 1`) is `N`. -/
theorem boundary_ends (n : ℕ) (u : ℝ) (hu0 : 0 ≤ u) (hu1 : u < 1) :
    ⌊(n : ℝ) * 0 + u⌋ = 0 ∧ ⌊(n : ℝ) * 1 + u⌋ = n := by
  constructor
  · simp only [mul_zero, zero_add]
    exact Int.floor_eq_zero_iff.mpr ⟨hu0, hu1⟩
  · simp only [mul_one]
    rw [Int.floor_eq_iff]
    constructor <;> push_cast <;> linarith

/-- A category's count is within 1 of its expected share. -/
theorem count_close (N C C' u : ℝ) :
    |((⌊N * C' + u⌋ - ⌊N * C + u⌋ : ℤ) : ℝ) - N * (C' - C)| < 1 := by
  have a1 := Int.floor_le (N * C' + u)
  have a2 := Int.lt_floor_add_one (N * C' + u)
  have b1 := Int.floor_le (N * C + u)
  have b2 := Int.lt_floor_add_one (N * C + u)
  push_cast
  rw [abs_lt]
  constructor <;> linarith

/-- Counts telescope: the categories' counts sum to the last boundary minus
the first, so with exact ends they sum to `N`. -/
theorem counts_sum (B : ℕ → ℤ) (n : ℕ) :
    ∑ k ∈ Finset.range n, (B (k + 1) - B k) = B n - B 0 :=
  Finset.sum_range_sub B n

/-! ## 2. The queue offset bound -/

/-- Over a set of blocks, the boundaries' total error is less than the
number of blocks: each block's cumulative seats are within 1 of expected.
Applied to women's and men's blocks, the men's queue offset
`Mc − Wc` is bounded by the active blocks plus the difference in
expectation (zero by the mean-field balance). -/
theorem offset_bound {ι : Type*} (s : Finset ι) (hs : s.Nonempty) (N C u : ι → ℝ)
    (hu : ∀ i, 0 ≤ u i ∧ u i < 1) :
    |∑ i ∈ s, ((⌊N i * C i + u i⌋ : ℝ) - N i * C i)| < s.card := by
  calc |∑ i ∈ s, ((⌊N i * C i + u i⌋ : ℝ) - N i * C i)|
      ≤ ∑ i ∈ s, |(⌊N i * C i + u i⌋ : ℝ) - N i * C i| := Finset.abs_sum_le_sum_abs _ _
    _ < ∑ _i ∈ s, (1 : ℝ) :=
        Finset.sum_lt_sum_of_nonempty hs (fun i _ => boundary_close _ _ _ (hu i).1 (hu i).2)
    _ = s.card := by simp

/-! ## 3. The Kronecker counting identity -/

/-- For `0 < m` and `d ≤ m`: `⌊(v + (m − d))/m⌋ = ⌊v/m⌋ + [v mod m ≥ d]`. So
`[v mod m < d] = 1 − (⌊(v + m − d)/m⌋ − ⌊v/m⌋)`, and counting the phases
below `d` over a range is a difference of two floor sums. -/
theorem below_indicator (v m d : ℕ) (hm : 0 < m) (hd : d ≤ m) :
    (v + (m - d)) / m = v / m + (if v % m < d then 0 else 1) := by
  have hv := Nat.div_add_mod v m
  have hr : v % m < m := Nat.mod_lt v hm
  have e : v + (m - d) = (v % m + (m - d)) + m * (v / m) := by omega
  rw [e, Nat.add_mul_div_left _ _ hm]
  split_ifs with h
  · rw [Nat.div_eq_of_lt (by omega)]
    omega
  · have : (v % m + (m - d)) / m = 1 := by
      apply Nat.div_eq_of_lt_le <;> omega
    omega

/-- Counting: over `i < n`, the phases `f i mod m` below `d` number `n`
minus the difference of the two floor sums. -/
theorem count_below (f : ℕ → ℕ) (n m d : ℕ) (hm : 0 < m) (hd : d ≤ m) :
    ((Finset.range n).filter (fun i => f i % m < d)).card
      + ∑ i ∈ Finset.range n, (f i + (m - d)) / m
      = n + ∑ i ∈ Finset.range n, f i / m := by
  have key : ∀ i ∈ Finset.range n,
      (f i + (m - d)) / m = f i / m + (if f i % m < d then 0 else 1) :=
    fun i _ => below_indicator (f i) m d hm hd
  rw [Finset.sum_congr rfl key, Finset.sum_add_distrib, Finset.card_filter]
  have split : ∀ i ∈ Finset.range n,
      (if f i % m < d then 1 else 0) + (if f i % m < d then 0 else 1) = 1 := by
    intro i _
    split_ifs <;> rfl
  have total : ∑ i ∈ Finset.range n,
      ((if f i % m < d then 1 else 0) + (if f i % m < d then 0 else 1)) = n := by
    rw [Finset.sum_congr rfl split]
    simp
  rw [Finset.sum_add_distrib] at total
  omega

/-! ## 4. Rotations and mirrors keep category counts -/

/-- Reading a pattern rotated by `s` keeps each category's count over the
group: men's re-partnering delays (their group's pattern read half a group
away) send as many men as women to every year. -/
theorem rotation_count {n : ℕ} [NeZero n] {α : Type*} [DecidableEq α]
    (f : ZMod n → α) (s : ZMod n) (c : α) :
    (Finset.univ.filter (fun j => f (j + s) = c)).card
      = (Finset.univ.filter (fun j => f j = c)).card := by
  apply Finset.card_bij (fun j _ => j + s)
  · intro j hj
    simpa using hj
  · intro a _ b _ h
    simpa using h
  · intro b hb
    exact ⟨b - s, by simpa using hb, by ring⟩

/-- Mirroring positions (`q ↦ K − 1 − q`) keeps each layer's size: the men's
layers, read as the women's pattern mirrored, have exactly the women's
sizes, so rank matching within each layer is a bijection. -/
theorem mirror_count {K : ℕ} {α : Type*} [DecidableEq α] (f : Fin K → α) (c : α) :
    (Finset.univ.filter (fun q : Fin K => f (Fin.rev q) = c)).card
      = (Finset.univ.filter (fun p => f p = c)).card := by
  apply Finset.card_bij (fun q _ => Fin.rev q)
  · intro q hq
    simpa using hq
  · intro a _ b _ h
    simpa using h
  · intro b hb
    exact ⟨Fin.rev b, by simpa using hb, by simp⟩

/-- The core/tail coupling (research note §7, the age-gap fix): with a
palindromic class set `T` (`q ∈ T ↔ rev q ∈ T`), mirroring within `T` keeps
each layer's size, so tail layers read reversed and core layers read
straight both match their women's counts. -/
theorem palindromic_mirror_count {K : ℕ} {α : Type*} [DecidableEq α] (f : Fin K → α)
    (T : Finset (Fin K)) (hT : ∀ q, q ∈ T ↔ Fin.rev q ∈ T) (c : α) :
    (T.filter (fun q => f (Fin.rev q) = c)).card = (T.filter (fun p => f p = c)).card := by
  apply Finset.card_bij (fun q _ => Fin.rev q)
  · intro q hq
    simp only [Finset.mem_filter] at hq ⊢
    exact ⟨(hT q).mp hq.1, hq.2⟩
  · intro a _ b _ h
    simpa using h
  · intro b hb
    simp only [Finset.mem_filter] at hb
    refine ⟨Fin.rev b, ?_, by simp⟩
    simp only [Finset.mem_filter, Fin.rev_rev]
    exact ⟨(hT (Fin.rev b)).mpr (by simpa using hb.1), hb.2⟩

/-! ## 5. Cohort bounds as differences of birth-year prefix sums -/

/-- The reframing proposed for arrival cohorts (research note §8): a
cohort that enters single at age `A` and can partner from `st` reads its
cumulative incidence off the birth year's prefix sums. With the birth
year's survival `l`, never-partnered survival `S` (`S (a+1) = S a · (1 − h a)`)
and hazard terms `w a = l a · S a · h a`, the cohort's sum of
`(l a / l A) · (S a / S st) · h a` over `[st, n)` is the difference of the
prefix sums of `w`, divided by `l A · S st`. -/
theorem cohort_from_prefix (l S h : ℕ → ℝ) (A st n : ℕ) (hst : st ≤ n)
    (hl : l A ≠ 0) (hS : S st ≠ 0) :
    ∑ a ∈ Finset.Ico st n, (l a / l A) * (S a / S st) * h a
      = (∑ a ∈ Finset.range n, l a * S a * h a
          - ∑ a ∈ Finset.range st, l a * S a * h a) / (l A * S st) := by
  rw [← Finset.sum_Ico_eq_sub _ hst, Finset.sum_div]
  apply Finset.sum_congr rfl
  intro a _
  field_simp

end Internot
