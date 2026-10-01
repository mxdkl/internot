# World packs: every statistic in editable files

**Date:** 2026-10-01
**Research:** `research/2026-10-01-world-definitions.md`.
**Founder request (2026-10-01):** everything is configurable through RON files. That means races, names, birth rates, countries, places and, later, services, "easily extendible by anyone". The specifics are mine to decide.

## 1. The rule

**Mechanisms are code; every number, list and name is data.**
- The code holds the ledger, the markets, keyed draws and the functional forms (Siler mortality, log-logistic first-union timing, Rogers–Castro migration ages).
- A **world pack** holds every parameter those mechanisms use, each next to a comment naming its source.
- A world is a pure function of `(pack, seed)`.

Structural bounds stay in code because they size arrays on hot paths:
- the maximum age (110);
- dissolution classes (40);
- union chains (16);
- the fertile window that sizes the parent slots (15–45);
- the oldest arrival age (80);
- the re-partnering age bound used by death's fast path (95).

A pack is validated against them. They are listed in `worlds/README.md` with how to raise them.

## 2. Layout

```
worlds/
  README.md            how packs work, the value vocabulary, every section
  us/                  the United States, 1840–2100
    world.ron          name, extends, timeline, scale, regions, test boosts
    mortality.ron
    unions.ron         first unions, markets, age gaps, same-sex share
    repartnering.ron
    fertility.ron
    dissolution.ron
    immigration.ron
    heritage.ron       groups, founder and immigrant mixes, intermarriage
    households.ron
    names.ron          (N1) name sources and naming practice
    data/names.bin     (N1) distilled name tables
  us-tiny/
    world.ron          extends "us": 1900–1990, 300 births a year, test boosts
```

Packs live at the repository root, because services (the `internot` crate) will add their own sections later.
- `us` and `us-tiny` are embedded in the binary (`include_str!`/`include_bytes!`), so nothing depends on the working directory.
- Any other pack loads from a directory.

## 3. Format: RON, with three conventions

1. **Unnamed structs** `( field: value, ... )`. No data-carrying enums: `ron::Value`, which merging uses, drops variant names. Choices are string ids or fields.
2. **Unknown fields are errors** (`deny_unknown_fields`).
3. **Lists of things with identity are lists of records with an `id`:** regions, heritage groups, later countries, places and services. Their order is their index.

## 4. Values: a small vocabulary

| Type | RON | Meaning |
|---|---|---|
| `Series` | `[(1840, 0.004), (1960, 0.005)]` | piecewise-linear in year, clamped outside |
| `GeoSeries` | same | geometric (log-linear) interpolation, for multipliers |
| `VecSeries<N>` | `[(1840, [0.06, ...]), ...]` | componentwise piecewise-linear, for distributions |
| `Steps` | `(steps: [(34, 1.0), (44, 0.75)], above: 0.03)` | value by upper bound, inclusive |
| `Ranges` | `[(-1, 1, 0.09), ...]` | value on inclusive integer ranges, 0 elsewhere |
| `BySex<T>` | `(female: T, male: T)` | |

Each section is a typed struct in its consumer's crate. A section's methods (`mortality.death_prob(sex, age, year)`) keep the exact arithmetic of today's free functions.

## 5. Extending a pack

`world.ron` may say `extends: Some("us")`. The child pack's files are then **merged into the parent's** before they are read:
- **records (maps):** merged field by field, recursively, with the child winning;
- **lists of records with an `id`:** merged by `id`; a matching id merges its fields, and a new id is appended;
- **anything else** (numbers, strings, plain lists): the child's value replaces the parent's.

So a variant is as small as its difference. For example, a US without immigration:

```ron
// worlds/us-closed/immigration.ron
( rate: (scale: 0.85, anchors: [(1840, 0.0)]) )
```

**Errors:**
- syntax errors carry the file, line and column;
- type errors after a merge carry the file and the field path (`serde_path_to_error`).

## 6. Compilation and speed

`Params::load` (or `Params::embedded`) reads, merges, validates and deserializes once.

**Lookups:**
- read the typed sections directly;
- hot paths keep today's arithmetic, and where it pays, tables are precomputed at `World::build` (for example death probabilities by sex, age and year).
- Measured with the kinship suite.

## 7. Identity

`Params::fingerprint()`: a 64-bit hash of the merged, canonically re-serialized sections plus the bytes of each data file. Uses:
- reports print it;
- the LLM render cache keys on it (later);
- a published seed is meaningful together with its pack fingerprint.

## 8. Crates

- **`internot_def`** (new, small): pack sources (directory, embedded), `extends` resolution, the merge, errors, the fingerprint, and the value vocabulary (`Series`, `Steps`, ...). It depends on `serde`, `ron` and `serde_path_to_error`.
- **`internot_society`:** its sections (`mortality`, `unions`, ...) and `Params` (the compiled world definition) built from a pack.
- **Later:** services read their own sections from the same pack.

## 9. Migration (bit-identical)

1. Fingerprint today's worlds (`examples/world_fingerprint.rs`, recorded 2026-10-01: tiny `b19c209ec97e8c4c` / `f3bd18ec8c5afb2e`, prototype `e47e257ad076f57d` / `bc764f24987f394f`).
2. Build `internot_def` and the `us` / `us-tiny` packs.
   - Values come from today's constants, printed with Rust's shortest round-trip format (e.g. `1.05 / 2.05` is written as `0.5121951219512195`), so parsing gives the same bits.
3. Move each `params.rs` function to a method on its section, one section at a time. The fingerprint must not change.
4. Remove `params.rs`'s numbers. What's left is `Sex`, structural bounds and the vocabulary's use.
5. Then heritage groups come from the pack (no `Heritage` enum; groups are ids), and names are built pack-first.

## 10. Later sections

- **places** (regions, then GeoNames-based places);
- **countries** (immigrant origins with their own name tables);
- **services:** mail volumes, calendar habits and the like.

Each is a new file and a typed section. A pack that lacks a section gets the code's documented default only where one exists; otherwise loading fails with the missing file's name.

## Outcome, steps 1 to 4 (2026-10-01)

**Built:**
- **`internot_def`:**
  - sources (directory and embedded);
  - `extends` with the merge (records field by field, id'd lists by id);
  - errors with the file and the line or field path;
  - the fingerprint;
  - the vocabulary `Series`, `VecSeries`, `Steps`, `Bands`, `Ranges`, `BySex`.

  Seven tests.
- **The packs:**
  - `worlds/us/` has nine sections: world, mortality, unions, re-partnering, fertility, dissolution, immigration, heritage and households. Every number carries its source comment.
  - `worlds/us-tiny/` is one file extending `us`.
  - `worlds/README.md` is the guide for anyone making or changing a world.
- **`internot_society::params`:**
  - It is now the typed sections plus `Params` (load, embed, check), with the old free functions as methods of the same arithmetic.
  - `build.rs` embeds `worlds/`.
  - The non-union second-birth gaps, which were hard-coded in `plan.rs`, moved to the pack too.
- **Structural bounds stay in code,** listed in the README: maximum age, minimum union age, fertile window, arrival and re-partnering bounds, dissolution bands, roommate frame size, at most 64 groups.

**Checks:**
- **The world fingerprint is unchanged:** tiny `830e0ddfaf6b1702` / `f3bd18ec8c5afb2e`, prototype `fc1cc62ffcc638bb` / `bc764f24987f394f`. The baseline was re-recorded with heritage printed as its index.
- **Tests:** all 860 workspace tests pass, including new ones:
  - the packs load;
  - a directory pack extends an embedded one (the README's example);
  - a bad value is reported with its file and field.
- **Clippy:** clean for the new code.
