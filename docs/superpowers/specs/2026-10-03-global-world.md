# The global world: every country, place and people, as a function of (seed, id, t)

**Date:** 2026-10-03
**Status:** design, being built in phases (§13). Data sections are filled from the research notes as they land.
**Founder request (2026-10-03):**
- "every person should have a name, race, a place they live, job(s)/education(s), friends (or no friend), family, co workers, etc all as a function of time meaning changing over time all while being realistic";
- "these should be added by making those rust json files (easy to expand on)";
- "its should be all countries and cities and towns and races of people. extreme high fidelity";
- the goal: "implement that without sacrificing speed or memory footprint".

**Builds on:**
- the monotone world (`internot_society::mono`, AGENTS.md "The society world"), which stays the kinship engine;
- the cell world's groups, open market and life cells (`research/2026-10-02-cell-world-math.md` §16–17), carried over to the monotone world;
- the society spec's residence, education, work and tie mechanisms (`specs/2026-09-29-society-as-a-function.md` §8–12), unchanged in principle.

**Research behind it** (`docs/superpowers/research/`):
- `2026-10-03-global-demography-data.md`, `2026-10-03-global-places-data.md`, `2026-10-03-global-ethnicity-and-names-data.md`, `2026-10-03-global-education-and-work-data.md` (data sources, licenses);
- the 2026-09-29 notes on affiliation and friendship, work and organizations, time-consistent evolution; `2026-10-01-education-data-and-targets.md`; `2026-10-01-heritage-and-names.md`; the residence notes of 2026-10-01.

---

## 1. What has to hold

Every guarantee of the monotone world stays:
- **G1 determinism:** same pack, seed and query, same answer, whatever the query order, thread or cache state;
- **G2 two-sided consistency:** a child's mother lists the child; partners, co-workers, classmates and friends are mutual; `x ∈ roster(W, t) ⇔ employer(x, t) = W`;
- **G3 life bounds:** nobody acts before birth or after death; minors don't work; no close-kin couples beyond the measured debt;
- **G4 time:** any t is answerable directly; nothing depends on `now`.

New:
- **G5 coverage:** every country and territory of the pack, every settlement in its place data, every group in its group catalogue;
- **G6 realism:** each feature has targets (§12) checked by reports and property tests;
- **G7 speed:** the monotone world's lookups keep their latencies (the `society` gate); new per-person attributes cost microseconds; rosters milliseconds;
- **G8 memory:** nothing grows with population. Derived state is built per cell when first touched, so a session pays for the cells it uses: a US-only session stays near today's ~10 MB. Pack data (places, names) is read from compact binary files.

---

## 2. Principles added to the monotone world's rules

**P10. Cells.** A *cell* is (country, area, group): people who live their adult life in one area of one country and belong to one heritage group. Every per-cohort and per-year structure of the monotone world exists per cell. A cell's structures depend only on the pack and on integer flow counts that are themselves pure functions of the pack, never on another cell's built structures, so cells build independently and lazily.

**P11. Life cells, derived birth cells** (cell world §17.4). A person is indexed by the cell where they partner and bear children, their *life cell*. Their *birth cell* (where their mother lived) is derived through an exact mover map. Migration between areas and between countries is the same mechanism.

**P12. Counted residence units** (society spec §8). Inside an area, people live in residence units (a couple's union, an adult's single spell). Units form a dense counted space per area; static keyed coordinate systems map units to (neighbourhood, slot); moves switch the active system. Rosters of a place enumerate preimages.

**P13. Shared tables over per-cell tables.** Anything that can be indexed by a few numbers (mortality level, union-age median, an education distribution) is a table shared by every cell with the same numbers, not a table per cell.

**P14. Shared factors for correlations.** Correlations between people who are related by a shared object (spouses' education, siblings' traits) come from a factor keyed on that object (the union, the mother), so no market has to sort people by the trait.

**P15. Every statistic in a pack; every mechanism in code.** Countries, areas, groups, places, name tables, school systems, occupations and their rates are pack data (RON plus distilled binary files). Code holds mechanisms and structural bounds.

---

## 3. Ids and cells

**Pid.** `Pid { cell, y, i }`: life cell, birth year, index in the cell cohort's life order.

**External id** (u64, below 2⁵³ so JSON carries it): `cell` 15 bits (32,768 cells), `y − 1700` 9 bits (to 2211), `i` 29 bits (536M per cell cohort). The real world's largest cell cohort is about 20M births (a large Chinese or Indian province), so ids hold at real scale and to about ×25 of it. Larger worlds use 64-bit ids serialized as strings (later).

**Cell catalogue.** The pack lists countries; each country lists its areas (admin-1 regions or groupings of them, chosen so a marriage market is meaningful) and its groups. A country with one area and one group is one cell. The cell index is the position in the catalogue.

**Lazy building.** `World` holds the pack, shared tables and a slot per cell (`OnceLock`). A cell's kinship structures (the monotone world's tables) are built the first time any of its people is asked about: about 1–15 ms. Global counts (`count_alive` over a country or the world) use per-cell closed forms that need only the pack (sizes, class boundaries and thresholds computed on the fly), not the built structures.

---

## 4. Population (L1) across countries

**Births.** A country's births by year come from the pack (UN WPP from 1950, historical series before). Within a country, births split over its cells by apportionment, weighted by each cell's women of childbearing age and its group's fertility factor (as the cell world did for groups).

**Founders.** The world starts in a pack year (1840 by default). Everyone alive then is a founder, drawn from a stable population per country.

**Mortality.** By country, sex and year, from the pack's life tables (WPP 1950–2100; before 1950 a model life table at the pack's historical life expectancy). A cohort's death law is read off along its diagonal. Death ages come from quantile tables (`quantile::OctaveTable`); by P13, tables are shared across cells through a mortality index (country table family, e0 level), not built per cell.

**Unions, separation, births to mothers** stay as in the monotone world, with each country's parameters: union-age distribution (median by cohort, shape), ever-partnered share, age-gap kernel, dissolution by duration and era, births by mother's age and marital status.

**Groups** (cell world §16): each group is its own union space, plus an open market per (country, area) where partners meet regardless of group, at the pack's open shares by era and sex. Children take their mother's group (debt: mixed heritage is a label on the child, drawn from both parents, used by names and reports).

**Migration** (cell world §17.4, generalized):
- **Movers:** of a birth cell's adults, an exact Beatty share moves at the pack's rate, at a keyed age before a first union (v1), to another area of the country or to another country.
- **Destinations:** within a country, by area attractiveness; between countries, by the origin's sparse destination shares (UN bilateral migrant stocks), so Mexicans go mostly to the US and Algerians to France. A mover's group in the destination is the pack's mapping of (origin country, origin group) to the destination's catalogue (an Algerian in France is in France's North African group).
- **Life cell of a mover** = (destination country, destination area, mapped group). In the destination, in-movers are a run after the stayers, concatenated over origins in catalogue order (a sparse prefix), so a person ↔ life rank map is closed form both ways.
- v1 debts: one move per person before a first union; no couple or family migration; no return migration. Couples and families moving are v2 (the monotone world's couple moves, cell world stage 3).

---

## 5. Kinship (L2)

Unchanged mechanisms per cell. Added lookups, derived from existing ones: grandparents, grandchildren, aunts and uncles, cousins, nieces and nephews, in-laws. Re-partnering (open item 3 of the monotone world) comes before households need it.

---

## 6. Names

**Per culture, not per country.** A *naming tradition* holds: given names by sex and birth year (frequency tables), surnames, middle-name practice, surname rules (inheritance, order, change at marriage), script and Latin transliteration. A (country, group) uses one tradition, or a mixture by era. Data per tradition from national statistics offices where open; a regional pool otherwise (research note).

**Given names** are drawn by a keyed categorical draw from the tradition's table for the birth year and sex (`table::CumTable`), so a name is O(1).

**Surnames:**
- founders and in-movers from abroad draw from their tradition's surname table;
- a child's birth surname follows the tradition's rule (father's; father's then mother's; patronymic from the father's given name; the mother's when no father);
- marriage changes and reversions by era follow the tradition;
- the paternal line is walked through `father` until a founder, a mover from abroad or a child without a father: a few lookups per generation, memoized per call.

---

## 7. Residence (L4): places and units

**Place hierarchy per country:** country > area (marriage-market region) > settlement (city, town, village) > neighbourhood. Settlements come from the pack's place data with population weights by decade, 1840–2100. Neighbourhoods are procedural subdivisions of settlements (a few thousand people each).

**Units** (P12). In an area, a *unit* is a couple's union or an adult's single spell (from leaving home to a union, between unions, after the last). Units are counted: couples by their union index, single spells by (person, spell ordinal). Children live in their mother's unit (the father's when the mother is dead, then the nearest kin); a household is a unit and the people living in it.

**Addresses.** K static keyed coordinate systems per (area, entry decade) map units to (settlement, neighbourhood, slot), with slot counts proportional to population weights at the decade. A unit's move stream (hazard by the household head's age, era and duration) switches the active system; the destination draw is person-side, weighted by distance from the current address. A new unit starts near its source (leaving home: the parents'; a union: one partner's).

**Rosters.** `residents(neighbourhood N, t)`: preimages of N's slots under each system, kept if their active system at t is that one, then each unit's members at t. Time blocks by entry decade keep the preimages near t.

---

## 8. Education (L5)

**Attainment.** A latent score per person: `z = a·own + b·(mother's own + father's own)/√2 + c·u_union`. Here `own` is the person's key, the parents' terms give intergenerational correlation without recursion, and `u_union` is a factor shared with the person's first partner (P14), which gives educational assortative mating. The attainment level is the quantile of `z` in the (country, sex, cohort) attainment distribution (Wittgenstein Centre data).

**School career.** The country's system (entry age; durations of primary, lower and upper secondary; tertiary) and the attainment level give dated spells: enrolment, completion or dropout, with repetition by country and era.

**Institutions:**
- primary and secondary schools are procedural per settlement and neighbourhood (catchments), with sizes by country;
- universities and colleges are real institutions from the place data (open registries), with sizes;
- a pupil's school is their neighbourhood's at the school-year start;
- a tertiary student chooses among institutions by size, distance and the latent score.

**Classmates.** A school-grade's pupils at t are the catchment's residents of the cohort (L4 roster); class sections partition them by a keyed hash. Tertiary peers: the institution's cohort block (a static coordinate system over the country's cohort, filtered by "attends").

---

## 9. Work (L6)

**Careers** (society spec §10.2): a dated list of spells per person (employed, unemployed, out of the labour force, self-employed, retired), by a hazard walk from labour-force entry, with rates by country, sex, age, education and era (ILOSTAT, OECD tenure).

**Employers.** Establishments per (area, settlement, industry), sized by the country's establishment-size distribution, with open and close dates; firms group establishments. Large real employers are out of scope (real organizations are not modelled; names are procedural and collision-checked).

**Jobs.** At a spell's start, the person's job coordinate maps their unit (P12) onto (establishment, seat) through K static systems; acceptance by occupation fit, capacity at the start and commute distance. Occupation (ISCO) and industry (ISIC) follow the country's shares by education, sex and era.

**Co-workers.** `roster(W, t)` enumerates preimages of W's seats; the team and manager structure is derived from the roster (society spec §10.4).

---

## 10. Ties (L8)

Friendship on foci (`research/2026-09-29-affiliation-and-friendship.md` §8):
- foci: household, kin, school class and cohort, tertiary cohort, workplace and team, neighbourhood, interest groups, geographic buckets;
- a tie exists iff some shared focus's pair hash passes its probability (`p = 1 − exp(−κ η w_a w_b/(s − 1))`, sociability weights lognormal), so ties are symmetric and carry where they met;
- ties outlive their focus with a keyed lifetime;
- dense circles inside large foci give clustering; stratifying by age and sociability gives assortativity;
- "no friends" happens: low sociability and few foci give empty lists at the measured rates.

---

## 11. Packs

A global pack (`worlds/earth/`) holds:
- `world.ron`: timeline, scale, defaults;
- `countries/<iso3>.ron` per country: births, life tables or their index, union and fertility parameters, areas, groups, open shares, naming traditions, school system, labour-force rates; regional defaults fill what a country lacks (`extends` per field);
- `traditions/<id>.ron`: naming traditions;
- `data/*.bin`: distilled binary tables (places with population weights by decade, name frequencies, life tables), each produced by a script in `internot_society/data/` from open sources, with its sources and license in its header.

`worlds/us` stays as the one-country pack and the regression baseline.

---

## 12. Realism targets (to fill from the research notes)

- Population by country and year (WPP), within 2% from 1950.
- Composition by group, intermarriage by group and era where measured.
- Names: top names by year and country match their tables; surname distribution by country.
- Residence: share in the largest city, urban share by year (WUP), moves per year, distance to parents.
- Education: attainment by cohort and country (Wittgenstein), enrolment by age.
- Work: participation by age and sex, occupation shares (ILOSTAT), tenure.
- Ties: degree about 125–155 active ties, layers about 4/11/30/130, clustering 0.1–0.15, a few percent with no close friend.

---

## 13. Phases

Each phase ends only when its exactness tests, realism report and speed gate pass (the monotone world's lookups must not slow; memory per cell as budgeted).

1. **Cells in the monotone world** (one country): a population spec per cell instead of the US pack; lazily built cells; the US's five groups with the open market (§4). Gate: `us` with one group reproduces today's checksum; with groups, composition and intermarriage near the cell world's.
   - *Status 2026-10-03:* built (AGENTS.md "Cells"): the five groups and the open market, births by realized eligible mothers in 15-year waves, group mortality by scaling the country tables, flat layouts. The one-group world reproduced the checksum through every step but one deliberate change (merged late wife classes). Exhaustive tests pass on both worlds. Not yet: lazy cells (eager is fine for one country), and composition near the cell world's, which needs migration (Phase 4).
2. **Names** (US data in hand): given, middle and surnames, inheritance and marriage changes. Then the directory serves names and groups.
   - *Status 2026-10-03:* built (AGENTS.md "Names"), served by `read_person`; birthdays fixed on the way (siblings no longer share their mother's day).
3. **Residence in an area**: units, places from `places.bin` (US), moves, addresses, rosters.
   - *Status 2026-10-03:* households and forward residence built (AGENTS.md "Households", "Residence"), with mail addresses (ZIPs, postal places, streets) served by `read_person`. Rosters and realistic distances to kin wait for areas as cells; household realism waits for re-partnering.
4. **Countries**: the global pack (distillation scripts), cells per country, international movers.
5. **Education**: attainment, school careers, institutions, classmates.
   - *Status 2026-10-03:* attainment, timelines and institutions built for the US (AGENTS.md "Education"), served by `read_person`. Classmates (school rosters) wait for rosters; partners' education correlation is 0.43 against 0.65–0.73.
6. **Work**: careers, establishments, jobs, co-workers.
   - *Status 2026-10-04:* careers, occupations, titles, employers and pay built for the US (AGENTS.md "Work"), served by `read_person`. Establishments are procedural per (county, industry, size class) and have no open or close dates yet; co-workers wait on rosters (the areas decision).
7. **Ties**: foci, friendship, layers.
8. **Directory**: every feature in `read_person` and new views (household, place, workplace, school rosters).

---

## 14. Open decisions (founder)

- **Granularity of areas and groups per country** (cost per cell against fidelity): proposed default, admin-1 regions merged until each has at least ~1M people at real scale, and groups with at least ~0.5% of a country or a pack override.
- **Real organizations:** universities are real (public registries); employers are procedural. Real large employers could be added from open data with their real names.
