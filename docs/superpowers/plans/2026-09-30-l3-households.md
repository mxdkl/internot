# L3 households: who lives with whom, as a function of (seed, id, t)

**Date:** 2026-09-30
**Spec:** `specs/2026-09-29-society-as-a-function.md` §7 (L3), §14, §15. It completes Phase 1.
**Research:**
- `research/2026-09-29-kinship-and-households.md` §6, §8 rows 24 to 30, §9.6;
- `research/2026-09-30-household-targets.md` (history back to 1850, roommates, custody);
- `research/2026-09-30-residence-enumeration-problem.md` (option B: no region trajectories, so households need none).

**Crate:** `internot_society`, new module `household.rs`. Households are a view over the existing world, so the ledger doesn't change.

## 0. What must hold

1. **Reciprocity.** `x ∈ members(household(x, t), t)`, and every member `m` of `h` has `household(m, t) = h`. So everyone present at `t` is in exactly one household.
2. **Pure function.** `household(x, t)` depends only on `(seed, x, t)` through world lookups. Nothing is stored per person.
3. **Time consistency.** A household changes only at an event:
   - leaving home;
   - a union starting or ending;
   - a death;
   - an elder moving in;
   - a roommate epoch boundary.

   Decisions that persist (an elder moving in) use one keyed uniform per person against a curve that is nondecreasing along the person's life, so they don't flip back and forth.
4. **Sense.** Minors never live alone. Parents and their minor children live together unless custody separates them. Partners in a union live together.
5. **Realism** against the targets in the household note.
6. **Speed** that stays out of the way of workloads (founder decision, 2026-09-30). Budgets are set from measurement.

## 1. Household identities

```
Household = Union { a, b, start }                 // a < b, the partners; start of the union
          | Solo { person, spell }                // a single adult's own home
          | Roommates { region, band, epoch, frame, group }
```

- **Union.** Both partners and the start date. The start is needed because two people can divorce and later remarry each other.
- **Solo.** `spell` counts the person's unions that ended before `t` (0, 1 or 2), so each single stretch of life is its own household. A Solo household holds the adult plus anyone who lives with them: minor children with custody, a never-left adult child, an elder parent, an orphaned grandchild.
- **Roommates.** Identified by the frame that formed it (§5). Its members are single, childless adults.
- **No dorms.** They need colleges (L5). CPS counts dorm students as living with their parents, so the targets don't need them either.

## 2. Rules, per person, in order

`household(x, t)` for a person present at `t` (born, alive, and arrived if an immigrant):

1. **Dependent.** If `x` lives at home at `t` (§3), return `household(P, t)` for the parent or guardian `P`.
2. **Elder with a child.** If `x`'s elder unit co-resides with a host child `C` at `t` (§4), return `household(C, t)`.
3. **In a union.** Return `Union` of the union active at `t`.
4. **Single.** If `x` is in a roommate group at `t` (§5), return it. Otherwise return `Solo { x, spell }`.

**No cycles.**
- Rule 1 always moves to an older person who is responsible for `x`.
- Rule 2 moves to a younger person who has left home. Such a person never uses rule 1, only rule 2 again (towards their own child) or a terminal rule.
- So every chain is some rule-1 steps, then some rule-2 steps, then a terminal rule, and it always ends.

## 3. Living at home

- **Leaving age.** `t_leave(x) = min(birth + A_x, start of x's first union)`.
  - `A_x` is a keyed draw from an independence schedule by sex and birth cohort (§7).
  - Unions always end living at home, so married and cohabiting people never live with their parents.
- **Custodial parent `P(x, t)`.**
  - **Primary parent:**
    - the mother by default;
    - the father, if the parents' union has ended in divorce before `t` and the union's custody draw went to him (probability 0.25, keyed on the union, so siblings stay together);
    - for a child with no in-world father (a non-union birth, or conceived after his death), always the mother.
  - If the primary parent is dead at `t`, the other parent, if alive.
- **At home** at `t`: `t < t_leave(x)` and `P(x, t)` exists.
- **No living parent:**
  - **Adult (18+):** they become independent when the last parent dies.
  - **Minor (an orphan):** they live with a guardian: the first living grandparent (maternal grandmother, maternal grandfather, paternal grandmother, paternal grandfather). Failing that, the eldest living sibling who is 18+ and has left home. If none of these exists, `Solo` as a measured residual.
- **People with no in-world parents** (founders, single immigrants) count as having left home.
  - Founders who were minors at `y0` therefore live alone until 18. This affects 1840 to 1857 only and is reported.

## 4. Elders with a child

- **Elder unit:** a single elder, or a couple. A couple decides together: the woman's draw (the left partner's, for same-sex couples), using the older partner's age.
- **Hosts:**
  - A host is a child who is alive, has left home and is 35+ at `t`.
  - The age floor keeps hosts and roommates disjoint (§5), which removes a circular dependency.
  - **Preference order:** eldest daughter, then eldest son. For a couple, the woman's children come first, then the man's.
- **Decision.** The unit co-resides with its host at `t` if it has one and `u_x < q(age, cohort, status)`.
  - `u_x` is one keyed uniform per person.
  - `q` is nondecreasing in age, and higher for a single elder than for a couple at the same age. So widowhood can only add co-residence, and nobody moves back out except when the host dies or no host remains.
- **Era.** `q` falls across birth cohorts. Ruggles' anchor is about 70% of people 65+ living with adult children in 1850 and fewer than 15% by 1990–2000. Children who never left (§3) count towards that share, and the calibration includes them.
- **Stem families.** A married child who stays in the parents' house (common in the 19th century) appears as the elders living with that child. The same people share one roof; only the householder differs.

## 5. Roommates (the risky part)

Grouping must be reciprocal: every member has to compute the same group. It is done over a **static frame**, not over the eligible set, which changes all the time.

1. **Bands.**
   - A band is 3 consecutive birth years within one lineage region.
   - Its people are the concatenated id ranges of its 3 blocks: a dense index space of size `N`.
2. **Epochs.**
   - An epoch is 2 years, with a keyed phase per band so that bands don't all switch on the same day.
   - Epoch `e` of band `k` covers `[t0 + 2e years, t0 + 2(e+1) years)`.
3. **Frames.**
   - Each (region, band, epoch) has a keyed permutation `π` of `[0, N)`, a `CompactPerm` built per call at no storage cost.
   - Frame `f` is positions `[fK, (f+1)K)`, with `K` = 12 (8 at first; raised in calibration so that groups form more often).
4. **Groups.**
   - **Who is eligible:** a frame member is eligible at the epoch start if they are:
     - present, aged 18 to 34;
     - left home, not in a union, never had a child;
     - and their keyed uptake draw for that epoch is under `w(age, era)`.
   - **Chunking:** eligible members in slot order are chunked by count:

     | Eligible | 0 to 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 |
     |---|---|---|---|---|---|---|---|---|---|---|---|---|
     | Groups | none | 2 | 3 | 2+2 | 3+2 | 3+3 | 3+2+2 | 3+3+2 | 3+3+3 | 3+3+2+2 | 3+3+3+2 | 3+3+3+3 |

   - The chunks are fixed for the epoch.
5. **At time `t`:**
   - The group's members are its chunk members who are still eligible at `t` (alive, still single and childless; the uptake draw is per epoch).
   - With 2 or more left, they are a `Roommates` household; with 1 left, that person is `Solo`.
   - Groups thin out as members marry or have children. They never reshuffle in mid-epoch, and nobody joins mid-epoch.
6. **Cost:**
   - About `K` eligibility checks at the epoch start, plus a check of each chunk member at `t`.
   - The cheapest tests run first: uptake draw, then age, then union, then children, then parents.
   - Only uptake-passing single young adults pay it.
7. **Why this works.** Each member decodes the same frame from its own id. It evaluates the same eligibility predicates on the same people and chunks them the same way. Reciprocity is by construction, and the exhaustive test checks it.

**Risk to measure:**
- The frame is drawn from everyone in the band, so the density of eligible people (single, childless, uptake) sets how often a group forms.
- Uptake `w` and `K` must hit roughly 7% of 18 to 24 and 5% of 25 to 34 year olds in roommate households (2019).
- If no `(w, K)` hits both, the band could exclude the married by construction. That would need a counted space of singles, which doesn't exist, so it would be a design change to bring back.

## 6. Members

`members(h, t)` starts from seeds and adds dependents recursively.

**Seeds:**
- `Union`: both partners.
- `Solo`: the person.
- `Roommates`: the still-eligible chunk.

**Dependents of a member `s`:**
- **Rule 1.** Children of `s` whose custodial parent at `t` is `s` and who are at home.
  - This includes stepchildren in a union household, through the partner who has custody.
  - Orphans with guardian `s`: grandchildren whose parents are both dead, and younger siblings when `s` is the eldest adult sibling.
- **Rule 2.** The parents of `s`, if their elder unit's host is `s`, together with the parent's partner at `t`. That partner may be a step-parent of `s`.
- **Recursively**, the dependents of each dependent (a teen mother's baby; an elder's own minor child; an elder's own parent).

Each candidate is confirmed by evaluating its `household` and comparing, so `members` can't disagree with `household`. Enumeration only has to be complete, and the exhaustive test checks that.

## 7. Parameters (provisional; calibrated in the report)

| Parameter | Shape | Calibrated against |
|---|---|---|
| Independence age `A` | survival `S(a) = tail + (1 − tail) / (1 + exp((a − m)/s))` by sex and cohort (`m`, `s`, `tail` interpolated between anchor cohorts) | AD-1 at home 1960 to 2025 by sex and age band; Pew 1940; AD-3 "child of householder" 11.6% |
| Custody to father | 0.25 | FM-1 |
| Elder co-residence `q` | logistic in age; level by cohort; single above couple | Ruggles ~70% (1850) to ~15% (1990); 65+ alone 28% (2023) |
| Roommate uptake `w`, frame size `K` | `w` by age and era, zero before about 1960; `K` = 8 | 7% of 18 to 24 and 5% of 25 to 34 in roommate households (2019); other nonfamily households 6.7% (2025) |
| Band, epoch | 3 birth years, 2 years | fixed |

## 8. Realism report (new section)

At `t` = 1880, 1900, 1940, 1960, 1980, 2000, 2025, sampling people present at `t`. Household-level shares weight each sampled person by `1/size`.
- Mean household size, and the size distribution 1 to 7+ (HH-6, HH-4).
- Household types: couple, couple with children under 18, single mother, single father, one person, roommates, other family (HH-1).
- At home, 18 to 24 and 25 to 34, by sex (AD-1).
- Adults: alone, with partner, child of householder, with other relatives, with non-relatives (AD-3, 2023).
- 65+: alone, and with an adult child (Ruggles).
- Residuals: orphan minors without a guardian; founder minors living alone.

## 9. Tests

- **Unit:** the chunking table, epoch phases, the leaving schedule's survival function (monotone, in [0, 1]), `q` monotone in age.
- **Exhaustive, tiny world** (`tests/households.rs`), at every second year from `y0` to `y1` and at keyed times within years:
  1. reciprocity both ways, so the household partition is exact;
  2. both partners of an active union share a household;
  3. no minor is alone, except the founder residual;
  4. roommate groups have 2 to 3 members, each eligible;
  5. every member is present at `t`.
- **Time consistency:** for sampled people, `household(x, t)` and `household(x, t + 1 day)` differ only across an event, and every change in a lifetime is explained by one.
- **Prototype:** sampled reciprocity on 10⁵ people at several `t` (`tests/realism.rs`).

## 10. Performance

- `kinship/household` and `kinship/household_members`: uniform people present at `t`, with `t` weighted to 1990 to 2030, plus the worst cases (roommate-eligible young adults, large union households).
- Budgets come after the first measurement, sized to workloads: for example a view that resolves one household and its members per call.

## 11. Build order

1. Parameters and their unit tests (`params.rs`): independence schedule, custody, elder `q`, roommate uptake.
2. `household.rs`:
   1. rules 1 to 4 without roommates;
   2. `members`;
   3. the exhaustive tiny suite.
3. Roommate frames, and their tests.
4. Realism section and calibration.
5. Perf suite entries; measure; propose budgets.
6. Docs: AGENTS.md, and this plan's outcome log.

## 12. For the founder (proceeding with these unless told otherwise)

1. **No dorms** until education (L5). CPS counts dorm students at home, so nothing is lost against the targets.
2. **No boarders, lodgers or servants.** Nineteenth-century households will be smaller than the census's (about 5.5 in 1850, a figure not verified here). The report will show the gap instead of tuning around it.
3. **Couples always head their own household.** Three-generation homes come from elders moving in with a child and from children who never left. The 19th-century stem family appears with the same people under one roof but a different head.
4. **Cohabitation and marriage are not distinguished**, as in R1. The report compares couple households with married plus cohabiting.
5. **No boomerang returns home** in this version.
6. **Founder minors** (no in-world parents) live alone until 18; this affects 1840 to 1857 only.

## Outcome, steps 1 to 4 (2026-09-30)

**Built:**
- `household.rs`:
  - `World::household(x, t)`, `World::members(h, t)`, `World::dependent_of`, `World::present_at`;
  - the four rules, roommate frames and member enumeration.
- Parameters in `params.rs`, each with a unit test:
  - the independence schedule and its inverse;
  - custody;
  - elder co-residence (monotone in age, single above couple);
  - roommate uptake.
- `examples/household_report.rs` (about 25 s).

**Tests:**
- `tests/households.rs` runs exhaustively over all 39.6k people of the tiny world at 15 dates.
- Reciprocity is exact both ways at every date, first time.
- No household flickers from one day to the next; at most 7 changes in a lifetime.
- The checks found two rule gaps, now fixed:
  - independence at 17 let minors live alone, so the minimum is now 18;
  - a minor whose union had ended lived alone, so minors now return to a parent when not in a union.

**Residuals, measured and allowed by the test:**
- **Founder minors**, who have no in-world parents, live alone until 18 (1840 to 1857 only).
- **Kinless orphans** have no living parent, grandparent or adult sibling: 0.15% of minors in 1900 and none by 2025. Their grandparents are founders.

**Speed** at 2025, 100k uniform people present:

| Call | p50 | p90 | p99 |
|---|---|---|---|
| `household` | 10.6 µs | 24 µs | 80 µs |
| `members` | 39 µs | 85 µs | 141 µs |

The mean household has 2.9 members. This is well within workload needs (founder decision 2026-09-30); no optimization yet.

**Calibration of the household parameters** (report at 2025 unless noted):

| Measure | Model | Target |
|---|---|---|
| at home 18 to 24, men / women, 2000 | 59.4 / 46.4% | 57.1 / 47.1% |
| at home 18 to 24, 2025 | 64.1 / 56.2% | 58.8 / 56.4% |
| at home 25 to 34, 2025 | 17.2 / 8.9% | 19.2 / 13.6% |
| at home 25 to 34, 1960 | 6.0 / 3.7% | 10.9 / 7.4% |
| 65+ with an adult child, 2000 | 15.0% | under 15% (Ruggles) |
| adults with non-relatives (roommates) | 2.5% | about 2.5% (71% of 3.5%) |

- 25 to 34 at home stays low in 1960 because married children living in their parents' home are out of scope (§12 item 3).
- Returns home after a divorce are also out of scope (§12 item 5).

**Gaps that come from kinship, not from households:**

| 2025 | Model | Census |
|---|---|---|
| one-person households | 50.9% | 29.5% |
| adults living alone | 31.9% | 14.8% |
| adults with a partner | 48.9% | 57.8% |
| 65+ living alone | 53.8% | 28% |
| mean household size | 2.00 | 2.50 |

**Decomposition of the 31.8% of adults living alone in 2025:**
- **Divorced: 13.8%.** Expected is about 4%: roughly a third of the 11.3% divorced or separated live alone.
- **Never-partnered immigrants: 6.4%.** Single arrivals have no marital history from abroad and partner little after arriving.
- **Widowed: 5.1%** (about 3.4% expected).
- **Never-partnered natives: 6.4%** (plausible).

**Census A1 (2023) against the model, by age:**

| Age | Model, divorced and single | Census, divorced or separated |
|---|---|---|
| 45 to 54 | 21.8% | 16.3% |
| 55 to 64 | 26.7% | 19.4% |
| 65 to 74 | 30.7% | 17.6% |
| 75+ | 26.9% | 11.5% |

| Age | Model, in a union | Census, married plus cohabiting (approximate) |
|---|---|---|
| 45 to 54 | 59.6% | ~70% |
| 65 to 74 | 43.6% | ~64% |
| 75+ | 26.1% | ~51% |

Widowhood matches (65 to 74: 12.7% against 11.7%; 75+: 33.0% against 33.2%).

**Likely causes:**
1. R1c's re-partnering hazard was calibrated to remarriage (NSFG), but unions here include cohabitation, which is the more common path after divorce.
2. At most two unions: everyone whose second union breaks stays single.
3. No widowed re-partnering (deferred by the founder).
4. Single immigrants arrive as "never partnered".

**For the founder:** see AGENTS.md, L3. Recommendation: recalibrate re-partnering to current-status targets (Census A1 by age), and give older single arrivals a previous union (divorced or widowed abroad) so they enter the re-partnering markets. Only then judge widowed re-partnering and third unions.

## 13. Revision (2026-09-30): kin co-residence, statistics not rules

The founder's direction ("don't make global rules for people; it's just statistics") retires §12 items 3 and 5 and the old rule 2. They are replaced by one mechanism.

**Measured reason.** In 2025, 59% of unpartnered adults live alone in the model, against about 35% in the Census. Census AD-3 has 12.3% of adults living with relatives other than a partner or parent, against 6.8% in the model.

**Units.** A unit is an independent single adult, or a couple (the woman decides; the lower id for a same-sex couple).

**Seeking kin.** Each unit has a keyed uniform `u` (the decider's) and a propensity `q` by situation, age and era. It seeks kin when `u < q`. The propensity depends only on the unit itself, never on anyone else's decision:
- **single, under 35, a parent alive:** moving back with a parent (boomerang);
- **single, any age:** living with relatives;
- **elder** (single or couple): the old `elder_coresidence`;
- **young couple:** living with a parent (a subfamily, common before 1960).

`q` is smooth in age, so a person moves in or out at most at a few natural points (a host's death also ends a stay).

**Hosts are anchors.** A host is present, independent, 18+, does not seek kin themselves, and is not a roommate. Anchors never seek kin, so guest-to-host is a single step and chains can't cycle. The chain is now: rule 1 any number of times, then at most one step from guest to host, then a terminal rule. Roommate eligibility adds "does not seek kin".

**Host order:**

| Unit | Hosts, in order |
|---|---|
| young single | mother, father, then siblings (eldest first) |
| single 35 to 59 | siblings, mother, father, adult children |
| elder | daughters, then sons (eldest first), then siblings |
| young couple | the woman's mother and father, then the man's |

Each list is the unit's own kin (for a couple, the decider's first, then the partner's).

**Members.** A host's guests are found among its kin: parents, adult children and siblings, with the partner of any couple guest. Each candidate is confirmed by `kin_host(candidate) == host`. Guests' dependents follow through rule 1.

**Calibration targets:**
- AD-3 2023: alone 14.8%, child of householder 11.6%, other relatives 12.3%, non-relatives 3.5%;
- AD-1 at home by year;
- 65+ alone 28%;
- 65+ with an adult child (Ruggles);
- household size and one-person share by year (HH-4, HH-6).

### §13 outcome (2026-09-30)

**Built** (`household.rs`):
- `World::kin_host` with units, `seeks_kin` (propensity by unit, age, era) and `is_anchor` (hosts never seek kin and aren't roommates), plus guest enumeration in `members`.
- **Roommates:** open to single adults aged 18 to 64 with no child under 18 and not seeking kin; uptake falls with age; frames of 12.
- **Params:** `kin_single` (moving back with a parent, other relatives, elder) and `kin_couple` (young couple with a parent, elder couple).

**Tests:**
- Exhaustive reciprocity still holds on the tiny world at 15 dates.
- The time-consistency test now checks for reverting the next day. Two different events can legitimately fall on consecutive days, such as a roommate group thinning just before a union.
- Workspace: 850 passed.

**Report:**

| Measure | Model | Census |
|---|---|---|
| mean household size, 1980 | 2.75 | 2.76 |
| couple households, 1980 | 60.9% | 60.8% |
| mean household size, 2000 | 2.44 | 2.62 |
| couple households, 2000 | 53.2% | 52.8% |
| mean household size, 2025 | 2.27 | 2.50 |
| one-person households, 2025 | 40.6% | 29.5% |
| adults living alone, 2025 | 22.5% | 14.8% |
| adults with a partner, 2025 | 53.0% | 57.8% |
| adults, child of the householder, 2025 | 9.7% | 11.6% |
| adults with other relatives, 2025 | 12.0% | 12.3% |
| adults with non-relatives, 2025 | 2.8% | 3.5% |
| 65+ with an adult child, 2000 | 18.5% | under 15% (Ruggles) |
| 65+ living alone, 2025 | 39.4% | 28% |

Living alone by age in 2023 (model; Census approximate, from the 2023 total and CPS age patterns):

| Age | 18 to 24 | 25 to 34 | 35 to 44 | 45 to 54 | 55 to 64 | 65 to 74 | 75+ |
|---|---|---|---|---|---|---|---|
| Model | 12.6% | 13.8% | 16.3% | 17.2% | 22.5% | 27.7% | 37.8% |
| Census (approx.) | ~4% | ~11% | ~9% | ~11% | ~16% | ~22% | ~38% |

**What is left:**
- About 5 points of the living-alone excess is the partner deficit from kinship: widowed re-partnering, mortality that ignores partnership, and too few young unions (18 to 24: 9.7% against 15%).
- About 2 points is too few adults living with parents.
- Roommates of older ages rarely group, because eligible people are sparse in a frame.
- 1960 one-person households: 25.6% against 13.1%. Partly no boarders or lodgers (§12 item 2), partly the elders.

**Speed** at 2025 (50k people present):

| Call | p50 | p90 | p99 |
|---|---|---|---|
| `household` | 17 µs | 49 µs | 176 µs |
| `members` | 167 µs | 382 µs | 731 µs |

Guest enumeration looks up each member's kin. Resolving a household and its members takes under 1 ms, fine for workloads.
