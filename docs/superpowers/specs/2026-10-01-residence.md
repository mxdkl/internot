# L4 residence, design A: closed basins

**Date:** 2026-10-01. **Status:** prototype built and exact; realism blocked on the ledger (§7–§8). Sections 2–6 describe the first design; §7 records what changed (areas as closed units, five levels, anchored gravity).

**Founder decision (2026-10-01):** design A.
- Forward lookups by nested regeneration.
- Rosters exact, by enumerating a basin's history once and caching it.
- Moves across basin boundaries are static.

**Research:**
- `research/2026-10-01-residence-data-and-targets.md` (data and targets)
- `research/2026-10-01-residence-algorithms.md` (the obstruction)
- `research/2026-10-01-residence-closed-form.md` (forward without replay; why "near the parents" needs a closed unit)
- `research/2026-10-01-residence-moves-and-basins.md` (move hazards, distances, commuting zones)

## 1. What it must answer

- `address(h, t)`: where household `h` lives at `t`. A person's address is their household's.
- `roster(n, t)`: every household living in place `n` (a tract, or any node above it) at `t`, exactly the households whose address is in `n`.
- **Realism targets** (data note §2):
  - mover rate by era (20% a year in 1948–70, 7.8% in 2023) and by age;
  - move distances (about 76% of domestic moves under 50 miles);
  - at 26, 30% are in the same tract as at 16, and 58% within 10 miles;
  - 59.8% of adults 25+ have their nearest parent within 30 miles;
  - places growing and shrinking over the decades (county populations 1840–2020).

## 2. The problem as math

### 2.1 Places

- A fixed rooted tree of places with levels 0 to L−1: **basin** (a commuting zone, large ones split; §6), **county**, **cluster** (about 15 neighbouring tracts) and **tract**. So L = 4.
- Each tract has an **attractiveness** w(n, y) by year, which sums up the tree. It comes from population data and is calibrated (§2.8).
- Above the basins, each basin has a lineage region, a state and coordinates (for long-move tiers and for distances).

### 2.2 Units: who has an address

A household lives at its **anchor unit**'s position. Units are of two kinds:
- a **union**, from its start to its end;
- a **spell** (x, s), the s-th stretch of x's single independent life: s = 0 from first independence, s > 0 from the end of x's s-th union, until x's next union or death.

Every unit has a **position process** over its whole span, even while nobody lives there (the unit is then *latent*). The anchor of each L3 household:

| Household | Anchor | Address at t |
|---|---|---|
| `Union{a, b, start}` | the union | its position at t |
| `Solo{x, s}` | spell (x, s) | its position at t |
| `Roommates{…}` | the spell of the group's first member (the lease holder; eligibility is fixed at the epoch start) | that spell's position at the epoch start: the group doesn't move during its epoch |

- Dependents and kin guests live in their household, whose address follows from the table.
- Each unit has a **driver**: the person for a spell, the decider for a union (the woman, or the lower id for a same-sex couple, as L3's `unit()`).

### 2.3 Sources: where new units start

Each unit u starts at time b_u from a **source** σ(u), or from a **seed**:
- **Spell (c, 0)** (leaving home): the unit of the person at the end of c's dependent chain just before independence: that person's union, else their spell. Kin hosting is ignored here; the source is the parent's own unit even if they lived with kin.
- **Union:** the unit of the source partner just before the start. The source partner is a keyed coin on the union key, with a pack share by sex. If that partner was still a dependent, the source is their parents' unit.
- **Spell (x, s > 0):** the union that just ended. One partner is the **keeper** and stays in the dwelling; the other forms a new place nearby. The keeper is a keyed coin with pack shares (a widowed survivor always keeps).
- **Seeds:** founders' units in 1840 and immigrants' units at arrival, placed by a static bijection (§2.6).

σ(u) always started strictly before u: b_σ < b_u. Sources therefore form a forest by time, which the proof in §2.7 uses.

### 2.4 Position: nested regeneration (no replay)

pos(u, t) = (basin, county, cluster, tract) for t in [b_u, e_u).
- **Level 0, long moves:** the driver's long moves (§2.5) that fall inside the unit's span.
- **Levels 1–3, local moves:** per-unit Poisson streams M_{u,k}, `PoissonTree`s keyed by u. The intensity is λ(age, era, frailty) × share_k (age profile Rogers–Castro, era scale, a mover–stayer frailty; research pending). The marking theorem makes the per-level streams independent Poisson processes.
- **A move at level k redraws levels ≥ k.** Level k's state at t is the draw at its last regeneration, under the parent level's state in force at t. This is `stream::nested_regen_state`: one `last_before` per level, O(L · log T).
- **Formation (the initial state):** take the source's position P = pos(σ(u), b_u) and a keyed formation level k_f (channel-specific pack shares):
  - levels < k_f are copied from P;
  - levels ≥ k_f are drawn under the copied parent by attractiveness;
  - k_f = L means the same dwelling (a keeper).
  - Formation never changes the basin (k_f ≥ 1). Leaving the basin is always a long move.
- **Cost:**
  - one long-move scan (§2.5);
  - three `last_before`s;
  - for levels not regenerated since b_u, the source's position, recursively up the source forest until each level has a regeneration or a seed.
  - Local moves are frequent, so the tract and cluster stop within a unit or two. The basin can go back several units in a family that never moves far. That walk is memoized per process (P8), like the surname walk.

### 2.5 Long moves: a counted flow (P3), exact both ways

The destination of a move into another basin can't depend on the mover's dynamic origin (the obstruction). It must come from a dense static index. The construction:
- **Counted selection:**
  - c[β][y] = keyed round(λ_long(age, y) · |β|) long moves in year y for birth block β, with the age being y − birth year. Keyed systematic apportionment keeps small blocks unbiased.
  - Person x of block β is a mover in year y iff π_{β,y}(rank x) < c[β][y], with `CompactPerm` π keyed by (β, y). The day comes from a keyed uniform.
  - A selection counts only if x is then the driver of a unit (not a dependent, not a trailing partner). Otherwise it is wasted, a measured dilution.
- **Destination:**
  - Year y's movers of lineage region r form a dense domain D_{r,y} (blocks' counts concatenated). It is apportioned over basins by a **tiered kernel** around r: level shares × attractiveness at year y (gravity and nested-logit forms; research pending).
  - A keyed permutation of D_{r,y} assigns positions to destination segments (`partition::locate_in_segments`).
- **Inverse:**
  1. A basin's segment in D_{r,y} gives its mover indices.
  2. The block segments give (β, k).
  3. π_{β,y}^{-1}(k) gives the person.
  4. Cost: the number of arrivals, with no dilution beyond wasted selections.
- **Forward:** scan the driver's years in the unit's span (one perm evaluation each, about 25 ns; ≤ 100 per life).
- **Heterogeneity** (repeat movers): blocks split statically into frailty classes with rate multipliers, if the research supports it.
- **Debt:** the destination depends on the lineage region and the year, not on the current basin. That gives some return-like moves to the home region, and it argues for finer lineage regions (states) later.

### 2.6 Seeds: also a counted flow

- **Founders** are block ranges, and **immigrants** are arrival cohorts, which are contiguous ranges in the ledger.
- For each (region, entry year), entrants map by a keyed permutation onto basin segments apportioned by attractiveness × port-of-entry weights.
- Only the driver of each entering unit uses its slot.
- The inverse has the same shape as §2.5.

### 2.7 The closure: exact rosters

For basin B:
- **Entry(B)** is the set of units seeded in B, plus the units driven by movers whose long-move destination is B. Both are static inverses.
- **Succ(v)** = {u : σ(u) = v}: the children leaving home from v, the post-union spells of a union, and the unions sourced from v's people.
- **E(B)** is the least set containing Entry(B) and closed under: v ∈ E(B), u ∈ Succ(v), basin(v, b_u) = B ⟹ u ∈ E(B).

**Claim:** E(B) = {u : basin(u, t) = B for some t in u's span}.

**Proof** by well-founded induction on b_u.
- (⊇) Suppose u is in B at t.
  - If u had a level-0 regeneration before t, the last one is a long move with destination B, so u ∈ Entry(B).
  - Otherwise u's basin is its initial one. That is either its seed basin (Entry), or the source's basin at b_u, since formation keeps the basin.
  - In the second case σ(u) is in B at b_u and started earlier, so σ(u) ∈ E(B) by induction, and u ∈ E(B) by closure.
- (⊆) Every member is verified when added. ∎

**Enumeration** is a worklist from Entry(B), with deduplication.
- **Candidates for Succ(v):** the children, grandchildren and minor siblings of v's people (for orphans in care), and the unions of those candidates. Each candidate is confirmed by computing its σ.
- The memo per basin stores each member's span and basin intervals.
- `roster(n ⊂ B, t)`:
  1. take the members in B at t;
  2. compute each one's tract at t;
  3. keep those occupied at t as their household's anchor (or as a lease holder at the epoch start);
  4. return the households.
- **Gate:** on the tiny world, at many dates, the roster equals brute force: every present person's household, mapped to an address and grouped by tract.

### 2.8 Calibration as math

- **Basin populations by year** come from seeds, long-move flows and natural increase. Their expectation follows a multiregional cohort-component projection with this model's rates.
- Fitting the destination attractiveness so that the projected basin populations track the county series is a sequential (year by year) multiplicative fit: a raking, the same family as `fit::rake_columns`.
- **Within basins,** the draws use tract attractiveness. Occupancy lags attractiveness through inertia, so the tract weights get a fixed-point correction measured on the world.

### 2.9 Distances

Distances are great-circle (haversine) between tract population centres, through `dmath`.

## 3. Core inventory (rule 1)

| Need | In `procedural_core` |
|---|---|
| Nested regeneration | `stream::nested_regen_state` |
| Local move streams with time-varying intensity | `stream::PoissonTree` (additive `mass`) |
| Counted selection | `partition::apportion_systematic` / `SystematicShares`, `perm::CompactPerm` |
| Segments and inverse | `partition::locate_in_segments`, `segment_offset`, `count::QuotaTree` |
| Weighted draws by attractiveness | `table::CumTable` |
| Age profiles | `curve::rogers_castro_labour` (a migration profile may be added), `interp` |
| Raking | `fit::rake_columns`, `fit::ipf` |

**New in core** (each with property tests and golden values):
- `geo`: haversine distance (dmath) and an equirectangular fast path for kernels.
- `fixpoint::closure`: the least fixed point of a monotone successor relation from entries, with a verify step and deduplication. It is generic, and later inverse queries (workplaces, schools) reuse it.
- `tree::TieredKernel`: an ultrametric (nested) destination kernel on a place tree. It gives level shares, then weights within the ring, excluding inner rings, as a pmf for apportionment and as a draw. Nested-logit or radiation forms, depending on the research.
- Possibly `perm::SegmentedPerm`: a keyed bijection from a dense domain onto labelled segments (forward and inverse). §2.5 and §2.6 both use it, as did L3's roommate frames.

## 4. Pack sections

- `places.ron` plus `data/places.bin`:
  - the tree (basins, counties, clusters, tracts);
  - tract centres and population by census year;
  - county series 1840–2020;
  - the basin split rules.
  - Distilled from `datasets/geo/` by a script, like the names data.
- `residence.ron`:
  - move intensities (age profile, era scale, frailty classes) and level shares;
  - formation level shares per channel;
  - the source-partner and keeper shares;
  - the long-move rate and tier shares;
  - seed (port-of-entry) weights.

## 5. Prototype plan and gates

1. **Core primitives** (§3, new) with tests.
2. **Synthetic geography** (a grid of basins with counties, clusters and tracts, with coordinates), on the tiny world:
   - **exactness:** the roster equals brute force at ≥ 15 dates across every basin;
   - forward equals a replayed-history reference.
3. **The prototype world on the synthetic geography (costs):**
   - forward pos p50 and p99, cold and memoized;
   - first-touch closure time and memory per basin, against basin size;
   - roster time when warm;
   - dilution of the counted flows.
4. **Real geography** (distillate):
   - realism report against §1's targets;
   - calibration of rates, level shares and formation shares;
   - basin populations over time.
5. **Gates:**
   - exactness on the tiny world (property);
   - realism targets within bands (realism);
   - pragmatic budgets (performance): forward cold p99 about 1 ms, warm roster about 10 ms for a tract, first touch ≤ 10 s for the largest basin at prototype scale. Large basins are split until that holds.

## 6. Decisions taken here (delegated) and debt

- **Basins are commuting zones,** and the largest are split until the first-touch budget holds. Moves within a basin keep their dependence on the current place, and most moves stay within a CZ. Pending the research on CZ data.
- **Long moves are counted per (block, year),** and their destinations are tiered around the lineage region. Debt: no dependence on the current origin.
  - The fix is finer lineage regions (states), which the ledger's sparse markets can now afford, or the multiregional ledger (B at state level, already listed).
- **No dwelling capacity:** tract occupancy is calibrated in expectation. Dwellings and street addresses come later, as a layer under the tract.
- **Sources use the parent's own unit, not a kin host's.** A child raised in a grandparent's home forms near the parent's latent place.
- **Roommate groups don't move during their 2-year epoch.**

## 7. Prototype findings (2026-10-01)

**Built and exact** (`internot_society::residence`, `tests/residence.rs`):
- On the tiny world, every tract's roster equals brute force over every person (2 seeds × 7 dates). County rosters equal the union of their tracts.
- Replayed timelines equal the direct nested-regeneration positions.

**Structure after the research note** (`research/2026-10-01-residence-moves-and-basins.md`):
- **Tree:** area ⊃ commuting zone ⊃ county (or about 1M parts) ⊃ cluster (≤ 16 tracts) ⊃ tract, from `worlds/us/data/places.bin` (62 areas, 588 zones, 83,848 tracts, weights by decade 1840–2100; census-region shares within 0.3 points).
- **Areas are the closed units.** A move inside an area is part of a unit's own timeline and needs no inversion, so moves between zones of an area draw by gravity around the unit's home zone (its first zone, or where its last long move landed). That anchor is a draw at a coarser regeneration, so the regeneration lemma holds. Only moves between areas are static counted flows.
- **Rates, frailty, level shares and formation channels** follow the note §6.

**Cost** (prototype world, 13.8M people ever born):
- `address_of`: cold p99 0.6 ms, warm p99 0.3 ms.
- A warm tract roster: p99 under 3 ms.
- First touch of an area: about 25 µs per unit with rayon over closure layers (6–12 s for 200k–400k units). Most of it is the kinship layer's union lookups.

**Realism: the binding finding.** Families scatter far too widely. The main cause is outside residence:

| Measure | Model | Target |
|---|---|---|
| natives living outside their birth state | 43–58% | 21–34% |
| adults 25+ with a parent under 30 mi | 16% | 59.8% |
| born 1990–94: under 100 mi at 26 against 16 | 59% | 80% |

- Tracing individuals shows the cause is **residence-blind groupings**, not moves:
  - the ledger pairs partners from a lineage-region market (half a continent), so at every union one partner relocates;
  - children follow custody and their parents' later partners;
  - roommate frames are region-wide.
- Turning roommates off changes little. Partners dominate: even the 1900 birth-state share is twice the census.
- Lowering move rates and making county or zone moves rare change little too.
- Design A can only place a couple where one partner lives, so the remedy has to be in the ledger: partners must be paired among people living near each other.
- An experiment with one lineage region per state (`worlds/us-states`) improves cross-area moves (3.6–4.1% a year against 5%) and far-from-parent shares (30% against 45%). But it pairs by birth state (stale for migrants), and the world build takes 46 s against 5 s.

## 8. Proposal: residence areas in the ledger (B at area level)

The founder's "B later" option, made concrete by what the prototype built:
- **Lineage regions become areas** (or states). A block is (birth year, area, heritage).
- **A person's ledger area at `t`** is static: their block's area, or the destination of their last counted long move before `t`. Long moves are already counted per (block, year) with static destinations (§2.5).
- **The ledger counts residents per (block, area, year)** from those flows, exactly. Markets pair people by **ledger area**, with an inter-area share for long-distance couples. So a couple lives in its market's area, and children are born into it (lineage area = where the parents live).
- **Long moves can then be origin-keyed:** the origin area is a static function, so the static flow can run per (origin area, year) with gravity destinations. That removes the remaining long-move debt (§6).
- **Roommate frames** key on the ledger area, so roommates are local.
- **Within an area, design A stays as built:** nested regeneration, formation near sources, closures per area, rosters.
- **Cost:** union cells multiply with areas; build time and memory must be measured and optimized (46 s at 53 regions before any work). Prototype on the tiny world with the exhaustive kinship suite first, as with R1c.
