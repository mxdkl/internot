# The pure world: kinship without a stored ledger

**Date:** 2026-10-01, overnight (founder: "take the whole night and try solve the memory problem … the dream, everything is f(t, seed, id), and everything agrees with each other without having to worry, (and its really freaking fast)"; clock speed must not be sacrificed).
**Builds on:** `2026-10-01-memory-local-generation.md` (local access, rounding theory), `2026-10-01-two-sex-and-microsimulation.md` (two-sex models, matching in microsimulation).
**Status:** design and prototype (`internot_society::pure`); results in §6. Machine-checked lemmas: `proofs/2026-10-01-pure-world.lean`, `proofs/2026-10-01-beatty-trees.lean`.

## 1. What is stored today, and why

The ledger is a two-sex cohort-component projection whose integer counts feed back year to year (who is still single depends on every earlier market), so it is computed once and kept: 1.31 GB of layouts for `us` (§ "The memory problem" in AGENTS.md has the breakdown), 5.6 GB for `us-areas`. It grows with the number of union cells, about population^0.55, because at prototype scale cells are nearly as fine as couples (61% of `us-areas` cells hold one couple).

Four couplings make it global:

| | Coupling | Why it is global |
|---|---|---|
| G1 | Two-sex balance: every cell's count is the same seen from either side | Each side's counts come from its own pools |
| G2 | Capacity: a block's members are used at most once, while alive | Counts must respect what earlier years used |
| G3 | Renewal: births make the next cohorts, and ids are dense | A child's id is a running total over all mothers |
| G4 | The market recursion: pools depend on realized earlier markets | Integer feedback |

## 2. The construction

Every count is a **closed-form rounding of an expected value**, and every two-sided relation is a **monotone coupling on a shared line**. Nothing per person or per cell is stored; the stored state is a few small tables per year.

### 2.1 Blocks partition themselves (G2, exact)

A block (birth year, sex) of `N` members is ranked by a keyed permutation. Its members are split over first-union years by systematic rounding of its expected cumulative incidence `C(y)`: rank `r` partners in year `y` iff `⌊N·C(y) + u⌋ ≤ r < ⌊N·C(y+1) + u⌋`. Counts are within 1 of expectation, sum exactly to `N`, and need no state. Capacity holds by construction: each member has one first-union year (or none).

### 2.2 Women's seats are authoritative; men queue (G1, exact, no state)

Women's year-`y` seats `W(y) = Σ_b W_b(y)` and men's `M(y)` come from their own partitions, so they differ by rounding noise. Instead of carrying unmatched seats in a stored queue, **order all women's seats of all time** by (year, block, rank), and all men's seats likewise, and pair the `k`-th woman of all time with the `k`-th man of all time. The offset between the two sequences at year `y` is the cumulative imbalance `Mc(y) − Wc(y)`. Each block's cumulative count is within 1 of its expectation (§2.1), and the mean-field (§2.5) makes expected totals equal each year, so the offset stays bounded by the number of active blocks (typically a handful of seats).
- A woman's union year is her partition year, so births are exact.
- A man's union year is the year of the woman he is paired with, at most a year away from his partition year at the boundaries. His death is conditioned on surviving to that actual year, which is closed form (one search over the stored `Wc`), so nobody partners after dying.

The two-sex note found carry queues bounded (mean 2.8, p99 9 seats) only with cumulative rounding; §2.1 is that rounding.

### 2.3 The coupling inside a year: mirrored layers (exact bijection, closed-form counts)

Inside year `y`, the `K = W(y)` women sit on a line `[0, K)` youngest first; the men matched that year sit on `[0, K)` too. A bijection σ between them must:
1. be computable both ways in O(log);
2. disperse partners' ages realistically (a plain rank match pairs every woman with a man of the same quantile: a near-constant age gap);
3. keep **counts over a block's run closed form on both sides**, so couple attributes (dissolution, plans) can be counted per block for births and survival.

**Mirrored layers.** Split `[0, K)` into `S` segments. Assign each woman's position to one of `L` layers by a Kronecker pattern (§3) whose layer shares depend on the segment (piecewise constant). Men's layer membership is the women's pattern **mirrored**: man `q` is in layer `l` iff position `K − 1 − q` is. Layer totals are therefore equal on both sides by construction. Inside a layer, pair by rank: the women's `j`-th member with the men's `j`-th member.
- A layer with a flat profile is a rank match.
- A layer whose share rises with age holds older women and (mirrored) younger men, so its couples have smaller gaps; a falling profile, larger gaps.
- A mixture of layers with profiles `∝ exp(β_l (x − ½))` spreads gaps around the base quantile match.

In layer coordinates `(l, j)`, a block's run on either side is one rank range per layer. So any couple attribute defined as a pattern on `j` has closed-form counts per block on both sides, the men's side included. That is what makes divorced men countable without the "third margin" problem of R1c.

### 2.4 Couple attributes and births (G3)

A couple's plan (parity, first-birth offset, spacing) is one categorical **leaf**, drawn by a Kronecker pattern on its `(l, j)` coordinate. Leaves are ordered by `(first offset, spacing, parity)`. For each `(first offset, spacing)` group, "a birth at the `m`-th offset" is then a contiguous run of leaves (parity ≥ m), so births at any offset over any rank range cost one floor-sum pair per group. Dissolution is a separate pattern on the same coordinate (§5). Births don't depend on it: the plan is the woman's lifetime plan, and fathers are whoever she is with at conception.

**Child ids.** Year `c`'s children are ordered by their mother's couple coordinate: (line `y`, layer `l`, group `g`, rank `j`). A small stored prefix table per child year (lines × layers × groups) maps a child's index to `(y, l, g)`; a Kronecker select inside the group's leaf run finds `j`. That gives the mother; the father is her partner if he is alive at conception. The reverse direction is one floor-sum count.

### 2.5 Mean field instead of realized pools (G4)

Block partitions use expected incidence from a deterministic two-sex projection: women's first-union hazards from the pack, men's scaled each year so expected men's seats equal women's (one number per year). No realized count feeds back, so every year's detail is a pure function of small tables. The two-sex note: real squeezes changed timing and gaps more than ever-married shares, and the mean field reproduces aggregate squeezes but not chance local ones.

### 2.6 What is stored

| Table | Size | Grows with |
|---|---|---|
| per block: size, cumulative incidence by year | ~100 floats per block | birth years × groups |
| per year: `Wc`, `Mc`, men's balance factor, per-block seat prefixes | ~200 numbers per year | years × blocks |
| per year: layer counts by segment | `S × L` | years |
| per child year: birth prefix by (line, layer, group) | ~2,500 | years |
| life tables | 120 per block | birth years |

None of it grows with population; all of it fits in a few MB.

## 3. The primitive: Kronecker patterns with floor-sum counts

`procedural_core::lattice::Kronecker`. Position `i` has phase `(a·i + t) mod 2³²`, with `a/2³² ≈ α = (√29 − 5)/2 = [0; 5, 5, 5, …]`, and a category by threshold interval.
- **Evenly spread:** any run of `n` positions holds `n·share ± O(log n)` of each category (measured under 12 in any run up to 2³⁰). Any slope with bounded partial quotients gives this; the golden ratio's all-1 quotients give the smallest constant.
- **Exact counts:** how many positions of a category fall in `[lo, hi)` is a difference of two floor sums `Σ⌊(a·i + b)/m⌋` (the `⌊v/m⌋` terms cancel between the two bounds). Each floor sum is computed by a Euclid recursion (AtCoder's `floor_sum`) in 64-bit arithmetic.
- **Why not the golden ratio.** The recursion folds `n` by the slope's partial quotient at each step. The golden ratio's quotients are all 1, the deepest case (about 30 steps for `n ≈ 10⁶`); quotients of 5 take about 8. Together with dropping 128-bit division, switching made counts about 10× faster, and discrepancy stays within a few counts.
- **Search:** the `j`-th member of a category: bracket at the expected place `lo + j/share`, bisect to a few dozen positions, then step (a category test is a multiply and a comparison).

So a pattern is a split of a set into categories that needs no storage, has exact counts over any range, and is evenly spread. That is the property every count in §2 needs.

### 3.1 Beatty trees: the faster primitive (what the prototype uses now)

Kronecker counts need Euclid recursions (floor sums), and selects need bisection. A **Beatty tree** does the same job in O(depth) integer arithmetic. Categories are the leaves of a binary tree whose every node splits its members, in position order, by a Beatty sequence: with left share `p/2³²` and offset `t`, the left members among a node's first `n` number `L(n) = ⌊(n·p + t)/2³²⌋`.
- **Count:** descend the category's path, `n ← L(n)` going left and `n ← n − L(n)` going right: a multiply and a shift per level.
- **Select:** ascend, inverting each step in closed form.
  - Left side: the smallest `m` with `L(m) ≥ j + 1` is `⌈((j+1)·2³² − t)/p⌉`.
  - Right side: the smallest `m` with `m − L(m) ≥ j + 1` is `⌊(j·2³² + t)/(2³² − p)⌋ + 1`. A Beatty sequence's complement is one too (Rayleigh), so this side inverts as well.
- **Category:** descend, testing `L(i+1) − L(i)`.
- **Evenness:** each split is within 1 of its share over every prefix (Beatty sequences are the most even), so a category's count over any range is within its depth of expectation.
- **Runs:** every subtree is a contiguous run of categories that counts and selects as one. Plans are combs `(0, (1, (2, …)))` inside each (first offset, spacing) group, so "parity ≥ m" is a subtree; non-union births are grouped by first age.
- **Shapes** depend only on the structure (a group splits by count, not mass). They are built once and shared, and a tree is just its per-node shares.
  - The first version split by mass, so trees of different years had different shapes and the precomputed run subtrees pointed at the wrong nodes. The exhaustive test caught it.
- **Gain:** against Kronecker patterns, mother 1.18 → 0.49 µs and siblings 2.7 → 1.5 µs at p50, and lookups no longer depend on population (§6).

## 4. What it cannot do (honest limits)

- **Chance squeezes** in small places, and individual path dependence: the mean field has neither.
- **Thin markets** (`us-areas`, about 10 couples per market-year) make the offset of §2.2 large relative to the market. The two-sex note measured 28% carried at 10 unions per market-year. Areas need coarser markets plus an overflow tier.
- **The coupling's realism** (age gaps, heritage mixing) has to be tuned through layer profiles rather than an IPF kernel. Kernels on classes can use Fisher–Yates layers exactly (memory note §3).
- Re-partnering, same-sex unions, immigration and areas are designed (lines fed by lines, §2.3) but not in the first prototype.

## 5. Re-partnering: lines fed by lines (the "third margin")

R1c had to store divorced-pool "sources" because divorced men's seats are a sum over couples, not a block partition. Here they are closed form.

- **Dissolution** is a Kronecker category `d` of the couple's rank in its layer (`0` never; `d` the `d`-th year after the union year). Its shares are the pack's bands, spread evenly within each band.
- **Re-partnering delay `r`** is a category of the partner's rank `j_d` among the couples of its `(line, layer, d)` group.
  - Women read the group's pattern at `j_d`; men read it at `(j_d + ⌊n_d/2⌋) mod n_d`, the delay of the couple half a group away.
  - A rotation is a bijection of the group, so every delay's count over the group is the same for both sexes (Lean: `rotation_count`). Every source therefore sends exactly as many men as women to each year's re-partnering line, and **the re-partnering queue offset is 0 in every year** (measured).
- **A re-partnering line** for year `Y` has one run per source `(line, layer)` with someone re-partnering in `Y`. Inside a run, partners are ordered by `(d, j_d)`. A run's size and anyone's offset in it are sums over `d` of floor-sum counts.
  - Going back, a seat finds its source couple by walking `d`, then two Kronecker selects.
  - Re-partnering lines are sources too, so third unions and later need nothing new.
- **Which re-partnerings happen.** One happens only if both partners are alive (and at most 95) at its start. Deaths depend only on first unions and births, so this is a fact of the couple, and both partners agree. One that doesn't happen is skipped, and its separation still leads to the next couple.
- **Fertility and fathers.** A woman's plan is her **lifetime** plan, anchored at her first union. Her children are all its births. A child's father is her partner at conception, if alive:
  - her first partner for conceptions before their separation (one just before the wedding included);
  - otherwise the re-partnering that spans it.

  A man's children are his partners' plan births conceived inside those windows. Paternal half-siblings follow.

## 6. Prototype results (`internot_society::pure`)

**Exactness** (`tests/pure.rs`, every person of the tiny world, 2 seeds):
- every union, re-partnerings included, is mutual with the same start and separation;
- unions follow one another;
- both partners are alive at every union;
- mother/children and father/children are dual, and fathers include later partners (about 550 children per seed);
- mothers are alive and aged 15–45 at every birth, and fathers alive at conception;
- every native has exactly one mother;
- siblings share a parent (about 2,300 paternal half-sibling links per seed).

**Machine-checked lemmas** (`proofs/2026-10-01-pure-world.lean` and `proofs/2026-10-01-beatty-trees.lean`, Lean 4 + mathlib, standard axioms only). The Beatty file proves the closed-form inversions behind select:
- each step adds 0 or 1 left members, and `L(0) = 0`;
- left: `j+1 ≤ L(m) ⟺ ⌈((j+1)Q − t)/p⌉ ≤ m`;
- right: `j+1 ≤ m − L(m) ⟺ ⌊(jQ + t)/(Q − p)⌋ < m`.

The other file proves:
- systematic rounding (each boundary and count within 1, exact ends, telescoping);
- the queue offset bound;
- the Kronecker floor-sum identity and its summed form;
- rotation and mirror count preservation;
- mirroring within a palindromic class set keeps each layer's size (`palindromic_mirror_count`, for the core/tail age-gap construction in §7);
- a cohort entering single at age `A` reads its cumulative incidence off its birth year's prefix sums (`cohort_from_prefix`, the reframing proposed in §8).

**Speed against today's world** (prototype scale with every feature above, Beatty trees; single-call ns, p50/p99):

| Lookup | Pure | World |
|---|---|---|
| death | 351/571 | 521/3,116 |
| mother | 381/941 | 812/1,893 |
| father | 702/2,635 | 2,414/11,021 |
| union | 441/761 | 2,445/10,800 |
| children | 521/6,622 | 1,824/15,249 |
| siblings | 1,163/9,748 | 2,244/22,613 |

Faster than today's world on every lookup, at p50 and p99 (2–14× at p99), with the core/tail coupling of §7 (free once each line stores its class split, `lattice::BeattySplit`). Re-partnering chains read stored dissolution group sizes (1 MB) and compute a person's first couple and death once per walk. With Kronecker patterns the dissolution-by-delay totals were stored too (20 MB); with Beatty trees recounting costs only about 10% at p99, so they were dropped.

**Speed pass (2026-10-02).** Profiling the slowest 1% of calls (`internot_perf/examples/pure_profile.rs`, `TAIL=1`) found binary searches and repeated work, not arithmetic. Each step below was checked by an identical answer checksum:
- `decode` (id → cohort) by a direct-address bucket index over 16,344 cohort bases, in place of a 14-step binary search. Almost every lookup calls it.
- A re-partnering seat's source: search the line's runs, then walk the run's dissolution years.
- Walks hand back each partner's first couple (`woman_first`, `man_first`), so the partner's death needs no second partition lookup.
- `father` resolves only the couple whose window spans the conception (a woman's later couples never overlap).
- A line's delay trees share one array (`lattice::BeattyForest`).
- Beatty counts from 0 skip the lower prefix (`L(0) = 0`), two-ended counts hash each node once, and select divides in 64 bits below rank 2³¹.
- Stored per-(run, year) starts removed the counting loops but saved only about 5% at p99, for 8 MB growing to about 28 MB with population; dropped.
- Build: first-union hazards tabled by (sex, birth year, age), since every cohort of a birth year recomputed them: 0.74 → 0.27 s.
- Dense re-partnering runs (every source year, kind and layer keeps a run, empty or not), so a source's run index is arithmetic, with `u32` run starts: children p99 7.4 → 6.6 µs, siblings 10.8 → 9.8 µs, tables 38.6 → 37.3 MB.

**Memory and scale** (`examples/pure_scale.rs`; with re-partnering, non-union births and immigration):

| Founders | People ever | Tables | Build | union | mother | children |
|---|---|---|---|---|---|---|
| ×1 | 13.6M | 22.3 MB | 0.28 s | 441/781 | 381/892 | 521/6,602 |
| ×4 | 55M | 23.6 MB | 0.29 s | 441/842 | 381/931 | 531/6,592 |
| ×16 | 218M | 24.7 MB | 0.29 s | 460/851 | 381/891 | 531/6,503 |
| ×64 | 873M | 25.9 MB | 0.30 s | 451/812 | 390/892 | 541/6,593 |

Lookups are in ns, p50/p99. Neither lookups nor build depend on population: Beatty counts cost O(tree depth). Today's world at ×16: 6.9 GB, 73 s.

**Non-union births and immigration** (added after re-partnering):
- **Non-union births.** A woman's non-union births are a Kronecker category of her block rank: a count, a first age and a gap. Ordered by (first age, count, gap), "a birth at age a" is a handful of contiguous runs, and child ids get a second segment per year by (mother block, run).
- **Immigration.**
  - Ids are dense by cohort, in entry order: founders, then each year's natives and that year's arrivals by age.
  - Lines keep runs by birth year; each run sums its cohorts.
  - Each cohort has its own partition, a survival floor and, except arrivals, a residual table. Life tables are shared by birth year.
  - Arrivals are single adults; they partner from the year after arriving.
  - Men are balanced each year to the women's exact seats, and the queue offset stays within ±52 seats.
- About 2,600 immigrants and 1,900 non-union natives per tiny-world seed, all exact (`tests/pure.rs`).
- **Debt:** ever partnered is 81–87% by cohort (today's world 88–92%). Immigrants arrive single, and many arrive at 30+. Arriving couples (their own line kind) and unions formed abroad are the fix.

**Realism** (`examples/pure_report.rs`, pure against today's world):

| Cohort | e0 F/M | CFR | Ever partnered F/M |
|---|---|---|---|
| 1880 | 50.6/47.4 against 54.6/51.4 | 3.53 against 3.35 | 97.7/95.6 against 94.6/85.3 |
| 1920 | 68.8/64.2 against 69.2/63.8 | 2.62 against 2.56 | 92.7/91.8 against 89.2/90.5 |
| 1950 | 77.2/71.2 against 80.0/73.9 | 2.36 against 2.28 | 95.5/94.8 against 92.4/92.4 |
| 1980 | 81.5/75.9 against 82.5/77.8 | 1.99 against 1.77 | 90.5/86.8 against 88.3/88.1 |

- e0 needed residual death tables: without them it was about 10 years too high in early cohorts.
- Remarried within 5/10 years of a separation: 38/56%, against NSFG 54/75%.

**Kin repair.** Without repair, close kin pair at 3.6e-4 per couple on the tiny world and 2.2e-6 on the prototype: 14 couples, 2 of them parent and child.
- Like the ledger's repair (R1 plan D-R1.7), a build pass checks every line's couples.
- **Lines are repaired in year order,** reading the final repairs of earlier years. A couple's man depends on its own line's repairs and, in a re-partnering, on his earlier couples' lines (2+ years before). Partners' parents are couples of 17+ years before. So each check sees the people as they are in the finished world, and the guarantee holds by construction, except for a pair whose every arrangement is related (counted by the tests; none found). A first all-parallel version read default couples everywhere and could, in principle, miss a repaired couple's children; it made the same swaps on the prototype.
- In each layer, a rank pair `(2k, 2k+1)` holding a close-kin couple has its men swapped, when the swap leaves both couples unrelated. The swap is an involution in `mirror` and `unmirror`, so both partners agree.
- **Result:** zero close-kin couples on both worlds.
- **Cost:** the scan visits every couple, so the build becomes linear in population: 1.4 s at ×1, 70 s at ×64 (2.6 s and 155 s before its check got cheaper). Tables and lookups are unaffected.
- **The check** (`related_couple`) is exact with two shortcuts. A man is a woman's father only if at least 16 years older. Children of different mothers, each conceived in her mother's first union before it ended, have different fathers, because each man has exactly one first couple. So most couples need only both mothers. A unit test compares it with the full check on couples and on close kin of every kind.
- The count of close-kin couples per world is about constant across scales (about 15), so the scan looks hard for few cases.
- **Its output is tiny:** 11 entries at ×1, 23 at ×4. So the recommended form is to run the scan once per (pack, seed) and cache or ship the entries: every later build is 0.3 s at any scale, and still exact.
- **Alternatives:** per-line lazy scans; a check at lookup time (two `related_couple` calls, about 2 µs per union instead of 0.45 µs); or measured debt.
- **Swaps across layers** cover what layer repairs can't (a layer of one, or groups whose every arrangement is related): the couple exchanges its man with the nearest couple for which neither new couple is related, an involution on women's positions composed with `mirror`.
- **Robustness:** the exhaustive tests pass on 2,300 tiny-world seeds (1,099 with same-sex unions). The extra seeds found and fixed: deaths in the entry year drawn on or before the birth or arrival (about half of infants dying in their birth year died "before being born"); same-sex lines cut into segments too small to regroup; and odd layers' trailing members.

## 7. What is left, in order

1. **Speed** is no longer the gap (Beatty trees, §3.1, and the speed pass in §6). What is left of the tail is men with three or more unions, about 7 µs at p99 for their children, where each union pays a source chain. Next levers for memory, not needed now: pack shares into `u32` and share delay trees across lines of similar years.
2. **Realism:**
   - **The age gap (fixed, see the end of this item)** (plain mirror: mean 2.1–2.4, sd 3.6–5.0, 30–36% wife older by cohort, against 2.3, 4.0 and 22%). Measured 2026-10-02 (`pure_report` with layer arguments; two-piece normal layer weights added as `Coupling::beta_sd_neg`):

     | Layers (β range, weights) | sd of gap | Wife older |
     |---|---|---|
     | all β = 0 (comonotone) | 0.9–1.9 | 0% |
     | [−2, 2] | 1.6–2.1 | 4–10% |
     | [−8, 8], symmetric (default) | 3.6–5.0 | 30–36% |
     | [−14, 4] or [−16, 2], or skewed weights | 3.4–5.7 | 30–43% |

     - **The mean gap is fixed by the marginals.** Every coupling keeps both sides' seat ages, so the mean is the men's mean age minus the women's, whatever the layers. In quantile terms, a coupling is a copula, and E[y − x] = 0. Layers shape only the spread and the skew.
     - **Why the mirrored softmax can't give the real shape.** A layer's share at quantile `x` is `∝ w_l e^{β_l x}`, and within it a woman at `x` pairs with a man at `h_β(x) = 1 − ln(1 + e^β − e^{βx})/β`. That curve is above the diagonal for β < 0 and below for β > 0. Large |β| both selects the age extremes and gives them the widest gaps: at β = −16, a woman at the 10th percentile pairs with a man at the 98.6th. So spread comes only from extremes pairing with extremes, in both directions, and skewed weights add variance to both tails.
     - **The real shape** is a near-comonotone core (most couples a year or three apart) and a right tail of much older husbands.
     - **The fix, as a construction:** two classes of positions, a **core** matched by identity (rank within layer, so comonotone up to layer noise) and a **tail** matched by the mirror. Counts stay equal on both sides if the class pattern is palindromic: `class(q) = class(K − 1 − q)`. Then the men's core layer at `q` is the women's at `q`, and the men's tail layer at `q` is the women's at `K − 1 − q`; both are bijections of each layer, by the rotation and mirror lemmas. The core's spread comes from its own layer noise; the tail's share and β set the tail.
     - **Built (2026-10-02),** in a form that leaves every (layer, rank) structure alone: classes come before layers, layers are numbered core first, and only the position ↔ (layer, rank) maps and the mirror read classes. Defaults: core share 0.3 over 3 layers, tail of 6 layers over β ∈ [−8, 8]. Pure against today's world, mean/sd/% wife older: 1880 2.6/3.9/23% against 2.6/4.0/22%; 1920 2.2/2.9/20% against 2.3/3.8/22%; 1950 2.0/3.2/22% against 2.3/4.2/23%; 1980 2.3/4.3/23% against 2.4/4.5/27%. Hashing the class split per lookup cost about 10% at p50; storing it per line (`lattice::BeattySplit`) removed that. With a core share of 0 it is the old mirror exactly (identical checksum).
   - Re-partnering delays should depend on each person's age, not a group's typical age.
   - **Re-partnering rates** (re-partnered within 5/10 years of a separation: 37/56% against NSFG 54/75%). Measured 2026-10-02 on the prototype:
     - 14.2% of women's re-partnering seats are void: 5.4% because she dies first (outside the measure, which follows the living), 8.8% because the man assigned dies first. Deaths stay local by design, so those seats never form: about 5 points of the 10-year gap.
     - Reading the pack's hazard at duration `r − 1` instead of `r` (the first year after the separation's year as duration 0) made it worse (31/52%): the hazard is lower in the first year.
     - The rest is level: men read the women's hazard (the ledger has men × 1.2) and each group uses a typical age. A calibration factor on the delay shares, or per-age delay trees, would close it.
3. **Ever partnered.** Natives match the pack exactly (women alive at 50 who partnered, 1920/1950/1980: 92.8/95.7/90.3% against the schedule's 91.0/94.9/90.2%). The cohort shortfall (81–87%) is arrivals: they come single and partner at native hazards for their age, so the share their peers partnered before that age never partners.
   - **Tried (2026-10-02), off by default** (`Coupling::arrivals_partnered`): in their arrival year, arrivals get a first-union hazard equal to the share of native peers partnered by then. Ever partnered rises to 92–96% by cohort. But the population ever rises from 14.3M to 23.0M, since women partnered at 30+ on arrival get full lifetime plans with no births abroad. The CFR rises (1950: 2.13 → 2.35), and the age gap widens (sd 5.8–6.9, mean 1.3–1.9), because older arrivals pair at their line quantile with mostly younger natives.
   - So **arriving couples need their own line kind:** women's arrival seats authoritative and men queued, with plans conditioned on age at arrival (remaining births only) and children born abroad as arrival cohorts listing their parents.
   - **Design, ready to build** (each piece reuses a mechanism that already exists):
     1. **Partition.** An arrival cohort (birth year `b`, arrival year `E`, sex) splits its ranks first into "arrived partnered", ranks `[0, n_A)` with `n_A = ⌊N·P(A) + u⌋` (`P(A)`: native peers' share partnered by age `A = E − b`, or a pack rate), then the usual first-union bounds over the rest, from `A + 1`. `partition` returns the line kind with the year.
     2. **Arrival lines** (`Kind::Arrive`, one per year `E`). Runs are by birth year, each a single cohort `(b, E)`. Women's seats are authoritative. Men's partnered shares are scaled each year so their expected seats equal the women's exact seats (the first-union balance, with arrivals' own sex ratio), and men queue across arrival years as on first-union lines. Layers, core/tail, `mirror` and kin repair apply unchanged. `LineId` needs a second kind bit.
     3. **Plans with a virtual union year.** A couple whose woman arrives at age `A` gets the plan leaf of a first-union line, read with offsets from a virtual union year `v = E − (A − ā)`, where `ā` is the typical first-union age of her cohort. Births at `v + offset ≤ E` happened abroad and are not in the world. Births after `E` are in-world, so each line adds a block to the year's child rows, the same way first-union lines do, and `origin` decodes it like the non-union segment. A woman arriving at 38 then has the tail of a plan, not a whole one: this fixes both the 60% population excess and the CFR.
        - **Sizing the child rows.** The virtual year is constant within a run, since a run is one arrival age. Indexing rows like first-union lines, by (line, run, layer, group), would need about 260 MB (≈ 30 arrival lines × 50 runs × 9 layers × 16 groups per child year). Instead, rank a year's arrival-line children by (line, run, layer, rank): store per (line, child year, run) one cumulative count (about 1 MB in all), and count the layers before `l` inside the run at lookup (up to 8 Beatty counts, about 0.2 µs). `origin` searches the run cumulative, then the layers.
     4. **Children born abroad** (later): births at `v + offset ≤ E` to women arriving with their partner are the arrival cohorts' children. They become a segment of the child arrival cohorts of year `E` (ranked by mother's line position and birth order, like child rows), so `mother`/`father` resolve, and `children` lists them. Until then, child arrivals stay parentless, as now.
     5. **Re-partnering and deaths.** Arrival couples dissolve and feed re-partnering lines like first-union couples (`push_re` loops over kinds). Deaths condition on the arrival year and in-world births, as for first unions.
     6. **Tests to extend:** mutual partners, mother/father duality (including virtual-plan births), the close-kin scan over arrival lines, and "partners after arriving" (an arrival couple's union start is in the arrival year, before which neither was in the world: it should be reported as "arrived together", with the true start date abroad kept as a fact of the couple).
4. **Features, all within the construction** (non-union births and single arrivals are done):
   - arriving couples: their own line kind per arrival year (women's arrival cohorts authoritative, men queue), with a post-arrival plan; children born abroad as arrival cohorts listing them;
   - ~~same-sex unions~~ **built 2026-10-02:** one line per sex and year, partners paired at random within 8 age segments (permutation slots `i ↔ i ^ 1`), facts keyed by the couple's lower position; same-sex ranks first in each cohort, so the opposite-sex partition is unchanged; kin repair by groups of four slots, and a segment's trailing pair with the last group as six slots, re-paired by the first of the 15 perfect matchings with no related pair. Odd two-sex layers got the same treatment: a trailing triple repaired by a transposition (a sibling couple in the third member of a 3-member layer showed it). 1.40% of couples together in mid-2019 (ACS about 1.5%). No re-partnering after them and no plan children, as in the ledger;
   - heritage and areas: segment lines with an overflow tier for thin markets (two-sex note).
5. **Wiring.** A trait both worlds implement, so households, residence and names can run on the pure world and be compared directly.

## 8. The decision, and a migration plan

**What the prototype shows** (updated 2026-10-02, morning). Kinship can be exact both ways, faster than today at p50 and p99 (2–14× at p99), and independent of population size: 22–26 MB of tables built in 0.3 s at any scale up to 873M people. That covers first unions, re-partnering, lifetime fertility, non-union births, deaths, single arrivals and same-sex unions, with zero close-kin couples (a year-ordered scan whose output is a few dozen entries per world). The age gap matches (core/tail coupling). The exhaustive tests pass on 2,300 seeds.

**What it does not cover yet:**
- arriving couples and children born abroad (design in §7; the one realism gap left, ever partnered 81–87% by cohort);
- heritage, and areas (partner geography);
- the layers built on today's `World`: households, residence and names.

**And it changes the worlds.** Every person and relation differs from today's ledger, so realism needs recalibration: immigrants' partnering and the parity tilt (CFR about 0.1–0.2 high in late cohorts). Both are pack knobs or the arrival lines above.

**Options for the founder:**
1. **Adopt the pure architecture (recommended).**
   - Build `internot_society::pure` out to the current model's features, in this order: arriving couples; heritage segments; area segments with an overflow tier (same-sex lines are done).
   - Move households, residence and names behind a trait both worlds implement, compare them side by side with the realism reports, then switch and delete the ledger.
   - In area mode, residence rosters already need no history (snapshots), and the pure world can supply the same area membership from its partitions.
   - Roughly a week of work, with the calibration passes; memory and build stop being constraints at any scale.
2. **Keep the ledger, compact it.** The stored counts are about 30–60 MB of information inside 1.3 GB of layout. A careful re-layout keeps today's worlds bit-identical but still grows with granularity (`us-areas` would stay in the hundreds of MB).
3. **Both:** compact now for `us-areas`, adopt the pure world later.

**Risks of option 1:**
- **Tables grow with granularity, not population** (measured 2026-10-02). Of 37.3 MB: per-cohort first-union bounds 16.2 MB (16,338 cohorts, 15,987 of them arrival cohorts, one per birth year × arrival year), lines 10.4 MB, child rows 4.6 MB, cohort structs 3.4 MB. Heritage × areas (5 × 62) would multiply cohorts about 300-fold: gigabytes, as the ledger's cells do. Before areas, arrivals need restructuring:
  - pool arrivals by (arrival year, area, group), with birth year a dimension inside one cohort, not a cohort each;
  - store bounds trimmed (**done 2026-10-02**: without leading zeros and trailing repeats, 37.3 → 22.3 MB), or as `u16` deltas on top;
  - or compute a small cohort's bounds on demand (with first-union hazards tabled, a cohort's bounds are one pass over its ages).
  Natives scale as (birth year × area × group): about 109k cohorts at `us-areas`, about 100 MB as stored now.
  - **Proposed fix, a reframing (2026-10-02, not built): cohort bounds as differences of per-birth-year prefix sums.** A cohort's expected cumulative first unions by age `a` is `Σ_{a' ≤ a} l(a')·S(a')·h(a')` over its single years, where `l` is survival, `S` the never-partnered survival and `h` the hazard.
    - Every cohort of birth year `b` (the natives, and each arrival cohort of any arrival year) shares the hazards `h_b(a)`: they are tabled by (sex, birth year, age) already.
    - Arrivals come single at their arrival age `A`, so their never-partnered survival from `A` is `S_b(a)/S_b(A)`, and their survival `l_b(a)/l_b(A)`.
    - Hence with one prefix table per birth year, `P_b(a) = Σ_{a' ≤ a} l_b(a')·S_b(a')·h_b(a')`, an arrival cohort's cumulative incidence is `C(a) = (P_b(a) − P_b(A)) / (l_b(A)·S_b(A))`, and its bounds are `⌊N·C(a) + u⌋`: O(1) per year, no per-cohort storage.
    - **Men:** the balance factor `s(y)` scales every active cohort's hazard in year `y`, so along a birth year's diagonal every cohort sees the same `min(s(y)·h, 1)`. The same factorization holds with per-birth-year men's prefix tables built in the year loop.
    - **Partition:** a rank's union year is the first age where the bound exceeds it, a binary search over ages with O(1) evaluations.
    - **Runs:** a line's birth-year run still walks its cohorts (natives, then arrivals by arrival year), now O(1) each.
    - **State:** per (birth year, sex) prefix tables, about 0.5 MB, times heritage groups if their hazards differ; areas don't change hazards. Per-cohort state shrinks to its size and rounding key.
    - **Exactness is unchanged** (bounds are still a systematic rounding of a cohort's own expected incidence). The algebra is machine-checked (`cohort_from_prefix` in the Lean file).
    - **Tried 2026-10-02 for arrival cohorts, and reverted.** It reproduced every stored bound exactly: the answer checksum was identical, and the exhaustive tests passed. But at today's granularity, trimming had already shrunk arrival bounds, so tables went 22.3 → 22.8 MB. Union p99 rose about 30%, because a birth year's run walks up to about 70 arrival cohorts, each now a closed form instead of a read. It pays once heritage × areas multiply cohorts, together with a per-run index over cohorts (so a lookup doesn't walk them).
- Thin area markets: the queue offset is bounded by active blocks, so a union year shifts by at most about a year, but that needs measuring per area.
- ~~The coupling's realism (age-gap skew)~~: solved by the core/tail coupling (§7).
- Kin repair is built (§6) as a build-time scan that grows with population (1.5 s at ×1, about a minute at ×64), whose output is a few dozen entries. Recommended: cache those entries per (pack, seed). Alternatives: scan every build, lazy per-line scans, a lookup-time check, or measured debt.

