# The transport world: kinship from scratch, with no population memory

**Date:** 2026-10-02. **Founder:** "forget about all current implementation and try create a truly f(t, seed, id) from scratch with 0 memory footprint that is also fast … brand new ideas from the start."
**Status:** design and first prototype (`internot_society::transport`). Nothing else depends on it.

## 1. Why every design so far stored something

Three causes, and only three:

1. **Renewal.** How many people are born in year Y depends on how many couples formed earlier, which depends on how many were born before that. Realized integer counts feed back, so they accumulate into history (the ledger; the pure world's prefix tables).
2. **Diagonals.** Union year = birth year + age, birth year = parents' union year + duration. An index aligned with two of these makes the third a running sum (run offsets, child tables).
3. **Two orders.** A marriage market wants people ordered by age; a family wants them ordered by parent. The sort permutation between the two orders was stored (or recomputed by counting).

Compressing these tables (the pure world, Lipschitz trees) attacks the symptom. This design removes each cause at the root.

## 2. The idea in one line

**Renewal lives in expectation; integers are only transport.**

- **Aggregates are a mean field:** a deterministic, real-valued projection of the pack's rates (births, couples, deaths by year and age). It is the size of the pack (hundreds to thousands of numbers), computed in milliseconds, and never sees a realized integer. Cause 1 is gone: realized counts never feed back.
- **Every integer is one rounding of an expected cumulative:** a count over any prefix is `⌊E(x) + u⌋` for a keyed offset `u`, so every offset in every index space is O(1), with no sums of floors. Cause 2 is gone wherever one axis is primary (§4 says where a short sum remains).
- **Every relation is a monotone transport between index spaces:** the j-th member of a category in one space is the j-th member of the matching category in another.
- **Each space carries several orders, joined by keyed bijections (Feistel bridges, O(1) both ways).** A cohort's *birth order* (by parent) and *life order* (by sex and union year) are two orders of one space. Cause 3 is gone.

A person is a position. Their facts are categories of that position in one of its orders. Their relatives are positions reached by transports. Nothing about any individual, couple or realized count is stored.

## 3. The spaces and the maps (prototype v0)

Scope of v0: one group, first unions, births within unions, deaths. Re-partnering, non-union births, immigration, same-sex unions and groups come after (§6).

**Mean field** (computed once from the pack; real numbers):
- `CB(Y)`: expected births before year Y. Founder cohorts follow the pack's stable population; from the first year on, births are the projection's own (expected couples × expected fertility by duration and wife's age).
- For each cohort b and sex: `G_b(y)`, the expected share who first partner before year y (competing with death); the cohort life table.
- For each union line y: the men's age profile `CM_y(b)` (share of the line's men born before b).
- For each birth year Y: `S_Y(d)`, the share of its births at duration < d since the parents' union.

**People.** Ids are dense and global. Cohort Y is `[⌊CB(Y)+u⌋, ⌊CB(Y+1)+u⌋)`: one rounding each way, so id → birth year is an inversion of `CB` (O(1)).

**Life order of a cohort** (rank r within the cohort): women first, `W = ⌊B·σ + u⌋`; women grouped by first-union year, group y = `[⌊W·G_b(y) + u⌋, ⌊W·G_b(y+1) + u⌋)`; then the never-partnered; then men, grouped the same way by the men's groups (below).

**Union line y.** Women's seats: cohort runs, oldest cohort first, run length = the cohort's group-y size. Line total `M_y` = their sum. Men's seats: run starts `⌊M_y·CM_y(b) + v⌋`, a systematic rounding of the exact integer `M_y`, so the men's side has exactly `M_y` seats: no queue, no slip. A man's group y in his cohort is his run in line y.

**Coupling.** Woman's seat p ↔ man's seat σ_y(p), a keyed bijection of `[0, M_y)` (v0: comonotone with a block-local shuffle).

**Births.** The births of cohort Y in *birth order* are sorted by duration d since the parents' union: block d = `[⌊B·S_Y(d) + u⌋, ⌊B·S_Y(d+1) + u⌋)`, whose size `T` is exact. Line y = Y − d supplies them: its couples with the wife at most 45 at the birth (a suffix of the line, since the line is oldest first) carry a birth at d iff they are members of the rational Beatty sequence `⌊n·T/L + t⌋` over that suffix of length L. Its total is `⌊T + t⌋ = T`, so it is exact by construction; count and select are each one division.

**Bridge.** Life rank = π_Y(birth rank), π_Y a keyed Feistel permutation of the cohort. Sex and union timing are therefore independent of family position.

**Deaths.** Drawn from the cohort life table, conditioned to fall after the person's last structural event (union; last birth for women, last conception for men). The never-partnered draw from the residual table `d(a)·(1 − U(a))`, which is exactly the law of death with no union before it when union and death are independent.

**Every lookup** is O(1) arithmetic plus binary searches in tables the size of the pack:
- `mother(x)`: cohort, π⁻¹, duration block, Beatty select, line run, woman's rank.
- `father(x)`: the same seat through σ.
- `children(x)`: union seat, then for each duration a Beatty membership test, then π.
- `partner(x)`: group → seat → σ → run → rank.

## 4. First result (v0, 2026-10-02)

**One correction during the build.** Rounding birth blocks on the cohort side (`⌊B·S(d) + u⌋`) can exceed the realized eligible couples of a thin line by 1 (seed 105, tiny world). The fix makes the aggregate a **realized integer cohort-component projection**, computed in year order: each block is `⌊h·Σ eligible × survival + u⌋` over its line's realized eligible couples, so `T ≤ L` by construction (`⌊hL + u⌋ ≤ L` for `h ≤ 1`). The table holds counts per (cohort, year, duration), never anything per person, so its size is unchanged. The world is a census table the size of the pack, plus transport.

**Exactness:** `tests/transport.rs`, exhaustive on the tiny world (45k people), seeds 3, 7, 11, 42, 99 and 105 pass:
- partners mutual, with the same start;
- mothers and children, and fathers and children, each list the other;
- mothers aged 15–45 and alive at every birth; fathers alive at conception;
- every native has both parents, and the parents are a couple;
- everyone is born before they die.

**Cost at prototype scale** (`examples/transport_report.rs`, `us` pack): 6.07M people ever, **tables 1.83 MB, build 17 ms**.

| Lookup | p50 | p99 |
|---|---|---|
| birth | 110 ns | 121 ns |
| death | 430 ns | 721 ns |
| mother | 120 ns | 130 ns |
| father | 120 ns | 131 ns |
| union | 80 ns | 110 ns |
| children | 260 ns | 551 ns |

Against the pure world (p99, ns): mother 941, father 2,635, union 761, children 6,622; today's world: 1,893, 11,021, 10,800, 15,249.

**Realism gaps of v0** (by design, to fix next):
- the age gap's sd is 0.9 years (comonotone coupling), against about 4;
- childlessness among couples is 2–8% (no fertility heterogeneity), against 10–15%;
- 6.1M people ever, against 13.6M (pure) and 17.3M (today): no re-partnering, non-union births or immigration yet.

## 5. v0.1: coupling, parity classes, kin repair (2026-10-02)

**Coupling.** Three block shuffles of a line's seats, each a keyed permutation within a block:
- two full shuffles in blocks of half the line, the second staggered by a quarter;
- one long-range shuffle of a mobile 45% (a rational Beatty category) across the whole line.

The composition is a bijection, and its inverse runs the levels backwards. The mean gap is fixed by the marginals (2.2 years).

| Cohort | sd of gap | Wife older |
|---|---|---|
| 1880 | 3.7 | 24% |
| 1920 | 3.3 | 22% |
| 1950 | 3.1 | 20% |
| 1980 | 4.2 | 27% |

Today's world: sd 3.8–4.5, 22–27%.

**Parity classes.**
- Each couple has an intended parity `n ∈ 0..=8` with the pack's shares, as a **comb of rational Beatty splits**: class n takes its count of the seats left by classes before it.
- Count, class and select are O(n) multiply-divides. Selecting among the non-members uses the fact that the complement of a rational Beatty sequence `(S, len, τ)` is again one, `(len − S, len, len − 1 − τ)`.
- Class n's births at duration d use the pack's own timing for parity-n plans (first-birth offset and spacing), at most one birth a year.
- Childless couples: 12.6% (1880), 14.8%, 12.9%, 16.7% (1980).

**Kin repair at lookup time, exact, with nothing stored.**
- With one union each, siblings are exactly the children of one couple, and a couple is its wife's seat, which repair never moves. So "close kin" is `source(wife) == source(husband)`, exact in the repaired world too, with no recursion through earlier repairs and no build-time scan.
- Seats pair as `(2i, 2i+1)`. If either couple of a pair is close kin and exchanging the husbands leaves neither close kin, they exchange. This is an involution, so both partners agree.
- A cheap filter (siblings share a parents' line and parity class, both read off the birth block) skips the full parent resolution almost always.
- **Result:** 0 close-kin couples among 2,105,068 (10 without repair).

**Cost now** (prototype, `us`; 6.07M people; tables 2.53 MB; build 20 ms):

| Lookup | p50 | p99 |
|---|---|---|
| birth | 160 ns | 211 ns |
| death | 562 ns | 1,733 ns |
| mother | 170 ns | 220 ns |
| father | 802 ns | 952 ns |
| union | 731 ns | 992 ns |
| children | 350 ns | 1,543 ns |

Repair costs about 0.5 µs on union and father (four parent-block reads per pair). Every lookup is still faster at p99 than the pure world's (father 2,635, children 6,622).

## 6. v0.2: re-partnering, any number of unions (2026-10-02)

**Census.** All counts per year, the size of the pack:
- each line's dissolution classes: separating in the s-th year, or never, by the pack's `class_pmf`;
- each separation year's **pool**, ordered by source line (first-union lines, then re-partnering lines, by year);
- each pool's women's **delay classes** (re-partnering e years on, or never; the pack's hazard at a typical age);
- each **re-partnering line**'s women by delay. The men's runs are the line's exact total, apportioned over the pools by the men's hazards and capped by each pool's remaining men.

**Categories are balanced trees of exact rational Beatty splits.** A node over levels `[a, b)` sends exactly `cum[mid] − cum[a]` of its positions left. Class, count and select are each O(log levels), about 6 multiply-divides for 41 dissolution classes. These trees replace the combs.

**Chains.** A woman's chain: seat → dissolution class → pool rank → delay class → re-partnering seat → …. A man's: seat → pool rank (of the couple he is the default husband of) → a keyed permutation of the pool's men → his re-partnering line's run. Each person's union list is a walk along their chain, O(number of unions).

**What keeps it exact and simple:**
- **Kin repair pairs couples within a dissolution class.** Exchanging husbands then never changes which men separate in which year, so a pool names its men by the *default* coupling, and identifying the man at a seat never recurses through repairs.
- **Deaths depend on the first union only** (its start; a mother's plan births; a father's conceptions before the planned separation).
- **A re-partnering whose partner is already dead is void.** Both partners skip it and both continue through its planned separation, so both sides always agree. A first union that a death ends early still sends the survivor to its separation year's pool, which gives widowed re-partnering for free.
- **Births follow the woman's lifetime plan,** anchored at her first union (as in the pure world). The father is her non-void partner at conception, if he is alive then. A man's children are those his partners conceived while with him.

**Exact** on the tiny world (seeds 3, 11 and 105), every person:
- every union mutual, with the same start and separation;
- each union after the last one ended (separation or the partner's death);
- mothers and fathers dual with children; parents alive at birth and conception.

Per seed: about 15,700 couples, 2,400 of them re-partnerings; about 500 children whose father is a later partner.

**Prototype** (`us`): 6.07M people, **tables 2.97 MB, build 24 ms**. Close-kin couples: **0 of 2,919,555**. Female e0 by cohort is unchanged (52.5, 68.5, 77.7, 81.8), since deaths no longer wait for later unions.

| Lookup | p50 | p99 |
|---|---|---|
| birth | 161 ns | 201 ns |
| death | 621 ns | 1.9 µs |
| mother | 170 ns | 210 ns |
| father | 982 ns | 5.1 µs |
| union | 972 ns | 4.4 µs |
| unions | 1.0 µs | 27 µs |
| children | 381 ns | 24 µs |

**Performance debt.** A person with k unions costs about 0.7·k² + 2.5·k µs, because resolving the person at a re-partnering seat walks back k unions and repair resolves people at every step. With 2 unions it is 9 µs on average, with 6 it is 34 µs; 3% of people have 3 or more. Fixes, measured later:
- carry known identities (the asker, the partner) through the chain instead of resolving them again;
- cache each line's Feistel shapes (pack-sized);
- cheaper repair filters in re-partnering lines.

## 7. v0.3: non-union births, re-partnering by age, speed (2026-10-02)

**Non-union births.**
- A woman of cohort b not yet partnered in the year she turns a is a life rank in a **suffix** of her cohort's life order: women are grouped by first-union year, the never-partnered last.
- So births at age a to single women are one rational Beatty split of that suffix, with a total from the pack's rates (`nonunion_count_pmf`, `nonunion_age_weight`) times survival to a, capped at the suffix's size.
- They have no father. All of a woman's non-union births fall at her own phase of the year, a year or more apart, and before any union.
- A native's origin is now a couple's plan or a single mother. Kin repair compares mothers when either person is a single mother's child.

**Re-partnering by age.**
- One typical age for every separated person let 70-year-olds re-partner like 35-year-olds: 9.6% of the partnered reached a third union or more.
- Each pool now falls into 10 **segments** by line kind and duration band (a stand-in for age, since couples separating after 30 years are older). Each segment has its own women's delay comb and men's runs at its typical age.
- Result: 3 or more unions for 8.4% of the partnered (US about 6%); 2 unions 17%.

**Speed** (each change checked by an identical answer checksum):
- repair resolves 3 people per couple, not 5 (the asker passed in, the default husband reused);
- 64-bit arithmetic in rational Beatty count and select;
- a walk stops at the person's own death, since every later couple starts later;
- last births are found by scanning backwards.

**Exact** (tiny world, seeds 3, 11 and 105), with everything above. **Prototype** (`us`): 6.99M people ever, **tables 4.98 MB, build 77 ms**, 0 close-kin couples among 3.29M.

| Lookup | p50 | p99 |
|---|---|---|
| birth | 160 ns | 241 ns |
| death | 531 ns | 1.9 µs |
| mother | 171 ns | 210 ns |
| father | 982 ns | 4.9 µs |
| union | 932 ns | 3.6 µs |
| unions | 992 ns | 17.6 µs |
| children | 511 ns | 16.5 µs |

`unions` costs about 1.5 µs with one union, 7 µs with two, and about 5 µs for each one after.

| Cohort | Partnered | CFR | Childless couples | e0 (women) |
|---|---|---|---|---|
| 1880 | 62% | 2.19 | 12% | 53.7 |
| 1920 | 78% | 2.16 | 14% | 69.9 |
| 1950 | 89% | 2.41 | 12% | 79.5 |
| 1980 | 87% | 2.05 | 15% | 82.9 |

**Debt:**
- e0 runs about 1–2 years high: the residual death table does not yet account for non-union births;
- no paternal half-sibling check in kin repair (measured: 0 at prototype scale);
- no immigration, same-sex unions, heritage or areas yet;
- the re-partnering line tables (401 entries per line, twice) could be compressed.

## 8. v0.4: immigration (2026-10-02)

**Census by calendar year.** Each year Y, in order:
1. the natives of cohort Y, from the lines of the years before;
2. **union line Y**: every cohort's women first partnering this year, natives then immigrants who arrived before Y; men apportioned to the same total, capped by each cohort's natives and arrived immigrants left;
3. **year Y's arrivals**: the pack's rate times the living population, spread by the Rogers–Castro age profile and the men's share.

Only past years are ever needed, so one pass suffices.

**Immigrants live in their birth cohort's index space, inside its union groups.**
- A cohort's life order is, for each first-union year group, its natives then its immigrants; then the never-partnered, natives first.
- Natives keep a natives-only order for the birth-order bridge, and `native_to_full` / `full_to_native` map between the two in O(log) through the cumulative group tables.
- A line's run for a cohort covers both natives and immigrants, so immigrant women partner with natives in the ordinary lines, and their children are ordinary births.

**Arrival years are computed, not stored.**
- An immigrant's arrival year is a category of their rank within their group: one systematic rounding of weights over arrival years before the union year (expected arrivals still single then), or, for the never-partnered, over arrivals who never partner within the world.
- It costs O(arrival years) per lookup.
- Arrivals are adults, have no in-world parents, are alive on arrival (deaths wait for it), and partner only after arriving.

**One bug found by the exhaustive test.** Groups whose line falls past the world's end were never written in the incrementally built tables, so their cumulative counts dropped to 0 and two seats mapped to one child. They now carry the last value.

**Exact** on the tiny world (seeds 3, 11 and 105), about 3,500 immigrants per seed.

**Prototype** (`us`): **13.5M people ever** (pure world 13.6M), **tables 6.06 MB, build 82 ms**, 0 close-kin couples among 5.73M. Lookups are unchanged (mother p99 251 ns, union 3.7 µs, unions 17.6 µs).

| Foreign-born share of the living | 1870 | 1910 | 1950 | 1970 | 2000 | 2020 |
|---|---|---|---|---|---|---|
| Model | 12.8% | 15.4% | 6.7% | 5.1% | 10.5% | 13.7% |
| Census | 14.4% | 14.7% | 6.9% | 4.7% | 11.1% | 13.7% |

| Cohort | Natives partnered | Natives' CFR | Immigrants partnered | Immigrants' CFR |
|---|---|---|---|---|
| 1880 | 62% | 2.19 | 38% | 1.25 |
| 1920 | 78% | 2.16 | 7% | 0.22 |
| 1950 | 89% | 2.41 | 25% | 0.57 |
| 1980 | 87% | 2.05 | 44% | 0.94 |

**Debt: arrivals come single** (as in the pure world), so immigrant women partner and bear children far too little.

**The fix is arriving couples:** a third kind of line, one per arrival year, whose couples formed abroad. Its seats are immigrant women and men of their cohorts; its plans give children born abroad (immigrants arriving with both parents) and births after arrival.

## 9. v0.5: same-sex unions, complete kin repair (2026-10-02)

**Same-sex unions.**
- Each cohort's life order has a second set of first-union groups after the opposite-sex ones: the natives' same-sex groups, a separate rounding of incidence × the pack's same-sex share. Immigrants partner opposite-sex only in this version.
- Natives-only and full orders differ only inside the opposite-sex groups.
- Each year has one same-sex line per sex: seats by cohort (age order), shuffled within blocks, paired `2i ↔ 2i+1`.
- Kin repair re-pairs a group of four `0↔2, 1↔3`. No plan births and no re-partnering, as in the ledger and the pure world.

**Kin repair is complete.** Every union kind is checked against siblings, half-siblings through either parent, and parent and child.
- **A couple that repair cannot fix is void:** both partners skip it and both continue through its planned separation, as with a dead partner. Repair already computes whether the couple is related, so this costs nothing. Small lines on the tiny world need it, because a dissolution class there is often a single couple. Pairs must stay within a class so that pools can name men by the default coupling.
- **Paternal relations use actual fathers,** behind two exact filters:
  - a father relation exists only if one of the two was conceived in a re-partnering (a man has one first union), or the age gap allows parent and child;
  - repair only exchanges husbands within a pair, so a couple's actual husband is one of two default candidates (its own and its mate's).
  
  The repair-aware father, which recurses into earlier generations, runs only when a candidate matches.
- Two bugs found by the exhaustive check and fixed. A father–daughter check sat behind the siblings' age filter. Default fathers missed a repaired parent couple.

**Exact:** `tests/transport.rs` now asserts no close kin in any union, against actual parents. It passes on **68 tiny-world seeds** (1–3, 7, 11, 42, 99, 105, 200–259).

**Prototype** (`us`): 13.0M people, **tables 6.58 MB, build 88 ms**, 0 close-kin couples among 5.5M opposite-sex couples.

| Lookup | p50 | p99 |
|---|---|---|
| birth | 180 ns | 291 ns |
| death | 641 ns | 4.7 µs |
| mother | 190 ns | 241 ns |
| father | 1.4 µs | 6.6 µs |
| union | 1.5 µs | 8.1 µs |
| unions | 1.6 µs | 25 µs |
| children | 581 ns | 24 µs |

Repair now checks every relation, so lookups that resolve couples cost about 0.5 µs more than at v0.4.

## 10. Where this leaves the three worlds (2026-10-02)

| | Today's world (ledger) | Pure world | **Transport world** |
|---|---|---|---|
| State | 1.5 GB of layouts (5.6 GB in area mode) | 22 MB of realized prefix tables | **6.6 MB census of counts** (no individual, couple or line-cell state) |
| Build | 7 s | 0.3 s, plus a 1.4 s kin scan that grows with population | **88 ms** |
| Grows with population | yes (^0.55) | tables no, kin scan yes | **no** (counts change, tables keep their size) |
| Kin repair | build-time cells, about 1 in 9M left | build-time year-ordered scan, stored list | **at lookup, nothing stored**; unfixable couples void |
| mother p99 | 1.9 µs | 0.94 µs | **0.24 µs** |
| father p99 | 11 µs | 2.6 µs | 6.6 µs |
| union p99 | 10.8 µs | 0.76 µs | 8.1 µs |
| children p99 | 15 µs | 6.6 µs | 24 µs |
| Same-sex, re-partnering, non-union births, immigration | yes | yes | yes |
| Arriving couples | yes | no | no |
| Heritage, areas | yes | no | no |

**Why the transport world is different in kind.**
- Its only state is a census table: counts per (cohort, year, duration, class), the same shape as the pack.
- Every individual fact is a closed form over it: rational Beatty sequences, trees of exact splits, Feistel bridges.
- Every kinship relation is computed both ways from the same roundings, so duality holds by construction (tested exhaustively).
- Kin repair needs no scan, because "close kin" is a predicate the lookup evaluates itself, through exact filters.

**Where it is slower.** Tails of father, union, unions and children, for people with several unions. A re-partnering seat is resolved by walking back through pools, and kin repair resolves 3 people per couple. Levers: a cheaper pool walk (shapes cached per line), and fewer repair resolutions (most couples could skip repair with a coarser filter).

**Open: arriving couples.** Two designs:
- (a) **Arrival lines as a second kind of first-union line:** their own runs (couples kept together), births by duration since arrival as new birth blocks, dissolution into pools as a third source kind. More plumbing (pool sources, birth blocks), and it is exact.
- (b) **Arrival couples inside first lines,** interleaved in each cohort's run (keeping age order for births), with a separate coupling over the arrival subsequence through per-line cumulative tables. Less new structure, but every first-line coupling call has to translate ranks.

Children born abroad, linked to their parents both ways, need one more index under either design: births before arrival as blocks of the arrival line, placed into their cohorts' immigrant parts.

**Recommendation.**
1. Adopt the transport world as the kinship floor, behind the same API as today's world.
2. Build arriving couples with design (a).
3. Then heritage and areas. As groups, these are census dimensions, so they cost table size, not lookups.

**Speed after v0.5** (2026-10-02, identical answer checksum throughout):
- kin repair computes each person's origin once and derives every check from it;
- each cohort's and each pool segment's Feistel shape is cached (24 bytes each).

Prototype: tables 6.67 MB, build 86 ms.

| Lookup | p50 | p99 |
|---|---|---|
| birth | 160 ns | 271 ns |
| death | 601 ns | 4.1 µs |
| mother | 171 ns | 230 ns |
| father | 1.3 µs | 5.8 µs |
| union | 1.4 µs | 7.5 µs |
| unions | 1.4 µs | 22.7 µs |
| children | 551 ns | 21.8 µs |
