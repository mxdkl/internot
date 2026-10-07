# Areas and rosters in the monotone world (analysis; founder decision)

**Date:** 2026-10-04. **For:** the global world spec (`specs/2026-10-03-global-world.md`) P10–P12, residence (§7), work (§9), ties (§10).

## 1. The requirement

Every remaining feature that relates people to each other through a place needs a **roster**: given a place-like object and a time, the people in it, consistent both ways (G2):
- **coworkers:** `x ∈ roster(W, t) ⇔ employer(x, t) = W`;
- **classmates:** the same school, grade and year;
- **neighbours:** the same neighbourhood at `t`;
- **friends:** ties form inside foci (classes, workplaces, neighbourhoods; society spec §12), so friendship enumerates foci rosters;
- **residence realism:** partners must meet where they live, so the *kinship* layer must know where people live (the design-A finding: with national pairing, 43–56% of natives live outside their birth state against 21–34%, and 33–44% of adults 25+ have a parent within 30 miles against 59.8%; AGENTS.md "Residence").

Every construction of a roster reduces to one structure: **a counted (dense, closed-form) index of the people of a place**, so a static bijection can map it onto seats (society spec P3–P4) and preimages enumerate a seat's occupants. The monotone world's index is `Pid { cell, y, i }` with `i` in **life order** (young, wives by union age, never-partnered; husband segments by wife cohort), which is not ordered by place.

## 2. Options

### A. Areas as kinship cells, built lazily
Cells become (country, area, group). Each cell is a monotone world of its own (classes, eligibility, births, bridge, union spaces), with an open market per area, a national share, and movers between areas (`research/2026-10-02-cell-world-math.md` §17.3–17.4, adapted).
- **Gains:** couples meet in their area; children are born in their mother's area; a cell's people are a counted index, so area rosters exist (people of area A at `t` = the cell's living, plus or minus movers by their move times).
- **Cost:** per-cell state is fixed per cohort, not per person: about 14 KB per cell cohort today (class records ~2.4 KB, eligibility rows and prefixes ~6 KB, interleaves ~1 KB, Coh ~1.3 KB), so about 4 MB per cell. The US with 62 areas × 5 groups is 310 cells, about 1.2 GB eagerly. Lazy per-area builds (5 cells, ~20 MB) need an eager **census** first (sizes, eligible-mother totals for the births split, mover flows: cheap, ~60–100 ms parallel for 310 cells), because births and movers couple cells. A session touching 5 areas holds ~100 MB.

### B. Areas as runs inside group cells
Keep cells = groups; order every class's members by area (area runs), deal births into life classes by area (an area-preserving bridge), lay husband segments out by area (a systematic contingency of segment sizes × area counts), so rank-aligned couplings pair mostly same-area partners (Lemma M: minimal crossing).
- **Gains:** few cells; a cohort's people of area A are runs in every class: closed-form counts and rosters.
- **Cost:** births by area need eligible mothers by (class, area), so eligibility rows multiply by the number of areas (the same ×62 as A), or are computed on the fly; every lookup decodes runs. The cell world measured 5–20× slower lookups with labelled matching (§17.2); the monotone world's interleaves may do better, unmeasured.

### C. Computed classes: cells without per-class state
Replace the stored per-cohort structures with computations from a few numbers per cell cohort plus shared tables:
- **class sizes** from the union-age law (`mb[j] = qrc(wives, F(a0 + j)/top)`; the log-logistic CDF with an integer shape is a few multiplications);
- **eligible-mother prefixes** at year end from one death threshold per (group, cohort, year) (area-independent: the country tables and the group's AFT scaling; ~0.6 MB for the US) and the class sizes, summed with SIMD (`Σ round(n_j·w)`), instead of 64-byte rows and u32 prefixes;
- **class permutations** with an O(1) inverse (a power-of-two modulus with cycle walking, or a stored table of golden pairs by size) instead of a stored record per class;
- union tables and interleaves per space as now (they are per wife cohort, not per class).
- **Gains:** per-cell state ~64 bytes per cohort, so 310 US cells or every country's cells fit in a few MB eagerly; A's structure (areas as cells, migration, rosters) at B's memory.
- **Cost:** lookups 1.5–3× slower (mother: a threshold read, a class search with SIMD prefix sums; death: a class-size computation and a cycle-walked permutation), against the 2026-10-03 speed pass. Joint counts that rely on two affine maps (separated and alive) need a new closed form if the permutation family changes.

## 3. Recommendation

**C, then A on top of it.** C makes cells nearly free, so the global world (thousands of country × area × group cells) can be eager or lazily built at negligible memory, and A's structure (areas as cells with movers) gives realistic residence, migration and the counted indexes every roster needs. The price is lookup speed, which the founder set as a requirement: prototype C's mother and death lookups first and measure (rule 2: prototype the risky part), then decide.

Until then: features that need no roster are built (careers and employers, chosen near home; individual attributes). Coworkers, classmates, neighbours and friends wait.
