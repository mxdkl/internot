# R1 prototype: exact kinship from counts

**Date:** 2026-09-30
**Spec:** `specs/2026-09-29-society-as-a-function.md` §5–§6, risk R1 (§18).
**Research:**
- `research/2026-09-29-kinship-and-households.md` §9 (the base construction);
- `research/2026-09-30-residence-enumeration-problem.md` (why regions are *lineage* regions here, pending the founder's decision).

**Crate:** `internot_society`, a new workspace crate alongside `people` (D6). It depends only on `procedural_core`.

## Goal and pass criteria

Show that unions and births can be exact and reciprocal from integer count tables, with every person a pure function of `(seed, id, t)`, at realistic cost. It passes when:

1. **Exact duality**, checked exhaustively on small worlds and by sampling on large ones:
   - `partner_of(partner_of(x)) = x`, and union dates agree from both sides;
   - `x ∈ children_of(mother_of(x))` and `x ∈ children_of(father_of(x))`;
   - every child listed by a parent names that parent.
2. **Ledger closure:** block sizes, union slices and birth counts equal the ledger's integers exactly.
3. **Life bounds:**
   - mothers are alive and aged 12–50 at each birth, and fathers are alive at the birth;
   - partners are alive at union start;
   - nobody partners before 15.
4. **Kin rules:** no partner is a sibling, half-sibling, parent or child.
5. **Realism, sanity bands.** Prototype rates are simplified, so these are checks, not the §15 targets:
   - e₀ and TFR follow the configured eras;
   - median age at first union for recent cohorts is 27–32;
   - never partnered at 45–54 is 10–25%;
   - the parity distribution is close to the configured one.
6. **Performance:** p99 < 5 µs for `partner_of`, `mother_of` and `children_of` (§16.2 one-hop kin), measured by the perf gate.

## Design decisions

Each numbered decision below departs from or refines the research note.

**D-R1.1 Blocks are (lineage region, birth year).**
- A child's region is its mother's region, not where she lived when giving birth (see the residence note).
- Each block is laid out as `[natives | immigrants by arrival year]`.
- Founders (born before `Y0`) are blocks with no in-world parents, sized to the survivors at `Y0`.

**D-R1.2 Only unions and births are counted exactly.**
- Unions and births are the only relations that link blocks, so they are the only integer tables.
- **Deaths are keyed draws**, conditioned on the survival a person's own cells require:
  - partners survive to union start;
  - mothers survive to the end of their fertility plan.
  - *(Revised 2026-09-30, founder decision; see `research/2026-09-30-death-locality-problem.md`.)* Fathers used to be required to survive their partner's plan too. That made 3.9% of deaths look up the partner and put death's p99 at 2–2.6 µs against the spec's 1 µs. Now a father need only be alive at conception: `father()` is the mother's partner at conception, 266 days before the birth. A child conceived after his death has no in-world father until widowed re-partnering.
- People who never partner draw from the block's **residual** death distribution, i.e. the block life table minus the mass that partnered members carry. That keeps block mortality right in aggregate.
- Duality and life bounds are exact regardless, because each death is a pure function of the person.

**D-R1.3 Two independent partitions of each union-start cell.**
- A woman's union-start cell (her block, union year `t_u`) is partitioned twice: by **partner** (market kind and partner block) for couplings, and by **fertility plan** for births.
- The two partitions use separate keyed permutations of the cell. Both marginals are exact; their intersection is never needed.
- This avoids the product blow-up of nesting plans inside partner cells (§3 of the residence note).

**D-R1.4 Two-sex markets by IPF.**
For each market (region `r`, year `t`):
- Desired female unions by age are `F_i = λ_f(i, t) · S_f(i)`, and male `M_j = λ_m(j, t) · S_m(j)`, where `S` are the expected never-partnered pools from the projection.
- The market total is the harmonic mean `T = 2·ΣF·ΣM / (ΣF + ΣM)` (Schoen's rule at the aggregate).
- The cell matrix is IPF of the age-gap kernel `g(j − i)` to the margins `F·T/ΣF` and `M·T/ΣM`.
- It is then integerized by keyed stochastic rounding. Both sides read the **same** integer matrix, so no controlled rounding is needed.
- A block's union counts are defined as its row or column sums, capped by its size.

**D-R1.5 Plans are a catalog, apportioned by largest remainder.**
- A plan is (birth offsets in years from union start, dissolution duration or none).
- The catalog is generated per era and mother-age band. Births never follow the dissolution.
- Counts per (block, `t_u`, plan) are a pure function of the cell size and the catalog probabilities, so they are computed on demand, not stored.

**D-R1.6 Two orderings of a block's natives.**
- The **life line** (`π_life`) orders sex, then union-start cells, then the never-partnered remainder.
- The **parent line** (`π_parent`) orders by mother event: mother block, union-start year, plan, birth index, offset, then non-union births.
- Sex comes from the life line, so it is independent of the mother.

**D-R1.7 Kin repair by pair swap.**
- Within a coupling slice, pairs `(j, j⊕1)` swap partners if either pair is related.
- Both sides evaluate the same predicate over the same four people, so the swap is reciprocal.
- A residual related pair is counted by the test and must be 0 in samples.

## Build order

- **R1a:** one region; founders; first unions; plans with divorce; non-union births; conditional deaths; kin repair; the API and tests.
- **R1b:** two regions with a national cross-region market; immigrants.
- **R1c:** re-partnering; same-sex unions. Deferred from R1 if time runs short; documented.

## R1a outcome (2026-09-30)

R1a passes every criterion above. Numbers and lessons are in AGENTS.md ("Phase 1"). Semantics pinned while building it:
- **Calendar-year deaths.** A death falls in calendar year `birth year + age`. A required age `r` means alive through year `birth year + r − 1`.
- **Separations after births.** A separation falls in a calendar year at or after its band's start, so every plan birth precedes it. The father of a plan birth is always the mother's partner.
- **Two-stage death draw.** Draw the death age conditioned on the person's own constraints. If it passes the bound on what their births could require, keep it; otherwise redraw conditioned on the full constraints. This is exactly the conditional law, and a man's death needs no partner lookup unless he dies before his partner's childbearing ends.

## R1b design (2026-09-30)

R1b is split in two so that each half passes its gates before the next starts.

### R1b-1: lineage regions and a national market

**Blocks** are `(birth year, lineage region)`, indexed `(year − first_year) · R + r`. Every pair exists, possibly empty, so ids stay grouped by birth year. A child's block is `(birth year, mother's region)` (D-R1.1).

**Founders:** each region gets `founder_weight_r` of the founder births.

**Markets.** Each year, every block's desired unions `want` (hazard × never-partnered pool) split into two parts:
- `(1 − ρ(t))·want` goes to the block's **local** market, one per region;
- `ρ(t)·want` goes to the **national** market, one for all regions.

Both markets are cleared as in R1a: harmonic-mean total, IPF over the age-gap kernel, keyed rounding (the key includes the market), and caps against the remaining pools. Local markets clear first; the national market then clears against what is left.

The national kernel is region-blind, so its IPF factorizes. With aggregated age margins `F_i`, `M_j` and their IPF solution `X_ij`, each cell is `x_(r,i),(s,j) = X_ij · F_ri/F_i · M_sj/M_j`. At R = 2 the prototype runs the joint IPF directly; for many regions, use the factorized form, which keeps the cost at `O(A²)` per year plus the integerized expansion.

**Cells.** All of a year's markets are merged into one cell per (block, sex, year) before cells are recorded. The partner slices of the local and national markets add up, so there is still one union cell per year, and its plan partition covers the whole cell.

**ρ(t) is provisional.** Its meaning depends on how fine the lineage regions are, which waits on the residence decision. Proportions of spouses born in different states or divisions (IPUMS) are the calibration target once the partition is fixed. The prototype uses ρ rising from 0.20 (1840) to 0.50 (2020). With two regions of similar size, about half of national unions cross regions, so roughly 10% → 25% of unions are cross-region. The realism report prints the realized share.

**Tests:**
- all R1a properties, over a two-region tiny world;
- every child is in its mother's region;
- ledger closure per (block, block, year) slice;
- cross-region unions exist in both directions at roughly the configured share;
- each region's cohorts pass the realism bands.

### R1b-2: immigrants (settled 2026-09-30)

**Rejected: immigrants as their own blocks** `(birth year, region, arrival year)`. That would reuse all of the block machinery, but a year's market would then have about 100 blocks per birth year instead of 1–2. Even with the birth-year IPF, integerizing the pairs and storing partner slices would grow about 10⁴-fold. Arrival cohorts therefore live **inside** birth blocks, and the market still sees one participant per block.

**Step A: entry cohorts, adults arriving single.**
- **Ledger:**
  - A block holds cohorts `[natives, arrivals by year]`. Each cohort has its own expected pools (never partnered, alive) and its own used counts.
  - Each year `t`, the inflow is `immigration_rate(t)` × expected alive population. It is apportioned by region (`Region::immigrant_weight`), arrival age (18–80, adult profile) and sex, and appended as the arrival cohorts `(t − age, r)`.
  - The markets clear per block as before. A block's union count for the year is then apportioned over its cohorts by never-partnered pool, capped by each cohort's unused members, and recorded as the cell's `cohorts: [(cohort, count)]`.
  - Non-union plans are per cohort. Only births after arrival (`year ≥ t_a + 1`) are in-world and counted.
- **World:**
  - Raw ids are `[natives | arrival cohorts]`. Each cohort has its own life permutation and a line `[women | men]`, where each sex is `[sub-cells by union year | never]`.
  - A cell's in-cell index space is the concatenation of its cohorts' sub-cells, in cohort order. Partner and plan permutations act on it as before.
  - `LifePos` gains a cohort, and "in-cell index" replaces "offset − cell.start".
  - The parent line covers natives only, so immigrants have no in-world parents.
  - Death: the entry age of arrivals is `t_a − birth year + 1` (alive through the arrival year). The residual stays per block, over all cohorts.
  - New API: `is_immigrant(id)` and `arrival(id)`, a keyed date in the arrival year.
- **Tests:**
  - every R1a/R1b-1 property;
  - nobody partners before arrival;
  - no immigrant has in-world parents;
  - cohort and sub-cell counts match the ledger exactly;
  - the foreign-born share of the living is plausible (US: 9.7% in 1850 rising to 14.7% in 1910, 4.7% in 1970, 13.9% in 2022).

**Step B: arriving couples and their children (design pinned 2026-09-30).**

*Couples.*
- Each year and region, a share `κ(t)` of adult arrivals arrive as couples.
- Couple women's ages follow the arrival profile weighted by the share ever partnered at that age. Men are matched to them through IPF over the age-gap kernel, restricted to husbands no more than 3 years younger. Singles make up the rest of the arrivals, as in step A.
- A couple is in the arrival cohort `t_a` of both partners' blocks, and forms the union cell `(block, sex, t_a)`. Arrivals of year `t_a` never join that year's in-world market, so the `t_a` cohort's sub-cell holds exactly the arriving couples, and it is the cell's last sub-cell.

*Arrival plans.*
- A cell's plan order becomes `[in-world women | arriving women]`, with a separate keyed permutation for each part:
  - in-world women get `union_plans(n₁, t)`, as before;
  - arriving women get `arrival_plans(n₂, t_a, age)`, with leaves `(d, parity, first, spacing, dissolution)`.
- `d` is the number of union years before arrival, drawn from the first-union schedule, with `d ≤ age − 19`. Together with the gap restriction, both partners are then at least 16 at the union start `t_a − d`.
- Births fall at `t_a − d + o`:
  - **in years `> t_a`**: in-world natives, written into the leaf's usual mask relative to the cell year `t_a`;
  - **in years `≤ t_a`**: abroad children. They arrive with the couple if under 18 at arrival, and are stored as a per-leaf mask of arrival ages 0–17. Older children stay abroad.
- Dissolution bands start after arrival: the plan conditions on bands whose start exceeds `d`, and separation counts from `t_a − d`. One leaf therefore covers the couple's whole fertility, abroad and in-world, counted once.

*Children who arrive.*
- They are the whole arrival cohort `t_a` of blocks `(t_a − c, r)` for `c ≤ 17`; there are no single minors.
- That cohort has an **arrival parent line**, like the natives' parent line: slots by mother's age at the birth, then (mother cell, leaf, child index). Their mother is the arriving woman.
- Their father is her partner: the union started before every plan birth.

*Everything else is unchanged.*
- Kin repair: arriving adults have no in-world mothers.
- Deaths: the entry age covers the arrival year.
- Births after arrival go through the usual birth tables.

## R1b-1 and R1b-2 step A outcome (2026-09-30)

Both pass their gates; numbers are in AGENTS.md ("Phase 1").

**D-R1.7 is revised: kin repair in three layers.** Pairs `(q, q ⊕ 1)` alone leave two kinds of couple unrepairable: the last position of an odd cell, and any couple alone in its cell. The tiny world produced a brother–sister couple in a one-couple cell, so the spec's "residual ~10⁻¹²" does not hold for small cells.
1. **Women's side.** Within a woman's cell, positions form pairs, plus a trailing triple when the count is odd. The default men are permuted to minimise related couples, identity first on ties.
2. **Men's side.** A *free* couple is one whose woman is alone in her cell. Within a man's cell, any group (same grouping rule) that contains a free couple permutes its couples' women (after step 1) among its men. Free positions are precomputed per man cell. Only such groups change, so no step-1 decision is disturbed, and every member computes the same group.
3. **Ledger.** An *isolated* couple (a woman alone in her cell with a man alone in his) has no count-preserving repair at all, so the ledger defers it. Both people stay in their pools for next year; removing it empties both cells and creates no new isolated couple.

What remains possible is a group where every permutation still leaves a related couple. That needs two kin conflicts inside one group of two or three, which never occurred: zero related couples on 7 tiny-world seeds and on the full prototype (3.7M couples), checked exhaustively.

**Residual is natives only.** Never-partnered immigrants draw from the life table conditioned on arrival. Mixing them into the natives' residual had diluted native childhood deaths and pushed native e0 up by 8 years.

## R1b-2 step B outcome (2026-09-30)

Step B passes its gates; numbers are in AGENTS.md. Refinements made while building it:
- **Arrival cells are separate cells** keyed `(year, arrival)`, not the last sub-cell of the year's in-world cell. Slot coupling pairs the i-th woman of a slice with the i-th man of the mirror slice. With shared slices, an arriving wife would have been paired with a native man.
- **Abroad and in-world births.** Births in years up to and including the arrival year are abroad; later births are in-world. A child born in the arrival year is born before the family's arrival date, which is drawn after that birth.
- **The immigration rate counts adults.** Children arriving with couples come on top.
- **κ is low (0.10–0.20).** Couples arriving with their children bring two to three each, and historically many married men came ahead alone.
- **Isolated-couple deferral and men's-side repair apply to in-world cells only.** Arriving adults have no in-world mothers, so no arriving couple is ever kin.

## R1c (same-sex unions) outcome (2026-09-30)

Same-sex unions reuse the whole opposite-sex machinery:
- **The market.** A sex's same-sex seekers are split into a left and a right half and cleared as a two-sided market with a symmetrized age-gap kernel.
- **Coupling by role.** It works by *role* (left takes the women's role), not by sex. Kin repair's three layers and the ledger's isolated-couple deferral apply as they are.
- **Kin predicate.** `related()` became symmetric and sex-general: siblings, a mother or father of either partner.
- **No births (R1).** Same-sex couples plan no births. Dissolution is a keyed draw, since no count depends on it. A woman's non-union child is never fathered by her female partner.

Re-partnering stays blocked on the design choice in `research/2026-09-30-repartnering-exactness-problem.md`.

## Performance pass outcome (2026-09-30, after the founder's decisions)

- **Measurement was wrong first.**
  - Batch calibration timed repeated inputs from cache; single-call timing plus a thermal settle fixed it.
  - It turned out death (2.0–2.6 µs) and mother (1.3 µs) were failing, and union, father and children were at 4.4–4.9 µs against 5 µs.
- **Death:** D-R1.2 was revised by the founder. A father need only be alive at conception (`research/2026-09-30-death-locality-problem.md`), so no death looks up anyone else. p99 went from 2.3 µs to ~0.6–0.9 µs.
- **Locality,** with each step A/B-tested with identical answer checksums:
  - one array per search (slices, sub-cells, plan leaves with starts);
  - inline first cohort, union year and no-birth count;
  - cheap exact filters in the kin predicate.
- **Result:** every kinship budget passes with 13–60% margin (AGENTS.md, Phase 1 performance). Baselines were re-recorded under the performance governor.
- **Tools:** `examples/lookup_timing.rs` and `perf/ab.sh`.
- **For R1c:**
  - `same_mother` assumes one union cell per woman; R1c must revisit it, and its agreement test will fail if it breaks.
  - `siblings()` assumes one union per person.
  - Second unions must keep death local: a person's second-union year and a woman's second plan should come from her own sub-cells.
