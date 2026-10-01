# Households: calibration targets for L3

**Date:** 2026-09-30
**For:** L3 households (`plans/2026-09-30-l3-households.md`), spec §7 and §15.
**Builds on:** `research/2026-09-29-kinship-and-households.md` §6, §8 and §9.6, which has the 2023 to 2025 cross-section. This note adds the history back to 1850, because the world runs from 1840 and the parameters must change by era.

**Method:**
- The Census historical tables were downloaded as `.xls` from `www2.census.gov` and read in full.
- Ruggles (2007) and the Annual Review piece were read through NCBI E-utilities. Only the abstract of Ruggles (2007) was available; the full text is a PDF behind bot protection.
- The Pew page was fetched.
- Web search was not available in this session (its budget was used up), so pre-1940 household sizes are **not verified** here and are marked as such.

## 1. Young adults living in a parent's home (Census AD-1)

CPS counts unmarried college students living in dorms as living in their parents' home. A model without dorms can therefore compare directly.

| Year | Men 18 to 24 | Women 18 to 24 | Men 25 to 34 | Women 25 to 34 |
|---|---|---|---|---|
| 1960 (census) | 52.4% | 34.9% | 10.9% | 7.4% |
| 1970 (census) | 54.3% | 41.3% | 9.5% | 6.6% |
| 1980 (census) | 54.3% | 42.7% | 10.5% | 7.0% |
| 1990 | 58.1% | 47.7% | 15.0% | 8.1% |
| 2000 | 57.1% | 47.1% | 12.9% | 8.3% |
| 2010 | 57.3% | 49.2% | 16.4% | 10.5% |
| 2020 | 60.1% | 56.3% | 22.0% | 13.4% |
| 2025 | 58.8% | 56.4% | 19.2% | 13.6% |

Pew (2016), from census microdata, for all 18 to 34 year olds:
- living with a parent: 35% in 1940, 20% in 1960, 28% in 2007, 32.1% in 2014;
- living with a spouse or partner: 62% in 1960, 31.6% in 2014;
- living with a parent was the most common arrangement in 1880 (no percentage given in the article).

## 2. Household size (Census HH-6 and HH-4)

**Mean household size**

| Year | Persons | Under 18 | 18 and over |
|---|---|---|---|
| 1940 (census) | 3.67 | 1.14 | 2.53 |
| 1950 | 3.37 | 1.06 | 2.31 |
| 1960 | 3.33 | 1.21 | 2.12 |
| 1970 | 3.14 | 1.09 | 2.05 |
| 1980 | 2.76 | 0.79 | 1.97 |
| 1990 | 2.63 | 0.69 | 1.94 |
| 2000 | 2.62 | 0.69 | 1.93 |
| 2010 | 2.59 | 0.64 | 1.95 |
| 2025 | 2.50 | 0.54 | 1.96 |

Before 1940 (not verified here, recalled from Historical Statistics of the United States): about 5.5 in 1850 and 4.6 in 1900. Part of that is boarders, lodgers and servants, which this model doesn't have.

**Households by size, share of households**

| Year | 1 | 2 | 3 | 4 | 5 | 6 | 7+ |
|---|---|---|---|---|---|---|---|
| 1960 | 13.1% | 27.8% | 18.9% | 17.6% | 11.5% | 5.7% | 5.4% |
| 1980 | 22.7% | 31.4% | 17.5% | 15.7% | 7.5% | 3.1% | 2.2% |
| 2000 | 25.5% | 33.1% | 16.4% | 14.6% | 6.7% | 2.3% | 1.4% |
| 2025 | 29.5% | 34.5% | 15.0% | 12.3% | 5.5% | 2.1% | 1.1% |

## 3. Household types (Census HH-1)

Shares of all households.

| Year | Married couple | Other family, male householder | Other family, female householder | Nonfamily |
|---|---|---|---|---|
| 1940 (census) | 76.0% | 4.3% | 9.8% | 9.9% |
| 1960 | 74.3% | 2.3% | 8.4% | 15.0% |
| 1980 | 60.8% | 2.1% | 10.8% | 26.3% |
| 2000 | 52.8% | 3.8% | 12.1% | 31.2% |
| 2025 | 46.6% | 5.5% | 11.7% | 36.2% |

- In 2025, nonfamily households are one-person (29.5%) plus other nonfamily (6.7%) (HH-4 and the 2026-09-29 note).
- "Married couple" excludes cohabiting couples, who fall under other family or nonfamily. The model doesn't separate cohabitation from marriage, so it should compare **couple households** (married plus cohabiting) with married plus about 7.6%: 7.7% of adults lived with an unmarried partner in 2023 (AD-3), which is about 10.0M couples among 131.4M households.

## 4. Living arrangements of adults 18 and over (Census AD-3)

| Year | Alone | With spouse | Child of householder | With partner | With other relatives | With nonrelatives |
|---|---|---|---|---|---|---|
| 1993 | 12.6% | 57.5% | 10.9% | 3.7% | 11.5% | 3.7% |
| 2000 | 13.3% | 56.0% | 10.5% | 3.9% | 11.4% | 4.9% |
| 2010 | 13.7% | 52.7% | 10.6% | 7.1% | 12.1% | 3.8% |
| 2023 | 14.8% | 50.1% | 11.6% | 7.7% | 12.3% | 3.5% |

## 5. Living with non-relatives (roommates)

Source: "Non-family Living Arrangements Among Young Adults in the United States", Eur J Popul 2024 (PMC10917710), from ACS and census microdata.
- In 2019, nearly 10% of people aged 18 to 24 and 7% of people aged 25 to 34 lived with non-relatives only.
- About 71% of people in that situation live in households where nobody is related ("roommate" households). The other 29% are hosted by a family household.
- At 25 to 29, the share living with non-family rose by about 4 points between 1990 and 2019, for both men and women.
- In 2020, 12% of men and 9% of women aged 25 to 34 lived alone.

## 6. Older people living with adult children

- **Ruggles (2007), from IPUMS:** "In the mid-nineteenth century, almost 70 percent of persons age 65 or older resided with their adult children; by the end of the twentieth century, fewer than 15 percent did so."
- Ruggles (2012, Annual Review) repeats it as "almost 70% ... in 1850, compared with just 15% by 1990".
- Ruggles' explanation is that the decline came mainly from more opportunities for the young and less parental control, not from richer elders. For the model, this means the trend belongs in the **children's** leaving and the elders' co-residence together, not in elder wealth.
- Today about 28% of people 65 and over live alone (2026-09-29 note, CPS 2023).

## 7. Custody after divorce

Single fathers head 25% of one-parent families with children under 18 (2026-09-29 note, FM-1). That is the custody share to use for the father after a divorce.

## 8. What this means for the model

1. **Leaving home needs an era schedule.**
   - Women married young in 1960, so their at-home share was low (35%) even though few women lived alone.
   - The model gets that for free: leaving is cut short at the union's start.
   - The independent leaving schedule only needs to match what is left over: never-partnered people, and a modern tail (13 to 22% of 25 to 34 year olds live at home).
2. **No dorms are needed for the CPS targets**, because CPS counts dorm students at home.
3. **Elderly co-residence must fall from about 70% to about 15% across cohorts.** It comes from two sources: children who never left, and elders who move in with an independent child. Ruggles counts both.
4. **Roommates are a small, modern phenomenon:** about 7% of 18 to 24 year olds and 5% of 25 to 34 year olds in roommate households in 2019 (71% of the 10% and 7% living with non-relatives only).
5. **Boarders, lodgers and servants are out of scope.** Nineteenth-century mean sizes will therefore run below the (unverified) 5.5. The report should say so rather than tune around it.
6. **Compare couple households, not married couples.**

## Sources

- U.S. Census Bureau, Current Population Survey, Annual Social and Economic Supplements, historical tables AD-1, AD-3, HH-1, HH-4 and HH-6 (release of December 2025), from https://www2.census.gov/programs-surveys/demo/tables/families/time-series/
- Ruggles S. The Decline of Intergenerational Coresidence in the United States, 1850 to 2000. American Sociological Review 72(6), 2007. PMC3090139.
- Ruggles S. The Future of Historical Family Demography. Annual Review of Sociology 38, 2012. PMC3740453.
- Non-family Living Arrangements Among Young Adults in the United States. European Journal of Population, 2024. PMC10917710.
- Pew Research Center. For First Time in Modern Era, Living With Parents Edges Out Other Living Arrangements for 18- to 34-Year-Olds. 2016. https://www.pewresearch.org/social-trends/2016/05/24/for-first-time-in-modern-era-living-with-parents-edges-out-other-living-arrangements-for-18-to-34-year-olds/
