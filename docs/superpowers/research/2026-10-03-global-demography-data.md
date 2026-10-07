# Global demography data: population, mortality and migration, 1840–2100

**Date:** 2026-10-03. Research for `specs/2026-10-03-global-world.md` §4 (L1 population across countries) and §11 (packs). Three research passes (population before 1950, mortality and model life tables, international migration), checked live on 2026-10-03 unless marked **not verified** (the search budget ran out partway through each pass; later checks used direct fetches and APIs).

**Downloaded (git-ignored, `datasets/global/demography/`):**
- `population/`: `gapminder_pop_v8.xlsx`, `gapminder_population_historic_by_geo_time.csv`, `gapminder_population_by_country_year.csv`, Gapminder fasttrack `children_per_woman_total_fertility`, `lex`, `child_mortality_0_5_year_olds_dying_per_1000_born`; `mpd2023_web.xlsx` (Maddison); `clio_TotalPopulation_Compact.xlsx`; `owid_population.csv`; `federico_tena_world_1800-1938_2026_v01.xlsx` and its 1991-borders version; `frankema_jerven_african_population_v4.0.xlsx`; `WPP2024_Demographic_Indicators_Medium.csv.gz`.
- `mortality/`: `unpd_2011_mlt_130_2.5y_abridged.xlsx` (UN extended model life tables), `unpd_2011_mlt_notes.pdf`, `mortcast_LQcoef.rda` (log-quadratic coefficients).
- `migration/`: `undesa_pd_2024_ims_stock_by_sex_destination_and_origin.xlsx`, `undesa_pd_2024_ims_stock_by_sex_and_destination.xlsx`, `undesa_pd_2020_ims_stock_by_age_sex_and_destination.xlsx`, `dhs_lpr_fy2022_tables.xlsx`, `nber_c5127.pdf`.

Scratch tools in `.scratch/mortality/` (a pure-Python `.rda` reader, a PDF text extractor, the WPP 2024 methodology text, OWID's life-expectancy CSV); the WPP file index is `.scratch/wpp_downloads.json`. No Python xlsx library is installed; reading xlsx as zip/XML with the standard library works.

---

## 1. License summary (what may go into packs)

| Source | License | Use |
|---|---|---|
| UN WPP 2024 | CC BY 3.0 IGO | yes |
| UN International Migrant Stock 2024 (and 2020 by age) | CC BY 3.0 IGO | yes |
| Abel & Cohen bilateral flows (2019, 2022; current versions on IMS2024/WPP2024) | CC BY 4.0 | yes |
| World Bank Global Bilateral Migration Database (1960–2000) | CC BY 4.0 | yes (bulk file via DataBank, not verified) |
| Gapminder (population v8, life expectancy v14/v15, child mortality, TFR) | CC BY 4.0 | yes |
| Federico-Tena World Population Historical Database (1800–1938) | CC BY 4.0 | yes |
| Maddison Project Database 2023 | CC BY 4.0 (cite original papers for graphs or < 12 countries) | yes |
| HYDE 3.3/3.4 | CC BY 4.0 | yes (weak for 1840–1950) |
| Wittgenstein Centre WIC3 V15 (Zenodo 10.5281/zenodo.20447835) | CC BY 4.0 | yes (V14 had no license field) |
| IIASA-WiC POP 2025 (Zenodo 10.5281/zenodo.21738149) | CC BY 4.0 | yes |
| Human Mortality Database | CC BY 4.0 for HMD-constructed data; registration; input data not for commercial use | derived tables yes |
| OWID | CC BY 4.0 | yes |
| UNHCR population (HDX) | CC BY-IGO | yes |
| Clio-Infra | CC BY 3.0 (per OWID's export; not verified at source) | probably |
| MortCast `LQcoef`, `MLTlookup`; demogR | GPL ≥ 2 | numbers are facts; code not copied |
| UN Extended Model Life Tables | un.org Terms of Use (no CC notice) | not verified |
| Ferenczi & Willcox 1929 (NBER) | US public domain since 2025-01-01 | yes |
| DHS Yearbook, Census Working Paper 81 | US public domain | yes |
| Frankema-Jerven African population v4 | none stated | check before use |
| **IPUMS International / NAPP** | non-commercial, no redistribution | **avoid in packs** |
| **CShapes 2.0** | CC BY-NC-SA 4.0 | **avoid** |
| **Azose & Raftery 2019 data** | CC BY-NC-ND 4.0 | **avoid** (Abel–Cohen's `da_pb_closed` carries the same method under CC BY) |
| **Abel 2018 (flows by gender 1960–2015)** | unclear (SAGE TDM only) | **avoid** |
| **SlaveVoyages estimates** | CC BY-NC 3.0 US (source data public domain) | **non-commercial flag** |
| **IHME forecasts** | non-commercial user agreement | **avoid** |
| HLD (lifetable.de) | no CC; "do not pass your copy" | calibration only |
| IUSSP Tools standards | CC BY-NC-SA 3.0 | avoid (rebuild from UN files) |

---

## 2. Population totals

**Most defensible chain:** Federico-Tena (1800–1938, historical polities or 1991 borders, per-year quality grades A–D/E), a 1939–49 patch, then WPP 2024 (1950–2100), then WIC/SSP for scenarios. OWID's open PR (owid/etl#6157) rewires its own series the same way; WID and Maddison have committed to consuming Federico-Tena.

**Federico-Tena World Population Historical Database** (https://www.uc3m.es/investigacion/federico-tena-population, e-cienciaDatos):
- historical polities: doi:10.21950/3SK54X (v5, 2026-01-26), `world_1800-1938_FTWPHD_2026_v01.xlsx`, annual 1800–1938: 154 polities in 1800 (1.003 bn), 156 in 1840 (1.187 bn; Africa 107.9M), 160 in 1900 (1.602 bn), 169 in 1938 (2.252 bn);
- 1991 borders: doi:10.21950/GW7SOZ, 230 countries (Africa 57, Americas 51, Asia 51, Europe 48, Oceania 23);
- quality grades: doi:10.21950/U6AANV; African series from Frankema-Jerven v4; working paper https://hdl.handle.net/10016/45843.

**Gapminder Population v8** (https://www.gapminder.org/data/documentation/gd003/; Excel https://gapm.io/dl_popv8): 196 countries, annual 1800–2100; 1800–1949 from Gapminder's own series (mainly Maddison via Clio-Infra, Frankema & Jerven for Africa, Abad & van Zanden for Latin America; before 1820 the 1820 value carried flat); 1950–2100 WPP 2024. Pre-1950 values are identical across v6–v8 and OWID (1840: USA 17,485,246; Nigeria 13,041,988; China 411,819,451; India 226,508,764). GitHub: `open-numbers/ddf--gapminder--population_historic` (197 geos, 1800–2100; its post-1950 values are an older WPP vintage); `ddf--gapminder--population` holds 1950–2100 only; `ddf--gapminder--fasttrack` mirrors v8 as `pop`.

**Maddison Project Database 2023** (doi:10.34894/INZBF2; `mpd2023_web.xlsx`): 169 entities, benchmark-heavy (countries with population: 1820: 90, 1840: 25, 1850: 65, 1870: 87, 1900: 65, 1913: 87, 1950: 169); most of Africa empty before 1950; keeps Czechoslovakia, Former USSR, Former Yugoslavia, Sudan (Former).

**Clio-Infra Total Population** (hdl:10622/SNETZV): 190 countries (2012 borders), 10-year steps 1800–2000 (non-empty 1840: 41, 1850: 125, 1900: 140, 1950: 190); mirror at https://github.com/CLARIAH/wp4-clioinfra (`www/data/TotalPopulation_Compact.xlsx`); ~80 indicators (sex ratio, life expectancy by sex, infant mortality, height, urbanization, years of education, numeracy, homicide, GDP, real wages).

**HYDE 3.3** (CC BY 4.0; Yoda DOIs 10.24416/UU01-67UHB4, -AEZZIT, -94FNH0; 3.4 at 10.24416/UU01-BV65K3): 10,000 BCE–2023 5′ grids; country totals file `pop_c.txt` (layout for 3.3 not verified); OWID uses it only before 1800.

**OWID population** (https://ourworldindata.org/grapher/population.csv?v=1&csvType=full&useColumnShortNames=true): HYDE before 1800, Gapminder v7 1800–1949, WPP from 1950; current borders; per-country-year sources at `sources-population-dataset.csv`.

**Africa before 1950 is guesswork:** Frankema & Jerven 2014 (EHR 67(S1):907–931) back-project at 0.6–0.65%/yr; their v4 puts Africa at 106.5M in 1850 against Manning's 139.6M (Manning argues 1850 was underestimated by up to 50%); Maddison's sub-Saharan series is round guesses (60M 1820, 65M 1850, 70M 1870, 86M 1900); Gapminder's Africa 1840 sums to ~112M.

**Age and sex before 1950:** no open source covers all countries. HMD has ~17 countries by single age (Sweden 1751, France 1816, Denmark 1835, Iceland 1838, Belgium 1841, England & Wales 1841, Norway 1846, Netherlands 1850, Scotland 1855, Italy 1872, Switzerland 1876, Finland 1878, Spain 1908, Australia and Canada 1921, USA 1933, Portugal 1940). WID.world (technical note Gomez-Carrera et al. 2024/2025) has 0–14/15–64/65+ by sex for 216 countries 1800–2100, from Mitchell for 60 countries, otherwise frozen at 1950 shares. So Internot back-projects from WPP's 1950 structure with long-run life expectancy and fertility (inverse projection), or starts from a stable population per country (as the spec's founders do).

**Reliability by region:** Europe/North America/Oceania censuses from 1750–1870 (errors a few %); Latin American first censuses 1869–1900; China no modern census until 1953 (Maddison flat 412M 1840–50, 358M 1870); India censuses from 1871/1881; Gapminder TFR is constant 1800–1900 in 120 of 197 countries (a placeholder).

**WPP 2024** (base `https://population.un.org/wpp/assets/Excel%20Files/1_Indicator%20(Standard)/CSV_FILES/`; index `assets/downloads.json`; next revision postponed to 11 July 2027; Togo revised 19 Jan 2026):
- `WPP2024_Demographic_Indicators_Medium.csv.gz` (16.6 MB; 84,360 rows, 1950–2101, 237 countries/areas plus aggregates): TPopulation1Jan/1July (by sex), PopDensity, PopSexRatio, MedianAgePop, NatChange, PopGrowthRate, Births, Births1519, CBR, TFR, NRR, MAC, SRB, Deaths (by sex), CDR, LEx/LE15/LE65/LE80 (by sex), InfantDeaths, IMR, Q5, Q0040/Q0060/Q1550/Q1560 (by sex), NetMigrations, CNMR; thousands;
- single age by sex: `WPP2024_Population1JanuaryBySingleAgeSex_Medium_{1950-2023,2024-2100}.csv.gz` (62/68 MB); deaths by single age (45/49 MB); fertility by single age (302 MB); variants (low, high, constant fertility, zero migration, …) for 2024–2100;
- the data API (`/dataportalapi/api/v1/data/…`) needs a bearer token (how to get one not verified).

**Projections beyond WPP:** Wittgenstein WIC3 V15 (200 countries + 28 aggregates; 7 SSP scenarios; 5-year age × sex × education 2020–2100 plus the 1950–2015 reconstruction; `PROJresult_AGE_SSP*_V15.csv` 102–123 MB; drop `agest=-5`, keep `region_level=="country"`; education-specific life tables 10.5281/zenodo.20584088; extension to 2300 10.5281/zenodo.18596542; R package `wcde`).

---

## 3. Mortality

### WPP 2024 life tables
Complete (single age 0–100+, 101 rows) and abridged (0, 1–4, 5–9 … 100+), by sex, 1950–2100, 237 countries/areas: `WPP2024_Life_Table_Complete_Medium_{Both,Female,Male}_{1950-2023,2024-2100}.csv.gz` (~200 MB each), `WPP2024_Life_Table_Abridged_Medium_*` (~145 MB). Columns: `… Time, MidPeriod, SexID, Sex, AgeGrp, AgeGrpStart, AgeGrpSpan, mx, qx, px, lx, dx, Lx, Sx, Tx, ex, ax`.

**How WPP made them** (Methodology Report, PDF pp. 21–25, 55): empirical tables for 120 countries; model-based for 117: 61 Coale-Demeny/UN families matched on 5q0, 5q0+45q15, infant+5q0+45q15 or e0; 33 log-quadratic (DemoTools); 23 HIV-affected countries an HIV-calibrated SVD-Comp. Projections: modified Lee-Carter for 25 countries; for 204, a pattern-of-mortality-decline blended toward Coale-Demeny West (142), North (50) or UN Far Eastern (12) at the projected e0 — so after ~2050 WPP's tables are close to "family × e0". About 7,300 crisis-years (conflicts, disasters, epidemics, famines, COVID-19) are added as excess deaths outside the model tables.

### Before 1950
- **Gapminder life expectancy v14** (196 countries, 1800–2100, both sexes; 186 have values in 1840; by sex only 1950+ in `systema_globalis`): 1800–1950 from Gapminder v7, which assumes a pre-transition baseline of 25–40 years (continent averages from Riley 2005 where no estimate), interpolates linearly to the first real data point, and adds rough crisis "guesstimates" (1918: India 8.81, Nigeria 15.72, Sweden 49.8). Outside Europe, pre-1950 values are modelled, not observed.
- **Gapminder child mortality v11/v12** (196 countries, 1800–2100; e.g. Nigeria 1840 438‰, Sweden 1840 221‰).
- **OWID life expectancy** (HMD + Zijdeman/Clio-Infra before 1950, Riley for regions, WPP from 1950): countries with data 4 in 1840, 35 in 1900, 43 in 1940, 236 in 1950.
- **HMD** (CC BY 4.0 since after Jan 2022; registration; 41 countries; period tables from Sweden 1751 … USA 1933, Japan 1947); STMF weekly deaths for 35 countries.
- **HLD** (15,008 tables, 142 countries; one zip, no login; no CC).

### Model life tables as numbers
- **UN Extended Model Life Tables v1.3:** 9 families (Coale-Demeny West/North/East/South; UN General/Latin/Chilean/South Asian/Far East Asian) × 2 sexes × e0 20.0–100.0 by 2.5 (33 levels) × 28 ages (0, 1, 5 … 130) in the 2.5-year abridged file (16,632 rows, parsed); 1-year and complete versions exist (`undesa_pd_2026_mlt_un2011_130_1y_complete.xlsx`, 33 MB). Originals cover e0 20–75, extended to 100 by a modified Lee-Carter constrained toward HMD; single ages by Heligman-Pollard (1–4), pchip (5–49), Gompertz (50–79), Kannisto (80+).
- **MortCast** `MLTlookup`/`MLT1Ylookup`: the same 9 families, e0 20–115 by 2.5 (UN staff update, Oct 2021).
- **Coale-Demeny** regressions (demogR `cdmlt*`, 25 levels by e10, with Oeppen's 2018 correction).
- **Log-quadratic** (Wilmoth et al. 2012, PMC4046865): `log m_x = a_x + b_x h + c_x h² + v_x k`, `h = log 5q0`, `k` typically in (−2, 2); fitted by sex to 719 HMD tables, ages 0, 1–4, 5–9 … 105–109, 110+. Coefficients parsed from MortCast's `LQcoef` (72 rows: F/M/T × 24 ages × a, b, c, v; the 1–4 row is NaN, derived from 5q0 and 1q0). Check: female e0 29.8, 60.8, 77.9 at 5q0 = 0.40, 0.10, 0.01 (k = 0).
- **Newer:** SVD-Comp (Clark 2019, GPL-3 code), HIV-aware SVD (Sharrow et al. 2014), MDMx (Clark 2026, arXiv 2603.20518, CC BY 4.0).

### Accuracy (Wilmoth et al. 2012; Clark 2019)
e0 RMSE in years, female/male, in sample on 719 HMD tables:

| Input | Model | e0 RMSE | 45q15 RMSE |
|---|---|---|---|
| 5q0 | log-quadratic | 1.62/2.55 | 0.032/0.062 |
| 5q0 | Coale-Demeny West | 2.73/4.09 | 0.042/0.086 |
| 5q0 | UN General | 4.50/5.67 | 0.070/0.104 |
| 5q0 + 45q15 | log-quadratic | 0.70/0.57 | (input) |

Out of sample (log-quadratic, 5q0 only / with 45q15): WHO 1,802 tables 2.63/2.86 and 1.13/1.00; INDEPTH 4.06/3.72 and 2.70/2.02; HLD 2.39/2.78 and 0.90/0.89. SVD-Comp beats log-quadratic by 4–8% in total absolute 5qx error.

### Storage
| Representation | Size |
|---|---|
| log-quadratic coefficients (24 ages × 4 × 2 sexes) | 0.8 KB |
| UN model tables, abridged, e0 step 2.5, mx only | 65 KB |
| UN model tables, complete, e0 step 2.5 | 304 KB |
| per country-year-sex (h, k), 1840–2100 | ~1 MB |
| per country-year-sex e0 only, 1840–2100 | ~0.5 MB |
| WPP single-age mx, 237 × 151 × 101 × 2 | 29 MB |

**For Internot (spec §4, P13):** a per-country-year index into a shared family of tables — (h, k) of the log-quadratic model, or (family, e0) of the UN model tables — keeps memory independent of the number of countries; the monotone world's group scaling (`aft`, AGENTS.md "Cells") then carries group differences. Pre-1950: e0 from Gapminder (or HMD where present), 5q0 from Gapminder child mortality, through the log-quadratic model.

---

## 4. Migration

### Stocks (who lives where, by origin)
- **UN IMS2024** (https://www.un.org/development/desa/pd/content/international-migrant-stock): destination × origin × sex at mid-year 1990, 1995, 2000, 2005, 2010, 2015, 2020, 2024 (Table 1: 28,030 rows; ~233 countries/areas; origin "Others" code 2003; data-type codes B foreign-born, C foreign citizens, R refugees added, I imputed); totals, percentages and growth by destination; region × region matrices. **No age file in 2024**: the 2020 edition has destination × age (0–4 … 75+) × sex, 1990–2020.
- **World Bank GBMD** (Özden et al. 2011, WBER 25(1):12–56): bilateral foreign-born stocks by sex, 1960/70/80/90/2000 (92 → 165 million; 232 countries); DataBank export (bulk file not verified).

### Flows
- **Abel & Cohen 2019** (Sci Data 6:82; figshare collection 10.6084/m9.figshare.c.4470464): `bilat_mig.csv` (https://ndownloader.figshare.com/files/53235671, v8 2025-03-26, rebuilt on IMS2024/WPP2024; 313,736 rows; six periods 1990–95 … 2015–20; columns `year0, orig, dest, sd_drop_neg, sd_rev_neg, mig_rate, da_min_open, da_min_closed, da_pb_closed`); by type of move `bilat_mig_type.csv` (outward/return/transit); `country_list.csv`; `estimation.zip`.
- **Abel & Cohen 2022** (Sci Data 9:173; collection 10.6084/m9.figshare.c.5800838): `bilat_mig_sex.csv` (627,472 rows) and `bilat_mig_sex_type.csv`. The **pseudo-Bayesian closed demographic accounting method (`da_pb_closed`)** correlates best with reported flows (its weight was fitted mostly on intra-European flows, so totals may lean high).
- **Azose & Raftery 2019** (PNAS 116(1):116–122): 1.13–1.29% of the world migrates per 5 years; about 1 in 4 moves is a return. Data CC BY-NC-ND (avoid).

### Net migration, age schedules, projections
- WPP 2024 publishes total net migration and CNMR only (no age). Estimates: often the census residual, with Rogers–Castro age schedules (DemoTools `mig_un_fam()`: family, male labour or female labour). Projections: Azose–Raftery's Bayesian AR(1) on CNMR (`bayesMig`), split into immigration and emigration with Rogers–Castro schedules (Raymer et al. 2023); female shares from Abel & Cohen 2022; explicit Gulf return flows. `procedural_core::curve::rogers_castro_labour` exists, so packs hold only parameters.
- WIC3: gross immigration and emigration by age, sex and education, 2020–2100 (zero- and double-migration scenarios; method Yildiz & Abel 2024).

### Before 1960
- **Ferenczi & Willcox 1929, International Migrations Vol. I** (NBER; US public domain since 2025): national emigration and immigration series 1815/1820–1924, chapters c5125–c5150 (`https://www.nber.org/system/files/chapters/cNNNN/cNNNN.pdf`; c5132 international tables, c5134 United States, c5136 Argentina/Brazil/Paraguay/Uruguay/Chile, c5138 British Isles, c5145 Asia, c5146 Australia). OCR is uncorrected ("CENTUIUES"): tables need careful transcription.
- **Hatton & Williamson 1998, p. 33** (from Ferenczi & Willcox): gross emigration per 1,000 a year by decade:

| Country | 1850–59 | 1860–69 | 1870–79 | 1880–89 | 1890–99 | 1900–13 |
|---|---|---|---|---|---|---|
| Belgium | 1.90 | 2.22 | 2.03 | 2.18 | 1.96 | 2.32 |
| Denmark | – | – | 1.97 | 3.74 | 2.60 | 2.80 |
| France | – | 0.12 | 0.16 | 0.29 | 0.18 | 0.15 |
| Germany | 1.80 | 1.61 | 1.35 | 2.91 | 1.18 | 0.43 |
| Great Britain | 4.83 | 2.47 | 3.87 | 5.71 | 3.92 | 7.08 |
| Ireland | 18.99 | 15.16 | 11.28 | 16.04 | 9.70 | 7.93 |
| Italy | – | – | 4.29 | 6.09 | 8.65 | 17.97 |
| Netherlands | 0.50 | 1.67 | 2.66 | 4.06 | 4.62 | 5.36 |
| Norway | – | – | 4.33 | 10.16 | 4.56 | 7.15 |
| Portugal | – | – | 2.91 | 3.79 | 5.04 | 5.67 |
| Spain | – | – | – | 3.91 | 4.63 | 6.70 |
| Sweden | 0.51 | 2.52 | 2.96 | 8.25 | 5.32 | 2.93 |

- **McKeown 2004** (J. World History 15(2), Table 1), 1840–1940: Europe → Americas 55–58M (+2.5M from India, China, Japan, Africa); India and South China → Southeast Asia, Indian Ocean rim, Australasia 48–52M (+5M); Northeast Asia and Russia → Manchuria, Siberia, Central Asia, Japan 46–51M. No open machine-readable data for Indian indenture or Chinese/Japanese emigration (use Ferenczi & Willcox c5145).
- **Destination series:** DHS Yearbook Table 2 (US LPRs by country of last residence, decades 1820s–2010s + 2020–22, ~70 countries, public domain); Census Working Paper 81 (US foreign-born by country of birth 1850–1930, 1960–2000); Statistics Canada Historical Statistics A350 (arrivals 1852–1977), A385–416 (by last residence 1956–76; Open Licence); ABS HPDC7–9 (license not verified).
- **Forced migration:** UNHCR (HDX, CC BY-IGO, 1951–2025; refugees, asylum seekers, IDPs, stateless by origin and asylum; demographics by sex × 0–4/5–11/12–17/18–59/60+ from 2001). Partition 1947 (12–20M displaced) and German flight and expulsions 1944–50 (~12M; 14.6M by West German estimate) have point estimates only: hand-entered one-off events. SlaveVoyages 1501–1866 by African embarkation and American disembarkation region (estimates CC BY-NC: flag).

### Pack chain for migration
- 1840–1960: Hatton–Williamson rates for European origins, Ferenczi–Willcox national series, DHS decade origin shares, Census WP81 stocks, McKeown regional totals (SlaveVoyages to 1866, flagged);
- 1960–1990: GBMD stocks;
- 1990–2024: IMS2024 stocks plus Abel–Cohen `da_pb_closed` flows by sex and type, fitted to WPP net migration by IPF (`procedural_core::fit::ipf` / `GroupedIpf`);
- 2024–2100: WPP net migration, corridor shares held at 2015–20/IMS2024 and scaled (WIC gross flows for structure);
- age: Rogers–Castro (in core); return: the Abel–Cohen `*_type` files' return share per corridor as a return hazard;
- the observable to calibrate against is the foreign-born share of the living (the `us` pack's lesson).
