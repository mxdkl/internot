# Residence as a direct function of (id, t): what is possible

**Date:** 2026-10-01
**Why:** the founder, on P1 (`research/2026-10-01-residence-algorithms.md`): "you said to find where Maria lives in 2025 you have to replay her whole moving history. That does not sound very f(time, id) to me." The task is to find a way to answer "where does x live at t" and "who lives in N at t" directly, and to put the math in `procedural_core`.

## 1. Forward without replay: hierarchical regeneration

**Construction.**
- A location is a path in a fixed hierarchy: region ⊃ county ⊃ tract (more levels are allowed).
- A household's relocations each have a level: a local move redraws the tract, a county move redraws county and tract, a long move redraws region, county and tract. Levels above the move's level are kept.
- Moves of each level form a point process on the household, with a deterministic intensity by age, era and duration (a time-rescaled Poisson process). Life-event triggers are deterministic times from the kin lookups.
- Let `r_k(h, t)` be the last event before t of any move at level ≤ k. Then the level-k coordinate at t is the draw made at `r_k`, under the level-(k−1) node in force at t.
  - That node is unchanged since `r_k`, because `r_{k−1} ≤ r_k`.
- **Cost:** `r_k` is one `PoissonTree::last_before` (O(log T)), and each draw is one keyed draw. Forward is O(L · log T) for L levels, independent of how many moves were made: `regen_state` (the regeneration lemma, already in `procedural_core::stream`) applied per level.
- **Nested regeneration:** a move at level j regenerates every level ≥ j. So `r_k = max_{j ≤ k} last_before(M_j, t)` for per-level streams `M_j`.

**Realism it gives:**
- move timing by age, era and life events;
- distance decay as level shares (most moves local, few long);
- persistence (no move, same address);
- growth through era weights in the draws.

What it can't give is a destination relative to *another* household's current place (§3).

## 2. Inverse: static draws make rosters cheap

Replace each free draw with a **bijection** over a dense, static domain, so every slot has exactly one possible owner.
- Time is bucketed (for example yearly), and draws at a level within bucket b come from a bijection `Ψ_{k,b}` from a dense index to (child node, slot).
- `roster(N, t)`:
  1. for each bucket b that can still be in force, take each slot of N under `Ψ_{k,b}` and its single preimage;
  2. keep the owner if its last level-k regeneration is in bucket b and its path matches.
- **Cost:** |N| × (buckets in force) × levels checks, each check O(L · log T). No history walk and no memo.
- **The dilution to watch:** with levels nested, a level's domain must cover the parent's slots across buckets. Done naively, the dilution multiplies across levels (buckets^L). Keeping it additive needs care: either per-level domains keyed to the household's static index plus a bounded move ordinal, or a two-level design (region by static flows, tract within a closed unit). This must be measured in a prototype.

## 3. The obstruction: locality relative to someone else's current place

The binding realism target is that new households form near the parents' current address (at 26, 58% within 10 miles of where they grew up). A move channel whose destination depends on another household's location at the move time can't have a static domain.
- **The catchment argument** (`research/2026-10-01-residence-algorithms.md` §3): an exact roster of N must evaluate every household that could have entered N through the channel.
  - Those households are the children of households located near N at the time.
  - Finding them is a roster query at an earlier time, so the query chains back through history.
- The chain is finite only inside a **closed unit** (an area whose history is enumerated), or if the place is part of a **counted structure** that indexes people statically, as the kinship ledger does for unions.
- So no exact design with a direct forward and cheap inverse gives "near the parents" unless the population ledger counts where people are born.

## 4. The three ways out

**A. Closed units, cached** (P1 refined by §1–2):
- **Forward:** O(L · log T), plus the parents' location at formation. That is one forward of the parent household; it recurses only for levels the line hasn't regenerated since, about generations since the line's last long move.
- **Rosters:** a county's (or basin's) history is enumerated once, then cached; after that, rosters are cheap. Estimated at seconds for a 20k-household basin (full-scale size), much less at prototype scale.
- **What keeps a unit closed:** every channel that crosses the unit's boundary must be static (seeds, itinerary bijections). Inside the unit, channels may depend on current places and on kin.
- **Distance decay inside the unit:** an extra level (clusters of tracts), so most local moves stay in the cluster. Distance is then hierarchical (ultrametric), the usual nested-gravity approximation.
- **Realism given up:**
  - the first long move of someone who inherited their place from their parents can't depend on that place (it is tiered around the lineage region);
  - partner geography stays unrealistic (§5).

**B. Place counted in the ledger** (the multiregional cohort-component ledger, spec §8.1, in a new form):
- A child's birthplace is the place of the mother's union cell, counted like births today.
- Markets run by place, so couples are local. Place-level flows are counted per block and year.
- Both directions are direct at the counted level; finer levels use §1–2 within it.
- **Cost:** the ledger's cells multiply with the number of counted places. Unknown at county granularity (could approach one cell per person: a simulation's cost); feasible at state granularity (several GB estimated). This needs its own research and a prototype.

**C. Static places only:** direct both ways, but no "near the parents". Fails the 58% / 60% targets badly.

**Ideas tried and why they fail** (so they aren't re-derived):
- **Slot hierarchies** (each level's slot drawn by a per-bucket bijection of the parent level's slot). Inverting needs, per level, every bucket in which the slot could have been drawn, so the cost multiplies: buckets^L. Encoding the bucket in the slot makes the inversion O(L), but then each bucket's slot space must cover every earlier slot, so the dilution comes back as buckets².
- **Children's places as a bijection of (parent's slot, child ordinal):** the slot space grows by the number of children per parent each generation (J^g).
- **A lineage coordinate that drifts** (a branching random walk on the family tree): forward is a walk up the maternal line, but the inverse is the descendant subtree of everyone who ever lived nearby, which is a closed unit again.
- **Child ids ordered by the mother's place within ledger blocks:** this needs every mother's place at build time, a simulation pass.
- **Interchange (stirring) processes:** local both ways, but movers are whoever occupies a site, so move rates can't depend on age or life events.

**Recommendation: A.** It is exact, its forward is direct, and it meets the proximity targets. Its costs are a cached first touch per unit and coarse partner geography. B at state level could later fix partner geography without changing A's within-state machinery.

## 5. Partner geography

The ledger pairs partners within lineage regions, which don't follow residence. Only B fixes this at its root. Under A or C, couples' previous homes are as far apart as the lineage region allows; measured as the distance between partners before the union.

## 6. For `procedural_core` (founder request)

**Landed (2026-10-01).** Each has property tests and golden values. The society crate now calls them, and the switch was proved bit-identical by the world fingerprints, including names.
- **Nested regeneration:** `stream::nested_regen` and `stream::nested_regen_state`. Level k's state at t is the draw at the last move of any level ≤ k, under the level-(k−1) state. Each costs one `last_before` per level and is tested against a full replay.
- **IPF:**
  - dense: `fit::ipf`, with `IpfStop`;
  - grouped by the kernel's classes: `fit::GroupedIpf`, `fit::Classes`;
  - in factor form with fixed row totals: `fit::rake_columns`, `fit::raked_weight`.
- **Sparse systematic rounding** of a structured matrix: `fit::GroupedIpf::class_cumulative` gives each row class's cumulative column weights; `partition::round_systematic_cumulative` rounds one row; `partition::SparseCounts` holds the result, with a column index.
- **Exponential tilting:** `fit::tilt_mean`.
- **Fixed-point cumulative tables:** `table::CumTable`.
- **Integer splits under caps:**
  - the keyed systematic sweep: `partition::sweep_capped`;
  - largest remainder, plain and capped: `partition::apportion_largest_remainder`, `apportion_largest_remainder_capped`;
  - `partition::trim_largest_first`.

**Round 2, the same day: all remaining generic math in `internot_society` and `internot_def`** (founder: "all math should be procedural code"). It is also bit-identical.
- `interp`: knots, steps, crossings.
- `life`: survival, the Siler model, cure hazards, conditional age draws.
- `curve`: named schedule shapes.
- `pmf`: weight-vector operations.
- `table`: coarse searches and the bucket index.
- `sample`: inversion exponential, linear pick, the lazy conditional draw.
- `partition`: pair groups, even parts, segments.
- `perm::least_cost_assignment`, `stream::Epochs` and `bits::ones`.

The full list of what moved and what stayed is in AGENTS.md (Phase 2).

**Still to add once a residence design is chosen:** bucketed bijective draws with inverse enumeration (`Ψ_{k,b}` and the roster filter, §2). Their shape depends on the choice between A, B and C.
