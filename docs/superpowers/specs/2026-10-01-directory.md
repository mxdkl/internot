# Directory: the society world through MCP (cutover from `people`)

**Date:** 2026-10-01. **Status:** v1 being built.

**Founder decision (2026-10-01):** the directory cutover comes next. Serve the new society world (`internot_society`) through the MCP tools, replacing the `people` service, whose relationships, time model and population are defective (AGENTS.md, "Known defects in `people`"). This is D6 (build alongside, then cut over) and the first of the D7 pilot services.

## 1. What changes

- **`Universe` holds one society world**, built once per process: `internot_society::World` plus `Residence` for addresses.
  - The pack is chosen by `INTERNOT_PACK`, default `us` (6 s, 1.5 GB). `us-areas` is the area-mode world (68 s, 5.6 GB). The seed is `INTERNOT_SEED`, default 42.
  - Tests use `Universe::for_pack("us-tiny", now)`.
- **A new `directory` service** with read-only views. Every answer is a pure function of `(seed, person id, now)`. The `at` parameter (ISO 8601) defaults to the Universe's `now`.
- **`people` is removed from `SERVICES`,** its module is deleted, and so is its name data (`internot/data/{first,last}_names.json`), whose provenance is the 2021 leak; new layers must not use it.
  - The individual attributes (personality, education, languages, hobbies, working hours) were reasonable but keyed to the old ids. They stay in git history, to re-key to the new ids when a service needs them.
- **The transport is unchanged:** tools appear from the registry.

## 2. Views (v1)

- **`read_person { person_id, at? }`**:
  - names at `at` (first, middle, surname, and birth surname if it changed);
  - sex, birth and death dates, age, alive or not;
  - heritage, and for immigrants the arrival date;
  - the area of upbringing (area mode);
  - the household at `at`: kind, members with names and relationship to the person;
  - the address at `at`: tract, county, commuting zone, area, coordinates;
  - the partner at `at`;
  - every union: partner, start, end, how it ended, marriage date;
  - parents, children and siblings, with names and whether alive at `at`.
- **`read_household { person_id, at? }`**: the household's kind, members (names, sex, age, relationship to its anchor) and address.

Ids are the society world's `PersonId` (u32, invariant 4).

## 3. Not in v1, and why

- **Rosters** ("who lives in tract X"): exact, but the first query in an area computes its history, which takes seconds to minutes at prototype scale. They come once histories are cached or precomputed.
- **Name search:** names are functions of ids, so finding by name needs an inverted index (per area and birth decade). It is not built yet; agents navigate from a person through family and household.
- **Individual attributes and work:** Phase 3.

## 4. Tests

- **Round trip:** every view runs through `registry()` with JSON params, as the transport calls it.
- **Agreement:** `read_person` agrees with the world (relatives, partner both ways, household members that list the person back).
- **Cost:** a view call is a handful of lookups, about a millisecond.
