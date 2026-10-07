# Education: data sources and realism targets

**Date:** 2026-10-01
**For:** the education layer (spec `specs/2026-09-29-society-as-a-function.md` §9): schools by catchment of residence, grades by age, classes, education level by intergenerational transmission, colleges.
**How it was gathered:**
- five parallel research passes (WebSearch, WebFetch, direct downloads) and one data pass;
- NCES and Census tables were downloaded as spreadsheets and parsed;
- scanned census reports were rendered and read by eye;
- the microdata were computed directly: CCD, EDGE, PSS, IPEDS, SABS, ACS 2023 PUMS, CPS ASEC 2023, GSS.

Raw files are in the git-ignored `datasets/education/`; scripts for every [computed] number are in `research/scripts/2026-10-01-education/`.

**Marks:**
- **[read]:** the table or page itself was read;
- **[abs]:** an abstract, search summary or secondary source;
- **[computed]:** worked out here from public microdata.

"Not found" and "not verified" mean what they say.

**Licences:**
- US federal works (NCES, Census, BLS, FHWA) are public domain (17 USC 105). That covers every file in `datasets/education/` except those listed below.
- **GSS:** NORC's terms (free download, citation requested).
- **Schwartz & Mare replication cells:** no licence stated.
- **Papers** (NBER, journals) and the TICAS, Pew and AACC briefs are copyrighted. They are cited, not redistributed.
- **Synthetic-population code** is listed with its licence in §8.

---

## 0. What the data say for the design

1. **Enrollment is near universal at 7–15 from about 1930.** The era signal is at the edges:
   - **at 5–6:** public kindergarten reached about 16 per 100 five-year-olds in 1910, and 82% of 5-year-olds were in kindergarten in 1980;
   - **at 14–19:** the high-school movement took 14–17 from 59% in 1910 to 94% in 1970;
   - **at 18–24:** college.
   Before 1880, Black children were almost all out of school.
2. **Attainment must be drawn by cohort, sex and heritage.** In cohorts born before 1950, men led in BA and women in high school; women have led in BA since the cohorts born about 1955–60.
   - Immigrants need their own, bimodal draw: 22% without high school against 6.5% of natives, and more graduate degrees.
3. **Transmission fits a latent normal well.** The parent–child latent correlation is about 0.55 in every cohort from 1883 to 1994, so educational expansion is just moving thresholds.
   - Siblings correlate about 0.6, about twice what parents' education alone gives. The model needs a family term.
4. **Education has to enter partnering.**
   - The spouse correlation has been about 0.56–0.64 in years (0.65–0.73 latent) since 1940.
   - Pairing by age, state and heritage, which the ledger does now, gives a spouse correlation of only 0.13–0.17 (latent) or 0.14 (years) [computed].
5. **Attendance zones are close to nearest-school partitions,** at least for segregation (Monarrez 2023).
   - Measured zones (SABS) hold a median of about 6,900 people (elementary), 21,000 (middle) and 29,000 (high).
   - **Tracts are too coarse for elementary zones in dense places:** about half of people live in tracts whose block groups straddle two or more elementary zones.
   - About 60% of districts have one school per grade, so their zone is the district, but they hold only 10–13% of people.
6. **Schools:** about 95k public and 30k private.
   - A typical elementary grade is 60–90 children, about 3–4 classes of 21. A high-school grade is about 350 students.
   - Rural schools are 30% of schools but only 20% of students.
   - About 81% of public-school students attend their assigned school (60% of all K–12 students in cities).
7. **College is local.**
   - Median distance from home is 17 miles (10 for community college).
   - 78% enroll in their home state.
   - Only 25% of 18–24 undergraduates live in group quarters, 47% at age 18. About 45% live with their parents.

---

## 1. Enrollment by age and era

### 1a. Sources

| Source | Coverage | Path (`datasets/education/enrollment/`) |
|---|---|---|
| NCES, *120 Years of American Education* (1993), Table 2: enrolled per 100 aged 5–19, by sex and race | 1850–1991 | `nces_120years_93442.pdf` |
| Statistical Abstract 1943, No. 216: attending school by single year of age 5–20, by sex; No. 217: ages 5–24 by colour and sex, 1940 | censuses 1910–1940 | `statab1943_education.pdf` |
| 1910 Census, *Abstract: School Attendance and Illiteracy*, Table 19: ages 5–9, 10–14, 15–20, by race and sex | 1900, 1910 | `census1910_abstract_school_attendance.pdf` (scanned) |
| Digest 103.20: enrolled by age group (3–4 … 30–34); 18–19 split into secondary and college from 1963 | 1940, 1945, 1947–2022 | `tabn103.20_d23.xlsx` |
| Digest 103.10: by sex × race (Black, Hispanic, White) | 1980–2022 | `tabn103.10_d23.xlsx` |
| Digest 103.30 (ACS): by age, with AIAN, Asian, Pacific Islander | 2022 | `tabn103.30_d23.xlsx` |
| Census CPS historical Table A-2: enrolled by age (13 groups), sex, race, Hispanic origin | 1947–2024 | `cps_hst_enroll02.xlsx` |
| CPS Table A-5b: 18–19-year-olds by status | 1967–2024 | `cps_hst_enroll05b.xlsx` |
| Digest 202.10 (2019 ed.): 3-, 4-, 5-year-olds in preschool and kindergarten, full- or part-day | 1970–2018 | `tabn202.10_d19.xls` |
| Digest 202.20 (ACS): 3–5-year-olds by family characteristics | 2013–2023 | `tabn202.20_d24.xlsx` |
| ACS 2023 1-year PUMS | 2023, single years, five heritage groups | `datasets/acs/pums2023_1yr/` [computed] |

**The question changed, so the series don't splice cleanly** [read, table notes]:

| Era | What counts as enrolled |
|---|---|
| 1850–1900 | attended at any time in the year before the census. 1900 asked months attended, and may undercount. |
| 1910–1930 | attended at any time since 1 September. The census date was 15 April in 1910 and 1 January in 1920. |
| 1940 | enrolled in a "regular" school between 1 March and 1 April |
| CPS, 1947 on | enrolled in October |

- In the CPS, kindergarten and nursery school are included from 1964 (Table A-2 excludes kindergarten for 1947–63; Digest 103.20 includes it). The 1994 preprimary method changed.
- The CPS covers the civilian non-institutional population only.
- The ACS counts attendance in the last three months, and reads 1–3 points above the CPS at 18–24.

### 1b. 5- to 19-year-olds enrolled, 1850–1950 (120 Years Table 2; HSUS H 433–441) [read]

| Year | All | White | Black and other | Male | Female |
|---|---|---|---|---|---|
| 1850 | 47.2 | 56.2 | 1.8 | 49.6 | 44.8 |
| 1860 | 50.6 | 59.6 | 1.9 | 52.6 | 48.5 |
| 1870 | 48.4 | 54.4 | 9.9 | 49.8 | 46.9 |
| 1880 | 57.8 | 62.0 | 33.8 | 59.2 | 56.5 |
| 1890 | 54.3 | 57.9 | 32.9 | 54.7 | 53.8 |
| 1900 | 50.5 | 53.6 | 31.1 | 50.1 | 50.9 |
| 1910 | 59.2 | 61.3 | 44.8 | 59.1 | 59.4 |
| 1920 | 64.3 | 65.7 | 53.5 | 64.1 | 64.5 |
| 1930 | 69.9 | 71.2 | 60.3 | 70.2 | 69.7 |
| 1940 | 74.8 | 75.6 | 68.4 | 74.9 | 74.7 |
| 1950 | 78.7 | 79.3 | 74.8 | 79.1 | 78.4 |

- 1900–1930 are ages 5–20; 1930 counts Mexicans as white.
- 1850–60 "Black and other" includes the enslaved, almost none of whom attended.
- Boys led by 4–5 points in 1850–60; girls were slightly ahead from 1900.

**By age group, 1900 and 1910** (1910 Census, Table 19; Negro shares computed from counts) [read]:

| Age | 1900 all | 1910 all | 1900 white | 1910 white | 1900 Negro | 1910 Negro |
|---|---|---|---|---|---|---|
| 5–9 | 48.1 | 61.7 | 52.0 | 64.8 | 23.7 | 41.2 |
| 10–14 | 79.8 | 88.2 | 84.0 | 91.1 | 53.8 | 68.6 |
| 15–20 | 26.9 | 32.9 | 28.3 | 33.7 | 17.5 | 26.5 |

### 1c. Single years of age, 1910–1940 (Statistical Abstract 1943, No. 216) [read]

| Age | 1910 | 1920 | 1930 | 1940 |
|---|---|---|---|---|
| 5 | 17.0 | 18.8 | 20.0 | 18.0 |
| 6 | 52.1 | 63.3 | 66.3 | 69.1 |
| 7 | 75.0 | 83.3 | 89.4 | 92.4 |
| 8 | 82.7 | 88.5 | 94.1 | 94.8 |
| 10 | 90.0 | 93.0 | 97.1 | 95.7 |
| 12 | 89.8 | 93.2 | 97.1 | 95.5 |
| 13 | 88.8 | 92.5 | 96.5 | 94.8 |
| 14 | 81.2 | 86.3 | 92.9 | 92.5 |
| 15 | 68.3 | 72.9 | 84.7 | 87.6 |
| 16 | 50.6 | 50.8 | 66.3 | 76.2 |
| 17 | 35.3 | 34.6 | 47.9 | 60.9 |
| 18 | 22.6 | 21.7 | 30.7 | 36.4 |
| 19 | 14.4 | 13.8 | 19.8 | 20.9 |
| 20 | 8.4 | 8.3 | 13.1 | 12.5 |

- **Grouped, population-weighted** (computed from the counts):

  | Age | 1910 | 1920 | 1930 | 1940 |
  |---|---|---|---|---|
  | 5–6 | 34.5 | 41.0 | 43.2 | 43.0 |
  | 7–13 | 86.1 | 90.6 | 95.3 | 95.0 |
  | 14–17 | 58.9 | 61.6 | 73.1 | 79.3 |
  | 18–19 | 18.7 | 17.8 | 25.4 | 28.9 |

  1940 matches Digest 103.20 exactly.
- **Sex:** within about 2 points at 5–14. Girls led at 15–18 in 1910–20 (at 17 in 1920, 37.2 against 32.1). Boys led at 19–20 throughout, and at 18 from 1930.
- **1940 by colour** (No. 217, male / female) [read]:

  | Age | White | Nonwhite |
  |---|---|---|
  | 6 | 69.6 / 71.3 | 58.8 / 62.4 |
  | 10 | 96.0 / 96.2 | 92.3 / 93.2 |
  | 16 | 77.6 / 78.3 | 60.2 / 65.1 |
  | 17 | 62.8 / 63.1 | 42.1 / 46.9 |
  | 18 | 39.7 / 35.7 | 25.1 / 27.1 |
  | 20 | 15.1 / 11.0 | 8.3 / 7.4 |

### 1d. By age group, 1940–2024 (Digest 103.20; CPS A-2 for 2024) [read]

| Year | 3–4 | 5–6 | 7–13 | 14–17 | 18–19 | 20–24 | 25–29 | 30–34 |
|---|---|---|---|---|---|---|---|---|
| 1940 | — | — | 95.0 | 79.3 | 28.9 | 6.6 | — | — |
| 1950 | — | 74.4 | 98.7 | 83.7 | 29.4 | 9.0 | 3.0 | 0.9 |
| 1960 | — | 80.7 | 99.5 | 90.3 | 38.4 | 13.1 | 4.9 | 2.4 |
| 1970 | 20.5 | 89.5 | 99.2 | 94.1 | 47.7 | 21.5 | 7.5 | 4.2 |
| 1980 | 36.7 | 95.7 | 99.3 | 93.4 | 46.4 | 22.3 | 9.3 | 6.4 |
| 1990 | 44.4 | 96.5 | 99.6 | 95.8 | 57.2 | 28.6 | 9.7 | 5.8 |
| 2000 | 52.1 | 95.6 | 98.2 | 95.8 | 61.2 | 32.5 | 11.4 | 6.7 |
| 2010 | 53.2 | 94.5 | 98.0 | 97.1 | 69.2 | 38.6 | 14.6 | 8.3 |
| 2019 | 53.7 | 93.6 | 97.8 | 95.3 | 67.3 | 38.9 | 11.0 | 6.0 |
| 2022 | 53.3 | 93.0 | 97.5 | 95.2 | 63.9 | 37.2 | 10.7 | 5.1 |
| 2024 | 58.8 | 92.4 | 97.3 | 94.4 | 66.1 | 36.3 | 10.6 | 4.8 |

- **2024 comes from A-2,** which splits the groups differently: 7–9 96.8 and 10–13 97.7; 16–17 91.4; 20–21 51.1 and 22–24 26.4. The 2024 cells for 7–13, 14–17 and 20–24 average those splits weighted by years of age, not by population.
- 2020 is a COVID outlier: 3–4 at 40.3%, 5–6 at 89.2%.
- The 97–98% plateau at 7–13 since 2000 is plausibly home schooling plus misreporting. Not checked.

**18–19-year-olds by where they are** (Digest 103.20 columns 10–11; CPS A-5b) [read]:

| Year | In high school | In college | Dropped out | Graduated, not enrolled |
|---|---|---|---|---|
| 1963 | 10.9 | 29.8 | — | — |
| 1970 | 10.5 | 37.3 | 16.2 | 36.1 |
| 1980 | 10.5 | 36.0 | 15.7 | 37.8 |
| 1990 | 14.5 | 42.7 | 14.2 | 28.6 |
| 2000 | 16.5 | 44.7 | 12.6 | 26.2 |
| 2010 | 18.1 | 51.2 | 7.3 | 23.5 |
| 2024 | 18.6 | 47.5 | 5.6 | 28.3 |

- Still in high school at 18–19, male / female: 14.5 / 7.0 (1970), 17.0 / 12.0 (1990), 20.2 / 15.9 (2010), 19.6 / 17.6 (2024).
- The rise follows later school entry (§5a).

### 1e. By sex (CPS A-2) [read]

| Year | 16–17 M / F | 18–19 M / F | 20–21 M / F | 22–24 M / F | 25–29 M / F |
|---|---|---|---|---|---|
| 1950 | 72.8 / 69.8 | 35.7 / 24.3 | 14.3 / 4.6 (20–24) | | 5.9 / 0.4 |
| 1960 | 84.5 / 80.6 | 47.8 / 30.0 | 27.1 / 13.1 | 15.0 / 3.4 | 8.4 / 1.8 |
| 1970 | 91.3 / 88.6 | 54.4 / 41.6 | 42.7 / 23.6 | 21.2 / 9.4 | 11.0 / 4.3 |
| 1980 | 89.1 / 88.8 | 47.0 / 45.8 | 32.6 / 29.5 | 17.8 / 14.9 | 9.8 / 8.8 |
| 1990 | 92.7 / 92.4 | 58.2 / 56.3 | 40.3 / 39.2 | 22.3 / 19.9 | 9.2 / 10.2 |
| 2000 | 92.7 / 92.9 | 58.3 / 64.2 | 41.0 / 47.3 | 23.9 / 25.3 | 10.0 / 12.7 |
| 2010 | 94.9 / 97.3 | 66.9 / 71.5 | 49.2 / 56.0 | 27.0 / 30.8 | 13.5 / 15.8 |
| 2019 | 92.6 / 92.8 | 65.7 / 68.9 | 50.1 / 57.0 | 25.4 / 31.8 | 10.1 / 11.9 |

- Women overtook men at 25–29 by 1990, and at 18–24 between 1990 and 2000.
- Men led 2–4 to 1 at 20–24 in 1950–60 (the GI Bill and draft deferments).

### 1f. By race and Hispanic origin

**CPS A-2** [read]:

| Year | Group | 5–6 | 14–15 | 16–17 | 18–19 | 20–21 | 22–24 | 25–29 |
|---|---|---|---|---|---|---|---|---|
| 1960 | White | 82.0 | 98.1 | 83.3 | 38.9 | 20.6 | 9.3 | 5.2 |
| 1960 | Black and other | 73.3 | 95.9 | 76.9 | 34.6 | 11.9 | 4.4 | 2.9 |
| 1970 | White | 90.3 | 98.2 | 90.6 | 48.7 | 33.1 | 15.7 | 7.7 |
| 1970 | Black | 84.9 | 97.6 | 85.7 | 40.1 | 22.8 | 8.0 | 4.8 |
| 1980 | Hispanic | 94.5 | 94.3 | 81.8 | 37.8 | 19.5 | 11.7 | 6.9 |
| 2000 | White, non-Hispanic | 95.5 | 98.9 | 94.0 | 63.9 | 49.2 | 24.9 | 11.1 |
| 2000 | Black | 96.3 | 99.6 | 91.4 | 57.2 | 36.6 | 24.2 | 14.3 |
| 2000 | Hispanic | 94.3 | 96.2 | 87.0 | 49.5 | 26.1 | 18.2 | 7.4 |
| 2000 | Asian / Pacific Islander | 97.6 | 99.6 | 98.4 | 78.8 | 66.2 | 45.3 | 18.7 |
| 2019 | White, non-Hispanic | 94.0 | 97.9 | 93.4 | 68.1 | 54.4 | 27.7 | 9.8 |
| 2019 | Black | 93.0 | 97.7 | 91.4 | 60.7 | 48.1 | 27.9 | 13.7 |
| 2019 | Hispanic | 93.3 | 97.4 | 90.7 | 63.7 | 48.1 | 26.5 | 10.2 |
| 2019 | Asian | 92.5 | 99.5 | 94.0 | 86.2 | 78.4 | 47.0 | 17.6 |

**ACS 2023, the world's five heritage groups** (PUMS, `SCH` = 2 or 3) [computed]:

| Age | All | Men | Women | White | Black | Hispanic | API | AIAN |
|---|---|---|---|---|---|---|---|---|
| 3–4 | 48.8 | 49.1 | 48.6 | 50.7 | 50.4 | 42.8 | 53.8 | 47.0 |
| 5–6 | 90.9 | 90.9 | 90.9 | 91.0 | 90.9 | 90.6 | 93.1 | 90.0 |
| 7–13 | 97.8 | 97.7 | 97.8 | 97.8 | 97.1 | 97.9 | 98.0 | 97.4 |
| 14–17 | 97.0 | 96.7 | 97.3 | 97.1 | 96.7 | 96.6 | 98.2 | 95.7 |
| 18–19 | 72.1 | 68.4 | 76.0 | 74.0 | 68.4 | 66.2 | 90.7 | 60.4 |
| 20–24 | 38.0 | 34.1 | 42.0 | 38.9 | 34.2 | 32.8 | 61.4 | 24.6 |
| 25–29 | 13.2 | 11.3 | 15.0 | 11.8 | 14.4 | 12.5 | 22.4 | 10.6 |
| 30–34 | 7.6 | 6.6 | 8.6 | 6.5 | 10.3 | 7.6 | 9.8 | 8.8 |

- **Single years:**

  | Age | 3 | 4 | 5 | 6 | 10 | 16 | 17 | 18 | 19 | 20 | 21 | 22 | 23 | 24 | 26 | 30 |
  |---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
  | Enrolled % | 37.2 | 60.3 | 85.4 | 96.4 | 98.0 | 97.2 | 95.0 | 80.8 | 63.1 | 54.6 | 49.7 | 38.6 | 27.3 | 20.3 | 14.6 | 8.6 |

- Digest 103.30 (ACS 2022) [read] agrees, and adds Pacific Islanders (18–19 61.0, 20–24 25.9) and two or more races (70.4, 36.8).
- **Pattern:** groups converged at compulsory ages by 1970. What persists is at 3–4 and at 18+: at 20–24, Asian enrollment is about 1.6× White and AIAN about 0.65×.

### 1g. Kindergarten and preschool

**Public kindergarten before 1970** (120 Years Table 10 counts; per 100 five-year-olds, with the denominator from Statistical Abstract No. 216) [read; ratio computed]:

| School year | Public kindergarten pupils | Per 100 five-year-olds |
|---|---|---|
| 1910–11 | 326,883 | 16.0 |
| 1919–20 | 481,266 | 20.5 |
| 1929–30 | 723,443 | 28.9 |
| 1939–40 | 594,647 | 27.8 |
| 1949–50 | 1,034,203 | — |
| 1959–60 | 1,922,712 | — |
| fall 1970 | 2,563,579 | — |

- Public schools only, counted at any time in the year.
- Kindergarten spread through the mid-1970s, with the South last (Deming & Dynarski 2008) [read].

**3-, 4- and 5-year-olds, 1970–2018** (Digest 202.10, CPS October; the 1994 method changed; shares of 5-year-olds computed from counts) [read]:

| Year | 3-year-olds enrolled | 4-year-olds enrolled | 5-year-olds in kindergarten | 5-year-olds in preschool | Full-day share of 5-year-olds' kindergarten |
|---|---|---|---|---|---|
| 1970 | 12.9 | 27.8 | 66.9 | 2.4 | 11.6 |
| 1980 | 27.3 | 46.3 | 81.6 | 3.0 | 29.2 |
| 1990 | 32.6 | 56.1 | 79.5 | 9.3 | 42.9 |
| 2000 | 39.2 | 64.9 | 73.5 | 14.2 | 60.7 |
| 2010 | 38.2 | 68.6 | 72.9 | 13.5 | 76.0 |
| 2018 | 39.7 | 67.7 | 67.6 | 16.9 | 82.8 |

- 5-year-olds in grade 1 are excluded. The fall in "5-year-olds in kindergarten" after 1980, while preschool rose, is plausibly earlier cutoffs keeping 5-year-olds in preschool (§5a); this is our reading, not a source's.
- **ACS 2013–2023** (Digest 202.20) [read]:
  - 3–4-year-olds enrolled: 46–49%; 40.2% in 2021.
  - 3–5-year-olds enrolled, 2023: 47.7% with parents below high school, 68.7% with a bachelor's.
- **Kindergarten law, 2018** (State Education Reforms Table 5.3) [read]: 17 states require full-day kindergarten to be offered and 28 half-day; 18 jurisdictions require attendance.

---

## 2. Educational attainment by birth cohort

### 2a. Sources and conversions

| File (`datasets/education/attainment/`) | What |
|---|---|
| `census_cps_taba-1.xlsx`, `taba-2.xlsx`, `taba-4.xlsx` | CPS Historical Tables A-1 (years of school, 25+, by age and sex, 1940–2024), A-2 (HS+ and BA+ by race, Hispanic origin, sex, at 25+ and 25–29), A-4 (detailed, 2000–2024) |
| `census_cps2024_attain01_2024_{1,3,4,5,6}.xlsx`, `census_cps2014_table-1-01.xlsx`, `census_cps2004_tab01-01.xls` | CPS Table 1: attainment by 5-year age group and sex; all races, NH White, Black, Asian, Hispanic |
| `census_cps2024_attain03_2024.xlsx` | CPS Table 3: years of school, with GED split from diploma |
| `census1947_p20-15_tab-01.pdf`, `census1940_p10-8_tab-0{1,2}.pdf` | P-20 No. 15 (April 1947, with April 1940) and P-10 No. 8 (1940 by race-nativity, by state); scans |
| `nces_digest_d24_tabn104.20.xlsx`, `d23_tabn104.10.xlsx`, `d23_tabn104.40.xlsx` | Digest 104.20 (25–29, 1920–2024), 104.10 (25+, 1910–2023), 104.40 (detailed race, ACS 2012 and 2022) |
| `nces_digest_d24_tabn219.10.xlsx`, `d15_tabn219.60.xls`, `d22_tabn219.60.xlsx` | diplomas per 17-year-old; GED takers and passers |
| `census_cps_foreignborn_2023_asec_*_table5.xlsx` | attainment by nativity, year of entry, region of birth |
| `nces_bb1617_first_look_2019241.pdf` | B&B:16/17 (age at the bachelor's) |
| ACS 2023 PUMS | cohort × sex × heritage, GED share [computed] |

**Definitions that change** [read, table notes]:
- **Years before 1992, degrees after.** Before 1992 the CPS asked years completed: "HS+" is 12+ years and "BA+" is 16+ years. From 1992 it asks the degree, and HS includes the GED.
- **Race columns.** A-2's "Black" is "Black and other races" in 1940–62. White and Black include Hispanics before 1980. In 1940 Mexicans were classed white.
- **1910–1930** figures are back-projections from the 1940 census.

**From period to cohort.** People aged a–b in a survey of year Y were born in Y−b−1 to Y−a−1; for example, 25–29 in 1940 is the 1910–14 cohort. Three measurement points:
1. **At 25–29:** clean for high school, but BA+ reads 5–7 points below final.
2. **At 40–49:** the recommended point for final attainment.
3. **At older ages** (1940, 1947): the only data for cohorts born before 1910. These read high because of mortality selection and over-reporting; the same cohort reads 2–5 points higher HS+ in 1947 than in 1940.

### 2b. Cohorts born before 1922 (1940 census and CPS 1947; % of those reporting; counts from P-20 No. 15, pp. 7–8) [read]

| Cohort | Measured (age) | ≤ 8 years | < 5 years | HS+ | BA+ | Median years |
|---|---|---|---|---|---|---|
| ≤ 1874 | 1940 (65+) | 79.9 | 24.2 | 13.1 | 2.6 | 7.5 |
| 1875–84 | 1940 (55–64) | 73.2 | 19.7 | 16.8 | 3.4 | 7.8 |
| 1885–94 | 1940 (45–54) | 67.7 | 16.8 | 19.6 | 4.0 | 8.1 |
| 1895–1904 | 1940 (35–44) | 57.8 | 10.9 | 25.4 | 5.1 | 8.6 |
| 1905–09 | 1940 (30–34) | 46.2 | 7.1 | 33.0 | 6.3 | 9.5 |
| 1910–14 | 1940 (25–29) | 39.7 | 5.9 | 38.1 | 5.9 | 10.4 |
| 1912–16 | 1947 (30–34) | 32.7 | 4.8 | 45.7 | 6.7 | 11.4 |
| 1917–21 | 1947 (25–29) | 25.7 | 4.3 | 51.4 | 5.6 | 12.0 |

**By sex, CPS April 1947, HS+ / BA+** [read]:

| Cohort | Men | Women |
|---|---|---|
| ≤ 1881 | 16.1 / 4.2 | 18.3 / 2.8 |
| 1882–91 | 20.6 / 4.5 | 22.3 / 3.1 |
| 1892–1901 | 24.0 / 5.7 | 27.9 / 4.2 |
| 1902–11 | 34.7 / 8.2 | 38.8 / 6.1 |
| 1912–16 | 44.6 / 7.9 | 46.7 / 5.6 |
| 1917–21 | 49.4 / 5.8 | 53.3 / 5.4 |

- Women led in high school and men in BA in every pre-war cohort.
- The men's dip for 1917–21 is the war. Their GI Bill degrees came after: at 25–29 in 1950, men's BA+ was 9.6 against women's 5.9.

**Nonwhite, CPS 1947** (96% Black in 1940) [read]:

| Cohort | < 5 years | HS+ | BA+ | Median years |
|---|---|---|---|---|
| ≤ 1881 | 62.8 | 5.2 | 0.6 | 4.0 |
| 1882–91 | 45.9 | 7.1 | 1.4 | 5.5 |
| 1892–1901 | 39.5 | 9.8 | 3.0 | 6.0 |
| 1902–11 | 24.3 | 13.2 | 2.4 | 7.3 |
| 1912–16 | 20.8 | 20.2 | 3.9 | 8.1 |
| 1917–21 | 19.2 | 22.3 | 2.8 | 8.4 |

**1940, everyone 25+, by race and nativity** (P-10 No. 8, Table 1) [read]:

| Group | < 5 years | ≤ 8 years | HS+ | BA+ | Median (M / F) |
|---|---|---|---|---|---|
| Native white | 7.5 | 53.7 | 28.8 | 5.4 | 8.8 (8.6 / 9.0) |
| Foreign-born white | 29.0 | 81.0 | 11.9 | 2.4 | 7.3 |
| Negro | 42.0 | 84.0 | 7.3 | 1.3 | 5.7 (5.3 / 6.1) |
| Other races | 35.9 | | | | 6.8 |

- **Regional medians, 1940:** North 8.5, South 7.8, West 9.4. States ranged from 6.6 (Louisiana) to 10.3 (DC) [read].
- **Medians at 25–29** (120 Years Table 5) [read]:

  | Year | White men | Black men | White women | Black women |
  |---|---|---|---|---|
  | 1940 | 10.5 | 6.5 | 10.9 | 7.5 |
  | 1950 | 12.4 | 7.4 | 12.2 | 8.9 |
  | 1960 | 12.4 | 10.5 | 12.3 | 11.1 |
  | 1970 | 12.7 | 12.1 | 12.5 | 12.2 |

### 2c. Mean years and the high-school movement

- **Mean years at age 30, US-born** (Autor, Goldin & Katz 2020, NBER w26705, text to Figure 2) [read]:
  - 7.3 for the 1876 cohort, rising to 13.2 for 1951 (0.79 years per decade);
  - flat for 1951–66;
  - 14.3 for the 1987 cohort (0.29 per decade).
  - Values for each cohort and by sex exist only in figures (Goldin & Katz 2008, Figure 1.4). An IPUMS extract would give them.
- **Diplomas per 100 seventeen-year-olds** (Digest 219.10; diplomas of any age ÷ population aged 17; female share of graduates) [read]:

  | Year | Per 100 | Female share | Year | Per 100 | Female share |
  |---|---|---|---|---|---|
  | 1869–70 | 2.0 | 56% | 1949–50 | 59.0 | 52% |
  | 1879–80 | 2.5 | 55% | 1959–60 | 69.5 | 52% |
  | 1889–90 | 3.5 | 58% | 1969–70 | 76.9 (peak) | 51% |
  | 1899–1900 | 6.4 | 60% | 1979–80 | 71.4 | 51% |
  | 1909–10 | 8.8 | 59% | 1989–90 | 73.4 | — |
  | 1919–20 | 16.8 | 60% | 1999–2000 | 69.8 | — |
  | 1929–30 | 29.0 | 55% | 2009–10 | 79.7 | 51% (public) |
  | 1939–40 | 50.8 | 53% | 2022–23 | 86.3 | 50% (public) |

- The 1970–2000 dip is matched by the GED (§2h).
- The public adjusted cohort graduation rate: 79% (2010–11), 87.4% (2022–23).
- **Grades 9–12 enrolled per 100 aged 14–17** (120 Years Table 9) [read]:

  | Year | Per 100 |
  |---|---|
  | 1900–01 | 10.6 |
  | 1909–10 | 14.5 |
  | 1919–20 | 31.2 |
  | 1929–30 | 51.1 |
  | 1940–41 | 73.0 |
  | 1943–44 (war) | 63.0 |
  | 1959–60 | 86.9 |
  | fall 1970 | 92.0 |

### 2d. Cohorts 1910–1998 at 25–29 (CPS A-2; Digest 104.20) [read]

**HS+, %:**

| Year | Cohort | All | Men | Women | White (NH from 1995) | Black | Hispanic | Asian |
|---|---|---|---|---|---|---|---|---|
| 1920 | 1890–94 | | | | 22.0 | 6.3 | | |
| 1940 | 1910–14 | 38.1 | 36.0 | 40.1 | 41.2 | 12.3 | | |
| 1950 | 1920–24 | 52.8 | 50.6 | 55.0 | 56.3 | 23.6 | | |
| 1962 | 1932–36 | 65.9 | 65.8 | 66.1 | 69.2 | 41.6 | | |
| 1970 | 1940–44 | 75.4 | 76.6 | 74.2 | 77.8 | 56.2 | | |
| 1980 | 1950–54 | 85.4 | 85.4 | 85.5 | 86.9 | 76.6 | 58.6 | |
| 1990 | 1960–64 | 85.7 | 84.4 | 87.0 | 86.3 | 81.7 | 58.2 | |
| 2000 | 1970–74 | 88.1 | 86.7 | 89.4 | 94.0 | 85.9 | 62.8 | |
| 2010 | 1980–84 | 88.8 | 87.4 | 90.2 | 94.5 | 89.0 | 69.4 | 93.2 |
| 2020 | 1990–94 | 94.8 | 94.6 | 94.9 | 96.4 | 94.9 | 89.9 | 96.6 |
| 2024 | 1994–98 | 94.7 | 93.8 | 95.6 | 96.6 | 94.9 | 88.3 | 97.9 |

**BA+, %:**

| Year | Cohort | All | Men | Women | White (NH from 1995) | Black | Hispanic | Asian |
|---|---|---|---|---|---|---|---|---|
| 1920 | 1890–94 | | | | 4.5 | 1.2 | | |
| 1940 | 1910–14 | 5.9 | 6.9 | 4.9 | 6.4 | 1.6 | | |
| 1950 | 1920–24 | 7.7 | 9.6 | 5.9 | 8.2 | 2.8 | | |
| 1962 | 1932–36 | 13.1 | 17.2 | 9.2 | 14.3 | 4.2 | | |
| 1970 | 1940–44 | 16.4 | 20.0 | 12.9 | 17.3 | 7.3 | | |
| 1980 | 1950–54 | 22.5 | 24.0 | 21.0 | 23.7 | 11.6 | 7.7 | |
| 1990 | 1960–64 | 23.2 | 23.7 | 22.8 | 24.2 | 13.4 | 8.1 | 43.0 (A/PI) |
| 2000 | 1970–74 | 29.1 | 27.9 | 30.1 | 34.0 | 17.5 | 9.7 | 54.3 (A/PI) |
| 2010 | 1980–84 | 31.7 | 27.8 | 35.7 | 38.6 | 19.0 | 13.5 | 55.4 |
| 2020 | 1990–94 | 39.2 | 34.7 | 43.8 | 44.6 | 27.9 | 24.9 | 71.0 |
| 2024 | 1994–98 | 40.2 | 34.9 | 45.5 | 45.3 | 29.9 | 25.2 | 72.5 |

- **A-2 and Digest 104.20 differ for young Black adults around 1960–70.** 104.20 gives 58.4 / 10.0 in 1970 against A-2's 56.2 / 7.3; A-2 includes other races to 1962.
- **Master's or higher at 25–29:** 4.5% (1995), 6.8% (2010), 9.8% (2024; men 7.4, women 12.3).
- **AIAN at 25–29:** HS+ 79.2 (2000), 97.3 (2024); BA+ 15.9 (2000), 17.5 (2024). CPS samples are small, with coefficients of variation of 30–50%.
- **Two or more races, ACS 2022, 25+:** HS+ 89.4, BA+ 30.0.

### 2e. Final attainment, measured at 30–69 (CPS Table 1, 2004 / 2014 / 2024; % of everyone in the cohort, immigrants included) [read; computed from counts]

| Cohort | Measured (age) | < HS | HS | Some college | Associate | BA | Graduate | BA+ men | BA+ women |
|---|---|---|---|---|---|---|---|---|---|
| 1934–43 | 2004 (60–69) | 18.4 | 36.3 | 15.6 | 6.3 | 13.7 | 9.9 | 28.7 | 19.1 |
| 1944–53 | 2004 (50–59) | 11.0 | 30.8 | 18.1 | 9.1 | 18.0 | 12.9 | 33.6 | 28.4 |
| 1954–63 | 2004 (40–49) | 11.4 | 32.1 | 17.0 | 10.4 | 19.3 | 9.9 | 29.7 | 28.6 |
| 1964–73 | 2014 (40–49) | 10.5 | 27.7 | 16.0 | 10.9 | 22.6 | 12.3 | 33.8 | 36.1 |
| 1974–83 | 2024 (40–49) | 8.4 | 25.5 | 12.7 | 11.3 | 25.0 | 16.9 | 38.1 | 45.8 |
| 1984–93 | 2024 (30–39) | 6.7 | 25.2 | 13.5 | 10.6 | 27.6 | 16.4 | 40.3 | 47.9 |

- **Graduate degrees keep accruing.** The 1954–63 cohort went from 9.9% (2004) to 11.3% (2014) to 12.8% (2024). Part is real late completion; part is mortality, immigration and reporting.
- **The gender reversal:**
  - The men-to-women BA ratio was about 1.25 for the 1880–1910 cohorts. It rose after 1930 (Depression, GI Bill, Vietnam deferments), fell back to 1.25 by the 1950 cohort, and reached parity at the 1960 cohort (Goldin, Katz & Kuziemko 2006, JEP) [read, text only].
  - At 25–29, women's BA+ passed men's in 1991–92 (A-2) [read].
  - Women's HS+ at 25–29 was below men's for cohorts born about 1935–50, and above before and after.
- **Race gaps at 25–29:**
  - **Black–White HS+ gap:** 29 points (1940), 22 (1970), 8 (2000), 2 (2024).
  - **Black–White BA+ ratio:** about 1:4 (1940), 1:2 (1980), 2:3 (2024).
  - **Hispanic:** HS+ 52–63% from 1975 to 2005, then 88–90% by 2020; BA+ 8–10% until 2000, 25% in 2024. This mixes arrivals and natives.
- **2024 cross-section, BA+ / graduate** [read]:

  | Age | Cohort | All | NH White | Black | Asian | Hispanic |
  |---|---|---|---|---|---|---|
  | 25–29 | 1994–98 | 40.2 / 9.8 | 45.3 / 10.6 | 29.9 / 7.4 | 72.4 / 29.0 | 25.2 / 4.1 |
  | 30–34 | 1989–93 | 43.8 / 15.6 | 48.4 / 16.5 | 33.6 / 12.7 | 77.6 / 36.7 | 27.3 / 7.3 |
  | 40–44 | 1979–83 | 42.3 / 16.8 | 48.9 / 19.0 | 30.8 / 11.6 | 67.1 / 34.2 | 22.6 / 7.6 |
  | 60–64 | 1959–63 | 33.5 / 12.4 | 36.6 / 14.0 | 25.9 / 8.0 | 45.9 / 16.1 | 19.6 / 6.1 |
  | 75+ | ≤ 1948 | 32.7 / 15.3 | 35.6 / 16.9 | 21.8 / 10.9 | 42.6 / 17.5 | 14.1 / 4.9 |

### 2f. ACS 2023: five levels by cohort and sex, heritage, GED [computed]

- Ages 25+; birth year ≈ 2023 − age.
- Levels: < HS (`SCHL` ≤ 15); HS or GED (16–17); some college or associate (18–20); BA (21); graduate (22–24).
- Mean years: grade completed; 12 for a diploma or GED, 12.5 / 13.5 for some college, 14 associate, 16 BA, 18 MA, 19 professional, 20 doctorate.
- Old cohorts are survivors, so they read high.

| Cohort | Men < HS / HS / some coll. / BA / grad | Women < HS / HS / some coll. / BA / grad | Mean years M / F | GED share of HS-only M / F |
|---|---|---|---|---|
| 1933–37 | 17.6 / 30.0 / 19.8 / 16.4 / 16.2 | 18.7 / 40.2 / 21.1 / 12.1 / 7.9 | 13.1 / 12.3 | 13.0 / 7.1% |
| 1943–47 | 11.2 / 25.7 / 25.0 / 18.8 / 19.3 | 12.5 / 32.7 / 27.1 / 14.8 / 12.8 | 13.8 / 13.2 | 13.8 / 10.2% |
| 1953–57 | 11.1 / 28.4 / 28.7 / 18.1 / 13.6 | 9.7 / 28.0 / 31.2 / 18.2 / 12.8 | 13.5 / 13.5 | 14.5 / 13.0% |
| 1963–67 | 12.3 / 29.7 / 26.4 / 19.2 / 12.5 | 10.2 / 26.2 / 30.1 / 20.6 / 13.0 | 13.4 / 13.6 | 16.0 / 15.7% |
| 1973–77 | 12.4 / 25.7 / 26.5 / 20.8 / 14.5 | 10.2 / 20.4 / 27.8 / 23.2 / 18.3 | 13.6 / 14.0 | 20.0 / 18.5% |
| 1983–87 | 10.6 / 25.0 / 26.7 / 23.2 / 14.4 | 8.0 / 18.8 / 27.8 / 26.1 / 19.4 | 13.8 / 14.3 | 20.1 / 18.5% |
| 1993–97 | 8.1 / 29.0 / 27.6 / 26.3 / 9.1 | 5.7 / 21.4 / 28.0 / 30.5 / 14.4 | 13.7 / 14.3 | 12.5 / 11.7% |

- **Men's BA+ peaks at 37–38% for cohorts born 1943–52** (Vietnam-era deferments), falls to 31–32% for 1953–67, then rises. Women reach parity with the 1958–62 cohort (30.7 against 30.6%) and lead after.
- **BA+ by heritage, both sexes** (< HS in brackets):

  | Cohort | White | Black | Hispanic | API | AIAN |
  |---|---|---|---|---|---|
  | 1943–47 | 35.3 (7.0) | 19.8 (19.6) | 15.1 (41.3) | 41.3 (22.9) | 16.1 (21.8) |
  | 1958–62 | 33.7 (6.2) | 21.3 (13.2) | 17.1 (33.0) | 43.3 (18.0) | 15.2 (17.4) |
  | 1973–77 | 44.2 (5.0) | 29.2 (9.6) | 20.5 (30.7) | 60.3 (9.3) | 15.9 (12.8) |
  | 1988–92 | 47.3 (4.3) | 28.6 (8.2) | 24.7 (17.2) | 69.8 (4.5) | 16.0 (9.9) |

### 2g. Immigrants: bimodal attainment from abroad

**CPS ASEC 2023, 25+** (foreign-born Tables 1.5, 2.5, 3.5) [read]:

| Group | < 9th grade | < HS | HS only | Some college / associate | BA+ | Master's+ |
|---|---|---|---|---|---|---|
| Native | 1.3 | 5.5 | 28.5 | 27.3 | 38.7 | 14.5 |
| Foreign-born | 13.1 | 21.8 | 26.7 | 14.6 | 36.9 | 16.2 |
| born in Mexico | 28.6 | 47.0 | 32.8 | 11.1 | 9.1 | 2.1 |
| born in other Latin America | 14.3 | 23.3 | 33.0 | 17.3 | 26.4 | 9.3 |
| born in Asia | 4.5 | 8.3 | 18.5 | 11.8 | 61.4 | 29.7 |
| born in Europe | 3.3 | 5.3 | 23.4 | 18.0 | 53.4 | 26.9 |
| entered 2010 or later | 9.8 | 17.2 | 23.9 | 13.1 | 45.8 | 20.7 |
| entered before 1980 | 15.0 | 22.4 | 27.0 | 18.1 | 32.5 | 14.3 |

- **ACS 2023, ages 25–64** [computed]:

  | | < HS | HS | Some college | BA | Graduate |
  |---|---|---|---|---|---|
  | Natives | 6.5 | 25.2 | 30.0 | 24.3 | 14.0 |
  | Foreign-born | 22.9 | 22.1 | 18.2 | 20.1 | 16.8 |

- **1940, foreign-born whites** [read]: median 7.3 years against 8.8 for native whites; 29.0% had under 5 years.
- **Hispanics, ACS 2021, 25+** (Pew tabulation) [read]: US-born HS or less 44%, BA+ 24%; foreign-born 69% and 15%.

### 2h. GED and equivalency

**GED passers against regular diplomas** (Digest 219.60, 2015 ed.; 219.10; share computed) [read]:

| Year | GED passers | Regular diplomas | GED share of new HS credentials |
|---|---|---|---|
| 1971 | 227k | 2,889k | 7.3% |
| 1980 | 479k | 3,042k | 13.6% |
| 1990 | 410k | 2,574k | 13.7% |
| 2000 | 487k | 2,833k | 14.7% |
| 2001 | 648k (rush before the 2002 test) | 2,848k | 18.5% |
| 2008 | 469k | 3,312k | 12.4% |
| 2013 | 541k (rush before the 2014 test) | 3,478k | 13.4% |

- **Age at passing:**

  | Year | 16–18 | 19–24 | 25–29 | 30–34 | 35+ |
  |---|---|---|---|---|---|
  | 2000 | 32.7% | 37.1% | 10.7% | 7.0% | 12.6% |
  | 2013 | 22.4% | 35.1% | 15.1% | 10.6% | 16.9% |

- **History** (Heckman, Humphries & Mader 2010, NBER w16064) [read]:
  - introduced for veterans in 1942; civilians in New York from 1947; California the last state in 1974;
  - GEDs were 12% of credentials in 2008; in 2005, 20% of Black and 11% of White credentials;
  - 8–14% were issued in prisons.
- **After 2014** the test redesign cut passers sharply. In 2019, HiSET and TASC passers were about 40k and 22k [read]; GED passers are not quantified here.
- **The stock of GED holders: the sources disagree.**

  | Source | GED holders without college | Notes |
  |---|---|---|
  | CPS 2024 | 2.6% of 25+ | 9.3% of the HS-only group [read] |
  | NLSY97 (born 1980–84) | 7.5% at 27, 7.8% at 39 | Black men 16.9% at 39 [read] |
  | ACS 2023 | 5–21% of the HS-only group by cohort and sex; 16–21% for cohorts born 1963–87, peaking at 21.3% (men born 1978–82) | [computed] |

  The ACS sits between the two; the CPS question likely under-reports.

### 2i. Timing: when education ends

- **Age at bachelor's, 2015–16 recipients** (NCES 2019-241, B&B:16/17 Table 1) [read]:

  | Age | All recipients | First-time recipients |
  |---|---|---|
  | 23 or younger | 63.4% | 65.2% |
  | 24–29 | 21.0% | 20.3% |
  | 30+ | 15.5% | 14.5% |

  B&B trend data (§9h) give the same shape for the 1993, 2000 and 2008 graduates.
- **Time from first enrollment to BA** (Fast Facts 569) [read]:
  - median 52 months, with 44.1% taking ≤ 48 months;
  - by age at the degree: 45 months at 23 or younger, 81 at 24–29, 162 at 30+.
- **The same cohort at 27 and at 39** (NLSY97, born 1980–84; BLS, Table 5) [read]:

  | Group | Age | Dropout | HS diploma | GED | Some college | BA+ |
  |---|---|---|---|---|---|---|
  | All | 27 | 8.0 | 17.8 | 7.5 | 38.2 | 28.5 |
  | All | 39 | 5.7 | 15.1 | 7.8 | 36.1 | 35.3 |
  | Men | 39 | 6.1 | 17.6 | 9.8 | 35.8 | 30.8 |
  | Women | 39 | 5.3 | 12.4 | 5.7 | 36.5 | 40.1 |
  | NH Black | 39 | 7.8 | 15.0 | 12.4 | 42.4 | 22.4 |
  | Hispanic | 39 | 9.7 | 18.3 | 10.0 | 40.0 | 22.1 |

  - About a fifth of BA+ at 39 was earned after 27.
  - About 30% of dropouts at 27 hold a credential by 39.
- **Median age at the highest degree:** not found as a published number.

---

## 3. Intergenerational transmission

### 3a. Sources and data in hand

| Source | What | Licence | Path |
|---|---|---|---|
| GSS 1972–2024 cumulative (release 3a) | respondent's DEGREE and EDUC; father's and mother's degree and years (PADEG, MADEG, PAEDUC, MAEDUC); spouse's (SPDEG, SPEDUC); partner's (CODEG, COEDUC, 2012+). 75,699 respondents born 1883–2006 | NORC: free, citation requested | `transmission/GSS_stata.zip`, unzipped in `transmission/gss/` |
| CPS ASEC 2023 public use | both partners' education for married and cohabiting couples | public domain | `transmission/asecpub23csv.zip` |
| Census FG3 / UC3 2023 | husband × wife education margins; "both / one / neither BA" | public domain | `transmission/cps2023_tabfg3-all.xls`, `tabuc3-all.xls` |
| Schwartz & Mare 2005 replication archive | husband × wife cell counts, 5 levels, wives 18–40, 1940–2003; newlyweds 1940–1995 | no licence stated | `transmission/schwartz_mare_2005/` |

Papers read (copyrighted, cited): Hertz et al. 2007; Hilger 2015 (NBER w21217); Card, Domnisoru & Taylor 2018 (NBER w25000); NCES 2018-421 and 2017-437; Bailey & Dynarski 2011 (NBER w17633); Pew 2021; Greenwood et al. 2014 (NBER w19829); Hirschl, Schwartz & Boschetti 2024 (PMC12176447).

### 3b. Correlation and slope in years of schooling

**Published estimates:**

| Source | Children born | Correlation | Slope |
|---|---|---|---|
| Hertz et al. 2007, Tables 2 and 7 (IALS 1994 + ISSP 2000, n = 3,351; mean of parents) [read] | 1929–80 | 0.46 (rank 15 of 42 countries); trend +0.022 per 5-year cohort, significant | 0.46; trend −0.008, not significant |
| Couch & Dunn 1997 (PSID), cited by Hertz [abs] | | 0.40–0.43 | |
| Hilger 2015, text p. 17 (census co-residence and imputation) [read] | 1911–14 → 1931–34 → 1971–74 | | Whites 0.50 → 0.39, then flat; Blacks fell by more than half, 1940–2000 |
| Card, Domnisoru & Taylor 2018, §4.3 (1940 census, higher parent) [read] | about 1922–24 | | about 0.35 where teacher pay was high, about 0.75 where low |
| Mulligan 1999, cited by Card et al. [abs] | | | 0.14–0.45 |

**GSS, respondents 30+, weight `wtssps`; r / b of the child's years on the parent's** [computed]:

| Child born | n | Father | Mother | Higher parent | Mean of parents |
|---|---|---|---|---|---|
| 1883–1909 | 1,236 | 0.52 / 0.47 | 0.54 / 0.54 | 0.53 / 0.49 | 0.59 / 0.60 |
| 1910–19 | 2,276 | 0.48 / 0.39 | 0.47 / 0.41 | 0.50 / 0.42 | 0.52 / 0.49 |
| 1920–29 | 3,570 | 0.45 / 0.36 | 0.43 / 0.38 | 0.46 / 0.39 | 0.49 / 0.46 |
| 1930–39 | 4,730 | 0.42 / 0.33 | 0.42 / 0.39 | 0.46 / 0.40 | 0.47 / 0.44 |
| 1940–49 | 7,662 | 0.45 / 0.34 | 0.41 / 0.36 | 0.47 / 0.39 | 0.48 / 0.43 |
| 1950–59 | 8,528 | 0.45 / 0.32 | 0.43 / 0.37 | 0.47 / 0.38 | 0.49 / 0.42 |
| 1960–69 | 5,763 | 0.49 / 0.34 | 0.43 / 0.35 | 0.49 / 0.38 | 0.51 / 0.42 |
| 1970–79 | 3,398 | 0.51 / 0.38 | 0.48 / 0.36 | 0.52 / 0.40 | 0.54 / 0.43 |
| 1980–94 | 2,437 | 0.45 / 0.34 | 0.42 / 0.33 | 0.45 / 0.36 | 0.47 / 0.39 |

- **The correlation stays at 0.45–0.55 in every cohort while the slope falls from about 0.6 to 0.4,** as the parents' spread grows and the children's shrinks. This matches Hertz.
- Parents' schooling is recalled by the child, which attenuates every estimate. The 1883–1909 cohort was surveyed at 63+.
- **The parents' own spouse correlation** (as reported by the child): 0.74 for children born 1883–1909, 0.61–0.65 for 1910–69, 0.71–0.74 for 1970–94.

### 3c. Transition tables

**GSS: child's highest degree by the higher parent's degree, row %, respondents 30+** [computed]. GSS "HS" includes some college without a degree; "AA" is a junior-college degree.

| Child born | Parent | Child < HS | HS | AA | BA+ | Parents in this row |
|---|---|---|---|---|---|---|
| 1883–1929 | < HS | 55.1 | 37.3 | 1.1 | 6.5 | 71.5% |
| | HS | 17.5 | 58.8 | 2.2 | 21.5 | 22.0% |
| | AA | 7.8 | 41.3 | 9.6 | 41.2 | 0.8% |
| | BA+ | 5.4 | 39.4 | 5.0 | 50.1 | 5.6% |
| 1930–49 | < HS | 34.0 | 51.2 | 3.7 | 11.1 | 44.9% |
| | HS | 7.8 | 59.7 | 5.4 | 27.1 | 42.2% |
| | AA | 1.4 | 42.2 | 10.3 | 46.1 | 1.7% |
| | BA+ | 2.1 | 28.5 | 5.4 | 64.0 | 11.2% |
| 1950–69 | < HS | 30.1 | 52.3 | 6.9 | 10.7 | 24.0% |
| | HS | 6.0 | 59.1 | 10.1 | 24.7 | 50.9% |
| | AA | 2.9 | 42.5 | 21.1 | 33.5 | 4.2% |
| | BA+ | 1.1 | 27.9 | 8.7 | 62.3 | 20.9% |
| 1970–94 | < HS | 37.9 | 44.5 | 5.7 | 11.8 | 18.1% |
| | HS | 7.6 | 57.9 | 10.4 | 24.1 | 44.0% |
| | AA | 4.8 | 39.8 | 20.2 | 35.2 | 8.2% |
| | BA+ | 2.7 | 26.1 | 10.0 | 61.2 | 29.7% |

**P(child BA+) by how many parents have a BA** (GSS) [computed]:

| Child born | Neither | One | Both | All |
|---|---|---|---|---|
| 1883–1909 | 7.7 | 38.7 | 68.5 (n = 14) | 9.2 |
| 1940–49 | 23.2 | 63.3 | 76.0 | 29.7 |
| 1960–69 | 21.6 | 55.9 | 76.7 | 33.2 |
| 1980–94 | 27.8 | 51.5 | 74.5 | 41.0 |

**Pew 2021 (Fry), from the Fed's SHED 2019, adults 22–59, % with BA+** [read]:

| Parents | Child BA+ |
|---|---|
| no parent beyond high school | 20% |
| a parent with some college | 34% |
| one parent BA+ | 60% |
| both parents BA+ | 82% |
| no BA parent / a BA parent | 26% / 70% |

By group (no BA parent / a BA parent): White 29 / 72; Black 21 / 57; Hispanic 15 / 58.

**NCES, ELS:2002 (sophomores of 2002, about age 26 in 2012), by the higher parent's education** [read]:

| Outcome | No college | Some college | BA+ |
|---|---|---|---|
| finished high school | 92% | 97% | 98% |
| enrolled in postsecondary | 72% | 84% | 93% |
| BA+ among enrollees (NCES 2017-437 Figure 8) | 23% | 31% | 56% |

Multiplying the last two rows gives about 17%, 26% and 52% of all sophomores with a BA by 26 (our arithmetic, not NCES's).

**Change across cohorts: BA by 25, by family-income quartile** (Bailey & Dynarski 2011, Figure 3; NLSY79 and NLSY97) [read]:

| Quartile | Born 1961–64 | Born 1979–82 |
|---|---|---|
| bottom | 5% | 9% |
| second | 14% | 17% |
| third | 21% | 32% |
| top | 36% | 54% |

- Bottom and top are stated in the text; the middle two are read from the figure.
- **College entry:** bottom 19% → 29%, top 58% → 80%.
- The widening at the top is mostly daughters.

### 3d. A latent normal reproduces the tables

A bivariate normal with thresholds (polychoric) fitted to the GSS four-level tables [computed]:

| Child born | Latent r, all | White | Black |
|---|---|---|---|
| 1883–1929 | 0.58 | 0.57 | 0.57 |
| 1930–49 | 0.55 | 0.55 | 0.44 |
| 1950–69 | 0.55 | 0.54 | 0.46 |
| 1970–94 | 0.56 | 0.58 | 0.32 (n = 1,295) |

- **One latent correlation of about 0.55 fits every cohort,** within about 3 points for < HS, HS and BA+.
- **Two systematic misses:**
  - children of AA parents stay AA 20–21% of the time, against 12–13% fitted;
  - children of < HS parents reach BA+ 11–12% of the time, against 8% fitted, partly because of immigrants' children.
- **Transmission is weaker for Black families after 1930**, as are Hilger's falling Black slopes.

### 3e. Mother against father; siblings; early cohorts

**Joint regression on both parents' years, standardized** (GSS) [computed]:

| Child born | Sons: father / mother | Daughters: father / mother |
|---|---|---|
| 1883–1929 | 0.31 / 0.25 | 0.29 / 0.33 |
| 1930–49 | 0.30 / 0.22 | 0.27 / 0.30 |
| 1950–69 | 0.33 / 0.23 | 0.31 / 0.23 |
| 1970–94 | 0.37 / 0.16 | 0.30 / 0.28 |

- **Both parents matter about equally,** with sons leaning to the father. The higher parent and the mean of the parents predict almost equally (Hertz p. 11 agrees) [read].
- Causal designs differ: Behrman & Rosenzweig 2002 (twins) find a mother's effect near zero [abs]. The world needs the association, not the causal effect.
- **Children of immigrants** do better at every parental level (GSS, foreign-born, born 1950–94): BA+ 13 / 35 / 43 / 70% against 10 / 24 / 33 / 60% for the US-born [computed]. Card et al. show the same in 1940 [read].
- **Sibling correlation in years: about 0.60** (Mazumder 2004, Chicago Fed Letter 209, NLSY79: brothers 0.62, sisters 0.60) [read]. Other estimates are 0.62–0.67 (NLSY, PSID) [abs].
  - Parents' education alone (latent r ≈ 0.55) gives siblings about 0.55² ≈ 0.30, so **a shared family term worth as much again** is needed.
- **Before about 1880:** no parent–child estimate was found. The 1940 census first asked highest grade. Feigenbaum 2018 (Iowa 1915 → 1940) reports higher mobility than modern estimates [abs]; its numbers were not reached.

---

## 4. Educational assortative mating

### 4a. Spouse correlation by era

| Year | Couples | r of years / latent r | Source |
|---|---|---|---|
| 1940 | wives 18–40 | 0.60 / 0.71 | Schwartz–Mare cells [computed] |
| 1960 | same | 0.58 / 0.66 | same |
| 1980 | same | 0.60 / 0.67 | same |
| 2000 | same | 0.62 / 0.68 | same |
| 2003 (CPS) | same | 0.65 / 0.71 | same |
| 1960, 2005 | married (Greenwood et al. Table 1) | latent 0.67, 0.65 | [computed from their table] |
| 1970s → 2010s | GSS married, 25–64 | 0.60, 0.61, 0.59, 0.60, 0.64 | [computed]; the 2020s web-mode value of 0.52 is suspect |
| 2023 | CPS opposite-sex married, all ages | 0.64; latent (4 levels, both 25+) 0.73 | [computed] |
| 2023 | CPS cohabiting | 0.60 / 0.65 | [computed] |
| 2023 | ACS householder and spouse, both 25+ | 0.560 (married), 0.506 (cohabiting), 0.454 (same-sex) | [computed] |

- **The spouse correlation is about 0.55–0.65 in years (0.65–0.73 latent) in every decade since 1940.** Margins changed, not the strength of matching.
- Greenwood et al.'s "clearly higher in 2005" (Kendall's τ) is small next to this level [read].

### 4b. Same level, and who has more

**Schwartz–Mare cells, 5 levels by years (< 10, 10–11, 12, 13–15, 16+), wives 18–40** [computed]:

| Year | Same level | Same if random | Wife more | Husband more | Both 16+ |
|---|---|---|---|---|---|
| 1940 | 59.3% | 37.4% | 22.4% | 18.2% | 1.7% |
| 1960 | 45.0 | 24.0 | 26.8 | 28.2 | 4.0 |
| 1970 | 45.6 | 24.5 | 23.1 | 31.2 | 6.9 |
| 1980 | 49.1 | 25.5 | 20.4 | 30.5 | 11.4 |
| 1990 | 50.9 | 27.3 | 22.5 | 26.6 | 14.5 |
| 2000 | 53.0 | 27.0 | 25.1 | 22.0 | 18.0 |
| 2003 (CPS) | 55.3 | 27.3 | 24.6 | 20.1 | 22.2 |

- **Homogamy fell from 1940 to 1960 and then rose:** a U shape, as in Schwartz & Mare's text [abs].
- Newlyweds: 45.5% (1960), 50.4% (1980), 54.8% (1995).

**ACS 2023, opposite-sex couples, householder and spouse/partner, both 25+, 5 levels** (< HS, HS/GED, some college or associate, BA, graduate) [computed]:

| Man's age | Kind | Couples (sample) | r of years | Same level | Wife higher | Husband higher |
|---|---|---|---|---|---|---|
| 25–34 | married | 59,261 | 0.557 | 46.2% | 34.5% | 19.2% |
| 25–34 | cohabiting | 21,505 | 0.541 | 45.5% | 36.1% | 18.5% |
| 45–54 | married | 121,645 | 0.565 | 43.3% | 33.1% | 23.5% |
| 65–74 | married | 140,200 | 0.556 | 42.7% | 26.4% | 30.9% |
| 75+ | married | 89,387 | 0.571 | 42.5% | 22.5% | 35.0% |
| all 25+ | married | 665,554 | 0.560 | 43.4% | 30.7% | 25.8% |
| all 25+ | cohabiting | 69,634 | 0.506 | 41.8% | 36.5% | 21.7% |

**Married couples 25+, husband (rows) × wife (columns), % of couples** (ACS 2023) [computed]:

| | < HS | HS/GED | Some coll. | BA | Grad |
|---|---|---|---|---|---|
| < HS | 3.9 | 2.5 | 1.7 | 0.6 | 0.3 |
| HS/GED | 1.8 | 10.3 | 6.9 | 3.0 | 1.5 |
| Some coll. | 1.1 | 5.5 | 11.3 | 5.7 | 3.0 |
| BA | 0.4 | 2.3 | 5.1 | 10.1 | 5.6 |
| Grad | 0.2 | 1.0 | 2.6 | 5.8 | 7.8 |

**CPS ASEC 2023, opposite-sex married, all ages, 4 levels** (n = 28,968; "both BA+" 31.8% matches Census FG3) [computed]:

| | < HS | HS | Some college | BA+ |
|---|---|---|---|---|
| < HS | 4.2 | 2.2 | 1.1 | 0.5 |
| HS | 1.6 | 13.9 | 6.4 | 4.8 |
| Some college | 0.6 | 4.9 | 10.0 | 7.9 |
| BA+ | 0.3 | 3.3 | 6.6 | 31.8 |

- **CPS, same 4-level education:** 59.8% (31.5% if random); wife more 22.8%; husband more 17.3%. For wives 25–34: 63.4 / 25.1 / 11.5%.
- **CPS cohabiting:** same 54.7%; wife more 28.7%.
- **The ACS and CPS don't quite agree.** Merging BA and graduate, the ACS table gives 54.8% same-level and 29.3% both BA+, against the CPS's 59.8% and 31.8%. The universes differ (householder couples, both 25+, against all couples), and so may the coding of some college. Pick one survey per target.
- **Other years:** Pew 2010, wives 30–44, 1970 against 2007 [read]:

  | Year | Same | Wife more | Husband more |
  |---|---|---|---|
  | 1970 | about 52% | 20% | 28% |
  | 2007 | about 53% | 28% | 19% |

- **Census FG3 2023** [read]: neither spouse BA 44.8%, one 23.4%, both 31.8%. Cohabiting couples (UC3): 60.0 / 20.6 / 19.4%.
- **Hirschl, Schwartz & Boschetti 2024** (Demography 61(5); 6 levels) [read]:
  - homogamy odds held at about 4:1 from about 1990, then declined somewhat;
  - for newlyweds, 3.7:1 (1980) to 3.3:1 (2020), mainly from women marrying less-educated men;
  - homogamy is lower for cohabiting than married couples, and lower again for same-sex couples.

### 4c. What the ledger gives for free

Partners are made independent within the strata the ledger already pairs on, and homogamy is recomputed.

**CPS 2023** (married, both 25+, wife 25–64, n = 22,113; each partner redrawn from same-sex adults of the cell) [computed]:

| Independent within | Same 4-level | Latent r |
|---|---|---|
| observed | 61.0% | 0.725 |
| 5-year birth cohort | 30.1% | 0.01 |
| heritage (5 groups) | 31.3% | 0.07 |
| state | 30.2% | 0.01 |
| cohort × heritage | 32.5% | 0.09 |
| cohort × heritage × state | 34.0% | 0.13 |
| cohort × heritage × state × nativity | 34.8% | 0.17 |

**ACS 2023** (married, both 25+, 5 levels; U-statistic excluding each couple's own pairing) [computed]:

| Independent within | Same level | r of years |
|---|---|---|
| observed | 43.4% | 0.560 |
| nothing (national) | 22.2% | 0.00 |
| man's and woman's 5-year age bands | 22.7% | 0.015 |
| man's 10-year age × state × man's heritage × woman's heritage | 25.1% | 0.14 |

- **Age, heritage and a state-sized area give at most a quarter of the observed correlation,** and only 3–5 of the 21–31 points by which homogamy exceeds chance.
- **Parental transmission adds nothing,** because spouses' parents are also unmatched.
- Without education in partnering, 2023 margins would give 3.6% of couples a BA wife with a husband without high school, against 0.5% observed (CPS).
- Finer geography (tract, school) adds some through residential sorting; not measured.

### 4d. Mechanisms that matter for a model

- **Meeting in school.** Mare 1991 (ASR 56(1)): homogamy is higher when people marry soon after leaving school, "given that people who meet in school likely have the same attainment" [abs, via Schwartz 2023, PMC10713356]. A school or college focus layer could supply part of the homogamy.
- Blossfeld 2009 (ARS 35) [abs]: educational systems act as marriage markets.
- **How couples met** (Rosenfeld, Thomas & Hausen 2019, PNAS; HCMST 2017) [abs]: 39% online in 2017, 20% through friends. The share who met in school or college was not verified.
- The 1940–60 dip in homogamy came with more hypergamy while women married young (Hirschl et al.) [read].

---

## 5. School structure

### 5a. Grades by age and entry cutoffs

- **Convention:** kindergarten at 5, grade 1 at 6, grade 12 entered at 17. The "expected grade" is age − 5 (Deming & Dynarski 2008) [read].
- **Ayres's "normal ages"** (1909): grade 1 at 6–8, to grade 8 at 13–15 [read].
- **Entry cutoffs, 2018** (State Education Reforms Table 5.3; the child must be 5 by the date) [read]:

  | Cutoff | States |
  |---|---|
  | 31 July – 15 August | 8 (HI, NE; AR, IN, KY, MO, ND; TN) |
  | 31 August | 4 (DE, KS, NC, WA) |
  | 1–2 September | 22 |
  | 10–15 September | 3 (MT, IA, WY) |
  | 30 September – 15 October | 6 (DC, LA, NV, VA, CO, ME) |
  | 1 January | 1 (CT) |
  | district option | 5 (MA, NJ, NY, OH, PA) |
  | none stated | 2 (NH, VT) |

  - Connecticut moved to 1 September for fall 2024 [abs].
  - Michigan's 1 December cutoff was phased to 1 September in 2013–15 [not verified].
- **Cutoffs moved earlier, mostly 1970–1990** (Deming & Dynarski 2008, NBER w14124) [read]:
  - the population-weighted mean cutoff went from 25 November to 14 October;
  - **6-year-olds in grade 1 or above:** 96% (1968) → 84% (2005);
  - entry laws explain a quarter to a third; the rest is redshirting, kindergarten repetition and transition classes;
  - **17-year-olds:** in college 6–7% in the late 1960s, 2–3% by 2008; in grade 12 or above 68% (1968) → 63% (2005).
- **Redshirting:** 4–5.5% of children (Bassok & Reardon 2013) [abs].
  - By group: nearly 6% of White children, under 1% of Black children; boys twice as often as girls; high-income families about three times as often as low-income.
  - ECLS-K:2011: 6.2% delayed [abs].
- **Grade by age, ACS 2023** (% of all children that age; interviews run all year, so each age spans two grades) [computed]:

  | Age | Grades |
  |---|---|
  | 5 | K 52.1, preschool 30.9, not enrolled 14.6, grade 1 2.2 |
  | 6 | grade 1 47.5, K 44.4, not enrolled 3.6, grade 2 3.0 |
  | 8 | grade 3 47.1, grade 2 43.5, grade 1 4.0, grade 4 2.4 |
  | 10 | grade 5 46.9, grade 4 44.3, grade 3 3.8, grade 6 2.4 |
  | 13 | grade 8 47.6, grade 7 42.7, grade 6 4.6, grade 9 2.3 |
  | 15 | grade 10 48.4, grade 9 42.0, grade 8 3.8, grade 11 2.5 |
  | 17 | grade 12 45.4, grade 11 41.9, not enrolled 5.0, grade 10 4.6, college 2.4 |
  | 18 | college 38.5, grade 12 38.1, not enrolled 19.2, grade 11 3.5 |

  About 4–5% of each age sit one grade below the two modal grades, and 2–3% one above.

### 5b. Retention and age-for-grade

- **Around 1900–1930:**
  - **Ayres, *Laggards in Our Schools* (1909), 31 cities** (Internet Archive, public domain) [read]:
    - 33.7% of pupils were over-age for their grade, from 7.5% in Medford to 75.8% for Black pupils in Memphis;
    - repeaters were "a little over 16" per cent (6% in Somerville to 30% in Camden);
    - late entry explains under a third;
    - cities "carry all of their children through the fifth grade, … one-half of them to the eighth grade and one in ten through the high school".
  - **The grade-1 bulge,** grade-1 enrollment ÷ grade 2 (120 Years Table 10) [read]:

    | Year | Ratio |
    |---|---|
    | 1910–11 | 1.59 |
    | 1919–20 | 1.64 |
    | 1929–30 | 1.48 |
    | 1939–40 | 1.29 |
    | 1949–50 | 1.20 |
    | 1959–60 | 1.09 |
    | 1970 | 1.04 |
    | 1980 | 1.03 |

  - K–8 enrollment ÷ 5–13-year-olds was 104–108% from 1900 to 1936, against 99–102% in 1960–80: over-age pupils.
- **Below the modal grade for age** (CPS A-3; break at 2011) [read]:

  | Year | 6–8 | 9–11 | 12–14 | 15–17 |
  |---|---|---|---|---|
  | 1971 | 11.1 | 19.7 | 22.0 | 22.5 |
  | 1990 | 21.5 | 27.6 | 31.0 | 30.1 |
  | 2010 | 18.8 | 25.1 | 29.1 | 29.8 |
  | 2024 | 25.2 | 28.8 | 30.7 | 31.8 |

  - Boys were 5–11 points higher in 1971–90, and 1–6 points in 2024.
  - Black and Hispanic 15–17-year-olds were near 40% in 1990.
- **Retained in the same grade as last year** (Digest 225.90) [read]:

  | Year | All grades | K–8 | 9–12 |
  |---|---|---|---|
  | 1994 | 2.9 | 3.1 | 2.6 |
  | 2000 | 3.1 | 3.3 | 2.6 |
  | 2010 | 2.1 | 1.8 | 2.7 |
  | 2019 | 2.5 | 2.3 | 2.9 |
  | 2022 | 2.1 | 1.9 | 2.6 |

  - By group: Black 5.0 / Hispanic 4.1 / White 2.3 in 2000; 1.9 / 2.4 / 2.0 in 2022.
- **Ever repeated a grade,** public K–12, 2007 (NHES; NCES 2010-015 Table 17a) [read]:
  - 11.5% overall: boys 13.9, girls 8.9;
  - by group: White 8.7, Black 20.9, Hispanic 11.8, Asian 3.5, AIAN 13.0.

### 5c. Dropout timing

**Status dropout rate, 16–24** (Digest 219.70; 1960 from the census) [read]:

| Year | All | Male | Female | White | Black | Hispanic |
|---|---|---|---|---|---|---|
| 1960 | 27.2 | 27.8 | 26.7 | — | — | — |
| 1970 | 15.0 | 14.2 | 15.7 | 13.2 | 27.9 | — |
| 1980 | 14.1 | 15.1 | 13.1 | 11.4 | 19.1 | 35.2 |
| 1990 | 12.1 | 12.3 | 11.8 | 9.0 | 13.2 | 32.4 |
| 2000 | 10.9 | 12.0 | 9.9 | 6.9 | 13.1 | 27.8 |
| 2010 | 7.4 | 8.5 | 6.3 | 5.1 | 8.0 | 15.1 |
| 2022 | 5.0 | 5.5 | 4.4 | 4.1 | 5.7 | 7.0 |

- **2022 detail** (Digest 219.73):
  - by group: AIAN 8.1, Asian 1.6;
  - born abroad 8.7%; foreign-born Hispanic 14.8%.
- **Event dropout by grade** (CPS A-4, share of 15–24-year-olds in the grade who left that year) [read]:

  | Year | Grade 10 | Grade 11 | Grade 12 |
  |---|---|---|---|
  | 1970 | 5.0 | 5.7 | 6.5 |
  | 1980 | 4.3 | 6.1 | 7.8 |
  | 2010 | 0.7 | 2.0 | 6.3 |
  | 2024 | 2.5 | 3.5 | 9.8 |

- **CCD 2018–19** (Digest 219.50) [read]:
  - grades 9–12: 2.6% left in the year; by grade 9 / 10 / 11 / 12: 1.7 / 2.1 / 2.6 / 3.9%;
  - by group: AIAN 5.1, Black 4.0, White 1.8, Asian 1.0.
- **By age, 2022** (Digest 219.57; enrolled in grades 10–12) [read]: 15–16 5.6, 17 3.6, 18 4.4, 19 5.4, 20–24 18.3. Over-age students leave far more often.
- **Leaving ages before 1950:** exits cluster at the end of compulsory age (14–16) and after grade 8.
  - 1910: 89% enrolled at 13, 81% at 14, 68% at 15, 51% at 16, 35% at 17 (§1c).

### 5d. Compulsory attendance and child labour

- **Adoption** (Goldin & Katz 2003, NBER w10075) [read]:
  - Massachusetts was first, in 1852; 27 of the 48 states had a law by 1890; Mississippi was last, in 1918.
  - The 1852 Massachusetts law required ages 8–14 to attend 12 weeks a year [abs].
  - Other years (Vermont 1867, Michigan and Washington 1871) are commonly cited, not verified.
  - The state list is in Steinhilber & Sokolowski 1966 (US Office of Education Circular 793); not obtained.
- **Age limits** [read, same source]:
  - In 1910, 60% of states with laws set a minimum entry age of 8+. By 1939, almost 75% set it at ≤ 7.
  - By about 1930, 42 states set the maximum at 14+.
  - Exemption by schooling: 13 states required 8+ years in 1910, 35 by 1925.
- **Cohort view** [read, same source]:
  - born 1896: 33% faced no law at 7; mean binding years 4.9 (attendance) and 4.6 (child labour);
  - born 1925: 8.0 and 8.0;
  - the laws explain only about 5% of the 2.45-year rise in schooling.
- **Work permits:**
  - every state required age 14+ by 1925; around 1935, 11 states raised it to 16;
  - federal regulation effectively began in 1938.
- **Current law, 2017** (State Education Reforms Table 5.1) [read]:

  | Age | Compulsory start, states | Compulsory end, states |
  |---|---|---|
  | 5 | 11 (with DC) | |
  | 6 | 25 | |
  | 7 | 13 | |
  | 8 | 2 | |
  | 16 | | 15 |
  | 17 | | 10 |
  | 18 | | 25 |
  | 19 | | 1 |

  Free education usually runs to 21.

### 5e. School calendar and attendance

**Public schools** (120 Years Table 8; Digest 201.10) [read]:

| School year | Term (days) | Days attended per enrolled pupil | Attending on an average day |
|---|---|---|---|
| 1869–70 | 132.2 | 78.4 | 59.3% |
| 1889–90 | 134.7 | 86.3 | 64.1% |
| 1899–1900 | 144.3 | 99.0 | 68.6% |
| 1909–10 | 157.5 | 113.0 | 72.1% |
| 1919–20 | 161.9 | 121.2 | 74.8% |
| 1929–30 | 172.7 | 143.0 | 82.8% |
| 1939–40 | 175.0 | 151.7 | 86.7% |
| 1959–60 | 178.0 | 160.2 | 90.0% |
| 1979–80 | 178.5 | 160.8 | 90.1% |
| 1999–2000 | 179.4 | 169.2 | 94.3% |
| 2017–18 | 179.2 | 166.9 | 93.2% |

- Minimum days by state, 2018 (Table 5.14) [read]: 180 in 28 states, 175 in 4; several states set hours instead. Start dates are mostly a district option.
- **High-school share of public enrollment:** 1.2% (1870), 3.3% (1900), 10.2% (1920), 26.0% (1940), 28.6% (1970).

---

## 6. Schools: counts and sizes

### 6a. Counts by era

Digest 214.10 (d23); enrollment from 201.10 and 208.20; students per school derived [read]:

| School year | Regular districts | Public schools | One-teacher schools | One-teacher share | Public enrollment (k) | Students per school |
|---|---|---|---|---|---|---|
| 1869–70 | — | 116,312 | — | | 7,562 | 65 |
| 1899–1900 | — | 248,279 | — | | 15,503 | 62 |
| 1909–10 | — | 265,474 | 212,448 | 80% | 17,814 | 67 |
| 1919–20 | — | 271,319 | 187,948 | 69% | 21,578 | 80 |
| 1929–30 | — | 248,117 | 148,712 | 60% | 25,678 | 103 |
| 1939–40 | 117,108 | 226,762 | 113,600 | 50% | 25,434 | 112 |
| 1949–50 | 83,718 | ≈ 152,800 | 59,652 | ≈ 39% | 25,112 | ≈ 164 |
| 1959–60 | 40,520 | ≈ 117,600 | 20,213 | ≈ 17% | 36,087 | ≈ 307 |
| 1970–71 | 17,995 | 89,372 | 1,815 | 2.0% | 45,890 | 513 |
| 1990–91 | 15,358 | 84,538 | 617 | 0.7% | 41,217 | 488 |
| 2010–11 | 13,588 | 98,817 | 224 | 0.2% | 49,484 | 501 |
| 2022–23 | 13,318 | 99,388 | 169 | 0.2% | 48,861 | 492 |

- Before 1959–60 enrollment is cumulative over the year, so early students-per-school is an upper bound on attendance.
- Private schools: 9,275 PK–8 and 3,258 secondary (1929–30); 20,764 (1980–81); 35,895 (2001–02); 29,730 (2021–22).
- **Pupil transport at public expense,** as a share of average daily attendance (120 Years Table 13) [read]: 8.9% (1930), 31.2% (1950), 43.4% (1970), 59.4% (1990). It tracks consolidation.

### 6b. Public schools now (CCD 2024–25, 50 states and DC) [computed]

- **95,462 operating schools with membership > 0, holding 49.03M students:**

  | Type | Schools | Students |
  |---|---|---|
  | regular | 89,443 | 48.14M |
  | alternative | 4,144 | 0.52M |
  | special education | 1,363 | 0.16M |
  | career and technical | 512 | 0.21M |

- Charter schools: 7,997 (3.95M students, 8.1%). Fully virtual: 1,042 (0.61M).
- PK 1.37M; K–12 plus ungraded 47.66M.
- Digest 214.10 counts 99,388 for 2022–23. Its count includes operating schools with no reported membership, which our 95,462 leaves out.

**Regular, not fully virtual, by CCD level:**

| Level | Schools | Students | Mean | p10 | p25 | Median | p75 | p90 | Student-weighted median |
|---|---|---|---|---|---|---|---|---|---|
| Elementary | 51,463 | 22.54M | 438 | 181 | 292 | 417 | 557 | 714 | 511 |
| Middle | 15,825 | 8.87M | 561 | 164 | 311 | 529 | 762 | 988 | 721 |
| High | 17,640 | 14.55M | 825 | 117 | 259 | 548 | 1,229 | 1,937 | 1,438 |
| Other | 2,190 | 1.32M | 602 | 63 | 167 | 368 | 785 | 1,339 | 1,043 |
| All regular | 88,519 | 47.58M | 538 | 150 | 282 | 439 | 648 | 971 | 655 |

- These agree with Digest 216.45 for 2021–22: elementary 432, middle 569, regular high 821 [read]. The 216.40 size bins are in `schools/tabn216.40_d22.xlsx`.
- The student-weighted median is the size of the school the typical student attends.

**Students in one grade of one school** (a grade's roster before it is split into classes):

| Grade | Schools enrolling it | p10 | p25 | Median | p75 | p90 | Student-weighted median |
|---|---|---|---|---|---|---|---|
| K | 50,145 | 22 | 41 | 63 | 88 | 117 | 83 |
| 3 | 50,436 | 23 | 44 | 67 | 93 | 122 | 87 |
| 6 | 32,232 | 16 | 38 | 75 | 153 | 264 | 192 |
| 7 | 27,470 | 16 | 38 | 88 | 198 | 303 | 230 |
| 9 | 19,901 | 16 | 44 | 113 | 289 | 483 | 374 |
| 12 | 19,424 | 18 | 43 | 103 | 268 | 456 | 352 |

**By locale** (EDGE `LOCALE`; median enrollment):

| Level | City | Suburb | Town | Rural |
|---|---|---|---|---|
| Elementary | 437 | 475 | 402 | 301 |
| Middle | 610 | 690 | 468 | 290 |
| High | 800 | 1,224 | 655 | 291 |
| Share of schools / students | 27.7 / 30.6% | 30.9 / 38.7% | 10.9 / 10.2% | 30.4 / 20.5% |

Digest 214.40 (fall 2021) [read] gives average sizes of city 559, suburban 634, town 433 and rural 366; within rural, fringe 549, distant 281 and remote 163.

### 6c. Grade configuration

**Spans now** (CCD 2024–25, regular; `GSLO`–`GSHI`) [computed]:

| Span | Share of schools | Share of students | Median enrollment |
|---|---|---|---|
| PK–5 | 19.2% | 16.9% | 451 |
| 9–12 | 14.9% | 26.1% | 704 |
| 6–8 | 12.0% | 13.3% | 578 |
| K–5 | 10.7% | 8.7% | 420 |
| PK–6 | 4.9% | 3.5% | 368 |
| PK–8 | 4.2% | 3.3% | 384 |
| K–6 | 4.0% | 3.4% | 442 |
| K–8 | 3.7% | 3.2% | 414 |
| PK–4 | 2.4% | 1.9% | 408 |
| 7–12 | 2.4% | 1.7% | 268 |
| 7–8 | 2.2% | 2.1% | 501 |
| 5–8 | 1.8% | 1.4% | 375 |
| KG–12 or PK–12 | 2.2% | 2.5% | 253–560 |

**1920s–30s: the 8-4 plan gives way to junior and senior high schools** (Biennial Survey 1937–38, ED543892, OCR) [read]:

| Share of high-school pupils in | 1922 | 1930 | 1938 |
|---|---|---|---|
| 4-year high schools (8-4 plan) | 77.2 | 53.2 | 43.5 |
| junior high schools | 8.6 | 19.0 | 19.0 |
| junior-senior and undivided 6-year | 11.6 | 17.9 | 24.4 |
| senior high schools | 2.6 | 9.9 | 13.1 |

- **High-school size in 1937–38:** the typical urban high school had over 800 pupils, the typical rural one under 130.
- **Variation by state:** 95% of schools were reorganized in Alabama and Delaware, under 4% in North Carolina.

**1967–2016: middle schools replace junior highs** (Digest 216.10, d17) [read]:

| Year | Middle schools | Junior highs | 3–4-year high schools | 5–6-year high schools | Combined | One-teacher |
|---|---|---|---|---|---|---|
| 1970–71 | 2,080 | 7,750 | 11,265 | 3,887 | 1,780 | 1,815 |
| 1980–81 | 6,003 | 5,890 | 10,758 | 4,193 | 1,743 | 921 |
| 1990–91 | 8,545 | 4,561 | 11,537 | 3,723 | 2,325 | 617 |
| 2000–01 | 11,696 | 3,318 | 13,793 | 3,974 | 5,096 | 411 |
| 2015–16 | 13,022 | 2,594 | 16,243 | 3,995 | 6,788 | 197 |

Shares for 1940–66 were not found (RAND MG-139 returned 403).

### 6d. Teachers per pupil and class size

**Pupil/teacher ratio** (120 Years Table 14; Digest 208.20) [read]:

| Year | Public | Private | Catholic (students per instructional staff) |
|---|---|---|---|
| 1869–70 | 34.3 | | |
| 1899–1900 | 36.6 | | |
| 1919–20 | 31.8 | | 38.9 |
| 1939–40 | 29.1 | | 29.6 |
| 1949–50 | 27.5 | | 32.5 |
| 1960 | 25.8 | 30.7 | 34.6 |
| 1970 | 22.3 | 23.0 | 26.3 |
| 1980 | 18.7 | 17.7 | 21.3 |
| 1990 | 17.2 | 15.6 | 18.9 |
| 2010 | 16.0 | 12.5 | |
| 2019 | 15.9 | 11.4 | |

CCD 2024–25 [computed]: elementary 14.9, middle 15.0, high 16.0, all 15.2.

**Class size, measured** (NTPS 2017–18, Table 6a; self-contained / departmentalized) [read]:

| School | All | City | Suburban | Town | Rural |
|---|---|---|---|---|---|
| Primary | 20.9 / 26.2 | 21.4 / 28.1 | 21.3 / 27.2 | 20.5 / 25.1 | 19.5 / 23.2 |
| Middle | 16.6 / 24.9 | 17.6 / 25.3 | 15.5 / 25.5 | 18.3 / 23.9 | 16.0 / 23.3 |
| High | 16.3 / 23.3 | 17.7 / 24.5 | 15.3 / 24.3 | 16.8 / 21.3 | 15.9 / 20.8 |

- Self-contained classes at middle and high level are mostly special education. Use self-contained for primary homerooms and departmentalized for secondary sections.
- **Primary class size by school size:** 14.8 in schools under 100 students, 22.1–22.3 at 750+.
- **SASS 2011–12** (Digest 209.30): elementary 21.2, secondary 26.8.
- **Before 1987,** no direct class-size table was found. Class size is roughly 1.3–1.5× the pupil/teacher ratio (2011–12). City classes of 50+ around World War I are commonly cited [not verified].
- **Sections per grade, derived:** a median elementary grade of 63–67 children is 3 classes of 21. A median high-school grade of 100–115 students (student-weighted 350–370) is 5–16 sections of 23.

---

## 7. Private and home schooling

### 7a. Private share of K–12 by era (Digest 105.30; share derived) [read]

| Year | Private (k) | Private share |
|---|---|---|
| 1889–90 | 1,611 | 11.2% |
| 1909–10 | 1,558 | 8.0% |
| 1929–30 | 2,651 | 9.4% |
| 1949–50 | 3,380 | 11.9% |
| fall 1959 | 5,675 | 13.9% (peak) |
| fall 1969 | 5,500 | 10.8% |
| fall 1985 | 5,557 | 12.4% (wider universe from here) |
| fall 2000 | 6,169 | 11.6% |
| fall 2010 | 5,382 | 9.8% |
| fall 2019 | 5,486 | 9.7% |

- **By level** (Digest 205.10) [read]: PK–8 against 9–12 was 12.9% against 8.6% in 1995, and 10.8% against 8.4% in 2021.
- **By region, 2021:** Northeast 13.0%, Midwest 10.8%, South 9.5%, West 8.3%.
- **Now:** PSS 2023–24 counts 5.10M private students against CCD 2024–25's 47.66M public K–12, a 9.7% private share [computed].
  - ACS 2023 counts 12.8% of K–12 as "private", but the ACS includes home schooling there [computed].

**The Catholic system** (120 Years Table 15, NCEA data) [read]:

| Year | Schools | Enrollment | Of which secondary |
|---|---|---|---|
| 1919–20 | 8,103 | 1,925,521 | 129,848 |
| 1939–40 | 10,049 | 2,396,305 | 361,123 |
| 1960–61 | 12,893 | 5,253,791 | 880,369 |
| 1964–65 | 13,249 | 5,601,000 (peak) | 1,067,000 |
| 1980–81 | 9,559 | 3,106,000 | 837,000 |
| 1990–91 | 8,587 | 2,475,439 | 591,533 |
| 2021–22 (PSS) | 6,120 | 1,816,480 | |

- Catholic schools were about 90% of private enrollment around 1960, and a third now.
- They averaged about 410 pupils in 1960–61, against about 300 in public schools.

### 7b. Private schools now (PSS 2023–24, weighted by `PFNLWT`) [computed]

- **30,553 schools and 5.10M students** (K–12 and ungraded). Every record has coordinates.

| Affiliation | Share of schools | Share of students | Mean size |
|---|---|---|---|
| Catholic | 20.4% | 33.7% | 275 |
| Other religious | 46.3% | 42.1% | 152 |
| Nonsectarian | 33.3% | 24.2% | 121 |

- **By typology** (share of students):

  | Typology | Share |
  |---|---|
  | Catholic diocesan | 16.5% |
  | Catholic parochial | 7.9% |
  | Catholic private | 9.2% |
  | Conservative Christian | 14.3% |
  | Other religious, affiliated | 13.8% |
  | Other religious, unaffiliated | 14.0% |
  | Nonsectarian regular | 17.2% |
  | Nonsectarian special program | 4.7% |
  | Nonsectarian special education | 2.3% |

- **By level:**

  | Level | Share of schools | Share of students |
  |---|---|---|
  | Elementary | 57.3% | 38.9% |
  | Secondary | 9.4% | 14.5% |
  | Combined | 33.3% | 46.6% |

- **Size:**
  - median 79 students (p10 11, p25 27, p75 205, p90 413), mean 167, student-weighted median 360;
  - 39.4% of schools have under 50 students, but they hold 5.3% of students.
- **Locale, share of schools / students:** city 35.3 / 45.1%, suburb 35.2 / 37.5%, town 8.5 / 5.9%, rural 21.0 / 11.5%. Private schooling is urban.
- PSS 2021 (Digest 205.40) [read] gives similar figures: 29,730 schools averaging 184 students; 29% under 50 students.

### 7c. Who goes private, and who is homeschooled (NHES 2019, Digest 206.20; % of ages 5–17 in K–12) [read]

| Group | Public, assigned | Public, chosen | Private | Homeschool |
|---|---|---|---|---|
| All | 70.8 | 16.8 | 9.3 | 2.8 |
| White | 70.7 | 13.0 | 12.0 | 4.0 |
| Black | 67.4 | 23.7 | 7.4 | 1.2 |
| Hispanic | 71.3 | 21.1 | 5.4 | 1.9 |
| Asian | 72.8 | 17.4 | 9.1 | ‡ |
| Parents high school or less | 79.8 | 14.6 | 3.5 | 1.8 |
| Parents with a bachelor's | 66.8 | 17.4 | 12.3 | 3.3 |
| Parents with a graduate degree | 64.6 | 15.6 | 16.7 | 3.1 |
| Poor | 74.9 | 17.1 | 5.2 | 2.6 |
| Two parents | 69.4 | 16.3 | 10.8 | 3.3 |
| One parent | 73.4 | 18.8 | 5.9 | 1.6 |
| City | 60.2 | 26.1 | 10.9 | 2.5 |
| Suburban | 74.4 | 14.2 | 8.7 | 2.4 |
| Town | 84.0 | 8.4 | 5.1 | 2.2 |
| Rural | 74.5 | 10.6 | 10.1 | 4.7 |

- **Defects in the spreadsheet:** the d22 sheet has corrupted 2016 locale and region cells (city sums to 87%).
- **Rural private, 10.1% in 2019,** disagrees with 6.4–6.8% in 2007 and 2016. Check it before using.
- **Family religion:** no federal table of private enrollment by family religion.

### 7d. Homeschooling by era

| Year | Share | Source |
|---|---|---|
| 1999 | 1.7% (850k) | NHES, Digest 206.10 [read] |
| 2003 | 2.2% | same |
| 2007 | 3.0% | same |
| 2012 | 3.4% | same |
| 2016 | 3.3% | same |
| 2019 | 2.8% (the hours cutoff changed) | same |
| 2022–23 | 3.4%; 5.2% taught at home including full-time virtual | NHES:2023, IES release [read] |
| 2020 | 5.4% → 11.1% of *households* | Household Pulse [read]; not comparable |

- Before 1999: about 0.3M around 1990 is commonly cited [not verified].
- **Who, 2019:**
  - by group: White 4.0%, Black 1.2%;
  - rural 4.7%;
  - three or more children 3.9%;
  - two parents, one in the labour force: 6.6%.

### 7e. Charter, magnet, virtual

| Year | Charter schools | Charter enrollment | Share of public enrollment |
|---|---|---|---|
| 1999–2000 | 1,524 | 0.34M | 0.7% |
| 2011–12 | 5,696 | 2.06M | 4.2% |
| 2022–23 | 7,998 | 3.72M | 7.6% |
| 2024–25 (CCD) [computed] | 7,997 | 3.95M | 8.1% |

- Charters by locale: city 56%, suburban 26%.
- **Magnet, 2019–20:** 3,497 schools with 2.69M students (5.3%).
- **Virtual, 2022–23:** 1,323 schools with 0.56M students.

---

## 8. Attendance zones and school assignment

### 8a. Who attends the zoned school

| Year | Assigned public | Chosen public | Private | Homeschool | Population and source |
|---|---|---|---|---|---|
| 1993 | 80 | 11 | 10 | excluded | grades 1–12; NCES 2010-004 [read] |
| 2007 | 73 | 16 | 12 | excluded | same |
| 1999 | 74.1 | 14.3 | 10.0 | 1.7 | ages 5–17 in K–12; Digest 206.20 [read] |
| 2016 | 68.8 | 18.7 | 9.2 | 3.3 | same |
| 2019 | 70.8 | 16.8 | 9.3 | 2.8 | same |

- **Among public-school students, the assigned share** was 83.9% (1999) and 80.8% (2019).
  - In cities in 2019: 60.2% of all K–12 students attended their assigned school, against 84.0% in towns.
- **Parents who moved to the neighbourhood for the school:** 27% in 2007, 19.1% in 2019 (Digest 206.40) [read].
- Public school choice was available to 42.3% of students in 2019.

### 8b. Measured zones: SABS 2015–16

- **The source:** the NCES School Attendance Boundary Survey (https://nces.ed.gov/programs/edge/sabs), public domain.
  - **File:** `SABS_1516.zip` (584 MB, downloaded) holds 75,128 polygons. Fields: `ncessch`, `schnam`, `leaid`, `gslo`, `gshi`, `level` (1 primary, 2 middle, 3 high, 4 other), `defacto`, `openEnroll`, `MultiBdy`.
  - **Coverage** (technical documentation) [read]: 12,855 qualifying districts. Boundaries were collected for 12,119: 4,891 non-de-facto and 7,964 de facto (one school per grade, so the district is the zone). That is 72,872 of 79,813 schools.
  - **Exclusions:** charter, magnet, special and alternative schools by default.
  - **No feeder patterns** between levels.
- **2013–14** (`SABS_1314.zip`, 569 MB) and level-split versions also exist; not downloaded.

**Overlaid on 2020 block-group and tract population centres** [computed]:

| Level | Zones | Population in de facto / address-based / open-enrollment zones | Address-based zone population p10 / p25 / p50 / p75 / p90 | Tracts per zone, median (p10–p90) | People in tracts split between 1 / 2 / 3+ zones |
|---|---|---|---|---|---|
| Primary | 43,976 | 13.0 / 77.1 / 15.7% | 3,003 / 4,686 / 6,873 / 9,509 / 12,776 | 3 (1–5) | 48.4 / 42.2 / 9.4% |
| Middle | 14,719 | 9.7 / 71.8 / 8.8% | 8,237 / 14,062 / 21,284 / 30,317 / 42,375 | 6 (3–13) | 75.0 / 22.8 / 2.3% |
| High | 14,172 | 11.8 / 75.3 / 18.7% | 7,612 / 16,430 / 29,244 / 43,959 / 61,147 | 8 (3–17) | 82.1 / 16.6 / 1.3% |

- **How the overlay works:**
  - zone population: each block group goes to the smallest address-based zone containing its centre;
  - split tracts: counted only for tracts whose block groups are all inside address-based zones;
  - population shares overlap because open-enrollment zones coincide with whole districts.
- **About 60% of districts are de facto, but they hold only 10–13% of people.** De facto districts are small and rural.
- **Splitting is a lower bound,** because blocks are finer than block groups. Half of people live in tracts that straddle elementary zones.
  - **So elementary catchments in dense areas need sub-tract units.** Middle and high zones mostly follow tract lines (75–82%).
- **Monarrez (2023), *AEJ: Applied* 15(3)** [read, abstract], from boundary maps for about 1,600 districts:
  - "attendance boundaries create 5 percent more integration than a distance-minimizing baseline, while school siting plays almost no role";
  - residential segregation explains more than all of school segregation.
  - **So nearest-school partitions are a sound null model.**
- **SABINS** (2009–12, via IPUMS NHGIS; free with citation): 13,169 zones in the 307 largest districts, about 43 per district [abs].
- **Segregation check** (Digest 216.50, fall 2023) [read]:
  - 40.2% of Black and 41.2% of Hispanic students attend schools that are at least 90% students of color;
  - 40.7% of White students attend schools under 25% students of color.

### 8c. Nearest-school catchments over 2020 tracts [computed]

Each populated tract centre (83,848 tracts, 331.4M people) is assigned to the nearest regular public school that enrolls the grade:

| Grade | Schools | Median distance, tract centre to school | p90 | Within 1 / 2 mi | Schools nearest to no tract | Median tracts per school | Median catchment population |
|---|---|---|---|---|---|---|---|
| 3 | 50,436 | 0.67 mi | 2.73 mi | 66.1 / 84.9% | 13.2% (9.5% of grade-3 students) | 2 | 6,559 |
| 7 | 27,470 | 1.12 mi | 3.78 mi | 44.8 / 74.0% | 9.7% | 3 | 10,600 |
| 10 | 19,572 | 1.40 mi | 4.29 mi | 34.3 / 66.9% | 8.0% | 4 | 13,909 |

- Distances are population-weighted.
- **Elementary Voronoi catchments match SABS sizes** (median 6,600 against 6,900 people). Middle and high Voronoi catchments are smaller than real zones (10,600 against 21,300; 13,900 against 29,200), because the nearest-school rule counts every school that enrolls the grade, including small rural and K–12 schools. Real zones serve the district's middle and high schools only.
- **The national ratio is about 92 residents per enrolled student of one grade.** Voronoi catchments range 48–213 (p10–p90) per grade-3 student, so pure nearest assignment over- or under-fills schools by about 2×. A capacity term is needed.

### 8d. Distance from home to school (NHTS)

- **K–8 students living under a mile away:** 25% in 1995, 22% in 2009 (National Center for Safe Routes to School 2011) [read].
- **Mean distance, NHTS 2009:** 4.4 miles; elementary 3.6, high school 5.5 (McDonald et al. 2011) [read].
- **Children aged 6–12 living a mile or more away:** 54.8% in 1969, about three-quarters in 2001 (FHWA brief, 2008) [read].
- Walking or biking among K–8 students: 47.7% (1969) → 12.7% (2009).
- **The figures disagree by definition.** A widely quoted "41% → 31% within a mile" was not found in the documents read.
- The tract-centre distances in §8c are to the nearest school, not the attended one, so they run lower.

### 8e. How synthetic-population tools assign schools (code or documentation read)

| Tool | Licence | School rule | Classes |
|---|---|---|---|
| **IDM SynthPops** (`synthpops/schools.py`, `pop.py`) | MIT | **No geography.** Students are listed in household order, "so siblings tend to share a school". It draws a school type for an age and a size from that type's distribution, then takes that many students from the front of the age lists. It cannot model K–8 or K–12 schools. | `random` (mean degree 20), `age_clustered`, or `age_and_class_clustered`: classes of Poisson(20) per grade with one teacher; student/teacher 20, student/staff 15 |
| **RTI 2010 US Synthetic Population v1** (README) | CC BY 4.0 | **Private:** the closest school within 50 km with capacity for the grade. **Public:** of the 3 closest schools in the same county offering the grade, the lowest enrolled/capacity ratio. Overfills by the same rule; widens to the state if the county has none. | not modelled |
| **FRED** (Grefenstette et al. 2013, `Neighborhood_Patch.cc`) | BSD-3 | Uses RTI's assignments. A child who reaches school age or moves draws from the schools that same-age residents of the grid patch attend. | optional classrooms |
| **SPEW** (Gallagher et al. 2017, §3.4) | GPL-3 | Gravity: P ∝ 1/distance × a step function of capacity | not modelled |
| **Covasim** (`population.py`) | MIT | No geography: everyone aged 6–21 in Poisson(20) clusters | clusters stand in for classes |
| **UVA Biocomplexity (NDSSL)** | own terms (not checked) | "assigned using NCES data"; the rule isn't stated | sub-location contact model |

None of them uses attendance zones. FRED's "draw from what same-age neighbours attend" is a natural rule for movers under static catchments.

---

## 9. College

### 9a. Participation by era, 1870–1991

Two series that don't splice:
- **Ratio** = total enrollment of all ages ÷ population 18–24. This is all that exists before 1970.
- **Rate** = share of 18–24-year-olds enrolled (CPS, §9b).

In 1970 the ratio is 35.8 against a rate of 25.7, because 28% of students were 25+.

| Year | Enrollment | Ratio to pop. 18–24 | Institutions | Mean size | Women % of students |
|---|---|---|---|---|---|
| 1869–70 | 52,286 | 1.3 | 563 | 93 | 21.3 |
| 1889–90 | 156,756 | 1.8 | 998 | 157 | 35.9 |
| 1899–1900 | 237,592 | 2.3 | 977 | 243 | 35.9 |
| 1909–10 | 355,213 | 2.8 | 951 | 374 | 39.6 |
| 1919–20 | 597,880 | 4.7 | 1,041 | 574 | 47.3 |
| 1929–30 | 1,100,737 | 7.2 | 1,409 | 781 | 43.7 |
| 1939–40 | 1,494,203 | 9.1 | 1,708 | 875 | 40.2 |
| fall 1949 | 2,444,900 | 15.2 | 1,851 | 1,321 | 29.6 |
| fall 1959 | 3,639,847 | 23.8 | 2,004 | 1,816 | 35.9 |
| fall 1969 | 8,004,660 | 35.0 | 2,525 | 3,170 | 40.7 |
| fall 1979 | 11,569,899 | 38.8 | 3,152 | 3,671 | 50.9 |
| fall 1989 | 13,538,560 | 51.4 | 3,535 | 3,830 | 54.3 |
| fall 2020 | 19,027,410 | — | 3,931 | 4,840 | 58.6 |

Sources: Digest 301.20 (enrollment, institutions, women) and 120 Years Table 24 (ratio) [read].

- **War and the GI Bill:**
  - the ratio fell to 6.8 in 1943–44, then rose to 12.5 (fall 1946) and 15.2 (1949);
  - men were 71% of students in 1947;
  - veterans were 49% of admissions in 1947 (VA) [read].
- **The Depression barely moved it:** 7.2 (1930), 6.7 (1934), 9.1 (1940).

### 9b. Share of 18–24-year-olds enrolled (Digest 302.60; CPS) [read]

| Year | Total | 2-year | 4-year | Men | Women | Black | Hispanic | White | Asian | AIAN |
|---|---|---|---|---|---|---|---|---|---|---|
| 1970 | 25.7 | — | — | 32.1 | 20.3 | 15.5 | — | 27.1 | — | — |
| 1980 | 25.7 | 7.1 | 18.6 | 26.4 | 25.0 | 19.4 | 16.1 | 27.3 | — | — |
| 1990 | 32.0 | 8.7 | 23.3 | 32.3 | 31.8 | 25.4 | 15.8 | 35.1 | 56.9 | 15.8 |
| 2000 | 35.5 | 9.4 | 26.1 | 32.6 | 38.4 | 30.5 | 21.7 | 38.7 | 55.9 | 15.9 |
| 2010 | 41.2 | 13.0 | 28.2 | 38.3 | 44.1 | 38.4 | 31.9 | 43.3 | 63.6 | 41.4 |
| 2019 | 40.7 | 10.3 | 30.4 | 37.0 | 44.3 | 37.0 | 36.3 | 41.1 | 62.0 | 23.6 |
| 2022 | 39.0 | 8.5 | 30.5 | 34.2 | 43.8 | 36.0 | 32.8 | 40.7 | 60.8 | 25.8 |

- AIAN figures carry standard errors of 4–7 points.
- **Women passed men** among 18–24s about 1980, and among all students in 1978–79.

**Immediate college entry of recent high-school completers** (Digest 302.10 and 302.20) [read]:

| Year | Total | 2-year | 4-year | Men | Women | White | Black | Hispanic | Asian |
|---|---|---|---|---|---|---|---|---|---|
| 1960 | 45.1 | — | — | 54.0 | 37.9 | | | | |
| 1970 | 51.7 | — | — | 55.2 | 48.5 | | | | |
| 1980 | 49.3 | 19.4 | 29.9 | 46.7 | 51.8 | 51.5 | 44.0 | 49.6 | — |
| 1990 | 60.1 | 20.1 | 40.0 | 58.0 | 62.2 | 63.0 | 48.9 | 52.5 | 81.4 |
| 2000 | 63.3 | 21.4 | 41.9 | 59.9 | 66.2 | 65.4 | 56.4 | 48.6 | 81.3 |
| 2009 | 70.1 (peak) | 27.7 | 42.4 | 66.0 | 73.8 | | | | |
| 2019 | 66.2 | 21.8 | 44.4 | 62.0 | 69.8 | 68.0 | 57.5 | 61.5 | 82.1 |
| 2023 | 61.4 | 15.6 | 45.9 | 57.6 | 65.3 | 63.3 | 61.7 | 54.7 | 79.9 |

Race columns are 3-year moving averages.

### 9c. Institutions

**Number by level and control** (Digest 317.10; branches excluded before 1979) [read]:

| Year | Total | 4-year | 2-year | Public | Private nonprofit | For-profit |
|---|---|---|---|---|---|---|
| 1949–50 | 1,851 | 1,327 | 524 | 641 | 1,210 (all private) | — |
| 1969–70 | 2,525 | 1,639 | 886 | 1,060 | 1,465 (all private) | — |
| 1979–80 | 3,152 | 1,957 | 1,195 | 1,475 | 1,677 (all private) | — |
| 1999–2000 | 4,084 | 2,363 | 1,721 | 1,682 | 1,681 | 721 |
| 2012–13 | 4,726 | 3,026 | 1,700 | 1,623 | 1,652 | 1,451 |
| 2022–23 | 3,896 | 2,628 | 1,268 | 1,599 | 1,614 | 683 |

- **Public 2-year colleges:** 297 (1950) → 926 (1980) → 817. The 1960s community-college build-out is the main structural change.

**IPEDS HD2024: active, degree-granting, Title IV, 50 states and DC: 3,867 institutions, all with coordinates** [computed]:

| Sector | Institutions | Share of undergraduates, fall 2023 | Mean undergraduates |
|---|---|---|---|
| public 4-year | 824 | 50.4% | 9,651 |
| public 2-year | 757 | 27.0% | 5,623 |
| private nonprofit 4-year | 1,495 | 17.5% | 1,850 |
| for-profit 4-year | 292 | 3.9% | 2,097 |
| for-profit 2-year | 352 | 1.1% | 492 |
| private nonprofit 2-year | 81 | 0.2% | 342 |

- Administrative units: 66. HBCUs: 100. Tribal colleges: 35.
- **Fall 2023 (EF2023A):** 19.00M students; 15.79M undergraduate, 3.21M graduate, 2.83M first-time.
- **Size:** median total enrollment 1,802 (p10 145, p90 12,748).
  - Institutions with 20,000+ students: 5.4% of institutions, 39.0% of students.
  - Under 1,000: 37% of institutions, 2.8% of students.
  - Digest 317.40 for fall 2021 is in `college/` [read].
- **Selectivity:**
  - IPEDS ADM2023 [computed]: of 1,949 institutions reporting applicants, the median admit rate is 78%; 16.3% admit under half and 5.4% under a quarter.
  - Digest 305.40 [read]: 25% of 4-year institutions have open admission; 92% of 2-year institutions have no application criteria.

### 9d. Enrollment mix

- **2-year share:**

  | Year | Share | Of |
  |---|---|---|
  | 1939–40 | 10% | all enrollment |
  | 1963 | 17.8% | all enrollment |
  | 1969 | 25.8% | all enrollment |
  | 1970 | 31.5% | undergraduates |
  | 1980 | 43.2% | undergraduates |
  | 2000 | 45.2% | undergraduates |
  | 2010 | 42.5% | undergraduates |
  | 2023 | 29.8% | undergraduates |

  Sources: 120 Years Table 24; Digest 303.70 [read]. Part of the post-2010 fall is community colleges reclassified as 4-year.
- **Public share of students:** 49% (1947), 74% (1969), 78% (1980), 73% (2023). The for-profit share peaked at 9.6% in 2010.
- **Part-time share:** 32% (1970), 41–43% (1980–2000), 39% (2023).

### 9e. Age of students

- **Share aged 25+, all levels** (Digest 303.40, 303.45) [read]: 27.8% (1970), 38.4% (1980), 44.1% (1990), 42.3% (2010), 33.6% (2021).
- **Undergraduates by age:**

  | Sector | Under 18 | 18–19 | 20–21 | 22–24 | 25+ |
  |---|---|---|---|---|---|
  | All, fall 2021 (Digest 303.50) [read] | 9.6 | 26.8 | 24.8 | 13.8 | 25.0 |
  | All, fall 2023 (EF2023B) [computed] | 11.8 | 27.7 | 23.7 | 13.0 | 23.8 |
  | Public 4-year, 2021 | 7.4 | 29.4 | 29.8 | 15.6 | 17.8 |
  | Public 2-year, 2021 | 18.2 | 23.6 | 15.2 | 11.5 | 31.5 |
  | Private nonprofit 4-year, 2021 | 4.4 | 30.8 | 31.4 | 12.5 | 20.9 |
  | For-profit 4-year, 2021 | 0.5 | 5.6 | 8.7 | 12.6 | 72.5 |

- "Under 18" is mostly part-time dual enrollment of high-school students.
- Full-time undergraduates are 85% under 25.

### 9f. Distance from home to college

**NPSAS:20** (Hillman, TICAS brief 2, Oct 2023; domestic degree-seeking undergraduates; excludes for-profit and fully online institutions; straight-line miles) [read]:

| Sector | Median miles | Mean | Within 25 mi | Within 50 mi |
|---|---|---|---|---|
| Community college | 10 | 54 | 79% | 89% |
| Public bachelor's/master's | 13 | 82 | 67% | 79% |
| Public research | 39 | 145 | 42% | 54% |
| Private nonprofit bachelor's/master's | 58 | 285 | 36% | 47% |
| Private nonprofit research | 75 | 392 | 35% | 45% |
| All | 17 | 141 | 57% | 69% |

- **Median miles by NPSAS wave, all students:** 15 (2000), 20 (2004), 20 (2008), 13 (2012), 13 (2016), 19 (2018), 17 (2020).
  - Community colleges stay at 9–12 throughout.
- The top two income quintiles travel farthest, mostly to private nonprofits.
- **Other estimates:**
  - Hillman & Weichman 2016 (ACE), via Inside Higher Ed: medians of 8 / 18 / 46 mi (public 2-year / public 4-year / private nonprofit) [abs];
  - Mattern & Wyatt 2009, SAT takers of 1999: median 94 mi (p25 23, p75 230) [abs]. That is a far more mobile population.
- **Distance from a tract centre to the nearest institution** (population-weighted) [computed]:

  | Nearest institution | Median | p90 | Within 25 mi |
  |---|---|---|---|
  | any degree-granting | 3.8 mi | 16.3 mi | 96.0% |
  | public 4-year | 8.7 mi | 31.4 mi | 84.7% |
  | public 2-year | 11.6 mi | 69.7 mi | 71.2% |
  | public or private nonprofit 4-year | 5.4 mi | 23.1 mi | 91.2% |

  - IPEDS has one point per institution, so multi-campus community colleges make the 2-year figure too high.
  - Students mostly attend near their nearest options: the actual medians are 10 mi (community college) and 17 mi (overall).

### 9g. In-state attendance and housing

**In-state share of first-time degree-seeking undergraduates:**
- **Fall 2022** (Digest 309.10 and 309.20) [read]: 78.4% of US residents enroll in their home state; 77.7% of recent high-school graduates.
  - **Highest:** Utah 88.6%, California 87.5%.
  - **Lowest:** Vermont 45.4%, Alaska 48.7%, New Hampshire 49.9%; DC 21.9%.
- **By sector, from IPEDS EF2022C** [computed]:

  | Sector | In-state share |
  |---|---|
  | public 2-year | 94.2% |
  | public 4-year | 79.3% |
  | private nonprofit 4-year | 45.3% |
  | for-profit 4-year | 38.7% |
  | all | 75.7% (78.8% of US residents; 3.0% foreign) |

**Living arrangements:**
- **NPSAS:16, all undergraduates** (AACC DataPoints, ERIC ED606186) [read]:

  | Sector | On campus | Off campus, not with parents | With parents |
  |---|---|---|---|
  | All | 15.6 | 56.9 | 27.5 |
  | Public 4-year | about 29 | 52.3 | 18.8 |
  | Private nonprofit 4-year | 43.1 | 43.8 | 13.1 |
  | Public 2-year | 1.5 | about 59 | 39.4 |

- **ACS 2023, undergraduates aged 18–24** [computed]:

  | Arrangement | All | Public | Private |
  |---|---|---|---|
  | with parents (child of householder) | 45.2 | 48.6 | 34.0 |
  | group quarters (a dorm proxy) | 25.0 | 19.1 | 44.3 |
  | own household | 15.0 | 16.2 | 11.0 |
  | roommates | 9.3 | 10.0 | 7.0 |
  | other | 5.5 | 6.1 | 3.7 |

  - **In group quarters by age:** 47% at 18, 38% at 19, 23% at 20, 17% at 21, 10% at 22, 3% at 24.
  - **With parents:** 43–47% at every age.
  - Caveat: students interviewed at home in summer show as living with parents.
- **History, ages 19–20** (Kim & Rury 2011, IPUMS) [abs]: in dorms or group quarters, over 40% (1960) and slightly under a third (1980); with parents, about 35% (1960) and 47% (1980).
- **Census college housing:** 2,521,090 (2010) and 2,792,097 (2020; not lower) [read]. ACS 2023 counts 2.88M undergraduates in non-institutional group quarters [computed].
  - Earlier counts: 2000 about 2.06M, 1990 1.95M [not verified].
- **Dorm beds** (IPEDS IC2023 `ROOMCAP`) [computed]: 3.21M in total.

  | Sector | Institutions with housing | Beds | Beds per full-time undergraduate |
  |---|---|---|---|
  | public 4-year | 74% | 1.76M | 0.32 |
  | private nonprofit 4-year | 77% | 1.34M | 0.60 |
  | public 2-year | 26% | 72k | 0.05 |

### 9h. Completion, time to degree, graduate school

**Bachelor's at the first institution, first-time full-time 4-year students** (Digest 326.10) [read]:

| Entry cohort | 4 years | 5 years | 6 years |
|---|---|---|---|
| 1996, all | 33.7 | 50.2 | 55.4 |
| 2017, all | 49.1 | 61.7 | 64.5 |
| 2017, public | 46.1 | 60.1 | 63.4 |
| 2017, private nonprofit | 56.5 | 66.5 | 68.5 |
| 2017, for-profit | 26.4 | 30.5 | 32.3 |

- **2017 cohort, 6-year rate:**
  - by sex: men 60.9, women 67.5;
  - by group: Asian 78.3, White 68.1, Hispanic 58.6, Black 45.4, AIAN 41.3;
  - by selectivity: open admission 30.4, admit rate under 25% 92.5.
- **2-year institutions within 150% of normal time:** 30.5% (2000 cohort), 34.1% (2019).

**Time to the bachelor's** (NCES 2017-407, B&B trends) [read]:

| Measure | 1992–93 graduates | 1999–2000 | 2007–08 |
|---|---|---|---|
| Median months, all | 56 | 57 | 52 |
| Median months, private nonprofit | 47 | 46 | 45 |
| Within 48 months | 35.5% | 38.7% | 43.6% |
| Over 120 months | 13.9% | 14.4% | 11.6% |
| Started at a public 2-year | 16.0% | 20.2% | 19.7% |
| Aged 23 or under at the degree | 65.7% | 62.2% | 66.8% |
| Aged 30+ at the degree | 15.0% | 15.9% | 13.4% |

**Transfer** (National Student Clearinghouse, fall 2018 community-college entrants) [read]: 34.1% transferred to a 4-year institution within six years, and 18.0% of all starters earned a bachelor's within six years.

**Graduate school** [read]:
- Of 2007–08 bachelor's recipients, 44.1% had enrolled again by 2012 (men 40.5, women 46.8).
- Master's enrollment by the first follow-up: 11.6% (1993 graduates), 15.7% (2000), 18.5% (2008).
- **Postbaccalaureate enrollment:** 1.21M (1970), 2.16M (2000), 3.07M (2019). Women have been the majority since about 1990.

**Degrees conferred** (Digest 301.20) [read]:

| Year | Bachelor's | Women % | Master's | Doctor's |
|---|---|---|---|---|
| 1869–70 | 9,371 | 14.7 | — | 1 |
| 1899–1900 | 27,410 | 19.1 | 1,583 | 382 |
| 1929–30 | 122,484 | 39.9 | 14,969 | 2,299 |
| 1949–50 | 432,058 | 23.9 | 58,183 | 6,420 |
| 1969–70 | 792,316 | 43.1 | 213,589 | 59,486 |
| 1989–90 | 1,051,344 | 53.2 | 330,152 | 103,508 |
| 2020–21 | 2,066,445 | 58.3 | 866,894 | 194,059 |

- The doctor's column includes first-professional degrees from 1969–70.
- `tabn318.10` has every year.

### 9i. HBCUs, women's colleges, land-grant colleges

- **HBCUs:**
  - **Founding** (NCES 2004-062) [read]: 81 of 115 were founded before 1900, 21 of them in the 1860s.
  - **1915–1927** (NCES 84-308) [read]: Black colleges went from 30 (1915) to 77 (1927), with about 14,000 college-level students in 1927.
  - **HBCU share of Black students** (Digest 313.20 ÷ 306.10) [computed from read tables]: 18.4% (1976), 13.1% (2000), 9.4% (2022).
  - **In the states with HBCUs,** they enrolled 62% of Black full-time undergraduates in 1970.
  - **HBCUs in 2022:** 289,426 students, 76% Black, 64% women.
- **Women's colleges:** about 230–280 in the 1960s and about 30 now are commonly cited [not verified].
- **Land-grant colleges:** created by the Morrill Acts of 1862 and 1890; 19 Black land-grant institutions [read]. HD2024 flags 115 land-grant institutions [computed].

---

## 10. Downloadable data in hand

All paths are under `datasets/education/`. Every file is a US federal work (public domain) unless noted. NCES geocodes ask for acknowledgement.

### 10a. Locations and microdata (the data pass)

| Path | Size | Rows | Key columns | Notes |
|---|---|---|---|---|
| `edge/EDGE_GEOCODE_PUBLICSCH_2425.zip` (TXT, XLSX, SAS, shapefile) | 30.4 MB | 102,178 | NCESSCH, LEAID, NAME, STREET, CITY, STATE, ZIP, CNTY, LOCALE, LAT, LON, CBSA, CSA, CD, SLDL, SLDU | 2024–25; no grades or enrollment (join CCD on NCESSCH). The TXT has no header. |
| `edge/EDGE_GEOCODE_PUBLICLEA_2425.zip` | 8.2 MB | 19,636 | district office locations | |
| `edge/EDGE_GEOCODE_PRIVATESCH_2324.zip` | 6.5 MB | 22,510 | PPIN, NAME, address, LOCALE, LAT, LON | PSS 2023–24 respondents |
| `edge/EDGE_GEOCODE_POSTSECSCH_2425.zip` | 2.7 MB | 6,605 | UNITID, name, address, LAT, LON, CBSA | |
| `edge/docs/EDGE_GEOCODE_{PUBLIC,PSS}_TECHDOC.pdf` | 0.9 MB | | | |
| `ccd/ccd_sch_029_2425_w_1a_073025.zip` (directory) | 13.4 MB | 102,178 | SCH_NAME, LEAID, addresses, SY_STATUS, SCH_TYPE, CHARTER_TEXT, G_PK…G_12_OFFERED, GSLO, GSHI, LEVEL | 65 columns |
| `ccd/ccd_sch_052_2425_l_1a_073025.zip` (membership) | 212.7 MB (2.35 GB CSV) | 11.17M | NCESSCH, GRADE, RACE_ETHNICITY, SEX, STUDENT_COUNT, TOTAL_INDICATOR | long format; deflate64 (use `unzip -p`) |
| `ccd/ccd_sch_059_2425_l_1a_073025.zip` (staff) | 5.9 MB | 100,237 | TEACHERS (FTE) | |
| `ccd/ccd_sch_129_2425_w_1a_073025.zip` (characteristics) | 5.6 MB | 100,237 | VIRTUAL, NSLP_STATUS, SHARED_TIME | |
| `ccd/derived_membership_{total,by_grade}_2425.csv` | 2.7 / 18.2 MB | 99,420 / 679,627 | NCESSCH, (GRADE,) STUDENT_COUNT | made by `membership.py` |
| `pss/pss2324_pu_csv.zip` → `pss2324_pu.csv` | 4.0 MB / 39.5 MB | 22,510 | PFNLWT and 88 replicate weights, NUMSTUDS, LEVEL, RELIG, ORIENT, TYPOLOGY, ULOCALE24, LATITUDE24, LONGITUDE24, grade span, teachers | 359 columns; `codebook2023_24.pdf`, `layout2023_24.pdf` |
| `ipeds/HD2024.csv` | 4.5 MB | 6,072 | UNITID, INSTNM, address, SECTOR, ICLEVEL, CONTROL, DEGGRANT, HBCU, TRIBAL, LANDGRNT, LOCALE, C21BASIC, INSTSIZE, LATITUDE, LONGITUD, CBSA, COUNTYCD | 72 columns |
| `ipeds/ef2023a.csv` | 27.3 MB | 115,156 | UNITID, EFALEVEL, LINE, LSTUDY, EFTOTLT and counts by race × sex | fall 2023. Codes are space-padded. |
| `ipeds/ef2023b.csv` | 12.6 MB | 158,917 | UNITID, EFBAGE (age band), LSTUDY, EFAGE01–09 | fall 2023, by age |
| `ipeds/ef2022c.csv` | 2.0 MB | 71,394 | UNITID, EFCSTATE, EFRES01, EFRES02 | residence of first-time students, fall 2022 (collected in even years) |
| `ipeds/IC2023.csv`, `IC2024.csv` | 1.5 MB each | 6,049 / 5,963 | ROOM, ROOMCAP, BOARD (2023); calendar, offerings | |
| `ipeds/EFFY2024.csv` | 27.7 MB | 115,026 | 12-month enrollment | |
| `ipeds/adm2023.csv` | 0.3 MB | 1,972 | APPLCN, ADMSSN, ENRLT, test scores | |
| `ipeds/*_dict.xlsx` | | | variable dictionaries | |
| `sabs/SABS_1516.zip` (+ unzipped `SABS_1516/`) | 584 MB (1.25 GB) | 75,128 polygons | ncessch, level, gslo, gshi, defacto, openEnroll, MultiBdy | EPSG:3857; `EDGE_SABS_2015_2016_TECHDOC.pdf` |

- **More years** are on the same pages: EDGE geocodes 2015–16 to 2024–25, CCD 1986– (via `https://nces.ed.gov/ccd/datatables/api/File/2/7/<yearId>/0/0/0`), PSS 1989–, IPEDS 1980–.
- **Not yet published (404):** IPEDS EF2024 and ADM2024.

### 10b. Tables and reports (from the five passes)

| Folder | Contents |
|---|---|
| `enrollment/` | 120 Years (NCES 93-442); Statistical Abstract 1943 education section; 1910 census school-attendance abstract (scan); HSUS 1975 chapter H (scan, no text layer); CPS historical enrollment A-1 to A-7; Digest 103.10/.20/.30, 201.10, 202.10 (d19), 202.20, 219.10/.50/.55/.57/.70/.71/.73, 225.90; State Education Reforms Tables 5.1, 5.3, 5.14 (HTML); NCES 2010-015 Table 17a |
| `attainment/` | CPS A-1, A-2, A-4; CPS Table 1 for 2004 / 2014 / 2024 and Table 3 (2024); 1940 P-10 No. 8 and 1947 P-20 No. 15 (scans); Digest 104.10/.20/.40, 219.10, 219.60 (d15, d22); CPS foreign-born Tables 1.5/2.5/3.5 (2023); B&B:16/17 |
| `transmission/` | GSS 1972–2024 (Stata, 598 MB unzipped; NORC terms); CPS ASEC 2023 microdata (150 MB); Census FG3 / UC3 2023; Schwartz & Mare 2005 cells (no licence stated); NCES 2017-437 and 2018-421; `scripts/` |
| `schools/` | Digest 105.30/.40, 201.10, 203.10, 205.10–.80, 206.10–.60 (d22), 208.20, 209.10/.20/.30, 214.10–.40, 216.10 (d17, d23), 216.20–.90; 120 Years; Biennial Survey 1937–38 (high schools); NCES 2010-004; SABS technical documentation; NTPS 2017–18 Table 6a; NHTS and Safe Routes travel-to-school reports (McDonald 2011 is an author's copy, Elsevier) |
| `college/` | Digest 301.20, 302.10/.20/.60, 303.10/.25/.40/.45/.50/.70/.80, 305.30/.40, 306.10/.20, 309.10/.20, 313.10/.20, 317.10/.40, 318.10, 326.10/.20, 330.10, 104.10/.20; 120 Years; B&B trend reports (NCES 2017-407, 2017-438); HBCU reports (NCES 2004-062, 84-308); AACC DataPoints (ED606186); TICAS Hillman briefs 1–2 (cited, not federal) |

### 10c. Not obtained

- **Behind a key or refused:**
  - the Census API (ACS B15003 and 2000 SF1) now requires a key;
  - BLS refuses curl, though WebFetch worked;
  - RAND MG-139 returned 403;
  - the ECS kindergarten report is gone (410);
  - the Springer and PNAS PDFs were blocked;
  - Feigenbaum 2018 was unreachable.
- **Would help, but needs an account:**
  - an IPUMS USA extract (free, citation required, no redistribution) for native-born attainment by cohort, sex and race, 1940–2000;
  - BPS / NPSAS PowerStats for first-time students' distance to college.
- **Not digitized:** the state list of first compulsory laws (Steinhilber & Sokolowski 1966, Office of Education Circular 793).

---

## Targets to check

The most binding, as tolerance-banded tests.

| # | Target | Value | Source |
|---|---|---|---|
| 1 | Enrolled at 5–19, by era and race | 47% (1850; Black 2%), 50.5% (1900; Black 31%), 75% (1940), 79% (1950) | 120 Years Table 2 |
| 2 | Enrolled by single age, 1910 → 1940 | at 7: 75 → 92%; at 14: 81 → 93%; at 16: 51 → 76%; at 17: 35 → 61% | Stat. Abstract 1943 No. 216 |
| 3 | Enrolled by age group, 1950 → 2022 | 14–17: 84 → 95%; 18–19: 29 → 64%; 20–24: 9 → 37% | Digest 103.20 |
| 4 | 5-year-olds in kindergarten | 16% (1910), 29% (1930), 67% (1970), 82% (1980) | 120 Years Table 10; Digest 202.10 |
| 5 | Grade for age | 6-year-olds in grade 1+: 96% (1968) → 84% (2005); 18–19 still in high school: 10.5% (1970–80) → 18–19% (2010s) | Deming & Dynarski 2008; CPS A-5b |
| 6 | Diplomas per 100 seventeen-year-olds | 6.4 (1900), 29 (1930), 51 (1940), 77 (1970), 86 (2023) | Digest 219.10 |
| 7 | HS+ / BA+ at 25–29 by cohort | 1910–14: 38 / 6%; 1940–44: 75 / 16%; 1970–74: 88 / 29%; 1994–98: 95 / 40% | CPS A-2 |
| 8 | Final BA+ (at 40–49), men / women | 29.7 / 28.6% (born 1954–63) → 38.1 / 45.8% (1974–83) | CPS Table 1 (2004, 2024) |
| 9 | Pre-1910 cohorts | median 7.5 years and HS+ 13% (born ≤ 1874); mean years at 30: 7.3 (1876) → 13.2 (1951) → 14.3 (1987) | 1940 census; Autor, Goldin & Katz 2020 |
| 10 | Black / White BA+ at 25–29 | 1.6 / 6.4% (1940), 11.6 / 23.7% (1980), 29.9 / 45.3% (2024) | A-2; Digest 104.20 |
| 11 | Immigrants, 25–64 | < HS 22.9% against 6.5% for natives; graduate 16.8 against 14.0% | ACS 2023 [computed]; CPS 2023 |
| 12 | Parent–child schooling | r ≈ 0.46–0.54 in years, latent ≈ 0.55, every cohort; P(BA \| a BA parent / none) = 70 / 26% | Hertz 2007; GSS [computed]; Pew 2021 |
| 13 | Siblings | r ≈ 0.60 in years | Mazumder 2004 |
| 14 | Spouses | r ≈ 0.56–0.64 in years (latent 0.65–0.73), 1940–2023; same level (5 levels, ACS) 43% against 22% random; wife higher 31%, husband higher 26% | Schwartz–Mare cells; CPS and ACS 2023 [computed] |
| 15 | Public school size | median elementary 417, middle 529, high 548 (student-weighted 511 / 721 / 1,438); rural median 290–301 | CCD 2024–25 [computed] |
| 16 | Students per grade per school | elementary median 63–67 (p90 117–122); high-school grade student-weighted median 350–375 | CCD 2024–25 [computed] |
| 17 | Class size | primary 20.9, secondary departmentalized 23–25; pupil/teacher 34–37 (1870–1910) → 22 (1970) → 15–16 | NTPS 2017–18; 120 Years; Digest 208.20 |
| 18 | Schools and districts over time | one-teacher schools 80% of schools (1910), 50% (1940), 2% (1971); districts 117k (1940) → 13.3k | Digest 214.10 |
| 19 | Private share of K–12 | 8% (1900–1920), 13.9% peak (1959), 9.7% (2019–24); Catholic 5.6M (1964) → 1.8M | Digest 105.30; PSS |
| 20 | Assigned school | about 81% of public students; city 60% against town 84% of K–12 | Digest 206.20 |
| 21 | Attendance-zone population | median: elementary 6,900, middle 21,000, high 29,000; about half of people in tracts split across elementary zones | SABS 2015–16 [computed] |
| 22 | 18–24 enrolled in college | 25.7% (1970) → 41.2% (2010) → 39.0% (2022); enrollment ÷ population 18–24 1.3 (1870) → 9.1 (1940) → 35 (1969) | Digest 302.60; 120 Years Table 24 |
| 23 | Distance to college | median 17 mi (community college 10, public research 39, private nonprofit 58–75); 69% within 50 mi | NPSAS:20 (Hillman 2023) |
| 24 | In-state first-time students | 78.4%; public 2-year 94%, public 4-year 79%, private nonprofit 45% | Digest 309.10; EF2022C [computed] |
| 25 | Undergraduates 18–24 in group quarters | 25% (47% at 18, 10% at 22); with parents about 45% | ACS 2023 [computed]; NPSAS:16 on campus 15.6% |
| 26 | Age at bachelor's | 63–67% at 23 or younger, 13–16% at 30+; median 52 months to degree | B&B 1993–2017 |

---

## Open questions

1. **Representation of attainment.** The transmission pass recommends a latent normal:
   - each person gets z = ρ · (parents' mean z) + a sibling-shared family term + noise, with ρ ≈ 0.55;
   - z is cut at thresholds per (cohort, sex, heritage, nativity) to hit the margins;
   - this is one draw per person, so a pure f(seed, id).
   Known misses: associate-degree stickiness, a fatter upper tail for children of parents without high school, and weaker Black transmission after 1930. Accept or patch?
2. **Education in partnering.** About three-quarters of the spouse correlation must come from education itself. Options:
   - (a) a z band as a market dimension, with an affinity like the existing status affinity;
   - (b) permute partners within each ledger cell to sort on z. This keeps counts, but cells may be too small to reach r ≈ 0.7; measure it;
   - (c) meeting at school or college as a union venue.
   Which, and is a lower same-sex homogamy modelled?
3. **Sub-tract catchments.** Half of people live in tracts that straddle elementary zones, and 13% of elementary schools are nearest to no tract centre.
   - Partition each dense tract's households into several catchments (keyed by address), or accept tract-level catchments (one elementary school per 1–3 tracts) and miss the dense-city schools?
   - Middle and high zones follow tract lines well enough (75–82% unsplit).
4. **Catchment rule.** Nearest-school (Voronoi) partitions over- or under-fill schools by about 2×. Options:
   - a capacity-constrained assignment (RTI: least-full of the 3 nearest);
   - a balanced partition (SABS-like sizes);
   - district boundaries wherever a district has one school per grade (60% of districts, 10–13% of people).
5. **Native-born cohort targets.** Every CPS and census cohort value includes immigrants. Fetch an IPUMS USA extract (free account, citation licence, no redistribution) for native-born attainment and mean years by cohort, sex and race, 1940–2000?
6. **Old cohorts.** 1940 and 1947 values for cohorts born before 1900 read high (survivors, over-reporting). Calibrate to the earliest observation?
7. **Heritage before 1970.** No Hispanic, Asian or AIAN series exist before about 1970 (1940 counts Mexicans as white; 1947 has only white and nonwhite). Back-cast from 1940 "other races" and foreign-born attainment, or start group differences in 1970?
8. **The GED stock** disagrees across sources: CPS 2.6% of 25+, NLSY97 7.8% at 39, ACS 12–21% of the HS-only group. Use ACS?
9. **Expected grade and cutoffs.** Model each state's cutoff over time (an earlier mean cutoff, 25 November → 14 October over 1970–90, then about 1 September), plus redshirting (4–6%) and retention as separate hazards?
   - The data mix them; CPS A-3 gives the combined "below modal grade" share as a check.
10. **Attendance before 1940.** Calendars could carry the term length (132 days in 1870, 179 since 1960) and the average-day attendance rate (59% → 93%). Is that in scope for the calendar service?
11. **College choice targets.** NPSAS medians cover all enrolled undergraduates. First-time 4-year entrants travel farther (SAT takers: median 94 mi). Calibrate the choice model to all students, or to entrants (BPS would settle it with an account)?
12. **Pre-1970 college rates.** Only enrollment ÷ population 18–24 exists, inflated by older students (28% were 25+ in 1970). Census 1940–1960 enrollment by age would convert it; not fetched.
13. **Survey disagreements to resolve before writing tests:**
    - ACS against CPS spouse homogamy (54.8% against 59.8% same 4-level);
    - Digest 206.20's corrupted 2016 cells and odd 2019 rural private share;
    - Digest 104.20 against A-2 for young Black adults around 1960–70.
