# Workplace rosters: "who works at E at t" in time proportional to the answer

**Date:** 2026-10-04. **Asked (founder):** "make searches like that fast while keeping memory low" (after measuring a full scan at 10 s on 16 threads at ×1). **Builds on:** `2026-10-04-areas-and-rosters.md`.

## 1. The problem, stated

Jobs are pairs `(x, j)` (person, job ordinal) produced by each person's career walk; the employer is `f(x, j) = (county, industry, size class, index)`, where the county is the county of `x`'s home when the job starts. The query is the preimage

`R(E, t) = { x : ∃ j, f(x, j) = E and job j of x is in force at t }`.

The goal is `O(|R| · polylog)` cost and memory that doesn't grow with the population, as for the monotone world's alive counts.

## 2. What answer-proportional preimages need

**Claim.** Enumerating `f⁻¹(E)` in time proportional to the answer needs a **counted index** (rank and select in closed form) of a stratum containing `f⁻¹(E)` whose size is within a constant factor of the answer, with `f` restricted to it an invertible map (monotone, affine or Feistel) onto `E`'s seats.

*Why:* without a counted stratum, the only way to find the preimages is to evaluate `f` forward on a superset that isn't indexed, which is a scan. With one, the preimages of `E`'s seat range are `f⁻¹(range)`, each checked once. This is the rank/select principle of succinct data structures. It is also the rule of hash-based procedural generation that **containment queries are answerable only along the generation hierarchy** (galaxy → system → planet): a world generated lineage-first answers "who are x's children" but not "who lives in z".

**What `E`'s stratum must contain.** Workers of `E` are people in `E`'s labour market (its commuting zone) at the jobs' starts, in its industry. Industry comes from the walk, so a static career field per person is needed to keep the stratum within a constant factor of the answer (one field holds most of a person's jobs; switchers go to a second system). **The commuting zone comes from residence, which today is a per-household walk (nested regeneration) with no index by place.** So every answer-proportional workplace roster needs a counted index of the people of a commuting zone at a time. Classmates, neighbours and friends need the same.

**Alternatives checked and rejected:**
- **Static seat systems over the nation** (the society spec's K systems, with acceptance on distance): a national coordinate lands in the right commuting zone with probability about 1/588 and the right industry about 1/100, so K or the candidate overhead would be around 10⁵.
- **Materialized indexes** (each person's zone history, or per-establishment job lists): 0.4–1.7 GB at ×1, growing with the population.
- **Key-ordered residence** (homes as keys along a Hilbert curve, moves as key steps): random-walk keys have no inverse; invertible key maps (affine permutations) lose locality; local window permutations can't follow the decade shifts of population without moving people who don't move.
- **Residence only, without kinship by place** (counted zone spells per household, layered on today's world): it can be made exact, but formation near the source household (couples and young adults settle near parents) makes the inverse recurse through generations. That costs in proportion to a zone's population, not the answer, and distances to kin stay as unrealistic as today (33–44% of adults with a parent within 30 miles, against 59.8%).

## 3. The design that meets the goal

**Kinship cells by commuting zone, with computed classes** (options A + C of the areas note, at commuting-zone grain):
1. **Cells are (commuting zone, group).** Each is a monotone world: births in the mother's zone, unions mostly within the zone (an open market per zone, plus a national market for the rest), deaths as now. So "the people born in zone Z" is a counted index.
2. **Movers between zones** are counted flows per (origin, destination region, year), assigned to people by rank couplings. People living in Z at `t` are then the natives who haven't left plus the movers in: both counted, with exact counts.
3. **Computed classes** (option C) keep memory low: per-cell state is a few numbers per cohort (class sizes from the union-age law, eligible-mother prefixes from shared death thresholds, class permutations from stored golden pairs), about 64 B per cell cohort. The US's 588 zones × 5 groups × ~250 cohorts is about 47 MB, built lazily per zone touched (plus an eager census of sizes and flows, ~0.1 s).
4. **Workplaces on top:** a static career field per person (an exact interleave per cohort and cell); per (zone, field), the people's job slots `(rank, ordinal)` map onto the zone's establishments' seats by an affine permutation. `R(E, t)` is then the preimage of `E`'s seats, checked for "job in force at `t`", with about 10× more candidates than answers. At ×1, a 1,000+ establishment (about 22 sampled workers) costs about 2–3 ms; at the real US's scale, about 0.1 s for 1,000 workers. It is flat in population.
5. **The same index serves** classmates (per zone, school and grade), neighbours (residents of a zone, then their tracts) and friends' foci.

**Costs and risks:**
- **Kinship lookups:** the areas note estimated computed classes at 1.5–3× slower mother and death lookups (unmeasured). This is the risky part and must be prototyped first (founder rule 2).
- **Realism improves:** couples meet in their zone and children are born there, which fixes distances to kin and the share living outside their birth state.
- **Effort:** a rebuild of the cell layer (`mono.rs` builds per cell), migration, then residence and work rewired to the zone index. Checksums change, since the world changes.

## 4. Prototype result (2026-10-04): computed classes fail per lookup, work as a memo

`examples/proto_computed_elig.rs` recomputes a cell year's eligible mothers by (kind, age), the 62 caps of the birth split a mother lookup needs, from the class sizes and one death threshold per mother cohort. It matches the stored rows exactly on 2,000 cell years. It costs **18 µs, against 75 ns for a whole mother lookup today (×240)**: the 31 mother cohorts' thresholds dominate.

**Consequence:** option C as "compute per lookup" is out for births and mothers. What works is **memoized cells**, applied to the cell layer:
- an eager **census** of a few numbers per cell cohort (sizes, open pools, eligible totals for the births split);
- per (cell, year) birth structures and per (cell, cohort) class structures, built on first touch (tens of µs) and kept;
- warm lookups run at today's speed plus one indirection; memory grows with what is touched (about 2.5 KB per cell year: a zone's five cells over a century are about 1.3 MB).

Today's per-cell structures cost about 2.4 MB per cell whatever its population, since they are per cohort and per year. So 2,940 zone × group cells built eagerly would be about 7 GB, and memoization is required, not optional.

**Remaining hard problems:**
1. **The partner market across zones.** About half of couples grew up in different zones. Today's open market is one space whose categories are (husband cohort × cell), which is O(cells) per wife cohort: about 0.6 GB for 2,940 cells. It must become partnering where people live (natives and movers in a zone's market), which makes movers part of the union spaces.
2. **Tiny cells.** At ×1, a zone × group cell averages about 20 people per cohort, and minority cells near none. The births' caps and the union room bind constantly. This calls for coarser cells (zones without the group split, or areas × groups with zones inside) or a minimum scale.
3. **The census build** computes eligibility for every cell year (about 588k × 18 µs, ~0.7 s on 16 threads) and its memory is about 20–35 MB.

## 5. Proposed order (revised after the prototype)

1. ~~Prototype C~~ (done, §4: per-lookup recomputation is ×240 for mothers).
2. **Memoized cells on today's five group cells**, with no change to the world (the checksums must not move): an eager census plus per-cell-year and per-cell-cohort structures built on first touch. Measure warm and cold lookups (`perf/society_ab.py`), memory per touched cell year, and the census's build time and memory.
3. **Design the zone partner market** (partnering where people live: each zone's market holds its natives and its movers in) and choose the cell grain (zone × group, zone alone with groups inside, or area × group with zones inside). Math note, Lean lemmas where needed, then founder review.
4. Zone cells with movers; residence on the zone index; workplaces (career fields, seats, `R(E, t)`); then schools and neighbours.
