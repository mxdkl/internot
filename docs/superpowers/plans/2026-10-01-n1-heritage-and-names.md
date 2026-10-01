# N1: heritage and names

**Date:** 2026-10-01
**Spec:** `specs/2026-09-29-society-as-a-function.md` §6.3 (surnames) and §11.3 (names).
**Research:** `research/2026-10-01-heritage-and-names.md` (composition, immigrant origins, intermarriage, naming practice).
**Founder decisions:**
- names come from public-domain sources only: SSA and Census (2026-09-30);
- heritage goes in the ledger (2026-10-01);
- exact data over smaller files (2026-10-01).

## 0. Why heritage comes first

First names and surnames depend strongly on race and Hispanic origin. For a family's names to make sense, partners must mostly share a heritage, at real intermarriage rates by era.

The ledger decides who partners with whom, and it doesn't know heritage. Adding heritage on top of a heritage-blind ledger can't work. Any labelling of the resulting union graph either mixes families at random within a few generations or lets one label take over. So heritage becomes a dimension of the ledger's blocks.

## 1. What must hold

1. **Every R1 to R1d guarantee**, checked by the exhaustive tiny suite: reciprocity, closure per (year, kind, class, block, block, sex), duality, life bounds, and zero close kin.
2. **Heritage is exact and inherited.** Every person has one heritage group, the mother's (as with regions, D-R1.1). Founders and immigrants have their block's.
3. **Names are pure functions.**
   - `first_name(x)` depends on `(seed, x)`.
   - `surname(x, t)` depends on `(seed, x, t)` through kin lookups. It changes only at a birth, a union or a separation.
4. **Realism:**
   - composition by heritage over time (drift from the Census is accepted, §8);
   - intermarriage by era, group and sex;
   - first-name frequencies by year, sex and group;
   - surname frequencies;
   - name changes at marriage by era.
5. **Speed:**
   - the world build stays near today's 2.4 s, using sparse market rounding (§3);
   - name lookups are measured and budgeted from measurement (pragmatic, founder 2026-09-30).

## 2. Heritage groups

Five groups, following the Census 2020 name files' columns:
- non-Hispanic White;
- non-Hispanic Black;
- non-Hispanic American Indian and Alaska Native (AIAN);
- non-Hispanic Asian and Pacific Islander;
- Hispanic (any race).

"Two or more races" is not a group. It describes people whose parents differ, and their names draw on both parents (§6).

**Lineage groups** are region × heritage, so a block is (birth year, region, heritage):
- `Block` gets `group`, `region` and `heritage`;
- block index = (year − first) × G + group, with group = region × H + heritage;
- the world exposes `region(x)` (geographic, as before) and `heritage(x)`.
- Roommate frames stay per region and mix heritages.

**Founders** (alive at y0 = 1840): each region's founders split by the 1840 heritage mix (research note §1).

**Immigrants:** each year's arrivals split by region weight × that era's heritage mix of arrivals (research note §2: DHS region of last residence, mapped to groups). Couples who arrive together share a heritage.

**Children** join their mother's group.
- **Debt:** children of mixed couples always take the mother's group in the ledger. In reality, how such children identify depends on the pairing and the era (research note §4).
- The realism cost is measured as the share of people whose ledger group differs from their father's.

**Same rates for everyone (debt).** Groups share fertility, mortality and union schedules. In reality they differ, for example in fertility, mortality and age at first union. Composition therefore comes only from founders, immigration and intermarriage, and it will drift. The report measures the drift against the Census.

## 3. Markets with heritage

Each year, a block's wants (never partnered, and divorced) split as follows:
- **Same-sex:** a share σ(t) of first-union wants. Of that, ω(t, h, s) goes to the sex's open same-sex market and the rest to the sex's heritage-h same-sex market.
- **Opposite-sex**, the rest:
  - **open:** a share ω(t, h, s) goes to one open market across all groups;
  - **national:** of the remainder, ρ(t) goes to the heritage-h national market across regions;
  - **local:** the rest goes to the (region, heritage) market.

The open market's kernel is heritage-blind (age gap and status only). So an open seeker of group h meets group h′ in proportion to h′'s supply there. Group h's outmarriage is then about ω(t, h, s) × (1 − h's share of the open market's other side).
- A small group outmarries more at the same ω, as in reality.
- Sex asymmetries (Black men against Black women; Asian women against Asian men) come from ω by sex.

ω is calibrated to the research note's intermarriage series: newlyweds by group and sex since 1967, the IPUMS historical rates before that, and the cohabiting share above the married one.

**Sparse rounding** (needed before more groups; it also retires the national market's regions² rounding debt).
- Today every market rounds every (row, column) cell with a keyed Bernoulli, which is O(rows × cols).
- Within a market, `x_ij = s_i · G[g_i][g_j] · c_j`: row seeker i's share of its (birth year, status) group's margin, times the IPF fixed point.
- So each row group g gets one cumulative weight over columns, `W_g[j]`. Row i places `floor` or `ceil` of its total by keyed systematic sampling: an offset `u`, then points `u, u + 1, ...` in units of `s_i`, each found by binary search in `W_g`.
- Each cell keeps its exact expectation, and row totals vary less than before. The cost is O(groups · cols + couples · log cols).
- Matrices become sparse rows (CSR). Caps and trims work on the same entries in the same order.

## 4. Name data

The distillate is `internot_society/data/names.bin`, about 7.8 MB, exact. It comes from `internot_society/data/distill_names.py`, whose docstring gives the sources, the cleaning and the format. The raw files stay in the git-ignored `datasets/names/`.
- **First names:** 113,282, the union of SSA and Census.
  - **SSA:** births by (name, sex, year), 1880 to 2025.
  - **Census 2020:** counts by the six race/Hispanic columns and by sex, and the "all other names" tail.
- **Surnames:** 156,621 Census 2020 surnames with the six counts, the tail, and a display form (O'Brien, McDonald, De La Cruz, St. John).
- **Cleaning,** each rule from an observed sample:
  - SSA placeholders for unnamed infants are dropped;
  - Census artifacts ("REF", "HIM") and surnames in the first-name field are dropped;
  - Spanish compounds are split back ("Maria del Pilar").

The data loads once per process: `include_bytes!`, decoded into a `OnceLock`, and shared by every world.

## 5. First names

**US-born** (natives, and immigrants born in-world are none):
- `P(name | year, sex, h) ∝ SSA(name | year, sex) · P(h | name, year)`.
- `P(h | name, year)` takes Census 2020's `P(h | name)` and shifts its odds by the group's share of births in that year: `∝ P(h | name) · π_h(year) / π_h(2020)`.
- Names absent from Census 2020 take `π_h(year)`.

Why the shift: the plain lift `SSA × P(h | name) / P(h)` gives Hispanic boys born 1910 mostly John and Frank. A name used only by one group then deserves weight `1 / π_h(year)`, not `1 / π_h(2020)`. Checked on samples:

| Year, sex, group | Top names |
|---|---|
| 1910, boys, Hispanic | Jose, Oscar, Manuel, Juan |
| 1910, boys, Black | Willie, James, Clarence |
| 1980, girls, Black | Tiffany, Angela, Crystal, Latoya |
| 2010, girls, Black | Nevaeh, Aaliyah, Jasmine |
| 1950, women, Asian | Linda, Mary, Erlinda |
| 2020, boys, Asian | Muhammad, Kai, Ethan, Ayaan |

**Years outside SSA:**
- births before 1880 use 1880 (founder decision);
- births after 2025 use 2025.
- **Debt:** names stop evolving after 2025.

**Immigrants** (named abroad): a mixture.
- With probability φ_h, a Census-only name of group h (names SSA never recorded for 5+ births in a year: Svitlana, Tigist, Venkata, Fiordaliza).
- Otherwise, the US-born formula at their birth year.
- φ_h = m_h / f_h, where m_h is group h's Census-only mass and f_h is group h's foreign-born share in 2020. So the Census-only names land on the foreign-born at their 2020 rate.
- The unlisted tail (24% of Asian Americans, 15% of Black Americans) can't be drawn, so foreign-looking names are under-represented. This is measured, not fixed.

**Sex:** SSA tables are by sex. Census-only names use Census `P(sex | name)`.

**Naming heritage of a child of a mixed couple:** the mother's or the father's group, by a keyed coin per child. A non-union birth uses the mother's.

**Sampling tables:**
- per (year, sex, group) cumulative weights over that year's SSA names, built lazily and cached per process (`OnceLock` per table);
- worst case about 52 MB if every table is touched (u32 cumulative weights, shared name arrays);
- measured, and compressed later if memory matters.

**Middle names:** decided after the research note's §5.

## 6. Surnames

**Drawn** for founders and immigrants from `P(surname | h)`, using Census 2020 counts by group.
- The unlisted tail (12% of people) is spread over the rarest listed names in proportion, so a person with an unlisted surname looks like one with a rare listed surname.

**Inherited.** A child's birth surname is the father's surname at the birth with probability p_f(era, union birth), otherwise the mother's.
- Non-union births take the mother's.
- Hyphenated names: decided after the research note §3.

**Changes at a union** (statistics by era, research note §1 and §2). Each union draws once whether one partner takes the other's surname:
- the woman takes the man's (by era);
- the man takes the woman's (small);
- in same-sex unions, one takes the other's at a lower rate;
- otherwise neither does.

The changer takes the other's surname as it was just before the union. That is determined by strictly earlier events, so no cycles.

**After a separation:** revert to the birth surname with probability r(era), or keep it. A later union can change it again.

**Cost:** a surname walks up the patriline to a founder or immigrant (up to about eight generations), plus partner lookups for changes. It is measured and budgeted from measurement.

## 7. API

- `World::heritage(x) -> Heritage`, `World::region(x)` (geographic).
- `World::first_name(x) -> &'static str`.
- `World::birth_surname(x)`, `World::surname(x, t)`.
- `World::name(x, t)`, the display name.

## 8. Tests and reports

**Exhaustive tiny world** (heritage boost, like the same-sex boost, so cross-group couples and every group occur):
- every existing kinship and household test;
- children in the mother's group;
- cross-group couples exist;
- blocks of size 0 behave.

**Names:**
- determinism;
- siblings with the same father and the same surname rule share a birth surname;
- a woman's surname at t matches her partner's whenever her union drew "takes";
- no surname changes without an event.

**Realism report** (`examples/names_report.rs`):
- composition by year against the Census;
- intermarriage by era, group and sex against the targets;
- top names by decade and group against SSA × Census;
- share of women with the husband's surname by era;
- distinct names per 1,000 people (non-repetition);
- full-name collisions against expectations.

**Perf:**
- world build;
- peak memory;
- `first_name`, `surname` and `name` p50/p99 over the full id space.

## 9. Order of work

1. Sparse rounding on today's two-region world. Tests pass and realism stays in its bands. Measure the build.
2. Heritage groups: params, blocks, markets, arrivals, founders. Tiny suite, then calibration of composition and intermarriage.
3. The names module: data loader, first names, surnames, changes. Tests, report, perf.
4. AGENTS.md, and the outcome below.

## Outcome

**Step 1, sparse rounding: DONE (2026-10-01).**
- `solve_market` returns sparse rows (`Sparse`, CSR). Rounding is keyed systematic per row over each row group's cumulative column weights.
- `apply_market` trims rows, then columns (by a counting sort of entries), with the same largest-first, lowest-index order as before.
- Every `internot_society` test passes.
- **Realism** (two regions): every cohort figure is within sampling noise of the old rounding:
  - CFR 2.43 against 2.47 for the 1950 cohort, 2.05 against 2.02 for 1970;
  - re-partnering within 10 years 72% against 71%;
  - same-sex couples 1.70% against 1.65%;
  - foreign-born shares within 0.2 points.

  The ledger is no longer bit-identical to before, as expected.
- **Build time:**
  - two regions: 3.30 s → 3.14 s in the realism report (the dense rounding was cheap at two groups);
  - 10 lineage groups with uniform mixing (the worst case): 10.4 s → 6.0 s, peak RSS 2.56 GB.
  - The rest of the 10-group cost is more, smaller cells and slices. Realistic endogamy should give fewer cross-group slices than uniform mixing.

**Step 2, heritage in the ledger: DONE (2026-10-01).** The world packs landed in between (`specs/2026-10-01-world-packs.md`), so heritage is pack-defined.
- **Groups and markets:**
  - Lineage groups are region × heritage, with five groups in `worlds/us/heritage.ron`.
  - Markets: local (region and heritage), national (heritage), and one open market across all groups. Same-sex markets are per heritage plus an open one, merged per sex.
- **Founders:**
  - The mix (1840 composition) is the composition of the living.
  - Each group's births are scaled by base over group survivors, so a high-mortality group starts from more births. Without that, Black founders came out 10% instead of 15%.
- **Immigrants:** the arrival mix by decade comes from DHS, corrected for unrecorded Mexican arrivals, Puerto Rican migration, unauthorized arrivals and IRCA timing. Groups can set their own immigrant sex ratio (Asian before 1920: about 95% men).
- **Group rates** (all in the pack, relative to the base schedules):
  - **Fertility factor** (TFR over the population average): it tilts the parity distribution, so higher fertility also means less childlessness, and it scales non-union births.
  - **Mortality factors,** infant and adult:
    - infant factors come from infant-mortality ratios;
    - adult factors are solved by `examples/calibrate_group_mortality.rs` to reproduce each year's e0 gap to White. Death-rate ratios weight old ages and understate the gaps, giving 4 years where the data says 14.6 in 1900.
- **Bit-identity:** with neutral factors the worlds are bit-identical to before (fingerprint), so the mechanism adds nothing on its own.

**Calibrated results** (prototype; `examples/heritage_report.rs`):

| Measure | Model | Census or Pew |
|---|---|---|
| Composition 2020: White / Black / AIAN / Asian / Hispanic | 62.6 / 14.2 / 0.77 / 5.8 / 16.7 | 60.6 / 12.6 / 0.70 / 6.4 / 19.6 |
| Composition 1850 | 83.7 / 13.9 / 1.9 / 0 / 0.6 | 82.4 / 15.4 / 1.7 / 0 / 0.5 |
| Composition 1900 | 87.3 / 10.5 / 0.9 / 0.4 / 1.0 | 87.1 / 11.6 / 0.35 / 0.3 / 0.7 |
| Composition 1970–2000 | within about 2 points of the Census | |
| Foreign-born mix 2000 | 25.6 / 6.7 / 0.3 / 21.9 / 45.4 | 22.0 / 6.8 / 0.4 / 22.7 / 45.5 |
| Intermarriage of new unions, 2013–17, all | 16.5 | Pew 2015 newlyweds: 17 |
| White (women / men) | 12.0 / 11.3 | 10 / 12 |
| Black (women / men) | 11.1 / 20.9 | 12 / 24 |
| AIAN (women / men) | 56 / 48 | 61 / 54 |
| Asian (women / men) | 36.6 / 20.6 | 36 / 21 |
| Hispanic (women / men) | 26.9 / 24.4 | 28 / 26 |
| Intermarriage, 1978–82, all | 7.4 | 6.7 |
| Before 1960 | under 1–2% except Asian and AIAN | under 1–2% |

**Effects elsewhere:**
- **Cohort realism stays in band:** CFR 2.46 for the 1950 cohort and 1.92 for 1970 (was 2.02); e0 within about a year.
- **De-isolation moves rose** from 2.5% to 6.3% of couples: smaller lineage groups leave more couples alone in a cell.
- **Builds:** the ledger takes 4.5 s and the world 4.9 s (3.3 s with two groups).
- **Tests:** every test passes. The tiny world's cross-region bound was widened, since the open market mixes regions.

**Debt (measured or named):**
1. **Children join their mother's group.**
   - Hispanic is 16.7% against 19.6% in 2020.
   - In reality about 75% of children of Hispanic–White couples are reported Hispanic, while the model keeps only the half with a Hispanic mother.
   - The fix routes births of mixed cells to either parent's group, which is a parent-line change.
2. **Black is 1.3–1.9 points high from 1930 on.** Birth plans are conditional on survival, and the Census undercounted Black Americans by several percent in the mid-century counts.
3. **Same union and non-union rates for every group.** Black non-marital births are 69% in reality, against 26% for NH White.
4. **No generation effect in intermarriage.** In reality immigrants marry out at 15% and the US-born at about 40%.
5. **AIAN identity growth after 1960** (self-identification) is not modelled.
6. **The 1840–80 AIAN decline is a provisional crisis factor:** 1900 comes out 0.86% against 0.35%.

**Step 3, names: BUILT (2026-10-01).**

**Pack:**
- `worlds/us/names.ron`: columns, first names, middle names, surnames, every rule with its source.
- `worlds/us/data/names.bin`, version 2, which declares its columns (moved from `internot_society/data/`).
- `unions.ron` gains a `marriage` section.

**Module `internot_society::names`:**
- **`NameData`** decodes once per process per data file (hash cache). `NameTables` builds lazily per world.
- **First names:**
  - The year's SSA names are split across groups by `P(group | name)` from Census 2020, raked by IPF to the world's own births by group that year, so the world's names have SSA's frequencies overall.
  - A child of mixed parents is named in either parent's tradition (50/50).
  - Immigrants get a foreign Census-only name with a group share, otherwise a name of their birth year.
- **Middle names:** by cohort, group and sex (NC voter file). For men, sometimes the mother's surname.
- **Marriages:** `World::marriage_date(x, &union)`, a fact of the union seen the same from both sides.
  - A union marries at its start, or later if the couple stays together (exponential delay), by era.
  - Same-sex couples can marry once it is legal for them.
- **Surnames:**
  - **Drawn:** founders and immigrants draw by group, with the unlisted tail spread over rare names.
  - **Inherited:** father, mother or both by marital status at the birth, plus Spanish double surnames. Which parent is decided before any surname is computed, so the walk follows one line.
  - **Changed at weddings:** the woman takes, hyphenates or keeps by era, more keeping for older brides and Hispanic women; men and same-sex partners rarely take.
  - **Reverted:** after separations with a probability (unsourced).
- **API:** `first_name`, `first_name_id`, `middle_name`, `middle_name_of`, `birth_surname`, `surname(x, t)`, `surname_text`, `full_name(x, t)`, `naming_group`, `marriage_date`, `married_at`.

**Tests** (`tests/names.rs`, exhaustive on the tiny world, 7 tests):
- names are pure functions;
- everyone is named, and middle names differ from first names;
- marriages are the same from both sides and fall inside the union;
- surnames change only at weddings and separations;
- a wedding change is exactly the partner's surname as it was, or a hyphenation;
- children's surnames come from a parent (88%+ the father's when there is one);
- first names follow sex (cross-sex names at the data's own rate).

All 868 workspace tests pass.

**Realism** (prototype; `examples/names_report.rs`):
- **Top first names by decade, sex and group look right:**
  - 1900s Black boys: Willie, George, James, Clarence;
  - 1900s Hispanic boys: Oscar, Jose, Edgar, Manuel;
  - 1980s Black girls: Ashley, Tiffany, Jessica, Latoya;
  - 2010s Black girls: Aaliyah, Ava, London, Mia, Nevaeh;
  - 2010s Hispanic boys: Adrian, Gabriel, Sebastian, Angel;
  - 2010s Asian girls: Sophia, Isabella, Grace.
- **Married share of couples living together:**

  | Year | Model | Target |
  |---|---|---|
  | 1950 | 98.5 | — |
  | 1970 | 97.9 | about 98.8 |
  | 1990 | 91.3 | about 94.8 |
  | 2010 | 87.4 | about 88.6 |
  | 2023 | 86.5 | 86.7 (CPS) |

- **Married women sharing the husband's surname, 2023:** 81.2% (Pew 2023: 79% took it, 5% hyphenated).
- **Distinctness, 10,000 people alive in 2025:**
  - 2,942 distinct first names and 6,406 surnames;
  - top surnames Smith, Johnson, Williams, Brown, Miller, Jones, Martinez, Garcia;
  - 1.3% share a first and last name with someone in the sample.
- **Families read coherently.** A child born before the parents married carries the mother's surname, and later siblings carry the father's (Tyren Elijah Williams, 2011; George Quintin Smith, 2014). Mixed couples hyphenate ("Devuono-Vaka"). Double Hispanic surnames use the mother's birth line ("Soto-Sanchez").

**Speed** (prototype, single calls over uniform ids, µs):

| Lookup | p50 | p90 | p99 |
|---|---|---|---|
| `first_name` | 5.5 | 11 | 24 |
| `surname` | 51 | 156 | 286 |
| `full_name` | 59 | 168 | 300 |

A surname walks up one line of ancestors (father lookups) to a founder or immigrant. Fine for views; bulk listing would want a memo.

**Debt:**
1. **Asian names mix origins** ("Khadija Zhao"): the Census lumps every Asian origin together. Countries of origin with their own name tables would fix it (pack sections, later).
2. **No women's middle-name pool** (Ann, Marie, Lee, Lynn); there is no public-domain frequency source.
3. **Names stop evolving after 2025;** births before 1880 use 1880.
4. **SSA lists only names given to 5+ babies,** so unique names (common among Black births since the 1970s) are missing.
5. **The revert-after-separation rate (0.5) and same-sex naming are unsourced.**
6. **1990's married share is 3.5 points low.**
7. **The surname walk costs 50–300 µs** (memoize if bulk use needs it).
8. **No maiden name as middle name** (18% of women who took the husband's name).
