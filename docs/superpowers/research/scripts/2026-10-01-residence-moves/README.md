# Scripts behind the [comp] figures of `../../2026-10-01-residence-moves-and-basins.md`

Quick analysis scripts, kept as written so the computed figures can be rerun. They read the git-ignored inputs under `datasets/` (paths are absolute to the original checkout; adjust `G` and the `pums/` paths) and print tables.

- `flows.py`: ACS 2016–2020 county-to-county flows rolled up to ERS 2020 commuting zones: zone, county and state containment of domestic moves.
- `flows_age.py`: the same by age (ACS 2011–2015 flows by age).
- `gravity.py`: cross-zone distance decay (piecewise power-law fit by distance band); run after `flows.py`, whose definitions it reuses.
- `split.py`, `nested.py <cz id>`: the cost of splitting big zones (county-line crossings observed; a distance kernel below the county), per zone.
- `pums_agg.py`: aggregates ACS 2023 1-year PUMS (mobility, tenure, age, birth state) into `pums/agg.pkl`.
- `interstate.py`: primary, return and onward interstate moves, and the birth-state against current-state destination overlap, from `pums/agg.pkl`.
