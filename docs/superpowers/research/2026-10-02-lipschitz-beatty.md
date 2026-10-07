# Lipschitz Beatty trees, and what the pure world must know at full granularity

**Date:** 2026-10-02 (founder: "keep doing research. When you find a good solution look for a better one … i really do mean it about creating new math with lean"; then "before adding it, do more looking … do research").
**Builds on:** `2026-10-01-pure-world.md` (the pure world), its §8 granularity risk.
**Proofs:** `proofs/2026-10-02-lipschitz-beatty.lean`. Every theorem is checked by Lean 4 with mathlib, on standard axioms only (`#print axioms`).
**Code so far:** `procedural_core::lattice::{SplitShape, LipschitzSplit, LipschitzTree, Shares::huffman}`, with tests and golden values (581 core tests pass). Nothing in `internot_society` uses them yet.
**Literature:** the session's web-search budget was exhausted, so the references below are from memory, not freshly checked.

## 1. The question

The pure world is flat in population (22 MB at 14M or 873M people) but grows with *granularity*. Heritage × areas is 5 × 62 = 310 groups, and every structure kept per (line, birth year), per cohort, per (child year, line, layer, plan group) or per re-partnering source multiplies by about 300.

So: what must an exact, age-ordered, two-sex world store per group, and what can be computed?

## 2. What is inherent (the floor), and what is not

**Marriage lines.** A year's line orders its seats by age, and a seat's position is the number of same-group, same-year seats born earlier. The seats per (group, birth year) are realized counts that differ by group, since each group has its own age structure.

Every escape route tried fails:
- **Consistent 2D rounding** (rows and columns both closed form): the 2D floor difference `⌊F(b+1,y+1)⌋ − ⌊F(b,y+1)⌋ − ⌊F(b+1,y)⌋ + ⌊F(b,y)⌋` is exactly consistent on both axes but can be −1. A rounding with both prefix families within 1 exists (the two interval families form a totally unimodular system, linear discrepancy < 1: Doerr), but only by a global flow computation, not locally.
- **Lattice points under curves** (Kronecker phases with piecewise-linear boundaries): counts are a few floor sums each, but select needs a search, about 50 µs.
- **Shapes shared across groups** (`K·Φ(n/K)`): this breaks the hard constraint that nobody partners before 16, because relative position in a group no longer fixes age.

So something of size groups × years × birth blocks is stored, or recomputed per lookup. The pure world's choice is how coarse and how cheap that is (§5).

**Births are the real cost.** Today a child's id is a cumulative count over (union line × layer × plan group) for its birth year: 4,320 entries per year, 4.6 MB for one group, about 1.8 GB for 310.

Births at one offset are a union of plan subtrees (one per first-offset × spacing group). The family "has a birth at offset o" lacks the consecutive-ones property, so no tree order makes it one interval; at least one count per plan group is unavoidable. Ids ordered by family (line, couple, birth number) would make mother ↔ child O(1), but the age-ordered index needed for marriage lines then becomes the same prefix problem.

## 3. New mathematics

### 3.1 Lipschitz Beatty splits

**Definition.** A Beatty node's left count `⌊n·p + t⌋` generalizes to `L(n) = ⌊F(n) + t⌋` for any `F` whose steps lie in `[0, 1]`: shares may vary along the positions.

Proven:
- `L_step`, `R_step`: both sides step by 0 or 1, so every position goes to exactly one side.
- `scaled_step`: a shape `Φ` on `[0, 1]` with slope in `[0, 1]` gives a valid split `F(n) = K·Φ(n/K)` at every size `K`. One shape serves every node, with nothing stored per node.
- `count_eq`: telescoping count.
- `select_galois`, `select_member`: select is the lower adjoint of count (a Galois connection).
- `path_galois`: Galois connections compose, so along any path of nested splits, select inverts count exactly.
- `select_piece`: on a linear piece `⌊(A + n·P)/Q⌋`, with any offset `A`, select is closed form.
- `nested`: if each level's realized count is within 1 of a 1-Lipschitz function of the previous level's, the realized count after `d` levels is within `d` of the composed expectation.
- `left_lipschitz`, `right_lipschitz`: both sides' expectations are 1-Lipschitz.

**Connection.** A Beatty tree is a **wavelet tree whose bitvectors are generated, not stored** (wavelet trees: Grossi, Gupta and Vitter 2003; as a 2D range-counting structure: Mäkinen and Navarro). Everything wavelet trees answer is then available at zero space: rank, select, counts over (index range × category range), range quantiles ("the k-th youngest among those married in 1990–99").

### 3.2 Constant discrepancy: Huffman-shaped Beatty trees

- `nested_weighted`: a level's error is damped by every later share. `e(d+1) = 1 + λ_d·e(d)`, where `λ` is the child's share of its parent's mass.
- `errBound_le_four`: if every two levels at least halve the mass (`λ_k·λ_{k+1} ≤ ½`), the error is at most **4 at every depth**.
- `grandchild_half`, `shares_halve`: trees where a node's sibling outweighs each of its children have that property. **Huffman trees do**: when two nodes merge, every other live node outweighs both, and so does anything later built from them. (The Huffman step itself is argued, not yet formalized.)
- **Result:** every category's count over every prefix stays within 4 of its share, and a category's access depth is its Huffman code length, about `log₂(1/share)`.
- Compare Tijdeman's chairman assignment theorem (1980): discrepancy below `1 − 1/(2k−2)`, but only by a sequential greedy, with no random access. Huffman-shaped wavelet trees are known for compression (Mäkinen and Navarro). A constant-discrepancy bound for generated (Beatty) trees is not something I know of.
- **Measured** (`lattice::tests`, 60 random skewed weight vectors with up to 41 categories, prefixes to 4,000): worst Huffman discrepancy 1.81 (bound 4); count-balanced shapes, as used now, 2.72.

### 3.3 Exact totals: two sides of a market without a queue

- `exact_total_up`: a valid split `N` of `M` positions whose expected total falls short of an integer target `W ≤ M` becomes `(1 − θ)·N + θ·n` with `θ = (W − N(M))/(M − N(M))`. Its steps are convex combinations of `N`'s steps and 1, so still valid, and its total is exactly `W`.
- `exact_total_down`: one whose expectation is over `W` is scaled by `W/N(M)`.
- `floor_total`: `⌊W + t⌋ = W` for integer `W` and any offset `t ∈ [0, 1)`.
- **Consequence:** give each node of the men's year tree the women's realized count as its target, top-down. Then men's and women's seats match in every year, every group and every node. The k-th-to-k-th queue goes, and men's union years no longer slip by a year.

## 4. Applying it: a better architecture (not built)

Per group (area × heritage), with ids ordered by (birth year, group) inside each cohort, so that a group's members form one index space through small per-(group, year) offset tables:

1. **First unions: one year tree per sex per group** over the group's members in birth order.
   - Node shapes come from national (or per-heritage) incidence and the group's cohort sizes at coarse knots (every 5 birth years, plus an exact knot at age 16).
   - A line's positions are category counts: no run starts, no per-cohort bounds, no segment tables.
   - Men's nodes take exact totals (§3.3).
2. **Coupling within a line:** unchanged (core/tail), with layers as a `LipschitzTree` on one shared shape per class, so nothing is stored per line.
3. **Plans: a Lipschitz tree over line positions** (fertility may vary with age, more realistic than plans by layer rank). Child ids then need cumulative counts per (child year, group, union line, plan group): ≈ 335 × 310 × 30 × 16 × 4 B ≈ 200 MB at full granularity; per (child year, group, union line), with plan groups summed on the fly (16 subtree counts, ~1 µs per child), ≈ 12 MB.
4. **Re-partnering:** separated partners in a per-group index space ordered by separation year, with re-partnering years as a year tree. Cumulative separations per (separation year, source line), ≈ 17 MB at full granularity, replace today's dense runs (≈ 450 MB at full granularity) and the dissolution-year sums.

**Estimated sizes:** at one group, under 2 MB, against 22 MB now. At full granularity, about 30–220 MB depending on the birth-table choice, against 5.6 GB for today's `us-areas` ledger.

**Lookups:** a year-tree descent computing node shapes on the fly costs about depth × knots ≈ 9 × 16 multiply-adds (an estimate, not measured). Storing node shapes per group (≈ 24 MB at full granularity) keeps it near today's speed. Measure before choosing.

## 5. Open problems, ranked

1. **Migration and arrivals with exact areas.** A person's marriage area must be where they live. Single moves before marriage put people into another group's index space; children who move with their families inherit their parents' move, which is not a pattern over the child's cohort. Options:
   - (a) single movers move in their union year, to partner where they move;
   - (b) families with children under 18 don't move between areas (a rule, so debt);
   - (c) an "abroad" group, with immigration as a move from it: immigrants then have parents, arriving couples are couple moves, children born abroad come with their parents;
   - (d) per-(birth year, destination, origin) offset tables (≈ 5 MB) for movers.
   Founder input needed: these trade realism against exactness and size.
2. **Child-id tables** (§4.3): the one structure that scales with groups × lines × plan groups. A plan structure where births at an offset form fewer subtrees would shrink it. The consecutive-ones obstruction says one subtree is impossible; few may not be.
3. **Huffman shapes for the plan and non-union trees** need their subtree runs (parity ≥ m) to survive a Huffman layout: weighted alphabetic (Hu–Tucker) shapes keep the order but may lose the halving property.
4. Formalize Huffman's construction in Lean, to close §3.2 fully.
