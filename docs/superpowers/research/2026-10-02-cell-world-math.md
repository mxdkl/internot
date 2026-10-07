# The cell world, as mathematics (draft; math before code)

## 0. Notation

- `Y` a birth year; people are `(Y, E, i)` (birth year, entry year, index). Natives: `E = Y`.
- `⌊·⌋_u` means `⌊· + u⌋` for a keyed offset `u ∈ [0, 1)`.
- A **rounded cumulative** of a nondecreasing real function `F` is `N(x) = ⌊F(x)⌋_u`. Counts are its differences `N(b) − N(a)`. They are nonnegative, they telescope, and each is within 1 of `F(b) − F(a)` (proven: `PureWorld.lean`, systematic rounding).
- A **rational Beatty set** over `len` positions with `t` members (`t ≤ len`, offset `τ < len`): position `n` is a member iff `C(n+1) > C(n)`, where `C(n) = ⌊(n·t + τ)/len⌋`. Exactly `t` members, at most one per position, and the complement is again one (proven: `Transport.lean`).

## 1. Index spaces

For each cohort `Y`, the **natives** `[0, B(Y))` with `B(Y) = ⌊b(Y)⌋_u`, where `b` is the pack's births series. This is closed form, with no renewal.

Each cohort carries two orders of the same people, joined by a keyed bijection `π_Y` (Feistel):
- **birth order:** by mother (blocks by mother's age and kind of union);
- **life order:** women first, then men. Each sex is laid out as `[died young][first-union segment][pools…][rest]`.

## 2. Births: children choose mothers

A cohort's birth order is cut into blocks `(kind κ, mother's age a)` by one rounded cumulative of a closed-form share `G_Y(κ, a)`.

In block `(κ, a)` there are `c` children and `M` eligible mothers (a contiguous range of cohort `Y − a`'s life order). The j-th child's mother is `μ(j) = ⌊j·M/c⌋`.

**Lemma B (duality).** `μ` is nondecreasing and onto `[0, M)` when `c ≥ M`. Mother `m`'s children are exactly `j ∈ [⌈m·c/M⌉, ⌈(m+1)·c/M⌉)`.

Proof: `⌊j·M/c⌋ = m ⟺ m·c ≤ j·M < (m+1)·c ⟺ ⌈m·c/M⌉ ≤ j < ⌈(m+1)·c/M⌉`. ∎

(When `c < M`, some mothers get no child in the block; the ranges are still exact.)

## 3. The marriage queue

**Data.**
- Demand cumulative `D` (people seeking a partner, born before `x`).
- Supply cumulative `S` (the pool of possible partners, born before `x`).
- A shift `g`: supply born `s` is meant for demand born `s + g`.

Both are nondecreasing and piecewise quadratic (integrals of piecewise-linear densities), so they are closed form.

**Definition.** The excess is `G(s) = D(s+g) − S(s)`. The used supply, born before `y`, is

    U(y) = S(y) + m(y),   m(y) = min_{s ≤ y} G(s)

(the Skorokhod reflection: unmet demand waits for later supply). The k-th demand meets the k-th used supply, both sides rounded with one offset `u`.

**Lemma Q1 (no cohort over-used).** `0 ≤ U(y+1) − U(y) ≤ S(y+1) − S(y)`, and `U(y) ≤ D(y+g)`.

Proof.
- `m` is nonincreasing, so `ΔU ≤ ΔS`.
- If `m` drops at `y+1`, then `m(y+1) = G(y+1)` and `m(y) ≤ G(y)`, so `ΔU ≥ ΔS + G(y+1) − G(y) = D(y+1+g) − D(y+g) ≥ 0`. Otherwise `ΔU = ΔS ≥ 0`.
- `U(y) = S(y) + m(y) ≤ S(y) + G(y) = D(y+g)`. ∎

**Lemma Q2 (rounded, per cohort).** `⌊U(y+1)⌋_u − ⌊U(y)⌋_u ≤ ⌈S(y+1) − S(y)⌉`. So a pool of `⌈ΔS⌉` people (or more) always holds what the queue takes from a cohort.

Proof: the difference of floors of two reals at distance `δ` is at most `⌈δ⌉`, and `δ = ΔU ≤ ΔS`. ∎

**Lemma Q3 (both sides end equal).** If supply outpaces demand from every point on, `S(∞) − S(s) ≥ D(∞) − D(s+g)` for all `s`, then `G` attains its minimum at the end, `U(∞) = D(∞)`, and the rounded totals agree. Every demand rank `k` then meets exactly one supply rank, and conversely.

A sufficient local condition: the supply density is at least the demand density, `S'(s) ≥ D'(s+g)` everywhere (integrate from `s` to the end). This is why every pool's supply holds a floor above its demand. ∎

**Cost.**
- `D` and `S` take O(log knots) each.
- `m(y)` takes O(1): the running minimum stored at each knot (about 50 numbers per queue, derived from the pack), plus inside the current piece the minimum of a quadratic, at an end or at the vertex.
- Year of a rank: a search over years, started at the matching demand year shifted by `g`.

## 4. Pools and the cohort layout

Each queue `Q` has a supply cumulative `S_Q`, whose density is a closed-form share of each cohort's expected adults.

A cohort's **pool** for `Q` has `P_Q(y) = ⌈S_Q(y+1) − S_Q(y)⌉ + 1` people. By Q2 the pool holds what `Q` takes from `y`. Its first `U`-difference people are matched; the rest stay unmatched (never partnered through `Q`).

**Layout of a sex in cohort `y`.** Disjoint consecutive segments of its life order:

    [died young][first-union segment][pool_1]…[pool_n][rest]

**Constraint F (fits).**

    young + first + Σ_Q P_Q ≤ n_sex(y)

Sufficient: the shares of the segments' expected sizes sum to at most `1 − ε`, where `ε·adults` covers the roundings (each `+1`, one per segment) and the splines' interpolation error. With the pack's shares, `ε = 0.1` holds at prototype scale (all 351 cohorts checked).

Checked on every cohort of both packs (the prototype and the tiny world, whose cohorts hold a few hundred people).

## 5. Separation and re-partnering

**Groups.** A couple belongs to the group of its demand-side cohort: the wife's, for first unions; for a re-partnering, the cohort of the couple it came from. Within its queue, each couple has a rank in its group.

**Separation is bottom-up.** Couple `i` of group `y` in queue `q` separates iff `i` is a member of a Beatty set with the pack's share `d(q, y)`. Counts over any rank range are closed form.

**Candidates.** For sex `σ` and level `ℓ` in group `y`, the candidates are the separated couples of a fixed list of source queues (§6), concatenated: `L = Σ` (separated count of each source). Every count is closed form.

**Re-partnering is top-down.** The re-partnering queue's demand for group `y` is `T = ⌊D(y+1)⌋_u − ⌊D(y)⌋_u` (its own spline). The re-partnering candidates are a rational Beatty set with exactly `min(T, L)` members over the `L` candidates.

**Lemma R (duality).** Event `e < min(T, L)` comes from candidate `select(e)`. Candidate `j` re-partners iff it is a member, and then it is event `count(j)`. Events `e ≥ L` are **phantoms**: they have no candidate, and their first-time partner's couple is void.

Proof: rational Beatty count and select are mutually inverse on members (Galois; `Lipschitz.lean`, `select_galois`). ∎

**Expected fit.** Choose the spline so that `E[T] ≤ ρ·E[L]` with `ρ < 1`: count the group's first-union couples alone, which are candidates in every group, times separation times `ρ`. Phantoms then arise only in cells where `E[L]` is a few units. Measured on the prototype: the tightest group is at 1.000 of its candidates, in the founder cohort 1750.

## 6. Acyclicity

**Queues.**
- `One`: first unions.
- `W0, W1, W2`: women's 1st to 3rd re-partnerings, with first-time men.
- `M0, M1, M2`: the same for men.

**Sources.** For a couple of group `y`, its source is a separated couple of the same group:
- `W0` takes from `One`, and also from `M0, M1, M2` (a woman whose first union was with a re-partnering man) **iff `y` is odd**;
- `M0` takes from `One`, and also from `W0, W1, W2` **iff `y` is even**;
- `Wℓ` takes from `W(ℓ−1)`, and `Mℓ` from `M(ℓ−1)`, for `ℓ ≥ 1`.

**Theorem A (depth bound).** A couple's times (start, separation) are defined by recursion over sources. With the parity rule, every step moves to a queue strictly earlier in one of these orders:
- odd `y`: `One < M0 < M1 < M2 < W0 < W1 < W2`;
- even `y`: `One < W0 < W1 < W2 < M0 < M1 < M2`.

So the recursion ends within **6 steps**.

Proof: check each edge against the order. ∎

**Without the rule**, `W0 ← M·` and `M0 ← W·` hold in the same group, and two couples can each be the other's source. The prototype found exactly such a 2-cycle (an infinite recursion).

**Corollary (union order).** A couple's start is its source's separation plus a positive delay, and a separation is after its couple's start. So a person's unions are strictly ordered in time.

## 7. Kin repair, and why it does not blow up

**Repair.** In each queue, couples pair as `(k, k ⊕ 1)` by demand rank, **within a group**: `k ⊕ 1` is `k`'s mate only if both have the same demand cohort. Otherwise `k` has no mate. If either couple is possibly close kin (`K`, below), and exchanging the two supply partners parts both, they are exchanged. The predicate is symmetric in the pair, so the map is an involution and both partners agree (`Transport.lean`, `repair_involution`). A pair that cannot be parted is void for both.

**Theorem I (times are repair-free).** A couple's start and separation do not depend on any repair decision.
- The demand side's times come from its source's times (§6), not from identities.
- The adulthood bound on the start uses **both** candidate supply partners of the pair (its own default and its mate's), so it is the same whichever way the pair goes.

**Husband candidates (repair-free).** Repair exchanges supply partners only, never the demand side. So a couple's husband after repair lies in a set computed with no repair:
- in `One` and `Wℓ` the husband is the supply side: `H(c) = {default supply partner of k, of k ⊕ 1}`;
- in `Mℓ` he is the demand side, the man of the couple he came from: `H(c) = H(source(c))`. By Theorem A this recursion ends within 6 steps.

A person's **conception couple** is the mother's couple whose window holds the conception. The mother comes from the births map, and windows come from `times` (Theorem I).

Which chain is the mother's depends on repair only when she sits on a **supply side** of her first union (a bride of a man's re-partnering, or of an immigrant man). Her default couple `k₀` (her supply rank, unshuffled) or its mate `k₀ ⊕ 1` is then hers, by repair.

**Candidate fathers.** `F(x)` is the union, over the mother's possible first couples (one, or `{k₀, k₀ ⊕ 1}`), of `H(c)` for the couple `c` of that chain whose window holds the conception.
- A chain's later couples follow from its first couple alone (§5), so `F(x)` is repair-free.
- `|F(x)| ≤ 4`.
- The actual father, if any, is in `F(x)`.

**Demand candidates (repair-free).** A couple's demand side is the person re-partnering, or the first-union wife or arrival.
- When they sat on a **supply side** of their source couple (a bride of a man's re-partnering, a groom of a woman's or of a first union), who they are depends on the source's repair.
- They are then one of the source pair's two default partners.
- Otherwise they are the source's own demand side, recursively.

So the demand side lies in a repair-free set `D(c)` with `|D(c)| ≤ 2`. By the parity rule a chain crosses to a supply side at most once.

**The repair predicate: possibly close kin.** `K(w, h)` holds if any of these does:
- `w` and `h` share a mother;
- `w` is `h`'s mother;
- `h ∈ F(w)` (possibly her father);
- `F(w) ∩ F(h) ≠ ∅` (possibly paternal half-siblings).

**Lemma K.** If `w` and `h` are close kin (siblings, half-siblings, parent and child), then `K(w, h)`. Proof: mothers are exact, and every father is in `F` of his child. ∎

**Theorem A′ (the whole dependency graph is acyclic).** A couple's identities depend on:
- its source (§6);
- its own repair, which reads its mate's demand side, so its mate's source.

Source edges stay in one group and raise the rank (Theorem A). A mate is in the same queue and the same group. So every chain of dependencies alternates "same rank" (a mate) with "lower rank" (a source), and ends within 2 × 6 steps.

Without the within-group rule, a pair can straddle cohorts `y` and `y + 1`. These have opposite parity, so their rank orders disagree, and the chain can loop. The prototype found such loops on 3 of 14 tiny-world seeds (a stack overflow in `info → repaired → mate's source → info`).

**Repair decides on candidates.** Pair `(k, k ⊕ 1)` is exchanged iff some candidate couple of either is possibly close kin, and in neither exchanged couple is any candidate possibly close kin. The candidate couples are `D(k) × {h}` and `D(k ⊕ 1) × {h'}`.

The same pair is decided from either side, so it is still an involution. The actual demand side is in `D`, so Lemma K carries over: no actual close-kin couple survives.

**Theorem R (repair is shallow and exact).**
- `K` and `D` read only repair-free facts (mothers, `F`, default partners), so deciding a pair never resolves another repair. Repair costs `O(1)` lookups beyond the source chains (at most 6 steps), with no recursion through generations.
- Three earlier versions recursed:
  - one resolved actual fathers on a candidate match;
  - one found a supply-side mother's couple by repair;
  - one found a supply-side demand person by the source's repair (a branching recursion down the source chain).
- Pairs parted under `K` are parted under close kin (Lemma K), so no union is between close kin.
- Pairs that cannot be parted are void.
- The price: unrelated people who share a candidate father are also kept apart. That happens with probability about `4 / (pool size)`.

## 8. Deaths

A person's death is drawn from their cohort's closed-form survival, conditioned to fall after their **structural events**:
- their first union's start;
- for a woman, her last birth;
- for a man, his last conception in his first couple.

Later unions never constrain it. A re-partnering whose partner is already dead is void for both, who both continue through it.

**Fathers.** A birth's father is the mother's partner at conception, if any. The births map says whether the mother was in its married range (first-union wives married before that age, and arrived couples' wives) or in its single range (§2, §10.5). The windows:
- **Union birth:** the first couple's window opens at its **adult bound**: both candidate partners (and the demand cohort) past 16, the same repair-free bound that postpones the start (Theorem I). This takes in pre-wedding conceptions and starts postponed by the bound, never a minor or someone not yet in the world.
- **Single-range birth:** the first couple's window opens at its **start**. A woman whose first union is as a bride (of a re-partnering man or an immigrant man) sits in the single range, and her births during the marriage have her husband as father. Births before it have none.
- **Later couples:** the window opens at the start.

Two earlier versions were wrong:
- the first window opened at the mother's birth, so non-marital births years before a union went to a future husband, once a ten-year-old;
- only union births had fathers, so brides' children had none.

`father`, `children` and the death bound all use this same window, so they agree.

**Lemma D (fathers alive).** A man is alive at every conception in his first couple: his death is conditioned on them. In a later couple, he is the father only if alive at the conception, which is checked.

## 9. What Lean must check

1. **B:** `⌊j·M/c⌋ = m ⟺ ⌈m·c/M⌉ ≤ j < ⌈(m+1)·c/M⌉` (integers, `c, M > 0`).
2. **Q1:** with `m` the running minimum of `G` and `U = S + m`: `ΔU ≤ ΔS`, `ΔU ≥ 0` when `D` is nondecreasing, and `U ≤ D(·+g)`.
3. **Q2:** `⌊a + δ⌋ − ⌊a⌋ ≤ ⌈δ⌉` for `δ ≥ 0`.
4. **Q3:** if supply after every point covers demand after it, the reflected total equals the demand total.
5. **A:** an edge relation that strictly decreases a rank into `{0, …, 6}` has no path longer than 6 (no cycle).
6. **I:** repair is an involution (done: `Transport.lean`).
7. **R:** rational Beatty count and select are inverse on members (done: `Lipschitz.lean`).

## 10. Immigrants (draft, math before code)

**The obstruction.** A couple's start comes from its demand side (§3). Anyone on the supply side must already be in the world by then. A person who arrives at 40 can't sit in a pool matched to a wedding at 23. So arrivals must never be supply unless they arrive before any match can happen.

**Three kinds of arrival.**
1. **Children** (ages 1 to 14) join their birth cohort. Every event that touches a cohort member comes at 15 or later, so they are always present.
2. **Couples** arrive together. The husband is attached to the wife, so no market is needed.
3. **Single adults** partner only as the demand side of their own queues, so they set the couple's time.

### 10.1 Arrival intensities (exact integers)

Each stream `X ∈ {child, AC (couples, one per wife), SW (single women), SM (single men)}` has a yearly intensity `A_X(e) ∈ ℕ` (fixed point), derived from the pack:
- the arrivals series;
- the couple share;
- the singles' male share `m_s = (m − c/2)/(1 − c)`, so that all arrivals have the pack's male share `m`.

Each stream keeps a prefix-sum array `C_X(e) = Σ_{e' < e} A_X(e')` over the world's years: about 260 integers per stream, independent of population.

**Age bands.** Adult ages are cut into bands `b = [lo_b, hi_b)` (18–20, 20–23, 23–26, 26–30, 30–35, 35–40, 40–50, 50–60, 60–81). Each stream has an integer weight `w_X(b)` per year of age (the pack's Rogers–Castro profile, averaged over the band).

### 10.2 A stream of one cohort

The expected arrivals of cohort `y` in stream `X` before year `e` are

    Φ(e) = Σ_b w(b) · ( C(clamp(e, y+lo_b, y+hi_b)) − C(y+lo_b) ),

an exact integer, nondecreasing in `e`, in `O(#bands)`. The total is `Φ∞ = Φ(y + 81)`.

The stream's **count** `T` is given:
- a keyed rounding of `Φ∞/scale` (couples; singles who never partner);
- or the group count of its queue (singles who partner, §10.4).

Its members are indices `j ∈ [0, T)`, in arrival-year order.

**Lemma P (arrival counts).** Let

    P(e) = ⌊ (2·T·Φ(e) + Φ∞) / (2·Φ∞) ⌋        (Φ∞ > 0).

Then:
- `P` is nondecreasing;
- `P(start) = 0` and `P(end) = T`;
- member `j` arrives in the unique year `e(j)` with `P(e) ≤ j < P(e+1)`.

So "arrived before year `e`" is exactly the prefix `[0, P(e))`, from the person's side and from the count's side alike.

Proof: `Φ` is monotone and the floor of a monotone integer ratio is monotone. At the ends, `⌊Φ∞/(2Φ∞)⌋ = 0` and `⌊(2TΦ∞ + Φ∞)/(2Φ∞)⌋ = T`. ∎

(This is the quantile placement `(j + ½)/T` in integers: member `j` arrives in `e` iff `Φ(e) ≤ (j + ½)·Φ∞/T < Φ(e+1)`.)

`e(j)` is found by bisection over at most 81 years. The arrival time inside `e(j)` is a keyed phase. Nothing in the world compares it with anything finer than its year, except the person's own later times.

### 10.3 Children

Cohort `y`'s child arrivals are `C_y = ⌊c_y + u⌋`, with `c_y = Σ_{a=1}^{14} A_child(y+a)/14` (uniform over ages 1–14). They take birth-order positions `[B_y, B_y + C_y)`, after the births, and have no mother. The bridge spreads them over the life order, so they fill segments like natives. Arrival years come from Lemma P over the 14 years.

**Lemma C.** A child arrival is present at every event the cohort structure gives them:
- a union starts no earlier than the start of the year after they turn 16 (§7);
- a birth comes at age 15 or later;
- if they die young, the death is drawn after arrival (conditioned on `[arrival age, 16)`).

They arrive in year `y + a`, with `a ≤ 14`, before any of these. ∎

### 10.4 Couples and singles

**Couples (AC).** Wife `j` of cohort `y`'s AC stream arrives with her husband, `(y, AC-husband, j)`. His birth year is `y − g_j`, for a keyed gap `g_j` from the pack's age-gap kernel, clamped so that he is at least 18 at arrival.
- The union's in-world start is the arrival. The displayed start is earlier, abroad.
- Separation follows §5, in group `y`, by the wife's rank.

**Singles who partner (SW, SM).** These are the demand side of two queues:
- `IW`: single immigrant women, with first-time native men from pools;
- `IM`: single immigrant men, with first-time native women.

The group is the immigrant's cohort, and the group's count is the stream's `T`.

Member `j` has a **delay class** `k(j) = (j + o_y) mod K`, with `K = 8` and a keyed offset `o_y`. Their union falls in year `e(j) + d_k`, for whole-year delays `d_0 ≤ … ≤ d_7` (quantiles of the pack-free delay law, keyed per cohort), later in that year than the arrival phase. So:
- **married before year `Y` ⟺ `j < P(Y − d_k)`**;
- among members of class `k`, the married are a prefix in class order.

Counting the married before `Y` costs `K` arrival counts plus `O(1)` per class:

    #married(Y) = Σ_k #{ i < P(Y − d_k) : i ≡ k − o_y (mod K) }.

**Theorem I (extended).** The new couples' times depend only on the demand side: an arrival year, a class and keyed phases. The adulthood bound still uses both candidate supply partners. So times stay repair-free.

**Theorem A (extended).** `IW`, `IM` and `AC` have no sources, so they are first unions of rank 0. They feed:
- `W0` (the wife re-partners: `IW`, `AC`, and the native bride of `IM`);
- `M0` (the husband: `IM`, `AC`, and the native groom of `IW`).

These edges all leave rank 0, so the rank argument is unchanged and the depth is still at most 6.

### 10.5 Births to immigrant mothers

Cohort `Y`'s births `B_Y` split into a native part and an immigrant part, `n_imm = ⌊B_Y · φ(Y) + u⌋`. Each part is cut into blocks (kind, age) by the same closed-form cumulative (§2). Native lookups then need only `n_nat`, never an immigrant count.

**Eligible immigrant mothers** of cohort `y = Y − a` at `Y`:
- union kind: AC wives arrived before `Y` (a prefix), then SW members married before `Y` (the class prefixes of §10.4);
- single kind: SW members arrived but not married, then the never-partnering SW arrived before `Y`.

Each part is closed form, and `M` is their sum.

**Spill rule.** A block `(κ, a)` with `M(κ, a) = 0` goes to the other kind at `a`. If both kinds are empty at `a`, it goes to the first age after `a`, cyclically in `[15, 45]`, that has eligible mothers.
- Every nonempty age `a*` then serves itself plus the maximal cyclic run of empty ages just before it, so a mother's children are still one closed-form range of one concatenated list.
- `φ(Y)` is set to 0 for any year with no eligible immigrant mother at any age, so the rule always lands.

`φ(Y)` is derived at build from the realized eligible counts (deterministic in the seed), about 260 numbers.

**Orders (exact both ways).** Fix the mother cohort `y` and the child year `Y`. Let:
- `P_X(e)` be stream `X`'s members arrived before year `e` (Lemma P);
- `o` be the cohort's keyed class offset, so member `j` has class `(j + o) mod K` and the smallest member of class `c` is `r_c = (c − o) mod K`;
- `cnt_c(P) = #{j < P : j ≡ r_c} = ⌈(P − r_c)/K⌉⁺`.

The **union** list is `[AC wives j < P_AC(Y)]`, then for `c = 0…K−1` the class-`c` singles married before `Y`: `j < P_SW(Y − d_c)`, in rank order.

The **single** list is, for `c = 0…K−1`, the class-`c` singles with `P_SW(Y − d_c) ≤ j < P_SW(Y)`, then `[never-partnering women j < P_NW(Y)]`.

Each list's length, a position's person (select within a class: `j = r_c + K·m`) and a person's position are `O(K)` closed forms.

**Spill list.** A nonempty target `(κ*, a*)` serves, in this order:
1. its own block;
2. the other kind's block at `a*`, if that kind is empty there;
3. for `a' = a* − 1, a* − 2, …` (cyclically, while both kinds are empty at `a'`), the blocks `(0, a')` then `(1, a')` that land on `κ*`. Block `(κ, a')` lands on `κ` if `κ` is nonempty at `a*`, else on `1 − κ`.

Mother `m` of the target's `M` takes positions `[⌈m·L/M⌉, ⌈(m+1)·L/M⌉)` of the list (length `L`), as in Lemma B.

**Lemma S (spill duality).** Fix `Y`. Let `N ≠ ∅` be the nonempty ages, and `t(a)` the first element of `N` at or after `a`, cyclically. Then `t(a) = a*` iff every age strictly between `a` and `a*` (cyclically) is empty. So the blocks served by `a*` are `a*` itself and the run of empty ages before it, read off by scanning down from `a*` until a nonempty age. ∎

## 11. Same-sex unions (draft, math before code)

**Queues.** Two more first-union queues, `SW` (women with women) and `SM` (men with men). Each has:
- **demand:** the cohort's same-sex seekers of that sex, a segment of its life order ranked by union age (as `One`'s wives), sized by the queue's group counts;
- **supply:** a pool of the same sex, `gap = 0`, as for the other queues (rounded supply `⌈ΔS⌉`, Lemma Q2).

The expected demand is `adults · union_share · σ(y + 25) / 2`, where `σ` is the pack's same-sex share. Half the same-sex partners are demand and half supply, so seekers and pools of a cohort together hold that share. The first-union wives and the men's first-union pool lose the same expected share.

**Times.** A demand rank's union age is the log-logistic quantile of its rank (as `One`). The start waits for the adult bound of both candidate partners (Theorem I unchanged).

**Separation, no re-partnering.** Separations are Beatty sets of the rank in the group, as for every first union.
- `SW` and `SM` feed no re-partnering queue, so they add no edges and Theorem A is unchanged.
- Debt: people whose same-sex union ends don't re-partner (as in the ledger and the pure world).

**Partners of the same sex.** A person's partner in a couple is "the other one", not "the person of the other sex".

**Kin.** The predicate is symmetric for the same sex:

    K(a, b) = shared mother ∨ mother(a) = b ∨ mother(b) = a ∨ a ∈ F(b) ∨ b ∈ F(a) ∨ F(a) ∩ F(b) ≠ ∅.

Repair pairs `(k, k ⊕ 1)` within a group, as before.

**Fathers.** A woman's same-sex partner is not a father. If the conception couple is `SW`, the birth has no in-world father. In `F`, `SW` couples contribute no candidates, which is still a superset. Births to women in `SW` couples come from the single range: they sit outside the wives segment.

**Layout.** One more pair of segments per sex, after the immigrants' pool:

    [seekers (demand), ranked by union age][pool (supply)]

Constraint F gains the two segments.

## 12. Re-partnering by chain depth (replaces the parity rule)

**The rule it replaces.** §6 kept the source graph acyclic by parity: in odd groups a woman whose first union was with a re-partnering man could re-partner and the mirror case could not, in even groups the reverse. So in half the groups, first-time partners of re-partnering people never re-partnered. Re-partnering levels were also capped at 3 per person.

**Depth.** Every couple has a **chain depth**: first unions (`One`, `IW`, `IM`, `AC`) have depth 0, and a re-partnering couple has its source's depth plus one. Queues are indexed by sex and depth, `W_d` and `M_d` for `1 ≤ d ≤ D`:
- `W_d` (a woman re-partners) takes candidates from every separated couple of depth `d − 1` in the group, of any kind. Its woman may have been that couple's demand side (`One`, `IW`, `AC`, `W_{d−1}`) or its bride (`IM`, `M_{d−1}`).
- `M_d` likewise for men.

So both partners of every separated couple are candidates, each for their own sex's next depth.

**Theorem A″ (acyclic by depth).** Every source edge goes from depth `d − 1` to depth `d`. Depth is a rank that every edge raises, so there are no cycles, and a couple's times recurse at most `D` steps. Mates pair within (queue, group) at equal depth, so Theorem A′ holds with rank `(depth, mate step)`. ∎

**What a person can do.** A person whose first union has depth `d₀` can re-partner `D − d₀` times. A first union at depth 0 gives up to `D` re-partnerings. A bride of a depth-`k` re-partnering couple has `D − k`.

The debt changes from "a third of people blocked, and 3 re-partnerings at most" to "chains deeper than `D` stop". With `D = 4` that affects about `(2ρs)⁴ ≈ 4%` of chains, `ρ` the re-partnering share and `s` the separation share.

**Counts.** Let `C(d)` be a group's expected couples at depth `d`, with `C(0)` its first unions. Each separated couple of depth `d − 1` offers one woman and one man, so:
- `E[W_d] = E[M_d] = ρ · s_{d−1} · C(d − 1)`;
- `C(d) = E[W_d] + E[M_d]`.

The candidates of `W_d` are the separated couples of depth `d − 1`, so `E[T] = ρ · E[L]` with `ρ < 1` (§5 fit).

**Demand candidates.** `D(c)` (§7) still has at most 2 members. A demand side crosses to a supply side only at the source couple of its own sex's chain, and then stops at that pair's two defaults.

## 13. Couples where both partners have histories

**Why.** In §12, every re-partnering person takes a first-time partner from a pool. Those partners can re-partner too, so each separation spawns about `2ρs` new couples, all drawing on pools. On the tiny world the pools then took 61% of a cohort's women, and the layout overflowed (Constraint F). In reality about two thirds of remarrying people marry another previously partnered person (Pew 2014: 20% of new marriages have both partners previously married, 20% one).

**Events.** In group `y` at depth `d ≥ 1`, the candidates are the `L` separated couples of depth `d − 1`. Each offers one woman and one man. Each sex has `T_σ = F_σ + R ≤ L` re-partnering **events**, selected by a rational Beatty set (§5):
- `F_σ`: the group's demand in the first-timer queue `W_d` or `M_d` (as before);
- `R`: the group's count of the both-histories kind, one rounding shared by both sexes.

Events `e < R` are **both-histories** events, rank `e`. The rest are first-timer events, rank `e − R`.

**Pairing.** The `r`-th both-histories woman of `(y, d)` partners the `π(r)`-th both-histories man, with `π` a keyed permutation of `[0, R)`. Both sides have exactly `R`, so it is a bijection with no queue and no phantoms.

**Times.** Each event person's readiness is their source's separation plus a keyed delay (repair-free, Theorem I). The couple starts at the later of the two. Separation is a Beatty category of `r` in `(y, d)`, at depth `d`'s share.

**Depth and acyclicity.** Both sources have depth `d − 1` and the couple has depth `d`. Its partners re-partner at `d + 1`, so Theorem A″ holds unchanged.

**Kin.** A both-histories couple is void if its partners are possibly close kin (`K` on each side's repair-free candidates, at most 2 each). It is not repaired:
- a swap would make the partners' identities depend on the repair, and double the candidate sets at every later crossing;
- with no swap, a person behind an event is determined by the event alone, so `D(c)` stays at most 2.

Voids are rare: two re-partnering people of one group, depth and sex pairing, who are siblings or parent and child.

**Counts.** Let each separated partner re-partner with chance `ρ`, a share `q` of them pairing with each other. Then:
- `R = q·ρ·s·C(d−1)`;
- `F_σ = (1 − q)·ρ·s·C(d−1)`;
- `C(d) = 2F + R = ρ·s·C(d−1)·(2 − q)`.

With `q = 2/3`, the branching `ρs(2 − q)` is two thirds of §12's `2ρs`, and pools carry only the `(1 − q)` share.

## 14. Early widowhood as a couple category

**Why.** Today a couple ends by a planned separation or by a death, and only separations feed re-partnering, so no widow or widower ever re-partners. Young widowhood was common before 1950, and young widows remarried at high rates.

**The obstruction.**
- A widow's re-partnering time would be her husband's death. In a first union the husband is a supply side, so which man he is depends on repair, and his death with him: that breaks Theorem I.
- The count of widowed survivors in a group would be a sum over couples' deaths, not closed form.

**Categories.** For a first union `c` of group `y` whose wife is repair-free (`One`, `IW`, `AC`; not `IM`, whose wife is a pool bride), the couples that don't separate are cut by a Beatty tree over their rank into:
- `E_H`, the husband dies early, with share `p_H(y)`;
- `E_W`, the wife dies early, with share `p_W(y)`;
- the rest, late or never widowed, whose deaths are drawn as before and whose survivors don't re-partner (debt: late widowhood).

"Early" means before `T₆₀ = start of year y + 60`, roughly the wife's 60th birthday. The shares come from the cohorts' Siler survival between 30 and 60, among couples that don't separate.

**Deaths and the widowhood time `ω(c)`.**
- `E_H`: a couple-level time `τ_c`, drawn from the male curve of the husband's default candidate's cohort, conditioned to `(start, max(T₆₀, start + 1y))`. The husband's death **is** `τ_c`, whoever he is after repair. The wife's death is conditioned to fall after `τ_c` (and after her births). `ω = τ_c`.
- `E_W`: the wife's death is conditioned to `(L_w, max(T₆₀, L_w + 1y))`, where `L_w` is her usual floor (start, adulthood, last birth). The husband's death is conditioned to fall after hers. `ω` is her death.

**Lemma W (times stay repair-free).** In both categories `ω(c)` depends only on the couple's rank, its start, and the wife's own facts. All of these are repair-free (Theorem I, and the wife is the demand side or fixed). So a survivor's re-partnering time, `ω + delay`, is repair-free, and Theorem I extends. ∎

**No cycles.**
- A husband's death in `E_W` reads his wife's.
- A wife's death in `E_H` reads `τ_c`.
- Neither reads the other's in return, and a wife's own death never reads her husband's.

**Candidates.** At depth 1, each source's candidates are:
- for women: its separated couples, then its `E_H` couples (their widows);
- for men: its separated couples, then its `E_W` couples (their widowers).

So `L_w` and `L_m` can differ. Both-histories pairs (§13) take rank `r < R` only if `r` is real on both sides; otherwise the couple is void for both. Event counts add `ρ·(p_H or p_W)·C(0)`.

**Fathers.** A couple's window for fathers ends at the earlier of its separation and `ω`. A conception after a husband's early death has no in-world father, unless a later couple of the widow spans it. Candidate fathers `F` keep their wider windows, still a superset (Lemma K).

## 15. Arriving families: children choose mothers who arrive the same year

**Why.** Children who arrive (ages 1–14, §10.3) join their birth cohort with no parents here. Couples arrive without children. Real flows are mostly families: couples bring two or three children each, which is the pack's own intent.

**Construction.** Fix a child cohort `y` and an arrival year `e`.
- By Lemma P, the cohort's child arrivals in year `e` are a contiguous range of `c_e` members.
- They are cut into blocks by mother's age `b ∈ [15, 45]`, by one rounding of a closed-form cumulative (the union births' age curve of year `y`, §2): `start(b) = ⌊c_e·G_y(b) + u⌋`.
- Block `b`'s children choose among the `M_b` wives of couples of cohort `y − b` arriving in year `e`. That is again a contiguous range, by Lemma P on the couples' stream. Child `j` of the block takes wife `⌊j·M_b/c_b⌋` (Lemma B).
- A block with `M_b = 0` spills to the next age with wives, cyclically in `[15, 45]` (Lemma S).
- If no couple's wife of any age arrives in year `e`, the children stay parentless.

**Duality.** A wife of cohort `y′` arriving in year `e` has, for each `b` with child cohort `y = y′ + b` and `1 ≤ e − y ≤ 14`, the children in her range of block `b` (and of the blocks spilling into it). Each side reads the same block counts, `M_b` and spill runs, so `mother` and `children` are inverse, as in Lemma B.

**Facts.**
- The child arrives at its mother's arrival time, in the same year by construction.
- It is born in year `y`, at the mother's phase (siblings whole years apart, as natives).
- The father is the arriving husband if the conception falls in their couple's window (§8): conceptions before the couple's start have none.
- The mother's age at the birth is in `[15, 45]`, and she is alive then: she was born `b ≥ 15` years before, and she is alive on arrival, after the birth.
- These births are not the mother's births here: she is not in the births map for them, and her death needs no new floor.

## 16. Groups (heritage) as union spaces

**Requirement.** People belong to heritage groups `g = 0…G−1` (the pack's, five for the US). Partners mostly share a group, and outmarriage follows the pack's open-market shares by era and sex. Children take their mother's group. Composition follows from group births, deaths and arrivals.

**Why a label can't be laid over a group-blind world.** Any labelling of a group-blind union graph either mixes partners at random (intermarriage near `1 − Σ share²`, about 25% in 1900 against 1–3% real) or lets one label take over. The research note behind decision N1 shows this, so groups must shape the markets.

**Index spaces.** Every index space of §1–§15 gains a group:
- native cohorts `(y, g)`;
- arrival streams `(y, g)`, split by the pack's immigrant shares;
- founders, by founder shares.

Births of year `Y` in group `g` are `B(Y)·β_g(Y)`. Here `β_g` is derived once from expectations: each group's expected women aged 15–45 at `Y` times its fertility factor, normalized. The expected women come from the same renewal over expected births, arrivals and the group's survival. That is a few thousand numbers, none population-sized. Children of `(Y, g)` choose mothers among the women of `(Y − a, g)` (Lemma B per group).

**Union spaces.** A union happens in one space `U ∈ {g₀, …, g_{G−1}, O}`:
- space `g` holds unions inside group `g`;
- space `O`, the **open market**, holds unions where partners meet regardless of group.

Each space has its own queues (first unions, re-partnering by depth, both-histories pairs, immigrants' singles, same-sex). Its pools sit in its groups' cohort layouts. A person's first union is in their group's space or in `O`, by segment: open seekers and open pools are a share `o_g^σ(y)` of the group's first-union demand and supply, from the pack.

**The interleaved axis.** Space `O`'s queues run over `x = (y − y_min)·G + g`. Every density is a table over `x`, and its cumulative is an exact sum. Within a year the groups are consecutive, so:
- the k-th open seeker meets the k-th open pool member in `(year, group)` order;
- the block shuffle (§3) spreads partners over about three years of the axis, all groups included. A seeker's partner group then follows the composition of the open supply: minorities mostly marry out, and the majority's outmarriage follows from theirs, which is the pack's intent.

The gap is `G · gap_years`. Lemmas Q1–Q3 hold on any axis.

**Lemma U (spaces are closed under re-partnering).** A separated couple of space `U` offers its partners as candidates for space `U` at the next depth. Its first-timer pools are `U`'s, and its both-histories pairs stay within `U`. So every candidate count of space `U` at depth `d`, anchor `a` (a year for a group space, an axis point for `O`) is a Beatty count over `U`'s own queues, closed form. The people of an open couple may belong to different groups, but no count depends on which. ∎

**Consequences.**
- People who once married out re-partner in the open market. That's a modelling choice, plausible since intermarriage persists, and it keeps Lemma U.
- Births eligibility: a group's married range is its in-group wives plus its open seekers, two segments each ranked by union age, so "married before age a" is two prefixes.
- Kin repair swaps within a space's queue pairs. In `O` a swap may exchange men of two groups, which changes no count (Lemma U).
- Theorems A″, I, R and Lemmas W, P, S hold per space, unchanged.

**Debt (v1):**
- children take the mother's group, with no "two or more races" (names can draw on both parents);
- single immigrants partner within their group's space only;
- same-sex unions stay within groups.

### 16.1 Thin groups: three lemmas the build needed

Splitting the world into groups makes some cohorts tiny (a group's early years) and some groups immigrant-heavy (Asian 1940–1980, Hispanic later). Three facts that never bound in the one-group world now do.

**Lemma C (layouts fit).** A cohort's layout for one sex holds `K = 2 + POOLS` segments (first unions, same-sex seekers, the pools), each sized by a queue's yearly count. A demand count is `⌊D(y+1)+u⌋ − ⌊D(y)+u⌋ ≤ ΔD + 1`, and a used supply is at most `room = ⌈ΔS⌉ ≤ ΔS + 1` (Lemma Q2; room is 0 when `ΔS = 0`, since then the used supply can't step). So if the densities of a cohort's segments sum to at most `A − K − ε`, where `A` is the cohort's realized adults of that sex, the counts sum to less than `A`. The build scales every segment density of cohort `y` by

```text
λ(y) = 1                    if Σ e_i ≤ A − K − ε
     = (A − K − ε)⁺ / Σ e_i otherwise,
```

with a knot every year so the densities can follow `A(y)`. In any cohort of a few dozen or more, `λ = 1`. ∎

**Lemma V (unmatched couples are void).** Lemma Q3 makes used supply equal demand when supply after every point covers demand after it. A thin group's queue can fall short, and the shuffle then maps some demand ranks to supply ranks `s ≥ U_total`, where no person exists. Such a couple is void: its times are undefined, so both sides skip it (the demand side finds no couple; no supply person has that rank). Repair pairs only couples whose two supply ranks both exist, so `mate` stays an involution. A void couple's events become phantoms (§5), which already void the couples downstream. ∎

**Lemma R (rationing the demand on first-timers).** A cohort's ever-partnered first-time women `E` serve three demands: the group's first unions, the brides of re-partnering men, and single immigrant men. Where immigrants dwarf the natives, the last two (`B`) can exceed `E`. In the US pack, `B/E` is 0.1–0.35 for White, Black and AIAN, 0.4–0.95 for Hispanic, and 1.1–3.6 for Asian in 1940–1980. Then the first-union residual `E − B` is negative and the group's natives never marry each other. Rationing with

```text
ρ(y) = min(1, θ·E(y) / B(y)),   θ = 0.75,
```

applied to the re-partnering events' densities (by the first-timers' cohort `y − gap`) and to the single-arrival streams, keeps natives' first unions at least `(1 − θ)E`. For arrivals, the rationed share moves from the "partners here" stream to the "doesn't" stream: the cumulatives become `⌊φ_single·ρ_q/2¹⁶⌋` and `φ_alone + φ_single − ⌊φ_single·ρ_q/2¹⁶⌋`. Both stay exact integers and nondecreasing, so Lemma P still places every member, and the arrivals' total is unchanged. `ρ = 1` exactly wherever it doesn't bind, so the one-group world is untouched there. It is a structural bound on a rate, not a rule on people; the open market (space `O`) and immigrants partnering each other are the realistic fix. ∎

**Measured** (us, five groups; White/Black/AIAN/Asian/Hispanic, % of the living): 1970 82.5/11.3/0.5/0.6/5.1 against the Census's 83/11/0.4/0.8/4.5; 2000 72.6/13.0/0.5/2.9/11.0 against 69/12/0.7/3.8/12.5; 2020 66.4/13.5/0.5/4.6/15.0 against 58/12/0.7/6/19 (plus 4% two or more races). Too few arrivals after 1990 is the known foreign-born debt (8.3% in 2000 against 11.1%).

### 16.2 The open market, concretely

**Space `O`** (index `G`) has its own first-union queue (open wives with open grooms), first-timer re-partnering queues `W(l)` and `M(l)`, and both-histories lines `R(l)`. Arrival couples, single immigrants and same-sex unions stay in group spaces (debt). Its queues run on the **year axis**. A year's open demand and open supply are the sums of the groups' open densities, and within a year the groups are concatenated in order. The block shuffle (§3) permutes partners within about three years of unions, so a seeker's partner group follows the composition of the open supply around them, as the interleaved axis would.

**Group shares of a year's counts.** `O`'s yearly integer counts (wives demand; used supply of each queue) are split across groups by keyed systematic apportionment of the groups' densities at that year. That gives integers `n_g(y)` summing exactly to the queue's count. Each part is at most its density plus 2: the count is at most `Σd + 1`, and the share of that `+1` is at most 1, plus 1 for the rounding. So Lemma C extends with a slack of 2 per open segment. A table per year (about 50 integers) and its prefix sums decode a year's rank `r` into `(g, r_g)`.

**One wives segment per cohort, split by an exact Beatty set.** A group cohort's first-union wives are `W = q1_g + n_g`: in-group wives (its group queue's demand) and open wives (its share of `O`'s). All `W` are ranked by union age as before. An exact rational Beatty set `RationalBeatty { t: n_g, len: W, τ }` marks which positions are open: count, select and complement select are closed forms. So:
- "married before age `a`" is still a prefix of the cohort's wives, and births don't change;
- the in-group queue's `r`-th wife is the `r`-th non-member, at `⌊(rW + τ)/(W − n_g)⌋` (complement select);
- the open queue's `r_g`-th wife of group `g` is the `r_g`-th member.

**Men's open pools** are new segments of each group cohort's men (an open first-union pool, then an open pool per re-partnering depth), sized exactly by the used supply's group shares, so all their members are matched. Women's open pools serve `O`'s re-partnering men in the same way.

**Expectations.** For each group and sex, `O`'s density is the open share `o_g^σ(y)` times the group's own: open wives `o^F·Q_g(y)` (`Q` the first-union wives), open grooms `o^M` times the men's first-union pool, and open re-partnering pools likewise. The group space keeps the `1 − o` parts. `O`'s events come from its own first unions (`Σ_g o^F·Q_g`). Its early-widowhood shares are the groups' shares weighted by their open wives.

**Lemma U** holds as stated: `O`'s couples re-partner in `O`, and every count is a Beatty count over `O`'s queues.

**Check.** With every open share 0, `O` is empty, `n_g = 0`, the Beatty set has no members, and the world is the group-space world bit for bit.

### 16.3 Integer layouts (replaces Lemma C's reserve)

Lemma C fits layouts by reserving the worst-case rounding (one per segment, two per open segment), and a reserve of 18 a cohort wipes out small groups' unions. Integers avoid any reserve.

**Allocation.** For each group cohort and sex, with `A` realized adults and expected segment sizes `e_i` (wives in-group and open, same-sex seekers, every pool in-group and open):

```text
T = min(A, ⌊Σ e_i + u⌋),   c_i = keyed systematic apportionment of T by e_i,
```

so `Σ c_i = T ≤ A` exactly, and each `c_i` is within one of `T·e_i/Σe`.

**Queues on integers.** A segment that is a queue's demand or supply is that queue's density at the cohort's year, a knot every year. Its prefix sums are then exact integers.

**Lemma C′ (integer supply is never over-used).** With integer supply `S` and any demand `D`, define the used supply as `U(y) = S(y) + ⌊m(y) + u⌋`, where `m(y) = min_{s≤y}(D(s+gap) − S(s))`. This is the same quantity as `⌊S + m + u⌋`, written so that floats can't move it. Then:
- `U(y+1) − U(y) = ΔS(y) + (⌊m(y+1)+u⌋ − ⌊m(y)+u⌋) ≤ ΔS(y)`, since `m` is nonincreasing;
- the step is `≥ 0`, since `S + m = min(S(y), D(y+gap) …)` is nondecreasing.

With integer demand, `dcum = D` exactly. So every pool holds exactly its allocation `c_i`, of which the queue uses a prefix, and a cohort's layout is exactly `young + Σ c_i ≤ n`. ∎

**The open market on integers.** `O`'s densities at year `y` are the sums of the groups' open allocations, so a year's open wives are the groups' integers concatenated: no apportionment, and a rank decodes by prefix. An open pool member at offset `r` among its year's pools (group by group) is matched iff `r` is below the year's used supply.

### 16.4 Re-partnering seeks in the open market too

The pack's open share is the share of *all* of a person's partner seeking, so separated and widowed people re-partner openly at their group's share as well. (Before this, only first unions could be open, and recent intermarriage came out at about half of Pew's.)

**Split.** For a group space `g`, sex `σ`, depth `l` and group year `y`, let `L_g` be the candidate list (separated, then early widowed, source by source; length `len_g`). An exact Beatty set `B_g = RationalBeatty { t: ⌊o·len_g + u⌋, len: len_g, τ }`, with `o` the group's open share at the re-partnering year (about `y + 25 + 10(l + 1)`), marks the open candidates.
- **Group `g`'s candidates** are the non-members: `E_g = len_g − t_g`, and its `j`-th candidate is `select_out(j)`.
- **`O`'s candidates** are its own list, then each group's members in group order: `E_O = len_O + Σ_g t_g`. Its candidate `len_O + Σ_{h<g} t_h + r` is `B_g.select(r)` in `L_g`.

Events in each space are a Beatty selection of `min(rr + demand, E_u)` among its `E_u` candidates, as before (§5). The reverse map (`next_couple`) takes a separated couple at position `p` of its space's list: a member of `B_g` becomes `O`'s candidate `len_O + Σ_{h<g} t_h + count(p)`, and a non-member stays as `g`'s candidate `p − count(p)`.

**Acyclicity.** Every edge still raises depth: `O`'s depth-`l` events come from depth-`l − 1` couples of `O` or of groups. Lemma U becomes "`O`'s counts are closed forms over `O`'s and the groups' queues", which is enough: no count depends on a resolved person.

**Expectations.** A group's events lose its open share, `(1 − o)·E_g`; `O`'s events gain `Σ_g o·E_g`. Both-histories counts use the mean of the two sexes' shares. `O`'s pools were already sized from the groups' total events times the open share.

### 16.5 Single immigrants seek openly; pools mirror their demand

**Open singles.** A group's single arrivals who partner here (`SINGLE_W`, `SINGLE_M`, `n` per cohort, Lemma P) are split by an exact Beatty set with `t = ⌊o·n + u⌋` members, at their group's open share.
- **Members** are the demand of `O`'s immigrant queues: `O`'s yearly count is `Σ_g t_g`, member `i` of group `g` has rank `Σ_{h<g} t_h + count(i)`, and a rank decodes back by prefix and `select`.
- **Non-members** are the group queue's demand, an integer `n − t` a year, with rank `i − count(i)` (inverse: complement select).

The stream's count is now its own keyed rounding `arr_n`, not the queue's: the queue takes integers.

**Pools mirror their demand.** A pool for the other sex's re-partnering or single arrivals is sized from that demand's expectation, so its open part is the demand side's open share, not the pool's own sex's.
- Splitting by the pool's sex left a group space short wherever that sex was more open than the demand: Asian native women (open 0.45–0.70) against Asian immigrant men (0.27) left 25% of the group's immigrant men unmatched (Lemma V voids).
- The men's first-union pool keeps the men's own share, which is where the pack's sex asymmetry in first unions lives.

**Measured** (`us`, natives, newlyweds across groups): 2.7% in 1965–69 (Pew 3%), 4.2% in 1978–82 (6.7%), 11.6% in 2010–15 (17%). By group in 2010–15: White 8 (11), Black 18 (18), AIAN 48 (58), Asian 38 (29 overall; US-born Asians intermarry more), Hispanic 25 (27). Every queue balances.

## 17. Areas (design, math before code)

**Requirement** (the founder's "B at area level" for the ledger, research `2026-10-01-residence-*`):
- people live in areas: 62 groupings of commuting zones (`us-areas`), or the `us` pack's lineage regions;
- most partners come from the same area, with a few percent across areas (one partner moves);
- children are born in the mother's area;
- natives move between areas: 21–34% live outside their birth state, rising over the century.

**The cost to avoid.** Areas as union spaces, like groups, would multiply every queue and table by the number of areas: about 62 × 5 spaces, roughly 40–80 MB of per-year tables. That is still flat in population, but it gives up the cell world's small state, and the build would grow to seconds.

**The idea: areas are runs, matched locally inside national queues.** Queues stay per group space and `O` (§16). Areas live in two places only:

1. **Cohort layouts with area runs.** Every segment of a cohort (sex × segment) is split into runs by area: an integer contingency table `T[segment][area]` with exact margins. The rows are the segment sizes (§16.3); the columns are the cohort's adults by area of union (below). It comes from `partition::contingency_systematic`: a few hundred integers per cohort, derived on demand or stored. A person's position decodes to (segment, area, rank in run) by prefix sums, and back.

2. **Labelled block matching** (new primitive). The queue still maps the k-th demand to the k-th used supply, and the shuffle permutes within blocks of about three years of unions. Within a block, instead of a free permutation:
   - each side's ranks carry an area label (from the runs);
   - the first `m_a = min(D_a, S_a)` demand and supply ranks of area `a` in the block pair by a keyed permutation of `0..m_a`;
   - the rest (each area's excess, plus a "national" Beatty share `ν` of each side) form two residual lists of equal length, paired by a keyed permutation. These are the cross-area unions.

**Lemma M (minimal crossing).** Any bijection between the block's demand and supply pairs at least `Σ_a max(0, D_a − S_a)` demand ranks across areas. The construction achieves that, plus the national share. So with similar compositions on both sides, nearly every couple is local, and the national share controls inter-area unions directly (ledger: 6–10%).

**Closed form.** A rank's area, its rank among its area in the block, and `D_a`, `S_a` are interval intersections over the 2–4 cohorts a block spans: O(cohorts) for a local pair. A residual pair needs every area's counts, O(areas × cohorts), about 250 operations for about 10% of couples.

**Migration of low rank.** A cohort's adults by area of union come from births by area plus migration:
- **stayers:** a Beatty share `1 − m_a(y)` of area `a`'s natives;
- **movers:** all areas' movers concatenated in area order, then re-split over destinations by apportionment with weights `w_b(y)`. That is rank 1: destination independent of origin. A block-rank variant adds a within-region share for distance.

The matrix is diagonal plus rank 1, so a mover's destination and rank there are a prefix decode over areas: no `areas²` table. A mover moves at a keyed age between adulthood and their first union, and lives in their birth area until then.

**Births by area.** A year's births of a group split over areas by apportionment, weighted by each area's eligible mothers times fertility. So the birth order is in area runs, and a child of area `a` chooses among area `a`'s mothers (Lemma B per area run). "Married before age `a`" is a prefix of each area's wives run, with union ages by quantile within the run. The bridge from birth order to life order permutes within each area: stayers to their life positions, movers through the national mover list.

**Re-partnering** stays in the couple's area: events are labelled by their source couple's area, and the event queues match by area in the same way.

**v1 debts:**
- no couple moves (couples live in their area of union);
- destinations of rank 1 (or region blocks);
- no return migration.

**Next:** the labelled block matching as a core primitive, with Lemma M in Lean and a brute-force test. Then a prototype on two areas (the `us` lineage regions) before 62.

### 17.1 Runs everywhere, and a v1 without migration

Labelled block matching needs every list a queue matches over to come in area runs, so a block's area counts are interval intersections:
- **Wives and pools:** cohort segments split into runs by a contingency table (above). With the open Beatty set over wives (§16.2), a group queue's ranks are the non-members, and the area-`a` ranks among them are the run's positions minus its members: one Beatty count per run end.
- **Re-partnering candidates:** order each list by area first, then source and kind. Within a source group the couples are already in their demand person's order (area runs, recursively for re-partnering sources), and the separated or widowed among them are Beatty sets. So area `a`'s candidates are Beatty counts within runs, and events (a Beatty selection over the list) inherit the runs.
- **Arrivals:** each stream splits by arrival area through an apportionment of its count (exact integers; Lemma P per run).

**Lemma M** is proven (Lean `proofs/2026-10-02-cell-world-areas.lean`). The construction is tested against brute force (`procedural_core::matching`).

**v1, the smallest exact version.**
- Women live in their birth area; a couple lives in the wife's area. A husband from another area (the residual and national share) moves there at the union.
- Births by area follow women, so they need no migration bookkeeping. Single moves (rank 1) come in v2.
- **Check:** with one area, everything must reproduce the current world bit for bit (the matching with one label and no national share is one keyed permutation per block). The staggered two-level shuffle is replaced, so the reproduction holds only against a one-level variant; build that variant first and verify it.

### 16.6 Interleave a year's open ranks (a bias, fixed)

A queue uses a *prefix* of each year's supply ranks. With `O`'s yearly ranks being the groups' parts concatenated in group order, that prefix took the first groups' open pools and left the last groups' unused whenever supply exceeded demand. Asian and Hispanic grooms and brides met the open market less than their shares said.

The year's ranks are now an exact interleave over the groups (`lattice::ExactInterleave`, §17): a balanced tree of exact rational Beatty splits by the groups' part sizes. Every group gets exactly its part, and any prefix holds each group within the tree depth of its share, so the used prefix draws on every group evenly.

Measured (`us`, natives, newlyweds across groups, 2010–15): 11.6% → 12.9%. By group: White 8, Black 19, AIAN 55, Asian 39, Hispanic 29 (Pew: 11, 18, 58, 29 overall, 27).

### 17.2 What the area build showed

**Built (v1a).**
- `Pid` has an area. People live in cells (group × area), and per-cohort tables are per cell.
- A space's yearly ranks for every part (wives, seekers, singles, couples, pools) are an exact interleave over its cells. The open market nests areas outside groups, so an area's ranks are one category.
- Queues match by area with the labelled staggered matching (Lemma M), with these exceptions:
  - re-partnering events carry no area yet, so they match nationally (debt);
  - supply ranks past the used supply are void.
- With one area, every step reproduced the previous world bit for bit.

**Exact.** With the two regions of `us-tiny`, the exhaustive test passes on 3 seeds. 16% of unions cross areas, mostly re-partnering, which matches nationally.

**Thin cells needed two rules** (62 areas × 5 groups gives cells of a few people):
- a cell with no woman of mothering age in a year has no births that year (`has_mothers`; cohorts are built in year order so a cohort can read its mothers');
- the natives' birth blocks skip mothers' ages with no women. The block cumulative is weighted by "this age has women", so no block is empty; the cross-kind spill in `eligible` already covers an age with one kind only.

**A kin-predicate defect, found by areas and fixed for every world.** The kin facts' conception walk ended a first union only at its separation. When the husband died early (widowhood, §14), a later child of the widow was credited to the dead husband in the kin predicate, so a half-sibling couple through the true father could form. The walk now ends the first union at the widowhood time, as `father` does (repair-free, Lemma W). The sampled answers were unchanged; the bad case is rare.

**Cost of labelled matching.** Every shuffle counts areas within blocks spanning several years: dozens of interleave operations.
- Two areas: lookups about 5× slower (unions p50 6 → 35–40 µs).
- 62 areas: the sample is about 19× slower, and the build takes 3 s.

Caching per-year area prefix sums (0.5 MB for two areas, about 10 MB for 62) and a 64-bit fast path in `RationalBeatty` helped about 1.7×.

### 17.3 Alternative: areas as sub-spaces (design, for the speed/state trade)

Labelled matching keeps state tiny but makes every shuffle count areas over multi-year blocks. The alternative makes each area its own queue system:
- **Spaces** are (group or `O`) × (area or national): `(G + 1)(A + 1)`.
  - A group's area space holds exactly one cell, so it needs no interleave: its yearly ranks are the cell's own.
  - `O`'s area space interleaves the groups of that area (the existing open mix).
  - A national space interleaves areas: each cell's national part, a Beatty share `ν` of its seekers and pools.
- **Matching** is the existing staggered shuffle inside each space, at the one-area cost.
- **Re-partnering** stays in its space (Lemma U per space), so it becomes local without new machinery. Open and national re-partnering seekers go to `O`'s and the national spaces through the §16.4 Beatty splits.
- **State:** per space, about 13 queues × years of (`dcum`, `ucum`, `room`). At 32 bits that is about 55 KB per space and about 21 MB for 62 areas; cohort facts per cell add a similar amount.
- **Build:** independent per space, so it parallelizes.

**Trade:** about 20–40 MB and near one-area lookups, against a few MB and 5–20× slower lookups.

**Measured, 62 areas (`us-areas`), labelled matching:** build 4.2 s; unions p50 128 µs and p99 15.5 ms; father 152 µs / 2.8 ms; death 104 µs / 3.8 ms; children p99 10 ms. That is 20–60× the one-area world, over the founder's roughly 25 µs budget for a one-hop lookup.

**Decision (2026-10-02): areas as sub-spaces.**
- Spaces are numbered so one area reproduces today's world: group spaces are the cells (`c = group·A + area`, one cell each), then one open market per area.
- Each area is then a self-contained world of groups and an open market, so matching, re-partnering and the open splits all run at one-area cost.
- **v1:** no cross-area unions; areas are linked only through the births and arrivals split.
- **v2:** a national market (a Beatty share of each cell's seekers and pools; national spaces interleave areas), then migration.
- The labelled matching stays in core as a tested primitive, for the national market's residual if needed.

### 17.4 Migration before a first union (design, math before code)

**Requirement.** Natives move between areas: 21–34% of the living live outside their birth state, rising over the century. Partners mostly meet where they live, so cross-birth-area couples come from migration, and a national market may be unnecessary.

**Two cells per person.**
- The **birth cell** `(h, a)`: where the person is born and grows up, and where their mother chose them (Lemma B).
- The **life cell** `(h, b)`: where they live from their move (or from adulthood) on, partner, re-partner and bear children.

Stayers have `b = a`.

**Exact streams** (per group `h`, cohort `y`, sex `s`):
- **Movers:** of birth cell `a`'s adults `A_a`, an exact Beatty subset leaves, `t_a = ⌊m_a(y)·A_a + u⌋`, at the pack's rate of moving between areas before a first union.
- **The national mover list** concatenates `t_a` over areas. It splits over destinations by an exact interleave (§17, `ExactInterleave`) with sizes `I_b` from an apportionment of `Σ t_a` by destination weights `w_b(y)`, rank 1 in v1.
  - A mover's destination and rank among `b`'s in-movers are one `locate`; the inverse is one `select` and a prefix over origin areas.
- **Life cell `b`'s adults:** `L_b = (A_b − t_b) + I_b`: its stayers, then its in-movers.

**Life ranks.** A life cell's adults of a sex are numbered `0..L_b` through a keyed permutation of (stayers, in-movers). The cohort's segments (wives, seekers, pools: §16.3) are allocated over life ranks, so every queue maps to life ranks exactly as it now maps to positions. The young (who die before 16) never move and stay in their birth cell's positions.

**Maps (all closed form).**
- person → life rank: birth position → adult rank → member of the movers' Beatty set? Then national rank → (`b`, in-mover rank); otherwise stayer rank → permutation.
- life rank → person: the inverse chain.

**Births.** Children of life cell `b` choose mothers among `b`'s resident women (life ranks), so a mover's later children are born in her new area. The child's birth cell is her life cell.

**What changes.** Every map between a person and a cohort position goes through birth position and life rank: union slots, births and mothers, kin facts.

**v1 debts:**
- one move per person, before a first union;
- destinations of rank 1;
- no couple moves or returns;
- `Residence` must read the move time to place people.

**Refinement: key natives by their life cell.** `Pid { y, g, a, NATIVE, i }` names the *life* cell, and `i` its life position, so every union map stays as it is. The birth cell is derived. Each birth cell keeps a birth order (children choose mothers among its resident women, Lemma B), and the bridge from birth order runs through three steps:
1. a birth layout (sex, young, adults);
2. stayers or movers (Beatty per sex over adults);
3. for movers, the national list and their destination.

Each life cell's adults (stayers, then in-movers) are mixed by a keyed permutation before its segments are allocated, so in-movers reach every segment. The young never move, so a life cell's young are its birth cell's. Only `origin`, `births_of` and the cohort sizes change. With no movers, every map reduces to today's: a check that must reproduce today's checksums.

### 17.5 A cycle Lemma W left open (found with areas, fixed)

Lemma W makes early widowhood repair-free when the **husband** dies first: his death is a couple-level time `τ_c`. When the **wife** dies first (`Widow::Wife`), the widower's re-partnering read her death through `death(wife)`, and `death` reads her first couple's `info`, which runs kin repair. That closed a cycle:
1. her death needs her couple's repair;
2. repair checks the kin facts of a candidate whose mother's first union was with her widower;
3. that union's start is her widowhood time, which is her death.

The cycle is structural (in the dependency graph, whatever the times) and needs a candidate in her couple's repair pair to be the widower's later wife's child. Thin areas' small pools made that reachable: stack overflow on one of 36 four-area seeds.

**Fix.** Her early death is now a function of repair-free facts only:
- the couple's start (Theorem I);
- her birth, arrival and last birth (the births map, Lemma B);
- her own key.

`wife_early_death(w, c, start)` serves both `death` (same value; checksums unchanged) and widowhood.

**Lemma W′.** Both widowhood times, the husband's `τ_c` and the wife's early death, are repair-free, so re-partnering never reads a repaired couple.

**Also found by areas:** the kin predicate's conception walk ignored the husband's early death (§17.2); the space field in couple ranks overflowed past 255 spaces.

## 18. Lookup speed (2026-10-03)

The cell world's lookups repeat work rather than doing much: kin repair resolves 4–6 people's kin facts per couple, and a person's chain re-resolves itself at every step. This section states the facts that remove the repetition. Each change was checked bit-identical against six worlds' answer checksums.

### 18.1 Lemma K (chain keys)

**Definition.** A person's *chain key* `κ(p)` is `(y, a)`: the demand group year and the area of their first couple. For a supply side, use the default couple of their supply rank. `κ(p)` is undefined if they never partner.

**Fact 1 (chains keep their key).** Every couple of a union chain has the chain's key.
- `next_couple(c, sex)` returns a couple of group `y = demand_group(c).0`: `group_base(u, ·, y)` plus a rank below that group's length. The space `u` is `c`'s, or its area's open market.
- A couple's repair mate shares its demand group (`mate` requires it).
- So every couple reachable from `c` has `c`'s key.

**Fact 2 (candidates live in the chain).** Every candidate husband `F ∈ H(C)` of a couple `C` has first couple `C` or `mate(C)`, or his chain contains `C`. So `κ(F) = key(C)`. By the queue `C` is in:
- **One, W, IW** (the husband is the supply side, a first-timer): `H(C)` is the pair's two default supply partners, whose first couples are `C` and `mate(C)`.
- **M, R** (the husband is a re-partnering demand side): `H(C)` is his event's candidates, the source pair's two men. Their chains contain the source, which is in group `y`, in `C`'s area.
- **AC, IM:** the husband himself, whose first couple is `C`.

**Lemma K.** Let `x` have mother `m`. Every candidate father in `kin_facts(x).fathers` has chain key `κ(m)`.

*Proof.* `kin_facts` walks `m`'s chain from her (possible) first couples to the conception couple `C`. By Fact 1, `key(C) = κ(m)`. By Fact 2, every `F ∈ H(C)` has `κ(F) = key(C)`. ∎

**Corollary.** Let `κ(m_w) ≠ κ(m_h)`, or let either be undefined. Then `w` and `h` share no candidate father. If moreover `κ(h) ≠ κ(m_w)`, `h` is not a candidate father of `w`.

So the predicate "possibly close kin" compares mothers first (one `origin` each). It resolves fathers, the chain walk, only when the keys agree. The keys agree in about 5% of checks. This is the same predicate, evaluated lazily.

### 18.2 Immigrant mothers' windows

1. **Arrival bounds the window.** By Lemma P, an immigrant woman `j` of a stream is among the eligible mothers of year `yc` iff `j < arrived(yc)`. That holds iff `yc > e*`, her arrival year (`arrived(e*) ≤ j < arrived(e* + 1)`). So her births here lie in the child years after `e*`.
2. **Births rise with the year.** A woman's birth time in year `yc` is `year_start(yc) + φ_m·len(yc)`, increasing in `yc`. So her last birth satisfying a monotone filter is the first one met scanning back from the latest year. A husband's death needs only the last conception inside his couple's window, which is such a scan, bounded by the window.
3. **Emptiness needs three counts.** The delay classes `r_c` are the residues mod `K`, a partition of the indices, and married ≤ arrived in each class. So the eligible mothers of both kinds sum to `p_ac + p_sw + p_nw`, the arrived couples, singles and women who don't partner here. The spill lists' "any mother at age `a`?" is that sum `> 0`.

### 18.3 The shuffle with one label

Areas are sub-spaces (§17.3), so every queue side has one label. Then `labelled_staggered` is `staggered_blocks`: the core test `one_label_is_staggered_blocks` asserts it. Its full blocks share one Feistel shape, `a = ⌈√block⌉`, `b = ⌈block/a⌉`, and the division inverse of `a`, which is precomputed per queue.

### 18.4 Memoization is not state

Every memoized function is a pure function of the world and its arguments:
- couple facts, times, deaths, first couples;
- mothers, arrivals, pair repairs;
- immigrant-mother counts and cohort constants.

The memo is per thread, direct-mapped and bounded, and tagged by world instance, with a fresh tag after the build. An entry is a cached value of the function, so no answer depends on what the memo holds. Repeated work disappears: a chain of depth `d` re-resolved its earlier couples at each step, `O(d²)` repairs, and now resolves each once.

### 18.5 A woman's death needs no repair

`death(w)` reads her first couple's start, planned separation and widowhood category (§14), her own births, and her keys. All of these are repair-free:
- times: Theorem I;
- widowhood: Lemmas W and W′;
- births: the births map, Lemma B.

When she is the couple's demand side (first unions, single arrivals' unions, arriving couples), the couple is known without repair too. So her death never runs kin repair. Repair only decides her husband's identity, which her death never reads. A man's death still needs repair, which decides which couple of the pair is his.

### 18.6 One pair decision settles both couples

Repair decides a pair `{k, k ⊕ 1}` as a whole. Swap iff `(r_kk ∨ r_mm) ∧ ¬r_km ∧ ¬r_mk`, a condition symmetric in the two couples. So computing one couple's result gives the mate's:
- swap: each takes the other's default;
- no swap: the mate keeps its own, void iff `r_mm`.

The memo records both.

### 18.7 Couples carry their demand year

A couple's demand year is a function of its queue and rank. Every construction but one knows it already:
- a wife's cohort;
- the group `y` of `next_couple` and `event_source`;
- the arrival cohort.

The exception is a supply side's default couple, found by an inverse shuffle, which costs one `year_of`. Carrying the year makes `demand_group` an array read.

### 18.8 Lemma D (deaths follow first couples)

Let `c` be a person's first couple. `death` raises their last required fact to at least `start(c)` and returns a time after it. Every early return also lies after `start(c)`: a husband's widowhood time is clamped above the start, and a wife's early death above her last fact, which includes the start. So **a person dies after the start of their first couple.**

In a re-partnering of queue W (M), the supply side is a first-time man (woman), and the couple is that person's first couple, whichever way repair goes. So the check "both alive at its start" holds on that side by Lemma D. That partner's death matters only to clamp a separation, and `unions`, `father` and `children` skip it otherwise.
