# Two-sex marriage markets and microsimulation: what a mean-field ledger keeps and loses

**Date:** 2026-10-01
**For:** the memory problem (AGENTS.md, "The memory problem: overnight research"). Today the demographic ledger is computed year by year from realized integer pools and kept in memory (1.31 GB of layouts for `us`, 17M people). The redesign under study:
- (a) compute a deterministic **mean-field** (expected-value) two-sex projection once and store it compactly;
- (b) draw each block's life courses top-down from that schedule, as an exact integer partition of the block;
- (c) enforce the two-sex constraint per market-year by matching that year's women's and men's **seats**, carrying unmatched seats in a small per-market queue.

**Question:** what does demography know about two-sex models, about how microsimulations balance partners, and about the realism lost when partners are chosen from expected rather than realized pools? Does any approach generate individual-level kin on demand?
**Builds on:** `2026-09-29-kinship-and-households.md` (GKP, Caswell, SOCSIM basics, the two-sex problem in brief; not repeated here) and `2026-09-30-repartnering-exactness-problem.md`.
**How it was gathered:** one pass by hand plus two parallel literature passes (microsimulation matching; marriage squeezes and stochastic two-sex theory), using WebSearch until the session's budget ran out, then WebFetch, OpenAlex, Europe PMC, PubMed, arXiv, GitHub and Wayback copies. Many publisher sites (Springer, JSTOR, Duke, Persée full texts, ScienceDirect) refused scripted access, so several classics are abstracts only. Three small computations back §3 and §5.
**Markings:** [read] full text or the relevant section read (sometimes a working-paper version, noted); [abs] abstract, metadata or search snippet only; [comp] computed here, scripts in `scripts/2026-10-01-two-sex/`; (inferred) reasoning, not a source. "Not found" means looked for and not reached.

## Summary

1. **The two-sex problem has many deterministic solutions, and all of them give an expected union matrix without simulating realized pools.** Harmonic and other Hölder means, IPF to one-sex margins with a harmonic-mean total (Matthews & Garenne 2013, which is nearly today's `solve_market`), Pollak's birth matrix–mating rule, and Choo–Siow (2006). Macro household projections (LIPRO) and every projection of the China and India squeezes found are mean-field.
2. **Choo–Siow is the best-behaved candidate for (a).** Its union matrix is μ_ij = Π_ij √(μ_i0 μ_0j), so a market-year is stored as two vectors of singles. Its equilibrium is unique and smooth in the pools, with real competition across classes (Decker et al. 2013), and Galichon & Salanié's IPFP solves it at IPF cost. The harmonic mean has no cross-class spillovers (Choo & Siow fn. 6) and fit US census decades poorly (Martcheva & Milner 2001).
3. **Microsimulations handle imbalance in five ways:**
   - **queue and retry:** SOCSIM two-queue, DYNASIM, CBOLT, Zinn's MicMac;
   - **force the counts equal:** DESTINIE 2 draws exactly as many men as women; IRELAND and NEDYMAS resize pools;
   - **female dominance:** SOCSIM one-queue, MOSART;
   - **create the spouse (open models):** LifePaths;
   - **inflate rates to compensate:** DYNASIM3, SimPaths.

   Reported imbalances: DYNASIM3's marriage rates came out about **5%** below the rates sent in. In Zinn's Netherlands run, about **10%** of seekers needed a longer search, and 5,397 seekers never matched against 40,585 unions in 17 years. SOCSIM's two-queue rates are "seldom accomplished".
4. **Greedy matching makes artifacts.** CORSIM and DYNACAN's best-pair-first matching put 73% of first marriages at a 1-year gap and 3.3% at 20+ years (census: 0%), because "after the 'best' marriages are made, there are only relatively 'bad' matches left". Fitting a kernel by IPF and rounding it, as Internot does, avoids this. Carried seats must go back through the kernel, not be paired as leftovers.
5. **Real squeezes moved who marries little and whom and when a lot:**
   - **WWI France:** women single at 50 went from 11% to 12%, and inter-cohort marriages absorbed 52–64% of the shock.
   - **WWII USSR:** sex ratios as low as 0.60 still left only 4–5% of women never married.
   - **US, 1951–1978:** spouse ages moved by about half a year.
   - **The exception that matters:** US marriage rates of *both* sexes fall in large cohorts, which two-sex functions get wrong for men (Bronson & Mazzocco 2024).
6. **Mean field against realized pools.** A law of large numbers holds for two-sex branching processes, and in finite populations the expected matches per head are at most the mean field (Fritsch et al. 2022). [comp]: with Poisson pools of n singles per side, the harmonic mean loses about 1/(2n) of its unions: 5% at n = 10, 0.5% at n = 100. At Internot's block sizes the mean field is the realized dynamics; in thin cells it overstates unions, which is the right choice because those cells are stratification artifacts, not isolated markets.
7. **Exact top-down partitions leave only rounding imbalance, and its size depends on the number of cells, not the market's size** [comp]. The per-year standard deviation is √((k_w + k_m)/6) seats for k cells per side.
   - **Rounding:** rounding each cell independently each year makes the carry a random walk (mean 30 seats, growing like √T at 40 cells per side). Rounding cumulatively along time keeps it bounded by the number of cells (mean 2.8, p99 9).
   - **Market size:** the carried share is 0.3% of a 1,000-union market but 28% of a 10-union one.
   - **Thin markets** need coarser cells and an overflow tier (SimPaths matches regionally, then nationally) before a carry.
8. **Kin on demand: nothing found.** Formal kinship models give means and, since Caswell (2024), variances, but "for the study of interactions … microsimulations are not only possible but essential", and pairing is the interaction. Kinship microsimulations are small: SOCSIM populations of up to 10,000 over 600 years, hours per run; LIAM2's MIDAS used 4.4 GB for 2.2M people. No approach was found that serves reciprocal individual kin from (seed, id) without a stored population.

---

## 1. The two-sex problem

### 1a. The problem

Male and female rates imply different numbers of unions as soon as the age–sex composition of the unpartnered moves (a baby boom, war deaths, migration, the sex ratio at birth). The count of unions between men of class i and women of class j is one number, so a model needs a **marriage (matching) function** μ_ij = M_ij(m, f) of the two pools m (men by class) and f (women by class), not two sets of one-sex rates.

- SOCSIM's documentation states the trade-off in its plainest form [read]: "one can meet *at most* two of the following three marriage market constraints: female age specific nuptiality rates; male age specific nuptiality rates; distribution of spousal age differences."
- Choo & Siow (2006, p. 177) write the accounting constraints every function must satisfy [read]: Σ_i μ_ij + μ_0j = f_j, Σ_j μ_ij + μ_i0 = m_i, all μ ≥ 0, where μ_i0 and μ_0j are the unmarried.

### 1b. Desiderata

The properties below recur in the literature. The original lists (McFarland 1972; Pollard 1974, 1977; Schoen 1981) were **not read** here; the wording follows the sources that were.

| Property | Meaning | Where read |
|---|---|---|
| Heterosexuality / accounting | no unions when either pool is empty; μ_ij ≤ min(m_i, f_j); the constraints above | Choo & Siow 2006 p. 177 [read] |
| Homogeneity of degree 1 | doubling both pools doubles unions (rates depend on composition, not scale) | Shyu & Caswell 2018 eq. 7 [read]; Choo & Siow's function is homogeneous of degree 0 in pools *and* marriages jointly, so it has "no scale effect" (p. 181) [read] |
| Monotonicity | more available men or women never lowers total unions | in the classic lists (not verified in a source read here); for Choo–Siow, Decker et al. 2013 sign every substitution effect [abs] |
| Competition / spillovers | more men of class k lower the unions of men of class i with any j | Choo & Siow 2006 fn. 6 [read]; Decker et al. 2013 [abs] |
| A well-posed growth model | the projection has an equilibrium growth rate and composition, ideally unique and stable | Pollak 1986, 1990 [abs]; Shyu & Caswell 2018 [read] |

Parlett (1972, "Can there be a marriage function?", in Greville (ed.), *Population Dynamics*, pp. 107–135) is the classic reference on whether one function can satisfy all of these; its content was **not found** (metadata only).

### 1c. The models

| Model | Form | Competition across classes | How the expected union matrix is computed | Source |
|---|---|---|---|---|
| One-sex dominance (female or male) | unions = one sex's rates × its pool | none; ignores the other sex | closed form | history in Shyu & Caswell 2018 [read] |
| Pollard (1948) | coupled Lotka equations: female births to males and male births to females, to reconcile the two growth rates | n/a (no explicit pairing) | linear, closed form | Shyu & Caswell 2018 [read] |
| Kendall (1949) | ODEs for males, females and couples with a nonlinear mating term | in totals only | ODE | Shyu & Caswell 2018 [read] |
| Generalized (Hölder) means: minimum, harmonic, geometric | M = (β f^α + (1−β) m^α)^{1/α}, α < 0 | in totals only, unless nested in an age kernel | closed form per pair of pools | Shyu & Caswell 2018 eq. 6, citing Hadeler 1989 and Iannelli et al. 2005 [read] |
| Keyfitz (1972) | age-structured marriage market "around preferences for mates of different ages", studied "as changes in age distributions change the availability of mates" | yes | projection | Proceedings of the Sixth Berkeley Symposium, vol. 4, pp. 89–108 [abs, search summary only] |
| Fredrickson (1971), Hoppensteadt (1975), Hadeler (1989, 1993) | McKendrick–von Foerster PDEs by age, with pair formation and separation | through the mating function | numerical PDE | Shyu & Caswell 2018 [read]; Hadeler, Waldstätter & Wörz-Busekros 1988: "three nonlinear homogeneous ordinary differential equations … existence and global stability of the bisexual state" [abs] |
| **Schoen (1981) harmonic mean** | μ_ij = α_ij m_i f_j / (m_i + f_j) per age pair (discrete case: "observed and model population rates calculated using the harmonic means of the number of persons in the relevant male and female age groups") | Schoen calls it "fully sensitive to the competitive nature of the 'marriage market'"; Choo & Siow (fn. 6) and the authors they cite (Pollak 1990b; Pollard & Höhn 1993–94; Pollard 1997) say the opposite: variations in m_k or f_g (k ≠ i, g ≠ j) do not affect μ_ij | closed form | Schoen 1981 [abs]; Choo & Siow 2006 fn. 6 [read] |
| **Pollak (1986, 1987, 1990): birth matrix–mating rule (BMMR)** | a mating rule (unions by male × female age from the pools) plus a birth matrix (births per union) plus survival; 1987 adds persistent unions | depends on the mating rule | a deterministic projection; equilibrium (exponential growth with a stable composition) exists under conditions, **can be multiple**, uniqueness and local stability under sufficient conditions | Pollak 1986 *Demography* 23(2):247–259 [abs]; 1987 *TPB* 32:176–187 [abs]; 1990 *JPE* 98(2):399–420 [abs] |
| Shyu & Caswell (2018) | BMMR as a continuous-time matrix model, dn/dt = ⅓(T + B + U) n, harmonic-mean mating | per stage | the "nonlinear model converges to exponential growth with an equilibrium population composition" | *Population Ecology* 60:21–36 [read] |
| **Choo & Siow (2006)** | transferable utility with extreme-value taste shocks: μ_ij = Π_ij √(μ_i0 μ_0j) (eq. 11) | **yes**: the effect of m_r on μ_ij is given by their eqs. (B3)–(B4), and "These derivatives are not zero" (p. 183) | the I × J quadratic system (13) given pools; locally unique (Prop. 1) | *JPE* 114(1):175–201 [read] |
| Decker, Lieb, McCann & Stephens (2013) | proof for Choo–Siow | "an increase in the population of men of any given type … leads to an increase in single men of each type, and a decrease in single women of each type"; also more singles among that type | **unique** equilibrium, **closed-form representation**, smooth dependence on pools, via a strictly convex variational principle | *JET* 148(2):778–792; arXiv 1012.1904 [abs] |
| Galichon & Salanié (arXiv 2021, rev. 2023) | generalizes Choo–Siow (separable unobserved heterogeneity) | yes | **IPFP**: alternate closed-form updates of the singles, √μ_x0 = (√(S² + 4n_x) − S)/2 with S = Σ_y exp(Φ_xy/2) √μ_0y, then the same for women; "The IPFP algorithm is extremely fast" | arXiv 2106.02371v2 §4.2, App. F.1.4, Algorithm 1 [read] |
| Mourifié & Siow (2021) | Cobb–Douglas MMF: Choo–Siow plus peer and scale effects | yes | estimated on US states 1990–2010 | *JOLE* 39(S1):S239–S274 [abs] |
| **IPF with a harmonic-mean total** (Matthews & Garenne 2013) | IPF of an age-preference kernel (normal in the gap) to the one-sex marginals; "the total number of new marriages is found as the harmonic mean of target totals for men and women obtained by applying reference population marriage rates"; an availability factor from the unmarried supply | via IPF across ages | deterministic, per year | *TPB* 88:78–85 and 86–93 [abs] |
| Consistency algorithms in household projections (LIPRO) | one-sex rates per household position, then a "consistency algorithm" forcing equal numbers of men and women into and out of unions each interval; the harmonic-mean version is the one used | in totals | deterministic macro projection | van Imhoff & Keilman 1991 (LIPRO 2.0, NIDI/CBGS 23) [abs, search summary only]; Keilman 1985, "Nuptiality models and the two-sex problem in national population forecasts", *EJP* 1(2–3):207–235 (metadata only) |

Two empirical verdicts:
- On Sweden 1961–64, Schoen's harmonic mean gave "results fairly similar to those of the other methods. None of the two-sex methods do particularly well at predicting the actual distribution of marriages", likely because preferences changed (Schoen 1981 [abs]).
- On US census decades, the differences between marriage functions "are of the same magnitude (or even smaller) than the errors between the projected and real data", and "the harmonic mean function frequently found and used in the literature is a quite poor performer" against others in their family (Martcheva & Milner 2001 [abs]).

### 1d. Which give an expected union matrix without simulating realized pools

All of the models in §1c are deterministic functions of expected pools, so any of them can drive a **mean-field projection**: a recursion n(t+1) = F(n(t)) over expected singles, unions and births, with no Monte Carlo and no integer pools. That is what macro household projections (LIPRO; Matthews & Garenne) do. What differs is how much work one year takes and how much of the matrix can be stored compactly:

- **Hölder means and Schoen's harmonic mean:** closed form per (i, j). The matrix for a year needs only the pools. No cross-class competition.
- **IPF with a fixed total (Matthews & Garenne; the current Internot ledger):** iterative, but the fitted matrix is μ_ij = a_i b_j K_ij, so a year's matrix is recoverable from two scaling vectors and the kernel.
- **Choo–Siow:** the matrix is recoverable from the two vectors of singles alone, μ_ij = K_ij √(μ_i0 μ_0j) with K_ij = exp(Φ_ij/2), and the singles come from Galichon–Salanié's IPFP. Unlike the harmonic-mean-plus-IPF hybrid, it fixes the union *total* and the distribution together, has competition and substitution, and its equilibrium is unique (Decker et al.).
- **BMMR:** the projection can be iterated directly; the analytic content (Pollak) is about the long-run equilibrium, which can be multiple.

None needs realized pools. Realized pools enter only when a model is run as a microsimulation (§2), or when, as in the current ledger, integer caps are applied to the expected solution.

**Where the current ledger sits.** `ledger::solve_market` takes the harmonic mean of the two sides' total *wants* (expected never-partnered or divorced members × one-sex rates), scales each side's wants to that total, fits an IPF over the (birth year, status) kernel, rounds each row by keyed systematic rounding, and then caps by the *realized* available members (`apply_market`). That is Matthews & Garenne's construction plus integer caps, and the caps are what make each year depend on every earlier year's integers.

## 2. How microsimulation models match partners

### 2a. Closed and open models

- **Closed:** "spouse is selected from within the existing population". **Open:** "new individuals are generated" when someone marries (O'Donoghue 2001 [read]).
  - Open models allow parallel runs, but make alignment "non-trivial … most dynamic models in use, utilise a closed model method".
  - Li & O'Donoghue's survey tables (Li 2011, Table 2.3 [read]; 2013 [abs]) list as **open:** DEMOGEN, LifePaths, Melbourne Cohort, MINT, US PENSIM, PSG and the Sfb3 cohort model. Most others are **closed**, including CORSIM, DESTINIE I and II, DYNACAN, DYNASIM I–III, FAMSIM, LIAM, MIDAS, MOSART, NEDYMAS, PENSIM2, SAGE, SESIM and SVERIGE.
- **Why kinship needs closed models** (Murphy 2004 [read]): "A closed model is more complex than open models in which a partner is created when required, but as they do not come with any demographic background, it is impossible to investigate general kinship."
- **Zinn (2012)** [read]: in closed models it "is not realistic to pull an appropriate spouse 'out of the hat'".

### 2b. Model by model

| Model | Closed / open | Matching | Unmatched seekers | Reported imbalance or artifacts | Source |
|---|---|---|---|---|---|
| **SOCSIM** (Berkeley; MPIDR rsocsim) | closed; no same-sex unions | **two-queue**: both sexes search by their own rates and wait in a queue if no suitable spouse is waiting. **One-queue**: only women have rates; "the lucky bride chooses the best match from *all* living unmarried males". Greedy score: an age-gap kernel (default peak 36 months, slope ratio 2, bounds ±120 months) or a target normal distribution of gaps; no closer than cousins | two-queue: wait in the queue "subject to the risk of other events"; one-queue: men "marry at the convenience of females" | two-queue: "the specified nuptiality rates are seldom accomplished". One-queue: female rates "generally met", slower in large populations. "Large mean age differences are difficult to achieve … unless a large fraction of people never marry" | rsocsim technical notes [read]; Murphy 2004 [read] |
| **MicMac** (Zinn) | closed | continuous-time market; ±0.5-year search window, about 120 candidates, logit compatibility, both sides' random aspirations must be met, and aspiration decays after rejections | (A) extend the search window (used); (B) drop the event; (C) import a spouse or emigrate the seeker, which "spoils the representativeness" if overused | Netherlands 2% sample (139,048 men, 134,910 women), 2004–2020: 40,585 unions; "the searching period of approximately ten percent of all seekers had to be extended"; 5,397 entered the market and never found a spouse, more often at older ages | Zinn 2012 [read] |
| **LIAM2 / MIDAS** | closed (immigrants cloned) | each member of set 1, in `orderby` order, takes the best-scoring unmatched member of set 2; `orderby='EDtM'` matches "unusual individuals" first; optional random `pool_size` | "the surplus of the largest set is ignored" | not found; marriages aligned to projections by sorting | LIAM2 guide §6.5.8 [read]; Kirn & Dekkers 2023 [read] |
| **LifePaths** (Modgen) | **open** | "a non-dominant spouse will be created whenever it is decided that a union formation will occur", with age and education from a pre-built "spouse market" or generated until one fits | none: there is always a spouse | n/a; non-dominant people are left out of most tables | StatCan overview [read via Wayback] |
| **DYNASIM3** (Urban Institute) | closed | random order within race × age × education cells; acceptance falls with the age and education distance; best of 10 tries (20 at 35+) | returned to the single pool, retry next year | "The average discrepancy between the rates going into the market and the rates coming out… was estimated in a trial simulation as 5 percent over the 1993–2003 period. DYNASIM3 applies this factor to each marriage probability to align marriage rates to the targets" | Favreault & Smith 2004 [read]; Perese 2002 [read] |
| **CORSIM, DYNACAN, POLISIM, SVERIGE** | closed | rates aligned so the pools "are the right size", then greedy best-pair-first ("stable"); DYNACAN later used a stochastic "dartboard" ∝ compatibility | match until one sex runs out; the leftovers' fate not found | greedy: ~73% of first marriages with the husband exactly 1 year older, 3.3% with 20+ years (census 0%), spouses' earnings correlation 0.32–0.56 against 0.13–0.17. Dartboard: the 1-year spike mostly fixed, but more than 15% of second marriages with a husband 20+ years older | Bouffard, Easther, Johnson, Morrison & Vink 2001 [read] |
| **CBOLT** (CBO) | closed; immigrants matched among themselves in their first year | random order; each man accepts a woman with probability p/max p from SIPP-estimated logits (age-gap splines, education, earnings, marriage order) | "cuts marriage candidates from their queue if the sizes of the pools are unequal … sent back to the general population that will be at risk of marriage again in the following year" | 1-year gap ≈15% as in SIPP; remarriage spike at 20+ years more than twice SIPP's; unmatched share not reported | Perese 2002 [read]; CBO 2009 [read] |
| **SimPaths / JAS-mine** | closed, opposite-sex | greedy global (all pairs sorted by score), "matching individuals within regions, but if the sufficient quantity and/or quality of matches cannot be achieved, matching is performed nationally"; alternative IPF over types | unmatched; partnership probit intercepts re-aligned | "under-representation of partner couples", no magnitude | Bronka et al. 2023/2025 [read]; JAS-mine source [read] |
| **DESTINIE 2** (INSEE) | closed | women drawn by their own logits; **men drawn to exactly the women's number** by alignment-by-sorting; each woman takes the nearest man (age gap, end of schooling) | age gap ≥ 21: the woman stays single; leftover men stay single | not found | INSEE source code [read] |
| **MOSART** (Norway) | closed | "female dominated": draw the husband's age given hers, then a random unmarried man of that age | not found | not found | Andreassen 1993 [read] |
| **SESIM** (Sweden) | closed | each chosen woman samples 10 unmarried men | single if none is within 5 years | not found | via Bouffard et al. 2001 [read] |
| **IRELAND (O'Donoghue)**, **NEDYMAS**, **HARDING** | closed | IRELAND resizes the men's pool to the women's; NEDYMAS sets marriages to the mean of the two pools and trims or tops up; HARDING uses rates halfway between the sexes and a fixed 2-year gap | by construction | – | via Bouffard et al. 2001 and O'Donoghue 2001 [read] |
| **FAMSIM+** (Austria) | closed | a partner is matched at a woman's first birth: draw his age and education, search for such a man, redraw his age if none | no explicit partner after the maximum tries | "in around 94% of the cases no repetition is needed and in 99.6% less than three repetitions are needed" | Spielauer 2004 [read] |
| **PENSIM2** (UK) | closed | Order of Decreasing Differences (hardest to match first) | not found | not found | Emmerson, Reed & Shephard 2004 [read] |
| **VirtualPop** (Willekens) | closed within a generation | random opposite-sex member near a target age gap, about 500 candidates | "some individuals remain without a partner" | not quantified | CRAN vignette [read] |
| **Dymium** (Melbourne) | closed | 30 candidates, weighted by age gap | pools topped up with extra people drawn by their transition probabilities: "no 'left over'" | – | Siripanich & Rashidi 2020 [read] |
| **EUROMOD** | static | none | – | – | Sutherland & Figari 2013 [read] |

### 2c. What the methods papers add

- **Perese (2002)**, the CBO survey [read]: "two distinct methodologies… a stable marriage approach and a stochastic approach". The stable (greedy) approach gives "an exorbitant proportion of marriages occurring where the husband is one year older than the wife… also … too many 'extreme' marriages… This bimodal distribution… because after the 'best' marriages are made, there are only relatively 'bad' matches left."
- **Cumpston (2010)** compares immediate matching (probability-weighted, best of 5–6) with batch stochastic and order-of-decreasing-difficulty matching [abs].
- **Walker & Davis (2013)** run full New Zealand census populations with greedy matching within random social networks and under-estimate ethnic homogamy (Auckland 1981, European/European: 13,476 actual against 9,415 simulated) [abs].
- **Geffen & Scholz (2017)**: brute-force pairing is O(n²); random, random-k and distribution-counting matching are O(n); weighted and cluster-shuffle matching are O(n log n) [abs].
- **The pattern across models:** the closed models that report a number lose or delay roughly 5–10% of intended unions, and they fix it by aligning rates upward (DYNASIM3, SimPaths) or by forcing the counts equal (DESTINIE 2, NEDYMAS, Dymium). Internot's mean-field design is of the second kind, with the balance coming from the projection rather than from trimming.

## 3. Matching rules and realism

### 3a. How big real squeezes were, and how markets absorbed them

The historical record is consistent: even very large sex-ratio shocks moved the *level* of marriage little and its *distribution* (spouse ages, previously married partners, foreigners, births outside marriage) a lot.

| Episode | Shock | Effect on who ever partners | Effect on the distribution | Source |
|---|---|---|---|---|
| WWI France | 1.4M military dead, 27–29% of men born 1892–95; men 18–59 per woman 15–49: 1.087 (1911) → 0.992 (1921), 0.864 in the worst regions; single men to single women aged ≤30 about 4:6 | women single at 50: 11% (born 1850–80) → 12% (born 1890–1900); Henry predicted 196–226 spinsters per 1000 for women born 1891–1900 without adjustment, and the market absorbed 65–107 of them | inter-cohort marriages (women marrying younger men) were 52–64% of the adjustment; the rest foreigners, widowers and divorced men, and lower male bachelorhood. A sex ratio falling from 1 to 0.9 narrowed the age gap by about 2 years (IV), and men married up (women down) 8.2 points more often | Vandenbroucke 2015 [read]; Henry 1966 via Knowles & Vandenbroucke 2013, Table 1 [read, secondary]; Abramitzky, Delavande & Vasconcelos 2011, working-paper version [read] |
| WWII USSR | sex ratio as low as 0.60 for women born 1924; 0.627 for Russian women aged 35–39 in 1959 | never married: 4.3–5.2% of women born 1915–41; a 2-SD fall in the sex ratio: −5.0 points currently married, +3.4 points never married, +3.1 points childless | more urban marriages with large age gaps | Brainerd 2017, IZA DP 10130 version [read] |
| WWII Bavaria | 105 → 76.5 men per 100 women, 1939 → 1946 | n/a | nonmarital birth ratio 9.4% → 16.1% (1939 → 1947) | Bethmann & Kvasnicka 2013, working paper [read] |
| US baby boom | the 1978 age–sex composition against 1951's | "little influence on the level of marriage" | men's mean age at marriage −0.54 y, women's +0.53 y; variance of men's ages −18%, women's +33% | Schoen 1983 [abs]; Akers 1967: 1960s trends "explained almost entirely by disproportions between the sexes at the prime ages" [abs] |
| US cohorts 1914–1981 | cohort size +42% (1940 → 1950 cohorts), +17% (1950 → 1960) | ever married by 30: −3.4 points for women and **−3.8 for men** (1940 → 1950); −1.5 and −1.4 (1950 → 1960); about 55% and 39% of the observed changes | higher age at first marriage | Bronson & Mazzocco 2024, working-paper version [read] |
| England and Wales, cohorts 1900–69 | large swings in births | "Marriage squeeze is found to be virtually absent" | partners "adapt to rather than … be constrained by the age distribution"; mean gap 2–3 years throughout | Ní Bhrolcháin 2001, 2005 [abs] |
| Five Western countries, 1910–76 | most severe squeezes in England and Wales, Sweden and Switzerland, 1915–35 | the squeeze did not play "a major role in recent changes" | | Schoen & Baj 1985 [abs] |
| China and India, projected | prospective grooms exceeding brides by more than 50% for three decades (best scenario) | men unmarried at 50: 15% in China by 2055, 10% in India by 2065 (Guilmoto); lifelong never-married men above 10% in 2044 and 13% in 2050 (Jiang et al. 2014) | | Guilmoto 2012 [abs]; Jiang, Feldman & Li 2014 [read]; Tuljapurkar, Li & Feldman 1995: about 1 million excess males per year after 2010 [abs] |

Three points matter for Internot:

1. **The projections of future squeezes are mean-field.** Guilmoto (2012) uses "a two-sex cohort-based procedure … based on the female dominance model" plus "two more-flexible marriage functions" [abs]. Jiang et al. (2011, 2014) use deterministic cohort-component projections and nuptiality tables [read]. No agent-based or microsimulation projection of the China or India squeeze was found.
2. **Two-sex functions get men's response to cohort size wrong.** Bronson & Mazzocco find marriage rates of *both* sexes fall when cohorts grow. "Using demographic models, Akers and Schoen predict the same positive relationship between changes in cohort size and men's marriage rates that is predicted by our standard matching model. Their models are therefore rejected by the data" [read]. Only a model in which the marriage surplus falls with cohort size fits both sexes and the rise in births outside marriage. Bergstrom & Lam (1994), by contrast, hold marriage rates fixed and let only the age gap adjust [read, via Bronson & Mazzocco; abs].
3. **Which function fits best is hard to tell from data.** Martcheva & Milner (2001) project US couples and births between censuses with a family of marriage functions: the differences between functions "are of the same magnitude (or even smaller) than the errors between the projected and real data", and "the harmonic mean function frequently found and used in the literature is a quite poor performer" [abs]. Keyfitz (1972) already noted that functions are hard to tell apart where the sex ratio varies little (cited in Shyu & Caswell 2018 [read]).

### 3b. The two rules in a toy squeeze [comp]

`scripts/2026-10-01-two-sex/squeeze_demo.py` solves one market-year with single-year ages (women 18–45, men 18–50), an attraction kernel normal in the age gap (mean 2.1 y, SD 5.1 y, the CPS 2023 couple gap) and a one-sex first-union hazard peaking at 25 for women and 27 for men. It compares:
- **H**, the current ledger rule: the harmonic mean of the two sides' wants, then IPF of the kernel to the wants scaled to that total;
- **C**, Choo–Siow with singles, solved by Galichon–Salanié's IPFP, with Π calibrated so that the baseline reproduces H's matrix exactly.

The squeeze: pools grow by 3% per year of age below 30 (a boom entering the market), or shrink by 3% (a bust).

| Rule | Pools | Unions | Mean gap (y) | First-union rate, women 18–24 | Rate, men 20–32 |
|---|---|---|---|---|---|
| H | flat | 1,886 | 2.24 | 0.1015 | 0.1051 |
| C | flat | 1,886 | 2.24 | 0.1015 | 0.1051 |
| H | boom (+3%/y) | 2,115 | 2.24 | 0.0977 (−3.7%) | 0.1053 |
| C | boom (+3%/y) | 2,111 | 2.17 | 0.0950 (−6.4%) | 0.1055 |
| H | bust (−3%/y) | 1,695 | 2.21 | 0.1055 | 0.1048 |
| C | bust (−3%/y) | 1,694 | 2.30 | 0.1087 | 0.1043 |

- Both rules reproduce the textbook squeeze for women: young women's rates fall in a boom.
- Only Choo–Siow moves the age gap, in the direction the history shows (narrower when young women are plentiful).
- Neither lowers men's rates in a boom (Bronson & Mazzocco's fact 2).

### 3c. Expected pools against realized pools

**Theory.** For bisexual Galton–Watson processes with superadditive mating functions there is a law of large numbers: if the initial population scales as m·z, the normalized process converges to the deterministic iterates M^n(z), and "M(z) = lim … E(Z₁|Z₀=⌊mz⌋)/m = sup_{m≥1} E(Z₁|Z₀=⌊mz⌋)/m" (Fritsch, Villemonais & Zalduendo 2022, Theorem 1 and Corollary 2 [read]). Two consequences:
- at population scale, realized and mean-field dynamics agree;
- in any finite population the expected number of matches per head is **at most** the mean-field value (a Jensen gap; their 2024 paper states E P(Z_n) < P(E Z_n) for concave P [read]).

The general result behind this is Kurtz's: density-dependent Markov chains follow their ODE or recursion with fluctuations of order N^{−1/2}, and the probability of a deviation larger than ε decays exponentially in N (Kurtz 1970, 1971 [abs]; Darling & Norris 2008, Theorem 4.2 [read]).

**Size of the Jensen gap** [comp] (`jensen.py`, independent Poisson pools of mean n on each side):

| n (expected singles per side) | 0.5 | 1 | 2 | 5 | 10 | 30 | 100 |
|---|---|---|---|---|---|---|---|
| E[FM/(F+M)] relative to the mean field | 0.37 | 0.57 | 0.75 | 0.90 | 0.95 | 0.983 | 0.995 |
| E[min(F, M)] relative to the mean field | 0.33 | 0.48 | 0.61 | 0.75 | 0.82 | 0.90 | 0.94 |

The harmonic mean loses about 1/(2n) of its unions (E ≈ H(n, n) − ½ for 2FM/(F+M), a derivation from the literature pass that the numbers confirm), and the minimum about 1/√(πn). So **with hundreds of singles per cell the mean field is within 1% of a realized simulation; with 5 it overstates unions by about 11%, and with 1 by about 75%.** No demographic paper quantifying this gap for marriage functions was found, and none comparing a macro two-sex projection with a microsimulation of the same rates.

**Monte Carlo noise in microsimulation** is treated in the literature as a cost, not a feature: van Imhoff & Post (1998) stress that microsimulation outcomes are subject to random variation, the larger the more explanatory variables there are [abs]; Wolf (2001) lists starting-sample error, Monte Carlo error and parameter uncertainty, with replication as the remedy for the second [read].

### 3d. Imbalance under local rounding and a carry [comp]

`scripts/2026-10-01-two-sex/carry_sim.py`: one market over 250 years, k cells per side, mean-field seats balanced exactly each year (Σ women's expected seats = Σ men's), each cell rounded on its own. Two rounding rules:
- **A:** each year independently (unbiased floor or ceiling);
- **B:** cumulatively along time, with one keyed offset per cell (systematic rounding of the cell's running total, i.e. `partition::apportion_systematic` over the years).

The market matches min(W + queued women, M + queued men) and carries the rest. Means over 40 seeds:

| Cells per side | Seats per cell per year | Rule | Mean \|W − M\| per year | Mean queue | Queue p99 | Queue at year 250 | Mean delay of a seat (y) |
|---|---|---|---|---|---|---|---|
| 40 | 0.25 | A | 2.8 | 30.9 | 114 | 44.1 | 3.03 |
| 40 | 0.25 | B | 2.8 | 2.9 | 9 | 2.6 | 0.28 |
| 40 | 2 | A | 2.9 | 30.3 | 100 | 42.4 | 0.37 |
| 40 | 2 | B | 2.9 | 2.8 | 9 | 2.7 | 0.034 |
| 40 | 25 | A | 2.9 | 32.1 | 115 | 47.3 | 0.031 |
| 40 | 25 | B | 2.9 | 2.8 | 9 | 2.7 | 0.003 |
| 150 | 2 | A | 5.7 | 60.9 | 236 | 96.0 | 0.20 |
| 150 | 2 | B | 5.7 | 5.6 | 18 | 5.4 | 0.018 |

- The per-year imbalance depends only on the number of cells: its SD is about √((k_w + k_m)/6) (a rounding error with a uniform fractional part has variance 1/6 on average).
- Under A the queue is a reflected random walk and grows like √T. Under B every cell's cumulative error stays in (−1, 1], so the queue is bounded by k_w + k_m and in practice stays near the one-year imbalance.
- **Overflow tier:** 60 thin local markets (40 cells per side, 0.25 seats per cell) first clear locally and send leftovers of both sexes to one pooled market the same year. Then 13.3% of seats overflow and 3.5% are carried. At 2 seats per cell, 1.7% overflow and 0.45% are carried.

### 3e. What matching rules did to realism in microsimulations

- **Greedy best-pair-first** (CORSIM, DYNACAN, POLISIM, SVERIGE) concentrated first marriages at a 1-year gap (about 73%) and pushed the leftovers into extreme gaps: 3.3% at 20+ years, where the census shows none. Stochastic "dartboard" matching fixed the spike but left more than 15% of remarriages with a 20+-year gap (Bouffard et al. 2001 [read]).
- **CBOLT's** normalized stochastic acceptance reproduced the 1-year share (about 15%, as in SIPP) and education concordance (about 75%), but doubled the remarriage spike at 20+ years (Perese 2002 [read]).
- **SOCSIM:** a large mean age gap cannot be held with a symmetric gap distribution when cohorts are equal in size, "unless a large fraction of people never marry". Old widows' remarriages pull the mean gap down (rsocsim notes [read]).
- **Earlier stochastic algorithms** gave gap distributions "too flat … not capable of reproducing the observed peak at differences of [−1,1]" (Zinn 2012, citing Leblanc, Morrison & Redway 2009, not found [read]).
- **Matching inside cells** (DYNASIM: race × age × education) forbids pairs across them, including interracial marriages in the older DYNASIM (Perese 2002 [read]).
- **The lesson for a carry:** leftovers are where artifacts concentrate. Seats carried to the next year must re-enter that year's kernel fit as ordinary rows of their class, not be paired among themselves.

## 4. Kinship at scale: memory, time, and kin on demand

### 4a. Formal kinship models: means (and now variances), no individuals

- **Goodman, Keyfitz & Pullum (1974)** and **Caswell's matrix models (2019 →)** compute the *expected* age distribution of each kind of kin of a focal individual from fertility and mortality schedules. Their state is a few vectors per kin type, so memory and time are trivial (covered in `2026-09-29-kinship-and-households.md` §1). Caswell (2022, part IV) adds two sexes; Alburez-Gutierrez, Williams & Caswell (2023) run the two-sex, time-varying model for every country in WPP 2022 [abs]: "a 65-yo woman in 1950 could expect to have 41 living kin, a 65-yo woman in 2095 is projected to have just 25 [18.8 to 34.7]".
- **Caswell (2024, part VI)** makes the kin vector a multitype branching process and projects (co)variances with the same matrices [read]. "For all kin other than direct ancestors … the variance is close to the mean … compatible with a Poisson distribution"; parents and other ancestors are under-dispersed (binomial).
- **The limit of the approach, in Caswell's own words** (2024, §6.4) [read]: the branching-process formulation assumes independent individuals. "When individuals interact, their outcomes are no longer independent, and the age distribution … is no longer a valid state variable … For the study of interactions, microsimulations are not only possible but essential. Important cases of interactions include pairing of individuals into marriages or partnerships, affinal kin resulting from such pairings, and interactions that take place among members of households."

So formal demography gives Internot **oracles** (expected kin counts and their variances for property tests) but no individual-level, reciprocal kin.

### 4b. Kinship microsimulation: SOCSIM and its relatives

Kinship microsimulations keep the whole population, and their reported sizes are small:

| Model | Population | Time and memory | Source |
|---|---|---|---|
| SOCSIM (Murphy's British runs) | initial 4,000–10,000, two 600-year runs | "a single simulation can take some hours to run"; the market clears "with the sorts of population sizes used here" | Murphy 2004 [read] |
| SOCSIM (kinship studies since: kinlessness, bereavement, the sandwich generation) | not found | not found | listed in Caswell 2024 §6.4 [read]; Verdery & Margolis 2017 [abs] |
| MIDAS_BE (LIAM2), 2002–2060 | 300k persons / 2.2M persons (one-fifth of Belgium) | 28 min and 832 MB peak / 2 h 54 min and 4.38 GB peak; matching 5–7% of the run | de Menten et al. 2014, Table 1 [read] |
| LifePaths (open) | "from 4 million to 30 million cases" per run | not found | StatCan overview [read via Wayback] |
| CBOLT | about 300,000 individuals (a 10% sample of the CWHS) | not found | CBO 2009 [read] |
| VirtualPop | per-generation closed populations | not found | CRAN vignette [read] |

- **Scale:** the 2.2M-person MIDAS run peaks at about 2 kB per simulated person. Internot's ledger layouts are about 80 bytes per person ever born (1.31 GB for 17M), so Internot is already some 25 times denser than a classic microsimulation (inferred).
- **Ego-centred alternatives:** CAMSIM (Smith & Oeppen 1993) "produces kin sets for a birth cohort of unrelated egos". Each ego's kin are drawn independently, so they are not reciprocal (2026-09-29 note §2.2).
- **Ruggles (1993), "Confessions of a microsimulator"**, the classic critique of such designs: not found (not accessed).

### 4c. Approaches from population genetics

Population genetics faces the same storage problem at much larger scale, and has two answers.

- **Store the pedigree compactly.** Kelleher, Thornton, Ashander & Ralph (2018, *PLoS Comput. Biol.* 14(11):e1006581) record every breeding event of a forward simulation as a *tree sequence* and periodically "simplify" it to the ancestry of the living. "A population of N individuals with M polymorphic sites can be stored in O(N log N + M) space"; they report speed-ups of one to two orders of magnitude [abs]. Anderson-Trocmé et al. (2023, *Science* 380:849–855) run the coalescent through a fixed real pedigree from about 4 million Quebec parish records and release simulated genomes for 1,426,749 individuals [abs].
- **Generate only what is asked for, backwards.** The coalescent (Kingman 1982) generates the genealogy of a *sample* backwards in time, without the rest of the population. Chang (1999, *Adv. Appl. Probab.* 31:1002–1026) and Rohde, Olson & Chang (2004, *Nature* 431:562–566) study random pedigrees in which each individual's parents are drawn at random from the previous generation (cited in Matsen & Evans 2008, *TPB* 74:182–190 [abs]). In such a model the parents of an individual *are* a pure function of (seed, id), but its **children are not**: they are everyone in the next generation who drew it, an inverse that needs enumeration (inferred). There is also no monogamy, no two-sex balance and no union duration.

### 4d. Kin on demand: what was found

No demographic or genetic approach was found that generates **individual-level, reciprocal** kin (partners, children and siblings that agree from both ends) on demand from (seed, id) at population scale without first simulating or storing a population. The nearest are:

| Approach | On demand? | Reciprocal? | Two-sex unions? |
|---|---|---|---|
| GKP / Caswell kin vectors | yes (expected values) | n/a (no individuals) | approximated (androgynous or two-sex rates) |
| Ego-centred kin sets (CAMSIM) | per ego | no: each ego's kin are drawn independently (see 2026-09-29 note §2.2) | no |
| Random pedigrees (Chang) | parents only | children need an inverse search | no |
| Coalescent / tree sequences | a sample's ancestry | yes within the simulated sample | no |
| Internot's ledger | yes, after a build that stores the integer ledger | yes | yes |

The novel part of Internot is the bijective, counted construction (keyed permutations cut into ranges sized by an integer ledger). What remains is to make the ledger itself cheap to store or recompute, which is what §5 is about.

## 5. What this means for Internot

The proposed redesign has three parts: (a) a deterministic mean-field two-sex projection, computed once and stored compactly; (b) each block's members partitioned top-down over life-course states from that schedule; (c) the two-sex constraint enforced per market-year by matching that year's women's and men's seats, with a small per-market carry. The literature supports (a) and (b) directly, and (c) is SOCSIM's two-queue market made deterministic.

### 5a. The mean-field projection (a)

1. **It is standard demography.** A deterministic two-sex projection with a marriage function is what macro household projections do (LIPRO's consistency algorithm; Matthews & Garenne's IPF with a harmonic-mean total, which is nearly the current `solve_market`). Nothing in it needs realized integer pools; the realized pools enter today only through the caps in `apply_market`.
2. **Store the market-year state, not the matrix.** With IPF the fitted matrix is μ_ij = a_i b_j K_ij; with Choo–Siow it is μ_ij = Π_ij √(μ_i0 μ_0j). Either way one market-year is two vectors over its classes (birth year × status), plus the kernel, which is shared. The per-block schedules (first-union hazard, remarriage, births) follow from those vectors and the pools, so they need not be stored either if the pools are recomputable. How much of the projection must be cached is a time–memory trade (see Open questions).
3. **Consider Choo–Siow with singles in place of "harmonic total + IPF".** In the toy squeeze of §3 [comp], both rules are calibrated to the same baseline; with younger cohorts 3% larger per year of age, the current rule keeps the mean age gap at 2.24 years and lowers young women's rate by 3.7%, while Choo–Siow narrows the gap to 2.17 years and lowers it by 6.4%. Choo–Siow's equilibrium is unique and moves smoothly with the pools (Decker et al.), it has competition across classes, and its IPFP (Galichon & Salanié, Algorithm 1) is the same cost as today's IPF. It would be a new `procedural_core::fit` primitive with property tests (margins, uniqueness, homogeneity, monotone singles) and golden values.
4. **What neither rule does:** men's marriage rates in large US cohorts *fall* (Bronson & Mazzocco: −3.8 points ever married by 30 for men between the 1940 and 1950 cohorts), while two-sex functions predict a rise or no change (their own reading of Akers and Schoen; toy model: flat or +0.4%). If that matters, it is a pack factor: attraction (surplus) falling with cohort size, which is their model (i).

### 5b. Top-down partitions (b)

5. **Exact partitions remove Monte Carlo noise, so "realized" pools equal the mean-field pools to within rounding.** A block partitioned by keyed systematic rounding (`partition::apportion_systematic`) has, in every state and year, the floor or the ceiling of its expected count. So the gap between "expected" and "realized" pools is O(1) per cell, not the O(√N) of a stochastic simulation (§3). At the scale of `us` blocks that is negligible; in thin blocks it is the whole story (next items).
6. **Round cumulatively along time within each (block, market).** In the simulation of §3 [comp], rounding each year independently makes the carry queue a reflected random walk: with 40 cells per side it averages 30 seats and reaches 44 after 250 years, growing like √T. Rounding cumulative counts along time (one keyed offset per cell, i.e. `apportion_systematic` over the years of one (block, market)) keeps every cell's cumulative error in (−1, 1], so the queue is bounded by the number of cells and averaged 2.8 (p99 9). This is the single most important detail of (c).
7. **Joint draws must keep the conditioning local** (lesson from R1). First-union year, death year, plan and dissolution are one joint partition per block, conditioned only on the person's own states (alive at the union year, alive at planned births), as now.

### 5c. Per-market matching with a carry (c)

8. **Expected imbalance.** Per market-year, the women's and men's seat totals differ by a sum of independent rounding errors: the standard deviation is about √((k_w + k_m)/6) seats for k cells per side, whatever the market's size (§3: mean |W − M| ≈ 2.9 at 40 cells per side, 5.7 at 150). So the share of seats that cannot be matched in their year is about that number over the market's unions:

   | Market | Unions per year | Share carried (cumulative rounding) |
   |---|---|---|
   | 40 cells per side, 25 seats per cell | 1,000 | 0.3% |
   | 40 cells per side, 2 seats per cell | 80 | 3.4% |
   | 40 cells per side, 0.25 seats per cell | 10 | 28% |

   In `us` markets this is a fraction of a percent; in the thin markets of `us-areas` it is large.
9. **Fixes for thin markets, in order of preference:**
   - fewer, coarser cells per market-year (e.g. 5-year birth bands in thin markets), since the imbalance grows with the number of cells, not the market's size;
   - an **overflow tier**: local leftovers of both sexes go to a pooled market the same year (cross-area partners), and only its leftover carries. In the simulation with 60 thin local markets, 13% of seats overflowed (more than the 6–10% inter-area share targeted for `us-areas`), and 3.5% were carried; with 2 seats per cell, 1.7% overflowed and 0.45% were carried;
   - a carry, last.
10. **Who absorbs the carry.** A carried seat changes someone's life course, so choose the side whose life course is cheapest to change:
    - **Women's seats authoritative** (SOCSIM's one-queue choice, "female age specific nuptiality rates are generally met"): births, the plan and the next generation's cohort sizes stay exactly as drawn. Men absorb the imbalance, as in the one-queue market where "males … marry at the convenience of females".
    - **A shortfall of men:** borrow men whose seat is next year. They are alive now, since their death is drawn conditioned on being alive at their union.
    - **An excess of men:** carry them forward. A man carried forward must be alive at his new union date, so either his death draw is conditioned on it or the carry skips men who die while queued. The alternative is to return them to the never-partnered pool, as DYNASIM and CBOLT do; the carry is the same thing with a memory.
    - The father's rule (alive at conception) is unaffected; the man's union year moves by a year for a few percent of men in mid-size markets.
11. **The carry state is tiny but sequential.** The queue after year t depends on all earlier years of that market, like today's ledger but with one small integer vector per market-year (queued seats by class) instead of the full cells. Store it (it is small), or recompute a market's queue by a scan over its years (O(years × cells), no other market involved). Either way, a market-year's detail becomes a pure function of the stored mean-field vectors, the blocks' partitions and that queue state.
12. **Kin repair and close kin** stay within the market-year's seat ranges, as now. Thin cells still make repairs fail occasionally (AGENTS: 5 sibling couples in 8.7M on `us-areas`); the redesign does not change that.

### 5d. What it can and cannot reproduce

| Phenomenon | Mean-field + carry | Why |
|---|---|---|
| Baby-boom and baby-bust squeezes (Akers, Schoen) | **yes**, in aggregate | cohort sizes enter the marriage function; women's rates fall when their cohorts are large |
| Adjustment through the age gap (Bergstrom & Lam; Ní Bhrolcháin) | **with Choo–Siow, partly**; with the current rule, barely | the current rule's gap is pinned by the one-sex margins and the kernel |
| Men's rates falling in large cohorts (Bronson & Mazzocco) | **no**, unless attraction falls with cohort size | a property of every two-sex function tried (their reading) |
| War deficits of men (post-WWI France, post-WWII USSR and Germany) | **yes**, if the deaths are in the mortality schedule by sex and cohort | lower male pools raise women's never-partnered share and shift age gaps through the function |
| Sex-ratio-at-birth or migration squeezes | **yes**, if the pack's flows are by sex | same mechanism |
| Chance squeezes in small places | **no** | there is no sampling noise left; thin cells follow the mean field |
| Individual-level path dependence (one widow competing with her daughter) | **no** | the market sees classes, not people |
| Social-interaction effects (the Wedding Ring ABM) | **no** | not in any mean-field function |

The losses are deliberate: Internot's thin cells (area × heritage × birth year) are artifacts of stratification, not isolated marriage markets, so the mean field is arguably *more* realistic for them than realized pools.

### 5e. Pitfalls

- **Rounding independent across years** turns the carry into a random walk (item 6).
- **Many cells per thin market** (item 8): the imbalance does not shrink with the market.
- **Jensen's gap goes the other way in thin cells.** For a concave function such as the harmonic mean, the expectation of the function of random integer pools is below the function of the expected pools, so a stochastic simulation of a small population makes fewer unions than the mean field. In Internot the thin cells are not real small markets, so following the mean field is the right choice, but it must be a choice.
- **The two-sex function's free parameters are not identified by one-sex rates.** Calibrate against observed couple age gaps and ever-partnered shares by sex (the realism report), not against the wants alone.
- **Multiple equilibria** (Pollak 1990) do not arise in a year-by-year projection, but a stable-population initializer (the founders) should be checked for them.

## 6. Open questions

1. **How many cells feed one market-year today?** Today's markets take seekers by (block, status) rows. The imbalance under (c) is about √(Σ_c f_c(1 − f_c)) over both sides' cells, where f_c is a cell's fractional expected seats. Instrument `solve_market` to print, per market-year, the row and column counts, Σ f(1 − f) and the expected unions, on `us` and `us-areas`. That turns §5c's table into Internot's own numbers, and decides whether thin `us-areas` markets need coarser cells or an overflow tier.
2. **What must be stored, and what recomputed?** The mean-field projection is global: one block's schedule depends on every block it shares a market with. Two options:
   - Store, per market-year, the two class vectors (IPF factors or Choo–Siow singles) and the carry queue. That is about markets × years × classes × 2 × 4 bytes: a few MB for `us`; on the order of 100 MB for `us-areas`'s ~1,100 markets a year, before compression (inferred).
   - Recompute the projection at startup. That is today's build without the integer bookkeeping, of unknown cost.

   Measure both.
3. **Choo–Siow as a flow model.** Choo–Siow is a static stock model; using it on one year's seekers (singles at risk, μ_0 = those who stay single this year) is a modelling choice. Choo's dynamic version (2015, *Econometrica*) was not read. Π must be calibrated from the pack's targets (couple age gaps, ever partnered by sex and cohort), not from one-sex wants alone.
4. **Men's rates in large cohorts.** Add a cohort-size term to attraction (Bronson & Mazzocco's model (i)) as a pack factor, or accept the known bias? It needs a decision on how much the realism report should show it.
5. **Who absorbs the carry, and deaths in the queue.** Women's seats authoritative with men lending (no death check), or a symmetric queue with a death check? The first keeps births exact by construction. Its cost is men's union years moving by a year for a few percent of men.
6. **Divorced men's seats are not a block partition.** A man's dissolution year is drawn on the couple (the woman's plan), so his availability for a second union is a sum over couples. That is R1c's "third margin" (`2026-09-30-repartnering-exactness-problem.md` §3) again. In the mean-field design the men's remarriage seats per market-year would differ from the projection by a hypergeometric amount, which the carry must also absorb. How large that is needs measuring.
7. **Same-sex markets** draw both sides from one pool, so their imbalance is only a parity (an odd seat); the carry handles it trivially, but kin repair inside one pool needs checking.
8. **Kin repair in thin seat ranges.** The redesign does not change the rare sibling couples in thin cells (5 in 8.7M on `us-areas`). The founder decision on that debt is still open.
9. **Not reached in this pass:**
   - Parlett 1972, McFarland 1972, Pollard 1977 and 1997, Pollard & Höhn 1993–94, and Schoen 1988 (*Modeling Multigroup Populations*): the original statements of the desiderata;
   - Ruggles 1993;
   - Qian & Preston 1993;
   - Glick, Heer & Beresford 1963;
   - full texts of Henry 1966 and Ní Bhrolcháin 2001 (the share of imbalance absorbed by age-gap shifts);
   - a paper comparing a macro two-sex projection with a microsimulation of the same rates (none found).

## Sources

Two-sex models:
- Schoen, R. 1981. "The harmonic mean as the basis of a realistic two-sex marriage model." *Demography* 18(2):201–216. https://doi.org/10.2307/2061093 (abstract via https://api.openalex.org/works/doi:10.2307/2061093) [abs]
- Keyfitz, N. 1972. "The mathematics of sex and marriage." *Proceedings of the Sixth Berkeley Symposium on Mathematical Statistics and Probability*, vol. 4, pp. 89–108. https://projecteuclid.org/proceedings/berkeley-symposium-on-mathematical-statistics-and-probability/Proceedings-of-the-Sixth-Berkeley-Symposium-on-Mathematical-Statistics-and/Chapter/The-mathematics-of-sex-and-marriage/bsmsp/1200514459 [abs; the PDF host did not resolve]
- Parlett, B. 1972. "Can there be a marriage function?" In T. N. E. Greville (ed.), *Population Dynamics*, Academic Press, pp. 107–135. https://doi.org/10.1016/b978-1-4832-2868-6.50009-6 (metadata only)
- Pollak, R. A. 1986. "A reformulation of the two-sex problem." *Demography* 23(2):247–259. https://doi.org/10.2307/2061619 [abs]
- Pollak, R. A. 1987. "The two-sex problem with persistent unions: a generalization of the birth matrix-mating rule model." *Theoretical Population Biology* 32(2):176–187. https://doi.org/10.1016/0040-5809(87)90046-3 [abs]
- Pollak, R. A. 1990. "Two-sex demographic models." *Journal of Political Economy* 98(2):399–420. https://doi.org/10.1086/261683 [abs]
- Hadeler, K. P., R. Waldstätter and A. Wörz-Busekros. 1988. "Models for pair formation in bisexual populations." *Journal of Mathematical Biology* 26(6):635–649. https://doi.org/10.1007/BF00276145 [abs]
- Shyu, E. and H. Caswell. 2018. "Mating, births, and transitions: a flexible two-sex matrix model for evolutionary demography." *Population Ecology* 60:21–36. https://doi.org/10.1007/s10144-018-0615-8 (full text: https://www.ebi.ac.uk/europepmc/webservices/rest/PMC6435235/fullTextXML) [read]
- Choo, E. and A. Siow. 2006. "Who marries whom and why." *Journal of Political Economy* 114(1):175–201. https://doi.org/10.1086/498585 (PDF: https://www.math.utoronto.ca/mccann/1855/papers/ChooSiowJPE06.pdf) [read]
- Decker, C., E. H. Lieb, R. J. McCann and B. K. Stephens. 2013. "Unique equilibria and substitution effects in a stochastic model of the marriage market." *Journal of Economic Theory* 148(2):778–792. https://doi.org/10.1016/j.jet.2012.12.005 (arXiv: https://arxiv.org/abs/1012.1904) [abs]
- Galichon, A. and B. Salanié. "Cupid's invisible hand: social surplus and identification in matching models." arXiv 2106.02371v2 (2023), §4.2 and Appendix F.1.4. https://arxiv.org/abs/2106.02371 [read] (journal version in the *Review of Economic Studies*, 2022; volume and pages not verified here)
- Mourifié, I. and A. Siow. 2021. "The Cobb-Douglas marriage matching function: marriage matching with peer and scale effects." *Journal of Labor Economics* 39(S1):S239–S274. https://doi.org/10.1086/711491 [abs]
- Siow, A. 2008. "How does the marriage market clear? An empirical framework." *Canadian Journal of Economics* 41(4):1121–1155. https://doi.org/10.1111/j.1540-5982.2008.00498.x [abs]
- Matthews, A. P. and M. L. Garenne. 2013. "A dynamic model of the marriage market. Part 1: matching algorithm based on age preference and availability" and "Part 2: simulation of marital states and application to empirical data." *Theoretical Population Biology* 88:78–85 and 86–93. https://doi.org/10.1016/j.tpb.2013.01.006, https://doi.org/10.1016/j.tpb.2013.05.002 (abstracts via PubMed 23357512, 23689022) [abs]
- Keilman, N. 1985. "Nuptiality models and the two-sex problem in national population forecasts." *European Journal of Population* 1(2–3):207–235. https://doi.org/10.1007/BF01796933 (metadata only)
- van Imhoff, E. and N. Keilman. 1991. *LIPRO 2.0: An Application of a Dynamic Demographic Projection Model to Household Structure in the Netherlands.* NIDI/CBGS Publications 23. https://www.researchgate.net/publication/236631298 [abs, search summary only]
- Li, N. 2025. "Two-sex renewal population forecast: the model and general solution." *China Population and Development Studies* 9(1):39–53. https://doi.org/10.1007/s42379-025-00181-y (metadata only)
- Martcheva, M. and F. A. Milner. 2001. "The mathematics of sex and marriage, revisited." *Mathematical Population Studies* 9(2):123–141. https://doi.org/10.1080/08898480109525499 [abs]
- Choo, E. 2015. "Dynamic marriage matching: an empirical framework." *Econometrica* 83(4):1373–1423. https://doi.org/10.3982/ecta10675 (metadata only)

Cohort size and marriage markets:
- Bronson, M. A. and M. Mazzocco. 2024. "Cohort size and the marriage market: explaining nearly a century of changes in U.S. marriage rates." *Journal of Labor Economics* 42(3):877–920. Working paper (March 2021) read: http://www.econ.ucla.edu/mazzocco/doc/CohortSizeMarriageMarket.pdf [read]
- Bergstrom, T. and D. Lam. 1994. "The effects of cohort size on marriage markets in twentieth-century Sweden." In J. Ermisch and N. Ogawa (eds.), *The Family, the Market and the State in Ageing Societies*, Oxford University Press, pp. 46–63. https://doi.org/10.1093/oso/9780198288183.003.0003 [abs]

Kinship models and microsimulation:
- SOCSIM technical notes ("Socsim oversimplified"), MPIDR rsocsim repository. https://github.com/MPIDR/rsocsim/blob/main/socsim_oversimplified.md [read]
- Caswell, H. 2024. "The formal demography of kinship VI: demographic stochasticity and variance in the kinship network." *Demographic Research* 51(39):1201–1256. https://doi.org/10.4054/DemRes.2024.51.39 [read]
- Caswell, H. 2022. "The formal demography of kinship IV: two-sex models and their approximations." *Demographic Research* 47(13):359–396. https://doi.org/10.4054/demres.2022.47.13 (read for the 2026-09-29 note)
- Alburez-Gutierrez, D., I. Williams and H. Caswell. 2023. "Projections of human kinship for all countries." *PNAS* 120(52):e2315722120. https://doi.org/10.1073/pnas.2315722120 [abs]
- Verdery, A. M. and R. Margolis. 2017. "Projections of white and black older adults without living kin in the United States, 2015 to 2060." *PNAS* 114(42):11109–11114. https://doi.org/10.1073/pnas.1710341114 [abs]
- Billari, F. C., A. Prskawetz, B. Aparicio Diaz and T. Fent. 2007. "The 'Wedding-Ring': an agent-based marriage model based on social interaction." *Demographic Research* 17(3):59–82. https://doi.org/10.4054/demres.2007.17.3 [abs]
- Bijak, J., J. Hilton, E. Silverman and V. D. Cao. 2013. "Reforging the Wedding Ring." *Demographic Research* 29(27):729–766. https://doi.org/10.4054/demres.2013.29.27 [abs]

Population genetics:
- Kelleher, J., K. R. Thornton, J. Ashander and P. L. Ralph. 2018. "Efficient pedigree recording for fast population genetics simulation." *PLoS Computational Biology* 14(11):e1006581. https://doi.org/10.1371/journal.pcbi.1006581 [abs]
- Anderson-Trocmé, L. et al. 2023. "On the genes, genealogies, and geographies of Quebec." *Science* 380(6647):849–855. https://doi.org/10.1126/science.add5300 [abs]
- Chang, J. T. 1999. "Recent common ancestors of all present-day individuals." *Advances in Applied Probability* 31:1002–1026; Rohde, D. L. T., S. Olson and J. T. Chang. 2004. "Modelling the recent common ancestry of all living humans." *Nature* 431:562–566, https://doi.org/10.1038/nature02842; Kingman, J. F. C. 1982. "The coalescent." *Stochastic Processes and their Applications* 13:235–248. All as cited in Matsen, F. A. and S. N. Evans. 2008. "To what extent does genealogical ancestry imply genetic ancestry?" *Theoretical Population Biology* 74(2):182–190, https://doi.org/10.1016/j.tpb.2008.06.003 [abs]

Marriage squeezes (literature pass B):
- Henry, L. 1966. "Perturbations de la nuptialité résultant de la guerre 1914-1918." *Population* 21(2):273–332. https://www.persee.fr/doc/pop_0032-4663_1966_num_21_2_13178 [abs]; its table as reproduced in Knowles, J. and G. Vandenbroucke. 2013. "Dynamic squeezing: marriage and fertility in France after World War One." ESRC CPC Working Paper 36, Table 1. http://cpc2.geodata.soton.ac.uk/docs/2013_WP36_Dynamic_Squeezing_Marriage_and_Fertility_in_France_after_WW1_Knowles_et_al.pdf [read] (published as "Fertility shocks and equilibrium marriage-rate dynamics", *International Economic Review* 60(4):1505–1537, 2019 [abs])
- Vandenbroucke, G. 2015. "How World War I changed marriage patterns in Europe." Federal Reserve Bank of St. Louis, *On the Economy*, 9 March. https://www.stlouisfed.org/on-the-economy/2015/march/how-world-war-i-changed-marriage-patterns-in-europe [read]
- Abramitzky, R., A. Delavande and L. Vasconcelos. 2011. "Marrying up: the role of sex ratio in assortative matching." *AEJ: Applied Economics* 3(3):124–157. https://www.aeaweb.org/articles?id=10.1257/app.3.3.124 [abs]; working-paper version (NBER SI 2008) https://users.nber.org/~confer/2008/si2008/DAE/abramizky.pdf [read]
- Brainerd, E. 2017. "The lasting effect of sex ratio imbalance on marriage and family: evidence from World War II in Russia." *Review of Economics and Statistics* 99(2):229–242. https://ideas.repec.org/a/tpr/restat/v99y2017i2p229-242.html; IZA DP 10130 version https://docs.iza.org/dp10130.pdf [read]
- Bethmann, D. and M. Kvasnicka. 2013. "World War II, missing men and out of wedlock childbearing." *Economic Journal* 123(567):162–194. https://doi.org/10.1111/j.1468-0297.2012.02526.x; working paper https://d-nb.info/1206815809/34 [read]
- Akers, D. S. 1967. "On measuring the marriage squeeze." *Demography* 4(2):907–924. https://read.dukeupress.edu/demography/article-abstract/4/2/907/172628/On-Measuring-the-Marriage-Squeeze [abs]
- Schoen, R. 1983. "Measuring the tightness of a marriage squeeze." *Demography* 20(1):61–78. https://doi.org/10.2307/2060901 [abs]
- Schoen, R. and J. Baj. 1985. "The impact of the marriage squeeze in five Western countries." *Sociology and Social Research* 70(1):8–19 (via OpenAlex; no DOI) [abs]
- Ní Bhrolcháin, M. 2001. "Flexibility in the marriage market." *Population: An English Selection* 13(2):9–47. https://www.persee.fr/doc/pop_0032-4663_2001_hos_13_2_7189 [abs]; 2005. "The age difference at marriage in England and Wales: a century of patterns and trends." *Population Trends* 120:7–14. https://eprints.soton.ac.uk/34801/ [abs]
- Guilmoto, C. Z. 2012. "Skewed sex ratios at birth and future marriage squeeze in China and India, 2005–2100." *Demography* 49(1):77–100. https://doi.org/10.1007/s13524-011-0083-7 [abs]
- Tuljapurkar, S., N. Li and M. W. Feldman. 1995. "High sex ratios in China's future." *Science* 267(5199):874–876. https://doi.org/10.1126/science.7846529 [abs]
- Jiang, Q., S. Li and M. W. Feldman. 2011. "Demographic consequences of gender discrimination in China: simulation analysis of policy options." *Population Research and Policy Review* 30(4):619–638. https://pmc.ncbi.nlm.nih.gov/articles/PMC3867633/ [read]
- Jiang, Q., M. W. Feldman and S. Li. 2014. "Marriage squeeze, never-married proportion, and mean age at first marriage in China." *Population Research and Policy Review* 33(2):189–204. https://pmc.ncbi.nlm.nih.gov/articles/PMC3948615/ [read]

Stochastic two-sex theory (literature pass B):
- Fritsch, C., D. Villemonais and N. Zalduendo. 2022. "The multi-type bisexual Galton-Watson branching process." arXiv:2206.09622. https://arxiv.org/abs/2206.09622 [read]; and 2024, arXiv:2406.19559, https://arxiv.org/abs/2406.19559 [read]
- Daley, D. J. 1968. *Z. Wahrscheinlichkeitstheorie verw. Geb.* 9(4):315–322. https://doi.org/10.1007/BF00531755; Daley, D. J., D. M. Hull and J. M. Taylor. 1986. *Journal of Applied Probability* 23(3):585–600. https://doi.org/10.2307/3213999 [abs]
- Asmussen, S. 1980. "On some two-sex population models." *Annals of Probability* 8(4). https://doi.org/10.1214/aop/1176994662 [abs]
- Kurtz, T. G. 1970. *Journal of Applied Probability* 7(1):49–58, https://doi.org/10.2307/3212147; 1971, 8(2):344–356, https://doi.org/10.2307/3211904 [abs]
- Darling, R. W. R. and J. R. Norris. 2008. "Differential equation approximations for Markov chains." *Probability Surveys* 5:37–79. https://arxiv.org/abs/0710.3269 [read]
- Kretzschmar, M. and J. C. M. Heijne. 2017. "Pair formation models for sexually transmitted infections: a primer." *Infectious Disease Modelling* 2(3):368–378. https://pmc.ncbi.nlm.nih.gov/articles/PMC6002071/ [read]
- van Imhoff, E. and W. Post. 1998. "Microsimulation methods for population projection." *Population: An English Selection* 10(1):97–138. https://www.persee.fr/doc/pop_0032-4663_1998_hos_10_1_6824 [abs]
- Wolf, D. A. 2001. "The role of microsimulation in longitudinal data analysis." *Canadian Studies in Population* 28(2):313–339. https://doi.org/10.25336/p67k5x [read]

Microsimulation matching (literature pass A):
- Perese, K. 2002. *Mate Matching for Microsimulation Models.* CBO Technical Paper 2002-3. https://www.cbo.gov/publication/14211 [read]
- Bouffard, N., R. Easther, T. Johnson, R. J. Morrison and J. Vink. 2001. "Matchmaker, matchmaker, make me a match." *Brazilian Electronic Journal of Economics* 4(2). https://web.archive.org/web/2005id_/http://www.beje.decon.ufpe.br/v4n2/neal.pdf [read]
- O'Donoghue, C. 2001. "Dynamic microsimulation: a methodological survey." *Brazilian Electronic Journal of Economics* 4(2). https://web.archive.org/web/2005id_/http://www.beje.decon.ufpe.br/v4n2/cathal.pdf [read]
- Li, J. and C. O'Donoghue. 2013. "A survey of dynamic microsimulation models: uses, model structure and methodology." *International Journal of Microsimulation* 6(2):3–55. https://microsimulation.pub/articles/00082 [abs]; Li, J. 2011. PhD thesis, Maastricht, Table 2.3. https://cris.maastrichtuniversity.nl/ws/files/1533074/guid-55d4193c-2ede-4279-8e11-0bbff52247d8-ASSET1.0.pdf [read]
- Murphy, M. 2004. "Tracing very long-term kinship networks using SOCSIM." *Demographic Research* 10(7):171–196. https://www.demographic-research.org/volumes/vol10/7/ [read]
- Zinn, S. 2012. "A mate-matching algorithm for continuous-time microsimulation models." *International Journal of Microsimulation* 5(1):31–51. https://doi.org/10.34196/ijm.00066 [read]
- LIAM2 User Guide, §6.5.8 "Matching functions." https://liam2.readthedocs.io/en/stable/processes.html [read]; de Menten, G., G. Dekkers, G. Bryon, P. Liégeois and C. O'Donoghue. 2014. "LIAM2: a new open source development tool for discrete-time dynamic microsimulation models." *JASSS* 17(3):9. https://www.jasss.org/17/3/9.html [read]
- Kirn, T. and G. Dekkers. 2023. "Introducing MIDAS_CH." *International Journal of Microsimulation* 16(3):100–129. https://doi.org/10.34196/ijm.00290 [read]
- Statistics Canada. "The LifePaths microsimulation model: an overview." https://www.statcan.gc.ca/en/microsimulation/lifepaths/lifepaths/lifepaths-overview-vuedensemble-eng.pdf [read, via Wayback]
- Favreault, M. and K. Smith. 2004. *A Primer on the Dynamic Simulation of Income Model (DYNASIM3).* Urban Institute Discussion Paper 02-04. https://www.urban.org/sites/default/files/publication/71226/410961-A-Primer-on-the-Dynamic-Simulation-of-Income-Model-DYNASIM-.PDF [read]
- Congressional Budget Office. 2009. *CBO's Long-Term Model: An Overview.* Pub. No. 3235. https://cbo.gov/sites/default/files/111th-congress-2009-2010/reports/06-26-cbolt.pdf [read]
- Bronka, P. et al. 2023 (rev. 2025). "SimPaths: an open-source microsimulation model for life course analysis." CeMPA WP 6/23. https://www.iser.essex.ac.uk/wp-content/uploads/files/working-papers/cempa/cempa6-23.pdf [read]; https://simpaths.org/overview/modules/family-composition/ [read]
- Richiardi, M. G. and R. E. Richardson. 2017. "JAS-mine: a new platform for microsimulation and agent-based modelling." *International Journal of Microsimulation* 10(1). https://microsimulation.pub/articles/00151 [read]; matching source code https://github.com/jasmineRepo/JAS-mine-core/tree/HEAD/src/main/java/microsim/matching [read]
- Sutherland, H. and F. Figari. 2013. "EUROMOD: the European Union tax-benefit microsimulation model." *International Journal of Microsimulation* 6(1):4–26. https://doi.org/10.34196/ijm.00075 [read]
- Spielauer, M. 2004. "Intergenerational educational transmission within families: an analysis and microsimulation projection for Austria." *Vienna Yearbook of Population Research* 2004:253ff. https://www.austriaca.at/0xc1aa5576_0x00062026.pdf [read]; Winkler-Dworak, M., É. Beaujouan, P. Di Giulio and M. Spielauer. 2021. "Simulating family life courses: Italy, Great Britain, Norway, and Sweden." *Demographic Research* 44(1):1–48. https://www.demographic-research.org/volumes/vol44/1/ [read]
- INSEE, Destinie 2 source code (`src/Separations.cpp::mise_en_couple`). https://github.com/InseeFr/Destinie-2 [read]
- Emmerson, C., H. Reed and A. Shephard. 2004. *An Assessment of PenSim2.* IFS WP04/21. https://ifs.org.uk/sites/default/files/output_url_files/wp0421.pdf [read]
- Andreassen, L. 1993. *Demographic Forecasting with a Dynamic Stochastic Microsimulation Model.* Statistics Norway DP 85. https://www.ssb.no/a/publikasjoner/pdf/DP/dp_085.pdf [read]
- VirtualPop tutorial vignette (CRAN). https://cran.r-project.org/web/packages/VirtualPop/vignettes/Tutorial.html [read]
- Siripanich, A. and T. H. Rashidi. 2020. "A demographic microsimulation model with an integrated household alignment method." arXiv:2006.09474. https://arxiv.org/abs/2006.09474 [read]
- Cumpston, J. R. 2010. "Alignment and matching in multi-purpose household microsimulations." *International Journal of Microsimulation* 3(2):34–45. https://www.microsimulation.pub/articles/00037 [abs]
- Walker, L. and P. Davis. 2013. "Modelling 'marriage markets': a population-scale implementation and parameter test." *JASSS* 16(1):6. https://www.jasss.org/16/1/6.html [abs]
- Geffen, N. and S. Scholz. 2017. "Efficient and effective pair-matching algorithms for agent-based models." *JASSS* 20(4):8. https://www.jasss.org/20/4/8.html [abs]
