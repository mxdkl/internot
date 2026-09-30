# Local access, keyed bijections, and consistent relationships

**Date:** 2026-09-29
**Status:** Research
**Context:** Reframes relationship generation (partners, households, coworkers) as consistent local access to a huge random object. Builds on, and does not repeat, [deterministic-graph-generation](2026-05-14-deterministic-graph-generation.md) (hash-threshold pairs, hyperbolic cells, cached stable roommates) and [graph-theory-foundations](2026-05-14-graph-theory-foundations.md) (which models are latent-variable, and so pure-function-friendly). The bug that prompted this note: `people/slot/tier3.rs` derives spouse, manager and mentor as `mail_id ^ xor_seed(pid)`, and `people/family.rs` searches from one side only. The results are neither symmetric nor guaranteed to land on a populated `member_idx < workplace_size_for(..)`.

## Summary: what is directly usable

1. The literature splits local access into **stateful** oracles (BRY Def. 9, Even et al.) and **memory-less** ones (BRY Def. 10, LCAs, GGN oracle machines). Only memory-less ones meet Internot's constraint, and BRY list "memory-less access to undirected random graphs" and "degree queries for G(n,p)" as **open problems**. So don't try to reproduce independent-edge G(n,p) or Chung–Lu with neighbour lists.
2. Constructions that are memory-less and exact share one pattern: **both endpoints derive the tie from a key they can both name** (a group id, a slot in a keyed bijection, a canonical pair), and the set of keys naming x can be listed from x alone. The working hypothesis is the right frame.
3. The keyed bijection to use on any [0,n) is the **Black–Rogaway generalized Feistel over Z_a×Z_b** with a=⌈√n⌉, plus cycle-walking. Measured: 14 ns forward or inverse at n≈4·10⁹ with 6 rounds, 1.00002 walk steps on average. Use **at least 4 rounds**: with 2–3 rounds, sequential inputs show strong structure (chi²/df ≈ 19).
4. **Pairing:** σ(x)=π⁻¹(π(x)⊕1) is a uniformly random perfect matching when π is uniform, and costs 36 ns. A union of d such involutions is contiguous to a uniform random d-regular graph (Wormald, Thm 4.15).
5. **Attribute-aware pairing without search:** block coupling with an integer transport plan M(a,b) over slot ranges. Reciprocity is exact, and the matching is uniform given the plan. Exact stable matching is provably non-local; only almost-stable and greedy LCAs are local, and only on bounded-degree candidate graphs.
6. **Variable-size groups without holes:** a sort-by-size layout composed with a keyed permutation is a uniformly random set partition with the *exact* size histogram. Finding a person's group costs O(log K); listing a group costs O(size).
7. **Random (not prescribed) counts:** GGN's interval-sum tree (binomial or Poisson splits keyed by node hash) gives memory-less rank and select in O(log n) samples. Consistency is structural: it does not depend on how accurate the sampler is.
8. **The configuration model is memory-less after all:** a stub space (prefix-summed degrees) plus a keyed involution on stubs. This corrects R6 of the 2026-05-14 note.
9. **Time:** staggered re-keying (O(1)), sliding windows (O(1), FIFO turnover), or an **interchange process driven by hash-derived Poisson clocks** (cost ∝ moves of the traced person). A prototype was bijective and invertible at arbitrary t, at about 2 µs per query for 50 simulated years.
10. **Where the hypothesis breaks:** holes in the id space (dense ids are needed first), correlation across relations (needs a derivation DAG: couples → households → neighbourhoods), large groups (co-membership is not a tie), preference matching, heavy-tailed degree (needs stub spaces), and time (§6).

## 0. Framing

An *implementation* answers queries about one global object X drawn from a target distribution D. Three properties matter:

- **Consistency:** every answer agrees with a single X.
- **Fidelity:** X ~ D, or X is close to D.
- **Memory-lessness:** an answer is a function of (seed, query) only. Query order doesn't matter, and two copies of the implementation always agree.

Internot needs all three, and memory-lessness is non-negotiable.

The crux for symmetric relations: **both endpoints must reach the same random bits.** A per-person seed (`spouse_xor_of(pid)`) is read by one endpoint only, so B cannot know that A chose it. A correct construction routes the decision through something both can name: a canonical pair (min,max), a group id, or a slot index of a shared bijection.

Enumeration also needs the set of such objects naming x to be computable without search. Canonical pairs fail this: there are n candidates, which is why the old note needed cell pushdown. Groups and slots pass: the inverse map lists them directly.

## 1. Huge random objects and local access

### 1.1 Goldreich–Goldwasser–Nussboim (FOCS 2003; SIAM J. Comput. 39(7), 2010)

**Model.** A *specification* is a machine reading a random tape; it may be infeasible to run. An *implementation* is an oracle machine with a random function as its only randomness, so it is **memory-less by construction**. Replace the oracle with a keyed hash and you get Internot's model exactly.

- *Close* means indistinguishable from the specification by polylog-query observers.
- *Truthful* (Def. 2.8) means the implemented object always lies in the specification's support: for example, it is always d-regular, never "d-regular with high probability".

**Interval sums (§5, Thms 5.1/5.2).** Put a binary tree of depth n over [2ⁿ]. Each left child's label comes from the oracle at that node, and each right child is parent − left (XOR for parity; for sums, the internal labels are drawn from binomial-type laws). Any node's label costs ≤ n+1 oracle calls, and an interval sum is the combination of at most 2n−1 subtree roots. This is the primitive behind §4.3 and §1.6.

**Bounded-degree graphs (Thm 3.10/8.4, Lemma 3.11/8.1).** For d>2, a random relabelling of *any fixed* d-regular graph of girth g cannot be distinguished from a uniformly random simple d-regular graph by q neighbourhood queries, except with probability O(q²/(d−1)^{(g−1)/2}). GGN get truthful close-implementations of random d-regular graphs with extra properties (connectivity, Hamiltonicity, logarithmic girth).

**For Internot:** this is the "coordinate system" hypothesis in its purest form: fixed structure plus keyed relabelling. The caveat is that global statistics are those of the fixed structure; they are not re-sampled.

GGN also implement random functions with inverse queries and iterated evaluation f^m(x) (Thms 3.3/3.4). These are not needed here.

### 1.2 Biswas–Rubinfeld–Yodpinyanee (arXiv 1711.10692)

**Model.** Def. 9 is an oracle *with internal state*, required to be consistent with one X from a distribution within ε=n^{−c} of the target. Def. 10 is **memory-less**: shared public random bits, no state, so answers don't depend on query order and independent copies agree.

**Results:**
- *Undirected graphs with independent p_uv* (Thm 1): Vertex-Pair, Next-Neighbor and Random-Neighbor in polylog time, space and random bits per query. This assumes ∏_{u=a}^{b}(1−p_vu) and Σ p_vu over index ranges can be computed in polylog time.
  - G(n,p), any p (Cor. 2): O(log³n) time, O(log³n) bits, O(log²n) space per query, w.h.p.
  - SBM with r randomly assigned communities (Cor. 3): O(r·polylog n) per query.
- *Kleinberg small-world* (directed), All-Neighbors: O(log²n) (Thm 5).
- *Dyck paths*: Height and First-Return queries, polylog.
- *Uniform q-colourings* of a huge bounded-degree graph, q≥9Δ: memory-less, and sublinear when q>12Δ (Thm 7).

**Why the graph oracles keep state.** When skip-sampling picks u′ as v's next neighbour after u, every vertex in (u,u′) must later refuse v as a neighbour, and there are potentially Θ(n) of them. Random-Neighbor must also avoid computing degrees, because a degree depends on n independent edges.

**Open problems listed (§6):** degree queries for G(n,p); **memory-less basic queries for undirected random graphs**; local access to random perfect matchings of lattices.

**Takeaways for Internot:**
- (i) Memory-less, exact, independent-edge undirected graphs are open. Target other distributions.
- (ii) Out-edge (directed) models are naturally memory-less, because each source's list depends only on its own randomness (inferred for the Kleinberg case). They give no reciprocity.
- (iii) BRY need a multivariate-hypergeometric tree to place SBM communities randomly over *fixed* labels. Internot chooses its own labelling, so a community can simply be a contiguous range of π(x). Counts are then trivial.

### 1.3 Other generators

- **Naor–Nussboim (RANDOM 2007):** sparse G(n,p) with p=polylog(n)/n and All-Neighbors queries. Per BRY §1.1, the output only *appears random to algorithms inspecting a limited portion*.
- **Naor–Nussboim–Tromer (TCC 2005):** huge graphs preserving first-order properties of random graphs (metadata only).
- **Even–Levi–Medina–Rosén (TALG 2021):** Barabási–Albert and random recursive trees on the fly. Each query costs polylog(n) time w.p. 1−1/poly(n), and each query adds polylog(n) space. This is **stateful**, and I know of no memory-less version (inferred). The static substitute stays PSO/hyperbolic, as in the earlier note.

### 1.4 Local computation algorithms (LCAs)

- **Rubinfeld–Tamir–Vardi–Xie (ICS 2011)** define LCAs: answer queries about locations of *some* legal output y∈F(x), consistently with one y.
- **Alon–Rubinfeld–Vardi–Xie (SODA 2012)** achieve polylog time *and space* using bounded-independence pseudorandomness, with parallel queries answered consistently. This is memory-less given a short seed.
- **Ghaffari (FOCS 2022):** an MIS LCA with poly(Δ)·log n probes. Applied to the line graph, it gives maximal matching (standard reduction; inferred).
- **Random-order greedy oracles** (Nguyen–Onak 2008; Yoshida–Yamamoto–Ito, STOC 2009): rank edges by hash, and put e in M iff no adjacent lower-ranked edge is in M. The oracle recurses on lower-ranked neighbours. YYI bound the expected recursion for a random query by roughly 1+m/n (recalled, not re-read). The randomness *is* the hash; memoization only speeds it up.

### 1.5 Summary table

| Object | Queries | Guarantee | Cost / query | Memory-less? |
|---|---|---|---|---|
| Random function + interval sums (GGN §5) | eval, Σ over interval | exact, truthful | O(log N) oracle calls | yes (oracle → hash) |
| Random d-regular (GGN §8) | all-neighbours | truthful; close vs q-query observers | polylog | yes |
| Sparse G(n,p) (Naor–Nussboim) | all-neighbours | looks random to limited inspection | polylog | per BRY: bounded-use |
| G(n,p), SBM (BRY) | pair, next-nbr, random-nbr | ε-close to exact | O(log³n) | **no** (open) |
| Kleinberg small-world, directed (BRY) | all-neighbours | exact | O(log²n) | yes in structure (inferred) |
| q-colouring (BRY) | colour(v) | ε-close to uniform | sublinear, q>12Δ | yes |
| BA / recursive tree (Even et al.) | next-nbr | exact | polylog w.h.p. | no |
| Greedy MIS / matching (LCA) | in-set(v/e) | consistent legal output | poly(Δ)·log n | yes |

### 1.6 Construction: a memory-less degree-corrected graph via Poisson stubs (inferred synthesis)

Independent-edge graphs are out of reach (§1.2). **Stub pairing** is in reach, and with Poisson stub counts it approximates Chung–Lu / Norros–Reittu (inferred; van der Hofstad, *Random Graphs and Complex Networks* vol. 1, recalled).

```text
// One block pair {A,B} (A may equal B). Person weights w_i are a closed-form function of the
// person's rank inside A (e.g. a Pareto quantile at (r+0.5)/n_A), so W_A[lo,hi) has a closed form.
K_AB = Poisson(λ_AB) from hash(seed,"stubs",min(A,B),max(A,B))   // one total per unordered pair
// Spread K_AB over A's rank range with a binary tree (the GGN interval-sum pattern):
split(node=[lo,hi), k): left_k = Binomial(k, W_A[lo,mid)/W_A[lo,hi)) from hash(seed,A,B,lo,hi)
stubs_A(x) = [prefix_A(x), prefix_A(x)+leaf_count(x))    // O(log n_A) samples
owner_A(s) = descend, choosing the child by prefix           // O(log n_A) samples
τ = keyed bijection on [0,K_AB)   (A==B: involution π⁻¹(π(s)⊕1); an odd K leaves one stub free)
neighbours_AB(x) = { owner_B(τ(s)) : s ∈ stubs_A(x) } minus self-loops and duplicates
```

Why this works:
- Given K, per-node counts are Multinomial(K; w_i/W). Degrees are therefore mixed-Poisson, and the pairing is uniform.
- **Degree is a local query here** (one range). That is exactly what BRY leave open for independent-edge G(n,p); the difference is that this model is not independent-edge.
- Children always sum to their parent, so consistency holds even if the Binomial sampler is approximate.

Cost: about 2×32 hash-seeded binomial samples per neighbour, a few µs (estimate, not measured).

### 1.7 Pitfalls

- GGN/BRY "indistinguishable" is against observers making **few** queries. Evaluators aggregate, so check the global statistics you commit to.
- Never "retry on collision" along a query path unless the retry is keyed only by objects both endpoints see.
- Drop self-loops and duplicate edges **symmetrically**: both endpoints see the same stub pair, so both drop it.

## 2. Keyed bijections on arbitrary finite domains

### 2.1 Constructions

**Black–Rogaway (CT-RSA 2002).**
- *Method 1 (prefix cipher)* sorts hash(key,i). It is an exact table, so use it only for tiny per-block domains.
- *Method 2 (cycle-walking)* runs E on a superset T⊇M and re-applies E until the output lands in M. The inverse walks backwards. **Thm 1:** if E is an ideal cipher on T, the result is a *uniform* permutation on M. With T the next power of two, the expected number of E calls is below 2.
- *Method 3 (generalized Feistel Fe[r,a,b])* uses ab≥k, L=m mod a, R=⌊m/a⌋. Round j sets L,R ← R, (L+F_j(R)) mod (a if j is odd, else b). The output is aL+R if r is odd, else aR+L, and out-of-range outputs are cycle-walked.

**Swap-or-not (Hoang–Morris–Rogaway, CRYPTO 2012).** Round i over Z_N:

```text
X′ ← K_i − X (mod N)
X̂  ← max(X, X′)
if F_i(X̂) = 1 then X ← X′
```

Each round is a keyed involution with a per-pair coin, so the inverse runs the rounds in reverse. It works natively on any N, with no walking.
- Security bound: Adv ≤ (8N^{3/2}/(r+4))·((q+N)/(2N))^{r/4+1}. "Roughly r = 6 lg N rounds to start to see a good bound."
- Their example: N≈2³⁰ needs 340 rounds for advantage <10⁻¹⁰ at q=10⁸.
- A point is left unmoved with probability ≈2^{−r}, so even statistical use needs r ≳ lg N (measured below).

**Granboulan–Pornin (FSE 2007).** An *exactly* uniform permutation using O((log n)³) RNG calls and O(log n) space per forward or inverse evaluation. It uses a binary tree of splits with hypergeometric counts. HMR call it impractical because of extended-precision hypergeometric sampling.

**NIST FF1/FF3-1 (SP 800-38G).** AES-based Feistel over numeral strings. The Rev. 1 second public draft (3 Feb 2025) removes FF3/FF3-1 after Beyne's tweak-schedule attack and keeps a minimum domain size of one million. FF1 uses 10 AES-based rounds (recalled). Cryptographic strength and AES cost are irrelevant here: **do not use**.

**Kensler (Pixar TM 13-01, 2013), `permute(i,l,p)`.**
- Mask w = next power of two minus one.
- Loop until i<l: a fixed chain of reversible operations (XOR with a constant, multiplication by an odd constant, XOR-shift of the masked value).
- Return (i+p) mod l.
- The constants were hill-climbed for avalanche. It is 32-bit with a 32-bit key p, and no inverse is given; every operation is invertible mod 2^k, so one can be derived (inferred).

**FastPRP (Stefanov–Shi, ePrint 2012/254)** reaches strong security through precomputation and caching. It is table-like, so skip it.

### 2.2 Measured cost and quality

**Setup:** scratch Rust (not committed), single thread, Ryzen 9 5900HX, rustc 1.97.1 at `-O3`. The round function is the splitmix64 finalizer of (x ⊕ round key), reduced with Lemire's method.

**Cost:**

| Construction | n | Forward | Inverse |
|---|---|---|---|
| BR Fe[4,a,b] + walk | 4.0·10⁹ | 9.3 ns | 9.4 ns |
| **BR Fe[6,a,b] + walk** | 4.0·10⁹ | **14.3 ns** | **14.4 ns** |
| BR Fe[8,a,b] + walk | 4.0·10⁹ | 21.5 ns | 21.5 ns |
| Swap-or-not, r=32 / 64 / 96 | 4.0·10⁹ | 177 / 378 / 578 ns | same |
| Involution π⁻¹(π(x)⊕1) on Fe[6] | 4.0·10⁹ | 35.8 ns | (self-inverse) |
| Kensler `permute` | 10⁶ | 4.2 ns | (not provided) |
| Swap-or-not, r=60 | 10⁶ | 289 ns | same |

**Quality at n=1,000,003** (every construction was verified bijective; ideal is ≈1.0 for both ratios):
- *Adjacency chi²/df* is computed over a 32×32 grid of (bucket π(x), bucket π(x+1)).
- *Block ratio* counts pairs of inputs in the same 1024-aligned input block that land in the same 1024-aligned output block, divided by the uniform expectation.

| Construction | Adjacency chi²/df | Block ratio | Fixed points |
|---|---|---|---|
| Fe[2] | **19.3** | 1.03 | 0 |
| Fe[3] | **19.3** | 1.10 | 0 |
| Fe[4] / Fe[6] / Fe[8] | 0.96 / 0.96 / 0.94 | 1.00 | 0 / 1 / 1 |
| Swap-or-not, r=10 | 0.93 | **1.52** | **1000** (≈ n·2⁻¹⁰) |
| Swap-or-not, r=20 / 60 | 0.96 / 0.98 | 1.00 | 2 / 0 |
| Kensler | 0.92 | 1.00 | 2 |

**Why 2–3 Feistel rounds fail.** Sequential inputs share R=⌊x/a⌋ and differ by +1 in L. After round 1 they still differ by +1 in one half, and the next round's F values are **reused across all rows**. The consecutive differences π(x+1)−π(x) therefore take only about a distinct values, each repeated about b times. A split confirmed this: same-row pairs gave chi²/df 19.3, cross-row pairs 1.05. Four rounds remove it.

Internot indexes blocks sequentially, so this matters: **4 rounds minimum, 6 by default.**

### 2.3 Domains that are not a power of two

- **Walk to the next power of two:** fewer than 2 expected calls, worst just above a power of two. The inverse must walk too.
- **BR a×b:** with a=⌈√n⌉ and b=⌈n/a⌋, 0 ≤ ab−n < a, so the wasted fraction is below 1/√n. Measured: 1.000019 calls at n=4·10⁹.
- **Swap-or-not:** native on Z_n, but about 20× slower at comparable quality.
- **Tiny blocks (n ≤ 16–64):** a memoized hash-sorted table (Method 1) is exact and fast.

### 2.4 Structured permutations

- **Blocked.** For block j = [s_j, s_j+n_j): π(x) = s_j + P_{key(j)}(x−s_j). The block is preserved, and each block gets an independent key.
- **Preserving coordinates.** Write x in mixed radix and permute only the free digits (jointly, on their product domain). Fixed digits such as city or cohort pass through unchanged.
- **Variable-size groups.** Compose π with a partition (§4): π(x) gives an index, and the partition turns the index into (group, slot).
- **Subsets with dense ranks.** S={x : π(x)<m} is a hash-chosen subset of *exact* size m, with rank_S(x)=π(x). Prefer this to per-person coin flips whenever a further bijection must run on the subset (remarriage pools, §3.7).
- **Growth stability (inferred, elementary).** Fix a permutation E on a capacity C and cycle-walk into [0,n). Growing n→n+1 changes **exactly one** old image: the unique x whose walk first hits n now maps to n, and n inherits x's old image. By contrast, re-keying a Feistel with a new n reshuffles everything. Use a fixed capacity for populations that grow (birth cohorts).

### 2.5 Pitfalls

- No floating point inside bijections, for cross-platform determinism.
- Derive round keys once per (relation, block, epoch) with the existing `hash_*`, then use an integer mixer per round. Streaming xxh3 over a string key in every round would dominate the cost (inferred).
- Never cap a cycle-walk loop: a cap breaks bijectivity. Its tail is geometric.
- Balanced Feistel on 2^m only produces even permutations (recalled folklore). This is irrelevant for simulation.

## 3. Pairings and matchings as algebra

### 3.1 A perfect matching from a permutation

On [0,n) with n even, σ = π⁻¹∘τ∘π with τ(y)=y⊕1.

**Claim:** if π is uniform on S_n, σ is uniform over fixed-point-free involutions, i.e. a uniform perfect matching. *Proof:* conjugation preserves cycle type, and all fixed-point-free involutions are conjugate to τ. For odd n, the last slot has no partner and stays single.

The pair id k=⌊π(x)/2⌋ is shared by both partners. Key everything pair-level on it: marriage date, divorce date, shared latents.

### 3.2 Partial matchings

- **Rate-based:** x is single iff hash(key,k) < q. Both partners compute the same k, so this is symmetric.
- **Count-based:** ranks < 2m are paired and the rest are single. This gives an exact couple count m (from demographic tables), and the singles form a dense subset (ranks ≥ 2m) that can feed a second pairing.

### 3.3 Constrained matchings: block coupling

**Market** (e.g. city×decade). Blocks a=1..K (for example age band × sex × orientation class), with sizes n_a that are pure functions of the market key.

**Plan.** A symmetric nonnegative integer matrix M, where M(a,b) is the number of couples spanning a and b. Row usage is r_a = 2M(a,a) + Σ_{b≠a} M(a,b) ≤ n_a. Compute M deterministically from the target mixing kernel (IPF, then largest-remainder rounding): O(K²·iters), a pure function of the market, and memoizable.

**Layout.** In π_a-rank order, block a holds consecutive slices S_a(b) of length M(a,b) (or 2M(a,a) when b=a), with offsets o_a(b). Ranks ≥ r_a are single.

```rust
fn partner(x: Person) -> Option<Person> {
    let a = block(x); let r = perm[a].fwd(rank_in_block(x));
    if r >= used[a] { return None; }
    let b = slice_of(a, r);                 // binary search over o_a: O(log K)
    let j = r - off[a][b];
    let slot = if b == a { off[a][a] + (j ^ 1) } else { off[b][a] + j };
    Some(unrank(b, perm[b].inv(slot)))      // two permutation evaluations in total
}
```

**Reciprocity** is exact: slot j of slice (a,b) is linked to slot j of slice (b,a).

**Uniformity:** with independent uniform π_a, the matching is uniform among matchings realising M, because S_{n_1}×…×S_{n_K} acts transitively on them (inferred, orbit argument).

**Fine-grained assortativity without search.** Draw u = hash(seed, market, a, b, j) once per couple slot. Derive later attributes (education, income percentile, religiosity) as f(own id, u) to get a tunable copula. This needs a **derivation DAG**: attributes that define blocks (birth year, sex, city) must never depend on the partner.

### 3.4 Unions of matchings, and regular graphs

- **Involutions.** With d independent keyed involutions σ_k, set neighbours(x) = {σ_k(x)}. Wormald's survey, Thm 4.15(ii),(iii) and eq. (26): for n even and d≥3, **G_{n,d} ≈ d·G_{n,1}** (contiguity, i.e. the same a.a.s. properties), where ⊕ is the sum conditioned on being simple.
- **Duplicates.** The unconditioned sum has on average about d(d−1)/4 duplicated edges in total, independent of n (inferred: each pair of matchings shares ≈½ an edge). Drop them.
- **Permutation model.** Greenhill–Janson–Kim–Wormald (CPC 2002): a random 4-regular pseudograph is contiguous to the union of two permutation pseudographs (edges {i, π(i)}). So neighbours {π_k(x), π_k⁻¹(x)} is another valid local model.
- **Use** these as a sparse background-acquaintance layer: exact degree, locally tree-like, low clustering. Clustering has to come from groups.

### 3.5 The configuration model via stubs

Degrees d_x define a stub space through a partition (§4). A keyed involution on stubs gives neighbours = owners of partner stubs. The degree sequence is exact (erased configuration model: drop loops and duplicates symmetrically). Blocked stub spaces (stubs of city c paired only within c) give assortativity.

This is point-deterministic, so the "sequence-dependent" objection in R6 of the 2026-05-14 note does not apply.

### 3.6 Preference-based matching and locality

- **Exact stable matching is non-local.** Kipnis–Patt-Shamir (ICDCS 2009) give a distributed lower bound of Ω(√n/log n) rounds (recalled, not re-read). The existing `graph::matching::stable_roommates_match` plus its cohort cache is a per-cohort global pass. It is acceptable only as a memo of a pure function over an exactly enumerable cohort.
- **Almost-stable matching is local.**
  - Floréen–Kaski–Polishchuk–Suomela: the ratio of matched individuals to blocking pairs grows linearly with the number of Gale–Shapley rounds, so a constant number of rounds suffices when preference lists have constant length.
  - Hassidim–Mansour–Vardi: an LCA achieving an arbitrarily good approximation to a stable matching when the men's lists are bounded.
- **Greedy maximal matching** over hash-ranked edges is memory-less (§1.4), but it needs a **bounded-degree candidate graph that can be listed locally** (e.g. the top 8 candidates per person inside a block).
- **Recommendation:** block coupling plus shared latents for demographics. Use LCAs only when a scenario truly needs preference semantics.

### 3.7 Pitfalls, and "partner at time t"

- **Partner at time t:** the involution is static within a birth-cohort block, and time comes from the pair key: partner_at(x,t) = σ(x) iff m(k) ≤ t < d(k).
- **Remarriage:** choose divorcing couples by a *couple-level* permutation prefix, which gives the divorced persons dense ranks in a second pool. A second coupling follows, with date = max(d(k_x), d(k_y)) + gap(k′), where k′ is the new pair key. Both partners compute the same value.
- Handle odd block sizes, give orientation classes their own blocks, and compute M from block sizes, never from sampled persons.

## 4. Variable-size groups without holes

The requirement: group_of(i)→(g, slot) and members(g)→range are both cheap, every index is covered exactly once, and group sizes follow a target distribution.

### 4.1 Size-class layout (recommended default)

**Layout.** Take a histogram {(s, c_s)}, meaning c_s groups of size s, with Σ s·c_s = n. Region R_s=[P_s, P_s+s·c_s) holds all groups of size s.
- group_of(i): binary-search s over the K class starts; then g = G_s+⌊(i−P_s)/s⌋ and slot = (i−P_s) mod s.
- members(g): binary-search over G to get a contiguous range.

Apply this to π(x). **Claim:** a uniform π gives a uniformly random set partition with that *exact* histogram (the symmetric group acts transitively on partitions of one type). Cost: one permutation plus O(log K).

**Getting c_s.** Start from the *group*-size pmf p_s, not the size-biased q_s = s·p_s/Σ s·p_s that a random person experiences. Set c_s = round(G·p_s) with G ≈ n/E[s], then absorb the residual with singletons.

### 4.2 Heavy tails (firms up to 10⁵)

Use log-binned classes. Inside a bin, use *superblocks* of B groups whose sizes are a fixed stratified quantile set of the within-bin distribution, shuffled per superblock by a tiny keyed permutation of [0,B). The superblock length L is then constant, so finding the superblock is O(1) and finding the group inside it is O(B), or O(log B) with a memoized prefix.

An alternative is antithetic pairs (s, T−s) of constant total. That needs a within-bin distribution symmetric about T/2 (inferred design).

### 4.3 Random counts: the interval-sum tree (GGN §5)

**Setup.** Treat group starts as Bernoulli(q) per index, which gives geometric sizes. Over a dyadic tree:
- count(root) ~ Bin(n,q);
- count(left | parent=k) ~ Hypergeometric(|node|, |left|, k), seeded by hash(seed, level, node).

**Queries.** rank(i) = number of starts ≤ i, computed in O(depth) samples; select(g) descends the tree. For weighted Poisson counts, use the Binomial split of §1.6.

**Properties.** Consistency is structural. Estimated cost is 32 levels × 50–200 ns per hash-seeded sample, about 2–6 µs (estimate). Use this when group or degree counts must *fluctuate* rather than match a histogram.

### 4.4 Recursive random splits (fragmentation)

Split [lo,hi) at lo+⌊(hi−lo)·β⌋ with β~Beta(α,α) from hash(node), and stop when the size falls below a hashed threshold. A leaf's size is a product of split fractions, so log-size is a sum and is approximately lognormal (multiplicative CLT / Gibrat's law).

group_of costs O(depth), and members is a leaf range. Nesting comes free (internal nodes can be neighbourhoods or departments), but hitting an exact target distribution is hard.

### 4.5 Hashing into bins: reject

group = hash(x) mod G gives Poisson(n/G) sizes, and listing members needs a scan or bit-field pushdown. That is the problem `Space::find` currently works around.

### 4.6 Nesting and units

City → neighbourhood → household: π_city ranks the city's persons, and a neighbourhood partition runs over that rank space. Inside each neighbourhood, re-permute with key(neighbourhood) and apply the household partition.

The household partition must run over a **unit space**: a couple from §3 is one unit of size 2, plus singles and dependants. Otherwise spouses land in different households.

| Method | group_of | members | Size control | Nesting |
|---|---|---|---|---|
| Fixed size s | O(1) | O(s) | constant | via blocks |
| Size classes (§4.1) | O(log K) | O(s) | exact histogram | yes |
| Quantile superblocks (§4.2) | O(1)+O(B) | O(s) | stratified, heavy tails | yes |
| Interval-sum tree (§4.3) | O(log n) samples | O(s)+O(log n) | random (geometric / Poisson) | yes |
| Fragmentation (§4.4) | O(depth) | O(s) | ≈ lognormal, loose | native |
| Hash into bins | O(1) | scan | Poisson only | no |

**Pitfalls:**
- Size-biasing (§4.1).
- Changing any parameter reshuffles everyone (see §2.4 for growth).
- Exact histograms make per-city counts deterministic: the number of 7-person households in a given city has no sampling noise.

## 5. Permutations that evolve over time

Goal: π_t: persons→seats, with both π_t and π_t⁻¹ cheap at arbitrary t and small change per unit time.

1. **Epoch re-key:** π_t = P_{key(⌊t/T⌋)}. O(1), but everyone moves at each boundary. Suitable only for per-event groupings (a conference table, a weekly rota).
2. **Staggered re-key:** split persons and seats into C classes of equal counts, using permutation-prefix ranges. Class c has period T_c and phase φ_c, and seat_t(x) = seatmap_c(P_{c,⌊(t+φ_c)/T_c⌋}(rank_c(x))).
   - Each boundary moves 1/C of the population, and tenure in class c is exactly T_c. Choose (T_c) as a discretised tenure distribution (remember length-biasing).
   - O(1) in both directions. A move is a full reshuffle within the class and block, with no locality.
3. **Sliding window ("conveyor"):** q_t(x) = (π(x)+⌊vt⌋) mod n, and group = partition(q). O(1). Each group swaps one member per 1/v, and everyone stays s/v in a group before moving to the *adjacent* group (FIFO). Several belts with different speeds give a mix of tenures.
4. **Composed sparse rounds:** π_e = ρ_e∘…∘ρ_1∘π_0, where ρ_e is a swap-or-not round with swap probability ρ. It is invertible (reverse the rounds), but **the cost is O(e) whatever x did**, and no way to skip rounds is known. Capping by periodic hard re-keys creates churn spikes.
   - Mixing references: random transpositions mix after about ½·n·log n steps (Diaconis–Shahshahani 1981), and riffle shuffles after about (3/2)·log₂n (Bayer–Diaconis 1992) (both recalled). Each riffle needs rank queries (interval-sum tree per step), so it is not worth it.
5. **Interchange process with hash-derived clocks (recommended for continuous, local drift).**
   - Seats [0,S); the swap graph is the union of D keyed involutions σ_k inside a block.
   - Each edge {v, σ_k(v)} carries a rate-λ Poisson clock. Its events in time bucket j come from hash(seed, k, min, max, j), so they are symmetric. At each event the two occupants swap.
   - `fwd(v,t)`: from s=0, repeatedly take the earliest event after s on the D edges at the current seat, stopping at t.
   - `bwd(v,t)` runs the same walk backwards from t to 0.
   - Person x's seat at t is fwd(π₀(x),t); the occupant of seat v at t is π₀⁻¹(bwd(v,t)).
   - The tagged person performs a continuous-time random walk on the swap graph. By Aldous' spectral-gap conjecture (proved by Caputo–Liggett–Richthammer 2010), the whole process relaxes as fast as that single walk (recalled), so λ is tuned by one number.

**Interchange prototype** (scratch Rust, unoptimized, allocating): S=2¹⁶ seats, D=3 involutions from a 6-round Feistel, λ=0.05 swaps per edge per year, bucket width 1/(Dλ). At every t tested, fwd was a bijection and bwd∘fwd was the identity.

| t | Moves / person | Fraction displaced | Bucket hashes / query | ns / fwd query |
|---|---|---|---|---|
| 1 y | 0.15 | 0.137 | 3.5 | 162 |
| 10 y | 1.50 | 0.689 | 12.2 | 483 |
| 50 y | 7.47 | 0.945 | 74.7 | 2212 |

Cost is ∝ D·(moves + t/bucket). Use integer ticks, not f64, for determinism.

**Semantics caveat:** moves come in pairs (x takes y's seat and y takes x's). That suits desks, teams, rotas and rooms, but is **wrong for households**, which should change through pair-keyed life events (§3.7), not permutation drift.

| Method | Cost at arbitrary t | Churn pattern | Move locality |
|---|---|---|---|
| Epoch re-key | O(1) | everyone at once | none |
| Staggered re-key | O(1) | 1/C per boundary | none (within class) |
| Conveyor | O(1) | continuous, FIFO | adjacent group |
| Sparse rounds | O(#epochs) | continuous | pairwise, global |
| Interchange + clocks | O(D·moves) | continuous, Poisson | swap-graph edges |

## 6. Verdict on the working hypothesis

**What holds:** "a relationship is a coordinate system; a tie is a shared coordinate; members are the inverse image". This is GGN's random relabelling (Lemma 3.11), the LCA notion of consistency, and the shape of every exact construction above. Reciprocity and enumeration come for free, with no search.

**Where it breaks or needs extending:**

1. **Holes.** A bijection needs a dense domain. Today's `(industry, city, workplace_seed, member_idx)` layout is mostly empty, because `member_idx < workplace_size_for(..)`. Fix: a dense person index p∈[0,P) with attributes derived from p through partitions (§4). An interim option is a rank/select adapter over the current layout: memoize sizes per (industry, city) cell, at O(256) per query (inferred).
2. **Independent keys give independent relations.** Spouses must share a household and a city. Coordinate systems have to form a **DAG over units** (persons → couples → household units → households → neighbourhoods), not sit side by side on persons.
3. **Large groups.** Co-membership-as-tie turns a 5,000-person workplace into a clique. Use nested partitions (teams) plus sparse involution or stub layers inside groups.
4. **Preferences.** Matching finer than block level is not a bijection, and exact stable matching is non-local. Use block coupling plus shared latents (§3.3), or bounded-degree LCAs.
5. **Degree heterogeneity.** One slot per person per coordinate system makes degree = group size − 1. Heavy tails need multiplicity, through stub coordinate systems (§1.6, §3.5).
6. **Time.** A static π has no t. Either keep the structure static and derive event times from pair keys (partners), or use an evolving permutation (§5) whose semantics match the relation.
7. **Fixed global statistics.** A relabelled fixed structure has fixed global statistics, and GGN's guarantees are only against few-query observers. Validation should check the chosen histograms, not expect sampling noise.
8. **Growth.** Permutations for different n are unrelated. Use cycle-walking over a fixed capacity (§2.4).

## 7. Recommended primitives for `procedural_core`

```rust
pub struct Key(pub u64);
impl Key { pub fn derive(seed: u64, labels: &[&str], ids: &[u64]) -> Key; }

pub trait Bijection { fn len(&self) -> u64; fn fwd(&self, x: u64) -> u64; fn inv(&self, y: u64) -> u64; }
pub struct FeistelPerm;   // new(n, Key) = BR Fe[6,a,b] + cycle-walk; with_rounds(n, Key, r >= 4)
pub struct GrowablePerm;  // new(capacity, Key); fwd(x, n), inv(y, n): walk into the live prefix [0, n)
pub struct BlockedPerm<P: Partition>; // per-block keys; block(fwd(x)) == block(x)

pub struct Pairing<B: Bijection>;     // new(perm, paired_prefix)
impl<B: Bijection> Pairing<B> { pub fn partner(&self, x: u64) -> Option<u64>; pub fn pair_id(&self, x: u64) -> Option<u64>; }
pub struct Coupling;                  // new(&[u64] block sizes, &SymPlan, Key) -> Result<Self, PlanError>
impl Coupling { pub fn partner(&self, block: usize, m: u64) -> Option<(usize, u64)>; pub fn couple_id(&self, block: usize, m: u64) -> Option<u64>; }

pub trait Partition { fn len(&self) -> u64; fn num_groups(&self) -> u64;
    fn group_of(&self, i: u64) -> (u64, u64); fn range_of(&self, g: u64) -> std::ops::Range<u64>; }
pub struct FixedSize; pub struct SizeClasses;  // from_histogram(&[(u64, u64)]), from_pmf(n, &[f64]) -> exact cover
pub struct CountTree;   // interval sums: count(lo, hi), rank(i), select(k); Binomial/Poisson splits
pub struct SplitTree;   // fragmentation partition
pub struct StubSpace<P: Partition>; // stubs_of(x) -> Range<u64>, owner(stub) -> u64
pub fn cm_neighbours<B: Bijection, P: Partition>(s: &StubSpace<P>, p: &Pairing<B>, x: u64) -> Vec<u64>;

pub struct StaggeredRekey;  // seat_at(x, t), occupant_at(seat, t)
pub struct InterchangeClock; // fwd(seat, tick), bwd(seat, tick); integer ticks
```

**Property tests (proptest):**
- **Bijections:** exhaustive bijectivity for every n ∈ [1, 2000]. For large n, inv(fwd(x)) = x on samples and fwd(x) < n. Pin golden values per (n, key) so an accidental change to the mixer is caught.
- **Pairing:** σ(σ(x)) = x; σ(x) ≠ x on the paired prefix; pair_id(x) = pair_id(σ(x)).
- **Coupling:** reciprocity; slice counts equal M exactly; singles = n_a − used_a.
- **Partitions:** i ∈ range_of(group_of(i).0); ranges tile [0, n); the histogram is exact; nested partitions stay inside their parent.
- **Relations:** for every y in rel(x), x is in rel(y); no reference points outside [0, P).
- **Statistics:** adjacency chi²/df < 1.5; fixed points ≈ Poisson(1); the number of cycles ≈ ln n (sanity bounds).
- **Time:** bijection at random ticks; bwd∘fwd = id; the Staggered and Interchange variants agree with a brute-force simulation for small S.

**Cost targets** (measured where marked ●, estimated otherwise):

| Primitive | Cost |
|---|---|
| FeistelPerm, forward or inverse | ● 14 ns |
| Pairing | ● 36 ns |
| Coupling | ≈ 2 perms + O(log K) ≈ 50 ns |
| SizeClasses::group_of | ≈ perm + 20 ns |
| CountTree rank/select | ≈ 2–6 µs |
| cm_neighbours | ≈ 5–10 µs per neighbour with CountTree, ≈ 50 ns with SizeClasses degrees |
| InterchangeClock | ● 0.2–2 µs for 0–50 simulated years |

**Integration notes:**
- Keep per-round mixers integer-only; derive keys through the existing `hash`.
- Replace `stable_roommates_match` (per-cohort global pass) with `Coupling` for partners.
- Retire the `*_xor_of` Tier-3 fields: once relations are coordinate systems, those bits are unnecessary.

## Sources

**Read (full text or relevant sections):**
- Goldreich, Goldwasser, Nussboim. *On the implementation of huge random objects.* FOCS 2003; SIAM J. Comput. 39(7), 2010. https://www.wisdom.weizmann.ac.il/~oded/PSX/toro2.pdf
- Biswas, Rubinfeld, Yodpinyanee. *Local access to huge random objects through partial sampling.* arXiv:1711.10692 (v3, 2020). https://arxiv.org/abs/1711.10692
- Black, Rogaway. *Ciphers with arbitrary finite domains.* CT-RSA 2002. https://web.cs.ucdavis.edu/~rogaway/papers/subset.pdf
- Hoang, Morris, Rogaway. *An enciphering scheme based on a card shuffle.* CRYPTO 2012. https://arxiv.org/abs/1208.1176
- Granboulan, Pornin. *Perfect block ciphers with small blocks.* FSE 2007. https://www.bolet.org/~pornin/2007-fse-granboulan+pornin.pdf
- Kensler. *Correlated multi-jittered sampling.* Pixar Technical Memo 13-01, 2013. https://graphics.pixar.com/library/MultiJitteredSampling/paper.pdf (read via web.archive.org)
- Wormald. *Models of random regular graphs* (survey; §4.3, Thm 4.15). https://users.monash.edu.au/~nwormald/papers/regsurvey.pdf
- NIST SP 800-38G Rev. 1, second public draft (Feb 2025). https://csrc.nist.gov/pubs/sp/800/38/g/r1/2pd

**Abstract only:**
- Stefanov, Shi. *FastPRP.* ePrint 2012/254. https://eprint.iacr.org/2012/254
- Even, Levi, Medina, Rosén. *Sublinear random access generators for preferential attachment graphs.* https://arxiv.org/abs/1602.06159 (TALG 2021, doi:10.1145/3464958)
- Rubinfeld, Tamir, Vardi, Xie. *Fast local computation algorithms.* ICS 2011. https://arxiv.org/abs/1104.1377
- Alon, Rubinfeld, Vardi, Xie. *Space-efficient local computation algorithms.* SODA 2012. https://arxiv.org/abs/1109.6178
- Ghaffari. *Local computation of maximal independent set.* FOCS 2022. https://arxiv.org/abs/2210.01104
- Floréen, Kaski, Polishchuk, Suomela. *Almost stable matchings in constant time.* https://arxiv.org/abs/0812.4893 (Algorithmica, doi:10.1007/s00453-009-9353-9)
- Hassidim, Mansour, Vardi. *Local computation mechanism design.* EC 2014. https://arxiv.org/abs/1311.3939
- Greenhill, Janson, Kim, Wormald. *Permutation pseudographs and contiguity.* CPC 11(3), 2002. https://doi.org/10.1017/S0963548301005065
- Jho, Lee. *Partition and mix: generalizing the swap-or-not shuffle.* DCC 2023. https://doi.org/10.1007/s10623-023-01199-4 (search snippet)

**Bibliographic metadata only** (content described is recalled; marked "recalled" in the text where used):
- Naor, Nussboim. *Implementing huge sparse random graphs.* RANDOM 2007. https://doi.org/10.1007/978-3-540-74208-1_43 (content via BRY §1.1)
- Naor, Nussboim, Tromer. TCC 2005. https://doi.org/10.1007/978-3-540-30576-7_5
- Yoshida, Yamamoto, Ito. STOC 2009. https://doi.org/10.1145/1536414.1536447
- Kipnis, Patt-Shamir. *A note on distributed stable matching.* ICDCS 2009. https://doi.org/10.1109/ICDCS.2009.69
- Ostrovsky, Rosenbaum. *Fast distributed almost stable matchings.* PODC 2015. https://doi.org/10.1145/2767386.2767424
- Caputo, Liggett, Richthammer. *Proof of Aldous' spectral gap conjecture.* JAMS 2010. https://doi.org/10.1090/S0894-0347-10-00659-4
- Diaconis, Shahshahani 1981. https://doi.org/10.1007/BF00535487
- Bayer, Diaconis 1992. https://doi.org/10.1214/aoap/1177005705
- Morris, Rogaway, Stegers. CRYPTO 2009. https://doi.org/10.1007/978-3-642-03356-8_17
- Morris, Rogaway. EUROCRYPT 2014. https://doi.org/10.1007/978-3-642-55220-5_18
- Ristenpart, Yilek. CRYPTO 2013. https://doi.org/10.1007/978-3-642-40041-4_22

**Recalled, not verified this session:**
- Nguyen, Onak (FOCS 2008), random-order greedy oracle.
- Luby, Rackoff (1988).
- Norros, Reittu (2006) and van der Hofstad, *Random Graphs and Complex Networks* vol. 1.
- FF1 round count.

**Background:** Log-normal distribution, multiplicative CLT / Gibrat's law. https://en.wikipedia.org/wiki/Log-normal_distribution
