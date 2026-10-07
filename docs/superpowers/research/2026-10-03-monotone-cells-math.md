# Cells in the monotone world: groups and the open market (math before code)

**Date:** 2026-10-03. Phase 1 of `specs/2026-10-03-global-world.md`.
**Adapts:** the cell world's groups and open market (`2026-10-02-cell-world-math.md` §16, §16.2, §16.6) to the monotone world's unions (one integer table per wife cohort, laid out by an exact interleave) and births (blocks of mothers' ages, proportional owner).

## 1. Index spaces

A country has groups `g = 0…G−1` (heritage, the pack's catalogue). A **cell cohort** `(g, y)` is the people of group `g` born in year `y`, numbered in life order exactly as one cohort of the monotone world: women first; per sex the young, then adults; women's adults are wives ranked by union age, then the never-partnered; men's adults are husband segments, then the rest.

`Pid { cell, y, i }`. With one group, every map below reduces to the monotone world's.

## 2. Births by group

Year `y`'s births `B(y)` (the pack's series) split over groups by keyed systematic apportionment with weights

```text
ω_g(y) = f_g(y) · Σ_{a=15}^{45} W_g(y − a) · S_g(y − a, a) · φ(y, a)
```

where `W_g(y')` is cell cohort `(g, y')`'s realized women, `S_g` its survival to age `a` (its own life table), `φ` the mother-age shape and `f_g` the group's fertility factor (pack). Cohorts before `y0` are founders: `B_g` for them splits the founders' births by the pack's founder shares. Computed in year order, since `ω_g(y)` reads earlier cohorts' realized sizes.

Within a cell, births choose mothers among the cell's women exactly as in one cohort of the monotone world (blocks, capped split, proportional owner): a child's group is its mother's.

## 3. Mortality by group

The pack's group factors scale the Siler terms: `infant` the infant term, `adult` the background and old-age terms, by year. A cell cohort's life table is its own (the death quantile tables are per cell cohort).

## 4. Union spaces

Unions happen in a space `U ∈ {0…G−1, O}`: group space `g` (both partners of group `g`) or the **open market** `O` (partners meet regardless of group).

**Wives.** A cell cohort's wives `W` (all its first-union women, ranked by union age) are split by an exact rational Beatty set `B = RationalBeatty { t: n_O, len: W, τ }`:
- members are its open wives (space `O`), counted and selected in closed form;
- non-members are its in-group wives (space `g`), the `r`-th at `select_out(r)`;
- `n_O = ⌊o_F(g, y + 25) · W + u⌋`, the pack's open share for women.

"Married before age `a`" is still a prefix of `W`, so union-age classes, births and deaths are unchanged.

**Men.** A cell cohort's adult men are, in order: in-group husband segments by wife-cohort category `c` (50 of them, as now), then open husband segments by `c` (50), then the rest. Sizes come from the spaces' tables.

**Tables and caps.** Each space has an integer table per wife cohort `yw`:
- group space `g`: categories `c` (husband cohort `yw + 19 − c`), entries filled in year order by `capped_split` under each husband cohort's in-group pool, exactly as now;
- open space `O`: categories `(c, h)` for every husband cohort and group, `50·G` of them, weighted by the age-gap kernel times the open pools' shares, capped by each `(yw + 19 − c, h)` open pool.

A cell cohort's men split into an in-group pool and an open pool by the pack's open share for men (`⌊o_M(h, s + 27) · M + u⌋`). The availability factors (12 passes) run per space.

**Open wives across groups.** Space `O`'s wives of cohort `yw` are all groups' open wives. Their order in `O` is an exact interleave over groups (`lattice::ExactInterleave` with part sizes `n_O(g, yw)`), so every prefix takes each group in proportion (the cell world's §16.6 fix). `O`'s rank `k` decodes to `(g, k_g)` by `locate`, and `k_g` to the cell cohort's wife position by the Beatty `select`.

**Layout.** As now, each space's wives of cohort `yw` are laid out over their categories by an `InterleaveTable` of the table's row. So:
- **wife → husband:** her rank in her space (in-group: `r − count(r)`; open: interleave rank of `(g, count(r))`), then `locate` → category and rank in it, then the husband's position in his segment;
- **husband → wife:** his segment gives the space and category, `select` gives the space rank, and the space rank decodes to the wife.

**Lemma O (one space per wife, one per husband).** Every wife position is a member or a non-member of `B`, never both, so every wife is in exactly one space. Every husband position is in at most one segment. In each space the table's row for `yw` sums to that space's wives of `yw` and its column for husband cell cohort `(h, s)` to that cohort's segment size, so the two layouts are bijections onto the same couples. ∎

**Union time** is the wife's, from her position in her cell cohort's wives (unchanged). Separation, voids and the father rule are unchanged (they read the couple through `spouse`).

## 5. Costs

- **Build:** per space as now; the open space's table has `50·G` columns.
- **Memory:** per cell as the monotone world's (G times), plus the open space's interleaves (`2·50G` nodes per wife cohort). Per-cell layouts must shrink (spec §2 P13, §3) before 200 countries.
- **Lookups:** in-group spouses as now plus one Beatty count; open spouses one group interleave more (`log G` levels).

## 6. Checks

- **One group, open shares 0:** the world reproduces the monotone world's answers (checksum by `(cell, y, i)`).
- **Exhaustive (tests/mono.rs):** spouses mutual across cells; every open couple's partners belong to the groups the decode names; mothers and children in the same cell; counts equal brute force per cell.
- **Realism:** composition by group (natives only until countries bring migrants), newlyweds across groups against Pew by era.
