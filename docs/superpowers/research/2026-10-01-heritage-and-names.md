# Heritage and names: calibration data

**Date:** 2026-10-01
**For:** `plans/2026-10-01-n1-heritage-and-names.md`.
**How it was gathered:** three research passes using WebFetch, Europe PMC, NCBI and data portals (WebSearch was exhausted), plus my own analysis of the name files in `datasets/names/`. Numbers below are copied from the reports with their sources. Confidence notes are the reports' own.

**Groups.** Five, following the Census 2020 name files: non-Hispanic (NH) White, NH Black, NH American Indian and Alaska Native (AIAN), NH Asian and Pacific Islander, and Hispanic (any race). Order in code: White, Black, AIAN, Asian, Hispanic.

## 1. Composition of the population

### 1a. 1840–1960, present-day borders (% of total)

Sources:
- Gibson & Jung 2002, Census WP56, Tables 1, 16 and 26, plus Tables C-5 to C-11 and E-1 to E-7. White, Black, Asian and Alaska/Hawaii adjustments.
- Gratton & Gutmann 2000, *Historical Methods* 33(3), Table 2. Hispanic.
- 1890 Census, *Report on Indians Taxed and Not Taxed*, p. 5, plus Thornton and Madley. AIAN before 1890.

| Year | White | Black | AIAN | Asian | Hispanic |
|---|---|---|---|---|---|
| 1840 | 80.65 | 16.33 | 2.56ᵃ | 0.00 | 0.45ᵇ |
| 1850 | 82.38 | 15.42 | 1.70 | 0.00 | 0.50 |
| 1870 | 85.80 | 12.56 | 0.81 | 0.16 | 0.67 |
| 1900 | 87.08 | 11.59 | 0.35 | 0.32 | 0.66 |
| 1930 | 88.23 | 9.65 | 0.29 | 0.45 | 1.38 |
| 1950 | 87.17 | 9.94 | 0.25 | 0.45 | 2.14 |
| 1960 | 85.33 | 10.52 | 0.31 | 0.55 | 3.24 |

- ᵃ About 450k (range 400–550k) within today's borders: the War Department counts of about 300k, plus California's about 150k. This is an interpolation, not a published figure. Confidence low (±25%).
- ᵇ About 80k Mexicans in the later Southwest (Nostrand 1975).
- **The AIAN decline.** The population fell from about 450k to 248k between 1840 and 1890 (disease, war, removal). The model does not reproduce this; see §5 of the plan's debt list.
- **Confidence:** high for White and Black; medium for Hispanic 1850–1960 (reconstructions); low for AIAN before 1890.

### 1b. 1970–2020 (% of total; 2000–2020 race alone among non-Hispanics)

| Year | White | Black | AIAN | Asian | Hispanic | NH two or more |
|---|---|---|---|---|---|---|
| 1970 | 83.47 | 10.88 | 0.41 | 0.76 | 4.46 | — |
| 1980 | 79.57 | 11.52 | 0.63 | 1.57 | 6.45 | — |
| 1990 | 75.64 | 11.75 | 0.72 | 2.80 | 8.99 | — |
| 2000 | 69.13 | 12.06 | 0.74 | 3.72 | 12.55 | 1.64 |
| 2010 | 63.75 | 12.21 | 0.73 | 4.84 | 16.35 | 1.93 |
| 2020 | 57.84 | 12.05 | 0.68 | 6.11 | 18.73 | 4.09 |

- **Renormalized** without NH two-or-more and some-other-race (the model's groups):
  - 2020: 60.6 / 12.6 / 0.7 / 6.4 / 19.6;
  - 2010: 65.1 / 12.5 / 0.7 / 5.0 / 16.7.
- **Two or more races in 2020 is not comparable with 2010.** The Census attributes the rise largely to better coding.
- **Historical classification:**
  - before 1960, enumerators put mixed White-and-other people in the minority race;
  - non-White mixes took the father's race through 1970 and the mother's in 1980–90 (WP56).

## 2. Immigrant arrivals by heritage

Source: DHS Yearbook 2019, Table 2, persons obtaining lawful permanent residence by region of last residence, 1820–2019; Table 3, country of birth, for the 2000s, the 2010s and FY2020–24.

**Mapping to groups:**
- **White:** Europe except Spain, Canada, Australia and New Zealand, and the Middle East and North Africa.
- **Hispanic:** Mexico, Cuba, the Dominican Republic, Central America, Spanish-speaking South America, and Spain.
- **Black:** Haiti, Jamaica, the other English- and French-speaking Caribbean, and Sub-Saharan Africa.
- **Asian:** East, Southeast and South Asia, and Oceania other than Australia and New Zealand.
- **Mixed origins:** split by judgment (e.g. Guyana 50/50 Asian/Black).

| Decade | White | Black | Hispanic | Asian |
|---|---|---|---|---|
| 1840s | 98.7 | 0.4 | 0.9 | 0.0 |
| 1850s | 97.6 | 0.2 | 0.8 | 1.3 |
| 1870s | 94.1 | 0.1 | 0.8 | 4.9 |
| 1900s | 95.7 | 0.8 | 1.5 | 2.1 |
| 1910s | 91.7 | 1.3 | 5.4 | 1.7 |
| 1920s | 82.1 | 1.5 | 14.5 | 1.8 |
| 1950s | 74.8 | 1.5 | 19.5 | 4.2 |
| 1960s | 51.7 | 4.8 | 34.1 | 9.3 |
| 1970s | 28.0 | 8.3 | 33.2 | 30.4 |
| 1980s | 20.1 | 9.5 | 34.1 | 36.3 |
| 1990s | 21.0 | 7.8 | 44.7 | 26.4 |
| 2000s | 24.0 | 10.8 | 35.3 | 30.0 |
| 2010s | 21.0 | 12.5 | 34.4 | 32.0 |

Country-of-birth mapping for FY2020–24: White 19.4, Black 11.3, Hispanic 38.9, Asian 30.4.

**Corrections the model applies** (`params::immigrant_heritage_mix`):
1. **Land arrivals were barely recorded before 1908.** Mexican-born stock grew 14–26k per decade from the 1850s to the 1890s, against 0.7–5k recorded arrivals; then by 118k (1900s) and 265k (1910s). So Hispanic is about 3% in the 1900s and 6–7% in the 1910s.
2. **Puerto Rican migration to the mainland is not in DHS data** (they are US citizens). Puerto Ricans on the mainland numbered about 326k in 1950 and 1.62M in 1970. This adds a large Hispanic flow in the 1940s–60s.
3. **The 2.6M IRCA legalizations of 1989–92** were mostly Mexicans who arrived in the 1970s and 1980s. They are moved back.
4. **Unauthorized arrivals.** The unauthorized population was 3.5M in 1990, 8.6M in 2000, 12.2M in 2007, 10.5M in 2017 and 14.0M in 2023 (Pew), about 70–85% Hispanic.

**Check: the foreign-born stock by race and Hispanic origin** (Gibson & Jung 2006, WP81, Tables 9 and 10):

| Year | White | Black | AIAN | Asian | Hispanic |
|---|---|---|---|---|---|
| 1970 | 73.4 | 2.6 | 0.2 | 5.7 | 18.7 |
| 1980 | 49.4 | 5.8 | 0.3 | 15.5 | 29.6 |
| 1990 | 31.2 | 7.4 | 0.2 | 23.1 | 39.7 |
| 2000 | 22.0 | 6.8 | 0.4 | 22.7 | 45.5 |

## 3. Intermarriage

**Definitions.** Pew counts a person as intermarried if the spouse is in a different group among Hispanic, NH White, Black, Asian, American Indian, multiracial and other.
- Newlywed rates are flows (married in the last 12 months); all-married rates are stocks. In 2015 the flow was 17% and the stock 10%.
- Rates are per person.

**Sources:**
- Pew 2017, Livingston & Brown, *Intermarriage in the U.S. 50 Years After Loving v. Virginia* (P17);
- Pew 2012, Wang, *The Rise of Intermarriage* (P12);
- Pew 2010, *Marrying Out* (P10);
- Census MS-3; Rosenfeld's IPUMS series (Stanford);
- Fryer 2007, *JEP* 21(2);
- Census C2010BR-14, Table 7; CPS ASEC FG3 and UC3;
- Weden, Rendall & Brown 2025, *Demography*;
- Choi & Tienda 2017; Lichter & Qian 2018.

### 3a. Newlyweds intermarried, all groups

| Year | % |
|---|---|
| 1960 | 2.4 (back-projected) |
| 1967 | 3 |
| 1970 | ~4 |
| 1980 | 6.7 |
| 1990 | ~8.3 |
| 2000 | ~11.2 |
| 2008 | 14.6 |
| 2010 | 15.1 |
| 2015 | 17 |

### 3b. Newlyweds by group and sex (men / women)

| Year | White | Black | Hispanic | Asian | AIAN |
|---|---|---|---|---|---|
| 1960 | 1.3 | 1.3 / 0.9 | — | — | — |
| 1980 | 4 / 4 | 8 / 3 | 26 | 26 / 39 | — |
| 2008 | 9.0 / 8.8 | 22.0 / 8.9 | 25.7 | 19.5 / 39.5 | — |
| 2010 | 9.5 / 9.4 | 23.6 / 9.3 | 25.9 / 25.4 | 16.6 / 36.1 | — |
| 2013 (race only) | 7 | 25 / 12 | — | 16 / 37 | 54 / 61 |
| 2015 | 12 / 10 | 24 / 12 | 26 / 28 | 21 / 36 | — |

**Who intermarried newlyweds marry:**
- 2015 pairings: Hispanic–White 42%, White–Asian 15%, White–multiracial 12%, White–Black 11%.
- 2008: of Hispanics who married out, 80.5% married a White spouse and 8.7% a Black spouse.

### 3c. Before 1967

- **Black–White couples** were about 0.06–0.19% of all couples from 1880 to 1960 (Rosenfeld/IPUMS). Black intermarriage was flat from 1880 to 1970 (Fryer).
- **Asian:** about 1% of Asian men were married to whites in 1880. Asians were at 10–15% from 1940 to 1970, with men above women until 1960 (Fryer).
- **Hispanic:** no national rate before 1970. 26% of newlyweds in 1980 (US-born 35%, foreign-born 16%).
- **AIAN:** no census rate found before 2000. Recently 52–58% (Pew 2013; Weden et al.).

### 3d. Cohabitation

- 2010 Census, couples of different race or origin: married 9.5%, unmarried opposite-sex 18.3%.
- 2015: 18% of cohabitors against 17% of newlyweds (P17).
- So recent unions mix about as often as newlyweds; the married stock mixes about half as often.

### 3e. Generation (not modelled; debt)

- **Hispanic newlyweds** married out at 15% if foreign-born and 39% if US-born in 2015; 15% against 44% in 2024.
- **Asian newlyweds:** 24% against 46% in 2015.
- **Married Hispanics with a Hispanic spouse:** 93% of immigrants, 63% of the second generation and 35% of the third.

### 3f. Children of mixed couples (Lichter & Qian 2018, ACS 2008–14, children aged 0–17)

| Couple | Child reported mixed | Minority only | White only |
|---|---|---|---|
| Black–White | 72.5 | 15.7 | 11.8 |
| Asian–White | 74.2 | 8.0 | 17.9 |
| AIAN–White | 48.4 | 23.3 | 28.3 |
| Hispanic–White | 59.7 (Hispanic and White) | 15.2 | 25.1 |

So about 75% of children of Hispanic–White couples are reported Hispanic.
- The father's group matters a little: a Black–White couple's child is reported Black 17.8% of the time when the father is Black, against 10.8% when the mother is.
- **The model's ledger puts every child in the mother's group** (plan §2). That loses about half of the Hispanic-fathered mixed children, against the 75% reported Hispanic. **Debt**, measured in the report.

## 4. Naming practice

### 4a. Surname at marriage (women)

| Era | Took husband's | Kept | Hyphenated / other | Source |
|---|---|---|---|---|
| Married before 1980 | — | 1.5% nonconventional, all kinds | — | Johnson & Scheuble 1995 |
| Early 1990s | 90 | 2 | 5 hyphenated, 3 other | Brightman 1994 |
| 2004 (ACS, native-born married women) | ~94 | 6.4 birth name | 1.3 | Gooding & Kreider 2010 |
| Hawaii 2006 registrations | 83.3 | 11.7 | 5.1 | MacEacheron 2011 |
| 2023, all married women | 79 | 14 | 5 | Pew 2023 |

- **By age (Pew 2023):** women 18–49 kept their name 20% of the time, against 9% at 50+.
- **By group:** Hispanic women kept 30%, White 10%, Black 9%.
- **Brides 45+ (Hawaii):** kept 18% and hyphenated 6%, against about 9% and 4% under 30.
- **Men:** 5% took the wife's name in 2023 (Pew). None did in 107 Ontario divorce files (MacEacheron 2021).
- **Cohabiting partners almost never share a surname:** 1.4% of births to unmarried Victorian parents had a shared surname (Dempsey & Lindsay 2017).
- **Same-sex couples:** no US data.

### 4b. After divorce

**No quantitative US estimate found**, by era or otherwise. There are only interviews (Ceynar & Gregson 2012): "keepers" cite their children, "changers" cite autonomy. Any reversion rate in the model is an unsourced assumption and is labelled as one.

### 4c. Children's surnames

- **Married parents:** about 97% or more get the father's surname. Fewer than 3% get the mother's or a joint one (Johnson & Scheuble 2002). Even when the mother kept her name, about 90% use the father's.
- **Unmarried parents** (Fragile Families, 1998–2000): 78% overall, 86% if cohabiting, 73% if romantically involved, 47% if no longer involved.
- **Common law:** a nonmarital child took the mother's surname (only a qualitative source).
- **Hispanic double surnames** (NC voter file, the research report's analysis, people born 2000–08):
  - 42% of Hispanics have two Hispanic surnames;
  - 12–21% of those born in the 1950s–90s;
  - 20.6% hyphenated.
- **Census name files strip hyphens and spaces**, so "Lopez-Garcia" is listed as LOPEZGARCIA.

### 4d. Middle names

**Share with no middle name** (NC voter file, by birth cohort, women / men):

| Cohort | White | Black | Hispanic | Asian |
|---|---|---|---|---|
| 1920s | 5–6 / 8–9 | 13–15 / 33–37 | — | — |
| 1950s | 4 / 4 | 10–11 / 20–22 | 31 / 36 | 27–31 / 32–36 |
| 1980s | 3 / 3 | 5–6 / 8–10 | 27–29 / 27–28 | 37–41 / 37–41 |
| 2000s | 2 / 1–2 | 5 / 5–6 | 35–36 / 32–34 | 32–34 / 32 |

- **Earlier:** middle names were rare before about 1800. About 3% of Albemarle Parish, Virginia births in 1750–75 had one; 92% of Princeton students in 1840 did (an elite sample). Low confidence.
- **Men:** 70–75% of middle names come from the male first-name pool, and 8–19% are surname-like (family names).
- **Women:** middle names come from a distinct, concentrated pool (Ann, Marie, Lee, Lynn, Jean, Elizabeth). No public-domain frequency source; debt.
- **Maiden name kept as a middle name:** 18% of women who took the husband's name (Scheuble & Johnson 2016), more in the South.

### 4e. First names by group: the Bayesian combination

- **Formula:** `P(name | year, sex, h) ∝ SSA(name | year, sex) · P(h | name, year)`.
- **Standard practice** (Imai & Khanna 2016; Rosenman, Olivella & Imai 2023) is to rake the seed table to two margins with IPF: SSA's per-name counts and the year's births by group. That is the minimum-KL adjustment.
- **P(h | name) is not stable over cohorts** (Fryer & Levitt 2004).
  - Black and White naming diverged sharply in the early 1970s.
  - Per-name Black shares drift: Mary 23% (1940s) → 4% (2000s); Jasmine 82% (1980s) → 42% (2000s).
- **The research report's NC test** used Census 2020 `P(h | name)` raked to each decade's margins, with total-variation distance from the truth:
  - White 0.02–0.06;
  - Black 0.08–0.24;
  - Hispanic 0.16–0.29;
  - ignoring race entirely gives 0.22–0.47 and 0.43–0.63 for Black and Hispanic.

  So the method recovers most of the signal. The rest is cohort drift.
- **My check** (this note's author, on the files in `datasets/names/`), top names for era-shifted Bayes:
  - Hispanic boys 1910: Jose, Oscar, Manuel, Juan;
  - Black girls 1980: Tiffany, Angela, Crystal, Latoya;
  - Asian boys 2020: Muhammad, Kai, Ethan, Ayaan.

  The plain 2020 lift (no year shift) gives Hispanic boys born 1910 mostly John and Frank.
- **Coverage:** SSA lists hold about 96% of births in 1950–70 and about 92–93% in 2000–2020. The missing tail is rare or unique names, and it is disproportionately Black.
- **Census 2020 tail:** "all other names" is 6.2% of people for first names. Within groups it is Black 15%, Asian 24%, AIAN 8%, Hispanic 6%, White 3%.
- **Census-only first names** (in Census 2020, never 5+ SSA births in a year) are 0.3–0.5% of each group but 7.1% of Asian Americans. They are immigrant names: Svitlana, Grazyna (White); Tigist, Olusola, Yvrose (Black); Venkata, Hua, Surinder (Asian); Fiordaliza, Maria del Pilar (Hispanic).

### 4f. Immigrants' first names

- **Biavaschi, Giulietti & Siddique 2017** (New York naturalizations, 1930): 30% had Americanized their first name. By origin: Russian Empire 57%, Germany 26%, Italy 19%, Ireland 4%. Only 7% Americanized their surname.
- **Abramitzky, Boustan & Eriksson 2020:** immigrants give children less foreign names the longer they have been in the US, about 0.4 index points per year.
- **Sue & Telles 2007** (Los Angeles, 1995): Spanish names went to 51% of sons of two immigrant Hispanic parents, against 20% of sons of two US-born Hispanic parents.

### 4g. Surnames by group (Census 2020)

| Group | Top 1 | Top 10 | Top 100 | Top 1,000 | Listed (count ≥ ~90) |
|---|---|---|---|---|---|
| White | 0.9% | 4.4% | 14.7% | 36.6% | 88.6% |
| Black | 2.1% | 12.2% | 32.8% | 61.8% | 90.4% |
| AIAN | 1.0% | 6.3% | 20.7% | 46.5% | 86.8% |
| Asian | 2.7% | 12.9% | 32.8% | 55.8% | 82.4% |
| Hispanic | 1.9% | 14.2% | 36.8% | 63.0% | 85.6% |

## 5. Fertility and mortality by group

**Definition traps:**
- before 1980, "White" includes almost all Hispanics;
- before 1970, "Black" life tables are nonwhite (about 90% Black);
- 1900–32 mortality covers only the death-registration states.

### 5a. TFR, White and Black (Haines, EH.Net, Table 1; Coale & Zelnik 1963; Coale & Rives 1973; NCHS Heuser 1976)

| Year | White | Black | B/W |
|---|---|---|---|
| 1850 | 5.42 | 7.90 | 1.46 |
| 1870 | 4.55 | 7.69 | 1.69 |
| 1880 | 4.24 | 7.26 | 1.71 |
| 1900 | 3.56 | 5.61 | 1.58 |
| 1910 | 3.42 | 4.61 | 1.35 |
| 1920 | 3.17 | 3.64 | 1.15 |
| 1930 | 2.45 | 2.98 | 1.22 |
| 1940 | 2.22 | 2.87 | 1.29 |
| 1950 | 2.98 | 3.93 | 1.32 |
| 1960 | 3.53 | 4.52 | 1.28 |

- From NCHS: 1970 White 2.39 / Black 3.10 (1.30); 1980 1.77 / 2.18 (1.23).
- Completed cohort fertility, nonwhite/White (Heuser), falls from 1.34 for the 1867–71 cohort to 1.11–1.18 for 1890–1924.

### 5b. TFR by race and Hispanic origin, 1980–2024 (NVSR 61-1, 66-1, 75-2; MVSR 32-6 suppl.)

| Year | NH White | NH Black | Hispanic | Asian/API | AIAN | All |
|---|---|---|---|---|---|---|
| 1980 | 1.69 | 2.35 | 2.53 | 1.95 | 2.17 | 1.84 |
| 1990 | 1.85 | 2.55 | 2.96 | 2.00 | 2.18 | 2.08 |
| 2000 | 1.87 | 2.18 | 2.73 | 1.89 | 1.77 | 2.06 |
| 2007 | 1.91 | 2.14 | 2.84 | 1.85 | 1.63 | 2.12 |
| 2010 | 1.79 | 1.97 | 2.35 | 1.69 | 1.40 | 1.93 |
| 2015 | 1.75 | 1.86 | 2.12 | 1.65 | 1.26 | 1.84 |
| 2019 | 1.61 | 1.78 | 1.94 | 1.51 | 1.61 | 1.71 |
| 2024 | 1.53 | 1.50 | 1.93 | 1.26 | 1.38 | 1.60 |

- The AIAN jump in 2016 is a definition change, so treat AIAN as ±25%.
- **CPS 2024, women 45–50** (children ever born, childless): NH White 1.89 and 18.3%; Black 2.17 and 11.0%; Hispanic 2.27 and 9.2%; Asian 1.76 and 12.3%.

### 5c. Life expectancy at birth

**White and Black, NCHS (NVSR 68-7; nonwhite before 1970):**

| Year | White | Black | Gap | Age-adjusted death-rate ratio, B/W |
|---|---|---|---|---|
| 1900 | 47.6 | 33.0 | 14.6 | 1.37 |
| 1910 | 50.3 | 35.6 | 14.7 | 1.30 |
| 1920 | 54.9 | 45.3 | 9.6 | 1.31 |
| 1930 | 61.4 | 48.1 | 13.3 | 1.43 |
| 1940 | 64.2 | 53.1 | 11.1 | 1.30 |
| 1950 | 69.1 | 60.8 | 8.3 | 1.25 |
| 1960 | 70.6 | 63.6 | 7.0 | 1.18 |
| 1970 | 71.7 | 64.1 | 7.6 | 1.27 |
| 1980 | 74.4 | 68.1 | 6.3 | 1.30 |
| 1990 | 76.1 | 69.1 | 7.0 | 1.37 |
| 2000 | 77.3 | 71.8 | 5.5 | 1.32 |
| 2010 | 78.9 | 75.1 | 3.8 | 1.21 |

- **Before 1900:** White 39.5 and enslaved Black 23.0 in 1850 (Haines, Steckel).
- **Infant mortality,** nonwhite/White (Historical Statistics B 142–144): 1915 1.84, 1930 1.66, 1950 1.66, 1960 1.89, 1970 1.74.

**By group, 2023** (age-adjusted death-rate ratio to NH White, men / women; NCHS Data Brief 521):

| Group | Men | Women |
|---|---|---|
| Hispanic | 0.76 | 0.71 |
| NH Black | 1.27 | 1.14 |
| NH Asian | 0.53 | 0.50 |
| NH AIAN | 1.41 | 1.39 |

- **e0 2024** (NVSR 75-5): NH White 78.9, Hispanic 81.8, NH Black 74.8, NH Asian 85.8, NH AIAN 71.1.
- **AIAN before 1940:** e0 about 39 around 1894–1904 (Hacker & Haines 2010). The 1853–1890 population fell 1.2–1.3% a year, which needs crisis mortality, out-classification or count error; the 1890s life table alone would grow.

### 5d. Births outside marriage, % of births (NVSR 48-16, 50-5, 61-1, 75-2)

| Year | All | White | Nonwhite / Black | Hispanic | NH White | AIAN | API |
|---|---|---|---|---|---|---|---|
| 1950 | 4.0 | 1.8 | 18.0 | | | | |
| 1970 | 10.7 | 5.7 | 37.6 | | | | |
| 1990 | 28.0 | 20.4 | 66.5 | 36.7 | 16.9 | | |
| 2010 | 40.8 | 35.9 | 72.5 | 53.4 | 29.0 | 65.6 | 17.0 |
| 2024 | 39.5 | | 68.6 | 54.2 | 26.2 | 68.6 | 11.5 |

The model's non-union births are births with no partner, married or cohabiting, so these ratios guide its relative levels, not its absolute ones.

## 6. Possible better sources (not used; founder decision needed)

State voter files carry first, middle and last names with birth year, sex, race and ethnicity. North Carolina's is free; Florida's is on request. Aggregated by cohort, they would give `P(race | name, cohort)` and middle-name pools. They are public records but individual-level data, so using them goes beyond the 2026-09-30 decision (SSA and Census only).
