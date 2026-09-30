# Deterministic graph generation without RNG state

**Date:** 2026-05-14
**Status:** Research
**Context:** Foundational research for `procedural_core::graph` (Phase 0 of the [social-graph substrate spec](../specs/2026-05-14-social-graph-substrate.md)).

## Brief

Internot's invariant is `f(id, key, t)`. The social-graph substrate extends this to edges: given `X` and `t`, return `N(X)` in O(|N(X)|), not O(|V|). No RNG stream, no stored adjacency, no shared mutable seed.

The constraint is sharper than "deterministic given a seed." A seeded Mersenne Twister is deterministic; so is a hash-keyed cuckoo table. Both are *sequence-deterministic*: the i-th edge depends on the first i-1. We need *point-deterministic*: edge(A, B) computable from A and B alone, in any order, on any machine. Hash functions are the only universally-available primitive with this property.

## 1. The deterministic-procedural constraint

Procedural content generation in games has hit this wall for thirty years.

**Sequence vs point-determinism.** Classical PCG — Diablo's dungeons, Minecraft biomes, most roguelikes — streams from a seeded PRNG. Reproduces serial observation but breaks random access. Fix: key the PRNG by `(seed, x, y)` per query — Minecraft's "biome seed splitting" and Notch's 2011 terrain post. Each chunk gets a hash-derived PRNG; the world is point-deterministic at chunk granularity.

**Recoverability of derived facts.** No Man's Sky derives 18 quintillion planets from a 64-bit seed, point-deterministic at planet level: planet `(system_id, idx)` derives terrain, flora, fauna from sub-hashes of the pair (Murray 2016, GDC). Dwarf Fortress is the counterexample — global sequential state (rivers carve mountains carve climate carve civs), shipped fifteen years without true random-access reproduction. Caves of Qud (Grinblat & Bucklew 2017) is in between: sequential generation, but every entity has a deterministic id schema for indexed cache lookup.

The lesson: **the moment a fact depends on a sequence of prior facts, you've lost point-determinism.** Two primitives fall out:

- **Hash-based independent sampling.** `edge(A, B) iff hash(A, B) < threshold(A, B)`. The pair is the only input; no global stream. This is Erdős–Rényi G(n, p) and degree-corrected SBM (Karrer & Newman 2011) implemented hash-deterministically.
- **Locality-sensitive hashing (LSH).** Andoni & Indyk (2008) formalized the family: hash so that similar points collide. MinHash (Broder 1997) and SimHash (Charikar 2002) are the workhorses. For graph construction this becomes "neighbors are bucket co-members" — enumerable without scanning all pairs, provided buckets are derivable from id alone.

The pure-hash discipline is already in `procedural_core::hash::*`. Extending it to graph queries is the work below.

## 2. Per-node neighbor enumeration without scan

Four families address `N(X)` in O(|N(X)|):

**2a. Hash + threshold (pairwise rejection).** Symmetric in `(X, Y)` iff `hash(min(X,Y), max(X,Y))` and the threshold are both symmetric. Reciprocity is automatic. Cost is fatal alone: enumerating peers is O(|V|). At population scale (4B persons, ~50 ties each), 4B hashes per query is unusable. Works only when paired with a candidate-restriction mechanism (bit-pattern envelope, LSH bucket, venue cohort) that brings the candidate set to ~|N(X)| · constant. This is exactly how `Space::find` pushdown works: narrow first, hash-accept second.

**2b. Latent-space embeddings.** Every node has an implicit `pos(X)` derived from id. Neighbors are within a metric distance. Random geometric graphs (Penrose 2003) realize this in Euclidean space — realistic clustering, but degree distribution is concentrated, wrong for social. The fix — **hyperbolic random graphs** — gets §3. Enumeration trick: partition the latent space into cells small enough that each holds O(1) nodes on average. `pos(X)`'s cell is a deterministic function of id, so cell enumeration is a pushdown bit-pattern query. **This maps cleanly onto Internot's existing `Space::find`.**

**2c. Group / cohort assignment.** `X` belongs to groups `G_1..G_k` derivable from id; `N(X) = ⋃ G_i \ {X}`. Enumeration cost is the sum of group sizes; reciprocity is automatic (group membership is symmetric). This is what Internot does for workplaces today: `workplace_seed: 8` lives in the person slot; `workplace_members_of` is a `Space::find` pushdown. Limitations: intrinsically bimodal (all members see each other, non-members see none), and group count per person is bit-budget-bounded. Standard fix is layering: dense within-cohort edges + sparse cross-cohort edges from a different mechanism (see §3 and §4).

**2d. Hierarchical / fractal decomposition.** Recursive subdivision mirrors recursive cluster structure (Watts 1999). Kleinberg's 2000 navigability theorem: in hierarchical models with the right link-distance exponent, greedy routing finds shortest paths in O(log² n) hops — the small-world phenomenon. For Internot the relevance is indirect: each level of the tree is a bit prefix, giving the bit-pattern structure for free. Composes well with `BitLayout`.

## 3. Latent-position / geometric / hyperbolic random graphs

### Hyperbolic random graphs (Krioukov et al.)

Krioukov, Papadopoulos, Kitsak, Vahdat & Boguñá (2010, Phys. Rev. E 82) introduced the model that simultaneously produces:

- **Power-law degree** P(k) ~ k^(-γ), γ tunable via radial density
- **Strong clustering** ~0.5+ (real social: 0.1–0.7; ER random: 1/n)
- **Small-world** average path ~log log n
- **Self-similarity** under coarse-graining

Construction:

1. Each node has a position in the Poincaré disk of radius R: angular `θ ∈ [0, 2π)`, radial `r ∈ [0, R]`. Radial density is `f(r) ∝ sinh(αr)` for α > 1/2 — nodes exponentially concentrated near the boundary.
2. Edge probability is Fermi-Dirac on hyperbolic distance: `p(u, v) = 1 / (1 + exp((d(u,v) - R) / 2T))`. T controls clustering; T→0 is sharp threshold, T→1 is soft random geometric.
3. Hyperbolic distance: `d(u,v) = arccosh(cosh r_u cosh r_v - sinh r_u sinh r_v cos Δθ)`. For r near R and small Δθ: `d ≈ r_u + r_v + 2 ln(Δθ/2)`.

Why hyperbolic? In Euclidean d-space, ball volume grows polynomially. In hyperbolic space, exponentially. That's the volume growth needed to fit a power-law degree distribution at constant density. Low-r (central) nodes have exponentially more neighbors than high-r (peripheral) nodes — the hubs.

**Relation to PSO.** Papadopoulos et al. (2012, Nature 489) showed hyperbolic random graphs are isomorphic to a Popularity-Similarity-Optimization growth model: each new node trades preferential attachment against angular proximity. Radial coordinate `r` = age (older = lower r = more popular); angular `θ` = similarity. PSO is the dynamic view; hyperbolic is the static view of the same model. Implication: a node id can carry an angular position (similarity) and a radial position (popularity), both interpretable.

**Pure-hash construction.** All inputs are continuous over bounded ranges. Replace RNG draws with hashes:

```text
θ(id) = hash_float(id, "lat_theta") · 2π
u     = hash_float(id, "lat_r")                # uniform [0,1)
r(id) = (1/α) · arccosh(1 + u · (cosh(αR) - 1))   # radial CDF inverse, closed form
edge(a, b) iff hash_float((a,b), "edge") < p(d(a,b), R, T)
```

Closed-form CDF inversion is ~30 ns. Edge probability is one exp + one hash per pair query.

Enumeration uses the Bringmann–Keusch–Lengler (2019) polar-grid decomposition: partition `[0, R) × [0, 2π)` into cells of bounded diameter. `N(X)` lies in O(constant) cells around `pos(X)`. Cells are deterministic from coordinates — they translate cleanly into a bit-pattern envelope on `(theta_idx, r_idx)`. O(n) total construction, O(deg(X)) per-node enumeration.

### Random geometric graphs

Penrose (2003) is the foundational text; Walters (2011) surveys variants. Euclidean version: uniform points in `[0,1]^d`, connect within distance r. Clustering present, but Poisson degree distribution — not heavy-tailed. Useful for *spatial-proximity* edges (geographic friends) when combined with a heavy-tailed mechanism for global edges.

### Latent-space models (Hoff–Raftery–Handcock)

Hoff, Raftery & Handcock (2002, JASA 97) introduced latent-space models for *inference*: actor `i` has unobserved `z_i` ∈ R^d, `P(edge) = logistic(α - ||z_i - z_j||)`. Construction is the same as a soft random geometric graph; their contribution is the inferential apparatus (irrelevant) and calibration to real-network properties (very relevant: tells us what α / d / radius give realistic graphs).

## 4. Stable matching as deterministic graph construction

Gale-Shapley (1962) produces a stable matching between two equal-size sets given total preference orderings. Properties:

- **Deterministic given preferences.** No RNG.
- **Reciprocity by construction.** `match(a) = b ⟺ match(b) = a`. Exactly the property we need for partner edges.
- **Linear in pair count** with the proposing-side variant; O(n²) worst case but in practice O(n) for well-distributed preferences.

Two issues at Internot scale:

**Cohort bounding.** Matching on 4B persons is infeasible. Fix is the spec's: pair within `(birth_year_cohort, region)` buckets. A ~10K-person cohort matches in ~10⁸ comparisons — milliseconds. Cache on bucket key.

**Preference function determinism.** Preferences derived from hash-similarity. `pref(a,b) = w_age · age_compat + w_geo · geo_compat + w_pers · personality_compat + ...`. Each summand is a pure function of two ids.

**Variants:**

- **Roth-Vande-Vate.** Iteratively repairs an unstable matching via blocking pairs. Same fixed point as Gale-Shapley, parallelizes better.
- **Stable roommates (Irving 1985).** Same problem on one undirected set — no bipartite split. Solution may not exist; when it does it's unique up to ties. For gender-agnostic pairing, technically more correct than Gale-Shapley (which assumes two sides). O(n²); fine for 10K cohorts.

Synthetic-population frameworks (§6) generally use greedy nearest-neighbor pairing because they sit downstream of IPF, which pins the joint distribution. They get reciprocity from cell structure, not the matching algorithm. Internot has no microdata seed to calibrate IPF, so stable matching is the cleaner option.

## 5. Configuration model with deterministic ordering

Given a target degree sequence `(d_1, ..., d_n)`, the configuration model (Bender & Canfield 1978; Bollobás 1980; Newman, Strogatz & Watts 2001) creates `d_i` half-edges per node and pairs uniformly at random. Deterministicization:

1. Canonical order on half-edges: `half_edge(i, j)` for `i ∈ V, j ∈ [0, d_i)`, total order on `(i, j)`.
2. Apply a deterministic permutation derived from world seed. Feistel cipher (Black & Rogaway 2002) gives a bijection on `[0, N)` from a key alone, no array storage.
3. Pair adjacent half-edges in the permuted order.

Cost: O(Σd_i) construction. Per-node enumeration is O(d_i · log N) — finding the j-th neighbor of i requires inverting the Feistel. Fine for typical d_i (median ~5 in households, Dunbar ~150) but expensive at the tail.

The deeper problem: no spatial / similarity structure. Two nodes with similar attributes are no more likely to be connected than two random ones. Wrong for social graphs (assortative mixing is universal). Useful as a *baseline* — a null model preserving degree distribution — but bad as a primary substrate.

## 6. Synthetic-population generation in research

The transport-planning literature has been doing this longest. **TRANSIMS** (Smith et al. 1995–2008, LANL) combinatorially fits Census marginals (age × sex × race × household-size) via Iterative Proportional Fitting (IPF, Deming & Stephan 1940); draws households from PUMS microdata weighted by fit. Family structure comes from sampled households, not constructed. **MATSim** (Balmer, Axhausen 2003–) layers traffic simulation; population synthesis is delegated to PopGen or custom IPF. **PopGen** (Beckman, Baggerly & McKay 1996) is the IPF reference: two-stage, household-marginals first, then individuals conditional on household type. **IPU** (Ye et al. 2009) fixes IPF's two-level inconsistency by jointly optimizing both; Auld & Mohammadian (2010) extended it for Chicago.

**Asymmetric relevance.** These produce 100M-person tables and store them. None deliver `f(id) → person`; they cannot be ported. What we can borrow:

- **Marginal conditioning structure.** Household-level + person-level split is sound — exactly the bit-layout decomposition the spec proposes.
- **PUMS calibration discipline.** ACS-PUMS publishes microdata. Internot's marriage / divorce / fertility already come from US 2020 demographics; household formation should calibrate to ACS marginals (age-of-first-marriage by region, household-size distribution, fertility by maternal age).
- **Hash-bucketing onto a microdata sample** exists in the literature — we won't use it, the static-file dependency is wrong for Internot.

## 7. Reproducible-by-seed graph generators in mainstream libraries

NetworkX `random_graphs` is the de facto reference. State handling across generators:

- `erdos_renyi_graph(n, p, seed)`: seeded RandomState, iterate pairs, Bernoulli each. State is PRNG order.
- `barabasi_albert_graph(n, m, seed)`: sequential preferential attachment; state is PRNG + running degree array.
- `watts_strogatz_graph`: ring lattice + per-edge rewire. State is PRNG.
- `stochastic_block_model`: per-block-pair Bernoulli. State is PRNG.
- `random_geometric_graph`: uniform points + kdtree range query. State is point positions.

Every "reproducible-by-seed" generator in NetworkX is sequence-deterministic — reproduce by re-run, not by random access to "neighbor of node 12345." igraph (Csárdi & Nepusz 2006), graph-tool (Peixoto 2014), SNAP (Leskovec & Sosič 2016) share the same architecture: PRNG + sequential construction + materialized adjacency. None needed point-determinism — their consumers materialize.

**Could outputs be reproduced by pure `f(node_id)`?** Yes for *static* models (positions and edges conditionally i.i.d. given positions): ER, SBM, random geometric, hyperbolic. No for *growth* models (Barabási-Albert) — preferential attachment is intrinsically sequence-dependent. The PSO ↔ hyperbolic isomorphism salvages BA-like degree distributions in a static formulation; pure BA does not survive.

## Recommended approach for Internot

A layered composition over existing `procedural_core` primitives. Six recommendations.

### R1. Venue cohorts as the primary structural primitive

Generalize the existing `workplace_members_of` pattern to `VenueSpace<W>` (already in the spec). Venue-id is an indexable bit field in a registered `Space<W>`; membership enumeration is `Space::find().where_eq()` pushdown — O(|members|), zero scan, zero state. **Bit budget: ~32 bits per venue-id** is plenty for households / future workplace / school / neighborhood scopes.

### R2. Pair-existence by hash-threshold within a venue

Within a venue cohort, every member is tied to every other; tie strength varies per role. **No pair-existence hashing needed inside a household for v1** — cohabitation implies tie. For sparser intra-venue structure in v2 (friendship subsets within a workplace): pure-hash construction, `edge(a, b) iff hash_int(canonical(a, b), "venue_edge", 1<<32) < threshold * (1<<32)`. **Zero bits per node**, ~30 ns per query.

### R3. Cross-venue weak ties via shared latent position

For v2's "friends not in your household" — sparse cross-venue ties at population scale — use **hyperbolic random graphs in their static (pure-hash) formulation**. Each person carries:

- **Angular** `θ ∈ [0, 2π)`: derive from `hash_float(person_id, "lat_theta") · 2π`. Free — hash-derived.
- **Radial** `r ∈ [0, R]`: derive from `hash_float(person_id, "lat_r")`, invert the radial CDF. Free.

Polar-grid decomposition (Bringmann et al.) partitions `[0, R) × [0, 2π)` into cells. **Register `theta_bin: 12, r_bin: 6` as indexable fields in a v2 `latent_position` Space** — 18 bits per entity, separate from the person slot. Pair existence: `hash_float((a,b), "weak_edge") < fermi_dirac(d_hyp(a,b), R, T)`. Enumeration: pushdown for cells within K of self's cell, ~8–20 cells × ~100 candidates, hash-threshold filter. **~1K pair-hashes per `weak_ties_of(X)`, returning ~50–150 ties** — comfortably O(|N(X)|).

### R4. Stable matching for the partner edge

Run stable-roommates (Irving 1985, gender-agnostic) within `(birth_year_cohort, region)` buckets at the cohort marriage-tick. Preference is a weighted sum of pure-hash similarities. Cache process-wide on bucket key. For 4B population the cohort×region grid is ~10K × 1K = 10M buckets, ~100 people each — ~1ms per match, amortizable at first access. **Reciprocity is structural**: stable matching returns `(a, b)` iff `(b, a)` by construction.

### R5. Tie-strength as a trajectory, ranges via stability radii

Reuse `procedural_core::trajectory::{step, smooth}` for the cohabiting-step + post-divorce-decay curve, as the spec already specifies. `procedural_core::trajectory::stability::min_of` composes stability radii. Range queries ("ties with strength > 0.5 during [T1, T2]") become efficient sweeps on existing `filter_exists_in_range` machinery. **Zero bits per node** — pure derivation from procedural attrs + life-event dates.

### R6. Reject configuration model, preferential attachment, and IPF-microdata

Configuration model: sequence-dependent in natural form, no spatial structure. Reject. Preferential attachment (BA): sequence-dependent; PSO salvages it but with more machinery than v1 needs. IPF + microdata: depends on a stored sample, wrong shape for `f(id)`. None belong in `procedural_core::graph`. The hyperbolic + venue + stable-matching trio covers heavy-tailed degree, clustering, small-world, and reciprocity with primitives that are pure functions of ids.

### Pseudocode for the recommended primitives

```rust
// procedural_core::graph

/// Pair-existence within a cohort, symmetric in (a, b).
fn pair_edge_within(a: u32, b: u32, key: &str, threshold_x_2_64: u64) -> bool {
    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
    hash_int_pair(lo, hi, key) < threshold_x_2_64
}

/// Hyperbolic latent position. Pure function of id.
fn latent_position(id: u32, alpha: f64, big_r: f64) -> (f64, f64) {
    let theta = hash_float(id as u64, "lat_theta") * std::f64::consts::TAU;
    let u = hash_float(id as u64, "lat_r");
    // r = (1/α) · arccosh(1 + u · (cosh(αR) - 1))
    let r = (1.0 + u * ((alpha * big_r).cosh() - 1.0)).acosh() / alpha;
    (theta, r)
}

fn d_hyp((t1, r1): (f64, f64), (t2, r2): (f64, f64)) -> f64 {
    let dt = (t1 - t2).abs().min(std::f64::consts::TAU - (t1 - t2).abs());
    (r1.cosh() * r2.cosh() - r1.sinh() * r2.sinh() * dt.cos()).acosh()
}

fn weak_tie_prob(d: f64, big_r: f64, t: f64) -> f64 {
    1.0 / (1.0 + ((d - big_r) / (2.0 * t)).exp())
}

/// O(|N(X)|) weak-tie enumeration via cell pushdown + hash threshold.
fn weak_ties_of(world: &World<W>, x: u32, now: DateTime<Utc>, big_r: f64, t: f64) -> Vec<u32> {
    let (theta_x, r_x) = latent_position(x, ALPHA, big_r);
    let (tb, rb) = bin_of((theta_x, r_x));
    let mut out = vec![];
    for (tb_k, rb_k) in cells_within(tb, rb, K) {
        for y in world.space("latent_position").find()
            .where_eq("theta_bin", tb_k)
            .where_eq("r_bin", rb_k)
            .execute()
        {
            if y == x { continue; }
            let py = latent_position(y, ALPHA, big_r);
            let d = d_hyp((theta_x, r_x), py);
            let p = weak_tie_prob(d, big_r, t);
            let h = hash_float_pair(x.min(y), x.max(y), "weak_edge");
            if h < p { out.push(y); }
        }
    }
    out
}

/// Stable matching within a cohort, cached process-wide on cohort key.
fn stable_matching_cohort(
    cohort_key: u64,
    members: &[u32],
    pref: impl Fn(u32, u32) -> f64,
) -> &'static [(u32, u32)] {
    MATCHING_CACHE.get_or_insert_with(cohort_key, || {
        // Irving stable-roommates, O(n²); cohort ~10⁴, ~ms.
        // Always returns canonical small-first pairs.
        irving_stable_roommates(members, pref)
    })
}
```

### Bit-budget summary

| Primitive | Per-node bits | Per-pair cost | Per-X enumeration |
|---|---|---|---|
| Venue cohort (R1) | 32 b venue id (indexable) | 0 (membership structural) | O(|members|) via `Space::find` |
| Pair edge in cohort (R2) | 0 | 1 hash | O(|members|) |
| Latent position (R3) | 18 b (theta:12, r:6 in v2 space) | 1 hash + 1 sigmoid | O(|N(X)|) via cell pushdown |
| Stable-matched partner (R4) | 0 (in cached matching) | 0 once cached | O(1) lookup |
| Tie-strength trajectory (R5) | 0 (derived) | 0 | n/a |

V1 cost: 32 bits per household-membership entity (already in the spec); zero added to the person slot. V2 cost: +18 bits per latent-position entity in a separate space; person slot still untouched.

### Mapping to existing `procedural_core`

- `hash_int`, `hash_float` → all pair edges
- `BitLayout` with indexable fields → cell decomposition (R3), venue cohort (R1)
- `Space::find().where_eq().execute()` → cell + cohort enumeration
- `trajectory::{step, smooth}` + `stability::min_of` → tie-strength curves (R5)
- New: `stable_matching_cohort` helper (R4) — the one piece that doesn't exist yet, modest

The substrate is a thin layer over machinery that's already there. The novel observation is that hyperbolic random graphs, reformulated to depend only on hash-derived positions, become a pure point-deterministic primitive — and that this composes cleanly with `Space::find` pushdown to deliver O(|N(X)|) neighbor enumeration without state.

## Sources

- Andoni & Indyk (2008). Near-optimal hashing algorithms for approximate nearest neighbor. *Comm. ACM* 51(1).
- Auld & Mohammadian (2010). Efficient methodology for generating synthetic populations with multiple control levels. *Transp. Res. Rec.* 2175.
- Beckman, Baggerly & McKay (1996). Creating synthetic baseline populations. *Transp. Res. A* 30(6).
- Bender & Canfield (1978). Asymptotic number of labeled graphs with given degree sequences. *J. Combin. Theory A* 24(3).
- Black & Rogaway (2002). Ciphers with arbitrary finite domains. *CT-RSA 2002* (LNCS 2271).
- Bollobás (1980). A probabilistic proof of an asymptotic formula for the number of labelled regular graphs. *Eur. J. Combin.* 1(4).
- Bringmann, Keusch & Lengler (2019). Geometric inhomogeneous random graphs. *Theor. Comput. Sci.* 760.
- Broder (1997). On the resemblance and containment of documents. *Compression and Complexity of Sequences*.
- Charikar (2002). Similarity estimation techniques from rounding algorithms. *STOC '02*.
- Csárdi & Nepusz (2006). The igraph software package. *InterJournal Complex Systems* 1695.
- Deming & Stephan (1940). On a least-squares adjustment of a sampled frequency table. *Ann. Math. Stat.* 11.
- Gale & Shapley (1962). College admissions and the stability of marriage. *Amer. Math. Monthly* 69(1).
- Grinblat & Bucklew (2017). Procedural generation in Caves of Qud. *Roguelike Celebration*.
- Hoff, Raftery & Handcock (2002). Latent space approaches to social network analysis. *JASA* 97(460).
- Irving (1985). An efficient algorithm for the "stable roommates" problem. *J. Algorithms* 6(4).
- Karrer & Newman (2011). Stochastic blockmodels and community structure in networks. *Phys. Rev. E* 83.
- Kleinberg (2000). The small-world phenomenon: an algorithmic perspective. *STOC '00*.
- Krioukov, Papadopoulos, Kitsak, Vahdat & Boguñá (2010). Hyperbolic geometry of complex networks. *Phys. Rev. E* 82(3).
- Leskovec & Sosič (2016). SNAP. *ACM TIST* 8(1).
- Murray (2016). Building worlds using maths(s). *GDC*.
- Newman, Strogatz & Watts (2001). Random graphs with arbitrary degree distributions. *Phys. Rev. E* 64.
- Notch / Persson (2011). Terrain generation, part 1. *The Word of Notch*.
- Papadopoulos, Kitsak, Serrano, Boguñá & Krioukov (2012). Popularity versus similarity in growing networks. *Nature* 489.
- Peixoto (2014). The graph-tool python library. *figshare*.
- Penrose (2003). *Random Geometric Graphs*. Oxford University Press.
- Smith, Beckman & Baggerly (1995). TRANSIMS. *LANL-95-1641*.
- Walters (2011). Random geometric graphs. *Surveys in Combinatorics 2011*.
- Watts (1999). Networks, dynamics, and the small-world phenomenon. *AJS* 105(2).
- Ye, Konduri, Pendyala, Sana & Waddell (2009). Methodology to match distributions of household and person attributes in synthetic populations. *TRB*.
