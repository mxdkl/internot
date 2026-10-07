Scripts behind the [computed] numbers in `research/2026-10-01-education-data-and-targets.md`.
Inputs are under the git-ignored `datasets/` (paths are written into each script). Run with
`uv run --no-project --with pandas --with pyarrow --with numpy --with scipy --with geopandas --with pyogrio --with shapely python <script>.py`.

| Script | Inputs | Gives |
|---|---|---|
| `membership.py` | CCD 2024-25 membership (052), piped through `unzip -p` | `datasets/education/ccd/derived_membership_{total,by_grade}_2425.csv` |
| `analyze_k12.py` | CCD directory (029), staff (059), characteristics (129), the derived membership files, EDGE 2024-25 geocodes, 2020 tract centres | §6 public schools: counts, sizes, grade spans, grade rosters, locale, pupil/teacher; §8 nearest-school catchments |
| `analyze_pss.py` | PSS 2023-24 public-use file | §7 private schools |
| `analyze_ipeds.py` | HD2024, EF2023A, EF2023B, EF2022C, IC2023, CCD per-grade file, 2020 tract centres | §9 institutions, sizes, ages, in-state share, dorm beds, distance to the nearest college |
| `load_pums.py`, then `analyze_pums.py` | ACS 2023 1-year PUMS persons | §1 enrollment by age, §5 grade by age, §2 attainment by cohort, §4 spouse matching and the "free" baseline, §9 where undergraduates live |
| `analyze_sabs.py`, `sabs_defacto.py`, `sabs_split.py` | SABS 2015-16 shapefile, 2020 block-group and tract centres | §8 attendance-zone sizes, de facto shares, tract splitting |
| `transmission/` | GSS 1972-2024, CPS ASEC 2023, Schwartz & Mare cells (see its README.txt) | §3 and §4 transmission and spouse tables |
