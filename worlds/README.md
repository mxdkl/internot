# World packs

A world pack is a folder of [RON](https://github.com/ron-rs/ron) files that defines a world. Internot's people, families and households are computed from a pack and a seed. The code holds only the mechanisms (how a hazard becomes a union, how markets pair people); every rate, share, age, list and name is in the pack. Each number sits next to a comment saying where it comes from.

Two packs ship with the code and are built into the binary:

| Pack | What it is |
|---|---|
| `us/` | The United States, 1840 to 2100, at prototype scale (about 20,000 births a year in 1840, 14 million people ever born). |
| `us-tiny/` | `us` at test scale: 1900 to 1990, 300 births a year, with rare things boosted so the exhaustive tests cover them. It is one short file, because it extends `us`. |

## Using a pack

```rust
use internot_society::{Params, World};

let us = World::build(Params::prototype(), 42);            // the embedded `us`
let tiny = World::build(Params::tiny(), 7);                // the embedded `us-tiny`
let mine = Params::load("worlds".as_ref(), "my-world")?;   // worlds/my-world/
let world = World::build(mine, 42);
```

`Params::load` reads `worlds/my-world/`. If that pack extends another, the parent is looked for in the same folder first, then among the built-in packs. So your pack can extend `us` without copying it.

## Making a variant

Make a folder with a `world.ron` that names its parent, and add only what changes. A pack that extends another inherits every file it doesn't have, and inside the files it does have, every field it doesn't mention.

```ron
// worlds/us-closed/world.ron
(
    name: "United States without immigration",
    description: "The us pack with no arrivals after 1840.",
    extends: Some("us"),
)
```

```ron
// worlds/us-closed/immigration.ron
(
    rate: (anchors: [(1840, 0.0)]),
)
```

**How merging works:**
- **Records** `( ... )` merge field by field, at any depth.
- **Lists of records with an `id`** (regions, heritage groups) merge by id. Naming an existing id changes only the fields you give; a new id is added at the end.
- **Everything else** (numbers, strings, plain lists such as `[(1840, 0.0)]`) is replaced as a whole.

So in the example above, `rate.scale` and every other immigration setting come from `us`, and only the anchors change.

## Making a new world

Copy `us/` and edit it. A pack needs these files:

| File | What it holds |
|---|---|
| `world.ron` | Name, `extends`, timeline, founder scale, sex ratio at birth, lineage regions, test boosts |
| `mortality.ron` | Siler mortality by sex, and its multipliers by era |
| `unions.ron` | First-union timing, market shares (national, same-sex), age-gap kernel, divorced-status affinity, which unions are marriages and from when |
| `repartnering.ron` | The hazard of a new union after a separation |
| `fertility.ron` | Births per union by era, timing and spacing, births outside a union, gestation |
| `dissolution.ron` | Share of unions that separate, and when |
| `immigration.ron` | Arrivals per year, their ages, couples, sex ratio |
| `heritage.ron` | Heritage groups: founder and immigrant shares, cross-heritage partnering, each group's fertility and mortality relative to the base, an optional immigrant sex ratio |
| `households.ron` | Leaving home, living with kin, custody, roommates |
| `names.ron` | Where the name data is and which column each group uses; how first, middle and surnames are given, passed down and changed at weddings |
| `data/names.bin` | The name tables: first names by year and sex, and first names and surnames by group (for `us`: SSA baby names and Census 2020, distilled by `internot_society/data/distill_names.py`) |

## The vocabulary

Every file uses the same few kinds of value:

| Kind | Looks like | Meaning |
|---|---|---|
| Series | `[(1840, 0.004), (1960, 0.005)]` | A value by calendar year: straight lines between the anchor years, constant before the first and after the last. |
| Distribution by year | `[(1840, [0.06, 0.05, ...]), ...]` | Several values by year, each interpolated the same way. Each row must sum to 1. |
| Steps | `(steps: [(34, 1.0), (44, 0.75)], above: 0.03)` | A value by whole number (age, years): the first step whose bound is at least it, else `above`. |
| Bands | `(below: [(25.0, 1.0), (30.0, 0.8)], above: 0.12)` | A value by real age: the first band whose bound it is below, else `above`. |
| Ranges | `[(-1, 1, 0.09), (2, 3, 0.10)]` | A value on inclusive ranges of whole numbers, zero elsewhere. |
| Per sex | `(female: 0.0005, male: 0.0011)` | One value for each sex. |

## Data files

Any file in a pack that isn't `.ron` is data, read by the section that names it (for example `names.ron` says `data: "data/names.bin"`). A child pack inherits its parent's data files, and a file at the same path replaces the parent's. The name data's format is in the docstring of `internot_society/data/distill_names.py`. It declares its own group columns, and `names.ron` maps each heritage group to one, so a world with other groups or other sources needs only its own file and mapping.

## Checks

A pack is checked when it loads, and errors say where:

```text
world pack `my-world`, unions.ron at national_market_share: anchor 0 (1.5) is outside [0, 1]
world pack `my-world`, my-world/unions.ron: 12:5-12:13: Unexpected field named `natonal`, expected ...
```

- Unknown fields are errors, so a typo never silently falls back to a default.
- Anchor years must increase, shares must lie in [0, 1], distributions must sum to 1, ids must be unique, and ages must fit the limits below.

## Limits that are code

A few bounds size arrays on hot paths, so they live in code (`internot_society/src/params.rs`). A pack must fit inside them, and raising one is a code change.

| Bound | Value |
|---|---|
| Oldest age | 110 |
| Youngest union age | 16 (a pack may raise it) |
| Fertile window | 15 to 45 |
| Oldest arrival age | 80 |
| Re-partnering age bound | 95 (where the hazard is negligible) |
| Separation durations | 5 bands: 1–3, 4–7, 8–12, 13–20, 21–40 years |
| Roommate frame | 12 people |
| Heritage groups | at most 64 |

## Identity

Every loaded pack has a fingerprint, a hash of its merged contents and data files (`Params::fingerprint`). Two packs with the same fingerprint build the same worlds from the same seed. Quote both the pack and the seed when you share a world.
