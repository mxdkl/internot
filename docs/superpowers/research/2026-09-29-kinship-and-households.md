# Kinship, partnerships and households: demographic math for a stateless world

**Date:** 2026-09-29
**Question:** What demographic theory and data make stateless kinship realistic and consistent? The constraints:
- Every fact is `f(person_id, t, seed)`.
- Up to 2³² ids.
- Relations must be reciprocal, age-consistent, turn over in time, and never produce sibling couples.

We also want to know where the working "keyed bijection / XOR-involution / partial matching" construction is naive.
**Builds on:**
- `2026-05-14-real-social-graph-structure.md` (tie structure; not repeated here).
- `specs/2026-05-14-social-graph-substrate.md` §6 (household model). This note replaces its partner and birth mechanics; see §0.

## Summary: what is directly usable

1. **Index people by birth cohort, not by age.**
   - An id is (birth city, birth year, rank) over everyone ever born in a window such as 1850–2150.
   - **Alive at t** ⇔ `birth ≤ t < death`, with death from cohort life tables.
   - This gives population turnover, fills the 17 missing ages, and makes minors ordinary ids.
2. **Cohort sizes come from a small global ledger.** It is a two-sex cohort-component (Leslie) projection over (city × year × sex), computed once in expected values and then integerized.
   - It stores only counts that two blocks must agree on: unions `U[c][y_m][y_f][band]`, births `E[c][y_f][y]` and the out-of-world share `I[c][y]`.
   - A child cohort's size is **defined** as the births to earlier cohorts plus `I`. That makes the pyramid and fertility consistent by construction.
3. **Relations are rank-range matchings between blocks, not permutations within a block.**
   - Permute each block with a keyed Feistel permutation (cycle-walking).
   - Cut the permuted rank line into nested quota ranges sized by the ledger.
   - Pair offsets across blocks.
   - This gives reciprocity and age-consistency by construction, with O(log) lookups. A prototype passed an exhaustive reciprocity check.
4. **Partner age gaps are a table, not a block.** US opposite-sex married couples (CPS 2023): 35.4% within 1 year, husband 2+ years older 50.2%, wife 2+ older 14.4%. Mean +2.1 y, SD ≈ 5.1 y (derived). A within-block involution gives a gap of 0.
5. **The two-sex problem:** union counts are one shared table computed from both pools, e.g. with Schoen's harmonic-mean function. They are never per-sex rates.
6. **Families.**
   - Children link to their mother's birth events indexed by (mother cohort, birth year).
   - The father is the mother's partner in that union. Siblings are the same mother's other events.
   - Nesting death cells inside union and fertility cells guarantees no births after the mother's death.
7. **Partial matching must be explicit and small.** Kin counts fall roughly with (1−p)^degree (inferred): 20% unmatched removes about half of all cousins. Put Y0 about 100 years before the era of interest; SOCSIM work uses runs of centuries.
8. **Targets computed here** (Caswell model, US 2023 rates):
   - P(mother alive) is 0.92 at age 30, 0.63 at 50 and 0.33 at 60.
   - Living grandparents are 3.4 at birth and 1.0 at age 30.
   - Living siblings are about 1.6. Observed: 62% of US women aged 45–64 had a living parent (PSID 2007).
9. **Random pairing produces sibling couples** (~10³–10⁴ across 2³², inferred). Add a deterministic pair-swap repair. SOCSIM forbids unions closer than cousins.
10. **Households are derived views** over union windows, leaving-home ages, widowed-parent co-residence and a roommate grouping. US targets (CPS 2025): average size 2.50, one-person 29.5%, married-couple 46.6%, other nonfamily 6.7%.

## 0. Relation to spec §6

Spec §6.3 anchors parents at `B−28` and runs Irving stable-roommates per (cohort, region). It then derives each child's id from the couple and hopes the child "lands in the right (birth_year, region) slot bucket". This note keeps the spec's principle, "edges are shared objects". It replaces the mechanics for five reasons:

1. **Cost.** Stable roommates is O(n²) on cohorts of ~10⁵.
2. **Nothing fixes the pool.** Stable roommates has no age-gap table, no never-partnered share and no two-sex balance, and gender-agnostic preference produces ~50% same-sex pairs unless orientation is encoded.
3. **Counts don't close.** Child ids can't land in fixed-size buckets unless the bucket size is defined from the births. That is the ledger's job.
4. **Parity, not lognormal.** A lognormal count per couple should be replaced by an observed parity distribution.
5. **Minors.** Minor stubs can't age into the same adult when `now` moves.

---

## 1. Kinship models: what rates imply about kin

### 1.1 Goodman–Keyfitz–Pullum (1974)

GKP (*Theor. Pop. Biol.* 5:1–27, addendum 1975) compute the expected numbers of female, matrilineal kin implied by one fertility and one mortality schedule in a stable population. The notation:

- `l(x)`: female survivorship.
- `m(x)`: daughters per woman per year at age x.
- `W(x) = e^{−rx} l(x) m(x)`: density of mothers' ages at birth. It integrates to 1 by Lotka's equation.

The key forms:

- **Mother alive when Focal is a:** `M₁(a) = ∫ W(x) · l(x+a)/l(x) dx`.
  - Ruggles uses exactly the discrete form, `Σ_x b_x l_{x+a}/l_x`, for US parents of 40–44-year-olds.
  - He uses *cohort* life tables for the parents. He treats mothers and fathers separately because spouses' mortality is correlated and independence would understate "no surviving parent".
- **Grandmother alive:** `M₂(a) = ∫ M₁(a + x) W(x) dx`.
- **Daughters.** Ever born by a: `∫_α^a m(x) dx`. Living: `∫_α^a m(x) l(a−x) dx`.
- Sisters, aunts and cousins are nested integrals of these pieces. GKP had to use finite approximations to evaluate them.

### 1.2 Caswell's matrix kinship models (2019 →)

Caswell (2019, *Demographic Research* 41:679–712) makes kin a population that evolves as Focal ages.

- `U` is survival (`p_x` on the subdiagonal). `F` is fertility (first row `f_x`).
- `A = U + F` is the Leslie matrix. Its dominant right eigenvector is the stable age distribution `w`.
- `π = (F(1,:)ᵀ ∘ w)/‖·‖` is the distribution of mothers' ages. Its mean is the generation time.
- Each kin type is a vector `k(x)` of living kin by age, and `k(x+1) = U k(x) + β(x)`:

| Kin | k(0) | subsidy β(x) |
|---|---|---|
| daughters a | 0 | F eₓ |
| granddaughters b | 0 | F a(x) |
| mothers d | π | 0 |
| grandmothers g | Σᵢ πᵢ d(i) | 0 |
| older sisters m | Σᵢ πᵢ a(i) | 0 |
| younger sisters n | 0 | F d(x) |
| aunts older / younger than mother | Σ πᵢ m(i) / Σ πᵢ n(i) | 0 / F g(x) |
| cousins via older / younger aunts | Σ πᵢ (nieces via older sisters)(i) / … | F r(x) / F s(x) |

- **Derived properties** are linear maps `Ψ k(x)`: counts, prevalence of a condition among kin, dependency, co-residence probability. Mean ages of kin come from age moments.
- **Extensions.**
  - II: multistate, parity and sibship (2020).
  - III: time-varying rates (Caswell & Song 2021). This lets the model follow real cohorts instead of a stable population.
  - IV: two-sex (2022). The "androgynous" approximation, which sets male rates equal to female rates, is adequate even for Senegal. Differences are "at most a fraction of an individual", and none at all for female children, parents and siblings.
  - VI: variance and covariance of kin numbers.
- **Software.** The DemoKin R package implements these models. It was archived from CRAN on 2026-06-29 at the maintainer's request; its docs are still online.
- **Global projections.** Alburez-Gutierrez, Williams & Caswell (PNAS 2023) apply the two-sex, time-varying model to UN WPP 2022.
  - A 65-year-old woman had 41 living relatives in 1950 versus 25 [18.8–34.7] projected for 2095.
  - Europe and North America: 25 → 15.9. Italy: 18 → 12.7.
  - Chinese newborns' living great-grandparents go from 1.7 → 5.3 and their cousins from ~11 → 1.1.

### 1.3 Numeric targets

These are computed here with the Caswell recursion.

- **Inputs:** US 2023 single-year life tables (NVSR 74-6) and US 2023 age-specific fertility rates (ASFR, NVSR 74-1); sex ratio at birth assumed 1.05.
- **Two-sex method:** androgynous approximation, with fathers 2 years older and the male life table.
- **Stable-population figures:** λ = 0.9917 and mean age at maternity 29.8.
- **Status:** these are (inferred) model outputs, not observations.

| Focal age | P(mother alive) | P(father alive) | living grandparents | living siblings | children | grandchildren | first cousins |
|---|---|---|---|---|---|---|---|
| 0 | 1.00 | 1.00 | 3.45 | 0.79 | 0 | 0 | 2.3 |
| 10 | 0.99 | 0.97 | 2.91 | 1.38 | 0 | 0 | 3.7 |
| 20 | 0.96 | 0.93 | 2.04 | 1.58 | 0.07 | 0 | 4.5 |
| 30 | 0.92 | 0.84 | 1.02 | 1.58 | 0.80 | 0 | 4.8 |
| 40 | 0.82 | 0.69 | 0.30 | 1.56 | 1.54 | 0.02 | 4.7 |
| 50 | 0.63 | 0.46 | 0.04 | 1.52 | 1.60 | 0.31 | 4.6 |
| 60 | 0.33 | 0.18 | 0 | 1.45 | 1.59 | 1.23 | 4.3 |
| 70 | 0.08 | 0.03 | 0 | 1.29 | 1.57 | 2.19 | 3.8 |
| *TFR = 2.0 (US completed fertility, women 45–50 in 2024)* — age 30 | 0.92 | 0.85 | 1.07 | 1.96 | 0.99 | 0 | 7.2 |
| *TFR = 2.0* — age 60 | 0.34 | 0.19 | 0 | 1.79 | 1.96 | 1.88 | 6.6 |

**Observed anchors.**

- DemoKin's Sweden 2015 example (female line, age 35): mother 0.93, maternal grandmother 0.18, older sisters 0.43, younger sisters 0.47, daughters 0.70.
- PSID, US women aged 45–64:
  - ≥1 living parent: 47.4% (1988) → 61.8% (2007).
  - Living parent and a child: 42.6% → 54.0%.
- Margolis & Verdery (2019, SOCSIM, US):
  - About 80% of white women and 70% of white men born in the 1930s–40s became grandparents.
  - The median age at first grandchild is mid-to-late 40s.
  - The median duration of grandparenthood for white women rose from 28 years (1880s cohorts) to 36 years (1960).
- Verdery & Margolis (2017): 14.9 M white and black Americans aged 50+ had no living partner or children in 2015. Of them, 1.8 M also had no living sibling or parent; the projection for 2060 is 6.3 M.

**Caveats for tests.**

- **Sibling dispersion.** The recursions treat births as independent, so siblings ≈ TFR × survival. The child's-eye family size is `E[K²]/E[K]`, so siblings ever born = `μ + σ²/μ − 1` (Preston's identity; derivation).
  - For US women aged 45–50 in 2024, μ = 2.02 and σ² = 1.79, giving **1.91 siblings ever born**. Only **8.8% of children are only children** (derived from the CPS parity table).
  - US parity is close to Poisson-dispersed, so the model's sibling numbers are near-correct for the US. They would be too high in two-child-norm countries.
- **Use Internot's own rates as the oracle.** Property tests should run the time-varying Caswell model on *Internot's own ledger rates* and compare sample means (≈10k people per age). The table above is a sanity band, not the assertion.

---

## 2. Microsimulation of kinship networks

### 2.1 SOCSIM

SOCSIM was built at Berkeley by Hammel and Wachter with Laslett (Cambridge). The operating manual dates from 1976; the `rsocsim` R package is from MPIDR.

- **Closed population.** People enter only by birth and leave only by death, so partners must be found inside the population. Each record links mother, father, the next-eldest sibling through the mother and through the father, the last-born child and the marriage id. It is written in C with linked lists.
- **Events.**
  - Monthly hazards by (event, group, sex, marital status [, parity]).
  - Each person's next event is the shortest of piecewise-exponential waiting times (competing risks).
  - `bint` sets a minimum birth interval (9 months recommended), and fertility rates are inflated to compensate.
- **Marriage market.**
  - *Two-queue*: both sexes have rates. A searcher marries only if a suitable spouse is already waiting; otherwise they queue.
  - *One-queue*: only females have rates. The bride takes the best man among **all** living unmarried males, which hits female rates more reliably.
  - Scoring either targets a normal distribution of age differences (`agedif_marriage_mean`/`_sd`), picking the union that most reduces the gap between observed and target distributions, or uses a preference peak (default 36 months). `marriage_slope_ratio = 2` penalises bride-older matches more.
  - `marriage_allowable()` excludes anyone related closer than cousins and former spouses.
  - `endogamy` ∈ [−1, 1] sets group rejection.
  - `random_father` assigns fathers to non-marital births from men over a minimum age, respecting incest and endogamy rules.
- **Murphy (2004), British runs.**
  - Initial populations of 4,000 and 10,000 with England's 1741 age structure; two 600-year runs (1250–1850 and 1750–2350).
  - Preferred gap about 2 years; the man is never more than 5 years younger or 10 years older.
  - Incest prohibition; cohabitation from 1960.
  - **All births are forced into unions**, because "full kinship links could not be constructed if out-of-partnership births were allowed".

### 2.2 CAMSIM and LifePaths

- **CAMSIM** (Cambridge Group; Smith & Oeppen 1993) is Monte Carlo microsimulation that "produces kin sets for a birth cohort of unrelated egos in a stable population". It is ego-centred with no shared network, so it is not reciprocal.
- **LifePaths** (Statistics Canada) is **open**. When a union happens, it *creates* a spouse whose age and education follow observed joint distributions, either from a pre-built "spouse market" or by generating candidates until one fits.
  - Murphy's critique of open models: only one partner has a demographic background, so general kinship can't be traced. This is exactly the risk of Internot's "partial matching".

### 2.3 Which mechanisms can be made stateless

| Mechanism | Stateless? | Stateless replacement |
|---|---|---|
| Rates by age / sex / marital status / parity | yes | pure functions plus nested quotas (§9.3) |
| Competing-risk waiting times | yes, if risks depend only on own state | hash-uniform → inverse CDF |
| Marriage queue (order- and history-dependent) | **no** | aggregate two-sex marriage function in the ledger plus rank-range pairing. Individual scoring is lost; the age-gap *table* is hit exactly. |
| Incest check | yes | local predicate plus deterministic pair swap (§9.4) |
| `random_father` | yes | one more count table plus rank-range matching |
| `bint` spacing | yes | built into the fertility-plan catalog |
| Closed population | yes | the id space *is* the population; founders and immigrants are the explicit open share |
| Burn-in from an initial population | yes | the ledger's founder era (§7.3) |

---

## 3. The two-sex problem and marriage functions

**The problem.** Male and female rates imply different numbers of marriages whenever the age–sex composition shifts (baby booms, the "marriage squeeze", excess male mortality). The count of unions between men aged i and women aged j is one number, `M_ij = M(m_i, f_j, competitors)`, where `m_i` and `f_j` are the unpartnered at risk.

Requirements usually imposed on M (inferred summary):

- Homogeneous of degree 1.
- Monotone in each pool.
- Zero if either pool is empty.
- `M_ij ≤ min(m_i, f_j)`.
- Competition: more people of the same sex at other ages lower `M_ij`.

**Schoen (1981) harmonic mean.** `M_ij = α_ij · m_i f_j / (m_i + f_j)`.

- `α_ij` is the force of attraction, estimated from observed marriages and pools.
- The harmonic factor is ≤ min(m_i, f_j), so no pool is over-drawn when α ≤ 1.
- The sum of the male and female rates (the "magnitude of marriage attraction") is invariant to composition.
- Choo & Siow (2006) call it "the current workhorse in demography". Their own function, μ_ij = Π_ij √(μ_i0 μ_0j), is the economists' alternative (formula not re-verified here).

**Implications for pairing.**

1. **One shared table.** Union counts are one table per (city, y_m, y_f, band). A block's partnered count is a **margin** of that table.
2. **Remainders differ by sex.** Men are unpartnered more at young ages; women dominate widowhood (65+ widowed: women 28.5%, men 11.1%). A fixed-point-free involution can express neither.
3. **Squeezes resolve themselves.** A large cohort spreads its partners over neighbouring cohorts and leaves more singles.

**Same-sex partnerships (US).**

- ACS 2024: ~1.4 M same-sex couple households. About 836k are married (≈450k female, ≈386k male) and ~551k unmarried. That is ≈1.0% of all households.
- ACS 2019: 1.5% of *coupled* households.
- 17% of married and 10% of unmarried same-sex households have children (2023).
- Gallup 2025: 9% of adults identify as LGBTQ+ and 5.3% as bisexual. Identification ≫ same-sex partnership, because most bisexual adults have opposite-sex partners.
- **Target:** ~1.5–2% of unions in recent cohorts, fewer in pre-1990 union cohorts (inferred).

---

## 4. Assortative mating

**Age gap (US, CPS 2023 FG3, 63.9 M opposite-sex married couples).**

| Wife 20+ older | Wife 10–19 older | Wife 6–9 older | Wife 4–5 older | Wife 2–3 older | Within 1 year | Husband 2–3 older | Husband 4–5 older | Husband 6–9 older | Husband 10–14 older | Husband 15–19 older | Husband 20+ older |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 0.28% | 1.2% | 2.5% | 3.3% | 7.0% | 35.4% | 20.0% | 12.2% | 11.2% | 4.6% | 1.3% | 0.9% |

- Mean +2.1 y (husband older) and SD ≈ 5.1 y, from category midpoints (derived).
- Couples with and without children under 18 look alike (36.4% vs 34.9% within 1 year).

**Cross-national gaps.** UN World Marriage Data 2019 gives the difference between men's and women's singulate mean age at marriage (SMAM). This is a proxy, not a within-couple gap. The median across 214 countries (2000+) is 3.1 y.

- US 1.7, Japan 1.3, Sweden 2.2, Italy 3.1, India 4.7 (26.1/21.4), Nigeria 7.3, Senegal 8.1.
- Casterline et al. (1986, 29 World Fertility Survey countries): gaps are not explained by random matching, and they track kinship systems and women's roles.
- **Consequence:** the gap kernel is a per-country ledger parameter, e.g. mean ≈ SMAM difference.

**Education.**

- US 2023 (FG3): neither spouse has a BA 44.8%; one has 23.4%; both have 31.8%. That is 76.6% concordant on BA/non-BA.
- Wives are now more often the more-educated spouse: 24% of marriages in 2022, versus 19% in 1972 (Pew).
- Eika, Mogstad & Zafar give the odds of marrying within group relative to random matching:

| Group | early 1960s | 1980 | 2013 |
|---|---|---|---|
| College graduates | 5× | 3× | 2× |
| Adults without a high school degree | 1.6× | 2.6× | 7.2× |

- Schwartz & Mare (2005): homogamy fell 1940–60 and rose 1960–2003.
- Worldwide, women out-educate men in most countries, and hypogamy follows (Esteve et al. 2016).
- **Implementation:** nest an education band inside the union table only when a scenario needs it (§9.1).

**Geography and meeting context.**

- Dutch registers (Haandrikman et al. 2011): half of cohabiting partners lived within 6 km of each other beforehand.
- US heterosexual couples, 2017 (Rosenfeld et al. 2019): 39% met online (2% in 1995), 20% through friends, 11% as coworkers, 7% through family, 3% as neighbours.
- **Consequence:** unions should be mostly within a birth or residence city, with a regional pool for the rest.

**Other homogamy.**

- 17% of US newlyweds in 2015 were intermarried by race or ethnicity (Pew).
- Consanguineous unions (second cousins or closer) and their children are ≈10.4% of world population, concentrated in North Africa, the Middle East and West/South Asia (Bittles & Black 2010). So "no siblings" is the hard rule; the cousin policy should be a per-country parameter.

---

## 5. Life-course event rates

### 5.1 United States

| Event | Value | Population / year |
|---|---|---|
| Median age at first marriage | men 30.8, women 28.4 (2024: 30.2 / 28.6); 1970: 23.2 / 20.8 | CPS MS-2, 2025 |
| Probability of first marriage by 25 / 30 / 35 / 40 | women 34/56/71/76%, men 24/50/66/75% | NSFG 2017–19 |
| Never married | ages 35–44: men 29.4%, women 22.1%; ages 45–54: men 18.5%, women 13.7% | CPS 2023 |
| Ever cohabited (ages 15–49) | women 58.9%, men 54.3%; currently cohabiting 13.9 / 12.0% | NSFG 2017–19 |
| First union type, women 15–44 | cohabitation 48%, marriage 23%, remainder none (≈68% of first unions are cohabitations) | NSFG 2006–10 |
| First premarital cohabitation within 3 years | 40% married, 32% still intact, 27% dissolved; median length 22 months | NSFG 2006–10 |
| First marriage intact at 5 / 10 / 15 / 20 years | women .80/.68/.60/.52; men .81/.70/.62/.56 | NSFG 2006–10 (NHSR 49) |
| Divorce hazard by duration (derived) | women 4.4%/yr (0–5 y), 3.2 (5–10), 2.5 (10–15), 2.8 (15–20) | from NHSR 49 |
| Median duration of first marriages ending in divorce | 8.0 years (to separation: 6.6–6.7) | SIPP 2009 |
| Crude marriage / divorce rate | 6.1 / 2.4 per 1,000 (2000: 8.2 / 4.0) | NCHS 2023 (divorce excludes 5 states) |
| Remarriage | 57% of divorced or widowed have remarried (men 64, women 52); median 3.7–3.8 y after divorce; 40% of new marriages involve a previously married spouse | Pew (ACS 2013); SIPP 2009 |
| Widowed | 65+: women 28.5%, men 11.1%; 75+: women 44.8%, men 17.9% | CPS 2023 |
| ASFR per 1,000 women | 10–14: 0.2; 15–19: 13.1; 20–24: 57.7; 25–29: 91.0; 30–34: 94.3; 35–39: 54.3; 40–44: 12.5; 45–49: 1.1 | NVSR, 2023 |
| TFR | 1.621 (2023), 1.600 (2024, record low) | NVSR |
| Mean age of mother | 1st birth 27.6, 2nd 30.0, 3rd 31.2, all births 29.7 | NVSR 2024 |
| Births to unmarried women | 39.5%. First births: 60.6% to married mothers, 24.0% to cohabiting | NVSR 2024; CPS 2024 |
| Completed fertility and childlessness | women 45–50: 2.00 children, 14.9% childless; women 40–44: 1.92, 18.8% | CPS June 2024 |
| Parity, women 45–50 | 0: 14.9%, 1: 17.8, 2: 35.9, 3: 19.1, 4: 8.2, 5+: 4.1 | CPS June 2024 |
| Interpregnancy interval (add ~9 months for birth interval) | median 24–29 months; 28.9% under 18 months; 4.6% over 120 months | NCHS DB 240, 2014 |
| Living in a parent's home | 18–24: men 58.8%, women 56.4% (includes college dorms); 25–34: men 19.2%, women 13.6% | CPS AD-1, 2025 |
| Distance to parents | Adults with a living parent: 5.9% co-reside with the nearest parent, 59.8% live within 30 miles, 9.2% live 500+ miles away | PSID 2013 |
| Distance to mother (married couples) | median 20–25 miles; P90 1,500 miles | NSFH 1992–94 |
| Multigenerational | 3.8% of households have 3+ generations (ACS 2024). 18% of people live in households with 2+ adult generations (Pew 2021) | different definitions; don't compare |

### 5.2 Contrast countries

| | Sweden | Japan | India |
|---|---|---|---|
| Mean or median age at first marriage | mean: men 37.1, women 35.1 (SCB 2025) | mean: men 31.1, women 29.8 (2024) | median: women 18.9 (25–49), men 24.9 (NFHS-5, 2019–21) |
| Never married around age 50 | — | men 28.3%, women 17.8% (2020; 1970: 1.7 / 3.3) | ages 45–49: women 0.9%, men 2.5% |
| TFR | 1.43 (2024), 1.42 (2025) | 1.15 (2024), 1.14 (2025 provisional) | 2.0 |
| Mother's age at first birth | mean 30.2 | mean 31.0 | median 21.2 |
| Births outside marriage | 57.1% (EU 41.1%) | — | — |
| Divorce | 2.1 per 1,000; mean duration of divorcing marriages 12.4 y | 1.55 per 1,000; 28% of divorces within 5 years of cohabitation | — |
| Leaving home (average age) | 21.9 (Eurostat 2024) | not collected | — |

- Other average leaving-home ages (Eurostat 2024): Italy 30.1, Croatia 31.3, EU-27 26.2.
- India's ASFR peaks at 20–24 (165 per 1,000), versus 30–34 in the US and Japan. Its median birth interval is 32.7 months.
- **Consequence:** each ledger "country" needs its own union timing, gap kernel, parity distribution and leaving-home schedule. The mechanics are identical across countries.

---

## 6. Household composition

**United States (CPS 2025; 134.8 M households, mean size 2.50: 0.54 minors + 1.96 adults).**

| Size | 1 | 2 | 3 | 4 | 5 | 6 | 7+ |
|---|---|---|---|---|---|---|---|
| Share | 29.5% | 34.5% | 15.0% | 12.3% | 5.5% | 2.1% | 1.1% |

| Type | Share of households |
|---|---|
| Married couple, with own children under 18 | 17.3% |
| Married couple, without own children under 18 | 29.3% |
| Other family, female householder (single mother with own children under 18: 5.7%) | 11.7% |
| Other family, male householder (single father with own children under 18: 1.9%) | 5.5% |
| One person | 29.5% |
| Other nonfamily (roommates) | 6.7% |

- **Adults 18+ (2023).** With spouse 50.1%, with unmarried partner 7.7%, alone 14.8%, child of the householder 11.6%, with other relatives 12.3%, with non-relatives 3.5%.
- **By age of householder (2023).**
  - One-person share: 24.8% under 35, 38.3% at 65–74, 50.9% at 75+.
  - Married-couple share at 25–34: 39.4%.
  - ≈28% of people aged 65+ live alone.
- **Single parents.** Single fathers are 25% of one-parent families with own children under 18 (derived, FM-1). Use this as the post-divorce custody proxy.

**Contrast countries.**

- **Japan, 2020 census.**
  - Mean household size 2.21; one-person households 38.0%.
  - Couple with children 25.1%; couple only 20.1%; lone parent 9.0%; other, including three-generation, 7.7%.
- **Sweden (EU-SILC 2025).** Mean size 2.0; one adult without children 44.0%.
- **India (NFHS-5).**
  - Mean size 4.38; one-person households 5.1%.
  - Extended-family households 39.2%, including three-generation 30.2%.
- **Implementation.** India-like extended households need a co-residence rule where married sons stay with parents: patrilocal. Sweden and Japan need early exit and solo living. It is the same rule engine with different parameters (inferred).

---

## 7. Demographic consistency without a simulation pass

### 7.1 How demographers make pyramid, fertility and mortality agree

- **Cohort-component projection (Leslie).**
  - `n(t+1) = A_t n(t)`, with `A_t = U_t + F_t` (Caswell eq. 1–2), plus net migration.
  - Births = Σ_x F_x,t n_x,t (female-dominant), split by the sex ratio at birth.
  - Cohort sizes are **outputs** of the fertility, mortality and migration inputs, never free parameters.
- **Stable population theory.**
  - Fixed rates converge any population to the stable distribution `w`, growing at rate λ.
  - Euler–Lotka: `1 = Σ_a λ^{−a} l(a) b(a)`, or continuously `1 = ∫ e^{−ra} l(a) b(a) da`.
  - R₀ is the dominant eigenvalue of `F(I−U)⁻¹`, and `r ≈ ln R₀ / T`.
  - With US 2023 female rates, λ = 0.9917 and T ≈ 29.8 (computed). A closed world held at 2023 rates shrinks 0.8%/yr, so the ledger needs time-varying rates and/or immigration.
- **Two-sex closure.** Births come from union plans, and unions come from the shared table (§3). Female-dominant fertility and fathers then agree automatically.

### 7.2 Mortality for the "alive at t" predicate

US 2023 period life table (NVSR 74-6).

- e₀ = 78.4 (men 75.8, women 81.1); e₆₅ = 19.5.
- Of 100,000 born: 99.4% survive to 1, 98.9% to 20, 83.1% to 65, 42.7% to 85 and 2.0% to 100.

| x | l(x) men | l(x) women | q(x) men | q(x) women |
|---|---|---|---|---|
| 0 | 100,000 | 100,000 | .00602 | .00513 |
| 20 | 98,705 | 99,108 | .00118 | .00044 |
| 40 | 94,685 | 97,333 | .00318 | .00166 |
| 50 | 91,067 | 95,245 | .00518 | .00306 |
| 60 | 84,464 | 91,038 | .01146 | .00703 |
| 65 | 79,011 | 87,354 | .01620 | .01014 |
| 70 | 71,877 | 82,295 | .02264 | .01476 |
| 80 | 51,062 | 64,735 | .05416 | .04049 |
| 90 | 19,252 | 30,673 | .15990 | .12854 |
| 100 | 1,069 | 2,826 | — | — |

**Gompertz fit (computed, ages 40–95).**

- Men: μ(x) ≈ 8.3·10⁻⁵ e^{0.083x}. Women: μ(x) ≈ 3.2·10⁻⁵ e^{0.091x}.
- The hazard doubles every 7.6–8.4 years. The median age at death is ~81 for men and ~85 for women.

**Recommendation.**

- Death age is a nested quota cell drawn from **cohort** mortality, i.e. the diagonal through period tables.
- Past years: historical NCHS/HMD tables. Future years: a drift from the 2023 table (e.g. Lee–Carter-style; not re-verified here).
- If only parameters are wanted, use Gompertz–Makeham per (sex, year) plus an infant term (q₀ ≈ 0.5–0.6% today).

### 7.3 Guaranteeing consistency with per-block counts plus bijections

1. **Ledger forward pass.** Run once, in expected values, then integerize so that every margin is a sum of integer cells:
   - `U[c][y_m][y_f][band]` from the harmonic-mean function on unpartnered survivors.
   - `E[c][y_f][y]` from union fertility plans plus non-union fertility.
   - `N[c][y] = Σ_{y_f} E[c][y_f][y] + I[c][y]`.
2. **Block quota trees.** Each block's quota tree (§9.3) is carved from those integer margins. Both sides of every matching see identical counts.
3. **Consequence.** "People born in y have parents from plausible earlier cohorts" holds by construction. The mother's cohort `y_f` is a sub-range, and `y − y_f` stays within the ASFR support. The father's cohort comes from the union table.

**What partial matching costs.**

- **Founders.** Blocks born before ~Y0+45 lack in-world parents, and grandparents and cousins stay truncated until ~Y0+90. Put Y0 about 100 years before the earliest era scenarios use. Murphy's SOCSIM runs used 600-year horizons from 4k–10k founders.
- **Immigrants and unknown fathers.** Represent them as explicit `I` and non-union-birth shares. Kin loss compounds with kin degree, roughly (1−p)^degree (inferred).
- **Hash-sampled counts.** If a count feeding a matching is hash-sampled rather than quota-assigned, realized totals differ from the ledger by O(√N) (≈0.25% at N ≈ 1.7·10⁵). They also force an enumerate-and-cache step to find the r-th event. Quota trees avoid both.

---

## 8. Realism targets (property tests)

"Sample" means a statistic over ≥10⁴ sampled ids at the given `t`. Tolerances should come from Internot's own ledger rates via the Caswell oracle (§1.3). The literal values below are sanity bands for a US-parameterized world at t ≈ 2023–2025.

| # | Metric | Target | Source |
|---|---|---|---|
| 1 | Partner reciprocity; mother↔child and father↔child reciprocity | exactly 1.0 | construction |
| 2 | Partners who are siblings, half-siblings or parent/child | 0 | SOCSIM `marriage_allowable` |
| 3 | Mother alive at child's birth; mother aged 12–50 at the birth | 100% | construction; ASFR support |
| 4 | Life expectancy e₀ (period, derived from sampled deaths) | men 75.8 ± 1, women 81.1 ± 1 | NVSR 74-6 |
| 5 | Survival from birth to 65 / 85 | 83% / 43% | NVSR 74-6 |
| 6 | P(mother alive) at 30 / 50 / 60 | ≈0.92 / 0.63 / 0.33 | Caswell model, US 2023 rates (computed) |
| 7 | Women 45–64 with ≥1 living parent | ≈60% | PSID 2007 |
| 8 | Living grandparents at 0 / 20 / 30 | ≈3.4 / 2.0 / 1.0 | computed |
| 9 | Siblings ever born, child's-eye; share of only children | ≈1.9; ≈9% | derived, CPS 2024 parity |
| 10 | Completed fertility and childlessness, women 45–50 | 2.0; 15% | CPS 2024 |
| 11 | Parity distribution 0/1/2/3/4/5+, women 45–50 | 15/18/36/19/8/4% | CPS 2024 |
| 12 | TFR (2023); mean age at first birth | 1.62; 27.5–27.6 | NVSR |
| 13 | Birth interval (IPI + 9 months): median; share with IPI < 18 months | ≈33–38 months; ≈29% | NCHS DB 240 |
| 14 | Births outside a union | 15–40% depending on definition (39.5% unmarried; about 16% neither married nor cohabiting) | NVSR 2024; CPS 2024 (inferred split) |
| 15 | Median age at first marriage | men ≈30.5, women ≈28.5 | Census MS-2 |
| 16 | Never married at 45–54 | men 18.5%, women 13.7% | CPS 2023 |
| 17 | First marriage intact at 10 / 20 years | ≈0.69 / 0.54 | NSFG (NHSR 49) |
| 18 | Divorce hazard, first 5 years vs years 10–20 | ≈4.3%/yr vs ≈2.5%/yr | derived, NHSR 49 |
| 19 | Divorced or widowed who have remarried | ≈57% (men 64, women 52) | Pew 2014 |
| 20 | Widowed at 65+ / 75+ | women 28.5 / 44.8%; men 11.1 / 17.9% | CPS 2023 |
| 21 | Spousal age gap: within 1 y; husband 2+ older; wife 2+ older | 35% / 50% / 14%; mean +2.1, SD ≈ 5 | CPS FG3 2023 |
| 22 | Same-sex share of coupled households | ≈1.5% | ACS 2019 |
| 23 | BA/non-BA concordance of spouses | ≈77% | CPS FG3 2023 |
| 24 | Household size shares, 1/2/3/4/5+ | 29.5/34.5/15.0/12.3/8.7% | CPS HH-4 2025 |
| 25 | Household type shares: married couple / one-person / single mother / single father / other nonfamily | 46.6 / 29.5 / 5.7 / 1.9 / 6.7% | CPS 2025 |
| 26 | Adults living with spouse / partner / alone | 50.1 / 7.7 / 14.8% | CPS AD-3 2023 |
| 27 | Living in a parent's home, 18–24 / 25–34 | ≈57% / ≈16% | CPS AD-1 2025 |
| 28 | 65+ living alone | ≈28% | CPS 2023 (derived) |
| 29 | 3+ generation households | ≈3.8% | ACS 2024 |
| 30 | Adults with a living parent: nearest parent within 30 miles or co-resident | ≈66% | PSID 2013 |
| 31 | Population age pyramid at t vs ledger `Σ N·l` | exact (integer) | construction |

---

## 9. Construction recommendations

### 9.1 Where the working construction is naive

| Working idea | Why it's naive | Fix |
|---|---|---|
| Keyed bijections permuting only within a (city, birth year) or (city, age band) block | A permutation of one block relates people only to the **same** block. Parents and children are never in the same birth-year block, and partners rarely are (gap SD ≈ 5 y). Blocks defined on *current* age change membership as `t` moves, which breaks relations over time. | Relations are **matchings between blocks**. Blocks are keyed on birth cohort. A shared count table sizes each rank range. |
| Partner involution σ(x) = π⁻¹(π(x) XOR 1) | (a) Everyone is partnered, once, for life. Reality: 13.7–18.5% never married at 45–54; Japan 28% of men at 50; about half of first marriages dissolve; widowhood; remarriage. (b) A sex-blind π makes ~50% of pairs same-sex, versus ≈1.5% observed. (c) The gap is 0, or uniform inside a band and forbidden across band edges. (d) An odd block leaves a fixed point. | A union table with rank-range pairing (§9.4). XOR-1 is correct in exactly one place: pairing *inside* one same-sex, same-cohort union range, whose size is even by construction. |
| Per-sex marriage rates or per-block pairing counts | The two-sex problem (§3): the union count between two cohorts is one number, and per-sex rates overdetermine it. | One shared table from a two-sex marriage function. |
| Households own child slots; people partially matched into them | Independently drawn child counts can't equal the fixed block sizes. A household is the wrong unit, since births span ~15 years and multiple unions. | Children link to **mothers' birth events**. Child-block size is *defined* as the event count plus `I`. Households are derived afterwards (§9.6). |
| Partial matching as general slack | Kin decay ~ (1−p)^degree. At p = 20%, about half of cousins vanish. | Keep slack explicit: founders, immigrants and non-union births with an unknown father. Everything else is exact. |
| Independent permutations for everything | Every relation is independent of every attribute. Real spouses share education and mortality risk, and fertility is correlated across generations. | Add a correlation by **nesting** that attribute into the quota tree (e.g. education band inside the union table). Each stratum multiplies ledger size, so add them only when a scenario needs one. |
| No kin check | Random pairing within a union range hits a sibling with probability ≈ (#sisters in range)/(range size), which is ~10⁻⁶–10⁻⁵. Over ~10⁹ unions that is ~10³–10⁴ sibling couples (inferred). | Deterministic pair-swap repair (§9.4). |

### 9.2 Id space and ledger (answers the turnover constraint)

- **Ids.** An id is anyone born in `[Y0, Y1)`, e.g. 1850–2150.
  - Block = (birth city c, birth year y); `id = base[c][y] + raw`, with blocks contiguous.
  - "Born in c in y" is one id range, which gives `where_range` pushdown.
  - Birth date = y + a hash-derived day with seasonality.
  - Alive ⇔ `birth ≤ t < death`.
  - Minors are ids with age < 18. Age is `t − birth` everywhere, so there are no buckets and no gaps.
- **Capacity.**
  - Alive at t ≈ B·e₀ for B births/yr; total ids = Σ B(y).
  - A 300-year window with e₀ ≈ 78 has ≈26% of ids alive (≈1.1 B of 2³²); 250 years gives ≈31%.
  - Choose the window first, then scale B by city. Real cities differ in size, so give blocks unequal sizes: this is why `city` should leave the fixed bit field.
- **Ledger contents.**
  - `N[c][y]` (derived after the founder era).
  - `E[c][y_f][y]`: ≈64 × 300 × 35 ≈ 0.7 M cells.
  - `U[c][y_m][y_f][band]`: ≈64 × 300 × 25 × 8 ≈ 4 M u32 cells ≈ 16 MB, or less with regional pools.
  - `I[c][y]`, plus sex split and same-sex tables.
  - Inner per-block quotas (death ages, plan cells) are recomputed on demand from block margins and memoized.
- **Integerization.** Integerize cross-block cells first and derive every margin as a sum of integer cells. Version the ledger with the seed, because any parameter change reshuffles ids.
- **Slot-layout tension.** Today's `(industry:6, city:6, workplace_seed:8, member_idx:12)` bakes life-course-varying facts into a birth-fixed id. Under this design:
  - `city` becomes birth city (the block).
  - Workplace becomes a time-varying matching of the employed population into workplace slots, per city per year.
  - Raise this before implementation; it touches CLAUDE.md's people-slot invariants.

### 9.3 Coordinates and quota trees (per block, local, pure)

- **Coordinates.** For block B and coordinate key k (`life`, `parent`, …), π_{B,k} is a permutation of `[0, N_B)`.
  - Build it as a 4-round Feistel on ⌈log₂N⌉ bits (round function = `procedural_core::hash`) with **cycle-walking** into `[0, N)` (Black & Rogaway 2002).
  - Padding to the next power of two keeps expected walks < 2; the inverse runs the rounds backwards.
- **Quota tree** on the `life` rank line, outermost first:
  1. **Sex**, from the ledger split.
  2. **First-union cell** (partner cohort, start band), sized from `U`. The remainder is never partnered.
  3. **Union-level plan cell**: parity, first-birth age, spacing, twins, divorce duration. It is keyed by union offset, so both partners compute it identically.
  4. **Own death-age cell**, conditional on surviving past union start and the last planned birth. The never-partnered get the *residual* of the cohort life table, which is where all childhood deaths correctly land.
- **The rule.** Counts are closed-form only along nesting. Never intersect two independent permutations in a count. Anything that must match across blocks goes in the tree; names, personality and day-level jitter stay `hash(id, key)`.

### 9.4 Partners

```text
partner_of(x, t):
  (c, y, raw) = decode(x)                         // prefix sums over ledger, O(log)
  r           = π[c,y,life](raw)
  (cell, i)   = locate_union_cell(c, y, r)        // binary search in quota tree
  if cell is NEVER: return None
  (y_m, y_f, band) = cell.key                     // canonical, identical on both sides
  i'          = repair(c, y_m, y_f, band, i)      // forbidden-kin pair swap, see below
  r'          = cell_start(c, other_cohort, cell.key) + i'
  x'          = base[c][other_cohort] + π[c,other_cohort,life]⁻¹(r')
  u           = UnionKey(c, y_m, y_f, band, male_side_offset(i, i'))  // dates, plan, divorce: f(u)
                                                  // (same-sex ranges: min of the two offsets)
  return x' if u.start ≤ t < min(u.divorce, death(x), death(x')) else None
```

- **Reciprocity.** Both sides compute the same cell key and the same offset, then invert the other block's permutation. A Python prototype of exactly this scheme (60 cohorts, 59k people, 21k couples, Gaussian gap kernel) passed an exhaustive `partner_of(partner_of(x)) == x` check with a 2.0-year mean gap.
- **Repair.** Offsets pair up as `(i, i^1)`. If either pair would join siblings, half-siblings or a parent and child, swap their partners. Both sides evaluate the same two-pair predicate, so the swap is reciprocal.
  - Residual conflicts run ~10⁻¹² per union. Iterate once more over `(i, i^2)` if paranoid.
  - This requires `mother_of` for four people: O(log) each.
- **Same-sex unions.** A small table `U_ss`. Within one cohort and sex, the range is even-sized and pairs `2j ↔ 2j+1` (this is XOR-1). Across cohorts, use the same mirrored ranges as above.
- **Re-partnering (phase 2).** Dissolved unions feed a pool per (city, band) whose composition is closed-form from the plan cells. Pair the pool with a second harmonic-mean table: SOCSIM's queue, aggregated. Parity-dependent remarriage is a further nesting.
- **Geography.** Most unions are within the city; the rest go through a regional pool. The union's residence city is a union attribute, so children are born where the union lives.

### 9.5 Parents, children, siblings, births over time

- **Birth events.** For mother cohort `y_f` and year `y`, events are ordered by (union cell, plan cell, offset), then non-union fertility cells, whose father is unknown or comes from a `random_father` table. A woman's *j*-th birth has a closed-form event index `e`. Cell counts sum exactly to `E[c][y_f][y]`.
- **Child side.** In child block (c, y), the `parent` rank line is cut into sub-ranges by mother cohort (sizes `E[c][·][y]`), followed by the `I` range.

```text
mother_of(x):  (c,y,raw) = decode(x);  r = π[c,y,parent](raw)
               (y_f, e) = locate_mother_cohort(c, y, r)      // None if r in I-range
               (cell, o) = locate_event(c, y_f, y, e)
               return base[c][y_f] + π[c,y_f,life]⁻¹(cell.lo + o)
father_of(x):  the other partner of that event's union (None for non-union births)
children_of(m): for each planned birth (y, e) in m's plan cell:
               base[c][y] + π[c,y,parent]⁻¹(start[c][y][y_f] + e)
siblings(x):   children_of(mother_of(x)) ∪ children_of(father_of(x)) − {x}
```

- **Guarantees.**
  - The mother is alive at each birth, because death cells nest after the plan end.
  - The mother's age is `y − y_f`, within the ASFR support; the father's age comes from the union table.
  - Twins are two events in one year of one plan cell.
  - The sibling distribution follows the parity catalog.
  - Children born after the father's death inside a plan are rare, because male death nests after the plan end too. The price is a slight selection: fathers cannot die young mid-plan.
- **"Births over time"** needs no simulation at query time: the ledger fixes all counts, and every event is a closed-form rank. This is the cohort-component projection with individuals as rank slots.

### 9.6 Households (derived views)

- **Membership rules at t, per person, in order:**
  1. Before leaving home: the mother's current union household, or the mother alone.
     - Leaving age is hash-drawn from the country's schedule (§5), truncated at own union start.
     - After a divorce, a minor goes to the father with p ≈ 0.25 (FM-1 proxy).
  2. During own union: the union household.
  3. Single adult: alone, or in a roommate group.
  4. Widowed parent at older ages: with a probability by age, lives in one child's household. The child is chosen deterministically, e.g. the eldest living daughter.
- **Reciprocity.** Every rule is evaluated from the *mover's* data. The host household finds its movers by enumerating children and each partner's parents: all O(k log) lookups.
- **Roommates** are the only household type needing a new matching: a per-(city, age band, 2-year epoch) coordinate that chunks single non-coresident adults into groups of 2–3. The target is 6.7% other-nonfamily households.
- **Validation.** Household size and type shares, 65+ living alone, 18–34 living at home and multigenerational share (§8, rows 24–30) emerge from these rules. Tune the four probabilities (leaving age, custody, elder co-residence, roommate uptake) against them.
- **Minors: make them full people.** Under this design they already are ids. A stub can't become the same adult when `now` moves. Views can still hide them (no mail or chat under 13, etc.).

### 9.7 Phasing

- **Phase A.**
  - Ledger with sex, first unions, divorce and one fertility plan catalog.
  - Mortality nesting; `alive_at`; `partner_of`, `mother_of`, `father_of`, `children_of`.
  - Kin-repair swap; per-city marriage markets; no migration.
  - Property tests rows 1–12, 15–21 and 31.
- **Phase B.** Remarriage pool, same-sex tables, regional marriage pools and residence city, immigrant share.
- **Phase C.** Households (§9.6), roommates, multigenerational co-residence, country parameter sets (Sweden, Japan, India).

---

## 10. Sources

Kinship theory and models:
- Goodman, Keyfitz & Pullum 1974, *Theor. Pop. Biol.* 5:1–27 — https://pubmed.ncbi.nlm.nih.gov/1220047/
- Caswell 2019, "The formal demography of kinship: A matrix formulation", *Demographic Research* 41(24) — https://www.demographic-research.org/articles/volume/41/24 (PDF: https://pure.uva.nl/ws/files/42993078/Caswell_2019_formal_demography_of_kinship_DR.pdf)
- Caswell & Song 2021, "III: time-varying rates" — https://www.researchgate.net/publication/353691338_The_formal_demography_of_kinship_III_Kinship_dynamics_with_time-varying_demographic_rates
- Caswell 2022, "IV: Two-sex models and their approximations", *Demographic Research* 47(13) — https://www.demographic-research.org/articles/volume/47/13
- Alburez-Gutierrez, Williams & Caswell 2023, "Projections of human kinship for all countries", PNAS — https://www.pnas.org/doi/10.1073/pnas.2315722120 (full text: https://pmc.ncbi.nlm.nih.gov/articles/PMC10756196)
- DemoKin one-sex reference vignette (Sweden 2015) — https://ivanwilli.github.io/DemoKin/articles/Reference_OneSex.html; CRAN archival notice — https://cran.r-project.org/web/packages/DemoKin/index.html
- Wiemers & Bianchi 2015, PSID parents/children of women 45–64 — https://pmc.ncbi.nlm.nih.gov/articles/PMC4649941/
- Margolis & Verdery 2019, grandparenthood — https://pmc.ncbi.nlm.nih.gov/articles/PMC6667684/
- Verdery & Margolis 2017, kinless older adults — https://www.pnas.org/doi/10.1073/pnas.1710341114 (figures via https://conversableeconomist.com/2018/10/31/kinlessness/)
- Ruggles, "The effects of demographic change on multigenerational family structure" — https://users.pop.umn.edu/~ruggl001/Articles/multidem.pdf

Microsimulation:
- SOCSIM technical notes (rsocsim) — https://github.com/MPIDR/rsocsim/blob/main/socsim_oversimplified.md; https://mpidr.github.io/rsocsim/
- Murphy 2004, "Tracing very long-term kinship networks using SOCSIM", *Demographic Research* 10(7) — https://www.demographic-research.org/volumes/vol10/7/10-7.pdf
- Smith & Oeppen 1993 (CAMSIM) — https://www.semanticscholar.org/paper/Estimating-numbers-of-kin-in-historical-England-Je-Oeppen/562096853e1cf4084825a9d2e0e6e24c59533d28
- Statistics Canada, LifePaths overview — https://www.statcan.gc.ca/en/microsimulation/lifepaths/overview

Two-sex problem, population theory, permutations:
- Schoen 1981, "The harmonic mean as the basis of a realistic two-sex marriage model", *Demography* 18(2) — https://www.jstor.org/stable/2061093
- Euler–Lotka equation — https://en.wikipedia.org/wiki/Euler%E2%80%93Lotka_equation
- Black & Rogaway 2002, "Ciphers with Arbitrary Finite Domains" — https://web.cs.ucdavis.edu/~rogaway/papers/subset.pdf

Mortality and fertility (NCHS):
- United States Life Tables, 2023 (NVSR 74-6) — https://www.cdc.gov/nchs/data/nvsr/nvsr74/nvsr74-06.pdf
- Births: Final Data for 2024 (NVSR 75-2) — https://www.cdc.gov/nchs/data/nvsr/nvsr75/nvsr75-02.pdf; for 2023 (NVSR 74-1) — https://www.cdc.gov/nchs/data/nvsr/nvsr74/nvsr74-1.pdf
- Marriage/divorce rates — https://www.cdc.gov/nchs/fastats/marriage-divorce.htm; https://www.cdc.gov/nchs/data/dvs/marriage-divorce/national-marriage-divorce-rates-00-23.pdf
- Copen et al. 2012, First marriages (NHSR 49) — https://www.cdc.gov/nchs/data/nhsr/nhsr049.pdf; Copen, Daniels & Mosher 2013, First premarital cohabitation (NHSR 64) — https://www.cdc.gov/nchs/data/nhsr/nhsr064.pdf
- NSFG key statistics — https://www.cdc.gov/nchs/nsfg/key_statistics/m-keystat.htm; https://www.cdc.gov/nchs/nsfg/key_statistics/c-keystat.htm
- Interpregnancy intervals (Data Brief 240) — https://www.cdc.gov/nchs/data/databriefs/db240.pdf; NVSR 64(3) — https://www.cdc.gov/nchs/data/nvsr/nvsr64/nvsr64_03.pdf

Census and survey tables:
- CPS MS-2 (age at first marriage) — https://www2.census.gov/programs-surveys/demo/tables/families/time-series/marital/ms2.xls
- CPS 2023 A1, FG3, H2 — https://www2.census.gov/programs-surveys/demo/tables/families/2023/cps-2023/taba1-all.xls; …/tabfg3-all.xls; …/tabh2-all.xls
- CPS historical HH-1, HH-4, HH-6, FM-1, AD-1, AD-3 — https://www2.census.gov/programs-surveys/demo/tables/families/time-series/households/hh1.xls; …/hh4.xls; …/hh6.xls; …/families/fm1.xls; …/adults/ad1.xls; …/adults/ad3.xls
- CPS June 2024 Fertility supplement — https://www.census.gov/data/tables/2024/demo/fertility/women-fertility.html
- Kreider & Ellis 2011 (SIPP 2009, P70-125) — https://www2.census.gov/library/publications/2011/demo/p70-125.pdf
- ACS B11017 multigenerational (2024) — https://api.censusreporter.org/1.0/data/show/latest?table_ids=B11017&geo_ids=01000US
- Same-sex couples — https://www.census.gov/library/stories/2026/04/same-sex-couples.html; https://www.census.gov/newsroom/press-releases/2021/same-sex-couple-households.html; https://www.census.gov/library/stories/2024/11/same-sex-tables-2023.html
- Gallup LGBTQ+ identification — https://news.gallup.com/poll/702206/lgbtq-identification-holds.aspx
- Pew: cohabitation (2019) https://www.pewresearch.org/social-trends/2019/11/06/marriage-and-cohabitation-in-the-u-s/; remarriage (2014) https://www.pewresearch.org/social-trends/2014/11/14/four-in-ten-couples-are-saying-i-do-again/; multigenerational (2022) https://www.pewresearch.org/social-trends/2022/03/24/the-demographics-of-multigenerational-households/; education (2010) https://www.pewresearch.org/social-trends/2010/01/19/women-men-and-the-new-economics-of-marriage/; (2023) https://www.pewresearch.org/social-trends/2023/04/13/in-a-growing-share-of-u-s-marriages-husbands-and-wives-earn-about-the-same/; intermarriage (2017) https://www.pewresearch.org/social-trends/2017/05/18/intermarriage-in-the-u-s-50-years-after-loving-v-virginia/
- Choi et al. 2020, parent–child distance (PSID 2013) — https://pmc.ncbi.nlm.nih.gov/articles/PMC7537569/; https://pmc.ncbi.nlm.nih.gov/articles/PMC7785112/
- Compton & Pollak 2013 (NSFH distances) — https://docs.iza.org/dp7431.pdf

Assortative mating:
- Eika, Mogstad & Zafar (NBER w20271) — https://www.nber.org/system/files/working_papers/w20271/w20271.pdf
- Schwartz & Mare 2005 abstract — https://eutils.ncbi.nlm.nih.gov/entrez/eutils/efetch.fcgi?db=pubmed&id=16463914&rettype=abstract&retmode=text
- Esteve et al. 2016, "The end of hypergamy" — https://pmc.ncbi.nlm.nih.gov/articles/PMC5421994/
- Rosenfeld, Thomas & Hausen 2019, PNAS — https://pmc.ncbi.nlm.nih.gov/articles/PMC6731751/
- Haandrikman et al. 2011 (6 km) — https://api.crossref.org/works/10.1111/j.1467-9663.2010.00642.x
- Casterline, Williams & McDonald 1986 abstract — https://api.openalex.org/works/doi:10.1080/0032472031000142296
- Bittles & Black 2010 — https://pmc.ncbi.nlm.nih.gov/articles/PMC2868287/; Hamamy 2012 — https://pmc.ncbi.nlm.nih.gov/articles/PMC3419292/
- UN World Marriage Data 2019 — https://www.un.org/development/desa/pd/sites/www.un.org.development.desa.pd/files/undesa_pd_2019_wmd_marital_status.xlsx

International:
- UN Household Size and Composition 2022 — https://www.un.org/development/desa/pd/sites/www.un.org.development.desa.pd/files/undesa_pd_2022_hh-size-composition.xlsx
- Japan 2020 census — https://www.stat.go.jp/english/data/kokusei/2020/summary/pdf/all.pdf; MHLW vital statistics 2024/2025 — https://www.mhlw.go.jp/toukei/saikin/hw/jinkou/geppo/nengai24/xls/R6hyou.xlsx; https://www.mhlw.go.jp/toukei/saikin/hw/jinkou/geppo/nengai25/xls/R7toukeihyou.xlsx; IPSS lifetime-unmarried — https://www.ipss.go.jp/syoushika/tohkei/Popular/P_Detail2025.asp?fname=T06-23.htm
- Eurostat API (yth_demo_030, demo_find, demo_nind, ilc_lvph01/02) — https://ec.europa.eu/eurostat/api/dissemination/statistics/1.0/data/
- Statistics Sweden API — https://api.scb.se/OV0104/v1/doris/en/ssd/BE/BE0101/BE0101L/GiftMedelalder
- India NFHS-5 via DHS API — https://api.dhsprogram.com/rest/dhs/data?surveyIds=IA2020DHS

**Not verified (flagged in text):**
- Choo–Siow functional form.
- Lee–Carter as the specific mortality-trend choice.
- The sex ratio at birth of 1.05 (assumed).
- The same-sex share of coupled households after 2019.
- The ~10³–10⁴ sibling-couple estimate and the (1−p)^degree kin-loss rule (back-of-envelope inferences).
