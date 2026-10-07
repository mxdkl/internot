import Mathlib

/-!
# Lipschitz Beatty splits

A Beatty node splits positions `0, 1, 2, …` by `L(n) = ⌊n·p + t⌋`: position `i`
goes left iff `L(i+1) − L(i) = 1`, and `L(n)` counts the left members among the
first `n`. Here the constant share `p` becomes any function `F` whose steps lie
in `[0, 1]`: `L(n) = ⌊F(n) + t⌋`. Shares may then vary along the positions
(by age, by birth time), and nothing about counting or selecting changes:

1. **Validity.** Each step of `L` is 0 or 1, and so is each step of the right
   count `n − L(n)`: every position goes to exactly one side.
2. **Scale-free shapes.** If a shape `Φ` on the reals is monotone with slope at
   most 1, `F(n) = K·Φ(n/K)` is valid for every size `K > 0`: one shape serves
   every node size, with nothing stored per node.
3. **Count and select.** For any 0/1-step count `c` from 0, the members among
   the first `n` number `c(n)`, and select (the least `m` with `k ≤ c(m)`) is
   the lower adjoint of count (a Galois connection). Galois connections compose,
   so along any path of nested splits, select is the lower adjoint of the
   composed count: a category's `j`-th member is found by inverting each level.
4. **Nested discrepancy.** If each level's realized count is within 1 of a
   1-Lipschitz function of the previous level's, then after `d` levels the
   realized count is within `d` of the composed expectation. Left counts
   (`F` with slope in `[0, 1]`) and right counts (`x − F(x)`) are both
   1-Lipschitz, so this holds for every path.
5. **Exact pieces.** On a linear piece `L(n) = ⌊(A + n·P)/Q⌋` in integers
   (`0 < P ≤ Q`, any offset `A ≤ (j+1)·Q`), the `j`-th left member is at
   `⌈((j+1)·Q − A)/P⌉ − 1`: select stays closed form.
-/

namespace Internot.Lipschitz

/-! ## 1. Validity -/

/-- The left count of a split: `L(n) = ⌊F(n) + t⌋`. -/
noncomputable def L (F : ℕ → ℝ) (t : ℝ) (n : ℕ) : ℤ := ⌊F n + t⌋

/-- **Validity.** If `F` steps by between 0 and 1, so does `L` (by an integer):
each position goes left (step 1) or right (step 0). -/
theorem L_step (F : ℕ → ℝ) (t : ℝ) (n : ℕ)
    (h0 : F n ≤ F (n + 1)) (h1 : F (n + 1) ≤ F n + 1) :
    L F t n ≤ L F t (n + 1) ∧ L F t (n + 1) ≤ L F t n + 1 := by
  unfold L
  constructor
  · exact Int.floor_mono (by linarith)
  · have h : ⌊F (n + 1) + t⌋ ≤ ⌊F n + t + 1⌋ := Int.floor_mono (by linarith)
    rwa [Int.floor_add_one] at h

/-- The right count `n − L(n)` steps by 0 or 1 too. -/
theorem R_step (F : ℕ → ℝ) (t : ℝ) (n : ℕ)
    (h0 : F n ≤ F (n + 1)) (h1 : F (n + 1) ≤ F n + 1) :
    (n : ℤ) - L F t n ≤ ((n + 1 : ℕ) : ℤ) - L F t (n + 1) ∧
      ((n + 1 : ℕ) : ℤ) - L F t (n + 1) ≤ (n : ℤ) - L F t n + 1 := by
  obtain ⟨a, b⟩ := L_step F t n h0 h1
  push_cast
  constructor <;> linarith

/-! ## 2. Scale-free shapes -/

/-- **Scale-free shapes.** A shape `Φ` that is monotone with slope at most 1
gives a valid split `F(n) = K·Φ(n/K)` at every size `K > 0`. -/
theorem scaled_step (Φ : ℝ → ℝ)
    (hΦ : ∀ x y, x ≤ y → Φ x ≤ Φ y ∧ Φ y - Φ x ≤ y - x)
    (K : ℝ) (hK : 0 < K) (n : ℕ) :
    K * Φ (n / K) ≤ K * Φ ((n + 1) / K) ∧
      K * Φ ((n + 1) / K) ≤ K * Φ (n / K) + 1 := by
  have hxy : (n : ℝ) / K ≤ (n + 1) / K := by
    apply div_le_div_of_nonneg_right _ hK.le
    linarith
  obtain ⟨m1, m2⟩ := hΦ _ _ hxy
  have hw : ((n : ℝ) + 1) / K - n / K = 1 / K := by
    field_simp
    ring
  constructor
  · exact mul_le_mul_of_nonneg_left m1 hK.le
  · have : K * (Φ ((n + 1) / K) - Φ (n / K)) ≤ K * (1 / K) := by
      apply mul_le_mul_of_nonneg_left _ hK.le
      linarith
    rw [mul_one_div_cancel hK.ne'] at this
    linarith

/-! ## 3. Count and select -/

section CountSelect

variable (c : ℕ → ℕ) (h0 : c 0 = 0) (hs : ∀ n, c n ≤ c (n + 1) ∧ c (n + 1) ≤ c n + 1)
include h0 hs

/-- The members among the first `n` positions number `c n` (telescoping). -/
theorem count_eq (n : ℕ) :
    ((Finset.range n).filter (fun i => c (i + 1) = c i + 1)).card = c n := by
  induction n with
  | zero => simp [h0]
  | succ n ih =>
    rw [Finset.range_add_one, Finset.filter_insert]
    split_ifs with h
    · rw [Finset.card_insert_of_notMem (by simp), ih, h]
    · have := hs n
      rw [ih]
      omega

omit h0 in
/-- A 0/1-step count is monotone. -/
theorem count_mono : Monotone c :=
  monotone_nat_of_le_succ fun n => (hs n).1

omit h0 in
/-- **Select is the lower adjoint of count.** With `s k` the least `m` where
the count reaches `k`, `s k ≤ m ↔ k ≤ c m`. -/
theorem select_galois (hex : ∀ k, ∃ m, k ≤ c m) :
    GaloisConnection (fun k => Nat.find (hex k)) c := by
  intro k m
  constructor
  · intro h
    exact le_trans (Nat.find_spec (hex k)) (count_mono c hs h)
  · intro h
    exact Nat.find_min' (hex k) h

/-- The selected position is a member, with exactly `j` members before it:
for `k = j + 1`, position `s k − 1` steps the count from `j` to `j + 1`. -/
theorem select_member (hex : ∀ k, ∃ m, k ≤ c m) (j : ℕ) :
    let m := Nat.find (hex (j + 1))
    0 < m ∧ c (m - 1) = j ∧ c m = j + 1 := by
  intro m
  have hm : j + 1 ≤ c m := Nat.find_spec (hex (j + 1))
  have hpos : 0 < m := by
    by_contra hz
    have : m = 0 := by omega
    rw [this, h0] at hm
    omega
  have hprev : ¬ (j + 1 ≤ c (m - 1)) :=
    Nat.find_min (hex (j + 1)) (by omega)
  have step := hs (m - 1)
  rw [Nat.sub_add_cancel hpos] at step
  refine ⟨hpos, ?_, ?_⟩ <;> omega

end CountSelect

/-- **Selects compose.** Two nested levels, each with select the lower adjoint
of its count, give a path whose select (outer after inner) is the lower adjoint
of its count (inner after outer). By induction, any path. -/
theorem path_galois {s₁ c₁ s₂ c₂ : ℕ → ℕ}
    (g₁ : GaloisConnection s₁ c₁) (g₂ : GaloisConnection s₂ c₂) :
    GaloisConnection (s₁ ∘ s₂) (c₂ ∘ c₁) :=
  g₂.compose g₁

/-! ## 4. Nested discrepancy -/

/-- One level's error: `⌊y + t⌋` is within 1 of `y` (`0 ≤ t < 1`). -/
theorem floor_err (y t : ℝ) (h0 : 0 ≤ t) (h1 : t < 1) :
    |(⌊y + t⌋ : ℝ) - y| < 1 := by
  have a := Int.floor_le (y + t)
  have b := Int.lt_floor_add_one (y + t)
  rw [abs_lt]
  constructor <;> linarith

/-- The right side's error: `x − ⌊F + t⌋` is within 1 of `x − F`. -/
theorem right_err (x y t : ℝ) (h0 : 0 ≤ t) (h1 : t < 1) :
    |(x - ⌊y + t⌋) - (x - y)| < 1 := by
  have := floor_err y t h0 h1
  rw [show (x - ⌊y + t⌋) - (x - y) = -((⌊y + t⌋ : ℝ) - y) by ring, abs_neg]
  exact this

/-- A share with slope in `[0, 1]` is 1-Lipschitz. -/
theorem left_lipschitz (f : ℝ → ℝ) (hf : ∀ x y, x ≤ y → f x ≤ f y ∧ f y - f x ≤ y - x)
    (x y : ℝ) : |f x - f y| ≤ |x - y| := by
  rcases le_total x y with h | h
  · obtain ⟨a, b⟩ := hf x y h
    rw [abs_of_nonpos (by linarith), abs_of_nonpos (by linarith)]
    linarith
  · obtain ⟨a, b⟩ := hf y x h
    rw [abs_of_nonneg (by linarith), abs_of_nonneg (by linarith)]
    linarith

/-- So is the right side's expectation `x − f(x)`. -/
theorem right_lipschitz (f : ℝ → ℝ) (hf : ∀ x y, x ≤ y → f x ≤ f y ∧ f y - f x ≤ y - x)
    (x y : ℝ) : |(x - f x) - (y - f y)| ≤ |x - y| := by
  rcases le_total x y with h | h
  · obtain ⟨a, b⟩ := hf x y h
    rw [abs_of_nonpos (by linarith), abs_of_nonpos (by linarith)]
    linarith
  · obtain ⟨a, b⟩ := hf y x h
    rw [abs_of_nonneg (by linarith), abs_of_nonneg (by linarith)]
    linarith

/-- **Nested discrepancy.** Level `k` maps the previous count through a
1-Lipschitz `g k`; the realized count `a (k+1)` is within 1 of `g k (a k)`, and
the expectation `x (k+1) = g k (x k)`. After `d` levels the realized count is
within `d` of the expectation (plus the start's error). -/
theorem nested (g : ℕ → ℝ → ℝ) (hg : ∀ k x y, |g k x - g k y| ≤ |x - y|)
    (a x : ℕ → ℝ) (ha : ∀ k, |a (k + 1) - g k (a k)| ≤ 1)
    (hx : ∀ k, x (k + 1) = g k (x k)) (d : ℕ) :
    |a d - x d| ≤ |a 0 - x 0| + d := by
  induction d with
  | zero => simp
  | succ d ih =>
    rw [hx d]
    calc |a (d + 1) - g d (x d)|
        ≤ |a (d + 1) - g d (a d)| + |g d (a d) - g d (x d)| := abs_sub_le _ _ _
      _ ≤ 1 + |a d - x d| := add_le_add (ha d) (hg d _ _)
      _ ≤ 1 + (|a 0 - x 0| + d) := by linarith
      _ = |a 0 - x 0| + ↑(d + 1) := by push_cast; ring

/-- **Two sides of a market.** If the women's and the men's realized cumulative
seats are each within `d` of the same expectation, they differ by at most `2d`:
the queue pairing the `k`-th woman's seat with the `k`-th man's stays within
`2d` seats, whatever the number of cohorts. -/
theorem queue_offset (w m e d : ℝ) (hw : |w - e| ≤ d) (hm : |m - e| ≤ d) :
    |w - m| ≤ 2 * d := by
  calc |w - m| = |(w - e) - (m - e)| := by ring_nf
    _ ≤ |w - e| + |m - e| := abs_sub _ _
    _ ≤ 2 * d := by linarith

/-! ## 5. Exact pieces -/

/-- **Select on a linear piece.** With `L(m) = ⌊(A + m·P)/Q⌋` in integers
(`0 < P`, `0 < Q`, and the piece's offset `A ≤ (j+1)·Q`), `L(m)` reaches
`j + 1` exactly from `m = ⌈((j+1)·Q − A)/P⌉` on. -/
theorem select_piece {Q P A : ℕ} (hQ : 0 < Q) (hP : 0 < P) (j m : ℕ)
    (hA : A ≤ (j + 1) * Q) :
    j + 1 ≤ (A + m * P) / Q ↔ ((j + 1) * Q - A + P - 1) / P ≤ m := by
  rw [Nat.le_div_iff_mul_le hQ, Nat.div_le_iff_le_mul_add_pred hP]
  have e : (j + 1) * Q - A + P - 1 = ((j + 1) * Q - A) + (P - 1) := by omega
  rw [e, mul_comm P m]
  omega

/-! ## 6. Weighted discrepancy: constant error for Huffman shapes -/

/-- The error bound along a path whose level `k` has Lipschitz constant
`λ k`: `e 0 = 0`, `e (d+1) = 1 + λ d · e d`. -/
def errBound (lam : ℕ → ℝ) : ℕ → ℝ
  | 0 => 0
  | d + 1 => 1 + lam d * errBound lam d

/-- **Weighted nested discrepancy.** If level `k` is `λ k`-Lipschitz (for a
Beatty node, `λ` is the child's share of its parent's mass), the realized count
after `d` levels is within `errBound λ d` of the expectation. Earlier errors are
damped by every later share. -/
theorem nested_weighted (g : ℕ → ℝ → ℝ) (lam : ℕ → ℝ) (hlam : ∀ k, 0 ≤ lam k)
    (hg : ∀ k x y, |g k x - g k y| ≤ lam k * |x - y|)
    (a x : ℕ → ℝ) (ha : ∀ k, |a (k + 1) - g k (a k)| ≤ 1)
    (hx : ∀ k, x (k + 1) = g k (x k)) (h0 : a 0 = x 0) (d : ℕ) :
    |a d - x d| ≤ errBound lam d := by
  induction d with
  | zero => simp [errBound, h0]
  | succ d ih =>
    rw [hx d, errBound]
    calc |a (d + 1) - g d (x d)|
        ≤ |a (d + 1) - g d (a d)| + |g d (a d) - g d (x d)| := abs_sub_le _ _ _
      _ ≤ 1 + lam d * |a d - x d| := add_le_add (ha d) (hg d _ _)
      _ ≤ 1 + lam d * errBound lam d := by
          have := mul_le_mul_of_nonneg_left ih (hlam d)
          linarith

/-- The bound is non-negative. -/
theorem errBound_nonneg (lam : ℕ → ℝ) (hlam : ∀ k, 0 ≤ lam k) (d : ℕ) :
    0 ≤ errBound lam d := by
  induction d with
  | zero => simp [errBound]
  | succ d ih =>
    rw [errBound]
    have := mul_nonneg (hlam d) ih
    linarith

/-- **Constant discrepancy.** If shares are at most 1 and every two
consecutive levels at least halve the mass (`λ k · λ (k+1) ≤ ½`), the error is
at most 4 at every depth. -/
theorem errBound_le_four (lam : ℕ → ℝ) (h0 : ∀ k, 0 ≤ lam k) (h1 : ∀ k, lam k ≤ 1)
    (h2 : ∀ k, lam k * lam (k + 1) ≤ 1 / 2) (d : ℕ) : errBound lam d ≤ 4 := by
  induction d using Nat.strong_induction_on with
  | _ d ih =>
    match d with
    | 0 => simp [errBound]
    | 1 => norm_num [errBound]
    | d + 2 =>
      have hd := ih d (by omega)
      have hn := errBound_nonneg lam h0 d
      simp only [errBound]
      have hp := h2 d
      have key : lam (d + 1) * (lam d * errBound lam d) ≤ 1 / 2 * 4 := by
        calc lam (d + 1) * (lam d * errBound lam d)
            = (lam d * lam (d + 1)) * errBound lam d := by ring
          _ ≤ 1 / 2 * errBound lam d :=
              mul_le_mul_of_nonneg_right hp hn
          _ ≤ 1 / 2 * 4 := by linarith
      nlinarith [h1 (d + 1), h0 (d + 1)]

/-- **Huffman shapes halve every two levels.** In a tree where a node's
sibling outweighs each of that node's children (Huffman trees do: when two
nodes merge, every other live node outweighs both, and so does anything later
built from them), every grandchild has at most half its grandparent's mass. -/
theorem grandchild_half (v c₁ c₂ w : ℝ) (hv : v = c₁ + c₂) (hw : w ≤ c₁)
    (huncle : w ≤ c₂) : w ≤ v / 2 := by
  linarith

/-- So along any path of a Huffman-shaped tree, consecutive shares satisfy
`λ k · λ (k+1) ≤ ½`: the share products are grandchild over grandparent. -/
theorem shares_halve (v c w : ℝ) (hv : 0 < v) (hc : 0 < c) (hw : w ≤ v / 2) :
    (c / v) * (w / c) ≤ 1 / 2 := by
  rw [div_mul_div_comm, mul_comm c w, ← div_mul_div_comm, div_self hc.ne', mul_one]
  rw [div_le_iff₀ hv]
  linarith

/-! ## 7. Exact totals: matching the two sides of a market without a queue -/

/-- **Exact totals, raising.** A valid split `N` (steps in `[0, 1]`, from 0)
of `M` positions whose expected left total `N M` falls short of a target
`W ≤ M` is blended with the identity: `F = (1 − θ)·N + θ·n`,
`θ = (W − N M)/(M − N M)`. Each step of `F` is a convex combination of a step
of `N` and 1, so still in `[0, 1]`; and `F M = W` exactly. -/
theorem exact_total_up (N : ℕ → ℝ) (M : ℕ) (W : ℝ)
    (hstep : ∀ n, 0 ≤ N (n + 1) - N n ∧ N (n + 1) - N n ≤ 1) (h0 : N 0 = 0)
    (hlo : N M ≤ W) (hhi : W ≤ M) (hroom : N M < M) :
    let θ := (W - N M) / (M - N M)
    let F := fun n : ℕ => (1 - θ) * N n + θ * n
    (∀ n, 0 ≤ F (n + 1) - F n ∧ F (n + 1) - F n ≤ 1) ∧ F 0 = 0 ∧ F M = W := by
  intro θ F
  have hpos : (0 : ℝ) < M - N M := by linarith
  have hθ0 : 0 ≤ θ := div_nonneg (by linarith) hpos.le
  have hθ1 : θ ≤ 1 := (div_le_one hpos).mpr (by linarith)
  refine ⟨fun n => ?_, ?_, ?_⟩
  · obtain ⟨a, b⟩ := hstep n
    have e : F (n + 1) - F n = (1 - θ) * (N (n + 1) - N n) + θ := by
      simp only [F]; push_cast; ring
    rw [e]
    constructor
    · nlinarith
    · nlinarith
  · simp [F, h0]
  · simp only [F]
    have : θ * (M - N M) = W - N M := div_mul_cancel₀ _ hpos.ne'
    nlinarith

/-- **Exact totals, lowering.** If the expected total exceeds the target
(`0 ≤ W ≤ N M`), scale: `F = (W / N M)·N`, steps in `[0, 1]`, `F M = W`. -/
theorem exact_total_down (N : ℕ → ℝ) (M : ℕ) (W : ℝ)
    (hstep : ∀ n, 0 ≤ N (n + 1) - N n ∧ N (n + 1) - N n ≤ 1) (h0 : N 0 = 0)
    (hW : 0 ≤ W) (hhi : W ≤ N M) (hpos : 0 < N M) :
    let c := W / N M
    let F := fun n : ℕ => c * N n
    (∀ n, 0 ≤ F (n + 1) - F n ∧ F (n + 1) - F n ≤ 1) ∧ F 0 = 0 ∧ F M = W := by
  intro c F
  have hc0 : 0 ≤ c := div_nonneg hW hpos.le
  have hc1 : c ≤ 1 := (div_le_one hpos).mpr hhi
  refine ⟨fun n => ?_, ?_, ?_⟩
  · obtain ⟨a, b⟩ := hstep n
    have e : F (n + 1) - F n = c * (N (n + 1) - N n) := by simp only [F]; ring
    rw [e]
    constructor
    · exact mul_nonneg hc0 a
    · nlinarith
  · simp [F, h0]
  · simp only [F, c]
    field_simp

/-- **The realized total is the target.** With an integer target `W` and any
offset `t ∈ [0, 1)`, `⌊F M + t⌋ = W`: the men's side of every node of a market
then has exactly the women's realized count, at every level, so the two sides
of every year's line match without a queue. -/
theorem floor_total (W : ℤ) (t : ℝ) (h0 : 0 ≤ t) (h1 : t < 1) :
    ⌊(W : ℝ) + t⌋ = W := by
  rw [Int.floor_eq_iff]
  constructor <;> push_cast <;> linarith

end Internot.Lipschitz
