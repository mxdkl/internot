# Residence: basins, move hazards, distances and destinations

**Date:** 2026-10-01
**For:** Phase 2, residence, design A (closed basins, nested regeneration; AGENTS.md "Phase 2, residence"). **Earlier notes:** `research/2026-10-01-residence-data-and-targets.md` (data, targets), `research/2026-10-01-residence-algorithms.md` (the obstruction), `research/2026-10-01-residence-closed-form.md` (forward by nested regeneration, A chosen).
**How it was gathered:** one pass of computation on public Census files (ACS county-to-county flows 2011–2015 and 2016–2020, ACS 2023 PUMS, the 2020 commuting zones, 2020 tract centres), plus three parallel literature passes (hazard, distances and destinations, new households; WebSearch and WebFetch) whose computed results are credited as "hazard research pass" and "distance research pass". Raw files are in the git-ignored `datasets/`.
**Markings:** [read] full text or the relevant table read; [abs] abstract or snippet only; [comp] computed here from the named public file; [est] estimate. The [comp] scripts are in `scripts/2026-10-01-residence-moves/`.

## 1. Basins: commuting zones

### 1a. The delineations

| Delineation | Zones | Source |
|---|---|---|
| ERS 1980 (Tolbert & Killian) | 765 | `ers_1980-and-1990-...xls` [comp] |
| ERS 1990 (Tolbert & Sizer) | 741 | same file [comp] |
| ERS 2000 | 709 | `ers_2000-commuting-zones.xls` [comp] |
| Penn State 2010 (Fowler, Rhubart & Jensen) | 625 | `counties10.csv`, column OUT10 [comp] |
| Penn State 2020 (Fowler 2024) | 593 (583 without Puerto Rico) | `county20.csv` [comp]; paper [read] |
| **ERS 2020** | **598** (588 without Puerto Rico) | `ers_2020-commuting-zones.csv` [comp] |

- **Method** (Fowler 2024, *Scientific Data* 11:975, PMC11379817) [read]: hierarchical clustering of counties on a proportional commuting-flow matrix (ACS 2016–2020 county-to-county commuting), cutoff 0.977 carried over from earlier decades.
  - 93% of the population lives and works in the same zone; mean county containment 88%, minimum 58%.
  - Six Penn State zones are non-contiguous. **ERS's 2020 edition adds a contiguity constraint**; 572 of its 598 zones are identical to Penn State's [comp].
- **Coverage:** every one of the 3,222 counties and county equivalents (50 states, DC, Puerto Rico) is in exactly one zone, rural counties included. 124 ERS zones cross a state line [comp]. Connecticut uses its 9 planning regions, which all fall in one zone (Hartford); ACS flows to 2020 still use the 8 old counties, which therefore all map to that zone.
- **License:**
  - ERS files are USDA works: public domain in the US (17 USC 105; USDA asks for attribution) [read].
  - The Penn State GitHub repository is GPL-2.0, and the paper is CC BY-NC-ND 4.0 [read]. **Use the ERS 2020 file.**

### 1b. Files (in `datasets/geo/cz/`)

| File | Columns | Use |
|---|---|---|
| `ers_2020-commuting-zones.csv` (also `.xlsx`) | FIPStxt, CountyName, StateName, CZ2020, CZName, CZContainment, CZAvgContainment | **the crosswalk to use**: county → zone, with names ("Los Angeles--Long Beach--Anaheim, CA") |
| `county20.csv` (Penn State; identical copy `county20_github.csv`) | GEOID, CZ20 | the non-contiguous variant |
| `county_fit_statistics.csv`, `cz_fit_statistics.csv` | Share.Work.Core, Contained, Pair.Wage.Corr, ... | quality per county and zone |
| `counties80/90/00/10.csv` | FIPS, ERS80/ERS90/OUT90/REP90/..., Pop, Wage, CBSA10 | earlier delineations |
| `CommutingZones2020_County_GIS_files.zip` | county20.shp/.dbf | county shapes with CZ20 |
| `ers_2000-commuting-zones.xls`, `ers_1980-and-1990-...xls` | | earlier ERS files |

Also downloaded:
- `datasets/geo/flows/acs2016_2020_CtyxCty_US.txt`, `acs2011_2015_CtyxCty_US.txt`, `acs2011_2015_CtyxCty_ager_US.txt` (by age), from `www2.census.gov/programs-surveys/demo/tables/geographic-mobility/{2020,2015}/county-to-county-migration-*/county-to-county-migration-flows/`. Fixed width: county A (current) at 0–6, county B (previous) at 6–12 (3-digit state, 3-digit county), age code at 13–15 in the age file; A's population, nonmovers, movers in the US, same county, other county same state, other state, abroad (each with its margin of error); then B's; the B→A flow is the last pair. This is the latest county-to-county release (2016–2020); none is published for later periods.
- `datasets/geo/puma/2020_Census_Tract_to_2020_PUMA.txt`.
- `datasets/acs/pums2023_1yr/csv_pus.zip`, `csv_hus.zip` (ACS 2023 1-year PUMS; the PUMS API now requires a key).

### 1c. Size distribution (ERS 2020, 2020 population; Puerto Rico included) [comp]

| Statistic | Population |
|---|---|
| Median zone | 157k |
| Mean | 560k |
| 10th / 25th / 75th / 90th / 95th / 99th percentile | 14k / 51k / 459k / 1.32M / 2.51M / 5.59M |
| Smallest | under 1k |

| Largest zones | Population | Counties |
|---|---|---|
| Los Angeles–Long Beach–Anaheim (LA, Orange, Riverside, San Bernardino, Ventura, plus Mohave and La Paz AZ) | 18.9M | 7 |
| New York city (the five boroughs, Nassau, Suffolk; not Westchester, not New Jersey) | 11.7M | 7 |
| Chicago | 9.1M | 11 |
| Houston | 7.3M | 13 |
| San Francisco–Oakland | 5.8M | 8 |
| Newark (northern New Jersey) | 5.7M | 11 |
| Boston | 5.6M | 7 |
| Detroit | 5.5M | 12 |
| Dallas (Tarrant is a separate zone) | 5.3M | 12 |
| Seattle–Tacoma | 5.2M | 10 |

- 81 zones of 1M+ hold 68% of the population; 42 of 2M+ hold 52%; 10 of 5M+ hold 24%; the top two hold 9%.
- Counties per zone: median 5, maximum 19.

### 1d. How many moves stay in a basin

**All ages, ACS 2016–2020 county-to-county flows** (domestic movers, population 1+; annual mover rate 13.2%) [comp]:

| Unit | Units | Same-unit share of domestic movers | Cross-unit moves per person-year |
|---|---|---|---|
| County | 3,142 | 58.2% | 5.5% |
| Metro CBSA + rest of state | 434 | 69.8% | 4.0% |
| CBSA (metro and micro) + rest of state | 967 | 68.4% | 4.2% |
| **Commuting zone (ERS 2020)** | 588 | **70.5%** | **3.9%** |
| CSA, else CBSA, + rest of state | 583 | 72.0% | 3.7% |
| State | 51 | 82.6% | 2.3% |

- Of intercounty moves, 29.5% stay in the zone and 58.4% in the state. Of moves that leave a zone, 43% stay in the state.
- ACS 2011–2015: 61.5% same county, 73.0% same zone, 83.9% same state (mover rate 14.3%).
- Don't compare the annual 70.5% with Chetty, Hendren & Sprung-Keyser's 69% "same zone at 26 as at 16". That is a ten-year stock, which counts non-movers and people who left and came back.

**By age, ACS 2011–2015** [comp] (from `acs2011_2015_CtyxCty_ager_US.txt`; its age-specific flows sum 5–10% short of the county totals, and the all-ages file shows the shortfall is cross-zone, so cross-zone moves are taken as total intercounty minus observed same-zone flows):

| Age | Mover rate | Same county | Same zone | Same state | Cross-zone moves per person-year |
|---|---|---|---|---|---|
| 1–4 | 19.6% | 67.5% | 78.7% | 86.0% | 4.2% |
| 5–17 | 12.8% | 69.5% | 79.3% | 86.9% | 2.7% |
| 18–19 | 27.0% | 39.8% | 50.6% | 77.5% | 13.3% |
| 20–24 | 29.8% | 58.7% | 69.7% | 83.7% | 9.0% |
| 25–29 | 27.3% | 59.5% | 72.4% | 82.9% | 7.6% |
| 30–34 | 20.3% | 61.4% | 74.0% | 83.7% | 5.3% |
| 35–39 | 15.5% | 63.0% | 74.6% | 84.3% | 3.9% |
| 40–44 | 12.4% | 63.7% | 75.2% | 84.8% | 3.1% |
| 45–49 | 10.3% | 63.6% | 75.2% | 85.3% | 2.5% |
| 50–54 | 8.8% | 62.4% | 74.2% | 84.7% | 2.3% |
| 55–59 | 7.7% | 61.1% | 72.6% | 82.9% | 2.1% |
| 60–64 | 6.8% | 58.7% | 70.0% | 80.7% | 2.0% |
| 65–69 | 5.8% | 56.8% | 67.5% | 78.0% | 1.9% |
| 70–74 | 5.4% | 58.3% | 69.2% | 79.6% | 1.7% |
| 75+ | 6.7% | 63.3% | 74.4% | 84.2% | 1.7% |

- The same-zone share is flat at 70–79% from age 25 to 75+. Only 18–24 (college, first jobs, military) and the early retirement ages leave zones more often.
- So the long-move channel is roughly a constant 25–30% of an adult's moves, and the age shape comes mostly from the overall mover rate.
- ACS rates run about 4 points above CPS (12.1% against 7.8% in 2023); keep the level from one survey (§2).

**Historical** (CPS Table A-1, `research/2026-10-01-residence-data-and-targets.md` §2a): the same-county share of domestic movers was 68% in 1948, 64% in 1970, 58% in 2000 and 57% in 2023, so the long-distance share of moves has grown. No zone-level series exists.

### 1e. Alternatives

- **CBSA + rest of state:** containment is about the same (68–70%), but units are uneven: the New York–Newark CBSA is 19.0M and crosses three states; 5.5% of the population is outside any CBSA and 14% outside metro CBSAs [comp].
- **CSA:** contains 72%, but New York's CSA is 20.3M and Los Angeles's 18.5M.
- **Counties:** contain 58%; the largest (Los Angeles County) is 10M, so they don't remove the size problem.
- **States:** contain 83%, but are far too large to enumerate.
- **Commuting zones** are the best of these: closed by construction (93% live and work in them), cover every county, balanced, and public domain.

### 1f. Splitting the largest zones

The closure enumerates a basin's whole history, so its cost grows with the basin. At full scale the Los Angeles zone has about 6.4M current households and New York's about 4.3M [est, 2020 population over persons per household].

**The problem in math:** balanced k-way graph partitioning, minimizing the cut weight of the move graph subject to part sizes ≤ a cap (NP-hard; standard heuristics are multilevel partitioners such as METIS, Karypis & Kumar 1998, and KaHIP). The move graph between tracts isn't published: the finest public flows are county to county, and migration PUMAs are whole counties or groups of counties (IPUMS, 2020 MIGPUMA definitions [read]), so Los Angeles County is one MIGPUMA.

**Splitting by county, observed** (ACS 2016–2020) [comp]:

| Zone | Share of within-zone moves that cross a county line |
|---|---|
| Phoenix | 5.7% |
| Miami | 8.4% |
| **Los Angeles** | **15.6%** (Los Angeles County alone stays 10M) |
| Houston | 17.2% |
| Chicago | 18.6% |
| Seattle | 19.6% |
| Dallas | 21.7% |
| Philadelphia | 22.7% |
| San Francisco | 23.1% |
| **New York** | **25.0%** (the boroughs and Long Island counties) |
| Washington | 27.2% |
| Newark | 28.3% |
| Boston | 28.4% |
| Atlanta | 33.1% |
| All zones | 17.5% |

**Splitting below the county, estimated** [est]:
- **Kernel:** a household moves from tract i to tract j with weight pop_j · (d_ij + 0.5 mi)^−2, using 2020 tract population centres.
  - This kernel reproduces the observed county-line crossings: 30% against 25% in New York (rivers make real crossings rarer), and 14.4% against 15.6% in Los Angeles.
  - Its median move is 2.0–3.4 miles; California's median is 3.7.
  - An exponential kernel with a 2-mile scale fits New York (25%) but not Los Angeles (4%).
  - It also holds in smaller zones (model against observed): Dallas 23.7% / 21.7%, Omaha 25.8% / 20.6%, Joplin MO 19.8% / 18.1%, Rice Lake WI 19.5% / 17.9%. Observed county-line crossings rise with zone size: 9.7% of within-zone moves in zones under 100k, 13–15% at 100k–1M, 17.5% at 1–3M, 20.5% above 3M.
- **Parts:** population-weighted k-means on tract centres. A barrier-aware partitioner would cut fewer moves, perhaps 10–30% fewer [est].

| Part size | Los Angeles zone (18.9M) | Los Angeles County (10M) | New York zone (11.7M) |
|---|---|---|---|
| Counties | 14.4% | (one county) | 30.3% |
| 2.5M | 18.5% | 18.8% | 21.2% |
| 1M | 28.8% | 29.9% | 33.7% |
| 500k | 39.4% | 41.5% | 44.9% |
| 250k | 48.0% | 52.8% | 56.6% |
| PUMA (about 140k) | 61.5% | 64.5% | 69.6% |

Cells: the share of moves within the zone (or county) that would cross a part boundary, under the power-law kernel.

- Splitting makes many local moves cross a basin boundary. Under design A a crossing move must be a static long move, which loses distance decay.
- **Splitting by county is the cheapest cut in the largest zones**: Los Angeles 15.6% and New York 25.0%, observed. It doesn't help where one county dominates: Los Angeles County (10M), Cook (5.3M), Harris (4.7M), Maricopa (4.4M).
- **Inside such a county**, 2.5M parts cut about 19%, and 1M parts about 30% (estimated). Natural barriers lower this: the Santa Monica Mountains separate the San Fernando Valley, and the San Gabriel Mountains the Antelope Valley.
- **So the closure should handle a whole zone**, or at most a county, rather than smaller parts. The alternative is a cheaper closure enumeration, not smaller basins.

## 2. The move hazard

### 2a. Mover rates by age

**CPS ASEC Table 1** (% of persons moving in the past year; rates from the counts) [read]:

| Age | 2000 | 2010 | 2019 | 2023 |
|---|---|---|---|---|
| All 1+ | 16.1 | 12.5 | 9.8 | 7.8 |
| 1–4 | 23.3 | 19.6 | 14.4 | 10.7 |
| 10–14 | 14.0 | 11.3 | 8.7 | 5.9 |
| 18–19 | 22.4 | 14.4 | 10.7 | 8.9 |
| 20–24 | 35.2 | 26.7 | 20.4 | 18.1 |
| 25–29 | 32.4 | 25.9 | 20.8 | 18.9 |
| 30–34 | 22.0 | 18.8 | 14.4 | 11.4 |
| 40–44 | 13.0 | 10.2 | 8.7 | 6.3 |
| 50–54 | 8.6 | 7.5 | 7.0 | 5.2 |
| 60–64 | 6.0 | 5.0 | 4.6 | 4.6 |
| 70–74 | 4.6 | 3.2 | 3.6 | 3.0 (65–74) |
| 80–84 | 4.3 | 2.9 | 3.2 | 2.5 (75+) |

- In 2023, same-county moves are 57% of domestic moves; different-state moves are 18.5%, rising to about 25% at 60–64 and 75+ [read].
- The CPS publishes only age groups.

**ACS 2023 PUMS, single years** (persons in households; tenure is that of the current home, so the destination for movers) [comp]. Annual mover rate, %:

| Age | All | Same state | Other state | Owner-occupied | Renter-occupied | Owner share of persons |
|---|---|---|---|---|---|---|
| 1 | 16.1 | 12.4 | 2.9 | 10.9 | 24.6 | 62% |
| 10 | 9.7 | 7.7 | 1.4 | 6.0 | 17.1 | 67% |
| 17 | 7.5 | 5.9 | 0.9 | 4.7 | 14.8 | 71% |
| 20 | 23.7 | 19.0 | 3.5 | 10.3 | 38.9 | 53% |
| 23 | **27.9** | 20.7 | 6.0 | 15.0 | 39.8 | 48% |
| 26 | 26.0 | 19.7 | 5.1 | 16.7 | 34.3 | 47% |
| 30 | 19.4 | 14.2 | 3.9 | 13.5 | 25.7 | 52% |
| 35 | 14.4 | 10.4 | 2.8 | 10.0 | 21.5 | 62% |
| 40 | 11.3 | 8.4 | 2.1 | 7.7 | 18.5 | 67% |
| 50 | 7.6 | 5.8 | 1.3 | 5.0 | 15.0 | 75% |
| 60 | 6.3 | 4.5 | 1.4 | 4.6 | 13.1 | 79% |
| 70 | 5.1 | 3.6 | 1.2 | 3.7 | 11.9 | 83% |
| 80 | 4.9 | 3.5 | 1.0 | 3.2 | 12.8 | 81% |
| 90 | 6.3 | 4.7 | 1.4 | 3.5 | 13.8 | 72% |

- The full single-year table (ages 1–90) was tabulated twice, by this pass and by the hazard research pass, with matching results.
- Including group quarters, ages 18–19 jump to 23–28% (college dormitories).
- The rise after 80 is real: moves into care and in with kin.
- **ACS runs about 1.55× the CPS** (12.1% against 7.8% in 2023), with a similar age shape (ratio about 0.6 at most ages, 0.45 at 75+). Use the CPS for levels and history, the ACS for shape.

**Continuing couples** (ACS 2023, married for more than a year, so union-formation moves are excluded) [comp]:

| Age | Married > 1 year | All persons | Ratio |
|---|---|---|---|
| 25–29 | 21.7% | 24.8% | 0.88 |
| 30–34 | 15.1 | 17.9 | 0.84 |
| 35–39 | 11.1 | 13.6 | 0.82 |
| 40–44 | 7.9 | 10.5 | 0.75 |
| 45–49 | 6.6 | 8.8 | 0.75 |
| 50–54 | 5.6 | 7.7 | 0.73 |
| 55–59 | 5.1 | 6.9 | 0.74 |
| 60–64 | 4.8 | 6.5 | 0.74 |
| 65–69 | 4.2 | 5.5 | 0.76 |
| 70–74 | 3.8 | 5.1 | 0.75 |

- Married within the past year: 47% moved at 20–24, 34% at 25–29, 16–21% at 40–74.
- Divorced, separated or widowed: 33% at 25–29, 16% at 40–44, 7% at 70–74.
- At 40+, continuing couples move at about 0.75× the all-person rate. The gap is partly union events and partly tenure, since married people own more.

### 2b. Rogers–Castro schedules

**Published US fit** (Rogers & Castro 1981, IIASA RR-81-30, Table 8; mean of 8 schedules between Census regions, 1970–71; normalized to GMR = 1, with a retirement peak) [read]:
- a1 = 0.021, α1 = 0.075;
- a2 = 0.060, μ2 = 20.14, α2 = 0.118, λ2 = 0.569;
- c = 0.002;
- a3 = 0.002, μ3 = 81.80, α3 = 0.430, λ3 = 0.119.

US 1966–71 female GMRs [read]:

| Moves | GMR |
|---|---|
| All | 14.3 |
| Within county | 9.3 |
| Between counties | 5.0 |
| Between states | 2.5 |

All four levels have similar shapes, peaking near 22.

**Fits to ACS 2023 single-year hazards** (λ = −ln(1 − p), households, ages 1–90) [est, from the hazard research pass]:
- Model: m(x) = a1·e^(−α1·x) + a2·exp(−α2(x − μ2) − e^(−λ2(x − μ2))) + c + a4·e^(λ4(x − 90)).
- The last term is an upward slope at the oldest ages.

| Schedule | a1 | α1 | a2 | μ2 | α2 | λ2 | c | a4 | λ4 | Peak | Σλ | RMSE (pp) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| All moves | .138 | .0885 | .402 | 19.33 | .0843 | .608 | .0457 | .0235 | .250 | 22.5 | 10.20 | 0.60 |
| Same state | .107 | .0764 | .271 | 19.03 | .0771 | .736 | .0299 | .0211 | .219 | 22.0 | 7.49 | 0.49 |
| Other state | .0237 | .191 | .0825 | 20.57 | .1105 | .454 | .0111 | .0031 | .50 | 23.7 | 1.81 | 0.17 |
| Owner-occupied | .0859 | .108 | .266 | 21.39 | .0989 | .340 | .0343 | .0037 | .41 | 24.9 | 6.29 | 0.42 |
| Renter-occupied | .177 | .0896 | .475 | 18.49 | .0793 | 1.233 | .1176 | .0384 | .210 | 20.6 | 18.5 | 1.14 |

- Multiply a1, a2, c and a4 by about 0.64 for the 2023 CPS level.
- **Peak age:** cross-nationally it tracks finishing education, labour-force entry, union formation and first birth (Bernard, Bell & Charles-Edwards 2014, PDR 40:213) [abs]. It ranges from about 19 to 31; Europe and North America peak in the late twenties on 5-year data (Bernard & Bell, arXiv:1812.08913) [read].
- **US peak by cohort:** 20 (Silent), 23 (Boomers), 24 (Gen X), 25 (Millennials) (Foster 2017, DemRes 37:47) [read].

### 2c. Era

**CPS Table A-1** (% of persons 1+) [read]:

| Year | Movers | Year | Movers |
|---|---|---|---|
| 1948 | 20.2 | 1990 | 17.9 |
| 1955 | 20.4 | 2000 | 16.1 |
| 1965 | 20.7 | 2005 | 13.9 |
| 1970 | 19.1 | 2010 | 12.5 |
| 1981 | 17.2 | 2015 | 11.6 |
| 1985 | 20.2 | 2019 | 9.8 |
| | | 2023 | 7.8 |

- **Composition of the decline** (Foster 2017, 1982–2015) [read]: ageing explains only 12% (interstate), 18% (intrastate) and 23% (within county). The rest is falling rates within groups, steepest under 35.
- **Measurement:** CPS imputation inflated interstate rates before 2006. It explains 90% of the 2005–06 drop and 42% of the 2000–2010 drop in interstate migration (Kaplan & Schulhofer-Wohl 2012, Demography) [abs].
- **Under 60, 2000 over 2023 is about 1.7–2.2×; at 65+ about 1.5–1.6×** [read, computed from Table 1].
- **Before 1948:** linked censuses show 10-year county-change rates for young men of 50–55% in 1850–80, against 42% in 1971–81 (Ferrie 2005; `research/2026-10-01-residence-data-and-targets.md` §2b). Mobility over 1850–1940 was U-shaped. No annual series exists.
- **Lifetime moves** (sum of −ln(1 − p) over ages 1–89) [est]: 13.5 in 2000, 10.5 in 2010 and 6.7 in 2023 on the CPS; 10.2 on ACS 2023 households.
- **Cohort counts:** the US cohort born 1947–51 made 4.1 moves between 17 and 50; 15% never moved and about 40% moved 5+ times (Bernard et al. 2017, retrospective) [read].

### 2d. Tenure

**Movers by tenure** (CPS Table A-4, % per year, destination tenure) [read]:

| Year | Owners | Renters |
|---|---|---|
| 1988 | 9.5 | 35.2 |
| 2000 | 9.1 | 32.5 |
| 2010 | 5.2 | 28.5 |
| 2019 | 4.9 | 19.7 |
| 2023 | 4.1 | 16.2 |

ACS 2023 gives 6.6 against 21.5 [comp].

- **Within an age group the renter/owner ratio is 2–3**, not the aggregate 4: about 2.0 at 25–30, 2.4 at 40, 3.0 at 50 and 2.8–3.2 at 60–70 [comp]. CPS 2023: 8.7 against 28.6 at 20–29; 2.6 against 9.5 at 45+ [read].

**Homeownership by age of householder** (HVS Table 12, %) [read]:

| Age | 1982 | 2000 | 2019 | 2023 |
|---|---|---|---|---|
| <25 | 19.3 | 21.7 | 23.2 | 23.6 |
| 25–29 | 38.6 | 38.1 | 32.8 | 35.2 |
| 30–34 | 57.1 | 54.6 | 48.0 | 49.4 |
| 35–39 | 67.6 | 65.0 | 57.2 | 59.2 |
| 40–44 | 73.0 | 70.6 | 63.3 | 66.2 |
| 50–54 | 78.8 | 78.5 | 71.9 | 72.1 |
| 60–64 | 80.1 | 80.3 | 76.6 | 76.4 |
| 70–74 | 75.2 | 82.6 | 80.6 | 80.5 |
| 75+ | 71.0 | 77.7 | 77.3 | 78.5 |

- **Decennial homeownership** [read]: 46.5% (1900), 47.8% (1930), 43.6% (1940), 55.0% (1950), 61.9% (1960), 64.4% (1980), 66.2% (2000).
- **Tenure transitions:**
  - Among movers (AHS 2007), 17% went from owning to renting and 13% from renting to owning [read, secondary].
  - First-time buying (JCHS on AHS 2017) [read]: 4.4% of households under 35 per year, 1.6% at 35–54, 0.3% at 55+.
  - Median first-time buyer age: 30–34 (1997–2017).

### 2e. Duration: cumulative inertia or heterogeneity

**The literature:**
- The raw hazard falls with duration of residence (Morrison 1967; Land 1969) [abs].
- **Once heterogeneity and non-stationarity are controlled, duration effects are weak**:
  - Pickles, Davies & Crouchley 1982 [abs];
  - Davies, Crouchley & Pickles 1982, Demography 19:291 [abs];
  - Davies & Crouchley 1986: a mover–stayer fit doesn't need a true dichotomy; a continuous mixture explains it [abs].
- Studies of intentions and of small samples find a hump peaking at 3–5 years, not monotone inertia [abs/read]:
  - Gordon & Molho 1995;
  - Thomas, Stillwell & Gould 2016;
  - Bostanara et al. 2021, a lognormal AFT model peaking at 3.2–3.7 years.
- The McGinnis and Blumen–Kogan–McCarthy originals weren't read.

**Heterogeneity checks** [est, hazard research pass]:
- **Five-year against one-year rates** (CPS 2005–10, against the 2010 one-year rate assumed for everyone):

  | Group | Observed 5-year | Predicted, homogeneous | Gamma variance needed |
  |---|---|---|---|
  | All | 35.4% | 48.7% | 2.5 |
  | Renters | 66.6% | 81.3% | ≈ 1.0 |
  | Owners | 22.2% | 23.4% | ≈ 0.6, weakly identified |
  | Age 25–29 | | | 0.9 |
  | Age 65–69 | | | 4.6 |

  Tenure absorbs most of the heterogeneity.
- **Duration at the current address, ACS 2023 householders:** fitted gamma variances are 0.15 for owners and 0.50 for renters. Without frailty, renters aged 55 would be 14.8% at 10+ years (observed 26.6%, gamma fit 24.9%).
- **Cohort move counts** (mean 4.1; 15% never move): a plain Poisson gives 1.7% never moving. A negative binomial with variance 0.73 gives 15%, with 35% making 5+ moves (observed about 40%).
- **The apparent duration effect from frailty alone**, as the ratio of hazards at durations 1 and 10: 2.2–3.6× for renters, 1.1–1.3× for owners.
- **Not found:** a frailty-controlled US hazard ratio by duration.

**Duration at the current address, ACS 2023 householders** (% in each bin; a calibration target) [comp]:

| Householder | <1 y | 1–2 | 2–5 | 5–10 | 10–20 | 20–30 | 30+ |
|---|---|---|---|---|---|---|---|
| Owners, all ages | 5.7 | 5.3 | 17.4 | 18.1 | 21.0 | 16.1 | 16.4 |
| Renters, all ages | 25.6 | 12.3 | 28.5 | 17.0 | 10.7 | 3.6 | 2.3 |
| Owners 35–44 | 7.8 | 8.1 | 28.3 | 31.3 | 21.2 | 2.2 | 1.1 |
| Renters 35–44 | 22.8 | 11.8 | 32.6 | 21.2 | 10.0 | 1.1 | 0.5 |
| Owners 65–74 | 3.0 | 2.6 | 9.6 | 12.2 | 18.8 | 22.0 | 31.8 |
| Renters 65–74 | 12.4 | 7.2 | 23.2 | 22.0 | 19.5 | 8.7 | 7.1 |

Medians: owners in the 10–20 year bin, renters in 2–5, all households in 5–10.

## 3. Distances of local moves

### 3a. Distributions

| Source | Coverage | Result | Mark |
|---|---|---|---|
| CPS Table A-6 | intercounty moves, distance between county centres, 2005–21 | <50 mi 40–42%, 50–199 21–23%, 200–499 13–15%, 500+ 23–24% | [read] |
| CPS P20-565 | intercounty moves | median 90–103 mi (2006–09), mean 358–419 | [read] |
| CPS A-1 with A-6, 2020–21 | all domestic moves | <50 mi 76.0%, 50–199 9.1%, 200–499 5.2%, 500+ 9.7% | [read, computed] |
| **Census MAF-ARF working paper (2024)** | address-linked administrative records, moves over 500 ft, 2000–21 | **median 6.3–6.8 mi (2000–10), 7.0–8.1 mi (2011–21)**; 10th percentile 0.6–0.8 mi; 90th 460–580 mi; mean 147–167 mi | [read] |
| MIGRATE (Infutor harmonized to ACS, 2010–19) | block group to block group | 37% under 5 mi, 40% 5–50 mi, 23% over 50 mi; the over-50 share rises 22% → 26% over the decade | [read] |
| AHS 2015 recent movers | households | 20% moved 50+ mi | [read] |
| UC Consumer Credit Panel, California 2016–25 | | 3 in 5 stay in their county; more than half move ≤10 mi | [abs] |
| California birth records (existing note) | | median 3.7 mi; leaving the ZIP 6.7 mi; leaving the county 41 mi | |
| Netherlands registers, 2024 | | median 4 km, 75th percentile 17 km | [abs] |
| England and Wales, owner moves | | median 3.2 mi | [abs] |

- Infutor and credit panels under-cover the young, renters and short moves.
- The MAF-ARF counts address changes at about twice the CPS rate.
- **Main reason by distance** (CPS 2009, intercounty) [read]: under 50 mi, housing 40%, family 30%, jobs 19%. At 50+ mi, jobs are 44–54%.

### 3b. Staying in the tract or neighbourhood

| Source | Result | Mark |
|---|---|---|
| ECHO cohort movers | 13.6% of moves stay in the tract (all moves, including 6.2% to another state) | existing note |
| **PSID 1979–2011** (Leibbrand & Crowder 2018, *Annals AAPSS* 680) | **19.36%** of young adults are still in their adolescent tract at their first independent household; **9.24%** ten years later | [read] |
| AHS 2009, Table 2-11 (respondent-defined neighbourhood) | 4.0% of past-year mover householders moved within the "same neighborhood" (owners 3.9%, renters 4.1%) | [read] |
| MAF-ARF | about 10% of moves are under about 0.7 mi, roughly within an urban tract | [est] |

- The AHS 4% is a self-defined neighbourhood and probably under-counts tract-level stays [est].

### 3c. A kernel calibrated on county crossings

The kernel in §1f (destination tract weight pop_j · (d + 0.5 mi)^−2, from 2020 tract centres) reproduces observed county-line crossings within about 5 points in zones from 157k to 18.9M people. It gives a median within-zone move of 2.0–3.4 miles, and 23–31% of moves within 1 mile, in New York and Los Angeles [est].

**Its moves by the highest level changed** (nested hierarchy: county, then parts of about 1M, 250k and 40k (about 10 tracts), then tract; population-weighted k-means within each parent; % of within-zone moves) [est]:

| Highest level changed | Los Angeles (18.9M) | Dallas (5.3M) | Omaha (1.0M) | Joplin MO (266k) | Rice Lake WI (157k) |
|---|---|---|---|---|---|
| County | 14.4 | 23.7 | 25.8 | 19.8 | 19.5 |
| ~1M part | 19.2 | 9.1 | – | – | – |
| ~250k part | 18.3 | 14.6 | 7.9 | – | – |
| ~40k cluster | 20.4 | 20.0 | 24.1 | 3.7 | – |
| Tract within the cluster | 17.8 | 19.3 | 25.8 | 42.2 | 35.0 |
| Same tract | 9.9 | 13.3 | 16.4 | 34.3 | 45.5 |
| *Observed county crossing (ACS)* | *15.6* | *21.7* | *20.6* | *18.1* | *17.9* |

- **In metros, each level of about 4–6× population carries about 15–25% of within-zone moves.**
- A smooth kernel under-produces same-tract moves: it gives 10–16% of within-zone moves in metros, against about 19% implied by ECHO (13.6% of all moves) and 19% from PSID at first independence.
- In small rural zones, tracts are large and the kernel keeps 34–46% in the tract. That part isn't validated by any source found.

## 4. Long moves (between basins)

### 4a. How far [comp]

Cross-zone moves, ACS 2016–2020, distance between county population centres:

| Statistic | Distance (mi) |
|---|---|
| 10th percentile | 40 |
| 25th | 89 |
| **Median** | **240** |
| 75th | 791 |
| 90th | 1,536 |

| Within | Share of cross-zone moves |
|---|---|
| 50 mi | 13.7% |
| 100 mi | 27.7% |
| 250 mi | 50.8% |
| 500 mi | 65.0% |
| 1000 mi | 81.1% |

- IRS 2019–20 county flows give a median of 146 miles and 43% under 100 miles (distance research pass) [comp]. IRS suppresses county pairs under 20 returns, which drops 27% of intercounty migrants, mostly long, small flows; the ACS file has no suppression.

### 4b. Distance decay

**Unit-mass gravity, cross-zone flows over P_i·P_j by distance band** (ACS 2016–2020, 588 zones) [comp]:

| Band (mi) | 25–50 | 50–75 | 75–100 | 100–150 | 150–200 | 200–300 | 300–400 | 400–600 | 600–800 | 800–1000 | 1000–1500 | 1500–2000 | 2000–3000 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Flow / (P_i·P_j) × 10¹² | 1,911 | 2,065 | 1,207 | 797 | 414 | 239 | 137 | 80 | 60 | 57 | 51 | 39 | 41 |

**Log–log slopes** (bands weighted equally; the 0–25 mile band, a few adjacent pairs, is left out):

| Range | Slope |
|---|---|
| Below about 60 mi | about 0 (flat) |
| 50–300 mi | −1.55 |
| 300–1000 mi | −0.93 |
| 1000–3000 mi | −0.31 |
| 25–3000 mi, one exponent | −1.1 |

Decay is steep at regional range and nearly flat across the continent, where big and amenity-rich destinations dominate.

| Study | Data | Result | Mark |
|---|---|---|---|
| Distance research pass | ACS 2019 state flows, doubly constrained Poisson maximum likelihood | power law γ = 1.05 places 81% of flow correctly; exponential 76%; radiation 50%; destination only 68% | [comp] |
| Distance research pass | IRS 2019–20 between 740 zones (1990 CZs) | γ = 1.64 (best fit 1.5–1.75), 60% placed; radiation 55%, and it puts 23% of flow beyond 300 km against 46% observed | [comp] |
| Kluge & Schewe 2021, Phys Rev E 104:054311 | IRS county flows | two-part gravity: distance exponent 0.66 within states, 0.75 between; radiation systematically under-predicts long moves | [read] |
| Simini et al. 2012, Nature | IRS 2007–08 among others | radiation model; contradicted at long range by later work | [read] |
| Poot et al. 2016 (review) | | migration distance decay is "generally less than 2"; US flows about d^−1 (Zipf) | [read] |
| Stillwell et al. 2016, EPA (IMAGE, 60+ countries) | | decay is stable across spatial scales; the US is among the lowest | [read] |

### 4c. Primary, return and onward moves

**Native-born interstate movers, ACS 2023 PUMS** (birth state, state a year ago, current state) [comp]:

| Age | Primary (leaving birth state) | Return (to birth state) | Onward | Foreign-born share of all interstate movers |
|---|---|---|---|---|
| <18 | 42% | 33% | 25% | 7% |
| 18–24 | 49 | 21 | 30 | 8 |
| 25–34 | 29 | 27 | 44 | 17 |
| 35–49 | 26 | 24 | 50 | 25 |
| 50–64 | 27 | 23 | 50 | 17 |
| 65+ | 27 | 21 | 52 | 13 |
| **All** | **35** | **25** | **40** | **14** |

The first three columns are shares of native-born interstate movers.

- 32.6% of the native-born live outside their birth state [comp]; ACS 2019 gives 31.5% (distance research pass) [comp].
- **Of interstate moves by natives living outside their birth state, 38.7% are returns** [comp].
- Return-home movers are 4.1% of all movers (ACS 2023) [comp]. The Census series gives 4.1% (2017), 4.2% (2019), 5.0% (2022), 4.9% (2023), on a definition that includes some returns from abroad [read].
- Kennan & Walker 2011 (Econometrica; NLSY79 white men with high school, ages 18–27; "home" = state at 14) [read]: repeat moves are 49.5% of interstate moves, returns home 28.1%, returns to another earlier state 8.4%.
- About a quarter of inter-metro moves are returns (DaVanzo 1983, via Molloy, Smith & Wozniak 2010) [read].
- Return migration was about 14% of moves in 1955–60 [abs], so the return share has risen [est].
- 75% of people aged 15–50 in 2001 made no interstate move in 2001–16 (Ellis, Fiorio & Foster 2025, CES-WP-25-19) [read].

### 4d. What a static home region costs

**Test:** native interstate movers in ACS 2023 [comp]. The actual joint distribution of (birth state, origin, destination) is compared with destinations drawn from P(destination | key), where the key is:
- the current origin state (origin-keyed);
- the birth state, excluding the origin (birth-keyed);
- the birth census division;
- nothing (the national distribution).

Overlap is Σ min(actual, model) over cells; sampling noise caps it well below 1.

| Destination drawn from | Overlap with actual | Mean distance moved | Share ≤300 mi |
|---|---|---|---|
| Actual | – | 865 mi | 23.9% |
| P(dest given origin state) | 0.56 | 865 | 23.9 |
| **P(dest given birth state)** | **0.62** | **1,007** | **15.1** |
| P(dest given birth division) | 0.52 | 996 | 14.7 |
| National | 0.42 | 1,167 | 7.3 |

**Split by whether the mover still lives in the birth state:**

| Movers | Share of native interstate movers | Overlap, origin-keyed | Overlap, birth-keyed | Share ≤300 mi, actual | Share ≤300 mi, birth-keyed |
|---|---|---|---|---|---|
| Living in birth state | 35% | 0.86 | 0.83 | 27.0% | 20.5% |
| Living elsewhere | 65% | 0.40 | 0.51 | 22.1% | 12.1% |

- **The birth state predicts a long move's destination at least as well as the current state**, because a quarter of moves are returns and migration fields persist by origin. Stuart & Taylor 2021 (AEJ: Applied) find each Black Great Migration migrant from a birth town drew 1.9 more to the same destination county [abs].
- **What is lost is near moves for people living away from home:** the share within 300 miles halves (22% → 12%), and mean distance rises by about 160 miles.
- A key coarser or staler than the birth state loses more. The birth division falls to 0.52, between the birth state (0.62) and the national draw (0.42). A lineage region inherited from a parent or grandparent behaves somewhere in that range [est].
- **At zone level** (IRS 2019–20, in-sample upper bounds), the share of inter-zone flow placed correctly is 36% from nothing, 51% from the origin's division, 68% from the origin's state, and 60% from gravity on the origin (distance research pass) [comp]. A static key loses most where moves are short and between neighbouring zones.

## 5. New households

### 5a. A couple's first joint home

**United States:**
- **NSFH 1992–94** (Compton & Pollak, IZA DP 7431; married couples with both spouses 25+) [read]:
  - Distance to his mother: median 25 mi (25th / 75th / 90th percentiles 5 / 350 / 1,500).
  - Distance to her mother: median 20 mi (4 / 300 / 1,200).
  - Single adults: median 5–8 mi, or 15 excluding those living with the mother.
- **NSFH, couples within 30 miles of each mother** [read]:

  | Within 30 mi of | All couples | Neither spouse has a degree | Both have degrees |
  |---|---|---|---|
  | Neither mother | 29.3% | 18.9% | 49.4% |
  | Her mother only | 16.9 | 17.4 | 15.9 |
  | His mother only | 14.9 | 13.9 | 16.3 |
  | Both mothers | 38.9 | 49.9 | 18.4 |

  Couples live with his mother in 0.9% of cases and with hers in 1.5%.
- **Census 2000, US-born couples** [read]:
  - 59.2% of spouses were born in the same state: 64.1% when neither has a degree, 45.9% when both do.
  - The couple lives in both birth states in 47.5% of cases, hers only 12.6%, his only 12.4%, neither 27.5%.

**Elsewhere:**

| Country, source | Finding | Mark |
|---|---|---|
| UK, Chan & Ermisch 2015 (Demography 52:379) | Within 15 min of the woman's parents 36.7%, the man's 33.9%. 33% of couples live closer to hers, 30% to his. Far from both 46% (74% when both have degrees). | [read] |
| Norway, Løken, Lommerud & Lundberg 2013 (Demography 50:285; registers) | At 34, 25.3% of wives and 31.9% of husbands live in the same postcode as their own parents; 13% in another region. About half of couples have both sets of parents in one municipality; of the rest, 45% live closer to his, 35% to hers. | [read] |
| **Sweden, Brandén & Haandrikman 2019** (Eur J Pop 35:435; 69,861 couples) | **At the start of coresidence, she moves into his home 28%, he into hers 21%, both to a new home 52%.** Median distance moved 6 km (men), 8 km (women). Where partners lived under 50 km apart the median is 4–5 km; over 50 km, 76–114 km. | [read] |
| Germany, pairfam (Krapf et al. 2022) | No significant gender difference; 75% of partners lived under an hour apart | [read] |

- **Synthesis** [est]: in the US and UK the tilt is slightly toward the woman's side (about 53:47 among couples near one side only); in Norway and the Netherlands toward the man's (about 44:56).

### 5b. Leaving the parental home

**Distance moved at first leaving home** (Germany, SOEP 2000–09, Leopold, Geißler & Pink 2012; college and military moves excluded; N = 1,425; km) [read]:

| Education | 10th | 25th | **Median** | 75th | 90th | Share ≥100 km |
|---|---|---|---|---|---|---|
| All | 0.55 | 1.8 | **9.5** | 74 | 250 | |
| Low | 0.38 | 1.2 | 4.4 | 18.8 | 99 | 9.7% |
| Intermediate | 0.39 | 1.4 | 6.1 | 28.8 | 165 | 20.7% |
| High | 1.2 | 4.7 | 28.3 | 133 | 294 | 31.0% |

**US** [read]:
- At first independent household, 19.4% are in their adolescent tract (PSID).
- At 26, 30% are in the same tract as at 16, 58% within 10 miles, 80% within 100 miles, and 69% in the same zone (Chetty, Hendren & Sprung-Keyser).
- Near a parent at 25+: 71.5% without a degree against 54.7% with one (existing note).
- Ages 18–26 (PSID-TA 2005–11): 44% of those living with parents leave within 2 years; 19% of those living independently move back within 2 years (Lei & South 2016).

**Other countries:**
- Netherlands: mean distance to parents is 42.5 km for early leavers against 21.6 km for others (Michielin & Mulder 2007) [read].
- UK: movers under 30 travel a mean of 21 km (low education), 44 km (intermediate) and 65 km (degree) (BHPS, via Chan & Ermisch) [read].

### 5c. After separation

**Who leaves the joint home** (Netherlands registers, divorces 2003–04, N = 18,217; Mulder, ten Hengel, Latten & Das 2012) [read]:

| Group | Man leaves | Woman leaves | Both leave |
|---|---|---|---|
| All | 38.6% | 41.4% | 20.0% |
| With children under 17 | 40.7 | 39.9 | 19.4 |
| Without children | 29.3 | 47.8 | 22.8 |
| Owner-occupied | 28.0 | 47.8 | 24.2 |
| Rented | 56.2 | 30.8 | 13.1 |
| Only his parents nearby | 33.7 | 47.9 | 18.4 |
| Only her parents nearby | 44.6 | 35.3 | 20.1 |

**Elsewhere:**
- Belgium, grey divorce [read]: when all children stay with the mother, she keeps the home in 66% of cases; when a child stays with the father, he keeps it in 67%.
- Both partners leave in 24% of cases in Sweden and 30% in Denmark [read, secondary].
- Italy, among couples where one partner left: the man leaves in 54% of cases [read].
- **No US national split was found.**

**How far the leaver goes:**
- **UK BHPS** (Thomas, Mulder & Cooke 2017) [read]:
  - 66–71% of fathers and 46–52% of mothers move within a year.
  - Mothers who move go a median 2.6–4.8 km.
  - Ex-partners end up about 3.7 km apart at separation (geometric mean; mean 26 km), and the distance grows about 10% a year.
- **Belgium, separated mothers over 3 years** (Schnor & Mikolai 2020) [read]:
  - 32% don't move; 26% move only within their municipality; 21% make one move to another municipality in the province; 12% leave the province.
  - The median for one cross-municipality move is 13 km.
- **US, PSID** [read]:
  - 9% of separated people change state within 2 years of dissolution (Cooke, Mulder & Thomas 2016).
  - Per 1–2-year interval, 8.1% of the separated move 50+ km: 2.0% back to the county where they grew up and 6.1% elsewhere. The comparable figures are 3.75% for the married and 7.5% for the never-married (Spring et al. 2021).
  - A parent within 50 km lowers migration.

### 5d. Distance between partners before living together

| Source | Finding | Mark |
|---|---|---|
| Netherlands (Haandrikman et al. 2008) | median about 6 km between new cohabiters | [abs] |
| Sweden (Haandrikman 2019) | median about 9 km, rising over time | [abs] |
| Sweden (Brandén & Haandrikman 2019) | about 25% lived over 50 km apart | [est] |
| US, Census 2000 | 59% of spouses were born in the same state | [read] |
| Germany, pairfam | 75% lived under 1 hour apart | [read] |

## 6. What this means for the design

All values are [est] unless a section is cited; every number goes into the `us` pack with its source.

**6.1 Basins.**
- **Use the ERS 2020 commuting zones** (`datasets/geo/cz/ers_2020-commuting-zones.csv`): 588 zones for the 50 states and DC, 598 with Puerto Rico. They are public domain, cover every county and hold 70.5% of domestic moves.
- Fix them for 1840–2100; vary attractiveness weights by era.
- **Keep each zone whole as the closed unit** (§1f):
  - Splitting turns local moves into static long moves: about 30% of within-zone moves at 1M parts, about 50% at 250k.
  - If the largest zones must be split, cut along county lines only (Los Angeles 15.6%, New York 25.0%). That still leaves Los Angeles County at 10M.
  - The lever is the closure's cost per household, not smaller basins.
- **Inside a zone, a population hierarchy with branching 4–10:** zone ⊃ county (or about 1M parts in counties over 2M) ⊃ sector (about 250k) ⊃ cluster (about 40k, about 10 tracts) ⊃ 2020 tract ⊃ dwelling. Levels collapse in small zones.
- Distance is then ultrametric in population, which fits intervening-opportunity behaviour.

**6.2 The local-move hazard of a continuing household** (moves not caused by formation, union or separation events, which are separate channels):

λ_h(t) = z_h · E(year, age) · m_τ(age)

- **m_τ(age):** the tenure-specific Rogers–Castro schedule (§2b, owner and renter rows), by householder age, scaled to CPS (× 0.64) and by about 0.75 for event moves (§2a, continuing couples).
  - Target values at the 2023 CPS level, owner / renter per year:

    | Age | Owner | Renter |
    |---|---|---|
    | 30 | 6.5% | 12% |
    | 40 | 3.7 | 9 |
    | 50 | 2.4 | 7 |
    | 70 | 1.8 | 6 |

  - Calibrate the event channels and m_τ together, so that the total by age matches CPS Table 1 and the ACS shape.
- **E(year, age):** relative to 2023:

  | Period | Factor |
  |---|---|
  | 1948–1970 | 2.5 |
  | 1981–1990 | 2.2–2.6 |
  | 2000 | 2.06 |
  | 2010 | 1.60 |
  | 2019 | 1.26 |
  | 2023 | 1.0 |

  - The decline was smaller at 65+ (2000/2023 about 1.5–1.6 there, against 1.7–2.2 under 60), so use about half the factor's excess over 1 at those ages.
  - Before 1948, hold the 1948 level (no annual data; the linked censuses say 1850–80 young men were at least as mobile).
- **z_h:** gamma frailty, mean 1, drawn per household unit, with variance 0.5 for renters (range 0.5–1.0) and 0.3 for owners (0.15–0.6).
  - To hit an annual probability p, set λ0 = ((1 − p)^(−σ²) − 1)/σ².
  - Calibrate against the duration table (§2e), the 5-year/1-year ratios and the cohort move counts (15% never move).
- **No duration term.** The evidence for true duration dependence is weak once heterogeneity is controlled (§2e). Without it, moves stay a Poisson process given z_h, so `PoissonTree::last_before` serves nested regeneration directly; a duration term would force a renewal walk, O(moves).
- **Tenure** is a career of its own: age at first purchase f(id, cohort), from HVS ownership by age and AHS first-time-buying rates. It is independent of the move stream, and a purchase is a forced move at a known time.

**6.3 Levels of a move:**
- **Leaving the zone (a long move):** P(long | move) by householder age from §1d: about 0.49 at 18–19, 0.30 at 20–24, 0.25–0.28 at 25–59, 0.30–0.33 at 60–74, 0.26 at 75+.
- **By era:** about 0.7 × the CPS cross-county share of moves: about 23% in 1948, 26% in 1970, 29% in 2000, 30% in 2023.
- **Counted form** (per birth block and year): cross-zone moves per person-year by age (§1d, ACS 2011–15) are 13.3% at 18–19, 9.0% at 20–24, 7.6% at 25–29, 5.3% at 30–34, 3.1% at 40–44 and about 2% at 50+. Scale to the CPS level and era with E.
- **Within the zone,** by highest level changed:

  | Level | Metro zones | Small zones (under 300k) |
  |---|---|---|
  | County (or 1M part) | 15–25% | about 20% |
  | Sector | 15–20% | – |
  | Cluster | about 20% | – |
  | Tract within the cluster | 15–20% | 35–40% |
  | Same tract (dwelling only) | about 19% | 35–45% |

  - The metro column follows the §3c kernel table, with the same-tract share raised to the observed 19%. Renormalize the other shares.
  - A move at a level picks a child of the kept parent node, weighted by its era attractiveness (population share), as nested regeneration requires.
- **Checks:**
  - 76% of domestic moves under 50 miles (CPS 2021);
  - MAF-ARF median 6–8 miles;
  - 58% / 69% / 30% at 26 (Chetty);
  - county-line crossings (§1f).

**6.4 Destinations of long moves** (static: no dependence on the current origin):
- **A return branch.**
  - For a household whose home region differs from its current zone, about 39% of long moves go home (natives living away, §4c).
  - Overall, returns are about 25% of interstate moves.
  - "Home" is the lineage region at the finest static level available.
- **Otherwise, gravity from the home region:**
  - P(zone j) ∝ A_j(era) · K(d(home, j)), with K piecewise power law and continuous (§4b):

    | Distance | K(d) |
    |---|---|
    | up to 60 mi | 1 |
    | 60–300 mi | (d/60)^−1.55 |
    | 300–1000 mi | then ^−0.93 |
    | beyond 1000 mi | then ^−0.31 |

  - A single-exponent alternative is γ ≈ 1.1 on ACS (1.6 on IRS, which drops small long flows; 1.05 between states).
  - Don't use radiation or exponential decay.
  - Target the cross-zone distance distribution in §4a: median 240 mi, 28% within 100 mi, 35% beyond 500 mi.
- **The measured cost of the static rule** (§4d): with the birth state as key, long moves within 300 miles fall from 24% to 15%, and from 22% to 12% for people living away from home. With a staler lineage key it is worse; the birth division sits halfway to the national draw.
- **The fix is static too:** make lineage regions geographic and fine (state or zone groups), so the key is close to where people were born.

**6.5 Formation channels** (destination relative to the source household's address at the event):
- **Leaving home:**
  - Within the parents' zone, levels relative to the parents' address: same tract 0.20; same cluster, other tract 0.20; same sector, other cluster 0.25; elsewhere in the zone 0.35 (PSID 19.4% same tract; SOEP median 9.5 km, 75th percentile 74 km).
  - Moves out of the zone at these ages come from the long-move channel, whose rate at 18–24 is high (9–13% a year, §1d).
  - Spread by education when the world has it (SOEP medians 4.4 to 28.3 km).
- **Union:**
  - Who moves: she moves into his home with p = 0.28, he into hers 0.21, both to a new home 0.52 (Sweden). The US tilt to the woman's side is slight, so use 0.25 / 0.25 / 0.50.
  - A new home is drawn around the anchor partner's address, chosen by the same split, with local level shares: median 6–8 km, mostly cluster or sector.
  - When the partners are in different zones (about 25% of couples lived over 50 km apart), the non-anchor partner leaves their zone. The union household belongs to the anchor's zone, whose closure finds it through the anchor's union edge (the kin channel of design A).
- **Separation:**
  - Who stays: woman 0.40, man 0.40, neither 0.20.
  - With children, the parent who keeps the children keeps the home in about two thirds of cases.
  - Renters: the woman stays 0.56. Owners: the man stays 0.48.
  - The leaver's new home: same tract or cluster about 50%, elsewhere in the zone about 40%, leaving the zone about 10% (UK median 2.6–4.8 km; Belgium 13 km for cross-municipality moves; US 8% at 50+ km in 1–2 years, a quarter of them back to where they grew up, a kin channel "near parents").

**6.6 Calibration targets to gate on:**
- the age schedule by tenure (§2a);
- the duration table (§2e);
- same-zone shares by age (§1d);
- the cross-zone distance distribution (§4a);
- the primary / return / onward split (§4c);
- the existing proximity targets (59.8% within 30 miles of a parent; 58% within 10 miles at 26).

## 7. Not found

- A frailty-controlled US hazard ratio by duration of residence; the McGinnis, Blumen–Kogan–McCarthy, Goodman and Spilerman originals weren't read.
- US Rogers–Castro fits split into local and long-distance moves (only the 1970–71 interregional fit).
- Tract-to-tract or sub-county flows anywhere public:
  - migration PUMAs are counties or groups of counties;
  - county-subdivision (MCD) flows exist only in the 12 strong-MCD states.
- A US national split of who keeps the home after divorce.
- The NSFH or Add Health distribution of distance at first leaving home.
- Primary, return and onward shares from Long (1988), Eldridge (1965) or Newbold (paywalled); this note computed its own from ACS 2023.
- Pre-1948 annual mobility.
- Homeownership by age before 1982.
- CPS distance bins finer than intercounty (the CPS has none for within-county moves).

## 8. Sources

**Data and computation:**
- **Zones:**
  - ERS commuting zones: https://www.ers.usda.gov/data-products/commuting-zones-and-labor-market-areas
  - Penn State: https://sites.psu.edu/psucz/data/ and https://github.com/csfowler/CommutingZones2020
  - Fowler 2024: https://doi.org/10.1038/s41597-024-03829-5 (PMC11379817) [read]
- **ACS county-to-county flows:** https://www2.census.gov/programs-surveys/demo/tables/geographic-mobility/2020/county-to-county-migration-2016-2020/ and `.../2015/county-to-county-migration-2011-2015/`
- **ACS 2023 PUMS:** https://www2.census.gov/programs-surveys/acs/data/pums/2023/1-Year/
- **Tract to PUMA:** https://www2.census.gov/geo/docs/maps-data/data/rel2020/2020_Census_Tract_to_2020_PUMA.txt
- **IPUMS 2020 migration PUMAs:** https://usa.ipums.org/usa/volii/20migpuma.shtml [read]
- **Scripts (scratch, not committed):**
  - zone flow shares and gravity: `flows.py`, `flows_age.py`, `gravity.py`;
  - split and level kernels: `split.py`, `nested.py`;
  - PUMS: `pums_agg.py`, `interstate.py`.

  They live in the session scratchpad and are reproducible from the files above.

**Census tables:**
- CPS Table 1: https://www2.census.gov/programs-surveys/demo/tables/geographic-mobility/2023/cps-2023/mig_01_2023_1yr.xlsx
- CPS A-1: https://www2.census.gov/programs-surveys/demo/tables/geographic-mobility/time-series/historic/hst_mig_a_1.xlsx
- CPS A-4: `.../hst_mig_a_4.xlsx`
- CPS A-6: `.../hst_mig_a_6.xlsx`
- P20-565: https://www.census.gov/content/dam/Census/library/publications/2011/demo/p20-565.pdf
- HVS: https://www.census.gov/housing/hvs/data/histtab12.xlsx
- Decennial ownership: https://www2.census.gov/programs-surveys/decennial/tables/time-series/coh-owner/owner-tab.txt
- AHS 2009 Table 2-11: https://www2.census.gov/programs-surveys/ahs/2009/tables/2-11.xls
- AHS 2015 recent movers: https://www2.census.gov/programs-surveys/ahs/2015/infographs/2015%20Housing%20Profile%20Recent%20Movers.pdf
- Return-home movers: https://www.census.gov/library/stories/2025/01/return-home-migration.html [read]
- MAF-ARF working paper: https://www.census.gov/content/dam/Census/library/working-papers/2024/econ/mafarf_migration.pdf
- Ellis, Fiorio & Foster: https://www2.census.gov/library/working-papers/2025/adrm/ces/CES-WP-25-19.pdf

**Hazard:**
- Rogers & Castro 1981: https://pure.iiasa.ac.at/1543/1/RR-81-030.pdf
- Foster 2017: https://www.demographic-research.org/volumes/vol37/47/37-47.pdf
- Bernard & Bell: https://arxiv.org/pdf/1812.08913
- Bernard, Bell & Charles-Edwards 2014: https://doi.org/10.1111/j.1728-4457.2014.00671.x
- Kaplan & Schulhofer-Wohl 2012: https://doi.org/10.1007/s13524-012-0110-3
- Davies, Crouchley & Pickles 1982: https://pubmed.ncbi.nlm.nih.gov/7117628/
- Davies & Crouchley 1986: https://doi.org/10.1177/0049124186014004001
- Pickles et al. 1982: https://doi.org/10.1068/a140615
- Bostanara et al. 2021: https://www.osti.gov/pages/servlets/purl/1855766
- Bernard et al. 2017: https://openresearch-repository.anu.edu.au/bitstreams/52c12204-bcd3-4c1c-b9e2-9fce601e25e1/download
- JCHS first-time buyers: https://www.jchs.harvard.edu/sites/default/files/media/imp/harvard_jchs_first_time_homebuyers_rieger_2019.pdf

**Distances and destinations:**
- MIGRATE: https://arxiv.org/html/2503.20989
- Leibbrand & Crowder 2018: https://doi.org/10.1177/0002716218797981 (PMC6910251) [read]
- Kluge & Schewe 2021: https://publications.pik-potsdam.de/pubman/item/item_26331_3/component/file_27467/26331oa.pdf
- Simini et al. 2012: https://pdodds.w3.uvm.edu/research/papers/others/2012/simini2012a.pdf
- Poot et al. 2016: https://www.aecr.org/old/images/ImatgesArticles/2016/12/03_Poot.pdf
- Stillwell et al. 2016: https://eprints.whiterose.ac.uk/id/eprint/96816/3/Submission%20for%20Symplectic.pdf
- Kennan & Walker 2011: https://users.ssc.wisc.edu/~jfkennan/research/ECTA4657.pdf
- Molloy, Smith & Wozniak 2010: https://www.federalreserve.gov/monetarypolicy/files/fomc20101201memo01.pdf
- Stuart & Taylor 2021: https://www.aeaweb.org/articles?id=10.1257%2Fapp.20180294

**New households:**
- Compton & Pollak: https://www.iza.org/publications/dp/7431
- Chan & Ermisch 2015: https://discovery.ucl.ac.uk/id/eprint/1534614/1/Chan_demography3.pdf
- Løken, Lommerud & Lundberg 2013: https://docs.iza.org/dp5685.pdf
- Brandén & Haandrikman 2019: https://pmc.ncbi.nlm.nih.gov/articles/PMC6639436/
- Krapf et al. 2022: https://pmc.ncbi.nlm.nih.gov/articles/PMC8964618/
- Leopold, Geißler & Pink 2012 (SOEPpaper 368): https://www.econstor.eu/bitstream/10419/150913/1/diw_sp0368.pdf
- Lei & South 2016: https://www.demographic-research.org/volumes/vol34/4/34-4.pdf
- Michielin & Mulder 2007: https://www.demographic-research.org/volumes/vol17/22/17-22.pdf
- Mulder et al. 2012: https://pure.uva.nl/ws/files/2475689/161482_476180.pdf
- Fiori 2019: https://www.demographic-research.org/volumes/vol40/20/40-20.pdf
- Zilincikova & Schnor 2021: https://www.demographic-research.org/volumes/vol45/9/45-9.pdf
- Schnor & Mikolai 2020: https://www.demographic-research.org/volumes/vol42/9/42-9.pdf
- Thomas, Mulder & Cooke 2017: https://pmc.ncbi.nlm.nih.gov/articles/PMC6153513/
- Cooke, Mulder & Thomas 2016: https://www.demographic-research.org/volumes/vol34/26/34-26.pdf
- Spring et al. 2021: https://pmc.ncbi.nlm.nih.gov/articles/PMC9552123/
- Haandrikman et al. 2008: https://research.rug.nl/en/publications/geography-matters-patterns-of-spatial-homogamy-in-the-netherlands/
