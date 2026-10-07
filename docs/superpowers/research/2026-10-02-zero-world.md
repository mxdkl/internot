# The cell world: a table-free `f(seed, id, t)`

**Date:** 2026-10-02.

**Founder:** "forget the current implementation all together. be radical, invent new math if you need", after the transport world (`2026-10-02-transport-world.md`) still kept a 6.7 MB census.

**Status:** prototype `internot_society::zero`.
- Natives, re-partnering, kin repair, immigrants (children, couples, singles, births to immigrant mothers) and same-sex unions.
- The math, written before each piece of code: `2026-10-02-cell-world-math.md`. Lean proofs: `proofs/2026-10-02-cell-world.lean` and `proofs/2026-10-02-cell-world-immigrants.lean`.
- Exhaustive tests: `tests/zero.rs` (`ZERO_SEEDS=…`), over every stream of every cohort.
- Report: `examples/zero_report.rs`. Speed checks: `examples/zero_checksum.rs` (bit-identical answers) and `internot_perf/examples/zero_profile.rs` (`WHAT=…`, `TAIL=1`).

## 1. What is stored

**Nothing about the world's people, couples or history.** A `Zero` holds the pack, a key, and numbers derived from the pack in about 20 ms (none of them depend on population):
- each queue's knots and running minima, and its integer cumulatives per birth year (demand, used supply, pool room);
- four yearly arrival flows with exact integer prefix sums, and band weights;
- one small record per native cohort (sizes, women, died young, wives, pool starts);
- the yearly share of births to immigrant mothers;
- two survival splines.

That is about 10k numbers, the same kind of thing as the pack's own anchors.

The pack needs one new input: births and net arrivals by year (real US series). They are currently constants in the module, scaled to the pack's founders.

## 2. The ideas

1. **Births are data, not renewal.**
   - A year's births are one rounding of a closed-form series.
   - Ids are (birth year, entry year, index), so nothing needs a global running sum.
   - Each child **chooses** its mother: the j-th child of block (kind, mother's age) goes to eligible woman `⌊j·M/c⌋`. That is exact both ways, and never infeasible (more children than mothers means twins).
   - The block cumulative is closed form (a non-union share × an algebraic-sigmoid age curve), so a child's block is found by direct inversion.

2. **Union age is monotone in the married rank.** It is the log-logistic quantile of the rank. So "married before age a" is a prefix and "not yet married" a suffix, both closed form.

3. **Marriage is a queue, solved in closed form.**
   - Demand: one side, ranked by birth year. Supply: the other side.
   - Supply used before y is `S(y) + min_{s≤y} (D(s+gap) − S(s))`, a Skorokhod reflection.
   - So no cohort gives more than it has, and unmet demand waits for later supply (the marriage squeeze: older or younger partners).
   - Densities are piecewise linear (splines), so cumulatives are closed form. The running minimum is kept at the knots, and inside a piece it is a quadratic's minimum, O(1).
   - Both sides round with one offset: the k-th demand meets the k-th supply. A block shuffle spreads the age gap.

4. **Every couple has at most one partner with a history.** Seven queues:
   - Q1: first-time women with first-time men;
   - Rw1–3: women's 1st to 3rd re-partnerings, with men in their first union;
   - Rm1–3: the same for men.
   
   First-time partners come from **pools**: a closed-form share of each cohort's adults, sized from the queue's own supply, so a pool always holds what its queue uses. Every chain is then linear in union order.

5. **Divorce is bottom-up, re-partnering top-down.**
   - Separation is a Beatty category of a couple's rank in its group (the demand cohort), and its duration a quantile of the separated rank.
   - A group's re-partnering count comes from the queue's spline (top-down), and is spread over the group's separated candidates by a rational Beatty sequence with exactly that many members.
   - Candidates are counted from first unions alone, so the count always fits. An event beyond the candidates (tiny cells only) is a phantom whose first-time partner's couple is void.

6. **No cycles.**
   - Every dependency stays inside one group.
   - In each group only one of two edges is allowed, alternating by the group's parity: "a woman's first union with a re-partnering man feeds her re-partnering" or "a man's first union with a re-partnering woman feeds his".
   - Without that, two couples could each be the other's source (found by a stack overflow).

7. **Kin repair decides on repair-free facts.**
   - Couples pair as `(k, k ⊕ 1)` within a demand group, and the pair's supply partners are exchanged when that parts a possibly-close-kin couple.
   - "Possibly close kin" reads only:
     - mothers (exact);
     - candidate fathers (the candidate husbands of each conception couple, over both couples a supply-side mother could be in);
     - the demand sides' candidates (at most two).
   - So a repair decision never resolves another. Lemma K: actual close kin are always flagged.
   - Three earlier versions recursed (actual fathers; a supply-side mother's couple; a supply-side demand person), and a pair straddling two groups looped.

8. **Immigrants without breaking presence.** A couple's start comes from its demand side, so arrivals never sit in pools.
   - Children (1–14) join their birth cohort.
   - Couples arrive together, with the husband attached to the wife.
   - Single adults are the demand side of their own two queues, partnering first-time natives after arrival plus one of 8 keyed delays.
   - Arrival counts are exact integers: "arrived before year e" is `⌊(2TΦ(e) + Φ∞)/(2Φ∞)⌋` (Lemma P).
   - A share φ(y) of each year's births goes to immigrant mothers, by the same block maps. Empty blocks spill to the other kind, then to the next age with mothers.

9. **Fathers are the partner at conception.**
   - The first couple's window opens at its adult bound for births to married mothers, and at its start for births to women in the single range.
   - Births before any union have no in-world father.

10. **Deaths depend on the first union only.**
   - Deaths are conditioned on the first union's start and births (a mother's every birth; a father's conceptions in his first couple).
   - A re-partnering whose partner is already dead is void for both, who both continue through it.
   - A father is always alive at conceptions in his first couple, so no death lookup is needed there.

## 3. Results

**Exhaustive on the tiny world:** 30 seeds (3, 11, 100–127), about 142k people each (131.7k natives and child arrivals, about 10k adult arrivals). Every stream of every cohort is checked:
- unions mutual (same start and separation), ordered after the last ended, partners alive, opposite sex;
- **no close kin in any union** (shared mother, shared father, parent and child), against actual parents;
- mothers and fathers dual with children;
- mothers aged 15–45, alive and arrived at births;
- fathers alive at conception, and arrived by the birth;
- births before deaths;
- arrivals alive on arrival, adults (or children of 1–14 joining their cohort), with no parents here;
- couples arrive together, partnered abroad;
- everyone else partners after arriving.

Per seed: about 38.3k unions (15% re-partnerings), 202k parent links, 7.3k of them children of immigrant mothers.

**Prototype** (`us`): all nine queues end exactly balanced, and the tightest re-partnering group is at 1.000 of its candidates. Uncalibrated realism:

| Measure | 1870/1880 | 1910 | 1950 | 1970 | 2000 | 2020 |
|---|---|---|---|---|---|---|
| foreign-born share of the living (Census) | 10.5% (14.4) | 14.9% (14.7) | 7.0% (6.9) | 4.8% (4.7) | 8.0% (11.1) | 10.1% (13.7) |
| births to immigrant mothers (US: ~20% 1910, ~6% 1970, ~23% 2020) | 11.7% | 10.0% | 1.2% | 2.7% | 9.0% | 6.6% |

Births to immigrant mothers are about half the real share: `φ` assumes native fertility, and the foreign-born share is low after 1990.

**Speed** (quiet machine, everything included):

| Lookup | p50 | p99 |
|---|---|---|
| mother | 0.18 µs | 1.2 µs |
| unions | 3.2 µs | 106 µs |
| father | 4.5 µs | 31 µs |
| death | 4.2 µs | 32 µs |
| children | 0.3 µs | 94 µs |

Kin repair made lookups about 6× slower at first. These changes brought them back, each proven bit-identical by `zero_checksum`:
- per-year queue cumulatives;
- kin facts once per person;
- each couple's two default partners computed once;
- demand candidates in place of resolved demand persons;
- per-cohort married-before counts;
- per-year birth-block starts.

The build takes 7 ms.

**Heritage groups** (math §16–16.1, 2026-10-02): five group spaces from the pack; the open market is next. Births split by `β_g(y)` (each group's expected women of 15–45 times its fertility factor, in year order), founders and arrivals by the pack's mixes, mortality by group factors.

Thin groups needed three new lemmas:
- **C:** layouts fit their cohort's adults, so counts never overflow;
- **V:** couples a short queue can't reach are void on both sides;
- **R:** re-partnering and single immigrant partners take at most 75% of a cohort's ever-partnered first-timers.

All three are proven in Lean where they're arithmetic (`proofs/2026-10-02-cell-world-groups.lean`). The exhaustive test passes on 46 grouped tiny seeds.

| White/Black/AIAN/Asian/Hispanic, % of the living | 1900 | 1970 | 2000 | 2020 |
|---|---|---|---|---|
| cell world | 88.2/9.7/0.5/0.4/1.2 | 82.5/11.3/0.5/0.6/5.1 | 72.6/13.0/0.5/2.9/11.0 | 66.4/13.5/0.5/4.6/15.0 |
| Census | 87/12/0.3/0.2/0.7 | 83/11/0.4/0.8/4.5 | 69/12/0.7/3.8/12.5 | 58/12/0.7/6/19 (+4% multiracial) |

**Open market** (math §16.2–16.6): a separate union space where each group sends its open share of first unions, re-partnerings and single immigrants. Newlyweds across groups, 2010–15 (natives): 12.9% overall, against Pew's 17%. By group (model, Pew): White 8 vs 11, Black 19 vs 18, AIAN 55 vs 58, Asian 39 vs 29 overall, Hispanic 29 vs 27.

**Integer layouts** (§16.3, Lemma C′): each cohort's adults split into exact integer segments that are the queues' densities, so layouts fit with no reserve.

**Areas** (§17–17.5): each area is its own world of group spaces and an open market (sub-spaces). Labelled block matching inside national queues (Lemma M, proven) was built first, but was 20–60× slower at 62 areas.
- **Migration before a first union** (§17.4): natives keyed by their life cell, exact mover streams, destinations by an exact interleave.
- **At 62 areas** (`us-areas`): build 0.68 s; about 185 MB beyond the pack; unions 6.9/291 µs, father 10/65 µs (p50/p99).
- **Natives living outside their birth area** (Census, birth state): 1900 30.5% (21), 1960 33.4% (30), 2020 25.1% (34).
- **Exact** on 36 four-area seeds and 3 two-area seeds.

## 4. Rules and debts (named, to replace)

- Chains deeper than 4 stop: up to 4 re-partnerings after a first union at depth 0 (math §12; the parity rule and the 3-level cap are gone).
- Couples where both partners re-partner are void, not repaired, when possibly close kin (math §13).
- Only early widowhood (before the wife's cohort turns 60) of first unions with a repair-free wife feeds re-partnering (math §14). Late widowhood and widowhood in later unions don't.
- First-union wives are capped at 90% of adults less the brides' pools. This only binds in cohorts with very high ever-partnered shares.
- Survival is a three-year blend of Siler curves per cohort, not the cohort's own table.
- Single arrivals partner only natives; immigrant-to-immigrant unions come only from couples arriving together.
- Delays from arrival to a first union take 8 values per cohort.
- A child arrival has no parents here only when no couple's wife of a fitting age arrives that year (about 2% on the tiny world).
- No re-partnering after a same-sex union (as in the ledger and the pure world).
- Same-sex couples are 2.5% of couples together in 2019, against ACS ~1.5%: the pack's share is provisional.
- Groups: partners within the group only, until the open market (space `O`) is built. Single immigrants partner natives of their group; same-sex unions stay within groups; children take the mother's group.
- Lemma R's `θ = 0.75` caps the share of a cohort's first-timers who partner re-partnering or immigrant partners (binds for Asian 1940–1980). Immigrants partnering each other would replace it.
- Areas v1: partners are always from the same life area; cross-birth-area couples come only from moves before a first union. One move per person, destinations of rank 1 (no distance decay), no couple moves; the era trend of moves runs opposite the Census after 1960.
- Calibration not yet compared against today's world.

## Lookup speed pass (2026-10-03; math §18, proofs `proofs/2026-10-03-cell-world-speed.lean`)

Every step was checked bit-identical on six worlds and by the answer hash of `examples/zero_timing.rs`. The exhaustive tests pass afterwards on the default world (seeds 3, 11, 17, 100–111) and the four-area world (3, 11, 20–33).

**What changed:**
- **Lemma K:** fathers are resolved only when the mothers' chain keys agree.
- **A per-thread memo** of pure functions, so each couple is resolved once per lookup.
- **Immigrant mothers:** birth windows start at arrival, last births are found by reverse scans, cohort constants are computed once, and emptiness takes three counts.
- **Shuffles:** precomputed shapes, and the plain staggered blocks for one label.
- **Repair:** a woman's death runs none (§18.5); one pair decision settles both couples; couples carry their year.
- **Lemma D:** a re-partnering's first-timer outlives its start, so that partner's death is read only to clamp a separation.

| `us`, cold, p50/p99 µs | death | father | unions | children | mother |
|---|---|---|---|---|---|
| before | 6.6/62 | 7.2/52 | 6.9/277 | 0.81/209 | 0.23/2.4 |
| after | 1.4/14 | 2.0/9.1 | 1.5/26 | 0.75/24 | 0.13/1.5 |

At 62 areas: death 11.7/87 → 3.0/26, father 11.5/71 → 4.5/19, unions 11.7/306 → 3.3/42, children 2.1/235 → 1.8/38.

**The floor of this design:** about four mothers per repaired couple, plus the shuffles' Feistel networks and key hashes. Beyond it, either change the world (fewer Feistel rounds, cheaper keys) or store a kin list from a one-time scan of all repair pairs (removes repair from lookups, but the scan grows with population; a founder decision).
