# Residence: geography data and mobility targets

**Date:** 2026-10-01
**For:** Phase 2, residence (spec §8; decision note `research/2026-09-30-residence-enumeration-problem.md`).
**How it was gathered:** two research passes (WebFetch and direct downloads; WebSearch was exhausted). Raw files are in the git-ignored `datasets/geo/`.

## 1. Geography data (all public domain unless noted)

US federal works are public domain (17 USC 105). USGS asks for acknowledgement. Paths are under `datasets/geo/`.

### 1a. Population centres, with population

Source: `www2.census.gov/geo/docs/reference/cenpop{2020,2010,2000}/`. Each centre is the population-weighted mean of the block internal points. Paths are under `cenpop/`.

| Level | 2020 file | Rows | Columns |
|---|---|---|---|
| Tract | `CenPop2020_Mean_TR.txt` | 85,395 (614 with no population) | STATEFP, COUNTYFP, TRACTCE, POPULATION, LATITUDE, LONGITUDE |
| Block group | `CenPop2020_Mean_BG.txt` | 242,335 | as tract, plus BLKGRPCE |
| County | `CenPop2020_Mean_CO.txt` | 3,221 | includes county and state names |
| State | `CenPop2020_Mean_ST.txt` | 52 | |

The 2010 and 2000 tract and block-group versions are also in hand.

### 1b. Names

**Gazetteer files** (`gazetteer/`), 2020 and 2026: places (31,909, incorporated places and CDPs), counties, county subdivisions, ZCTAs, tracts, CBSAs, each with internal points.
- Only the 2010 Gazetteer carries population (POP10).
- Populations from 2020 on: `popest/sub-est2025.csv`, incorporated places and minor civil divisions, 2020–2025, no CDPs.
- **Tract-to-place mapping:** no national 2020 file exists. Exact assignment comes from the PL 94-171 block records, which carry PLACE, TRACT and POP100 per block (`pl2020/`; one zip per state, Delaware checked). In Delaware, 131 of 258 populated tracts span more than one place.

**Neighbourhoods:** USGS GNIS populated places (`gnis/`, 190,923 rows, no population). The `census_class_code` from FedCodes marks places inside an incorporated place under another name (U4: Reseda, Bay Ridge; 5,684), plus about 69k uncoded points, many of them city neighbourhoods. GeoNames (CC BY 4.0) mostly derives from GNIS and isn't needed.

### 1c. Metro areas

OMB Bulletin 23-01 (July 2023), `cbsa/list1_2023`: 935 CBSAs (393 metropolitan, 542 micropolitan), built from counties, with central and outlying flags. Connecticut uses its 9 planning regions.

### 1d. Historical populations

| Data | Coverage | Source | Path |
|---|---|---|---|
| Counties | 1800–1990 | Forstall 1996, machine-readable XLS: Part III counts, Part IV FIPS and boundary-change dates | `history/forstall1996/` (CSV conversions included) |
| Counties | 2000–2025 | intercensal and post-censal estimates, plus the CenPop county counts | `popest/` |
| States | 1790–1990 | Forstall Part II | |
| States | 1910–2020 | apportionment counts | `history/states/` |
| 100 largest urban places | each census 1840–1990 | Census WP27 (Gibson 1998), fixed-width text | `history/twps0027/` |
| Urban share by state | 1900–1990 | `urpop0090.txt` | |
| Urban share by state | 2010, 2020 | XLSX | |

Before 1900, the urban share exists only in scanned PDFs.

### 1e. Streets and distances

**Street names.** No published national list was found, so one was computed from TIGER/Line 2025 FEATNAMES: one street per distinct LINEARID with a primary name, road types S1100/S1200/S1400. That gives 8.29M road features and 980,030 distinct names (`tiger2025/street_name_counts_tiger2025.csv`, plus a version with suffixes).
- **Top names:** 2nd, 3rd, 4th, 1st, Main, 5th, Park, 6th, Oak, Maple, 7th, Pine, 8th, Elm, Washington, Cedar.
- **With suffix:** Main St 19,919, 3rd St 17,398.
- ADDRFEAT house-number ranges exist (2.6 GB, not downloaded).

**Distances:**
- haversine on the population centres;
- the NBER county distance database (`distance/`, 100-mile file; derived from Census, no license stated);
- Census county adjacency (`distance/county_adjacency2026.txt`).

**Migration flows:** ACS county-to-county flows 2016–2020 (98 MB, not downloaded) are available for calibration.

## 2. Mobility targets

### 2a. Annual mover rate (CPS ASEC Table A-1, population 1+)

| Year | Movers % | Same county | Other county, same state | Other state | Abroad |
|---|---|---|---|---|---|
| 1948 | 20.2 | 13.6 | 3.3 | 3.1 | 0.3 |
| 1960 | 19.9 | 12.9 | 3.3 | 3.2 | 0.5 |
| 1970 | 19.1 | 11.7 | 3.1 | 3.6 | 0.8 |
| 1981 | 17.2 | 10.4 | 3.4 | 2.8 | 0.6 |
| 1990 | 17.9 | 10.6 | 3.3 | 3.3 | 0.6 |
| 2000 | 16.1 | 9.0 | 3.3 | 3.1 | 0.6 |
| 2010 | 12.5 | 8.6 | 2.1 | 1.4 | 0.3 |
| 2019 | 9.8 | 5.9 | 2.1 | 1.5 | 0.4 |
| 2023 | 7.8 | 4.2 | 1.8 | 1.4 | 0.4 |

- **Owners against renters,** movers per year: 9.5% against 35.2% in 1988; 4.1% against 16.2% in 2023.
- ACS runs 3–4 points above CPS; the two series don't splice.
- **By age, CPS 2023:** 25–29 18.9%; 65+ 2.8% (`research/2026-09-29-time-consistent-evolution.md` §5.1).

### 2b. Before 1948

**Linked censuses, white native men, share moving county or state within 10 years** (Ferrie 2005, NBER w11324, Table 2):

| Age, decade | Moved county | Moved state |
|---|---|---|
| 20–29, 1850–60 | 49.5% | 26.2% |
| 20–29, 1870–80 | 54.7% | 30.1% |
| 20–29, 1971–81 | 41.7% | 21.5% |
| 45–59, 1850–60 | 21.2% | 10.6% |
| 45–59, 1966–76 | 16.0% | 8.1% |

- 19th-century US moves: median 36 miles, mean 213 miles.

**Native population living in its state of birth** (HSUS C 1–14; ACS B05002):

| Year | Share |
|---|---|
| 1880 | 77.9% |
| 1900 | 79.1% |
| 1930 | 76.2% |
| 1960 | 70.3% |
| 1970 | 67.9% |
| 2000 | 67.5% |
| 2024 | 66.3% |

- Living in the region of birth: about 91% in 1880–1910, 79.4% in 1970.
- Mobility over time is U-shaped, falling from 1850 to about 1900 and rising after 1940 (Rosenbloom & Sundstrom, NBER w9857).

### 2c. Distance to parents (the binding locality target)

**PSID 2013, adults 25+, nearest parent** (Choi et al. 2020, *J Marriage Fam* 82(2), PMC7537569; parents include in-laws):

| Nearest parent | Share |
|---|---|
| Living with them | 5.9% |
| Under 30 miles | 59.8% |
| 500+ miles | 9.2% |

- All parents within 30 miles: 41.8%.
- **By education:** coresident or under 30 miles is 71.5% without a degree and 54.7% with one.

**Distance to mother by race, PSID 2013** (PMC7785112):

| Distance | White | Black | Hispanic |
|---|---|---|---|
| Same household | 4.4% | 8.9% | 7.0% |
| Under 30 miles | 51.0% | 61.5% | 42.4% |
| 30–199 miles | 18.5% | 11.0% | 7.8% |
| 200–499 miles | 8.7% | 6.3% | 3.6% |
| 500+ miles | 15.9% | 9.5% | 9.1% |
| Abroad | 1.5% | 2.8% | 30.1% |

**Young adults, age 16 to 26** (Sprung-Keyser, Hendren & Porter 2022, CES WP 22-27):

| Measure | Share |
|---|---|
| Same tract at 26 as at 16 | 30% |
| Moved under 10 miles | 58% |
| Moved under 100 miles | 80% |
| Moved over 500 miles | 10% |
| In the same commuting zone | 69% |

- At 35: 74% within 100 miles of where they were at 16.

**Within 1 mile** (Spring et al. 2017, PSID metro residents):
- 14.9% live within 1 mile of a parent.
- A parent within 1 mile lowers the odds of leaving the tract by 47%.

**Median distance to mother:**
- NSFG, married women: 20 miles; HRS: 18 miles (secondary source).
- PSID 1988: heads still in their childhood state are over half within 10 miles of the mother.

### 2d. Move distances

- **No national distribution including within-county moves was found.** Derived from CPS 2021 (Tables A-1 and A-6): about 76% of domestic moves are under 50 miles, 9% 50–199, 5% 200–499, 10% 500+.
- **Proxies for the median:**
  - California, moves between births: median 3.7 miles, out of the ZIP code 6.7 miles, out of the county 41 miles;
  - moves during pregnancy: medians under 10 km;
  - ECHO movers: 13.6% within the tract, 54.3% to another tract, 25.9% to another county, 6.2% to another state.

### 2e. Years at the current address (ACS 2023)

| Households | Median years at address |
|---|---|
| All | about 7 |
| Owners | about 11 |
| Renters | about 2 |

Owners aged 65+ have been there a median of about 23 years; renters aged 15–34 under 2.5. Census 2000: 5, 9 and 2 years.

### 2f. Moves at life events (ACS 2023 PUMS)

Moved within a year of the event:

| Event | All ages | By age |
|---|---|---|
| Marriage | 28.5% | 46.6% at 18–24; 33.5% at 25–29 against 21.7% for people married longer |
| Divorce | 25.3% | 25.4% at 35–44 against 9.5% for the married |
| Widowhood | 10.6% | |
| A birth | 18.7% | against 16.4% without one |

The move into a shared home often precedes the wedding (cohabitation).

### 2g. Geographic composition for checks

**Regional shares, 1850–2020** (CPH-2-1 Table 20; censuses):

| Year | Northeast | Midwest | South | West |
|---|---|---|---|---|
| 1850 | 37.2 | 23.3 | 38.7 | 0.8 |
| 1900 | 27.6 | 34.6 | 32.2 | 5.7 |
| 1950 | 26.1 | 29.4 | 31.2 | 13.3 |
| 2000 | 19.0 | 22.9 | 35.6 | 22.5 |
| 2020 | 17.4 | 20.8 | 38.1 | 23.7 |

**Urban share:**

| Year | Urban % |
|---|---|
| 1850 | 15.4 |
| 1900 | 39.6 |
| 1950 | 64.0 (new definition) |
| 2000 | 79.0 |
| 2020 | 80.0 |

**Metro share:**

| Year | Metro % |
|---|---|
| 1910 | 28.4 |
| 1950 | 56.1 |
| 2000 | 80.3 |
| 2020 | 86.3 |

## 3. What the targets imply for the design

1. **New households form near the parents' current address.** At 26, 30% are in the same tract as at 16 and 58% within 10 miles. A design whose first homes ignore the parents' location (option B as written) fails this by a wide margin.
2. **Couples form locally.** The ledger's marriage markets decide who pairs with whom, so partner geography needs the ledger to see geography at some level. Today it sees two placeholder regions.
3. **Long-distance moves are a minority but real:**
   - interstate moves are about 1.4–3.6% of people a year, about 15–20% of moves;
   - about a third of adults live outside their birth state;
   - about 10% live more than 500 miles from their parents.
4. **Places grow and shrink:** the West went from 0.8% to 23.7% of the population, and the urban share from 15% to 80%. County history 1840–2020 is in hand.
