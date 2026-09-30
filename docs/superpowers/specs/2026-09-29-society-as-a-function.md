# A society as a function of (id, t)

**Date:** 2026-09-29
**Status:** Design approved by the founder 2026-09-29 (§17, D1–D8). Nothing is built yet. Next: the Phase 0 benchmark harness, then the risk prototypes (§18).
**Supersedes:** the relationship, time and population parts of `internot/src/people/`; `specs/2026-05-14-social-graph-substrate.md` (its principle "edges are shared objects" survives, its mechanics don't); `specs/2026-09-26-eval-first-service-rebuild.md` (services are rebuilt on this model instead).
**Keeps:** `procedural_core` (extended in §4), `procedural_overlay`, `internot_renderer`, the MCP transport, the individual-attribute derivations in `people` (re-keyed in §11), and the coherence experiment in `specs/2026-09-29-coherence-eval-awareness-experiment.md`, which becomes the first consumer.
**Research behind it** (all in `docs/superpowers/research/`, dated 2026-09-29 unless noted):
- `local-access-and-bijections`
- `kinship-and-households`
- `time-consistent-evolution`
- `work-and-organizations`
- `affiliation-and-friendship`
- `happenings-and-content-diversity`
- the 2026-05-14 notes, as corrected by the above.

Numbers in this spec are quoted from those notes; each note gives the source.

---

## 1. What this has to achieve

**The founder's constraints:**
- Every fact about a person is a pure function of `(seed, id, t)`.
- No materialized population and no simulation pass over people.
- The social graph looks real.
- Content isn't repetitive.
- The world is only content served through tools, not generated websites.

**Guarantees.** Each one becomes a property test (§14).

| # | Guarantee |
|---|---|
| G1 | **Determinism.** Same seed, same query, same answer, regardless of query order, thread, cache state or `Universe.now`. |
| G2 | **Two-sided consistency.** Every relation is one object seen from both ends. Partners are mutual. A child's mother lists the child. `x ∈ roster(W, t) ⇔ employer(x, t) = W`. Ties are symmetric. |
| G3 | **Life bounds.** Nobody does anything before birth or after death. Minors don't work. Mothers are alive and aged 12–50 at each birth. No partners are siblings, half-siblings, or parent and child. |
| G4 | **Time.** Any t from 1840 to 2150 is answerable directly. Event streams recombine: events in [a,b) ⊎ [b,c) = events in [a,c), bit for bit. |
| G5 | **Realism.** The population, families, households, moves, work and friendships match the target tables in §15 within the stated tolerances. |
| G6 | **Content.** Every piece of content traces to a structured happening. Prose passes the diversity and faithfulness suites in §13. |
| G7 | **Speed, measured.** Per-person queries in microseconds, rosters in milliseconds, view calls under a second, all at p99 over the full id space and the worst cases, and scaling with cores. Every budget is a benchmark gate (§16). |

**Out of scope:**
- Generated websites or UIs.
- LLM-as-judge for task verdicts.
- Agent convenience tools.
- Real people or organizations. Names are real *names*, not real individuals, and organization names are procedural and collision-checked (§11.3).

---

## 2. The mathematical frame

Every construction in this spec is an instance of the following principles.

### P1. Relations live on shared objects

A relation between people is never derived from one person's seed. It lives on an object both people can name:
- a union,
- a household,
- a job spell,
- a group cell,
- the unordered pair,
- a happening.

Each participant reaches the object through their own derivation and reads the same facts from it. This fixes the old spouse and manager bugs by construction.

### P2. Coordinate systems

A coordinate system is a keyed bijection π from a dense index space onto (group, slot). A group's members are the preimages of its slots, so enumerating a group needs no search.

Two implementations:
- **Black–Rogaway generalized Feistel with cycle-walking, 6 rounds by default and never fewer than 4.** Measured at 14 ns per evaluation at n ≈ 4·10⁹. With 2–3 rounds, sequential inputs show strong structure (chi²/df ≈ 19).
- **A hash-sorted table** for blocks of 64 or fewer.

### P3. Counted spaces

A bijection needs a dense domain. Every index space in this spec is dense because its size is a **count** shared by both sides. There are two ways to get one:

- **Aggregate count tables.** The demographic ledger (§5.2) holds the numbers that two sides must agree on:
  - people born per block,
  - unions between cohorts,
  - births by mother cohort,
  - migrants by region pair.
  
  A range of such a count is cut into nested quota cells, and positions in the cells are the index space.
- **Bounded per-object enumeration.** Examples: the members of a household, a person's spells, a workplace's roster. These are small, so they are simply enumerated.

This is the same trick demographers use to make cohort-component projections consistent, applied to individuals as rank positions.

### P4. Static bijections, dynamic indices

Membership that changes over time (where you live, where you work) uses a few static bijections per space, K of them. The person's own history, a hazard walk (P6), decides which bijection is active in each spell.

A roster enumerates the K preimages of each slot and keeps the people whose active index matches. It is consistent by construction, and it allows any tenure law and any age effect.

Per-epoch reshuffling is rejected. It is slow (about 36,500 rounds per century), blind to who is being moved, memoryless in tenure, and forces every move to be a two-way swap.

### P5. Bijections at city scale, hash partitions inside bounded groups

Bijections are only needed where the parent set is unbounded (a region's residents). Inside a bounded group (a household, a class, a workplace roster, a neighbourhood) the parent is enumerated and sub-groups are hash partitions of it: `circle = hash(x, group) mod n`.

### P6. Absolute time keys

Every random draw is keyed on an absolute coordinate, never on the query window. The coordinate is one of:
- an absolute day bucket,
- a dyadic time-tree node,
- an ordinal counted from a fixed anchor such as birth, union start or tie start.

This fixes the existing `enumerate_events` recombination defect, which has two causes: the event index restarts at the window start, and so does the daily grid.

Four recipes (`time-consistent-evolution` §2):

| Recipe | Construction | Use |
|---|---|---|
| A | Absolute buckets with a Poisson count per bucket | dense streams such as messages |
| B | Dyadic count tree with binomial splits | "how many before t" over centuries, ~1 µs |
| C | Hazard inversion walked from an anchor | life-course events; ≤ ~30 per life, 3–6 µs |
| D | Hawkes processes via their cluster representation | only if burstiness tests fail |

### P7. One owner per fact, a derivation DAG

Every fact has exactly one owner. Anything that defines a block (birth year, birth region, sex) never depends on anything derived from it. The DAG is in §3. A cycle is a design bug.

### P8. Memoization is allowed; state is not

A pure function may be cached: per block, per person, per request. A cache must never change an answer (G1), and tests clear caches and compare.

The demographic ledger is such a cache. It is a set of integer tables computed once from `(seed, parameters)` and versioned with them.

### P9. Time never comes from `now`

A fact is a function of the time asked about. `Universe.now` only picks which t a view asks for. Tests check that facts don't change when `now` moves.

---

## 3. Architecture

Dependencies point downward only.

```
L10 services (directory, mail, calendar, tasks) + renderer         ─ views of L9
L9  happenings (projects, meetings, requests, appointments, plans…) ─ from L6–L8 + time
L8  social ties & strength (foci, circles, kin ties, layers)       ─ from L2–L7
L7  individual attributes (names, personality, education, occupation, schedule, voice)
L6  work (establishments, firms, careers, rosters, org charts)     ─ from L4, L5, L7
L5  education (schools, classes, colleges)                          ─ from L3, L4
L4  residence (regions via ledger; cities/neighbourhoods via bijections)
L3  households (derived from kinship + leaving home + roommates)
L2  kinship (unions, births, parents, children, siblings)          ─ ledger + L1
L1  population (ids, blocks, birth, sex, death, alive_at)          ─ ledger
L0  procedural_core primitives (§4) + the demographic ledger (§5.2)
```

`people` keeps its individual-attribute code (L7). Kinship, residence, work and ties move into new sibling modules under `internot/src/`:
- `population/`
- `kinship/`
- `households/`
- `residence/`
- `education/`
- `work/`
- `social/`
- `happenings/`

---

## 4. L0 — primitives added to `procedural_core`

Each primitive is domain-free and ships with property tests. The costs marked ● were measured by the research prototypes; the rest are estimates.

| Primitive | Contract | Cost |
|---|---|---|
| `FeistelPerm::new(n, key)` → `fwd`, `inv` | Bijection on [0,n); 6 rounds (4 minimum); no floats; golden values pinned per (n, key) | ● 14 ns |
| `GrowablePerm` | Cycle-walk over a fixed capacity. Growing n→n+1 changes exactly one old image. Used for blocks whose size may be revised. | ~20 ns |
| `Pairing` | σ = π⁻¹∘(⊕1)∘π over a paired prefix. σ(σ(x)) = x; pair id shared by both. | ● 36 ns |
| `Coupling` | Rank-range matching between blocks from an integer plan M(a,b). Exact reciprocity; slice counts equal M. | ~50 ns |
| `SizeClasses` | Partition of [0,n) with an exact size histogram; `group_of` O(log K), `range_of` O(1) | perm + 20 ns |
| `CountTree` | Interval sums with binomial/Poisson splits (Goldreich–Goldwasser–Nussboim); `rank`, `select`, `count(lo,hi)` | 2–6 µs |
| `SplitTree` | Fragmentation partition (≈ lognormal sizes, native nesting) | O(depth) |
| `QuotaTree` | Nested integer cells over a rank line; `locate(r)`, `range(cell)`. Every margin is a sum of integer cells. | O(log cells) |
| `EventStream` A/B/C/D | Window-independent streams (P6); `events(key, t1, t2)`, `count_before(key, t)` | per recipe above |
| `HazardWalk` | Inverse-hazard walk from an anchor with piecewise age/duration hazards and marks | ≈ 5 µs per life |
| `regen_state` | Markov "hold or redraw" state at t by regeneration lookback | < 1 µs |
| `Lomax` / tie survival | `T = U^{1/γ} − 1`, the power-law survival used for ties (Burt γ by type) | O(1) |
| Structured keys | `(seed, stream: u16, entity: u64, a: u32, b: u32)` hashed with xxh3, replacing `format!` keys | saves ~23 ns per draw |

`graph::enumerate_events` is rewritten on Recipe A. Its ignored recombination test is re-enabled.

`graph::stable_roommates_match` is retired: `Coupling` replaces it, because exact stable matching is provably non-local.

---

## 5. L1 — population

### 5.1 World scope

The world is one country at full population, United States parameters in v1 (§17 D4). Every region is modelled down to neighbourhoods, workplaces and schools (§17 D3). Everything is lazy, so this detail costs nothing until something is queried. The real costs are geography data and validation, so realism tests run first on 1–2 chosen metros. Abroad is a source of immigrants only.

Geography comes from GeoNames (CC BY 4.0) for places, coordinates and populations, and Census for region populations. It is not hand-curated, which retires the `CITIES` table.

### 5.2 The demographic ledger

The ledger is a two-sex, multiregional, multistate cohort-component projection over:
- (region × year × sex × union status), for 1840–2150,
- plus immigration from abroad and migration between regions.

It runs once in expected values, then is integerized so that **every margin is a sum of integer cells**. Its inputs are public rate tables: historical and projected fertility, mortality (cohort life tables) and migration schedules (Rogers–Castro age profiles fitted to CPS). Its outputs are the counts the two sides of every relation must agree on:

| Table | Meaning | Used by |
|---|---|---|
| `N[r][y]` | people born in region r in year y (derived, not free) | id blocks (§5.3) |
| `A[r][y][y_a]` | immigrants from abroad, born y, arriving y_a | immigrant blocks |
| `U[r][y_m][y_f][band]` | first unions formed in r between cohorts, by start band (two-sex harmonic-mean function) | partners (§6.1) |
| `U2[…]`, `U_ss[…]` | re-partnering and same-sex union tables | §6.1 |
| `E[r][r_f,y_f][y]` | births in region r in year y to mothers born in block (r_f, y_f). Stored sparse, since most births happen in or near the mother's birth region. | mothers (§6.2) |
| `M[r→r′][age band][status][y]` | moves between regions, split into persons and couple households | residence (§8.1) |

- **Size:** roughly 20–40 MB of u32 cells, on the order of 64 regions × 310 years.
- **Cost:** seconds, once. The ledger is memoized and versioned with the seed and parameter set.
- **Change policy:** any parameter change re-ids the world, like a seed change.

This is the only computation that is not per-person. It never touches an individual; it only fixes the counts that individuals are then assigned to.

### 5.3 Ids

An id is a dense u32 over everyone ever born in [1840, 2150), plus immigrants.
- **Block layout.** Ids are laid out as contiguous blocks keyed by (birth region, birth year) and (arrival region, birth year) for immigrants.
- **Decoding.** `decode(id) → (block, raw)` is a binary search over prefix sums of about 40k blocks.
- **Size.** About 1.1·10⁹ ids at US scale. Invariant 4 (32-bit references) holds with room to spare.
- **Pushdown.** "Born in r in y" is one id range, so `where_range` pushdown works.

**Birth region is the mother's residence region at the birth.** Because residence moves between regions are ledger-counted (§8.1), `E` knows where births happen. This is what lets 40% of Americans live outside their birth state without breaking the counts.

### 5.4 Per-person basics

Inside a block, a keyed Feistel permutation turns `raw` into a position on the block's **life line**. A `QuotaTree` cuts that line, outermost first, into:
1. **Region at leaving home.** This is derived, not drawn: it is the region of the household the person grows up in on the day they leave. The count of block members per region is closed-form, because leaving-home age is itself a quota cell on the parent line (§6.2), nested under the mother's trajectory cell. Custody after divorce and early parental death nest the same way. §8.1 explains why this level is needed; prototype R2 checks it.
2. **Sex.** Split from the ledger; sex ratio at birth 1.05.
3. **Single region-trajectory cell.** Migration while single, from `M`. After a union ends, the trajectory continues from post-dissolution cells in the re-partnering pool.
4. **First-union cell** (market region, partner cohort, start band) from `U`. The market region is the person's residence region at union start, which step 3 fixes. The remainder of the line is never partnered.
5. **Union plan cell,** keyed by the union so both partners compute it identically. It holds:
   - type: cohabitation, marriage or conversion;
   - parity and birth spacing;
   - dissolution duration;
   - the couple's region trajectory.
6. **Death cell,** from the cohort life table, nested after the plan end so that mothers are alive at every planned birth. People never partnered get the table's residual, which is where childhood deaths land.

From these:
- **Birth date:** birth year plus a hash-derived day with seasonality.
- **Death:** a date inside the death cell.
- **`alive(x, t)`:** `b(x) ≤ t < d(x)`.
- **Age:** `t − b(x)` everywhere, with no buckets. This removes the 17 missing ages and the frozen 18–80 population.
- **Minors** are ordinary people. Views can hide them.

---

## 6. L2 — kinship

This section adopts `kinship-and-households` §9 and extends it with region trajectories.

### 6.1 Unions

- **How couples are matched.** A person's first-union cell names the other cohort's matching slice. The couple is slot j of slice (a,b) in one block and slot j of slice (b,a) in the other (`Coupling`). Reciprocity is exact.
  - The age-gap distribution is hit exactly: 35% within a year, husband 2+ years older in 50%, mean +2.1 years.
  - The two-sex problem is handled: union counts are one shared harmonic-mean table.
  - A Python prototype passed an exhaustive reciprocity check (59k people).
- **Union facts come from the union key.** The key is (region, cohorts, band, offset). From it: start date, type, conversion to marriage, dissolution date, parity plan and the couple's region trajectory. Both partners compute the same.
- **The union ends** at `min(dissolution, first death)`.
- **Kin repair.** Offsets pair as (i, i⊕1). If either pair would join siblings, half-siblings or parent and child, the two pairs swap partners. Both sides evaluate the same predicate, so the swap is reciprocal.
  - **Revised 2026-09-30.** Pairs alone leave one-couple cells and odd tails unrepairable, so the original "about 10⁻¹² per union" was wrong for small cells. The prototype now uses three layers: women's-side groups (pairs plus a trailing triple), men's-side groups for couples whose woman is alone in her cell, and ledger deferral of couples isolated on both sides.
  - **Result:** zero conflicts, checked exhaustively on the prototype and on 7 tiny worlds. See the R1 plan, "R1b-1 and R1b-2 step A outcome".
- **Re-partnering.** Dissolved and widowed people enter pools per (region, band). The pools' composition is closed-form from the plan cells. A second harmonic-mean table `U2` pairs them. Targets: 57% of divorced or widowed people have remarried, and remarriage follows divorce by a median of 3.7 years.
- **Same-sex unions.** Handled by a `U_ss` table (about 1.5% of couples). Within one cohort and sex, the range pairs 2j ↔ 2j+1; that is the only place XOR pairing appears.
- **Cousin policy** is a per-country parameter; "no siblings" is absolute.

### 6.2 Births, parents, siblings

- **Mother events.** For mother cohort y_f, year y and region r, events are ordered by (union cell, plan cell, offset), then by non-union fertility cells. A woman's j-th birth has a closed-form event index.
- **The child side.** The child block (r, y) has a **parent line**, a second keyed permutation of the block. It is cut, outermost first, into sub-ranges by:
  1. mother block (r_f, y_f), with sizes `E[r][r_f,y_f][y]`;
  2. the mother's trajectory cell;
  3. leaving-home-age cells.
  
  The immigrant range follows. Levels 2–3 are what make "region at leaving home" (§5.4 step 1) closed-form.

```
mother_of(x)   = invert the mother cohort's life line at (event cell start + offset)
father_of(x)   = the other partner of that birth's union (None for non-union births
                 without a matched father)
children_of(m) = for each planned birth of m: invert the child block's parent line
siblings(x)    = children_of(mother) ∪ children_of(father) − {x}
```

**Guarantees:**
- The mother is alive at every birth, and her age is within the age-specific fertility range.
- The father's age comes from the union table.
- Twins are two events in one year.
- The sibling-count distribution follows the parity catalog. For example, only about 9% of children are only children.

**Kin reach.** Grandparents, cousins, aunts and in-laws come from composing these functions. Kin counts are checked against a Caswell kinship model run on the ledger's own rates.

### 6.3 Surnames

Surnames live here because they are inherited. Parameters are US defaults:
- A child takes the father's surname, otherwise the mother's.
- A spouse adopts the partner's surname with an era-dependent probability, and reverts after divorce with some probability. **So surnames are time-varying.**
- Founders' and immigrants' surnames are drawn by heritage.

---

## 7. L3 — households

Households are derived, not generated. A household id is a sum type:

```
Household = Union(union_key)
          | Solo(person, ordinal)
          | Roommates(region, band, epoch, group)
          | Dorm(college, cohort)
```

- **Membership at t** follows rules evaluated from the mover's side, in order (`kinship-and-households` §9.6):
  1. Before leaving home: live in the mother's current union household, or with the mother alone. After a divorce, a minor goes to the father with p ≈ 0.25.
  2. During one's own union: the union household.
  3. As a single adult: alone, with roommates, or in a dorm.
  4. As a widowed elder: in one child's household, with a probability by age.
- **Member ordinals** record the order of joining and are never reused. That gives each person a stable (household, ordinal) identity while they are a member, which L4–L6 use as an index.
- **Roommates** are the only household type needing a new matching. Single, non-co-resident adults are chunked into groups of 2–3 by a coordinate system per (region, age band, staggered 2-year epoch). The target is 6.7% of households.
- **Calibration.** Four probabilities (leaving age, custody, elder co-residence, roommate uptake) are tuned against the §15 household targets: mean size 2.50, one-person 29.5%, married couples 46.6%, and so on.

---

## 8. L4 — residence

This is the part the research left open. The constraint is that bijections need dense, static index spaces, while residence is dynamic and depends on where you are now. The solution applies P3 twice.

### 8.1 Region level: counted

- **Moves between regions are ledger quantities,** a multiregional projection in the Rogers tradition:
  - Single people carry region-trajectory cells (§5.4 step 2).
  - Couples carry a joint trajectory in their union plan cell.
  - Children follow their household.
  - When a person leaves home, their own trajectory cells nest under their **region at leaving home**. That region is derived from the parents' trajectory, and the count of block members per region-at-leaving-home is closed-form from the mothers' cells, so the nesting stays exact.
- **Consequence:** "who has a residence spell in region R" is a dense counted space. Each spell has an index; call it the **region-spell index**.

### 8.2 City and neighbourhood level: bijections over region-spell space

Within a region:

- **Residence units.** A residence unit is a single person's region spell or a couple's joint region spell. Units, not individuals, have addresses. A household lives at its anchor unit's address; the anchor is the union for couples, the founder for solo households, and the lowest-id member for roommates. Children and other members live with the household and use no slot of their own.
- **Candidate addresses.** The region's unit-spell space carries K = 4 static coordinate systems ψ_k, each mapping a unit spell to (neighbourhood cell, slot). Slot counts per cell are proportional to its population.
- **Moves within the region** come from the household's move stream (Recipe C):
  - The hazard follows a Rogers–Castro age profile × duration at address × triggers.
  - Triggers are keyed on the causing event: union, birth, job-with-relocation, leaving home.
  - Each move switches the active index. The choice is weighted by distance from the current address, which is legal because the *choice* is person-side. This gives CPS-like distances without origin-free destinations.
- **Joint moves.** A move is keyed on (household, move ordinal), so every member moves on the same day to the same place. Split-offs (leaving home, divorce) are new households, not moves.
- **Rosters:**
  - `roster(neighbourhood N, t)`: enumerate N's slots under each ψ_k, take the preimage spells, and keep those whose active index is k at t.
  - City rosters are unions of neighbourhood rosters. They are rarely needed.
- **Time dilution** is the risk to measure. Region-spell spaces cover all of history, so only a fraction of preimages are active at t. Region-spell spaces are therefore **time-blocked by the spell's entry decade**, so a roster at t touches only the decades that can still be active.

### 8.3 Regions are metros

Regions are metro areas (commuting zones), plus one "rest of state" region per state for everyone outside a metro, so every region has cities and neighbourhoods. There is no separate "background" class: every person lives somewhere with neighbourhood and workplace rosters. Realism is validated first on 1–2 metros (§17 D3).

---

## 9. L5 — education

- **Schools need no bijection.**
  - `school(x,t) = catchment(neighbourhood(x,t))[grade(age at school-year start)]`.
  - Catchments are a static partition of each city's neighbourhoods.
  - A class roster is the catchment's resident households' members of the matching birth cohort, enumerated through L4.
  - Enrolment runs from age 5 to 18, with dropout by the person's education plan.
  - Aging out and moving house fall out of the definitions.
- **Education level** is a function of the person and their parents' education: intergenerational transmission plus a hash. It replaces the current independent draw.
- **College.** Colleges sit in cities with capacities. The college coordinate system runs over the **birth-cohort block**, which is static and dense: its preimages are candidate students, filtered by "attends college" and the person's college choice.
  - College residence is the Dorm household.
  - A college class is a focus in L8.

---

## 10. L6 — work

This section adopts `work-and-organizations` §8, with the roster index moved onto L4's residence spaces.

### 10.1 Establishments and firms

- **Establishments per (city, industry).** Sizes follow the fitted mixture: `1 + LogNormal(0.9, 1.7)` with probability 0.93, otherwise `Pareto(10, 0.9)`, capped at 4,096. It matches all nine Census size classes within about 2 points; 56% of establishments have fewer than 5 employees.
- **Dates and growth.** Each establishment has open and close dates and a growth path, which set a time-varying capacity.
- **Firms** group establishments across cities (SUSB structure), so "same firm, different office" is a real tie.
- **Names** are procedural and collision-checked (§11.3).

### 10.2 Careers: the only primitive

A person's career is a list of dated spells, built by a hazard walk from labour-force entry:
- **Kinds:** employed, unemployed, out of the labour force, self-employed, retired.
- **Job length** is `LogNormal(μ(a), σ(a))` interpolated by start age. This reproduces BLS median tenure by age and NLSY's completed-duration shares.
- **Unemployment** is lognormal with median 11.4 weeks.
- **Retirement** follows a hazard fitted to participation by age.
- **Within a job,** promotions are mostly grade changes and demotions are rare.
- **Known gaps:** early-career job counts come out too low and the share of weeks employed too high. Fix them with short teen/student jobs and out-of-labour-force spells, especially around childbirth.

### 10.3 Choosing and finding employers

- **Choosing (person side).** At spell start the person lives in region R. A region is a metro, which is a commuting zone, so every establishment in it is reachable. Their job coordinate is `π_{R,k}(unit spell index, member ordinal, job-spell ordinal)`, a static coordinate system mapping the region's residence space onto (establishment, seat).
  - The unit spell index and member ordinal are those of the person's household at spell start.
  - The effective index is the first of K candidates that passes acceptance: the establishment is open, the seat is below the establishment's capacity at spell start, and there is a soft hash acceptance on job-family fit and commute distance.
  - A job with relocation is a household move first. The job spell then starts in the new city.
- **Finding (roster side).** `roster(W, t)`: for each k and each seat of W, take the preimage and keep the person if their job spell at t has index k and coordinate (W, seat). Because the coordinate is keyed on the spell's start context (the household spell and ordinal), a person who later moves out of the household keeps the job, and the roster still finds them.
- **Guaranteed by construction:**
  - Retirees, children, the unemployed and the dead appear on no roster.
  - The "manager index doesn't exist" defect cannot recur.

### 10.4 Org chart at t

The org chart is derived from the roster at t; nothing about it is stored.
- **Layer** = min(grade, Lmax(roster size)).
- **Manager** = weighted ring successor, with lognormal arc weights, among members in the nearest populated layer above. This reproduces Gallup's span distribution (median 5–6 reports, mean 12, 13% with 25 or more). A hire or exit changes only the reporting lines in one arc.
- **Team** = a manager's reports. **Business unit** = a subtree under the head's direct reports.
- **Titles** come from (industry, job family, grade, layer). Job families are SOC; the ladder is split into IC grade and management layer.

### 10.5 Who talks to whom at work

Coworker contact and tie weights are `exp(−0.94·h)` in org-tree distance h, × 6 for the same business unit, and × 2 for the same function. Targets: median 10 sustained email contacts, about 40 distinct correspondents per quarter. §12 and §13 consume these.

---

## 11. L7 — individual attributes

### 11.1 Kept, re-keyed on the new id

These are pure functions of the id, as today:
- personality (Big Five with facets, Schwartz values, HEXACO H, attachment, dark triad, self-monitoring, CSE),
- chronotype and working hours,
- voice (formality, verbosity, register),
- languages.

They are re-keyed from the new id and conditioned where research says they should be:
- Sociability, which drives L8, is lognormal, age-dependent and correlated with extraversion.
- Languages come from heritage and region.

### 11.2 Re-derived from L2–L6

These stop being independent draws:
- current city and residence history (L4),
- education (L5),
- occupation, employer, title and income (L6),
- marital status and children (L2),
- surname (§6.3).

### 11.3 Names, and collisions with the real world

- **First names** are drawn by (sex, birth year, heritage) from SSA baby-name data (public domain); immigrants' names come from their origin country. Names change by cohort, as they do in reality.
- **Organization names** are procedural. They are checked against a list of real brand and company names; the idea already exists in pre-reset `vendors.rs`.
- **Domains** use reserved TLDs (`.example`, `.test`), so no generated identifier can resolve to a real site. This is the lesson of the Gemini/Irregular incident.
- **Hobbies** become conditioned on age, sex and personality, and feed L8 interest foci.

---

## 12. L8 — social ties

This section adopts `affiliation-and-friendship` §8, with the time model from `time-consistent-evolution` §4.

### 12.1 Foci

Ties form inside foci: the groups people share. Foci come in two kinds:
- **Dense circles** carry most degree. They are households, extended kin, teams, friend circles inside school classes and neighbourhoods, and small hobby groups. Sizes are 5–40, with within-circle density ρ ≈ 0.6–0.8.
- **Sparse contexts** add a few ties each, with `p = 1 − exp(−κ·η_g·w_a·w_b/(s−1))`. They are whole workplaces, whole classes, neighbourhoods, and former workplaces. This keeps a 3,000-person employer from giving anyone 900 ties.

The full catalogue with starting parameters is `affiliation-and-friendship` §8.2. Every focus is a group from L3–L6, or a hash partition inside one (P5).

### 12.2 Tie existence

```
tie(a, b, t):  for each group g shared by a and b over their lifetimes,
               with overlap window [a_g, b_g):
                 formed_g = hash(g, lo, hi, "tie") < p_g         // OR-rule, per group
                 alive_g  = t ≥ start_g  and  t < b_g + Lomax(hash(g, lo, hi, "life"), γ_type)
               tie exists at t iff any formed_g ∧ alive_g
```

- **Provenance.** One hash per (pair, group) records how the pair met ("met at school").
- **Survival** after leaving a focus follows Burt's power-law decay: γ = −0.466 for kin, −0.716 for non-kin, −1.842 for colleagues.
- **Kin ties** come from L2 directly.
- **Long-range ties** use geographic hierarchy buckets (neighbourhood → city → region → country). Their tie rates are chosen so expected ties per scale are about equal, which reproduces rank-based friendship. A small global ε-community supplies the geography-independent third.

### 12.3 Realism fixes built in

The naive version fails in five ways. Each has a fix, confirmed by the research simulation (n = 10⁵):

| Naive failure | Fix |
|---|---|
| Clustering too low: T ≈ 0.05 | Circles |
| Degree too narrow: CV 0.08 | Sociability weights, Poisson counts of elective memberships |
| No degree assortativity: r ≈ 0 | Stratify coordinate systems by sociability tercile and age |
| Degree explodes in big workplaces | The κ/(s−1) rule for sparse contexts |
| No homophily | Stratify foci by age, place and industry (Feld) |

With the fixes, the simulation reached transitivity 0.128, local clustering 0.14 at degree 100 (Facebook: 0.14), assortativity 0.26–0.31, and clustering that falls with degree.

### 12.4 Strength and Dunbar layers

- **Strength** = Σ over shared foci of (base × shared leisure hours × affinity × decay), plus kin terms. Hall's thresholds of 50 / 90 / 200 hours with work hours discounted mark casual friend / friend / close friend.
- **Layers** are **absolute** strength thresholds, calibrated to population means of 4 / 11 / 30 / 130. Absolute thresholds keep layers symmetric and lognormal across people; per-person top-k would not.
- **Contact rates** per layer (weekly, monthly, yearly) feed `graph::comm_intensity`.

### 12.5 Neighbour enumeration

`neighbours(x, t)` is the union over x's current and past foci of the co-members that pass `tie`, plus kin. Large sparse contexts are pre-bucketed (√s buckets) so no query scans a 3,000-person roster. Results are memoized per request.

---

## 13. L9–L10 — happenings, services, rendering

This section adopts `happenings-and-content-diversity` §6–7.

### 13.1 Happenings

**The record.** A happening is a pure function of (kind, anchor, epoch, index). Its fields are:
- `kind`
- `anchor` (person, pair, venue or account)
- `motive`
- `cast` (bound to roles by graph queries)
- `object`
- `parent`
- `times`
- `labels`

**Three generator families:**
1. **Cadence:** 1:1s, standups, bills.
2. **Poisson roots per fixed epoch:** week, month or quarter by kind, so slices recombine.
3. **Spawned children**, for example a meeting's action items becoming tasks and a follow-up email.

**Where the catalogue and rates come from:**
- the ATUS activity lexicon for life happenings;
- work calendar statistics: 48% of meetings recurring, 49% of one-offs with 2 people, about 3.8 action items per meeting;
- 52% of work emails carrying a request.

**Casting** weights candidates by tie strength, traits and role distance.

**Variety comes from the facts.** The research found that more LLM sampling doesn't help (same-model outputs are over 0.8 similar 79% of the time). So variety comes from four sources:
- different causes and casts;
- a per-person pacing director with busy and quiet phases;
- keyed-permutation shuffle bags, so no object repeats before its pool is exhausted;
- labelled history, so "per our call Tuesday" is true.

**Vocabulary** comes from O*NET task statements keyed by the SOC codes people already carry, not from hand-written tables.

### 13.2 Services for the pilot

Each service is a view of the log:

| Service | What it shows |
|---|---|
| **directory** | people, org chart, contacts |
| **mail** | communicative acts: requests, replies, status, invites, broadcasts |
| **calendar** | scheduled happenings, with cancellations and reschedules |
| **tasks** | action items, requests and errands; about 40% never completed |
Tool names and parameter schemas follow the existing surface where it still fits. Additions (email and date parameters beside ids) are allowed; renames are not. Chat, files and money come after the pilot (§17 D7). Ad hoc calls and quick coordination, which chat would carry, show up as short mail or not at all until then.

### 13.3 Rendering

- **Pipeline:** placeholder prose from deepseek-flash (thinking off), then a closed-world validator (no stray names, digits, dates or history claims), then retries, then a template fallback.
- **Cache:** results are cached by (namespace, prompt version, model version, act id). The cache ships with published seeds, because LLM output isn't reproducible.
- **Faithfulness:** a reply's facts come from the parent's record, never from the parent's prose. Chaining prose is how OrgForge spread invented facts.
- **Cost:** about $0.0002 per email; about $1–3 for the pilot corpus.

### 13.4 Content test suites

Thresholds come from the research note, calibrated on 1,000 real Enron emails:
- **A. Happening diversity:** kind entropy, top-kind share, cooldown with zero exact repeats in 8 weeks.
- **B. Calibration against real marginals:** meeting hours, recurring share, request share, thread length, reply latency, and so on.
- **C. Cross-service coherence:** zero dangling references.
- **D. Prose diversity:** gzip compression ratio ≤ 2.9 (Enron 2.45, templated 13.2), self-BLEU ≤ 0.30, masked-opener top-1 share ≤ 3%, POS template rate ≤ 0.65, LLM-tic lexicon.
- **E. Faithfulness:** zero slot errors, zero leaks, ≤ 2% template fallback.

---

## 14. Consistency guarantees as property tests

These live in `internot/tests/`. Each runs on sampled ids at sampled t, with caches cleared between runs.

| Test | Asserts |
|---|---|
| Duality | For each relation R ∈ {partner, mother/child, father/child, household member, neighbourhood resident, class member, employee, manager/report, tie}: `y ∈ R(x,t) ⇔ x ∈ R⁻¹(y,t)` |
| Uniqueness | At every t an alive person has exactly one household, one residence, at most one union and at most one employer (unless multiple jobs are modelled explicitly). Rosters list each person once. |
| Life bounds | Nothing happens outside [b(x), d(x)). No job before 16. No union before 18. Mothers aged 12–50 at birth. School grade matches age. |
| Kin rules | No partner is a sibling, half-sibling, parent or child. Partners are mutual. |
| Recombination | For every stream kind, `events([a,b)) ⊎ events([b,c)) == events([a,c))`, bit for bit, including non-day-aligned boundaries. |
| Timeline/state | State at t equals the state rebuilt from the event list before t. |
| Joint events | Union dates, move dates and destinations, and reorg permutations are identical from every participant. |
| Determinism | Identical results across query orders, threads, cleared caches and different `Universe.now`. |
| Ledger closure | The sampled population pyramid equals Σ N·l from the ledger, exactly. Every quota margin sums to its parent. |
| Primitive laws | Bijectivity (exhaustive for n ≤ 2000); σ(σ(x)) = x; slice counts equal the plan; partitions tile. |

---

## 15. Realism targets

Each target becomes a tolerance-banded test in `internot/tests/realism_*.rs`. Sample sizes come from `affiliation-and-friendship` §6; for example, about 38k wedges estimate transitivity to ±0.01 at 99.9%. The full tables with sources are in the research notes:

| Area | Examples | Table |
|---|---|---|
| Population | e₀ 78–79; survival to 65 / 85 = 84% / 44%; TFR 1.6; pyramid vs Census | `kinship-and-households` §8 rows 4–12, 31 |
| Kinship | P(mother alive) at 30 / 50 / 60 = 0.92 / 0.63 / 0.33; living grandparents at 0 / 30 = 3.4 / 1.0; ~9% only children | same, rows 6–9 |
| Unions | median first marriage 30.5 / 28.5; never married at 45–54 = 18.5% / 13.7%; first marriage intact at 10 / 20 years = 0.69 / 0.54; age gaps | same, rows 15–23 |
| Households | size shares; one-person 29.5%; 18–24 living at home 57%; 65+ alone 28% | same, rows 24–30 |
| Mobility | 7.8% moved last year; peak 18.9% at 25–29; 54 / 23 / 18% same county / state / interstate; distance histogram | `time-consistent-evolution` §7 |
| Work | establishment and job shares by size class; tenure by age; 61% of jobs started at 18–24 end within a year; spans; layers | `work-and-organizations` §7 |
| Ties | degree 125–155, CV 0.6–0.8; transitivity 0.10–0.15; assortativity 0.1–0.3; layers 4 / 11 / 30 / 130; 62.8% of ties within 100 miles; path length 4.3–4.7 | `affiliation-and-friendship` §7 |
| Tie dynamics | colleague ties re-cited after 1 / 2 / 3 years = 24.7 / 10.1 / 8.0%; 48% of a network still present after 7 years | `time-consistent-evolution` §7 |
| Content | tests A–E | `happenings-and-content-diversity` §6 |

**Calibration rule (the Synthea lesson).** A model output that contradicts a public marginal is a bug. Synthea shipped a diabetes amputation rate 4,000× too high because nobody checked. No layer ships without its realism tests passing.

---

## 16. Performance: budgets and profiling

Speed is a founder requirement: the world must provably scale. Every number in this section is a **gate**, not an estimate. Until a benchmark measures it on the reference machine, it is a hypothesis. A budget miss means a design change, or an exception the founder explicitly approves. A budget is never raised silently.

### 16.1 What "scale" means here

Four axes, each measured separately:
1. **Population.** All ~1.1·10⁹ ids and the full US-scale ledger. Queries sample the whole id space and the whole 1840–2150 range, not a convenient corner.
2. **Fan-out.** Queries whose cost grows with group size: rosters, neighbours, inboxes. These are measured on the largest real instances, not the median.
3. **Concurrency.** Many eval sessions per machine. Throughput must scale with cores, with no global lock contention.
4. **Memory.** Caches are bounded. A long run must not grow without limit.

### 16.2 Budgets

Percentiles are over uniformly sampled inputs plus the worst-case set (§16.4). "Cold" means empty caches; "warm" means a memo hit.

| Query | p99 cold | p99 warm | Basis |
|---|---|---|---|
| decode, birth, death, alive_at | < 1 µs | — | prefix-sum search plus a few hashes |
| partner_of, mother_of, children_of (one hop) | < 5 µs | < 1 µs | 2–4 permutations plus quota lookups |
| cousins, in-laws (multi-hop kin) | < 50 µs | < 5 µs | composed hops |
| career, residence history | < 20 µs | < 2 µs | hazard walks, ≤ ~30 events |
| roster, median establishment (~90) | < 20 ms | < 100 µs | K × seats preimages, filtered |
| roster, largest establishment (4,096) | < 500 ms | < 1 ms | same, worst case |
| roster(neighbourhood), class roster | < 1 s | < 1 ms | time-blocked spell spaces |
| neighbours(x, t) | < 50 ms | < 1 ms | bucketed large contexts |
| a viewer's happenings, 30-day window | < 100 ms | < 5 ms | epoch-enumerated roots plus casting |
| a view call (inbox, schedule, person) | < 1 s | < 200 ms | end to end through MCP |
| ledger build | < 30 s, < 100 MB | — | once per seed and parameter set |

**Also gated:**
- **Throughput:** warm view calls scale near-linearly from 1 to 16 threads, at ≥ 80% efficiency.
- **Memory:** a process stays under a configurable cache cap (default 1 GB) through a 10⁶-query soak test.
- **Allocations:** hot-path primitives make zero heap allocations (Feistel, quota lookup, hashing, event draws).

### 16.3 Tools and method

- **Microbenchmarks:** `criterion` for every §4 primitive and every layer query. `procedural_core/benches/` already exists, so it gets extended rather than created.
- **Distribution harness:** a benchmark driver that samples ids uniformly over the full id space and t over 1840–2150 (weighted toward 1990–2030). It reports p50, p99, p99.9 and max, plus the slowest inputs so they can be replayed.
- **Profilers:**
  - `perf` with flamegraphs (`cargo flamegraph` / inferno) for CPU;
  - `perf stat` for cache misses and branch mispredicts on the hot primitives;
  - `dhat` or `heaptrack` for allocations, plus a counting-allocator test that fails on hot-path allocations.
- **Tracing:** per-layer `tracing` spans. Every view call can print its cost breakdown by layer.
- **End to end:** a Rust driver replays each pilot scenario's tool calls against `internot-mcp` over stdio and reports per-tool latency.
- **Concurrency:** 1-to-64-thread runs of the cache layer. If contention shows up, the current `parking_lot::RwLock` global maps become sharded or lock-free maps.
- **Baselines and regressions:** baselines are committed under `perf/` with the reference machine's spec. A check fails when a benchmark regresses more than 10% or breaks a §16.2 budget.

### 16.4 Worst cases, and full-scale runs

**Worst-case inputs** are generated deliberately and kept as a fixed set:
- the largest establishments, households and neighbourhoods;
- the people with the most events (many jobs, moves and unions);
- the deepest kin recursions (long remarriage chains, large sibships);
- the people with the most memberships;
- the busiest viewers (managers of large teams).

**Full-scale runs:**
- **Sweep:** at least 10⁶ uniformly sampled ids derived through every layer, looking for panics, invariant violations and slow outliers.
- **Ledger:** built at full US scale.
- **Soak:** 10⁶ mixed queries, watching memory and p99 drift.
- **Determinism under load:** 1 and 64 threads must give bit-identical answers.

### 16.5 Hot spots the research already points at

| Hot spot | What to watch or try |
|---|---|
| Roster enumeration (K × seats × person evaluations) | the K and acceptance tuning |
| Time dilution in neighbourhood rosters | entry-decade blocking |
| `neighbours()` fan-out in large contexts | √s bucketing |
| Kin recursion through remarriage pools | depth and memoization |
| Casting in happenings | tie queries per happening |
| Keys built with `format!` | 23 ns per draw, measured; replaced by structured keys |
| Global cache locks | contention under concurrency |

Each of these gets a dedicated benchmark before its layer is called done.

---

## 17. Founder decisions

| # | Decision | Status | Why |
|---|---|---|---|
| **D1** | **What an id means.** A dense index over everyone ever born 1840–2150, in (birth region, birth year) blocks. City, workplace and household become relations that change over time. Every fact is still `f(seed, id, t)`; only what the id's bits encode changes. | **Approved 2026-09-29** | Kinship, turnover, migration and exact rosters all require it. |
| **D2** | **The demographic ledger.** One set of integer count tables computed once from the seed, including moves between regions. | **Approved 2026-09-29** (full ledger, with migration) | It is the only way the two sides of families and migration can agree exactly. It never touches individuals. |
| **D3** | **Scope of detail.** Neighbourhoods, workplaces and schools everywhere. Realism validated first on 1–2 metros. | **Approved 2026-09-29** (revised from the first draft's "detailed metros only") | Everything is lazy, so detail costs nothing until queried, and there is no second class of person. |
| **D4** | **Country.** US parameters in v1. Other countries are parameter sets over the same mechanics; the existing 64-country names become immigrant origins. | **Approved 2026-09-29** | The research data is mostly US. |
| **D5** | **Minors** become full people; views hide them where appropriate (no mail under 13, etc.). | **Approved 2026-09-29** | A stub can't grow up into the same adult. |
| **D6** | **How `people` is replaced.** Build the new layers alongside the old code. The old slot layout, family, cohort and Tier-3 stub code and their ~600 tests are deleted once the new layers pass their property, realism and performance gates. Individual-attribute code is kept and re-keyed. | **Approved 2026-09-29** (build alongside, then cut over) | Nothing breaks during the transition. |
| **D7** | **Services for the pilot:** directory, mail, calendar, tasks. Chat, files and money come after the pilot. The pilot's chat-based quiet-hours scenario is replaced by a calendar version (coherence spec §6). | **Approved 2026-09-29** (without chat) | Four services cover the pilot; less to build and render first. |
| **D8** | **Extreme profiling.** Speed is a requirement. Every budget in §16 is a measured gate at p99 over the full id space and worst cases, including concurrency and memory. | **Founder requirement, 2026-09-29** | "Make sure this can really scale." |

---

## 18. Risks, and what gets prototyped before building

Each item gets a throwaway prototype with a pass/fail criterion before its layer is built. Prototypes whose question is speed (R1, R3, R5) are written in Rust and report timings through the §16 harness, because a Python timing says nothing about the real cost. The research prototypes already cleared some risks:
- bijection speed and quality;
- partner reciprocity;
- the interchange process;
- the job-spell model;
- the affiliation calibration.

| # | Risk | Prototype and pass criterion |
|---|---|---|
| R1 | The multiregional ledger with union-status states is novel as a closed-form individual assignment. | Two regions plus abroad, 1840–2100, 10⁵-scale blocks. Exact duality on every relation; the pyramid matches the ledger exactly; interstate share of moves 17.5% ± 2 pp. |
| R2 | Nesting a leaving-home person's trajectory under their parents' region is intricate. | Enumerate one block fully and check that every member's region at leaving home equals their mother's household region at that date. |
| R3 | Roster cost under time-blocked spell spaces. | A median-size establishment (about 90 people) at < 50 ms cold and a neighbourhood at < 1 s. If it fails, fall back to memoized per-block spell tables (`work-and-organizations` §8.5). |
| R4 | The within-region distance weighting may not produce CPS-like move distances. | 40% of moves between counties under 50 miles, ± 5 pp. |
| R5 | Tie calibration must hit several targets at once. | The §15 tie row on 10⁴ egos, starting from the research simulation's final configuration. |
| R6 | Rendering faithfulness at the budget. | 200 renders per prompt version: zero slot errors, ≤ 2% fallback, suite D passes, total ≤ $0.50. |

---

## 19. Phases and definition of done

A phase is done when three sets of gates pass:
- its property tests (§14),
- its realism tests (§15),
- its performance gates (§16): budgets met at p99 on the reference machine, including its worst cases, with flamegraphs of its three most expensive queries committed under `perf/`.

Nothing is built on a layer that isn't done.

| Phase | Scope | Done when | API cost |
|---|---|---|---|
| 0 | Benchmark and profiling harness (§16.3) **first**; then primitives (§4) and the `enumerate_events` fix | harness runs in CI; primitive laws; recombination; primitive budgets and zero hot-path allocations | $0 |
| 1 | Ledger, ids, population, kinship, households (prototypes R1–R2 first) | duality, life bounds, kin rules, ledger closure; population, kinship, union and household targets | $0 |
| 2 | Residence and education (R3–R4) | residence and class duality; mobility targets | $0 |
| 3 | Work | roster duality, org-chart invariants; work targets | $0 |
| 4 | Ties (R5) | tie symmetry; tie and tie-dynamics targets | $0 |
| 5 | Happenings, pilot services, renderer (R6) | suites A–E; round-trip test per mutating tool | ≈ $1–3 |
| 6 | Coherence pilot: perturbation layer, natural-language prompts, DeepSeek runs | per the experiment spec | ≈ $5 |

**Code changes.** New modules under `internot/src/` (§3); `people` keeps only individual attributes. The transport is unchanged. `procedural_core` gains §4.

**Invariant changes to record in AGENTS.md once approved:**
- Invariant 3 is strengthened by P9.
- P1–P8 are added.
- The people-slot description is replaced.

---

## 20. What this deliberately does not do

- It doesn't simulate people step by step, and doesn't use LLM agents to plan. Every fact is a closed-form function.
- It doesn't use stable matching. It is provably non-local; the ledger's union tables with rank-range matching replace it.
- It doesn't aim for exact independent-edge random graphs (G(n,p), Chung–Lu). Memory-less local access to them is an open research problem. Ties come from foci, which are locally exact.
- It doesn't do per-epoch reshuffling for anything except rare bulk events such as a team reorganisation.
