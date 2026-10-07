import Mathlib

/-!
# The cell world: the lemmas its exactness rests on

`research/2026-10-02-zero-world.md` (math draft `.scratch/zero/math.md`).

* **B** (births): child `j` of a block of `c` children over `M` mothers has
  mother `⌊j·M/c⌋`; mother `m`'s children are an exact range.
* **Q1** (the queue): with the running minimum `m` of the excess `G = D − S`
  and `U = S + m`, no step of `U` exceeds the supply's step, `U` never falls
  when demand does not, and `U ≤ D`.
* **Q2**: a rounded step is at most the ceiling of the real step.
* **Q3**: if supply after every point covers demand after it, `U` ends at `D`.
* **A** (acyclicity): a chain along which a rank in `{0, …, 6}` strictly
  falls has at most 6 steps.
-/

namespace CellWorld

/-! ## B: children choose mothers -/

/-- Child `j`'s mother is `m` iff `m·c ≤ j·M < (m+1)·c`. -/
theorem mother_iff (j M c m : ℕ) (hc : 0 < c) :
    j * M / c = m ↔ m * c ≤ j * M ∧ j * M < (m + 1) * c := by
  constructor
  · rintro rfl
    exact ⟨Nat.div_mul_le_self _ _, (Nat.div_lt_iff_lt_mul hc).mp (Nat.lt_succ_self _)⟩
  · rintro ⟨h1, h2⟩
    have a := (Nat.le_div_iff_mul_le hc).mpr h1
    have b := (Nat.div_lt_iff_lt_mul hc).mpr h2
    omega

/-- The lower end of mother `m`'s range: `m·c ≤ j·M ⟺ ⌈m·c/M⌉ ≤ j`. -/
theorem first_child_iff (j M c m : ℕ) (hM : 0 < M) :
    m * c ≤ j * M ↔ (m * c + M - 1) / M ≤ j := by
  rw [Nat.div_le_iff_le_mul_add_pred hM, Nat.mul_comm M j]
  omega

/-! ## Q1: the reflected queue -/

variable (D S : ℕ → ℤ)

/-- The excess of demand over supply. -/
def G (n : ℕ) : ℤ := D n - S n

/-- Its running minimum. -/
def runMin : ℕ → ℤ
  | 0 => G D S 0
  | n + 1 => min (runMin n) (G D S (n + 1))

/-- The supply the queue uses. -/
def U (n : ℕ) : ℤ := S n + runMin D S n

theorem runMin_le (n : ℕ) : runMin D S n ≤ G D S n := by
  cases n with
  | zero => simp [runMin]
  | succ n => simp [runMin]

theorem runMin_anti (n : ℕ) : runMin D S (n + 1) ≤ runMin D S n := by
  simp [runMin]

/-- No step of the used supply exceeds the supply's step. -/
theorem step_le_supply (n : ℕ) : U D S (n + 1) - U D S n ≤ S (n + 1) - S n := by
  have := runMin_anti D S n
  simp only [U]; linarith

/-- The used supply never falls while demand does not. -/
theorem step_nonneg (n : ℕ) (hD : D n ≤ D (n + 1)) (hS : S n ≤ S (n + 1)) :
    U D S n ≤ U D S (n + 1) := by
  simp only [U, runMin]
  rcases le_total (runMin D S n) (G D S (n + 1)) with h | h
  · rw [min_eq_left h]; linarith
  · rw [min_eq_right h]
    have := runMin_le D S n
    simp only [G] at this h ⊢
    linarith

/-- The used supply never exceeds demand. -/
theorem used_le_demand (n : ℕ) : U D S n ≤ D n := by
  have := runMin_le D S n
  simp only [U, G] at this ⊢
  linarith

/-! ## Q2: rounding -/

/-- A rounded step is at most the ceiling of the real step. -/
theorem floor_step_le_ceil (a δ : ℝ) (_hδ : 0 ≤ δ) : ⌊a + δ⌋ - ⌊a⌋ ≤ ⌈δ⌉ := by
  have h1 := Int.floor_le a
  have h2 := Int.lt_floor_add_one (a + δ)
  have h3 := Int.le_ceil δ
  have : ((⌊a + δ⌋ - ⌊a⌋ : ℤ) : ℝ) < ⌈δ⌉ + 1 := by
    push_cast
    have := Int.floor_le (a + δ)
    have := Int.lt_floor_add_one a
    linarith
  exact_mod_cast Int.lt_add_one_iff.mp (by exact_mod_cast this)

/-! ## Q3: both sides end equal -/

theorem runMin_eq_last (N : ℕ) (h : ∀ n ≤ N, D N - D n ≤ S N - S n) :
    runMin D S N = G D S N := by
  have key : ∀ n ≤ N, G D S N ≤ runMin D S n := by
    intro n hn
    induction n with
    | zero =>
      have := h 0 (Nat.zero_le _)
      simp [runMin, G]; linarith
    | succ k ih =>
      have := h (k + 1) hn
      simp only [runMin, le_min_iff]
      exact ⟨ih (by omega), by simp only [G]; linarith⟩
  exact le_antisymm (runMin_le D S N) (key N le_rfl)

/-- If supply after every point covers demand after it, the queue's used
supply ends equal to demand: both sides of the bijection have one total. -/
theorem used_ends_at_demand (N : ℕ) (h : ∀ n ≤ N, D N - D n ≤ S N - S n) :
    U D S N = D N := by
  simp only [U, runMin_eq_last D S N h, G]; ring

/-! ## A: acyclicity -/

/-- A chain along which a rank bounded by 6 strictly falls has at most 6
steps: source recursion ends within 6 steps. -/
theorem chain_le_six {α : Type*} (rank : α → ℕ) (hr : ∀ a, rank a ≤ 6)
    (f : ℕ → α) (L : ℕ) (hf : ∀ i < L, rank (f (i + 1)) < rank (f i)) : L ≤ 6 := by
  have : ∀ i ≤ L, rank (f i) + i ≤ rank (f 0) := by
    intro i hi
    induction i with
    | zero => simp
    | succ k ih =>
      have := hf k (by omega)
      have := ih (by omega)
      omega
  have := this L le_rfl
  have := hr (f 0)
  omega

/-- The cell world's queues: first unions, and each sex's 1st to 3rd
re-partnerings. -/
inductive Q
  | one
  | w (l : Fin 3)
  | m (l : Fin 3)
  deriving DecidableEq

/-- Which queue's separated couples feed which re-partnering queue, in a
group of the given parity. -/
def feeds (odd : Bool) : Q → Q → Prop
  | Q.one, Q.w l => l.val = 0
  | Q.one, Q.m l => l.val = 0
  | Q.m _, Q.w l => odd ∧ l.val = 0
  | Q.w _, Q.m l => !odd ∧ l.val = 0
  | Q.w a, Q.w b => b.val = a.val + 1
  | Q.m a, Q.m b => b.val = a.val + 1
  | _, _ => False

/-- The topological rank: odd groups put men's queues first, even groups
women's. -/
def rank (odd : Bool) : Q → ℕ
  | Q.one => 0
  | Q.w l => if odd then 4 + l.val else 1 + l.val
  | Q.m l => if odd then 1 + l.val else 4 + l.val

/-- **Theorem A.** Every source edge strictly raises the rank (sources come
first), and ranks lie in `{0, …, 6}`: with `chain_le_six`, a couple's times
recurse at most 6 steps. Without the parity rule (`m → w` and `w → m` in one
group) no such rank exists. -/
theorem feeds_raises_rank (odd : Bool) (a b : Q) (h : feeds odd a b) :
    rank odd a < rank odd b := by
  cases a <;> cases b <;> cases odd <;> simp_all [feeds, rank] <;> omega

theorem rank_le_six (odd : Bool) (a : Q) : rank odd a ≤ 6 := by
  cases a <;> cases odd <;> simp [rank] <;> omega

end CellWorld

#print axioms CellWorld.mother_iff
#print axioms CellWorld.first_child_iff
#print axioms CellWorld.step_le_supply
#print axioms CellWorld.step_nonneg
#print axioms CellWorld.used_le_demand
#print axioms CellWorld.floor_step_le_ceil
#print axioms CellWorld.used_ends_at_demand
#print axioms CellWorld.chain_le_six
#print axioms CellWorld.feeds_raises_rank
