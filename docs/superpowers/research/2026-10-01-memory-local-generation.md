# Memory-local generation: what mathematics and CS say about a ledger that is not a database

**Date:** 2026-10-01
**For:** "The memory problem" (AGENTS.md, overnight research 2026-10-01 → 02). The founder: "the whole point is that it doesn't [need to be stored]… I don't like that we are basically replacing a file db with an in memory one."
**Builds on, does not repeat:** [local-access-and-bijections](2026-09-29-local-access-and-bijections.md) (GGN, BRY stateful vs memory-less, LCAs, keyed bijections, `Coupling`, `CountTree`, configuration model). Read that first; this note only adds what is new since.
**How it was gathered:** one literature pass in four parallel threads (local access and LCAs; contingency tables and controlled rounding; online discrepancy; checkpointing, succinct storage and population generators), plus my own reading of the rounding theory and a small simulation. The session's shared web-search budget ran out part-way; after that only known URLs (arXiv, DOIs, author pages) were fetched, never other search engines. Downloads are in the git-ignored `.scratch/memlit/papers/`; the simulation is `.scratch/memlit/sim/rounding.py`.
**Markings:** [read] full text, or the relevant section, read; [abs] abstract, metadata or a secondary source only (the secondary is named); [recalled] from memory, not verified this session; [inferred] my own argument, not in any source; [meas] measured here.

## Summary: what is directly usable

1. **The recursion runs on counts only.** The ledger's year step reads per-block aggregates (pools, carried wants, divorced pools, births) and writes per-block aggregates. Deferral and de-isolation are count-based (`ledger.rs`: `defer_isolated`, `deisolate`), and market rounding is already *row-local* given the IPF class factors (`solve_market`: "each row is a pure function of its seeker"). So the 1.3 GB of layouts are an **interior** that a small **boundary** (per block-year aggregates, tens of MB for `us` [inferred estimate, §6.1]) determines year by year. This is the main lever, and it needs no new mathematics: store the boundary, regenerate the interior.
2. **Exact two-sided local access exists for one law: Fisher–Yates.** Any integer table induced by a uniformly random bijection between row-units and column-units has exact margins on both sides. Its quadrant counts, and therefore every cell and every prefix sum in either direction, can be computed with **O(log² n) keyed hypergeometric draws and no state**: Granboulan–Pornin's split tree [read], or BRY's multivariate hypergeometric trees [read]. No paper states this for contingency tables; the derivation is short (§2.2) [inferred, checked against GP07's Splitter].
3. **Kernels that are constant on classes factor exactly** into one small class table plus independent Fisher–Yates layers (§2.3) [inferred, contingency-tables thread]. Internot's IPF kernel depends only on (birth year, status) classes (`GroupedIpf`), so within a class pair the Fisher–Yates law is the *right* law, not an approximation.
4. **For a general kernel, no local exact sampler is known or expected.** Counting tables with fixed margins is #P-complete even with two rows (Dyer–Kannan–Mount) [abs], and the noncentral (kernel-weighted) partition function is a permanent [inferred]. Every published sampler (Diaconis–Gangolli MCMC, Patefield, sequential importance sampling, Barvinok, divide-and-conquer) is global [read/abs].
5. **Offline, both sides can be held within 1 of expectation at once.** Each block's time-ordered cells form a chain, and the women's chains and the men's chains are two laminar families, so the incidence matrix is totally unimodular. Knuth's two-way rounding (error ≤ n/(n+1), best possible) [read], Doerr's linear-discrepancy bound for TU matrices (≤ 1 − 1/(n+1)) [read/abs] and Doerr–Friedrich–Klein–Osbild's unbiased matrix rounding (all row and column prefix errors < 1, O(mn log mn)) [read] all apply.
6. **Error < 1 on both sides is provably non-local** [inferred, §3.3]. In the half-integral case it forces a proper 2-colouring of cycles that can span the table, and 2-colouring a long cycle is a classic non-local task (Linial) [recalled]. Every exact two-sided rounding algorithm found (Cox 1987, Causey–Cox–Ernst, Knuth, Doerr et al., GKPS dependent rounding) cancels cycles or solves flows over the whole table [read/abs].
7. **Online, no constant is possible in the worst case.** Rounding fractional bipartite edges year by year is, within a factor of 2, the carpool / edge-orientation problem (Ajtai et al. 1998, Thm 5.1) [read].
   - **Deterministic algorithms, or any algorithm against an adaptive adversary:** the forced error is Θ(min{T^{1/3}, n}). The lower bound is Ajtai et al.'s; the matching upper bound is per-vertex Greedy, Bansal–Prabhu–Singla–Sundaram 2026 [read].
   - **Randomized algorithms against an oblivious adversary** (a sequence fixed in advance): O(√log T) is achievable, in linear time and locally per edge (Kulkarni–Reis–Rothvoss 2024; Aden-Ali 2026). The lower bound is Ω(∛log) [read].
   - **Per-pair memory removes the growth in T.** Each block's error is then at most its number of distinct partner blocks (Ajtai et al.; Naor 2005: "every two participants record their mutual history (actually a single bit)") [read].
   - Internot's pair-cumulative rule (item 8) is that algorithm, made stateless: the pair's history is recomputed from the fractional cumulative instead of stored.
   - **Integer feedback makes Internot's input adaptive.** The oblivious-adversary guarantees then do not formally apply.
8. **Local one-sided rounding is solved.** Systematic rounding (Madow 1949, Fellegi 1975) gives every prefix of one sequence an error < 1 in O(1) given prefix sums [read]. **Measured** [meas], two-sided local rules give errors that stay bounded in time but grow with partners per block: per-pair cumulative systematic rounding kept every block on both sides within ±4.7 (rms 1.3–1.7) over 140 years with ~19 partner blocks each. The current row-systematic rule lets the men's side drift (rms 4.5–6.4, max 16–17).
9. **Checkpoint and replay is the literature's answer to random access into a nonlinear sequential computation.** Griewank's binomial checkpointing targets *reversal*, not random access. For random access to any year: uniform spacing, worst-case replay ≈ T/(k+1) steps, and online (growing T) ratio bounds 1.39–1.59 against a lower bound of 1.306 (Bringmann–Doerr–Neumann–Sliacan) [read, via the checkpointing thread]. Systems practice matches: zlib `zran` access points, time-travelling VMs. Jump-ahead exists only for *linear* recurrences.
10. **Compression is a constant factor, not a fix.** Elias–Fano stores a sorted offset array in about 2 + ⌈log₂(U/n)⌉ bits per entry with O(1) access [read]. For the cell offsets that is about 3 bits against 64 (≈ 20×), but the size still grows with cells.
11. **Nothing in practice generates a population with exact two-sided relations on demand.** SOCSIM, Dwarf Fortress and synthetic-population tools simulate and store. No Man's Sky and Elite generate stateless objects with no cross-entity kinship. The theory line (GGN, BRY, Even et al.) mostly memoizes [read/abs]. Internot's combination appears to be new.

## 0. The problem, stated formally

**Objects.**
- Blocks v: (birth year, group, sex). Years t.
- Cells c = (A, B, t, kind, class), with integer counts y_c.
- Margins m_v(t) = Σ_{c ∋ v, year t} y_c.

**Recursion.** S_{t+1} = F(S_t, Y_t) and Y_t = G(S_t), where:
- S_t is the per-block state: never-partnered pools, carried wants, divorced pools by divorce year, block sizes from births;
- Y_t is the year's cells and their one-sided splits (plans, dissolution classes, cohort sub-cells, remarriage parts).

**Requirements.**
- **R1, reciprocity:** both sides read the same y_c. *Any deterministic function of c gives this*; it does not need exact margins.
- **R2, tiling and caps:** a block's rank line is tiled by its cells in (year, kind, class, partner) order, then the never-partnered. So Σ_{c ∋ v} y_c ≤ |v|, and remarriages ≤ the divorced available.
- **R3, local access:** y_c, the prefix sums of y along each side's order, and the inverse (rank → cell), each in small time.
- **R4, memory:** independent of population (the founder: also of granularity).

**Two memories, to keep apart.**
- **The boundary S:** O(blocks × years) aggregates. It grows with granularity, not with population.
- **The interior Y:** O(cells), which grows about as population^0.55 (AGENTS.md, scaling table) because more (A, B, t, class) combinations become nonzero.

`us`, measured this session (`target/release/examples/build_time us`) [meas]:
- 3,510 blocks, 1,958,215 union cells (both sides) and 17.3M people;
- 1,309 MB of heap in layouts, 11.4 MB of ledger cohorts and 0.6 MB of ledger blocks;
- build 7.1 s.

The interior is about 99% of the memory.

**Formal core.** A sparse three-way array y(A, B, t) of non-negative integers, together with:
1. **two families of prefix sums:** along each A in (t, B) order and along each B in (t, A) order;
2. **closeness:** y ≈ x, where x is a fractional target in product form x = a_A(t)·b_B(t)·K(class(A), class(B)) (the IPF fixed point);
3. **caps:** R2.

We want entries and both prefix families computable from S alone.

## 1. Local access to huge random objects: what is new since 2026-09-29

The earlier note covered GGN, BRY (2017–20), Naor–Nussboim, Even–Levi–Medina–Rosén, RTVX/ARVX LCAs, Ghaffari 2022 and random-order greedy matching.

**Definitions sharpened.** Even–Medina–Ron: "A CENTLOCAL-algorithm is stateless if the algorithm does not store any information between queries. In particular, a stateless algorithm does not store previous queries, answers to previous probes, or answers given to previous queries." Their footnote 4 notes that the state of the ARVX/MRVX/MV13 LCAs "only stores a random seed that is fixed throughout the execution", so in BRY's terms those LCAs are memory-less given a seed [read]. Biswas et al. 2025 (Def. 1.11) call a local access generator memoryless "if it does not store its answers to prior queries", and note that this is "a requirement in the setting of LCAs, but not always achieved for LAGs" [read].

| Result | Object and queries | Cost per query | Memory-less? | Source |
|---|---|---|---|---|
| Biswas, Cao, Marcussen, Pyne, Rubinfeld, Shapira, Tauber 2025, Thm 1.12 | G(n,p) plus an MIS of it | polylog(n)/p w.h.p. (sublinear only for p ≥ 1/polylog n) | yes | [read] |
| Biswas–Pyne–Rubinfeld, ITCS 2022, Thm 3.1 / 5.2 | random walks on d-regular graphs / abelian Cayley graphs | Õ(√n/(1−λ)) / d·polylog | no (stores query times, fills gaps with multivariate hypergeometric splits) | [read] |
| Biswas–Pyne–Rubinfeld, lower bounds Thm 4.3/4.4 | walks on random d-regular graphs | Ω(√n/log n) probes adaptive, Ω(n^{1/4}) fixed sequence | — | [read] |
| Mörters–Sohler–Walzer, RANDOM 2022, Thm 2 | Chinese restaurant process: seating, table sizes, founders | polylog(N) w.h.p. | no ("a stateful implementation in the sense of [GGN]") | [read] |
| Dong–Mani, SAT 2025 | near-uniform k-SAT solutions | polylog | yes | [read] |
| Even–Medina–Ron, Inf. Comput. 2018, Cor 11 | (Δ+1)-colouring, MIS, maximal matching, deterministic | Δ^{O(Δ²)}·log* n probes | yes, no seed | [read] |
| Mansour–Rubinstein–Vardi–Xie, ICALP 2012, Thm 4/5 | maximal matching; online load balancing (d-choice, capacitated bins) | O(log³ n) / O(log n) | yes (seed) | [read] |
| Mansour–Vardi, APPROX 2013, Thm 3 | (1−ε) maximum matching, bounded degree | O(log⁴ n) | yes (seed) | [read] |
| Levi–Rubinfeld–Yodpinyanee, Algorithmica 2017 | MIS, maximal and (1−ε) matching at degree d | 2^{O(log² d)} log³ n (maximal matching) | yes (seed) | [read] |
| Hassidim–Mansour–Vardi (arXiv 1311.3939), Thm 3.2 / Cor 4.5 | stable matching, random preference lists of length k | O(log n); O(1) for bounded lists on both sides | yes (seed) | [read] |
| Behnezhad–Roghani–Rubinstein, FOCS 2023, Thm 1 | (1, εn)-approximate maximum matching, bipartite, degree Δ | **≥ Δ^{Ω(1/ε)} queries** | lower bound | [read] |
| Feige–Patt-Shamir–Vardi, ICALP 2018 | maximal matching | Ω(√n) strong probes on some graphs | lower bound | [abs] |
| Sanders–Schulz, IPL 2016 | Barabási–Albert **out-edges** | O(1) expected hash calls per edge (geometric chain, mean 2) | yes | [read] |
| Funke et al., IPDPS 2018 / JPDC 2019 | G(n,m), G(n,p), geometric graphs: "PE i and PE j ≤ i redundantly generate chunk (i,j) using the same set of random values" | O((n+m)/P + P) w.h.p. (Thm 2); one vertex's neighbourhood touches Θ(P) chunks | yes, at chunk granularity | [read] |
| Sanders–Lamm–Hübschle-Schneider–Schrade–Dachsbacher, ACM TOMS 2018 | uniform n-subset of [N]: "the t-th random deviate is the hash of the triple (j,k,t)" | ≤ ⌈log p⌉ hypergeometric draws per processor | yes | [read] |
| Boldyreva–Chenette–Lee–O'Neill, EUROCRYPT 2009, Prop. 4.3 | random order-preserving map [M] → [N], both directions | "at most log N + 1" hypergeometric draws worst case | yes (PRF replaces the tape) | [read] |
| Granboulan–Pornin, FSE 2007 | a *uniform* permutation of [n], forward and inverse | "O(log n) space and O((log n)³) RNG invocations" | yes | [read] |

**What this adds for Internot.**
- **BRY's open problems are still open** (memory-less undirected G(n,p), degree queries, random perfect matchings of lattices). No progress was found in 2018–2026 beyond the dense G(n,p)+MIS case [read; queries listed by the local-access thread].
- **Correction to the 2026-09-29 note** (§1.3, "I know of no memory-less version" of preferential attachment): Sanders–Schulz give one for **out-edges**. `generateEdge(i)` sets r := 2i+1 and repeats r := h(r) until r is even, then returns (⌊i/d⌋, ⌊r/2d⌋) [read]. In-edges and degrees still need the whole edge set, and the stateful Even et al. generator remains the only random-access one for those.
- **The count-splitting idea is everywhere, and memory-less whenever split points are fixed and keyed:**
  - GGN §5 interval sums;
  - BRY Thm 20 (community counts in any range, O(r·polylog n)) and Thm 56 (marbles among the first k colours, O(polylog B));
  - Sanders' subset sampling, BCLO, GP07.
  - Biswas–Pyne–Rubinfeld and Mörters–Sohler–Walzer keep their trees as state only because their split points depend on the queries. With a fixed dyadic tree keyed per node, the same splits would be memory-less [inferred].
- **Converting an online algorithm into an LCA** (MRVX) works only for a *random* arrival order with bounded or binomial degree: the query-tree bound rests on random ranks. Internot's order is prescribed (calendar years) and its markets have huge degree, so the conversion does not apply [read].
- **Optimized matchings are not local in high-degree bipartite markets** (Behnezhad–Roghani–Rubinstein). Internot's counts must stay *generated* (sampled or rounded from a target), never *optimized*, if they are to be local [read].
- **Not found:**
  - any memory-less, two-sided generator for a bipartite table or matching with prescribed block counts under a non-independence kernel;
  - any communication-free generator for a prescribed degree sequence (beyond configuration-model shuffles);
  - any such generator for a stochastic block model with prescribed block-pair edge counts. Funke et al. list "the stochastic block-model" as future work [read].

## 2. Exact counts without a global recursion

### 2.1 Counts first, members second (one side)

- **The pattern:** draw a node's count, split it into children by a law with a closed-form counting oracle (binomial, hypergeometric, multinomial), and seed each split by a keyed hash of the node.
- **Consistency is structural:** children sum to their parent whatever the sampler. That is GGN Thm 5.2 ("answers the query (α,β) … by Σ_{α≤s≤β} f(s)") [read], BRY Thm 20 [read], and `procedural_core::count::CountTree`, which already exists with binomial splits.
- Every one-sided interior in the ledger is of this kind given its parent count: plan leaves of a cell, dissolution classes of a slice, cohorts of a cell, remarriage parts of a source. Keyed systematic apportionment (`partition::SystematicShares`) already defines them. They are stored today for speed, not because they could not be regenerated [inferred from `world.rs`/`ledger.rs`].
- **What is missing in core:** an exact hypergeometric sampler. Core has exact Poisson and binomial; BCLO used Kachitvichyanukul–Schmeiser (J. Stat. Comput. Simul. 22, 1985; ACM TOMS Alg. 668, 1988) [abs].

### 2.2 Two sides: the Fisher–Yates table is locally accessible

**The identity** [read, contingency-tables thread, standard]:
- Lay out N row-units (r_i for row i) and N column-units (c_j for column j), and draw a uniform bijection σ between them.
- The number of σ inducing a table y is Π r_i! Π c_j! / Π y_ij!. So the table follows the **Fisher–Yates (multiple hypergeometric) law** P(y) = Π r_i! Π c_j! / (N! Π y_ij!), with mean r_i c_j / N and **exact margins on both sides by construction**.

**Local quadrant counts** [inferred; GP07's algorithm read and checked].
- Granboulan–Pornin build a uniform permutation φ of [n] from nested splits:
  - `Repartitor` draws one hypergeometric count: how many of p selected elements fall in the first ⌊n/2⌋;
  - `Splitter` extracts p elements with a "distribution tree" whose nodes record "how many elements among those descending from this node are 'extracted'". It preserves order within the extracted group and within the rest;
  - `Permutator` recurses into the half that x went to.
- Q(a, b) = |{x < a : φ(x) < b}| then follows:
  1. let u be the number of extracted elements among x < a: one root-to-leaf prefix sum in the distribution tree, O(log n) `Repartitor` calls;
  2. because order is preserved, those u land on [0, u) of the lower half, and the other a − u land on [h, h + a − u) of the upper half;
  3. so Q(a, b) = Q_lower(u, b) if b ≤ h, and u + Q_upper(a − u, b − h) otherwise.
- **Cost:** O(log² n) hypergeometric draws, the same as one evaluation of φ.
- **Use:** a cell is y_ij = Q(R_i, C_j) − Q(R_{i−1}, C_j) − Q(R_i, C_{j−1}) + Q(R_{i−1}, C_{j−1}), with R and C the cumulative margins. Row i's prefix sum up to column j is Q(R_i, C_j) − Q(R_{i−1}, C_j), and the same holds for columns. Person-level partners come from φ itself, so cells, prefix sums and partners all agree, with **no stored state**.
- **Alternative tree:** the contingency-tables thread derived the same thing from BRY's trees: a lazy tree over row positions whose nodes carry counts over dyadic column-label sets, O(log N · log m) draws [inferred there].
- **Prior art:** not found as a stated result. Queries were listed by both threads; GP07 and BCLO "do not discuss quadrant or dominance counts".

**Caveats.**
- The law is independence (mean r_i c_j / N) given the margins. A target kernel enters only through §2.3.
- **Sampling noise.** Cells fluctuate around their mean like hypergeometric variables, roughly √x in spread. They are not rounded to within 1 as today.
- **The law is a modelling choice.** Uniform tables and Fisher–Yates tables "may demonstrate very different behavior" under skewed margins (Barvinok 2010; Dittmer–Lyu–Pak 2020 find a sharp phase transition in uniform tables) [read]. Fisher–Yates is the natural one here, because it is the law of a random matching of people.
- **Exactness of the law needs an exact hypergeometric sampler.** Exactness of the *margins* does not: any feasible split keeps the bijection valid. Biased splits (toward a target) keep locality and exact margins but change the law [inferred, contingency-tables thread].

### 2.3 Kernels constant on classes factor exactly

[Inferred, contingency-tables thread; the proof sketch checks.]
- **Setup:** K_ij = k(a(i), b(j)), with rows grouped in classes a and columns in classes b. The kernel-weighted law P(y) ∝ Π K_ij^{y_ij} / y_ij! given both margins factors exactly into:
  1. a **class table** Y_ab with the same noncentral law and class margins (small);
  2. for each row class, a Fisher–Yates table with margins (r_i, Y_ab over b);
  3. for each column class, a Fisher–Yates table with margins (c_j, Y_ab over a), independent of (2);
  4. for each class pair, a Fisher–Yates table with margins (r_ib, c_ja).
- **Proof:** within a block, Σ Π 1/y! = Y!/(Π r! Π c!), the Fisher–Yates normalizer.
- **The product-form factors cancel:** under x = a_i b_j K_ij, the a and b factors cancel given exact margins. Only K's interaction matters.

**Why this fits Internot.** `solve_market` fits a `GroupedIpf` whose kernel "depends only on the birth-year gap and the statuses", so "the IPF runs over (birth year, status) classes and each seeker takes its share of its class's margin". Within a class pair the fitted table is rank one: exactly the Fisher–Yates mean.

- **Where the factoring pays:** open and national markets, which have many groups (heritages × regions or areas) per class.
- **Where it does not:** a local market has one row per class, so its class table *is* its cell table. There the kernel-carrying part is a banded birth-year × birth-year table:
  - about 75 birth years × 2 statuses per side;
  - a gap kernel whose support is a few dozen columns per row.

  That is small enough to *regenerate* on demand from the stored class factors: a few thousand entries per market-year, microseconds [inferred estimate]. It is not something that can be made local entry by entry with two-sided control (§3).

### 2.4 Global samplers and hardness (why there is no general local answer)

| Work | Law | Structure | Local? |
|---|---|---|---|
| Diaconis–Gangolli 1995 | uniform on tables with given margins | 2×2 ±1 moves, MCMC; ergodic, no mixing bound | no [abs, via Cryan et al.] |
| Diaconis–Sturmfels 1998 | conditional laws given sufficient statistics | Markov bases from toric ideals | no [abs, via Chen–Dinwoodie–Sullivant] |
| Patefield 1981 (AS 159) | Fisher–Yates | raster order, each entry from the conditional hypergeometric given remaining totals; O(rc) draws | no: entry (l,m) depends on all earlier entries [read R's `rcont.c`] |
| Chen–Diaconis–Holmes–Liu 2005; Chen–Dinwoodie–Sullivant 2006 | uniform or any target, by importance weights | fill cell by cell from interval-supported proposals | no [read CDS; CDHL via CDS] |
| Dyer–Kannan–Mount 1997 | uniform | polytope sampling plus rounding; **exact counting #P-complete "even when there are only two rows"** | no [abs, via Cryan et al.] |
| Cryan–Dyer–Goldberg–Jerrum–Martin 2006 | uniform | heat-bath 2×2 chain, rapid mixing for a constant number of rows; "No technique currently exists for polynomial-time sampling when the row and column sums can be arbitrary" | no [read intro] |
| Barvinok 2010 | uniform | independent geometric entries with means z_ij have constant probability on the table set (Thm 1.7); the typical table solves ln((z+1)/z) = λ_i + μ_j | no (a convex program) [read §1] |
| DeSalvo–Zhao 2015 | uniform | probabilistic divide and conquer, exact for 2×n | no [read excerpts] |
| Northwest corner (stepping stones; Cox's n-d paper) | deterministic | y_ij = \|[R_{i−1}, R_i) ∩ [C_{j−1}, C_j)\| | **yes, O(1)**, but it ignores the target [read] |

For a general kernel, the noncentral partition function is Σ_y Π K^y / y! = per(K̃) / (Π r_i! Π c_j!), with K̃ the N×N block expansion of K [inferred, contingency-tables thread]. With unit margins this is per(K): #P-hard (Valiant 1979) [recalled], approximable by Jerrum–Sinclair–Vigoda [recalled]. So no polylog split oracle is expected beyond class-constant kernels.

## 3. Rounding theory

### 3.1 Offline two-sided rounding

| Result | Statement | Algorithm | Source |
|---|---|---|---|
| Baranyai 1975 | any real matrix rounds so that every entry, row sum, column sum and the total is the floor or ceiling of its value | flow | [abs, via Doerr et al.] |
| Bacharach 1966; Causey–Cox–Ernst 1985 | the same, found independently ("in a slightly weaker form" for Bacharach); zero-restricted 3-D controlled rounding is **not always feasible** | transportation / flows | [abs, via Doerr et al. and Census RR-90/10] |
| Cox 1987, JASA 82:520–524 | unbiased controlled rounding of 2-D tables: every entry and total goes to an adjacent multiple of the base, with E = the unrounded value. "an unbiased, controlled rounding of an arbitrary three-dimensional table does not always exist" | each unrounded cell lies on a circuit of unrounded cells; step ±m along it with E(m) = 0 until one more cell is a multiple | [abs: the paper is paywalled; Cox's own description in ASA SRMS 1986 and his n-d paper read] |
| Knuth 1995, SIAM J. Discrete Math. 8:281–290 | for any x ∈ [0,1)^n and permutation σ, a rounding with prefix errors in both orders ≤ **n/(n+1), best possible**. With integer total m: ≤ (2m+1)/(2m+2) (Thm 1, also best possible as a function of m) | integral max-flow, O(mn); "in practice it runs much faster on random data"; with m ≪ n the optimum is near ½ | [read] |
| Spencer (via Knuth) | two orderings, error ≤ 1 − 2^{−2n} | indirect (discrepancy) | [read, as cited by Knuth] |
| Doerr, EJC 7 (2000) R48; Combinatorica 24 (2004) | lindisc(A) ≤ 1 − 1/(n+1) for totally unimodular A (first for at most two nonzeros per row, then all TU) | via Ghouila-Houri: every column subset of a TU matrix splits with row imbalance ≤ 1 | [read EJC; abs Combinatorica] |
| Doerr–Friedrich–Klein–Osbild, SWAT 2006 | all row-prefix and column-prefix errors and the total error < 1, in O(mn log mn); an unbiased version in O(mnℓ) in which every prefix sum is itself a randomized rounding. Arbitrary intervals: error < 2, and a lower bound of 1.5 is cited | bitwise (Beck–Spencer), then at each bit level pair consecutive ½-entries per row and per column. The pairs form disjoint even cycles; alternate along each, choosing the phase at random for unbiasedness | [read] |
| Gandhi–Khuller–Parthasarathy–Srinivasan, JACM 53 (2006) | on bipartite edges: (P1) Pr[X=1] = x; (P2) every vertex's degree ∈ {⌊d⌋, ⌈d⌉} with probability one; (P3) negative correlation. General graphs with prescribed degrees: \|D_i − d_i\| < 2, in O(n + m²) | find a cycle or maximal path among floating edges by DFS and shift it randomly; "the total running time is O((\|A\|+\|B\|)\|E\|)" | [read, journal version] |

**Applied to the ledger** [inferred, elementary].
- **Setup:** order each block's cells by (t, partner); the women's chains form one laminar family and the men's chains another. The prefix-incidence matrix of the union of two laminar families is totally unimodular (Edmonds; Schrijver, *Combinatorial Optimization*, for matroid intersection) [recalled].
- **Direct construction:** Knuth's lemma does it with one dummy element per block, which makes each block's total integral. Then:
  1. concatenate the women's chains into one ordering and the men's chains into the other;
  2. every block boundary is now at an integer prefix sum, so it is rounded exactly;
  3. so every block's own prefix error is < 1 on both sides at all times.
- **Consequence:** an offline rounding of the whole ledger exists that keeps every block's cumulative union count within 1 of its expectation, both sexes, every year. It costs one max-flow over all cells (or Doerr et al.'s O(N log N) cycle method). It is a build-time computation.

### 3.2 One side: local

- **Madow 1949 / Fellegi 1975 systematic rounding:** "the same argument holds for the sum of any consecutive numbers" (Fellegi). Cell i is the change in ⌊(S_i + u)⌋ between consecutive prefix sums: O(1) given prefix sums, with every prefix error < 1 [read Fellegi; Madow abs].
- `partition::round_systematic_cumulative` and `SystematicShares` are this. Cox (1986) calls it "applicable only to one-way tables" [read].

### 3.3 Why exact two-sided control is not local

[Inferred, elementary; please check.]
- **Setup:** take a matrix with entries in {0, ½} and integral row and column sums (Doerr et al.'s Lemma 7 setting). Pair the (2k−1)-th and 2k-th ½-entries of each row, and likewise each column.
- **Forced complementarity:** if every row-prefix and column-prefix error is < 1, each pair holds exactly one 1. The prefix ending just before a pair has an integral value, so it must be rounded exactly; the prefix ending inside the pair must then add exactly 0 or 1 by its end, which forces complementarity.
- **So every rounding with error < 1 is a proper 2-colouring** of the pair graph, which is a disjoint union of even cycles alternating row pairs and column pairs (Doerr et al., Lemma 7) [read].
- **Lower bound:** a cycle can be as long as the table. Deciding one entry consistently with its neighbours is then a 2-colouring of a long cycle from local information, and that needs Ω(cycle length) probes (Linial 1992, 2-colouring a ring in the LOCAL model) [recalled].
- **Conclusion:** the < 1 guarantee of §3.1 cannot be had locally in the worst case.
- **Open:** whether a constant bound c > 1 can. Cutting cycles into short segments does not obviously work, because the defects at the cuts accumulate within a row like a random walk.

### 3.4 Online: rounding year by year

**The question.** Fractional cells x_e arrive year by year on a bipartite graph (women blocks × men blocks). Each must be rounded to ⌊x_e⌋ or ⌈x_e⌉ before later years are known. Can every block's cumulative error stay within a constant, on both sides, at all times?

**The reduction** [read]. Ajtai, Aspnes, Naor, Rabani, Schulman and Waarts, "Fairness in scheduling" (J. Algorithms 29, 1998), §5, study *vector rounding*: real columns arrive, and each entry and each column sum must be rounded with bounded accumulated error.
- An edge e = (i, j) is the column (x_e at woman i, −x_e at man j). Its sum is 0, and flipping the men's sign gives "both endpoints receive the same error".
- Thm 5.1: an edge-orientation algorithm with unfairness Δ(n) gives a vector-rounding algorithm with accumulated difference at most 2Δ(n). It works bit by bit, with about 2 log T carpool instances.
- On Internot's columns, every instance stays bipartite.
- Conversely, edges with x = ½ *are* unweighted edge orientation. So upper and lower bounds transfer within a factor of 2.

**Known bounds** (general graphs unless marked; from the online-discrepancy thread).

| Setting | Upper bound | Lower bound | Sources |
|---|---|---|---|
| Deterministic, or any algorithm against an adaptive adversary | O(min{T^{1/3}, n}) by Greedy, which keeps only per-vertex counters (Thm 1.1); (n−1)/2 by global greedy (Thm 2.2) | Ω(min{T^{1/3}, n}); exactly ½⌈(n−1)/2⌉ on K_n, a chip game in which the adversary keeps requesting two vertices with equal counters (Thm 2.3: k³ requests force unfairness k) | Bansal–Prabhu–Singla–Sundaram, arXiv 2609.21348 [read pp. 1–6]; Ajtai et al. [read] |
| Randomized, oblivious adversary | O(√log T) w.h.p., independent of n (Kulkarni–Reis–Rothvoss, Cor. 5, existential); √(18 log(2T²/δ)) in O(dT) time, local per edge (3 reals per vertex) (Aden-Ali Thm 2); O(log(nT)) (Alweiss–Liu–Sawhney); O(√(n log n)) (Ajtai local greedy) | Ω(∛log n) (Ajtai Thm 3.2); Ω(∛log T) (as cited by KRR). KRR's Conjecture 2 asks whether O(∛log T) is achievable for edges | KRR STOC 2024, arXiv 2308.01406 [read]; Aden-Ali arXiv 2607.04388 [read pp. 1–4]; ALS STOC 2021, arXiv 2006.14009 [read] |
| Stochastic (i.i.d.) arrivals | Θ(log log n) on K_n (Ajtai; Altschuler–Tikhomirov for sparse binary vectors); O((log n / log Δ)^{1/3} + log log n) on Δ-regular graphs with T = O(n) | Ω((log n / log Δ)^{1/3}) | BPSS Thms 1.2–1.3 [read]; Altschuler–Tikhomirov arXiv 2509.02432 [abs] |
| Per-pair state (each pair keeps its net imbalance within 1) | O(d_v) deterministic, O(√(d_v log n)) randomized, with d_v the number of distinct partners; **independent of T** | — | Ajtai "local greedy" [read]; Naor, J. Algorithms 55 (2005): "There is no need for maintaining any global information… It is enough that every two participants record their mutual history (actually a single bit)" [read] |
| With recourse (past edges may be re-rounded) | polylog n discrepancy at polylog n amortized recourse; without recourse, Ω(n) against adaptive adversaries | — | Gupta–Gurunathan–Krishnaswamy–Kumar–Singla SODA 2022, arXiv 2111.06308 [read §1] |
| Offline | < 1 for bipartite (§3.1); 1 for general carpools (Tijdeman, via Ajtai) | — | — |

**Notes.**
- **Spencer's Ω(√T) lower bound needs arbitrary coefficients.** BPSS note "a trivial Ω(T^{1/2}) lower bound for such [2-sparse] vectors, already when n=2" only with general weights. Edges and Internot's rounding escape it [read].
- **The chairman-assignment variant is online-optimal by greedy.** There each year's fractions over m rows sum to 1, and the bound is Σ_{j=2}^m 1/j = Θ(log m) against adaptive adversaries (Coppersmith–Nowicki–Paleologo–Tresser–Wu, TALG 2011) [abs]. The offline bound is ≤ 1 − 1/(2m−2) (Meijer 1973; Tijdeman 1980) [via Liu–Reis, ITCS 2026, read].
- **Online dependent rounding is no help here.** Naor–Srinivasan–Wajc (arXiv 2301.08680) and Buchbinder–Naor–Wajc give only *marginal* guarantees (Pr[e ∈ M] / x_e ≥ 0.646), not bounded cumulative counts. Lossless online rounding "is impossible" (Devanur et al., via BNW) [abs].
- **Bipartite graphs specifically:** no published bound was found. The online-discrepancy thread solved small games exactly by search over reachable states (adversary picks a woman–man pair and a fraction; the algorithm picks up or down), and reproduced Ajtai's ⌈(n−1)/2⌉ on K_n:

  | Graph | Fractions allowed | k = 1 | 2 | 3 | 4 | 5 | 6 |
  |---|---|---|---|---|---|---|---|
  | K_{k,k} | ½ only | ½ | ½ | 1 | 1 | 1.5 | 1.5 |
  | K_{k,k} | ¼, ½, ¾ | 0.5 | 0.75 | 1.25 | 1.5 | — | — |

  K_{k,k} had the same value as K_{k+1} in every case computed. So the forced error still grows linearly in the number of blocks, at about half the rate [computed by that thread; evidence, not proof].
- **Batches (a whole year at once) do not help in the worst case:** a year can be one fractional edge plus integers [inferred]. The obstruction is concrete: a woman block carrying error +0.9 and a man block carrying −0.9 that share a single fractional edge cannot both stay below 1, whichever way it rounds.

**What it means for the ledger** [inferred].
- **Today's ledger is in the adaptive regime.** Year t+1's targets depend on year t's integers (pools, caps), so only the deterministic or adaptive bounds apply, and those grow.
- **The adaptive bounds are worst cases, and the ledger is not adversarial.** Its kernels are smooth and every block has many partners each year. That is closer to the stochastic rows, where log log n behaviour is typical.
- **An expectation-driven recursion (C3/C4 in §6) makes the input oblivious, indeed fully known at build time.** Then:
  - the offline < 1 bound applies outright (one global computation);
  - stateless per-pair rounding gives O(partners) with no growth in T (§3.5).

### 3.5 Measured: local two-sided rules

**Setup** [meas] (`.scratch/memlit/sim/rounding.py`):
- 80 women blocks and 80 men blocks (birth years), over 140 years;
- x_AB(t) = s·h(age)·g(gap), with an age bump around 26 and a gap kernel centred on +2 that covers 19 partner blocks;
- about 45k nonzero cells, mean x 0.01–0.23.

**Errors.** A block's error at year T is Σ_{its cells, t ≤ T} (x − y). The table reports the maximum over all blocks and years, and the rms at the end.

| Rule | Local? | Max \|err\|, women | Max \|err\|, men | rms women / men |
|---|---|---|---|---|
| independent ⌊x + u_c⌋ | yes | 7.3–15.7 | 7.3–12.1 | 2.4–5.2 / 2.0–4.9 |
| row-systematic per (A, t), fresh offset (the current `solve_market`) | yes, per row | 4.8–6.9 | 5.9–17.0 | 1.8–2.1 / 2.1–6.4 |
| row-lifetime systematic (one offset per A, along (t, B)) | yes, per row | **0.97–0.98** | 6.5–17.1 | 0.3–0.4 / 2.1–6.3 |
| **pair-cumulative systematic**: C_AB(T) = ⌊F_AB(T) + u_AB⌋, cell = C_AB(t) − C_AB(t−1) | **yes, per cell** | **4.4–4.7** | **4.4–5.8** | **1.3–1.6 / 1.3–1.7** |
| pair-cumulative, Kronecker or u_A + u_B offsets | yes | 3.6–4.6 | 4.1–4.4 | 1.2 / 1.2–1.5 |

The ranges span three scales (s = 0.2, 1, 5).

**Reading.**
- **One-sided rules control one side perfectly and let the other side drift as a random walk.** The current rule rounds each (row, year) afresh, so even the women's side drifts.
- **The per-pair rule bounds both sides in time.** Each pair's cumulative error stays in (−1, 1) forever, and a block's error is a sum over its K partner pairs: O(K) worst case, about √(K/12) typically. It needs the fractional cumulative F_AB(t), i.e. a fractional (expected-value) recursion, and per-block prefix sums cost O(K) by scanning.
- **Correlated offsets barely help.**
- **Scale to Internot:** a real block meets hundreds of partner blocks (birth years × regions × heritages × kinds × statuses). Expect rms about 4–6 per block [inferred].

## 4. Recomputing intermediate states: checkpointing and random access

| Work | Problem | Bound | Source |
|---|---|---|---|
| Griewank 1992; Griewank–Walther 2000 (`revolve`, ACM TOMS 26:19–45) | **reversal** of an l-step chain with s snapshots | minimal recomputations t·m − C(s+t, t−1) for C(s+t−1, t−1) < m ≤ C(s+t, t); reach β(s, r) = C(s+r, s) with no step run more than r+1 times | [abs; Prop. 1 read in Zhang–Constantinescu, arXiv 2106.13879; Walther–Narayanan, OSTI 1364654] |
| Stumm–Walther 2010; Wang–Moin–Iaccarino 2009 | reversal with an unknown number of steps | optimal or near-optimal online placement | [abs] |
| Chen–Xu–Zhang–Guestrin 2016 | backprop memory | O(√n) memory for one extra forward pass; O(log n) memory at O(n log n) | [read] |
| Kirisame et al. 2020 (Dynamic Tensor Rematerialization) | online eviction with recomputation | evict by h = c/(m·s) (recompute cost / (memory · staleness)); Θ(N) work on an N-chain with Ω(√N) memory; adversarial Ω(N/B) for any deterministic heuristic | [read] |
| Bennett 1989; Levine–Sherman 1990; Li–Tromp–Vitányi 1998 | reversible pebbling | time O(T^{1+ε}), space O(S log T); exact Θ(T^{1+ε}/S^ε) and Θ(S(1+ln(T/S))); 2^n − 1 reachable with n pebbles | [abs; LTV read] |
| **Bringmann–Doerr–Neumann–Sliacan, ICALP 2013** | **rewind to any t ≤ T** with k checkpoints, T growing | longest gap ≤ q·T/(k+1): uniform (known T) q = 1; online q ≤ 1.586 + O(1/k) (linear), ≤ ln 4 + o(1) ≈ 1.39 (binary, k a power of two); lower bound 2 − ln 2 − O(1/k) ≥ 1.306 | [read] |
| King–Dunlap–Chen 2005 (time-travelling VMs) | random-access "goto" in a recorded execution | checkpoint every 25 s; under a space cap thin to exponentially growing gaps backwards; average replay = half the interval | [read] |
| zlib `zran.c` | random access into a forward-only decoder | an access point every SPAN = 1 MB; on average SPAN/2 replayed | [read] |
| Subversion skip-deltas | random access to revision N | ≤ lg N deltas per revision | [read] |
| Gosper 1984 (Hashlife) | fast-forwarding a cellular automaton | memoized quadtree; "performs very poorly on highly chaotic patterns" | [abs, via secondary] |
| Haramoto et al. 2008 | jump-ahead | polynomial arithmetic for **F₂-linear** recurrences only | [abs] |

**For the ledger** [inferred].
- **Random access, not reversal.** Revolve's binomial schedule answers the wrong question. The relevant bound is the plain one: with k stored states over T years, a cold query replays at most about T/(k+1) years (cost-weighted if years differ in cost).
- **Jump-ahead is ruled out.** An *unrounded* linear Leslie projection could jump by matrix powers, but two-sex markets, integer rounding and caps make the step nonlinear. Checkpoint and replay is all that is left.
- **Eviction:** DTR's cost/(memory·staleness) is the literature's rule for a cache of recomputed states.

## 5. Other relevant work

**Succinct prefix sums** [read].
- **Elias–Fano** (Vigna 2013, "Quasi-succinct indices"): "at most 2 + ⌈log(u/n)⌉ bits per element". Constant average-time access and NextGEQ with forward pointers; pointers cost about 1% of the index.
- **Partitioned Elias–Fano** (Ottaviano–Venturini 2014): on Gov2 docIDs, 4.10 bits/int against 7.53 for plain EF.
- **Pibiri–Venturini survey** (2020): PEF 3.12 bits/int at 0.76 ns per decoded int.
- **Estimate for the ledger:** about 9.5M offsets over U = 17M is about 3 bits each, 3.6 MB against 76 MB as u64 [inferred, checkpointing thread]. Each access costs 2–3 dependent cache misses, so it competes with today's `Coarse` index on latency.

**Population generators in practice.**
- **SOCSIM** (Mason, "Socsim Oversimplified", 2016) [read]:
  - monthly discrete-event competing risks; writes the whole population (`.opop`, `.omar`);
  - states the two-sex problem as "one can meet at most two of the following three marriage market constraints: female age-specific nuptiality rates; male age-specific nuptiality rates; distribution of spousal age differences" (Internot's IPF meets all three in expectation by fitting the kernel to both margins);
  - forbids marriages closer than cousins.
- **Beckman–Baggerly–McKay 1996:** IPF of microdata to census margins, then materialized [abs].
- **Caswell 2019:** expected kin counts by matrix recursion with "no simulation"; aggregate, not individual [abs].
- **Games** [read]: Dwarf Fortress simulates and stores history. No Man's Sky and Elite are seeded and stateless, with no kinship.
- **Random123** (Salmon et al. 2011): counter-based RNGs "require little or no memory for state". Internot's `Key` already works this way [read].

**Random access to a sequential random process.** Even–Levi–Medina–Rosén (TALG 2021, Thm 19): preferential attachment with O(log⁵ n) time and O(log² n) added *state* per query [read]. Their trick inverts the process, so vertices sample their future children with harmonic probabilities. It works because the marginal structure is simple, which market clearing lacks.

## 6. What this means for Internot

### 6.1 Constructions that make the interior a function of a small state

Ranked from least to most change to the world.

**C1. Store the boundary, replay the interior** (checkpoint every year; no new mathematics).
- **What is stored:** per block-year (or per market-year):
  - the market inputs: seekers' wants and caps, carried wants, divorced availability by divorce year;
  - the outputs needed as offsets: cumulative unions by kind and class, births by mother block.
- **Size:** about 3,510 blocks × about 100 active years × a few dozen numbers, i.e. tens of MB for `us` against 1.3 GB [inferred estimate]. It is independent of population, but grows with granularity (about 30× more blocks in `us-areas`).
- **On demand:** a query needing year t's cells re-runs that year's markets (`solve_market`, `apply_market`, `record`, the class and plan splits) from the stored inputs, and keeps the result in a cache bounded by bytes (DTR-style eviction).
- **Guarantee:** **bit-identical to today** if the replay is the same code; a fingerprint test proves it.
- **Cost:** the whole `us` build is 7.1 s for 260 simulated years, about 27 ms per year including layouts [meas]. One market-year is a fraction of that (not measured).
- **Rows are cheaper still:** each row of a market is "a pure function of its seeker" given the fitted class factors, so a woman's cells cost O(columns + couples·log) without the rest of the market. A man's cells need every row of his column classes, or the cached market.
- **Risks:**
  - caps and settlement (`apply_market`, `settle`) couple rows within a market, so replaying a single row is exact only if the capped takings are part of the stored boundary;
  - kin lookups touch several years (parents' union years, children's birth years), so cold multi-hop queries replay several market-years.
- **Granularity independence:** keep every k-th year and replay up to T/(k+1) years cold (Bringmann et al.).

**C2. Fisher–Yates layers for the many-group markets** (local, exact margins, a new law inside classes).
- **Use:** where a class pair holds many blocks (open and national markets, and `us-areas` above all), generate the block-level split of each class-pair count by the GP07 / BRY hypergeometric tree (§2.2–2.3).
- **What is stored:** only the class table and the block margins.
- **Guarantees:** exact margins both sides; reciprocity down to persons, because the partner is φ(slot); O(log² n) draws per cell or prefix sum; no state and no cache.
- **The law:** exactly the conditional law given class counts, with no rounding to within 1. Hypergeometric noise appears instead.
- **Prerequisite:** an exact keyed hypergeometric sampler in `procedural_core` (property tests, golden values), then a `FisherYatesTable` primitive with `cell`, `row_prefix`, `col_prefix` and `partner`.

**C3. Expected-value recursion plus pair-cumulative rounding** (local everywhere, no integer feedback).
- **Model change:** drive markets by expected (fractional) pools only. The ledger already does this for mortality: "the ledger's expected never-partnered pools only size the markets". Then x_AB(t) is a function of per-block-year fractional factors.
- **Cells:** y_AB(t) = ⌊F_AB(t) + u_AB⌋ − ⌊F_AB(t−1) + u_AB⌋.
- **Guarantees:**
  - reciprocity, exact (both sides compute the same number);
  - block errors bounded in time but O(√K) typical (measured ±4.7 at K ≈ 19, §3.5);
  - no stored interior and no cache.
  - This is the per-pair "local greedy" of Ajtai et al. and Naor (§3.4), with the pair's one bit of history recomputed from F_AB instead of stored.
- **What breaks:** the caps are no longer exact. Feasibility needs expected slack larger than the error, which fails in thin blocks and late in life. Either keep a cap-aware fallback for thin blocks (store them) or accept rare deferrals.

**C4. Expected-value recursion plus offline TU rounding of the boundary** (the best error guarantee).
- **Method:** round all block-years' cumulative counts at build with Knuth/Doerr (§3.1), so every block on both sides is within 1 of expectation at every year. Store the integer margins (the boundary) and generate interiors by C1 or C2.
- **Guarantee:** this gives the caps their margin (slack ≥ 1 suffices). It costs one global flow at build time.

**C5. Compress what remains.** Elias–Fano offsets and packed leaves: a constant factor (up to about 20× on offset arrays). Do it last, on whatever C1–C4 still stores.

### 6.2 What does not work, and why

- **Exact two-sided control with error < 1, computed locally:** impossible in the worst case (§3.3).
- **Local exact sampling under a general kernel:** permanent-hard (§2.4). Only class-constant kernels factor (§2.3).
- **Converting the year-by-year market into an LCA** (MRVX): needs random arrival order and bounded degree (§1).
- **Jump-ahead through years:** only linear recurrences jump (§4).
- **Optimizing matchings locally:** ruled out by Behnezhad–Roghani–Rubinstein.

### 6.3 Granularity

- **What the boundary is:** the cohort-component recursion at the chosen granularity *is* the demographic model. A Hispanic cohort of OH-2 has its own history because its blocks carry their own pools.
- **Memory independent of granularity therefore needs one of:**
  - replay from sparse checkpoints, trading time per cold query (§4);
  - a coarse recursion with fine counts split top-down by keyed shares (GGN-style trees over regions → heritage → birth year). That stops fine-level history from feeding back, which is a model change.
- **No closed form** for a nonlinear two-sex recursion was found, and none is expected.

### 6.4 Recommended order (for the founder)

1. **C1, starting with the women's rows**, which are already row-local. Measure:
   - market-year replay time;
   - cache hit rates on a `read_person` workload;
   - boundary size on `us` and `us-areas`.
   Prove it bit-identical with `world_fingerprint`.
2. **The exact hypergeometric sampler and `FisherYatesTable` in core** (golden values, property tests: margins exact, quadrant counts agree with φ, the law matches Fisher–Yates on small tables). Then prototype C2 on the open and national markets of `us-areas`, where the factoring pays most.
3. **Decide whether the integer feedback is worth keeping** (C3/C4 against C1). That is a modelling decision; §3.5's numbers give its cost.

## 7. Open questions

1. **Local two-sided rounding with constant error:** is there a rule computable from O(polylog) cells that keeps every block's prefix error on both sides ≤ c for a constant c > 1? §3.3 rules out c < 1. Local Lovász Local Lemma algorithms (RTVX 2011's hypergraph colouring) are the natural tool, but were not searched this session.
2. **Online bipartite rounding:**
   - Is the forced error on K_{k,k} exactly that of K_{k+1}, as the small exact games suggest?
   - Does a smooth kernel with many partners per block per year (the ledger's real input) keep greedy-with-carry within a small constant in practice? Simulate it on the real `us` targets before choosing between C1 and C3/C4.
   - KRR's Conjecture 2 (O(∛log T) for edges, oblivious adversary) is open.
3. **Thin blocks under C3/C4:** what share of block-years has expected slack below 2–3, and what does a cap-aware fallback cost? Measure on `us-areas`.
4. **The law inside classes:** does Fisher–Yates noise (C2) change any realism target, compared with today's row-systematic rounding? It is likely negligible at class-pair counts above about 10; check intermarriage tables.
5. **The mother inverse under C1/C2:** a child's mother is located through per-(child block, year) mother shares (boundary), then the mother block's rows in up to 35 earlier market-years. Cold cost needs measuring.
6. **Boundary size for `us-areas`** (109k blocks): if it reaches hundreds of MB, sparse checkpoints (§4) or Elias–Fano on the boundary itself are the next step.
7. **Write-up for `procedural_core`:**
   - the exact hypergeometric sampler;
   - the Fisher–Yates table;
   - pair-cumulative rounding (a two-line wrapper over `round_unbiased`);
   - the Knuth/Doerr offline two-way rounding, as a build-time primitive.

## Sources

**Read (full text or the relevant section):**
- Knuth, D.E. *Two-way rounding.* SIAM J. Discrete Math. 8(2):281–290, 1995. https://arxiv.org/abs/math/9504228
- Doerr, B., Friedrich, T., Klein, C., Osbild, R. *Unbiased matrix rounding.* SWAT 2006, LNCS 4059:102–112. https://arxiv.org/abs/cs/0604068
- Doerr, B. *Linear discrepancy of basic totally unimodular matrices.* Electron. J. Combin. 7 (2000) R48. https://www.combinatorics.org/ojs/index.php/eljc/article/view/v7i1r48
- Gandhi, R., Khuller, S., Parthasarathy, S., Srinivasan, A. *Dependent rounding and its applications to approximation algorithms.* J. ACM 53(3):324–360, 2006. https://www.cs.umd.edu/~srin/PS/2006/depround-jou.ps
- Granboulan, L., Pornin, T. *Perfect block ciphers with small blocks.* FSE 2007, LNCS 4593:452–465. https://www.bolet.org/~pornin/2007-fse-granboulan+pornin.pdf
- Boldyreva, A., Chenette, N., Lee, Y., O'Neill, A. *Order-preserving symmetric encryption.* EUROCRYPT 2009; full version IACR ePrint 2012/624. https://eprint.iacr.org/2012/624
- Goldreich, O., Goldwasser, S., Nussboim, A. *On the implementation of huge random objects.* SIAM J. Comput. 39(7):2761–2822, 2010. https://www.wisdom.weizmann.ac.il/~oded/PDF/toro.pdf
- Biswas, A.S., Rubinfeld, R., Yodpinyanee, A. *Local access to huge random objects through partial sampling.* arXiv:1711.10692. https://arxiv.org/abs/1711.10692
- Biswas, A.S., Cao, Marcussen, Pyne, E., Rubinfeld, R., Shapira, A., Tauber. *Beyond worst case local computation algorithms.* arXiv:2403.00129 (v2 2025). https://arxiv.org/abs/2403.00129
- Biswas, A.S., Pyne, E., Rubinfeld, R. *Local access to random walks.* ITCS 2022, LIPIcs 215, 24. https://arxiv.org/abs/2102.07740
- Mörters, P., Sohler, C., Walzer, S. *A sublinear local access implementation for the Chinese restaurant process.* APPROX/RANDOM 2022, LIPIcs 245, 28. https://doi.org/10.4230/LIPIcs.APPROX/RANDOM.2022.28
- Dong, Mani. *Random local access for sampling k-SAT solutions.* SAT 2025. https://arxiv.org/abs/2409.03951 (intro and definitions)
- Even, G., Medina, M., Ron, D. *Best of two local models: centralized local and distributed local algorithms.* Inf. Comput. 262(1):69–89, 2018. https://arxiv.org/abs/1402.3796
- Mansour, Y., Rubinstein, A., Vardi, S., Xie, N. *Converting online algorithms to local computation algorithms.* ICALP 2012. https://arxiv.org/abs/1205.1312
- Mansour, Y., Vardi, S. *A local computation approximation scheme to maximum matching.* APPROX 2013. https://arxiv.org/abs/1306.5003
- Levi, R., Rubinfeld, R., Yodpinyanee, A. *Local computation algorithms for graphs of non-constant degrees.* Algorithmica 77(4):971–994, 2017. https://arxiv.org/abs/1502.04022
- Hassidim, A., Mansour, Y., Vardi, S. *Local computation mechanism design.* https://arxiv.org/abs/1311.3939
- Behnezhad, S., Roghani, M., Rubinstein, A. *Local computation algorithms for maximum matching: new lower bounds.* FOCS 2023. https://arxiv.org/abs/2311.09359
- Fischer, M. *Improved deterministic distributed matching via rounding.* https://arxiv.org/abs/1703.00900 (abstract and §1)
- Sanders, P., Schulz, C. *Scalable generation of scale-free graphs.* Inf. Process. Lett. 2016. https://arxiv.org/abs/1602.07106
- Funke, D., Lamm, S., Meyer, U., Penschuck, M., Sanders, P., Schulz, C., Strash, D., von Looz, M. *Communication-free massively distributed graph generation.* IPDPS 2018; JPDC 2019. https://arxiv.org/abs/1710.07565
- Sanders, P., Lamm, S., Hübschle-Schneider, L., Schrade, E., Dachsbacher, C. *Efficient parallel random sampling — vectorized, cache-efficient, and online.* ACM TOMS 44(3), 2018. https://arxiv.org/abs/1610.05141
- Penschuck, M., et al. *Recent advances in scalable network generation.* https://arxiv.org/abs/2003.00736 (§§3, 6, 7, 10)
- Chen, Y., Dinwoodie, I.H., Sullivant, S. *Sequential importance sampling for multiway tables.* Ann. Statist. 34(1):523–545, 2006. https://arxiv.org/abs/math/0605615
- Cryan, M., Dyer, M., Goldberg, L.A., Jerrum, M., Martin, R. *Rapidly mixing Markov chains for sampling contingency tables with a constant number of rows.* SIAM J. Comput. 36(1):247–278, 2006. http://homepages.inf.ed.ac.uk/mcryan/cdgjm.pdf (introduction)
- Barvinok, A. *What does a random contingency table look like?* Combin. Probab. Comput. 19(4):517–539, 2010. https://arxiv.org/abs/0806.3910 (§1)
- Dittmer, S., Lyu, H., Pak, I. *Phase transition in random contingency tables with non-uniform margins.* Trans. AMS 373(12):8313–8338, 2020. https://arxiv.org/abs/1903.08743 (abstract and §1)
- DeSalvo, S., Zhao, J. *Random sampling of contingency tables via probabilistic divide-and-conquer.* https://arxiv.org/abs/1507.00070 (excerpts)
- Patefield, W.M. Algorithm AS 159, via R's `src/library/stats/src/rcont.c` (the translation, not the paper).
- Fellegi, I.P. *Controlled random rounding.* Survey Methodology 1(2):123–133, 1975. https://www150.statcan.gc.ca/n1/pub/12-001-x/1975002/article/54825-eng.pdf
- Cox, L.H., Fagan, J.T., Greenberg, B., Hemmig, R. *Research at the Census Bureau into disclosure avoidance techniques for tabular data.* ASA Proc. SRMS 1986, 388–393. http://www.asasrms.org/Proceedings/papers/1986_072.pdf
- Cox, L.H. *On properties of multi-dimensional statistical tables.* https://www.math.ucdavis.edu/~deloera/MISC/LA-BIBLIO/trunk/CoxLawrence/n-dfract.pdf
- Fagan, Greenberg, Hemmig. Census SRD RR-88/02 (1988); Ernst, Census RR-90/10.
- Bringmann, K., Doerr, B., Neumann, A., Sliacan, J. *Online checkpointing with improved worst-case guarantees.* ICALP 2013. https://arxiv.org/abs/1302.4216
- Chen, T., Xu, B., Zhang, C., Guestrin, C. *Training deep nets with sublinear memory cost.* https://arxiv.org/abs/1604.06174
- Kirisame, M., et al. *Dynamic tensor rematerialization.* https://arxiv.org/abs/2006.09616
- Li, M., Tromp, J., Vitányi, P. *Reversible simulation of irreversible computation.* Physica D 120:168–176, 1998. https://arxiv.org/abs/quant-ph/9703009
- Zhang, H., Constantinescu, E.M. *Optimal checkpointing for adjoint multistage time-stepping schemes.* https://arxiv.org/abs/2106.13879 (quotes Griewank–Walther as Prop. 1)
- Walther, A., Narayanan, S.H.K. *Extending the binomial checkpointing technique for resilience.* https://www.osti.gov/servlets/purl/1364654
- King, S.T., Dunlap, G.W., Chen, P.M. *Debugging operating systems with time-traveling virtual machines.* USENIX ATC 2005. https://www.usenix.org/legacy/event/usenix05/tech/general/king/king.pdf
- Adler, M. zlib `examples/zran.c`. https://github.com/madler/zlib/blob/master/examples/zran.c
- Subversion, `notes/skip-deltas`. https://svn.apache.org/repos/asf/subversion/trunk/notes/skip-deltas
- Vigna, S. *Quasi-succinct indices.* WSDM 2013. https://arxiv.org/abs/1206.4300
- Ottaviano, G., Venturini, R. *Partitioned Elias–Fano indexes.* SIGIR 2014. https://doi.org/10.1145/2600428.2609615
- Pibiri, G.E., Venturini, R. *Techniques for inverted index compression.* ACM Comput. Surv. 2020. https://arxiv.org/abs/1908.10598
- Vigna, S. *Broadword implementation of rank/select queries.* https://vigna.di.unimi.it/ftp/papers/Broadword.pdf
- Mason, C. *Socsim oversimplified.* Harvard Dataverse, 2016. https://doi.org/10.7910/DVN/JOCRUR
- rsocsim (MPIDR). https://github.com/MPIDR/rsocsim
- Even, G., Levi, R., Medina, M., Rosén, A. *Sublinear random access generators for preferential attachment graphs.* TALG 17(4):28, 2021. https://arxiv.org/abs/1602.06159
- Salmon, J.K., Moraes, M.A., Dror, R.O., Shaw, D.E. *Parallel random numbers: as easy as 1, 2, 3.* SC11. https://doi.org/10.1145/2063384.2063405
- Ajtai, M., Aspnes, J., Naor, M., Rabani, Y., Schulman, L.J., Waarts, O. *Fairness in scheduling.* J. Algorithms 29(2):306–357, 1998 (SODA 1995). https://www.cs.yale.edu/homes/aspnes/papers/soda95-full.pdf (§§1–3, 5)
- Naor, M. *On fairness in the carpool problem.* J. Algorithms 55(1):93–98, 2005. https://www.wisdom.weizmann.ac.il/~naor/PAPERS/fair_carpool.ps (introduction and conclusion)
- Bansal, N., Prabhu, Singla, S., Sundaram. *The cube-root phenomenon in online carpooling.* arXiv:2609.21348, 2026 (pp. 1–6).
- Kulkarni, J., Reis, V., Rothvoss, T. *Optimal online discrepancy minimization.* STOC 2024. https://arxiv.org/abs/2308.01406 (§1, §§9–10)
- Aden-Ali, I. *Optimal online discrepancy minimization in linear time.* arXiv:2607.04388, 2026 (pp. 1–4).
- Alweiss, R., Liu, Y.P., Sawhney, M. *Discrepancy minimization via a self-balancing walk.* STOC 2021. https://arxiv.org/abs/2006.14009 (§1)
- Gupta, A., Krishnaswamy, R., Kumar, A., Singla, S. *Online carpooling using expander decompositions.* FSTTCS 2020. https://arxiv.org/abs/2007.10545 (§§1–2)
- Gupta, A., Gurunathan, V., Krishnaswamy, R., Kumar, A., Singla, S. *Online discrepancy with recourse for vectors and graphs.* SODA 2022. https://arxiv.org/abs/2111.06308 (§1)
- Bansal, N., Jiang, H., Singla, S., Sinha, M. *Online vector balancing and geometric discrepancy.* STOC 2020. https://arxiv.org/abs/1912.03350 (Thm 1.4)
- Bansal, N., Jiang, H., Meka, R., Singla, S., Sinha, M. *Online discrepancy minimization for stochastic arrivals.* SODA 2021. https://arxiv.org/abs/2007.10622 (Thm 1.1)
- Liu, Reis. *Weighted chairman assignment and flow-time scheduling.* ITCS 2026. https://arxiv.org/abs/2511.18546 (introduction, §2)
- Bérczi, K., Liu, Reis, Tarnawski. arXiv:2608.13983 (skimmed: counterexample to the offline weighted-carpooling conjecture; "two chains of constraints" are totally unimodular).

**Abstract, metadata or secondary only:**
- Fagin, R., Williams, J.H. *A fair carpool scheduling algorithm.* IBM J. Res. Dev. 27(2):133–139, 1983. https://doi.org/10.1147/rd.272.0133 (via Ajtai et al.)
- Coppersmith, D., Nowicki, T., Paleologo, G., Tresser, C., Wu, C.W. *The optimality of the online greedy algorithm in carpool and chairman assignment problems.* ACM TALG 7(3):37, 2011. https://doi.org/10.1145/1978782.1978792
- Tijdeman, R. *The chairman assignment problem.* Discrete Math. 32:323–330, 1980 (via Liu–Reis).
- Bansal, N., Spencer, J. *On-line balancing of random inputs.* Random Struct. Algorithms 57(4):879–891, 2020. https://arxiv.org/abs/1903.06898
- Altschuler, J., Tikhomirov, K. arXiv:2509.02432 (sparse binary online discrepancy, Θ(log log n)).
- Naor, J., Srinivasan, A., Wajc, D. *Online dependent rounding schemes for bipartite matchings, with applications.* SODA 2025. https://arxiv.org/abs/2301.08680
- Buchbinder, N., Naor, J., Wajc, D. *Lossless online rounding for online bipartite matching (despite its impossibility).* SODA 2023. https://arxiv.org/abs/2106.04863
- Efron, Patel, Stein. SOSA 2025, arXiv:2411.07553 (recourse via cycles).
- Fiat, A., Karlin, A., Koutsoupias, E., Mathieu, C., Zach, R. *Carpooling in social networks.* ICALP 2016 (cited by KRR for Ω(∛log T); not read).
- Doerr, B. *Linear discrepancy of totally unimodular matrices.* Combinatorica 24(1):117–125, 2004. https://doi.org/10.1007/s00493-004-0007-x
- Cox, L.H. *A constructive procedure for unbiased controlled rounding.* JASA 82(398):520–524, 1987. https://doi.org/10.1080/01621459.1987.10478456 (via Cox 1986 and the n-d paper)
- Cox, L.H., Ernst, L.R. *Controlled rounding.* INFOR 20(4):423–432, 1982. https://doi.org/10.1080/03155986.1982.11731877
- Causey, B.D., Cox, L.H., Ernst, L.R. *Applications of transportation theory to statistical problems.* JASA 80(392):903–909, 1985. https://doi.org/10.2307/2288551
- Bacharach, M. *Matrix rounding problems.* Management Science 12(9):732–742, 1966. https://doi.org/10.1287/mnsc.12.9.732
- Baranyai, Zs. *On the factorization of the complete uniform hypergraph.* Infinite and Finite Sets, Colloq. Math. Soc. J. Bolyai 10, 1975 (via Doerr et al.).
- Diaconis, P., Gangolli, A. *Rectangular arrays with fixed margins.* IMA Vol. 72:15–41, 1995. https://doi.org/10.1007/978-1-4612-0801-3_3
- Diaconis, P., Sturmfels, B. *Algebraic algorithms for sampling from conditional distributions.* Ann. Statist. 26(1):363–397, 1998. https://doi.org/10.1214/aos/1030563990
- Chen, Y., Diaconis, P., Holmes, S., Liu, J.S. *Sequential Monte Carlo methods for statistical analysis of tables.* JASA 100(469):109–120, 2005. https://doi.org/10.1198/016214504000001303
- Dyer, M., Kannan, R., Mount, J. *Sampling contingency tables.* Random Struct. Algorithms 10(4):487–506, 1997.
- Madow, W.G. *On the theory of systematic sampling, II.* Ann. Math. Statist. 20(3):333–354, 1949. https://doi.org/10.1214/aoms/1177729988
- Goodman, R., Kish, L. *Controlled selection.* JASA 45:350–372, 1950; Bryant, E.C., Hartley, H.O., Jessen, R.J. *Design and estimation in two-way stratification.* JASA 55:105–124, 1960 (via Kim, Heeringa, Solenberger, Survey Methodology 40(2), 2014).
- Deville, J.-C., Tillé, Y. *Efficient balanced sampling: the cube method.* Biometrika 91(4):893–912, 2004 (via Chauvet, Bernoulli 18(4), 2012, arXiv:1211.5442).
- Fog, A. *Sampling methods for Wallenius' and Fisher's noncentral hypergeometric distributions.* Comm. Statist. Simul. Comput. 37(2):241–257, 2008.
- Even, G., Medina, M., Ron, D. *Deterministic stateless centralized local algorithms for bounded degree graphs.* ESA 2014, LNCS 8737:394–405.
- Feige, U., Patt-Shamir, B., Vardi, S. *On the probe complexity of local computation algorithms.* ICALP 2018. https://arxiv.org/abs/1703.07734
- Griewank, A. *Achieving logarithmic growth of temporal and spatial complexity in reverse automatic differentiation.* Optim. Methods Softw. 1(1):35–54, 1992. https://doi.org/10.1080/10556789208805505
- Griewank, A., Walther, A. *Algorithm 799: revolve.* ACM TOMS 26(1):19–45, 2000. https://doi.org/10.1145/347837.347846
- Stumm, P., Walther, A. *New algorithms for optimal online checkpointing.* SIAM J. Sci. Comput. 32(2):836–854, 2010. https://doi.org/10.1137/080742439
- Wang, Q., Moin, P., Iaccarino, G. *Minimal repetition dynamic checkpointing algorithm for unsteady adjoint calculation.* SIAM J. Sci. Comput. 31(4):2549–2567, 2009. https://doi.org/10.1137/080727890
- Bennett, C.H. *Time/space trade-offs for reversible computation.* SIAM J. Comput. 18(4):766–776, 1989. https://doi.org/10.1137/0218053
- Levine, R.Y., Sherman, A.T. *A note on Bennett's time-space tradeoff for reversible computation.* SIAM J. Comput. 19(4):673–677, 1990. https://doi.org/10.1137/0219046
- Ahlroth, L., Pottonen, O., Schumacher, A. *Approximately uniform online checkpointing.* Algorithmica 2013. https://doi.org/10.1007/s00453-013-9772-5
- Gosper, R.W. *Exploiting regularities in large cellular spaces.* Physica D 10:75–80, 1984 (via Wikipedia, "Hashlife").
- Haramoto, H., et al. *Efficient jump ahead for F₂-linear random number generators.* INFORMS J. Comput. 20(3):385–390, 2008. https://doi.org/10.1287/ijoc.1070.0251
- Beckman, R.J., Baggerly, K.A., McKay, M.D. *Creating synthetic baseline populations.* Transp. Res. A 30(6):415–429, 1996. https://doi.org/10.1016/0965-8564(96)00004-3
- Caswell, H. *The formal demography of kinship: a matrix formulation.* Demogr. Res. 41:679–712, 2019. https://doi.org/10.4054/DemRes.2019.41.24

**Recalled, not verified this session:**
- Union of two laminar families gives a TU matrix (Edmonds; Schrijver, *Combinatorial Optimization*).
- Linial (1992), *Locality in distributed graph algorithms*: 2-colouring a ring needs Ω(n) rounds.
- Valiant (1979) on the permanent; Jerrum–Sinclair–Vigoda (2004) FPRAS.
- Kachitvichyanukul–Schmeiser hypergeometric sampler (via BCLO).
- Grimm, Pottier, Rostaing-Schmidt (1996), optimality of binomial checkpointing.
