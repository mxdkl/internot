# R1d: statistics, not rules

**Date:** 2026-09-30
**Why:**
- **Founder direction (2026-09-30):** "Don't make global rules for people; theoretically anything can happen, it's just statistics. Do what you think is realistic."
- **What found it:** L3's household report (`plans/2026-09-30-l3-households.md`, outcome). Against Census 2023 tables A1 and UC3, too few people live with a partner from 45 on:

  | Age | Model | Census |
  |---|---|---|
  | 45 to 54 | 59.6% | ~70% |
  | 55 to 64 | 52.4% | ~66% |
  | 65 to 74 | 43.6% | ~64% |
  | 75+ | 26.1% | ~52% |

  The gap shows up as too many divorced singles and never-partnered immigrants.

## 1. The hard rules in the kinship model, and what each costs

| Rule | Where | Measured cost (2025) |
|---|---|---|
| At most two unions | `CellKind::first_opposite` feeds sources; `seats` returns two | Divorced singles after a second union: 7.4 / 12.3 / 16.6 / 16.4% of people at 45–54 / 55–64 / 65–74 / 75+. They can never re-partner. |
| Single immigrants follow the first-union schedule for the never partnered | `want` in `Ledger::build` | 8 to 9% of people 45+ are immigrants who never partnered (natives: 4.5 to 7.3%). A single 45-year-old arrival has the hazard of a native who has stayed single to 45. |
| No first union after 65 (women) or 70 (men) | `MAX_UNION_AGE_F/M`, `first_union_hazard` range 16 to 65 | small (the hazard is low there) |
| No re-partnering after 75 | `MAX_REMARRIAGE_AGE`, `remarriage_base` | some at 75+ |
| No widowed re-partnering | R1c scope (founder, 2026-09-30) | The widowed share already matches A1 (12.7% / 33.0% against 11.7% / 33.2% at 65–74 / 75+), so it matters mostly for young widows and for stepfathers. |
| No same-sex re-partnering | R1c scope | small (1.5% of couples) |

Re-partnering was also calibrated to **remarriage** (NSFG), while the model's unions include cohabitation, the more common path after a divorce. So the hazard is too low even where no rule binds.

## 2. Changes, in order

1. **Smooth ages.**
   - The first-union hazard runs to any age and keeps declining, with no cutoff.
   - Markets admit seekers of any adult age.
   - Re-partnering's age factor declines smoothly, with no cutoff at 75. `MAX_REMARRIAGE_AGE` stays only as the death draw's fast-path bound, set where the hazard is effectively zero (95).
2. **Immigrants partner like the people around them.**
   - A single arrival keeps their status (single) and enters the in-world never-partnered markets.
   - Their yearly partnering rate is that of **natives who are single at the same age**, never partnered and divorced together: `(h_first · never + divorced want) / (never + divorced alive)` for their block.
   - Nobody is labelled; the arrival cohort partners at the rate of its age peers who are single.
   - `settle` splits a block's takings over its cohorts by want, meaning rate times pool, not by pool alone.
3. **Any number of unions.**
   - Every opposite-sex cell that separates becomes a divorced source, not only first unions.
   - "Second" kinds come to mean **re-partnering** (the member was partnered before), whatever the order.
   - In the world, `second_seat` becomes `next_seat` (a chain), and `person_in_cell` already recurses through sources.
   - `unions()` and `union_cells()` return an inline list (capacity `MAX_UNIONS` = 16, a technical bound). The prototype reports the largest count actually reached, and a test fails if the bound is ever hit.
4. **Calibrate re-partnering to current status.** Targets:
   - living with a spouse or partner by age (A1 married spouse present plus UC3 cohabiting, 2023);
   - divorced and single by age (A1 divorced or separated, less about 18% who cohabit).

   The NSFG remarriage timing stays as a lower bound: re-partnering including cohabitation should be at or above remarriage.
5. **Widowed re-partnering:** next, after 1 to 4. It has its own exactness problem (`research/2026-09-30-repartnering-exactness-problem.md` §4): a death is a keyed draw, not a ledger count. Reassess after measuring 1 to 4.

## 3. What must still hold

- Every R1/R1c guarantee, checked by the exhaustive tiny suite:
  - reciprocity on every union;
  - closure per (year, kind, class, block, block, sex);
  - zero close kin;
  - duality;
  - life bounds (each union starts while both are alive and after the previous one's separation).
- **Realism:** the R1 cohort bands; the R1c re-partnering report (now including cohabitation, so above NSFG remarriage); the new current-status table.
- **Speed:** pragmatic (founder decision). `death` loses its fast path for divorced people; measure it.

## 4. Tests to add

- Union chains: each union starts after the previous one separates, and the partner's chain holds the same union.
- The largest union count over the prototype stays under `MAX_UNIONS`.
- Arrival cohorts' partnering: in the report, immigrants never partnered at 45+ come close to natives.

## Outcome, steps 1 to 4 (2026-09-30)

**Built:**
- **Smooth ages:** no first-union cutoff, markets for any adult age, re-partnering declining to 95. Tiny expected wants (< 1e-6) are dropped as numerical noise.
- **Immigrants:** they partner at their single native peers' rate (`Pool::single_rate`), and settlement splits takings by want.
- **Any number of unions:** `CellKind::feeds_divorced`, `World::next_seat`, `MAX_UNIONS` = 16.
  - The kin predicate's paternal half-sibling check covers a father's re-partnering unions of any order. It used to check first against second only; without the fix, children of a man's second and third unions could have partnered.
  - Every test passes, including zero close kin, closure and the generalized union-chain test.
- **Calibration:**
  - re-partnering: stronger after 10 years and at 45+, with no era decline;
  - unions formed since 1990: about half separate (cohabitation included), sooner than marriages;
  - fertility compensated: fewer childless plans after 1975, and `SECOND_UNION_FERTILE` 0.55.

**Results** (prototype, 14.0M ever born, up from 12.1M):

| 2023, age | In a union | Census | Was |
|---|---|---|---|
| 25 to 34 | 58.7% | 53.8% | 61.5% |
| 35 to 44 | 66.9% | 68.9% | 69.4% |
| 45 to 54 | 65.8% | 70.1% | 64.1% |
| 55 to 64 | 62.7% | 66.2% | 58.3% |
| 65 to 74 | 54.7% | 63.7% | 49.5% |
| 75+ | 33.9% | 51.5% | 29.8% |

- **Fertility:** the 1970 cohort has 2.02 children, with 17.3% childless (was 1.86 and 21.8% after the dissolution change); the 1950 cohort has 2.47.
- **Re-partnering within 10 years:** 72% (NSFG remarriage only: 75%).
- **Unions per ever-partnered person born 1930 to 69:** 60.7 / 23.4 / 11.3 / 3.7 / 0.8% for one to five unions; the most is 9.
- **World build:** 2.4 s, over the 2 s budget (more cells). Deferred per the founder's pragmatic-speed decision; to be measured with the gate.

**Debt still open** (measured realism cost):
1. **No widowed re-partnering** (needs counted deaths: `research/2026-09-30-repartnering-exactness-problem.md` §3). Widowed singles: 15.2% against 11.7% at 65 to 74, and 39.0% against 33.2% at 75+.
2. **Mortality ignores partnership.** Married people live longer, so the old should be more often married. This accounts for part of the 75+ gap.
3. **No same-sex re-partnering.**
