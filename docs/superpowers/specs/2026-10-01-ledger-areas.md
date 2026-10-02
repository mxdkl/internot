# B at area level: residence areas in the ledger

**Date:** 2026-10-01. **Status:** design; first step (blocks and markets by area) next.

**Founder decision (2026-10-01):** B at area level (residence spec `2026-10-01-residence.md` §7–§8). The ledger pairs partners by where people live, at the level of residence areas (62 groups of whole commuting zones, about states).

**Why** (residence spec §7): with lineage-region markets (half a continent), every union relocates one partner, and families scatter: 43–58% of natives live outside their birth state (census 21–34%), and 16% of adults have a parent within 30 miles (59.8%). Residence alone can't fix it; the ledger must pair people who live near each other.

## 1. The problem as math

The ledger is a **multiregional cohort-component projection** (Rogers 1975; the spec's §8.1 plan, now at area level), made exact on integers.
- **State of a person at the ledger level:** (block, area, status), with status never partnered, partnered (in a union cell) or divorced.
- **Transitions:**
  - births into areas;
  - single moves between areas;
  - unions within an area's market (plus a small inter-area share);
  - couple moves;
  - dissolutions;
  - deaths (deaths stay per person, as now).
- **Every flow is an integer count**, apportioned with keyed systematic rounding (`partition::apportion_systematic`). Each person's area history is then a **static function of their id**: their block's area, plus the counted classes they fall in. This is the property that makes rosters invertible (design A's closures per area) and keeps the ledger and residence consistent.

## 2. What a person's area is

**A(x, t)**, a static function of the id:
1. **Upbringing area:** the block's area. A block is (birth year, **area of upbringing**, heritage). The area of upbringing is where the parents' household is when the child turns 18 (or the mother's area for a child born outside a union). The ledger knows it when it records the birth, because the parents' moves are counted classes decided when their cell is made (§3).
2. **Single migration before the first union:**
   - Each block is split by keyed systematic apportionment into **migration classes**: "stays", or (move year, destination area). Counts come from the single long-move rate by age and era and from the destination flows (gravity from the origin area, research note §6.4).
   - A member's class is their position under a keyed permutation of the block (the same device as cohort and class splits today).
   - From the move year until their first union, A(x, t) is the destination.
3. **In a union:**
   - The union's area: its market's area at formation.
   - A couple's later moves are **move classes on the union cell**: "stays", or (move year, destination), apportioned like R1c's dissolution classes and nested with them (sub-cells are (cell, dissolution class, move class)).
4. **After a separation or widowhood:** the union's area at its end. Later single moves are residence-level counted selections, as built today, and the ledger doesn't track them (debt: the divorced re-partner in the area where their union ended).

## 3. The ledger, year by year

**Pools:**
- Never partnered: per (block, migration class). A class is in area `dest` from its move year, else in the block's area.
- Divorced: per (block, source cell), in the cell's area at its end.

**Markets each year, per area:**
- The local market pairs the seekers in that area.
- An inter-area share pairs across areas, for couples who meet far apart (research note §5d: about 25% lived more than 50 km apart). The union's area is the woman's area, or a keyed choice.
- The national, open and same-sex markets keep their roles, now with area as an extra dimension where it matters.

**Union cells:**
- Cells gain an area (the union's area at formation).
- Their members come from the pools of that area: the blocks' migration-class ranges in that area, plus the divorced pools there.
- Coupling and kin repair work within (cell, class) as now.
- **Move classes** split each cell's couples by keyed apportionment from the couple long-move rate by age and era.

**Births:**
- Births per (sub-cell, year) come from plans, as now.
- The child's block is (birth year, area of upbringing, heritage), with the upbringing area being the sub-cell's area in the child's 18th year (or the year of the last birth for a cell whose last move is later: the area at 18 is what matters).
- Non-union births use the mother's single area at the child's 18th year (her migration class).

**Arrivals:** immigrants' blocks take their seed area, from today's seed flows, now inside the ledger.

## 4. Residence on top

Within an area, design A stays as built: nested regeneration below the area level, formation near sources, closures per area, and rosters.
- **Moves between areas** are exactly the ledger's classes: a single person's migration class, and a couple's move class. Later singles' moves (after a first union) are residence-level counted flows.
- **The closure's entries for an area** add the static inverses of migration classes and couple move classes into it (block and cell segments), besides seeds.
- **A child's upbringing area equals the residence area at 18** whenever the child lives with the parents' union. Custody to a divorced parent, kin hosting and the divorced's later moves are the measured mismatch.

## 5. Costs and risks (prototype first)

- **Blocks multiply by the number of areas:** 62 areas × 5 heritages = 310 groups per birth year, against 10. The `us-states` experiment (53 regions, no classes) built in 46 s against 5 s, so **profile and optimize the region scaling first**.
- **Cells multiply** by areas and move classes. Lookups slow down (more and colder cells); memory grows.
- **Kin repair in smaller pools:** small cells are harder to repair. Measure deferrals.
- **Order of work:**
  1. Regions = areas: blocks and markets by area, no classes. That is the `us-states` structure at area level. Profile and optimize the ledger build. Tiny world: the exhaustive kinship suite.
  2. Migration classes (singles).
  3. Couple move classes.
  4. Births by upbringing area.
  5. Residence on the ledger's classes; closures' entries.
  6. Realism report: birth-state share, distance to parents, partner distance. Cost report: build, memory, lookups, first touch.
- **Gates:**
  - the exhaustive kinship suite on the tiny world, including ledger closure per (area, …);
  - residence exactness (rosters equal brute force);
  - realism bands for the targets above;
  - pragmatic performance (build, memory, lookups) measured and reported.

## 6. Implementation plan (2026-10-01, after the founder chose to finish B)

The area mode is on when `places.ron` has `by_area: true` (lineage regions are areas). Off, every change below is a no-op and the worlds stay bit-identical (fingerprints).

**Stage 2b: births by area, cross-group parent lines.**
- **Cells carry an area:** the market's area (local markets: their region; national, open and same-sex: the woman's, or the left partner's, current area). Cell keys, permutation keys and plan keys include it.
- **The ledger records births per (year, mother block, child area).** A child block's mothers can then come from other groups.
- **World:**
  - Parent lines keep their age slots for in-group mothers and add an overflow list for mothers of other groups.
  - A mother block's birth line in a year is split by area: birth rows per area present in the block (union births by column, then non-union births of that area).
  - A child's position in its block's slot for the mother is its rank within that area's births.

**Stage 2c: migration classes as native cohorts.**
- A block's natives split at creation (keyed systematic apportionment) into cohorts: the stayers, and (destination area, move year) classes from the single long-move rate by age and era times destination shares. Cohorts are raw-id ranges; a natives-wide parent permutation keeps classes independent of mothers.
- **Pools are per cohort, as now.** A cohort's area in year t is its destination from its move year, else the block's area. A market of area A takes, from each block, the cohorts in A that year, and `settle` splits takings over them.
- **Non-union births** are planned per cohort, so their area follows the cohort.
- **L3:** a person in a migration class leaves home at their move year at the latest (moving away is leaving home), and moves only while still in their first single spell.
- **Divorced pools** stay in the area of the cell they ended.

**Stage 3: couple move classes.** Cells split by move class, (destination, year) or none, like dissolution classes. Births after the move are in the destination area, through the per-area birth rows.

**Stage 4: residence on the ledger's classes.**
- Long moves are: the migration class (in spell 0), the couple move class (driven by the union), and residence-level flows for later single spells.
- Closure entries are the static inverses of each.

**Gates for each stage:**
- the exhaustive kinship suite on an area-mode tiny world (four areas, migration boosted) and on today's tiny world;
- fingerprints unchanged with the area mode off;
- residence exactness.

## 7. Stage 2 at full scale: findings and fixes (2026-10-01)

Stages 2b and 2c are built (births by area with cross-group parent lines; migration classes as native cohorts). Running them at prototype scale exposed thin-pool defects. Most were already in the ledger and only became visible with 62 areas × 5 heritages, where blocks hold a few people a year.

**Measured first** (opposite-sex market totals over the build; `us-areas` before the fixes):

| | `us` | `us-areas` |
|---|---|---|
| couples solved by the markets | 6.96M | 3.15M |
| kept after the capacity caps (all markets) | 6.99M | 2.35M |
| deferred as isolated | 0.5% | 21% |
| people | 13.8M | 5.2M |

The markets' harmonic-mean totals were not the loss: their sum stayed at or above the women's wants. The losses were in the caps, deferral and the founders' first year, and they compounded over generations.

**Fixes** (both modes; the default worlds change, so the fingerprints are re-recorded):
1. **Caps rounded up.** A market's cap per pool was `⌊expected never-partnered survivors⌋`, and a pool with 0.7 expected singles could take no union. A seeker's expected unions are a yearly rate of its pool, at most doubled by the market's scaling, so the cap binds only through rounding noise, and a floor trims small pools systematically. The cap is now `⌈expected⌉`, never above the integer members available. `us-tiny` had lost 18% of its solved couples to it.
2. **The founders' first year.** Founders already partnered at y0 come from a market whose wants are the expected partnered members, but its cap was the expected *never*-partnered. So most of them stayed single: in `us`, women born 1800–1819 were 25% ever partnered. Founder pools are now `founding` during that market: their capacity is their expected partnered members, and the unions they take leave the never-partnered pool alone.
3. **Deferred, not lost.** A couple isolated on both sides (no other couple to repair with) is deferred. Before, it returned to the pool and sought again only at the normal hazard, so in thin blocks each deferral was mostly a lost union (`us-tiny`: a quarter of pairs). Now each side carries the deferred want into the next year's market: the union is delayed, and the extra demand makes a second couple per cell likelier. The carry is bounded (at most about 5 per pool at prototype scale).
4. **Arriving couples rounded without bias.** `round()` of a group's expected couples gave none to groups with one or two arrivals a year. Now it is `partition::round_unbiased`, new in core: `⌊x + u⌋` with a keyed `u`.
5. **Residual deaths.** The never-partnered natives' death table subtracts each partnered native's mass. Second-union cells were subtracted again, with their first divorced-source index misread as a cohort index. With many native cohorts (migration classes), the never-partnered then died too young: in `us-areas`, 84.5% of women born 1980 survived to 50, against 94.5% in `us`. Each partnered native is now subtracted once, at the first union.

**Area mode itself:**
- a block's members in one area can appear in several market groups a year, so `settle` takes exactly each group's count;
- a divorced source records each remarriage part's cell area (a national market puts it in the partner's area);
- the founders' first-year cells are sorted like every other year's.

**Test world.** `us-areas-tiny` is now four neighbouring areas (PA 1, PA 2, OH 1, OH 2) on a subset of the place tree, generated by `internot_society/data/make_area_test_pack.py` and extending `us-tiny` with 600 founder births a year. With all 62 areas, blocks held a handful of people, and pairwise kin repair could not always avoid siblings: two full siblings partnered in a two-couple cell whose other pairing was also related. The exhaustive kinship suite passes on it and on `us-tiny`.

**Open:**
- close kin at prototype scale in `us-areas`, where small groups in small areas have thin blocks: to be measured;
- build time: 415 s for `us-areas` (40 s at step 1); to be profiled.

## 8. Stage 3 design: couple moves in the woman's plan (2026-10-01)

**As math.** A union's area over time is a step function: its formation area, then at most one step to a destination. The ledger needs it only where it leaves a counted trace: the area of each birth (the child's block), and later the divorced pools. The world needs it per couple, invertibly.

**Where the move lives: in the woman's plan leaf.**
- The world already maps every woman in a union cell to a plan leaf (a block of women with the same birth plan). It builds the mother block's birth line, and its area runs, from the leaves in plan order.
- So each leaf is split, by keyed systematic apportionment, into "stays" and move parts `(move offset k, destination d)`. The move happens k calendar years after the union year, and only while the union lasts: k is below the separation offset.
  - The chance of a move in year k is the pack's long-move rate at the woman's age and the era (the same schedule as single moves).
  - Destinations are drawn from the ledger's `Migration` shares around the formation area.
  - A part is a sub-leaf: the same births, plus the move.
- **Births by area:** a birth at offset o is in the destination if `o >= k`, else in the formation area. The ledger records each part's births there (`Births::in_area`).
  - The world's area runs come from the same parts, in plan order. Mother/child lookups, which go through the runs, need no change.
  - Duality in the exhaustive kinship suite checks that the two sides agree.
- **Kin repair is unaffected:** the move follows the woman, and whoever her repaired partner is moves with her.
- **Deaths are unaffected:** a woman who dies before her move simply doesn't move. Births at or after the move need her alive anyway (her plan already conditions her death on them).
- **Determinism:** the split is a pure function of the leaf, the cell and the key, so the ledger and the world compute the same parts (`plan.rs`, shared).

**v1 limits (debt from the start, measured later):**
- at most one move per union;
- same-sex couples don't move (they have no plans);
- after a separation, the divorced seek partners in the formation area, where their source cell is. A divorced woman who had moved re-partners there, and residence moves her back at the new union.

**Stage 4 (residence) then takes moves between areas from three channels:**
- a never-partnered native's migration class (`World::migration`);
- the couple's move (`World::couple_move`, from the woman's leaf);
- for later single spells, residence's own counted flows, as now.

Each channel is a static function of ids, so the closure's entries for an area are their inverses.

## 9. Stage 4 design: residence on the ledger's areas (2026-10-01)

**Invariant.** In area mode, everyone independent lives in their *ledger area*: the area the ledger counts them in. It is a static function of ids, `World::area_at(x, t)`:
- a single before their first union: their block's area (of upbringing), then their migration class's destination from its move;
- in a union: the union's cell area, then the couple's destination from its move;
- after a separation: the formation area of the ended union, where their divorced pool is (v1);
- widowed: the union's area at its end.

Residence enforces the invariant, so every union forms in its cell area and every seeker lives where its market is.

**Moves between areas come only from the ledger:**
- a single's migration move (spell 0);
- a couple's move. If the union ended by the man's death first, the widow's spell carries it.

Residence's own counted long-move flows are off in area mode (debt: divorced and widowed singles don't move between areas).

**Formation, by ledger facts only, so closures stay exact:**
- **Unions:** the source is the woman (the left partner, for same-sex unions), whose ledger area is the cell area by construction. The man is the source only with the pack's share (`union_source_woman`) and only if his ledger area is the cell area too.
- **First single spell:** near the parents' household if the parents' ledger area is the block's area. Otherwise a fresh draw in that area, for example a child who left home before 18 and whose parents later moved. A spell that begins with the migration move starts fresh in the destination.
- **After a separation:**
  - if the union never moved: as now (keeper rules);
  - if it moved: both return to the formation area, near the union's first position (debt; until divorced pools are kept by area at dissolution).
- **Widowed:** keep the home.

**Closure entries for an area A**, each a static enumeration:
- seeds in A;
- migration movers into A;
- couple movers into A;
- fresh units in A: first spells drawn fresh, returns after a separation, and spells beginning with a migration into A.

Everything else enters A through a source already in A.

**L3, in area mode:**
- roommate frames are keyed by ledger area;
- kin hosting only joins kin in the same ledger area (debt: elders don't move to a child in another area).

**Gates:**
- residence exactness on `us-areas-tiny` (rosters equal brute force);
- the share of person-time where residence's area differs from the ledger area: zero except dependents living with kin elsewhere;
- realism (spec §7 targets) and costs (first touch, rosters) on `us-areas`.
