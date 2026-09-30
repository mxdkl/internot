# R1c: divorce re-partnering, design

**Date:** 2026-09-30
**Status:** design. Build it on the tiny world first, with the exhaustive kinship suite.
**Decided by the founder (2026-09-30):**
- divorce re-partnering now, via a couple-level dissolution partition;
- widowed re-partnering deferred (`research/2026-09-30-repartnering-exactness-problem.md` §4 item 1).

**Inputs:**
- `research/2026-09-30-remarriage-targets.md` (targets);
- `research/2026-09-30-death-locality-problem.md` (every constraint local);
- the R1 plan and its outcome log.

## 0. What must hold

The R1 guarantees, extended to second unions:
- **(G1) Reciprocity.** Both partners compute the same union: start, separation and end.
- **(G2) Ledger closure.** Counts per (woman's block, man's block, year, cell kinds, dissolution year) are exact integers that both sides read. Divorced pools per (block, sex, divorce year) are exact.
- **(G3) Availability.** A second union starts after the first has ended, with both partners alive and unpartnered.
- **(G4) Locality.** Every fact is a closed-form lookup. **Death depends only on the person's own cells.**
- **(G5) No close kin.** No couple are full or half siblings (by mother or by father), or parent and child.

## 1. Scope

- **Opposite-sex unions re-partner. Each person has at most two unions,** first and second.
  - Second unions can dissolve, but there is no third union.
  - That leaves out the 3% of adults (and 8% of newlyweds) on a third or later marriage.
- **Same-sex unions are unchanged:** keyed dissolution, and no re-partnering. They are 1.5% of couples.
- **Widowed re-partnering is deferred** (founder decision).
- **Households, custody and step-relations** belong to L3.

## 2. Dissolution classes live on the couple

- Every opposite-sex union cell partitions **each partner slice** `(i, j)` into **dissolution classes**:
  - `c = 0`: lasts until a partner dies;
  - `c = k` (1..=40): separates in calendar year `union year + k`. Arrival cells count from the arrival year, since the union began abroad.
- **Class counts** come from a **keyed systematic apportionment** of the slice's `n_ij` over the class pmf, keyed by `(year, cell kinds, i, j)`, so both sides compute the same counts:
  - with `E_c` the cumulative expected count and `u` a keyed uniform, `count_c = ⌊E_c + u⌋ − ⌊E_{c−1} + u⌋`;
  - the counts sum exactly to `n_ij`, each is unbiased, and each is within 1 of `n_ij·p_c`;
  - integer arithmetic keeps it exact.
  - Largest-remainder rounding is **not** used here: on the many one- and two-couple slices it always picks the modal class, so no small slice would ever divorce.
- **The class pmf:**
  - first unions use today's `dissolution_pmf(year)`, spread uniformly within each band;
  - second unions multiply the era's divorce share by 1.2 (NSFG: 39% against 33% disrupted at 10 years);
  - arrival couples use classes counted from arrival.
- **Consequence:** a couple's separation year is a fact of its slot, so both partners read it from their own cell. It replaces the dissolution band in the woman's plan leaf, and the keyed year within the band.

## 3. Sub-cells

**A sub-cell** is `(cell, class)`: the members of a cell whose couples share a dissolution class.

- **Index space:**
  - a cell's in-cell index space is class-major: sub-cells in class order, and within each, cohort parts in cohort order;
  - its partner order is also per sub-cell: a keyed permutation of the sub-cell, built on the fly from its size, puts it in slice order (partner block, then slot).
- **Coupling:** the woman at `(class c, partner block j, slot s)` pairs with the man at `(c, i, s)` in his cell, exactly as today with `(c, j)` in place of `j`.
- **Cohort × class contingency:**
  - a cell's class sizes come from its slices, and its cohort counts from the ledger's split over entry cohorts;
  - the integer table with both margins exact is built by sequential integer systematic rounding: cohort by cohort, apportion the cohort's count over the remaining class counts;
  - that is the hypergeometric mean, exact and unbiased;
  - most cells have one cohort (the natives), and then the table is trivial.
- **The cohort line** lists one `Sub` record per `(cell, class)` part: start on the line, index within the sub-cell, sub-cell descriptor, union year and class, in 16 bytes.
  - `death` therefore reads the union year and class from the record it already loads (§8).
- **Sub-cell descriptors** (per block, in an arena) hold:
  - the cell and class;
  - the size;
  - the ranges of the sub-cell's slices, plan leaves, remarriage parts, cohort parts and free couples.

### 3a. Revision (2026-09-30, after measuring stage A): a sub-cell is a cell

**The sub-cell design above cost too much.** Built as described (class-major cells with sub-cell tables, cohort parts and permutations built on the fly), it passed every exhaustive test. But the lookups regressed badly:

| Measure | Change |
|---|---|
| union p99 | +86% |
| father p99 | +132% |
| mother p99 | +124% |
| instructions | about 2× |
| branch misses | 2× |
| world build | 1.0 → 2.0 s |

The flamegraph put it in the extra levels of indirection: sub-cell table → slices → parts, cohort-line records three times larger, and permutations built per call.

**The fix: make every (cell, class) a cell in its own right.** The class joins the cell key next to the kind, `(year, kind, class)`. Each class-cell has its own slices, stored partner permutation, inline first cohort, plans, free list and birth-table column.

**Everything in §§2–5 still holds:**
- the mirror of class-cell `(i, year, kind, class)` is `(j, year, kind', class)`;
- kin repair works within a class-cell;
- de-isolation works at the class-cell level;
- the ledger splits each cell's cohorts over its classes with the two-margin systematic table, and records one `UnionCell` per class.

**Cost:** more, smaller cells, so more memory. **Gain:** every lookup path is the pre-R1c one, which is measured and fast.

## 4. Kin repair within sub-cells

- **Women's-side groups:** `repair_group` over partner positions within the sub-cell (pairs, with a trailing triple).
  - Permuting men among women of the same `(block, year, kinds, class)` keeps every count exact.
- **Men's side:** a couple is free when its woman is alone in her sub-cell. Men's-side groups run within the man's `(M, c)` sub-cell.
- **Isolated couples:**
  - A couple alone in both its women's sub-cell and its men's sub-cell has no count-preserving repair.
  - The ledger moves such a couple to the **nearest class** (by year distance, earlier on ties) in which the woman's cell or the man's cell has another couple.
  - **One pass suffices.** The move empties two singleton sub-cells and only adds members elsewhere, so no couple becomes isolated.
  - Couples alone in both *cells* are still deferred to the next year's market, as today; this runs before classes.
  - Moves shift a divorce by a year or so. The realism report tracks how many there are.
- **The predicate** (`related`) gains paternal half-siblings, which exist once a man has two unions:
  - **Cheap filter:** the two births come from union cells where the father is in his first union and in his second union respectively, which the cell kinds show.
  - **Full check:** compare fathers without repair, as today. That lookup goes through the unrepaired default partner in the birth's cell, so it cannot recurse.
- **`same_mother`'s shortcut:** a woman now has up to two union cells, her first union's and her second's.
  - Births in different cells can still share a mother when one cell is a second-union cell drawing from the other's block.
  - Only the first-union-against-first-union case keeps the shortcut. The agreement test (`world::tests`) must keep passing, and extends to second unions.

## 5. Plans per sub-cell

- **Partition:** each women's sub-cell gets its own plan partition over the union year's catalog:
  - births stop before the separation year (offset `< c`) and at 45;
  - the nested apportionment uses keyed systematic rounding, unbiased on small sub-cells;
  - the plan permutation is per sub-cell, built on the fly;
  - leaves live in a per-block arena, sub-cell by sub-cell.
- **The birth table** keeps one column per plan cell.
  - `locate_birth` scans the cell's leaves across its sub-cells, as today, so the sequential scan stays.
  - Finding a mother then goes sub-cell → leaf → plan position → inverse plan permutation → index in the sub-cell → cohort part → person.
- **Second unions:** a provisional schedule, the union year's parity pmf with its mass shifted one parity down. Mother-age truncation does the rest. The calibration target is **unsourced** (research note §6).
- **Arrival cells:** arrival plans, conditioned on the class.
- **Largest remainder stays** for the non-union partition, which covers whole blocks and is large.
- **Existing first-union plans** switch to keyed systematic rounding as well: small cells were getting only modal plans. Realism is re-checked afterwards.

## 6. Markets with status

**Seekers per block and sex:**
- the never-partnered, as today;
- the **divorced and available**, with their pool made of **sources**: the first-union sub-cells whose separation year has passed. Each source has an exact member count, and expected survivors like today's pools.

**Wants:**
- The divorced want `Σ_sources h_R(sex, age, years since divorce, year) × expected available survivors`.
- `h_R` is provisional:
  - by duration, about 15%/yr in years 1–3, 13% in years 4–5, 9% in years 6–10, and lower after;
  - × an age factor falling from 1 below 35 to 0 at 75;
  - × a sex factor (men higher);
  - × an era factor (young remarriage fell steeply after 1960).

**Clearing:**
- Rows and columns are `(block, status)`.
- The kernel is the age-gap weight times a status homophily factor:
  - unions involving a divorced partner use a wider gap kernel (16% have the husband 10+ years older, against 4% of first unions);
  - homophily is calibrated so that about 20% of new unions have both partners previously married and about 20% have one.
- IPF runs over `(birth year, status)` groups, with keyed rounding.
- Same-sex markets keep only the never-partnered.

**Cell kinds**, from `(own status, partner's status)`:

| Own status | Partner's status | Kind |
|---|---|---|
| never partnered | never partnered | `InWorld` |
| never partnered | divorced | `FirstWithSecond` |
| divorced | never partnered | `SecondWithFirst` |
| divorced | divorced | `SecondWithSecond` |

- `partner()` maps each kind to its mirror.
- Cell keys become `8·year + kind`.

**Takings:** a divorced block's takings settle over its sources (weight: hazard × expected survivors, capped by available members). Each second-union cell records its sources as `(source sub-cell, count)`.

## 7. Second-union membership

- **The remarriage partition:** each first-union sub-cell with `c ≥ 1`, whose members are all in their first union, carries **remarriage parts**:
  - one `(second-union cell, count)` per market it fed, in `(year, kind)` order;
  - the rest never remarry.
- **A member's part** comes from a keyed permutation of the sub-cell, built on the fly: position, then part.
- **A second-union cell's index space** is its sources in order.
  - Its partner order, slices, classes, plans and repair work as in §§2–5.
  - Mapping an in-cell index back to a person goes source part → source sub-cell index (inverse remarriage permutation) → cohort part → person.

## 8. Life constraints, all local

A person must survive through the year before each of:
- **entry** into the world;
- **the first union's year + 1**, from the `Sub` record;
- **the second union's year + 1**, from the person's own sub-cell's remarriage part;
- **for a woman, the last birth** of each union's plan (from her own sub-cells) and of her non-union plan.

**Rules that stay:**
- Nobody has to survive to a separation. A union ends at `min(separation, first death)`. Pools count the formally divorced, and those who remarry are alive at the remarriage by the constraint above.
- A father must be alive at conception (child side, founder decision).

**Death's fast path:**
- **Men:** entry and union years come from `Sub`. For a man in a class `c ≥ 1` first union, his own draw is compared with the sub-cell's latest remarriage year.
  - A small bound can sit in the descriptor, or remarriage stops at 75.
  - Only when the draw falls below it does he read his part.
- **Women:** the same, plus today's plan bound.
- **Risk:** of all people, 5–8% may need one or two extra loads. Measure it against the 1 µs budget; huge pages are the reserve lever.

## 9. Lookups and API

- **`unions(id) -> [Option<Union>; 2]`** (first, second).
  - `union(id)` stays as the first union.
  - `partner_at(id, t)` checks both.
- **`father`:** for a union birth, the mother's partner in that union if he was alive at conception. For a non-union birth, the mother's partner at conception.
- **`children`:**
  - a woman's are her non-union births plus both unions' plan births;
  - a man's are each partner's union births conceived while he lived, plus their non-union births conceived while that union was active.
- **`siblings`:** the mother's children ∪ the father's children. The father's other union adds paternal half-siblings; the mother's other union is already in hers.

## 10. Tests

**The tiny world, exhaustive, over several seeds.** The R1 suite, extended:
- reciprocity of both unions;
- closure per class and kinds;
- availability: the second union starts after the first's separation year, and nobody has two active unions;
- duality of mother, father and children across both unions;
- father alive at conception;
- siblings as the union of both parents' children;
- **zero close kin, including paternal half-siblings**;
- determinism;
- the ledger's pool accounting: every source's members = parts + never;
- the de-isolation invariant: no couple is alone on both sides of its class.

**The prototype, by sampling and realism report.** Targets from the research note:

| Target | Value |
|---|---|
| Women remarried within 1 / 3 / 5 / 10 years of divorce | 0.15 / 0.39 / 0.54 / 0.75 (NSFG, divorced < 45) |
| Median years from divorce to remarriage | ~4 |
| Median age at second marriage | men 36, women 33 |
| New unions with previously married spouses (2010s) | ~20% both, ~20% one |
| Couples with husband 10+ years older | 16% of remarriages, 4% of first unions |
| Second unions disrupted within 10 years | ~0.39 |
| Adults aged 50–69 with two unions | ~20% |
| Couples moved by de-isolation | report the share |

**Performance:**
- the kinship suite, plus new worst cases (the remarried, and the largest half-sibships);
- budgets: death 1 µs; mother 2 µs; union, father and children 5 µs; siblings 4 µs, to be revisited if half-siblings through fathers push it, since spec §16.2 has no sibling line and multi-hop kin gets 50 µs;
- memory reported: an estimated +60–80 MB (sub-cells, per-class slices, per-sub-cell plans, sources).

## 11. Build order

1. **Primitive:** keyed integer systematic apportionment (unbiased, exact sum). Also its two-margin form for the cohort × class table. Both get property tests, and live in `plan.rs` or `procedural_core::partition`.
2. **Ledger:**
   - slice classes and de-isolation;
   - cohort × class tables;
   - divorced sources and status markets;
   - the four kinds of opposite-sex cell, with second-union sources and remarriage parts;
   - per-sub-cell plans and their births.

   Ledger tests: closure, pool accounting, de-isolation, determinism.
3. **World:**
   - class-major sub-cells and `Sub` records;
   - coupling and repair within sub-cells;
   - second-union membership;
   - the lookups and predicates.

   Exhaustive tiny tests.
4. **Realism:** new report sections, then calibration of the provisional parameters (`h_R`, homophily, the remarriage gap kernel, second-union parity).
5. **Performance:** the suite and gates, with A/B for each layout choice.

## 12. For the founder (proceeding with the recommendation unless told otherwise)

1. **At most two unions per person.** A third union needs the same machinery one level deeper (second-union sub-cells with remarriage parts). It adds 3% of adults. Recommendation: two now.
2. **No same-sex re-partnering in R1c.** Same-sex dissolution is a keyed draw with no exact counts. Adding it means giving same-sex cells classes and status markets too. Recommendation: later, with widowed re-partnering.

## Stage A outcome (2026-09-30): dissolution classes and class-cells, no remarriage yet

**Built:**
- keyed systematic apportionment (`procedural_core::partition::{apportion_systematic, contingency_systematic}`, pinned goldens);
- per-slice dissolution classes and de-isolation in the ledger;
- one `UnionCell` per (year, kind, class), with cohorts split by the two-margin table;
- plans keyed per class-cell and truncated at the separation year;
- the world on class-cells.

**Tests:** all pass. The exhaustive kinship suite on the tiny world checks:
- reciprocity;
- closure per (year, kind, class, block, block);
- zero close kin;
- duality;
- life bounds;
- determinism.

Two unit tests were added (`Coarse`, and the `same_mother` agreement test).

**Realism** (all bands pass):
- De-isolation moved 0.59% of opposite-sex couples to a neighbouring divorce year.
- Divorce in early cohorts rose (1860: divorced by 20 years went from 3.2% to 5.9%) because largest remainder no longer starves small cells of divorces.
- Births per year fell 5–8% before 1950 as a result.

**Layout, as measured:**
1. **Sub-cells inside cells (§3):** 2× instructions; union +86% and father +132% p99.
2. **Class-cells (§3a):** 592k cells (was 97k), 113 MB of cells, and a 35 MB birth table.
3. **One cache line per cell:**
   - a 64-byte `CellLayout`, with `CompactPerm` stored as 24-byte `CompactParts` (new, additive);
   - cell arrays (slices, cohort parts, free lists, plan leaves, arrival data) in per-block arenas behind `CellView` and `PlanView`;
   - cells now take 80 MB.
4. **`Coarse` sorted arrays** (every 16th value indexed) for cell keys and cohort lines, which grew about 6×. Their binary searches were 20–30% of `union`. Result: death −14%, union −7%.
5. **Birth columns:**
   - grouping a year's class-cells into one column made every birth lookup scan all their leaves: mother +11%, siblings +25%;
   - reverted to one column per class-cell, with a coarse index per row: mother −20%, siblings −20%, children −10–20%.
6. **Build:**
   - allocation-free parity pmf and plan splits;
   - the class split as a sorted `Vec` (the `BTreeMap` of 328-byte values was 26% of the build);
   - union-age densities computed once per arrival year.

   World build went from 2.1 to 1.52 s.

**Result** (single-call p99, performance governor), against the pre-R1c baseline:

| Query | p99 | Before | Change | Budget |
|---|---|---|---|---|
| death | 0.60 µs | 0.60 µs | 0% | 1 µs |
| mother | 1.26 µs | 1.17 µs | +8% | 2 µs |
| father | 4.76 µs | 3.07 µs | **+55%** | 5 µs |
| union | 4.41 µs | 3.14 µs | **+41%** | 5 µs |
| children | 4.74 µs | 3.95 µs | +20% | 5 µs |
| siblings | 3.27 µs | 2.78 µs | +18% | 4 µs |
| world build | 1.52 s | 1.04 s | +46% | 2 s |

**The cost is structural.** Exact-year classes make about six times as many, smaller cells, so person lookups search longer arrays and touch colder cells.

**Risk for stage B:**
- **Margins:** father and children have about 5% margin left, and second unions add lookups.
- **Levers left:**
  - huge pages (measured 10–20%);
  - a two-level cohort line (year group, then class);
  - fewer dependent loads per person lookup.

## Stage B outcome (2026-09-30): divorced pools, status markets, second unions

**Built:**
- divorced sources: each first-union class-cell that separates, with its members' remarriage parts;
- divorced pools per block and sex, with a shared survival factor and per-divorce-year aggregates;
- status markets (never partnered or divorced) with the cell kinds `FirstWithSecond`, `SecondWithFirst` and `SecondWithSecond`;
- second-union class-cells whose members come from sources, split by a keyed systematic sweep (years by hazard × expected survivors, then sources, capped);
- seats, at most two unions per person;
- second-union plans as a mixture (`SECOND_UNION_FERTILE`);
- `siblings()` with paternal half-siblings.

**Tests:** all pass (845 in the workspace). The exhaustive kinship suite now covers both unions:
- reciprocity matched by partner and start;
- closure per (year, kind, class, block, block, sex);
- zero close kin;
- duality;
- life bounds (fathers alive at conception).

**Realism (calibrated):** remarriage within 1/3/5/10 years is 17/39/52/69% (NSFG 15/39/54/75). The rest is in AGENTS.md, R1c.

**Build time, 4.28 s → 1.45 s** (16 threads; 3.44 s on one thread). Every step was verified bit-identical by a ledger checksum and a lookup-answer checksum. Measured shares of the 4.28 s:
- the free-couple search into other blocks' ledger cells, 20%: now one pass over single-member cells;
- class split and de-isolation, 454 ms: precomputed `SystematicShares` with sparse parts, and dense per-block class arrays instead of hash maps;
- IPF, 457 ms: fused passes, eight row sums side by side;
- plans: `PlanShares` per union year;
- then `rayon` over blocks (layouts, the per-block record work), over a year's markets (solve in parallel, apply caps in order) and over rows (rounding).

**Gate (performance governor): the lookups fail.**

| Query | p99 | Stage A | Budget |
|---|---|---|---|
| death | 1.77 µs | 0.60 µs | 1 µs |
| mother | 1.93 µs | 1.26 µs | 2 µs |
| father | 12.3 µs | 4.76 µs | 5 µs |
| union | 11.4 µs | 4.41 µs | 5 µs |
| children | 15.2 µs | 4.74 µs | 5 µs |
| siblings | 22.4 µs | 3.27 µs | 4 µs |
| world build | 1.59 s | 1.52 s | 2 s |

**Memory: 890 MB peak RSS** (stage A not measured; R1: 152 MB).
- Union cells: 1.30M (stage A: 592k).
- Partner slices: 4.77M, of which 4.0M hold one couple.
- Cohort or source parts: 2.93M.
- Divorced sources: 656k, with 1.64M remarriage parts.

**Open:** profile the lookups and the memory before choosing a fix. The class-per-cell layout multiplies cells, and second unions add a hop through sources. This may need a design change, not tuning, and so a founder decision.
