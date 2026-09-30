# Work and organizations: sizes, hierarchies, careers, rosters, and who talks to whom

**Date:** 2026-09-29
**Scope:** the empirical facts and the math needed to make Internot's work life realistic and consistent from both sides: a person's career history and each employer's roster over time must agree. This covers workplace sizes, org-chart shape, occupations by level, tenure and mobility, vacancy-chain and manpower models, and collaboration networks. It ends with a stateless construction that the team can implement and test.
**Conventions:** "(inferred)" means my own derivation or design proposal, not a published number. "(fit here)" means I computed it for this survey from public Census tables or a small simulation (method stated inline). Every other number gives its population, year and source. Companion notes: `2026-05-14-*.md` in this folder.

---

## 0. Summary: what is directly usable

1. **Workplace means establishment.** In the US in 2022 (CBP), 55.7% of establishments have fewer than 5 employees but hold only 5.6% of jobs, while 0.11% have 1,000+ employees and hold 17.1% of jobs. The current `lognormal(2.0, 1.5)` has too few micro-workplaces and a tail that is far too thin.
2. **Replacement size distribution (fit here).** Use `size = 1 + LogNormal(0.9, 1.7)` with probability 0.93, else `Pareto(x_m = 10, α = 0.9)`, capped at 4,096. It matches all nine CBP size classes, for both the share of establishments and the share of employment, within about 2 percentage points.
3. **Viewers see the size-biased distribution.** Scenario viewers are sampled per person, not per workplace. So the median worker is in an establishment of 50–99 people and a firm of 750–999 (SUSB 2022). Firms are not workplaces: 36.3% of private jobs are in the 2,262 firms that have 5,000+ employees.
4. **Hierarchy (CMRH, French manufacturing 2002–07).** 18%, 28%, 35% and 19% of firm-years have 0, 1, 2 and 3 management layers. Layers are mostly adjacent and pyramid-shaped. Each layer up pays about 1.4–1.9× the layer below.
5. **Span of control is skewed (Gallup 2025).** Managers have a median of 5–6 direct reports and a mean of 12.1. 37% have fewer than 5 and 13% have 25 or more. Consistent hashing with lognormal arc weights (σ ≈ 1.1) reproduces this shape (fit here). A fixed branching factor b cannot.
6. **Tenure (BLS, Jan 2026).** Median tenure is 4.1 years overall: 3.0 at ages 25–34 and 9.6 at 55–64. 20.6% of workers have 12 months or less. Completed job durations are much shorter than tenure measured mid-job: 61% of jobs started at ages 18–24 end within a year (NLSY79).
7. **Job-spell model (fit here).** Draw job length `D ~ LogNormal(µ(a), σ(a))` in years, interpolating (µ, σ) linearly from (−0.53, 1.90) at start age ≤21 to (1.354, 1.68) at start age ≥50. This reproduces BLS median tenure by age (simulated 2.9/5.1/7.1/9.2 vs actual 3.0/4.7/7.0/9.6) and the NLSY completed-duration shares.
8. **Flows.** Hires and separations each run at 3.3% of employment per month (JOLTS 2025), and 60.6% of separations are quits. Employer-to-employer moves are 2.6% of the employed per month (CPS). About 45% of hires replace a quit, and about 55% of establishments have zero net headcount change from one quarter to the next (Elsby et al.).
9. **Who talks to whom.** Same business unit multiplies the email rate by 6.2 and same function by 2.1 (an IT firm with 30k staff, 2006). Tie probability falls off as exp(−0.94·h), where h is distance in the org tree (HP Labs). A knowledge worker receives about 117 emails and 153 chat messages a day (Microsoft 2025).
10. **Construction.** Make the person's career the single primitive (a list of spells, each with employer and level). Derive rosters by inverse enumeration over a bounded set of feeder labor pools, using memoized "pool career tables" (§8). Heap indexing by seniority and per-epoch permutations are naive; §8.8 says exactly why and what to use instead.

---

## 1. Firm and establishment size distributions

### 1.1 Zipf, and its correction

- **Axtell (2001, *Science* 293:1818).** Using Census data on the whole population of US tax-paying firms (about 5.5M firms, 1997), he found that the probability a firm has more than s employees is proportional to 1/s. The Pareto exponent was α = 1.059, and later literature reports estimates of 0.996–1.059. Luttmer (2010) found α = 1.06 in SBA data (as cited by Kondo et al.).
- **Kondo, Lewis & Stella (2018, FEDS 2018-075)** used maximum likelihood on the Census Longitudinal Business Database (all non-farm private employers, 1982–2012; about 6.6M establishments and 5.0M firms in 2012). Their findings:
  - Fit to the whole sample, the best Pareto has α ≈ 0.55–0.62, far from Zipf. A lognormal fits better "even far in the upper tail".
  - Shifted lognormal (size = 1 + LN), 2012: establishments µ = 1.37, σ = 1.61; firms µ = 1.14, σ = 1.80.
  - Lognormal–Pareto mixture, 2012. Establishments: µ = 1.24, σ = 1.60, lognormal weight p = 0.91, Pareto x_m = 4.47, α = 0.83. Firms: µ = 0.97, σ = 1.52, p = 0.89, x_m = 3.62, α = 0.68.
  - Zipf does hold in a *band* of the tail. For 1997 firms, the Pareto α fitted above a size threshold is 1.05 (threshold 10), 1.11 (25), 1.12 (50), 1.10 (100), 1.01 (500–1,000) and 1.23 (10,000).
  - By sector (average 1982–2012 lognormal), manufacturing establishments have µ = 2.28, σ = 1.76; services establishments have µ = 1.27, σ = 1.57. Manufacturing is larger and has a heavier tail.

**Takeaway.** Model the body as a lognormal and add a Pareto tail with α near 1 above roughly 10 employees. A pure Pareto overweights giants. A pure lognormal underweights large employers' share of jobs.

### 1.2 Firms (enterprises), US 2022: SUSB, computed here from `us_state_naics_detailedsizes_2022.txt`

There are 6,395,635 employer firms, 8,298,562 establishments and 135,748,407 employees. Mean firm size is 21.2 and mean establishment size is 16.4.

| Firm size | % of firms | % of employment | Cumulative % of employment |
|---|---|---|---|
| <5 | 63.00 | 4.6 | 4.6 |
| 5–9 | 16.17 | 5.0 | 9.7 |
| 10–19 | 10.27 | 6.5 | 16.2 |
| 20–99 | 8.77 | 16.2 | 32.4 |
| 100–499 | 1.46 | 13.5 | 45.9 |
| 500–999 | 0.16 | 5.3 | 51.2 |
| 1,000–4,999 | 0.13 | 12.5 | 63.7 |
| 5,000+ (2,262 firms) | 0.04 | 36.3 | 100 |

Firms under 500 employees hold 45.9% of employment, which matches the SBA Advocacy FAQ figure. The worker-weighted median firm is in the 750–999 class.

**By sector (SUSB 2022, computed here).** Columns are mean firm size, mean establishment size, and the share of employment in firms of <20 / <500 / 5,000+:

| Sector (NAICS) | Firm mean | Establishment mean | <20 | <500 | 5,000+ |
|---|---|---|---|---|---|
| Construction (23) | 9.4 | 9.2 | 35% | 81% | 7% |
| Manufacturing (31–33) | 50.9 | 42.7 | 8% | 41% | 33% |
| Retail (44–45) | 24.7 | 15.2 | 16% | 34% | 58% |
| Information (51) | 40.7 | 22.6 | 7% | 27% | 51% |
| Finance & insurance (52) | 27.9 | 14.2 | 10% | 28% | 52% |
| Professional, scientific & technical (54) | 11.6 | 10.4 | 23% | 55% | 27% |
| Management of companies (55) | 144.1 | 71.5 | 0% | 10% | 64% |
| Health care (62) | 30.5 | 21.7 | 13% | 44% | 35% |
| Accommodation & food (72) | 24.0 | 17.9 | 20% | 63% | 22% |
| Other services (81) | 7.5 | 6.9 | 48% | 85% | 6% |

### 1.3 Establishments, US 2022: CBP `cbp22us.txt`, computed here

| Establishment size | <5 | 5–9 | 10–19 | 20–49 | 50–99 | 100–249 | 250–499 | 500–999 | 1,000+ |
|---|---|---|---|---|---|---|---|---|---|
| % of establishments | 55.7 | 17.6 | 12.4 | 8.9 | 2.9 | 1.6 | 0.45 | 0.17 | 0.11 |
| % of employment | 5.6 | 7.1 | 10.3 | 16.4 | 12.2 | 14.9 | 9.4 | 7.0 | 17.1 |

The worker-weighted median establishment size is in the 50–99 class (the cumulative employment share crosses 50% there). Office-heavy industries are similar: Computer Systems Design (5415) has 72.5% of establishments under 5 and 17.8% of employment in 1,000+ establishments. Finance has 23.8% of employment in 1,000+ establishments.

### 1.4 What Internot's workplace-size distribution should be

I sampled 400k draws per model and compared them with the CBP shares above (fit here):

| Model | Mean | Median | p99 | Worker-weighted median | % of establishments per class (<5 … 1,000+) | % of employment per class (<5 … 1,000+) |
|---|---|---|---|---|---|---|
| CBP 2022 actual | 16.4 | 1–4 | — | ~90 | 55.7 17.6 12.4 8.9 2.9 1.6 .5 .2 .1 | 5.6 7.1 10.3 16.4 12.2 14.9 9.4 7.0 17.1 |
| Current `LN(2.0, 1.5)` | 23.7 | 8 | 242 | — | 37.1 19.7 17.4 15.6 6.0 3.2 .7 .2 .1 | 3.5 6.5 11.2 21.5 18.1 20.4 10.0 5.4 3.5 |
| Lognormal-only MLE, `1+LN(0.98, 1.82)` | 14.8 | 4 | 184 | — | (fits by construction) | 8.1 7.9 10.8 18.1 14.8 17.3 9.9 6.3 6.8 |
| **Recommended mixture, cap 4,096** | 17.2 | 4 | 190 | 83 | 54.3 17.2 13.8 9.4 3.0 1.6 .4 .2 .1 | 6.9 6.6 10.9 16.6 12.2 13.7 8.1 6.2 18.9 |

The recommended mixture is `1 + LN(µ = 0.9, σ = 1.7)` with probability 0.93, else `Pareto(x_m = 10, α = 0.9)`, rounded and capped at 4,096. Notes:

- **Cap.** The cap is the 12-bit member index (`MAX_WORKPLACE_SIZE`). Establishments of 1,000+ hold 17.1% of jobs, and a few (hospitals, head-office campuses) exceed 4,096. With the cap, the mixture still puts 18.9% of employment in 1,000+ workplaces, so the cap costs little (inferred).
- **Industry.** Shift µ per sector using §1.2 (for example manufacturing +1.0, other services −0.4, relative to the all-sector fit) (inferred).
- **Firms.** Add a *firm* layer above workplaces for large employers. In SUSB 2022, 1,386,100 establishments (16.7% of all) belong to firms with 500+ employees, and the 2,262 firms with 5,000+ employees operate 913,729 establishments (about 400 each). So "same firm, other workplace or city" is a real cross-workplace tie.
- **Time variation.** Headcounts change over time. Use `n_W(t) = n_W · g_W(t)`, with `g` built from `trajectory::smooth`, plus open and close dates (§8.3).

---

## 2. Hierarchy structure

### 2.1 Layers versus firm size: Caliendo, Monte & Rossi-Hansberg (CMRH)

*JPE* 2015 (NBER WP 18259, 2012). The data are French manufacturing firms, 2002–2007, with 72,918–79,260 firms per year. Layers come from PCS occupation codes:

- layer 0: clerks and blue-collar workers (their wages are almost identical);
- layer 1: supervisors, including quality-control technicians and technical, accounting and sales supervisors;
- layer 2: senior staff, including CFOs, heads of HR, and logistics and purchasing managers;
- layer 3: CEO or directors.

Findings:

- **Distribution of layers.** The average is 1.50–1.59 management layers. Firm-years with 0/1/2/3 management layers number 81,909 / 126,069 / 161,449 / 87,211, which is **17.9% / 27.6% / 35.4% / 19.1%**. Mean annual hours by layer count are 7,946 / 16,450 / 85,674 / 227,070. At about 1,600 hours per full-time worker, that is roughly 5 / 10 / 53 / 141 FTE (inferred). Value added rises steeply with layers.
- **Adjacency.** 81.6% of firms have adjacent layers starting at layer 0; weighted by value added it is 96.7%. Firms with gaps are the small ones.
- **Pyramid in hours.** The lower layer has more hours than the one above it in 74–87% of cases for each adjacent pair. But only 62.0% of 2-layer firms and 54.3% of 3-layer firms are pyramidal at every step.
- **Pyramid in wages.** Wages rise with layer in 88–97% of cases for each pair, and 79.7% of 3-layer firms are ordered at every step.
- **Pay by layer.** Median hourly wages (2005 €) are 15.2–15.6 for workers, 21.9 for supervisors, 36.0 for senior staff and 54.6 for CEOs. Representative 3-layer firm means are 16.1 / 23.1 / 38.6 / 72.0, so each step up is **×1.44, ×1.67, ×1.86**. About 50% of within-firm wage variance lies across layers (66% in 3-layer firms).
- **Dynamics.** 60–70% of firms keep the same layer count from year to year. Changes are almost always by one adjacent layer (75.4% of 0-layer firms that add a layer add layer 1; 82.9% for 1-layer firms). Firms that expand by adding a layer *lower* wages in the layers that already existed (−13.7%).

**Portugal replication:** Caliendo, Mion, Opromolla & Rossi-Hansberg, *JPE* 2020, Quadros de Pessoal data. Layers are top executives / middle managers and supervisors / higher-skilled professionals / the rest. More than 75% of firms have adjacent layers, and more than 76% of pairwise comparisons are pyramidal. About half of firms keep their layer count in a year (70% for firms with 3 management layers). About 12% add a layer and about 12% drop one. About 20% restructure every year.

### 2.2 Span of control, manager share, flattening

- **Gallup** (US managers). The average number of direct reports was 12.1 in 2025, up from 10.9 in 2024 and roughly 50% above 2013. The median is 5–6. In 2022–24 web surveys of 16,442 managers, 37% had fewer than 5 reports, 66% fewer than 10, 22% had 10–24 and 13% had 25 or more. The median manager spends 40% of their time on individual-contributor work.
- **Rajan & Wulf** (Hewitt data, about 300 large US firms, 1986–1999). The CEO's direct reports rose from 4 to 7 (median, balanced panel of 51 firms). The number of levels between the CEO and a division head fell from 1.58 to 1.15.
- **Manager share** (OEWS May 2025; total employment 155,495,730):
  - Management occupations (SOC 11-0000): 11,132,700, or **7.2%**, mean wage $145,260 versus $69,770 for all occupations.
  - Chief executives: 204,350 (0.13%). General and operations managers: 3,503,020 (2.25%).
  - First-line supervisors are coded inside each occupational group. The rows I retrieved (office 1.44M, retail 1.12M, non-retail sales 0.21M, building/grounds 0.31M, police/fire/security/corrections 0.44M) sum to 3.5M, or 2.2%. Food-service, construction, production, maintenance and transport supervisors were not retrieved.
  - So people-managers plus supervisors are about **10–12% of workers** (inferred). That implies about 8 workers per manager, consistent with Gallup's median and mean.

### 2.3 Team sizes

The only large-sample team-size figure I found is Gallup's span distribution above: median team about 5–6, heavily right-skewed. Amazon's "two-pizza team" (roughly 6–10) is folklore and I found no primary source for it. For Internot, a team is a manager plus their direct reports. Its size distribution is the span distribution, mixed over workplace size, because in a 4-person workplace the owner's span is at most 3.

---

## 3. Occupations by level

- **SOC encodes function, not seniority.** The only level signals in SOC are management (11-xxxx) and first-line supervisors (xx-1xxx). CMRH and the Portuguese study both build layers from national *hierarchical* occupation codes (France's PCS-ESE and Portuguese Decreto-Lei 121/78), not from SOC. So Internot needs two coordinates: **job family** (a SOC detailed occupation, persistent across jobs) and **level** (a grade ladder).
- **National mix** (OEWS May 2025, employment / mean wage):
  - office & admin 17.75M / $51,560
  - food preparation & serving 13.68M / $37,150
  - sales 13.42M / $54,960
  - management 11.13M / $145,260
  - business & financial 10.54M / $95,230
  - healthcare practitioners 9.82M / $108,700
  - education 9.10M / $67,540
  - computer & mathematical 5.26M / $120,080
  - Largest white-collar occupations: software developers 1.69M, accountants & auditors 1.45M, project management specialists 1.07M, management analysts 0.90M.
  - Industry-specific staffing patterns (the occupation mix within each NAICS industry) are in the OEWS industry tables (bls.gov/oes/tables.htm). I did not extract them here.
- **Wage versus level.**
  - Each management layer is worth ×1.4–1.9 in median wage (CMRH, above).
  - Promotions come with raises that are small relative to the gap between levels. Murphy (1985), in large US manufacturers: a VP promoted to president got a 21% real raise, while presidents earned 60% more than VPs (cited in Waldman 2008).
  - Promotions nearly always carry a raise: about 90% of NLSY promotions (MLR 1999).
- **Recommended mapping (inferred).** `title(industry, family, grade, layer)`:
  - layer 0: `{Junior, –, Senior, Staff, Principal} × family title` (for example "Senior Software Developer");
  - layer 1: the family's supervisor or manager SOC (43-1011 "Office Supervisor", 11-3021 "Engineering Manager");
  - layer 2: "Director of <function>";
  - layer ≥3: "VP / Chief <function> Officer";
  - the head of the workplace: 11-1011 or 11-1021, which in small workplaces means "Owner / General Manager".
  - Industry picks the function mix: a bank is mostly tellers, loan officers and admin; a software firm mostly software developers, product managers and sales.
  - The current single ladder `Junior…Principal → Manager…Exec` mixes the individual-contributor (IC) track with the management track. Split it into a *grade* (personal seniority) and a *layer* (position in this workplace's tree), and derive the layer from the grade and the workplace size (§8.6).

---

## 4. Tenure and mobility

### 4.1 Tenure (BLS Employee Tenure, CPS supplement, wage and salary workers)

| | Jan 2024 | Jan 2026 |
|---|---|---|
| All 16+ | 3.9 | **4.1** |
| 16–17 / 18–19 / 20–24 | 0.7 / 0.9 / 1.4 | 0.7 / 0.8 / 1.5 |
| 25–34 / 35–44 / 45–54 | 2.7 / 4.6 / 7.0 | 3.0 / 4.7 / 7.0 |
| 55–64 / 65+ | 9.6 / 9.8 | 9.6 / 9.9 |
| Men / women | 4.2 / 3.6 | 4.3 / 4.0 |

Other Jan 2026 figures:

- 20.6% of workers have 12 months or less of tenure (74.1% at ages 16–19, 9.2% at 55–64).
- 10+ years of tenure: 31.1% of men 25+, 29.0% of women 25+, 52.6% at ages 60–64, 20.3% at 35–39.
- Public sector 5.6 vs private 3.9 years. Highest private industry: financial activities, 5.0. Lowest: leisure and hospitality, 2.4.
- Management/professional occupations 4.9 vs service 2.9.

**Caution:** these are *elapsed* tenures of people currently employed. That is a length-biased sample of spells, so they are much longer than *completed* spell durations. Property tests must measure elapsed tenure at a snapshot date, not the average spell length.

### 4.2 Jobs over a lifetime: NLSY79, born 1957–64, followed through 2022–23 (release Aug 2025)

- 12.9 jobs from age 18 to 58 (men 13.1, women 12.7).
- By age bracket: 5.6 (18–24), 4.5 (25–34), 2.9 (35–44), 2.2 (45–54), 1.3 (55–58). A job spanning two brackets is counted in each.
- Of jobs started at 18–24, **61% end within a year and 87% within 5 years**. Of jobs started at 45–54, **21% within a year and 56% within 5 years**.
- Share of weeks from 18 to 58: employed 77%, unemployed 4%, out of the labor force 18%. Men 83% employed and 12% out of the labor force; women 72% and 24%.
- Internot's current arc has ~10 events at a median of 3 years apart, which is too few, too evenly spaced, and not front-loaded.

### 4.3 Flow rates

- **JOLTS 2025 annual.**
  - Hires: 63.0M (monthly average rate 3.3%). Total separations: 62.8M (3.3%).
  - Quits: 38.0M, 2.0%, 60.6% of separations. Layoffs and discharges: 21.2M, 1.1%, 33.8%. Other separations (retirement, death, transfers): 3.5M, 0.2%, 5.6%.
  - Accommodation and food services has the highest quit rate (4.8%); leisure and hospitality the highest hires rate (6.3%).
- **Employer-to-employer moves** (Fallick & Fleischman, CPS 1994–2003): 2.6% of employed people change employer each month. That is more than twice the employment-to-unemployment flow. Nearly two-fifths of new jobs were employer changes.

### 4.4 Promotions and internal moves

- **NLSY79 (MLR, Dec 1999).** Two-year internal promotion rates:
  - 1988–90 (ages about 25–32): men 34.2%, women 31.0%.
  - 1994–96 (ages 32–39): men 25.4%, women 26.0%.
  - About 90% of promotions came with a raise and more than 80% with more responsibility. A third of people who were *not* promoted also reported more responsibility.
- **Self-reported promotions often don't change the position.** Pergamit & Veum found that most self-reported "promotions" involve no change in position or duties. So the rate at which people move up a *layer* is well below the ~15%/year self-reported rate (inferred).
- **Internal labor markets** (Baker, Gibbs & Holmstrom 1994; Waldman 2008 survey):
  - Demotions are rare.
  - Fast tracks exist: people promoted quickly tend to be promoted quickly again.
  - Schooling raises promotion odds.
  - There are no clean "ports of entry": **firms hire from outside at every level**, not only at the bottom.

### 4.5 Unemployment, participation, retirement, self-employment

- **Unemployment duration** (CPS, Aug 2026, stock of unemployed): median 11.4 weeks, mean 26.3. Under 5 weeks: 28%; 5–14 weeks: 29%; 15+: 43%; 27+: 27%. A lognormal with median 11.4 weeks and σ = 1.29 matches the mean and gives 26% under 5 weeks and 25% over 27 weeks (fit here). Completed spells are shorter than this stock, because the stock is length-biased (inferred).
- **Labor force participation, 2025** (BLS): 16–19: 36.2%; 20–24: 71.4%; 25–34: 83.6%; 35–44: 84.4%; 45–54: 82.5%; 55–64: 66.6%; 65–74: 26.7%; 75+: 8.5%. All 16+: 62.4% (men 67.8%, women 57.3%).
- **Retirement.** Average retirement age rose from 62 to 64 for men and from 60 to 62 for women between the mid-1990s and about 2011 (Center for Retirement Research). Calibrate the retirement hazard so the synthetic population reproduces the participation rates above (inferred).
- **Self-employment, 2015** (BLS Spotlight): 15.0M people, 10.1% of employment. Unincorporated 9.5M, incorporated 5.5M. The rate was 12.1% in 1994. By age: 16–24 is 1.9% unincorporated + 0.3% incorporated; 65+ is 15.5% + 8.6%. By sex: men 7.4% + 4.9%; women 5.2% + 2.3%.

### 4.6 Establishments hold headcount steady while people churn

Elsby, Michaels, Gottfries & Ratner, "Vacancy Chains" (Philadelphia Fed WP 22-23), using QCEW and JOLTS microdata:

- **Steady headcount.** About 55% (QCEW) or 65% (JOLTS) of establishments report *exactly* the same employment as the previous quarter. After 1 year it is 42% (QCEW) or 47% (JOLTS); after 2 years, 35% or 39%.
- **But they still churn.** Establishments with zero net change still hire 3.6% of their workforce per quarter, 2.3 points of it replacing quits. Over two years they replace 35% of their workforce.
- **Replacement hiring.** Hires that replace a quit are **about 45% of all hires** (48% in 2007, about 35% at the trough of the Great Recession). The calibrated model's average vacancy-chain length (hires generated per new position) is about 2. Recalls are about 20% of hires (Fujita & Moscarini 2017, as cited).

The implication is that *positions persist while people flow through them*. That is the empirical case for a seat-based org chart.

---

## 5. Consistent mobility models

### 5.1 White's vacancy chains (1970)

Harrison White's *Chains of Opportunity* (Harvard UP 1970; the empirical case was clergy moves) reverses the usual viewpoint. The objects that move are **vacancies**, not people. A vacancy is created by a retirement, a death, an exit, or a new position. It is filled by someone from another position, which moves the vacancy to that person's old slot, usually one level down. The chain ends when a vacancy is filled from outside the system.

Every move of a person from A to B is the same event as a move of a vacancy from B to A. **A person's job history and an organization's vacancy history are literally two readings of one set of events.**

White's formalization is an absorbing Markov chain over strata. Q[i][j] is the probability that a vacancy at stratum i is filled from stratum j, and p_i0 is the probability that it is filled from outside. The expected chain length starting at stratum i is `[(I − Q)⁻¹ · 1]_i`. This is the standard absorbing-chain result (I did not re-verify White's clergy parameter values for this note).

### 5.2 Markov manpower planning (Bartholomew and successors; standard formulation, no source fetched here)

- **Push model.** Grade stocks evolve as `n(t+1) = n(t)·P + R(t+1)·r`, where P holds promotion and retention probabilities, rows sum to 1 minus the wastage rate, R is total recruits, and r is how recruits spread over grades.
- **Pull (renewal) model.** Grade sizes are fixed and promotions happen only when vacancies open. This is White's model in aggregate form.

Both give *flows and stocks by grade*, not identities. They are the right source of **targets** (grade sizes, promotion rates, recruitment by level), and they provide a pull-model rule for a seat-based simulation.

### 5.3 Matching and replacement hiring

Search-and-matching models with on-the-job search, plus Elsby et al.'s sunk "positions", explain the aggregate flows: quits trigger replacement hires, and chains average about 2 hires. Like manpower models, they carry no identities.

### 5.4 Which formulation gives "one object, two views"?

None of these models alone does. In the real data, the single object is the **linked employer–employee job table**: records of (person, employer, period), as in LEHD. Worker-flow statistics and firm statistics are both projections of that table.

The stateless analogue is a set of **spells** `(person, employer, layer, [start, end))` that is:

1. generated from exactly one side, so it is consistent by definition, and
2. indexable from the other side at bounded cost.

White's duality tells you what realism to aim for: moves come in chains, replacement is common, and hires happen at every level. The index structure is what makes both views cheap. §8 gives the construction.

---

## 6. Who actually talks to whom at work

- **Formal structure constrains communication** (Kleinbaum, Stuart & Tushman, *Org Sci* 2013).
  - Data: a large IT firm, 30,328 employees, email from Sep to Dec 2006. That is 114M dyadic message records, or 13M after dropping mass mail (more than 4 recipients), Bcc and administrative assistants.
  - Average number of distinct correspondents: 38 for men, 46 for women.
  - Among communicating pairs, **59.0% share a business unit** versus 17.0% of all pairs; 60.6% share a function versus 31.7%; 14.4% share an office versus 1.4%.
  - Controlling for everything else, a same-business-unit pair emails at **×6.18** the rate of a cross-unit pair. Same function gives ×2.13; same function and sub-function gives ×6.84. Same office also has a large effect, which decays with the log of distance between offices.
- **Decay with distance in the org tree** (Adamic & Adar 2005, HP Labs).
  - 430 people over about 3 months. A contact means at least 6 emails in each direction; emails with more than 10 recipients were removed.
  - Median 10 contacts, mean 12.9. Average shortest path 3.1 hops.
  - P(tie) ≈ **exp(−0.94·h)**, where h = 1 for your manager and for peers who share your manager, and grows recursively up the tree. P(tie) scales as g^(−3/4), where g is the size of the smallest unit containing both people.
  - Communication "clings" to the org chart, with occasional cross-unit bridges.
- **Informal ties churn; structure persists** (Kossinets & Watts, *Science* 2006; 43,553 students, faculty and staff at a university, one academic year of email). New ties form through shared affiliations and triadic closure. Network-level statistics stay near equilibrium while individual ties are unstable.
- **Remote work silos teams** (Yang et al., *Nat Hum Behav* 2022; 61,182 US Microsoft employees, Dec 2019 to Jun 2020). Firm-wide remote work:
  - cut the share of collaboration time spent with cross-group connections by about 25% of its pre-pandemic level;
  - reduced bridging ties;
  - made networks more static, with fewer ties added or dropped each month;
  - shifted time from synchronous to asynchronous media (meetings and calls −5%, more email and chat).
- **Volume** (Microsoft Work Trend Index 2025; M365 telemetry, plus a survey of 31,000 knowledge workers in 31 markets).
  - About 117 emails received per workday and about 153 Teams messages per weekday.
  - An interruption about every 2 minutes (about 275 per day).
  - 57% of meetings are ad hoc calls with no calendar invite.
  - More than 50 messages sent or received outside core hours; meetings after 8 pm are up 16% year over year.
  - The Enron corpus (CMU) has about 0.5M messages across about 150 mailboxes, mostly senior management. It is useful for senior-manager email patterns only.
  - I found **no clean public breakdown of email or meeting volume by role** (manager vs individual contributor). Gallup's "40% of manager time on IC work" is the only role-level anchor I found.

**Implications (inferred).** Weight coworker ties by a product of factors: `exp(−0.94·h)` in org-tree distance, ×6 for the same business unit, ×2 for the same function, and a large factor for the same office. Ties to your team and your manager should dominate volume. Cross-team bridges should be few, weak and time-varying, keyed by project or epoch. Manager volume should scale with span.

---

## 7. Realism targets (candidates for property tests)

| Metric | Target | Source |
|---|---|---|
| Share of workplaces with <5 employees | 55.7% (±3 pp) | CBP 2022 |
| Share of jobs in workplaces <5 / 20–49 / 100–249 / 1,000+ | 5.6 / 16.4 / 14.9 / 17.1% (±3 pp) | CBP 2022 |
| Mean workplace size | 16.4 (15–19 with cap) | CBP 2022 |
| Worker-weighted median workplace size | in 50–99 | CBP 2022 (fit here) |
| Share of jobs in firms <500 / 5,000+ | 45.9% / 36.3% | SUSB 2022 |
| Pareto α of firm tail (thresholds 25–1,000) | 1.0–1.12 | Kondo et al. 2018 (1997 LBD) |
| Share of workplaces with 0/1/2/3 management layers (sizes ≥ ~5) | ≈18/28/35/19% | CMRH 2002–07 |
| Adjacent layers from 0 | ≥80% of workplaces | CMRH |
| Lower layer has more people than the next one up (each pair) | ≥74% | CMRH Table 5 |
| Median wage ratio between adjacent layers | 1.4–1.9 | CMRH |
| Year-over-year share keeping the same layer count | 50–70% | CMRH; Portugal |
| Mean / median direct reports per manager | 12.1 / 5–6 | Gallup 2025 |
| Managers with <5 / <10 / ≥25 reports | 37 / 66 / 13% | Gallup 2022–24 |
| Head's direct reports, workplaces 1,000+ | median ≈7 | Rajan & Wulf 1999 |
| Managers plus supervisors as share of workers | 10–12% | OEWS 2025 (inferred) |
| Chief executives per 1,000 workers | ≈1.3 | OEWS 2025 |
| Median tenure, all / by age 25–34 / 35–44 / 45–54 / 55–64 | 4.1 / 3.0 / 4.7 / 7.0 / 9.6 yrs | BLS Jan 2026 |
| Share of workers with ≤12 months tenure | 20.6% | BLS Jan 2026 |
| Share of workers 25+ with ≥10 years tenure | 29–31% | BLS Jan 2026 |
| Jobs started at 18–24 ending <1 yr / <5 yrs | 61 / 87% | NLSY79 |
| Jobs started at 45–54 ending <1 yr / <5 yrs | 21 / 56% | NLSY79 |
| Jobs per person from 18 to 58 (by bracket) | 12.9 (5.6/4.5/2.9/2.2/1.3) | NLSY79 |
| Monthly hires rate = separations rate | 3.3% | JOLTS 2025 |
| Quits share of separations | 60.6% | JOLTS 2025 |
| Monthly employer-to-employer move rate | 2.6% of employed | Fallick & Fleischman |
| Share of workplaces with zero net change quarter over quarter | ≈55% | Elsby et al. (QCEW) |
| Replacement hires as share of hires | ≈45% | Elsby et al. (JOLTS) |
| Two-year promotion rate, ages ~25–32 / ~32–39 | 31–34% / 25–26% | NLSY79 (MLR 1999) |
| Demotions per year | ≪ promotions (<1–2%, inferred) | BGH; Waldman 2008 |
| Unemployment duration median / mean (stock) | 11.4 / 26.3 weeks | CPS Aug 2026 |
| Participation rate 16–19 / 20–24 / 25–54 / 55–64 / 65–74 / 75+ | 36 / 71 / 84 / 67 / 27 / 9% | BLS 2025 |
| Share of weeks employed / unemployed / out of labor force, ages 18–58 | 77 / 4 / 18% | NLSY79 |
| Self-employment share (all / 65+ / 16–24) | 10 / 24 / 2% | BLS 2015 |
| Average retirement age, men / women | ≈64 / ≈62 | CRR (~2011) |
| Retirees, the dead, children or the unemployed with an employer or manager | exactly 0 | invariant |
| Roster(W, t) contains x ⇔ employer(x, t) = W | always | invariant |
| Communicating pairs in the same business unit | ≈59% (baseline 17%) | Kleinbaum et al. 2013 |
| Email rate ratio for same business unit / same function | 6.2× / 2.1× | Kleinbaum et al. 2013 |
| P(sustained tie) vs org-tree distance h | ∝ exp(−0.94·h) | Adamic & Adar 2005 |
| Sustained email contacts per person | median 10, mean 12.9 | HP Labs |
| Emails / chat messages received per knowledge worker per day | ≈117 / ≈153 | Microsoft 2025 |

---

## 8. Construction recommendations

### 8.1 Principle: one primitive, two views

The single primitive is the **person's career**: an ordered list of spells, generated only from `hash(person_id, k, seed)` and the person's attributes. The roster of workplace W at time t is *defined* as `{x ∈ feeders(W) : employer(x, t) = W}`. Consistency is then automatic. The only engineering problem is making `feeders(W)` bounded and enumerable.

Everything in the org chart (layer, manager, team) is derived from the roster at t plus stable per-spell keys. Nothing about a workplace is stored.

### 8.2 Identity and population turnover

Keep the 32-bit id and its widths, but change what the top 20 bits mean. Today they mean "workplace". They should mean **home labor pool**: where the person enters the labor market.

```
person_id: u32 = home_ind:6 | home_city:6 | pool:8 | member:12
birth(x)  = cohort_warp(T0 + (member + jitter(x)) · Δ)   // monotone in member
death(x)  = birth(x) + mortality_sample(x)               // life table (not sourced here)
```

Because birth date is monotone in `member`, "alive and of working age at t" is a **contiguous member range**. That is `Space::find().where_range("member_idx", lo, hi)`, which pushes down cleanly.

Over a 100-year window, Δ ≈ 9 days. That gives about 41 births per year per pool, about 2,400 people aged 16–75 alive at any time, and about 1,400 employed (inferred). Births and deaths come for free: people age in and out of the member range. `cohort_warp` shapes cohort sizes, for example a baby boom.

People change city through their employment spells. Residence city at t is the city of the current workplace if employed, otherwise the city of the last spell. `home_city` is only the entry city.

### 8.3 Workplaces and blocks

Workplaces get their own id namespace:

```
W = ind:6 | city:6 | block:8 | slot:12
```

Block β = (ind, city, block) is served by **home pool** p(β) = (ind, city, pool = block) and by a small **neighbor set** N(β): up to 4 blocks in the same city in other industries, and up to 4 blocks in the same industry in other cities. N must be symmetric (β' ∈ N(β) ⇔ β ∈ N(β')), or at least cheaply invertible by scanning the ≤64 industries that can point at β.

`workplaces_of(β)` draws sizes from the §1.4 mixture, with µ shifted by industry, until Σ n_W reaches the pool's expected employment. This is a small, pure, memoized function.

Each workplace also gets:

- an open date and a close date from hashes;
- a growth path `g_W(t)` from `trajectory::smooth`;
- target size `n_W(t) = n_W · g_W(t) · 1[open_W ≤ t < close_W]`.

A workplace bigger than its block's capacity gets extra feeder pools (adjacent `pool` values). Large workplaces map to a firm id (`firm_of(W)`) that links establishments across cities.

### 8.4 Careers (the person side)

```
fn career(x) -> Vec<Spell>:                        // pure; memoized per pool (§8.5)
  t = entry_age(x, education); k = 0
  while t < death(x):
    if retires(x, t): push(Retired, t..death(x)); break   // hazard fit to participation by age (§4.5)
    s = next_state(x, k, age(t), prev_state)              // Employed | Unemployed | NotInLabor | SelfEmployed
    match s:
      Employed:
        β' = home block w.p. 1-μ, else pick from N(home)  // μ ≈ 0.1–0.2 (inferred)
        W  = pick ∝ n_W(t) over workplaces_of(β')         // inverse-CDF on hash(x,k,"emp")
        ℓ  = hire_layer(W, experience(x,t), hash)          // P(ℓ) ∝ W's layer pyramid; outside hires at every level
        d  = LogNormal(µ(a), σ(a))                         // (−0.53,1.90)@≤21 → (1.354,1.68)@≥50, in years
        sub-events inside d: promotions (≈15%/yr early, declining; mostly grade-only), rare demotions
      Unemployed:  d = LogNormal(ln(11.4 weeks), 1.29)   // shorten for completed spells
      NotInLabor:  d, and its probability, by age and sex, fit to participation and NLSY weeks
    push(Spell{s, W, ℓ, start: t, end: t+d, key: hash(x,k,"key")}); t += d (+ gap); k += 1
```

**Validation (fit here).** I simulated 40k people with only the employed/unemployed part of this model: entry at 18–23, retirement ~N(64, 4), 40% of moves direct, other gaps drawn from the unemployment lognormal. Results against targets:

| Check | Simulated | Target |
|---|---|---|
| Median elapsed tenure, 20–24 / 25–34 / 35–44 / 45–54 / 55–64 | 1.0 / 2.9 / 5.1 / 7.1 / 9.2 | 1.5 / 3.0 / 4.7 / 7.0 / 9.6 |
| Jobs started at 18–24 ending <1 yr / <5 yrs | 59 / 86% | 61 / 87% |
| Jobs started at 45–54 ending <1 yr / <5 yrs | 23 / 59% | 21 / 56% |
| Workers with ≤12 months tenure | 18.3% | 20.6% |
| Workers 25+ with 10+ years | 29.6% | 29–31% |
| Jobs per bracket, 35–44 / 45–54 / 55–58 | 2.8 / 2.2 / 1.3 | 2.9 / 2.2 / 1.3 |

Two gaps remain. Early-career job counts are too low (3.6 vs 5.6 at 18–24): add teen and student short and concurrent jobs. The share of weeks employed is too high (89% vs 77%): add out-of-labor-force spells, especially for women and around childbirth.

### 8.5 Rosters (the employer side) and query costs

```
fn pool_table(p) -> PoolTable:          // memoized; pure function of (p, seed)
  for x in p.members (all cohorts): for s in career(x): if Employed: by_workplace[s.W].push(s)
  // each by_workplace[W] is an interval index keyed by [start, end)

fn roster(W, t) = ⋃_{p ∈ feeders(block(W))} pool_table(p).by_workplace[W].stab(t)
fn employer(x, t) = career(x).binary_search(t)
```

This answers the coordinator's question directly. Members join and leave at different times, and each join or leave *is* a person's spell boundary. `roster(W, t)` is an interval-stabbing query over those same spells. The roster and each career cannot disagree because they are the same records.

| Query | Cost |
|---|---|
| `career(x)` | O(J), J ≈ 15–25 spells, about 100 hashes |
| `employer(x, t)` | O(log J) |
| `pool_table(p)`, first touch | O(4,096 · J), about 10⁵ hashes, ≈1–5 ms (inferred) |
| `roster(W, t)`, cold | at most (1 + \|N\|) ≤ 9 pool tables, ≈10–50 ms |
| `roster(W, t)`, warm | O(log S + k) |
| `manager(X, t)` | employer + roster + O(log n) ring lookup |
| `history(X)` | O(J), plus one roster per spell if the manager at each spell is needed |

Memory is about 1–2 MB per pool table; keep them in an LRU (inferred).

Retirees, children, the unemployed and the dead have no Employed spell. So they are in no roster and `manager(x, t) = None`. That fixes the "retirees have managers" defect by construction. The "manager index doesn't exist" defect also disappears, because managers are always drawn from the actual roster.

### 8.6 Levels, layers and managers (the org chart at t)

- **Layer.** `layer(x) = min(ℓ_x, Lmax(|R|))`, where R is the roster.
  - `Lmax(n) = 0` for n ≤ 3, 1 for n ≤ 15, 2 for n ≤ 100, 3 for n ≤ 700, 4 above (inferred from a span of about 7; check against CMRH shares).
  - The **head** is the member with the largest (layer, grade, key).
  - If no member has a management layer and n ≥ 4, the member with the top grade acts as lead. This is derived, not stored.
- **Manager.** For member x, find the lowest populated layer L' above x. That skips empty layers, which reproduces CMRH's non-adjacent small firms. Then `manager(x) = ring_successor(key(x), {members at L'})`, using a sorted ring of keys with lognormal arc weights (σ ≈ 1.1).
- **Why this span model (fit here).**
  - Plain consistent hashing: mean 13.0, median 9, 27% under 5, 52% under 10, 15% at 25 or more.
  - Weights with σ = 1.1: mean 12.7, median 7, 34% / 60% / 13%.
  - Gallup: 12.1, 5–6, 37% / 66% / 13%. Mixing over small workplaces lowers the median further.
- **Stability.** A hire or an exit changes only the reporting lines inside the arc owned by the person who joined or left. That is O(span) changes, not O(n).
- **Team.** A team is `reports(m, t)`: the members whose ring successor is m. Sibling teams are the teams under the same skip-level manager.

### 8.7 Collaborators and communication

Replace the current `frequent_collab_mail_id_of` (a random member index) with draws from `roster(W, t)`. Weight each candidate by:

- `exp(−0.94·h(x, y))` using org-tree distance at t;
- ×6 for the same business unit (the same subtree under the head's direct reports);
- ×2 for the same job family.

Add sparse cross-workplace ties to former coworkers from earlier spells (the same records, t < now) and to same-firm colleagues in other cities. Feed tie strength into `graph::comm_intensity`. Calibrate so the median worker has about 10 sustained email contacts, about 40 distinct correspondents per quarter, and about 100+ emails a day including broadcast mail. Managers get volume in proportion to span (inferred).

### 8.8 Where the ideas under consideration are naive

**Heap indexing over active members sorted by seniority, with manager(i) = ⌊(i−1)/b⌋:**

1. **Rank shift.** Any hire or exit at rank r shifts every rank below r, which reassigns the manager of about everyone below r. At 3.3% monthly turnover, a 100-person workplace would be re-wired every month. Reporting lines should change only inside the affected team.
2. **Seniority is not rank.** Firms hire at every level (Baker, Gibbs & Holmstrom), and external CEOs and VPs are common. Seniority ordering puts the longest-serving clerk above a new VP, and can put a "Senior Director" under an "Associate" when the career arc says otherwise.
3. **Uniform span and forced depth.** Every manager gets exactly b reports, depth is always ⌈log_b n⌉, and managers are always ≈1/b of staff. Real spans are skewed (median 5–6, mean 12, 13% ≥25). Layer count moves in discrete, size-dependent steps (CMRH). Head span differs from front-line span (the CEO span is about 7 in large firms).
4. **No layer semantics.** Heap depth doesn't map to titles or pay layers, so title, wage and manager can contradict each other.

Use instead: order by layer (from the career) with stable per-spell keys, and assign managers by weighted ring successor within the nearest populated layer above (§8.6). A heap over *seats* is acceptable as a static display layout.

**Membership as a time predicate `active(x, t)`:** this is right, and it is the core of §8.5. It is naive only if it is evaluated per workplace independently of the career (per-t hashes flicker, and x could be "active" in two places at once), or if it has no bounded feeder set. Define it as `employer(x, t) == W` over spells.

**Job changes as permutations that evolve over time:**

1. **Blind to state.** A permutation assigns an id to a slot whether that person is employed elsewhere, retired, a child, dead or not yet born. Restricting the domain to eligible people requires knowing state, which means spells.
2. **Wrong hazards.** An independent π_e per epoch moves everyone every epoch. To make most people stay, π_e must be the identity except on the people whose spells end at e, which again means spells. Tenure dependence, age dependence and replacement hiring can't be expressed.
3. **Cost.** A position at time t is π_t ∘ … ∘ π_1(x), which costs O(t / epoch) per query unless the permutations are structured.
4. **Global balance.** An exact bijection needs equal numbers of movers and slots in each epoch, which is global information.
5. **Closed world.** A permutation within one workplace keeps headcount fixed and allows no moves between firms, no unemployment and no retirement.

Where permutations are right: as the **invertible matching step inside a bounded computation**. Examples: matching seekers to vacancies within one block and one month in §8.9, or a Feistel choice of a founder or owner from a pool's eligible members.

### 8.9 Upgrade path: a block-local seat simulation (exact vacancy chains)

If tests need Elsby-style steady headcounts (55% zero net change) or explicit vacancy chains, keep §8.4's person-side choice of **which block and when**. Hand **which seat inside the block** to a pure, memoized block simulation:

```
fn block_ledger(β) -> Ledger:                  // pure function of (β, seed); memoized
  seats  = seat trees per W in β; layer sizes follow n_W(t) (pull model)
  people = per month, the persons whose career says "employed in β" (read from pool tables; no cycle)
  for month m:
     vacancies = exits (people whose block spell ended) + growth − closures
     fill top-down (White): each vacancy takes an internal candidate from the layer below w.p. q_ℓ,
        else a new arrival in β this month (Feistel matching over the arrival list)
     append (person, W, seat, layer, [m, …)) to the ledger
```

Its cost is O(people × months), roughly 2,400 × 1,200 ≈ 3M steps per block, about 10–50 ms, then cached (inferred). It is still a pure function, and it is local, not a global pass.

Consistency holds because block membership comes only from person-side hashes, and seat assignment comes only from the ledger. Every view reads the same ledger. Moves between workplaces inside β count as job changes in `history(x)`. The limit is that person-side hazards can't depend on seat outcomes (for example, promotion lowering the quit hazard) without joint simulation. That limit is acceptable.

### 8.10 Property tests to write first (in `internot/tests/`)

1. **Two-sided agreement.** For random (W, t): every x in `roster(W, t)` has `employer(x, t) == W`. For random (x, t): `x ∈ roster(employer(x, t), t)`.
2. **Invariants.** No roster contains anyone retired, dead, under 16, unemployed or not in the labor force. `manager(x, t)`, when present, is in the same roster, sits in a strictly higher layer, and the manager graph is a tree with exactly one head.
3. **Stability.** Between t and t + 1 month, the share of workers whose manager changed is at most roughly the monthly turnover rate × 2 (inferred bound).
4. **Distributions.** Every row of §7 whose target is marked with a tolerance, measured on a sample of about 10⁵ persons at `Universe::now`.

---

## 9. Sources

- Axtell, R. (2001), "Zipf distribution of U.S. firm sizes", *Science* 293: https://pubmed.ncbi.nlm.nih.gov/11546870/
- Kondo, I., Lewis, L. & Stella, A. (2018), "On the U.S. Firm and Establishment Size Distributions", FEDS 2018-075: https://www.federalreserve.gov/econres/feds/files/2018075pap.pdf
- US Census Bureau, Statistics of U.S. Businesses 2022, detailed enterprise sizes (raw file, tabulated here): https://www2.census.gov/programs-surveys/susb/tables/2022/us_state_naics_detailedsizes_2022.txt
- US Census Bureau, County Business Patterns 2022, US file (establishment size classes, tabulated and fitted here): https://www2.census.gov/programs-surveys/cbp/datasets/2022/cbp22us.zip
- SBA Office of Advocacy, FAQ 2026 (45.9% small-business share; seen via search, page returned 403): https://advocacy.sba.gov/2026/02/03/frequently-asked-questions-about-small-business-2026/
- Caliendo, L., Monte, F. & Rossi-Hansberg, E. (2015), "The Anatomy of French Production Hierarchies", *JPE* 123(4); NBER WP 18259: https://www.nber.org/system/files/working_papers/w18259/w18259.pdf
- Caliendo, L., Mion, G., Opromolla, L. & Rossi-Hansberg, E. (2020), "Productivity and Organization in Portuguese Firms", *JPE*: https://www.princeton.edu/~erossi/POPF.pdf
- Rajan, R. & Wulf, J., "The Flattening Firm" (NBER Digest summary; NBER WP 9633): https://www.nber.org/digest/oct03/flattening-corporate-management
- Gallup, "Span of Control: What's the Optimal Team Size for Managers?": https://www.gallup.com/workplace/700718/span-control-optimal-team-size-managers.aspx
- BLS OEWS, May 2025 national table 1: https://www.bls.gov/news.release/ocwage.t01.htm ; OEWS tables index (industry staffing patterns): https://www.bls.gov/oes/tables.htm
- BLS Employee Tenure, January 2026: https://www.bls.gov/news.release/tenure.nr0.htm ; table 1: https://www.bls.gov/news.release/tenure.t01.htm ; Jan 2024 release: https://www.bls.gov/news.release/archives/tenure_09262024.htm
- BLS, Number of Jobs, Labor Market Experience… (NLSY79), Aug 2025: https://www.bls.gov/news.release/nlsoy.nr0.htm
- BLS JOLTS, annual 2025 figures (March 2026 release): https://www.bls.gov/news.release/archives/jolts_03132026.htm
- Fallick, B. & Fleischman, C., "Employer-to-Employer Flows in the U.S. Labor Market": https://www.federalreserve.gov/econres/feds/employer-to-employer-flows-in-the-us-labor-market-the-complete-picture-of-gross-worker-flows.htm
- BLS Employment Situation table A-12 (unemployment duration, Aug 2026): https://www.bls.gov/news.release/empsit.t12.htm
- BLS Employment Projections, civilian labor force participation rates by age: https://www.bls.gov/emp/tables/civilian-labor-force-participation-rate.htm
- BLS Spotlight, "Self-employment in the United States" (2016): https://www.bls.gov/spotlight/2016/self-employment-in-the-united-states/home.htm
- Center for Retirement Research, "What is the average retirement age?": https://crr.bc.edu/what-is-the-average-retirement-age/
- Monthly Labor Review (Dec 1999), "Gender and job promotions" (NLSY79 promotion rates): https://www.bls.gov/opub/mlr/1999/12/art4full.pdf
- Pergamit, M. & Veum, J., "Promotions and job tenure" / "What is a promotion?" (abstract): https://www.bls.gov/osmr/research-papers/1995/ec950170.htm
- Waldman, M. (2008), "Theory and Evidence in Internal Labor Markets" (Handbook of Organizational Economics draft; summarizes Baker–Gibbs–Holmstrom, Murphy 1985): https://ecommons.cornell.edu/bitstreams/aeda4084-a9f8-4217-b0a2-76143f37a5ee/download
- Baker, Gibbs & Holmstrom (1994), "The Internal Economics of the Firm", *QJE* 109(4): https://ideas.repec.org/a/oup/qjecon/v109y1994i4p881-919..html
- White, H. (1970), *Chains of Opportunity*, Harvard UP: https://www.degruyterbrill.com/document/doi/10.4159/harvard.9780674437203/html ; overview: https://en.wikipedia.org/wiki/Vacancy_chain
- Elsby, M., Michaels, R., Gottfries, A. & Ratner, D. (2022), "Vacancy Chains", Philadelphia Fed WP 22-23: https://www.philadelphiafed.org/-/media/frbp/assets/working-papers/2022/wp22-23.pdf
- Kleinbaum, A., Stuart, T. & Tushman, M. (2013), "Discretion Within Constraint", *Organization Science* 24(5): http://faculty.haas.berkeley.edu/tstuart/publications/2013/Discretion%20Within%20Constraint.pdf
- Adamic, L. & Adar, E. (2005), "How to search a social network", *Social Networks* 27: http://snap.stanford.edu/class/cs224w-readings/adamic05search.pdf
- Kossinets, G. & Watts, D. (2006), "Empirical analysis of an evolving social network", *Science* 311: https://pubmed.ncbi.nlm.nih.gov/16400149/
- Yang, L. et al. (2022), "The effects of remote work on collaboration among information workers", *Nat Hum Behav* 6: https://www.nature.com/articles/s41562-021-01196-4 ; research brief: https://ide.mit.edu/wp-content/uploads/2021/11/HOLTZ_RB_11-23-21.pdf ; MSR page: https://www.microsoft.com/en-us/research/publication/the-effects-of-remote-work-on-collaboration-among-information-workers/
- Microsoft Work Trend Index (2025), "Breaking down the infinite workday": https://www.microsoft.com/en-us/worklab/work-trend-index/breaking-down-infinite-workday
- Enron Email Dataset (CMU): https://www.cs.cmu.edu/~enron/
- Not fetched; standard results cited from memory and marked in the text: Bartholomew-style Markov manpower models; the absorbing-chain formula for vacancy-chain length; White's clergy parameter values.
