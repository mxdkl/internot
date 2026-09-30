# Remarriage after divorce: calibration targets for R1c

**Date:** 2026-09-30
**For:** R1c, divorce re-partnering (`plans/2026-09-30-r1c-divorce-repartnering.md`).
**Method:** primary sources read in full.
- NCHS Series 23 No. 22 was parsed from its PDF.
- The Census P70-125 PDF was downloaded from www2.census.gov and parsed.
- The Pew page was fetched.
- The session's web-search budget ran out before sources on fertility in remarriages could be found. That target is marked **unsourced** below.

## 1. Timing of remarriage after divorce

NSFG 1995, women 15–44 (Bramlett and Mosher 2002, Table 37): probability of remarriage by years since divorce.

| Group | 1 y | 3 y | 5 y | 10 y |
|---|---|---|---|---|
| All | 0.15 | 0.39 | 0.54 | 0.75 |
| Age at divorce < 25 | 0.17 | 0.41 | 0.57 | 0.81 |
| Age at divorce 25+ | 0.14 | 0.37 | 0.51 | 0.68 |
| Non-Hispanic white | 0.17 | 0.42 | 0.58 | 0.79 |
| Non-Hispanic black | 0.08 | 0.23 | 0.32 | 0.49 |

The implied annual hazard is about 15% in years 1–3, about 13% in years 4–5 and about 9% in years 6–10.

SIPP 2009 (Kreider and Ellis 2011, P70-125, Table 8):
- about half of those who remarried after divorcing from a first marriage did so within about 4 years, for men and women in every race and Hispanic-origin group;
- Pew (ACS/SIPP) gives a median of 3.7–3.8 years.

## 2. Ages

SIPP 2009 (P70-125, Table 7), medians among people who had experienced each event by 2009:

| Event | Men | Women |
|---|---|---|
| Age at divorce from first marriage | ~32 | ~30 |
| Age at second marriage | 36 | 33 |
| Age at divorce from second marriage | ~42 | — |

Men are 2–3 years older than women at each first-marriage event.

## 3. How many remarry

- Pew 2014 (ACS 2013):
  - 57% of divorced or widowed adults have remarried: men 64%, women 52%.
  - 50% of the previously married aged 65+ had remarried (34% in 1960).
  - 43% of those aged 25–34 had remarried (75% in 1960). That is a steep decline by era at young ages.
- SIPP 2009 (P70-125):
  - 12% of adults had married twice and 3% three or more times;
  - about 20% or more of adults aged 50–69 had married twice;
  - 21% of men and 22% of women had ever divorced.
- Pew 2014:
  - 8% of newly married adults are on their third or later marriage;
  - almost a quarter of married people are remarried (17% in 1980, 13% in 1960).

## 4. Who remarries whom

- Pew 2014: 40% of new marriages (2013) involve at least one previously married spouse, about 20% of them both spouses.
- Pew 2014: 16% of newly remarried couples have a husband at least 10 years older than the wife, against 4% of first marriages. Age gaps in remarriages are much wider.

## 5. Stability of second marriages

NSFG 1995 (Bramlett and Mosher 2002), probability of disruption by years of marriage:

| Years | 1 | 3 | 5 | 10 | 15 |
|---|---|---|---|---|---|
| Second marriages | 0.05 | 0.15 | 0.23 | 0.39 | — |
| First marriages | 0.03 | 0.12 | 0.20 | 0.33 | 0.43 |

- By age at remarriage: under 25, 0.47 disrupted at 10 years; 25+, 0.34.
- SIPP 2009: the median duration of second marriages that end in divorce is the same as for first marriages, about 8 years.

## 6. Fertility in remarriages (unsourced)

No primary source was read in this session. The model uses a provisional rule instead:
- the union year's parity schedule is shifted down;
- mother-age truncation at 45 does the rest.

Calibration target to source later: the share of births to women in a second union, and the share of remarried women under 40 who have a birth in the new union.

## 7. What the model can and cannot match (R1c scope)

- **Matchable:**
  - the remarriage hazard by duration, sex and age;
  - median years to remarriage;
  - median ages at second marriage;
  - the share of new marriages with previously married spouses;
  - wider remarriage age gaps;
  - higher second-marriage divorce.
- **Not in R1c:**
  - third and later unions (3% of adults; 8% of new marriages);
  - widowed remarriage (founder decision: deferred);
  - same-sex re-partnering.

  Pew's 57% includes the widowed, so the model's share of the ever-divorced who remarry should run somewhat above it for the divorced alone (NSFG: 75% within 10 years for women divorced before 45).

## Sources

- Bramlett MD, Mosher WD. *Cohabitation, Marriage, Divorce, and Remarriage in the United States.* Vital Health Stat 23(22), NCHS, 2002. https://www.cdc.gov/nchs/data/series/sr_23/sr23_022.pdf (Tables 37, 41, and the first-marriage disruption table).
- Kreider RM, Ellis R. *Number, Timing, and Duration of Marriages and Divorces: 2009.* Current Population Reports P70-125, U.S. Census Bureau, 2011. https://www2.census.gov/library/publications/2011/demo/p70-125.pdf (Tables 3, 7, 8).
- Pew Research Center. *Four-in-ten couples are saying "I do," again.* 2014. https://www.pewresearch.org/social-trends/2014/11/14/four-in-ten-couples-are-saying-i-do-again/
