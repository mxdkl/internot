import Mathlib

/-!
# The cell world: immigrants (math §10)

* **P**: arrival counts `P(Φ) = ⌊(2TΦ + Φ∞)/(2Φ∞)⌋` are monotone, start at 0
  and end at `T`; each member arrives in exactly one year.
* **A′**: the queue graph with the immigrant first unions (`iw`, `im`, `ac`)
  still has a rank in `{0, …, 6}` that every source edge raises.
-/

namespace CellWorldImm

/-- Members arrived for an expected cumulative `Φ` (of `Φinf`), `T` members. -/
def P (T Φinf Φ : ℕ) : ℕ := (2 * T * Φ + Φinf) / (2 * Φinf)

theorem P_mono (T Φinf : ℕ) {a b : ℕ} (h : a ≤ b) : P T Φinf a ≤ P T Φinf b := by
  unfold P
  apply Nat.div_le_div_right
  have := Nat.mul_le_mul_left (2 * T) h
  omega

theorem P_zero (T Φinf : ℕ) (h : 0 < Φinf) : P T Φinf 0 = 0 := by
  unfold P
  simp only [mul_zero, zero_add]
  exact Nat.div_eq_of_lt (by omega)

theorem P_full (T Φinf : ℕ) (h : 0 < Φinf) : P T Φinf Φinf = T := by
  unfold P
  have : 2 * T * Φinf + Φinf = Φinf + T * (2 * Φinf) := by ring
  rw [this, Nat.add_mul_div_right _ _ (by omega), Nat.div_eq_of_lt (by omega)]
  simp

/-- A monotone count that starts at or below `j` and ends above it steps
over `j` at exactly one point. -/
theorem exists_unique_step (f : ℕ → ℕ) (hf : Monotone f) (s n j : ℕ)
    (hs : f s ≤ j) (hn : j < f n) :
    ∃! e, s ≤ e ∧ e < n ∧ f e ≤ j ∧ j < f (e + 1) := by
  have hsn : s < n := by
    by_contra h
    have := hf (show n ≤ s by omega)
    omega
  -- the last point at or below `j`
  classical
  let S := (Finset.Icc s n).filter (fun e => f e ≤ j)
  have hne : S.Nonempty := ⟨s, by simp [S, hs, hsn.le]⟩
  let e := S.max' hne
  have he : e ∈ S := S.max'_mem hne
  simp only [S, Finset.mem_filter, Finset.mem_Icc] at he
  have hen : e < n := by
    rcases lt_or_eq_of_le he.1.2 with h | h
    · exact h
    · rw [h] at he; omega
  refine ⟨e, ⟨he.1.1, hen, he.2, ?_⟩, ?_⟩
  · by_contra hc
    push Not at hc
    have : e + 1 ∈ S := by simp [S]; omega
    have := S.le_max' _ this
    omega
  · rintro e' ⟨h1, h2, h3, h4⟩
    have hmem : e' ∈ S := by simp [S]; omega
    have hle : e' ≤ e := S.le_max' _ hmem
    rcases lt_or_eq_of_le hle with h | h
    · have := hf (show e' + 1 ≤ e by omega)
      omega
    · exact h

/-- **Lemma P.** Member `j < T` arrives in exactly one year: the year where
the arrival count steps over `j`. -/
theorem arrival_year_unique (T Φinf : ℕ) (Φ : ℕ → ℕ) (hΦ : Monotone Φ) (s n j : ℕ)
    (hpos : 0 < Φinf) (h0 : Φ s = 0) (h1 : Φ n = Φinf) (hj : j < T) :
    ∃! e, s ≤ e ∧ e < n ∧ P T Φinf (Φ e) ≤ j ∧ j < P T Φinf (Φ (e + 1)) := by
  apply exists_unique_step (fun e => P T Φinf (Φ e)) (fun a b h => P_mono T Φinf (hΦ h))
  · simp [h0, P_zero T Φinf hpos]
  · simp [h1, P_full T Φinf hpos, hj]

/-! ## A′: acyclicity with immigrant first unions -/

inductive Q
  | one | iw | im | ac
  | w (l : Fin 3)
  | m (l : Fin 3)
  deriving DecidableEq

/-- First unions of every kind (native, immigrant single women, immigrant
single men, arriving couples) feed both sexes' first re-partnering. -/
def first : Q → Bool
  | Q.one | Q.iw | Q.im | Q.ac => true
  | _ => false

def feeds (odd : Bool) : Q → Q → Prop
  | Q.w _, Q.m l => !odd ∧ l.val = 0
  | Q.m _, Q.w l => odd ∧ l.val = 0
  | Q.w a, Q.w b => b.val = a.val + 1
  | Q.m a, Q.m b => b.val = a.val + 1
  | a, Q.w l => first a ∧ l.val = 0
  | a, Q.m l => first a ∧ l.val = 0
  | _, _ => False

def rank (odd : Bool) : Q → ℕ
  | Q.w l => if odd then 4 + l.val else 1 + l.val
  | Q.m l => if odd then 1 + l.val else 4 + l.val
  | _ => 0

theorem feeds_raises_rank (odd : Bool) (a b : Q) (h : feeds odd a b) :
    rank odd a < rank odd b := by
  cases a <;> cases b <;> cases odd <;> simp_all [feeds, rank, first] <;> omega

theorem rank_le_six (odd : Bool) (a : Q) : rank odd a ≤ 6 := by
  cases a <;> cases odd <;> simp [rank] <;> omega

end CellWorldImm

#print axioms CellWorldImm.P_mono
#print axioms CellWorldImm.P_full
#print axioms CellWorldImm.arrival_year_unique
#print axioms CellWorldImm.feeds_raises_rank
#print axioms CellWorldImm.rank_le_six

/-! ## A″: re-partnering by chain depth, no parity -/

namespace CellWorldDepth

/-- Queues by kind: first unions (depth 0) and each sex's re-partnering at
depth `d ≥ 1`. -/
inductive Q
  | first
  | w (d : ℕ)
  | m (d : ℕ)
  deriving DecidableEq

def depth : Q → ℕ
  | Q.first => 0
  | Q.w d => d
  | Q.m d => d

/-- Every separated couple of depth `d − 1` feeds both sexes' queues at
depth `d`, in every group (no parity condition). -/
def feeds : Q → Q → Prop
  | a, Q.w d => 1 ≤ d ∧ depth a + 1 = d
  | a, Q.m d => 1 ≤ d ∧ depth a + 1 = d
  | _, Q.first => False

theorem feeds_raises_depth (a b : Q) (h : feeds a b) : depth a < depth b := by
  cases b <;> simp_all [feeds, depth] <;> omega

/-- So no chain of sources returns to where it began. -/
theorem no_cycle (f : ℕ → Q) (L : ℕ) (hL : 0 < L) (hf : ∀ i < L, feeds (f (i + 1)) (f i)) :
    f L ≠ f 0 := by
  have : ∀ i ≤ L, depth (f i) + i ≤ depth (f 0) := by
    intro i hi
    induction i with
    | zero => simp
    | succ k ih =>
      have := feeds_raises_depth _ _ (hf k (by omega))
      have := ih (by omega)
      omega
  intro h
  have := this L le_rfl
  rw [h] at this
  omega

/-- Chains of sources from a depth-`D` couple have at most `D` steps. -/
theorem chain_le_depth (f : ℕ → Q) (L : ℕ) (hf : ∀ i < L, feeds (f (i + 1)) (f i)) :
    L ≤ depth (f 0) := by
  have : ∀ i ≤ L, depth (f i) + i ≤ depth (f 0) := by
    intro i hi
    induction i with
    | zero => simp
    | succ k ih =>
      have := feeds_raises_depth _ _ (hf k (by omega))
      have := ih (by omega)
      omega
  have := this L le_rfl
  omega

end CellWorldDepth

#print axioms CellWorldDepth.feeds_raises_depth
#print axioms CellWorldDepth.no_cycle
#print axioms CellWorldDepth.chain_le_depth
