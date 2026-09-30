# Graph Theory Foundations for Procedural Social-Graph Synthesis

**Date:** 2026-05-14
**Scope:** Empirical properties of real social graphs; classical generative models; state-of-the-art for realistic synthesis; deterministic / pure-function construction implications for Internot.

---

## 1. Properties of Real Social Graphs to Match

Before picking a generator, we need to know the target. A "realistic" social graph hits a constellation of statistical fingerprints simultaneously, and many famous models hit some while failing others. The fingerprints below are the standard battery (see Newman 2003, Boguñá & Pastor-Satorras, Leskovec 2010).

### 1.1 Degree Distribution

The default story — "social networks are power-law" — is contested. **Broido & Clauset (2019)** ran rigorous statistical tests across ~1000 networks and found that *truly* scale-free networks (p(k) ~ k^-α with α in [2,3]) are rare; only ~4–10% pass the strongest tests, and many "scale-free" claims are artifacts of insufficient model comparison ([Broido & Clauset 2019, Nature Communications](https://www.nature.com/articles/s41467-019-08746-5)). The follow-up [Voitalov et al. 2019](https://royalsocietypublishing.org/doi/10.1098/rspa.2019.0742) pushed back, showing that with regularly-varying tail definitions ~65% of networks are "power-law with at least 80% power."

**Empirical shapes seen in social graphs specifically:**

- **Power-law with exponential cutoff.** The tail decays as a power law but bends down at very high k because of practical limits (Dunbar number, time budgets). Mobile-phone call graphs (Onnela et al. 2007, [PNAS](https://www.pnas.org/doi/10.1073/pnas.0610245104), 4M-node Hungarian carrier dataset) and the [Facebook 100 dataset](https://arxiv.org/abs/1102.2166) (Traud, Mucha & Porter 2012) both show this.
- **Log-normal.** Often a *better* fit than pure power law for friendship / email networks (Lee 2024, [Statistica Neerlandica](https://onlinelibrary.wiley.com/doi/full/10.1111/stan.12355)). The right-skew is real; the strict heavy tail is not always there.
- **Bimodal / mixture.** Twitter (Kwak et al. 2010, [WWW '10](https://www.cs.cornell.edu/courses/cs6241/2019sp/readings/Kwak-2010-Twitter.pdf)) famously has a *non*-power-law follower distribution because of celebrity asymmetry — top accounts have orders of magnitude more followers than followings; the in-degree and out-degree distributions have different shapes.
- **In/out asymmetry on directed graphs.** LiveJournal ([SNAP soc-LiveJournal1](https://snap.stanford.edu/data/soc-LiveJournal1.html)): in-degrees follow a heavier tail than out-degrees because most users follow a few prolific posters.

**Typical exponents (where power law is a defensible fit):** α ∈ [1.8, 2.5] for the in-degree of dense social graphs; α ∈ [2.1, 3.0] for sparse acquaintance networks.

### 1.2 Clustering Coefficient

Social graphs are *much* more clustered than random graphs of equivalent density — this is the "social" signature.

| Network | Nodes | Avg clustering C | Source |
|---|---|---|---|
| Facebook (all friends, median user with k=100) | ~720M | ~0.14 local | [Ugander et al. 2011, arXiv:1111.4503](https://arxiv.org/pdf/1111.4503) |
| Facebook 100 (university subnets) | ~1k–40k | 0.20–0.35 | Traud et al. 2012 |
| LiveJournal | 4.85M | **0.3123** | [SNAP](https://snap.stanford.edu/data/soc-LiveJournal1.html) |
| Pokec | 1.63M | **0.1094** | [SNAP soc-Pokec](https://snap.stanford.edu/data/soc-Pokec.html) |
| Email (Enron, internal) | 36k | ~0.50 | SNAP |
| Mobile-phone calls (Hungary, Onnela) | 3.9M | ~0.10 | Onnela et al. 2007 |
| Co-authorship (DBLP) | 317k | ~0.63 | SNAP |
| Twitter follower | 41.7M | ~0.08 (asymmetric, low) | Kwak et al. 2010 |

**Rule of thumb:** real social graphs have C in the range **0.1–0.7**, several orders of magnitude above the C ~ ⟨k⟩/n you would expect from a random graph at the same density. Tight-knit communities (academic co-authorship, university friendship) sit at the high end; mass-broadcast platforms (Twitter, Pokec) sit at the low end.

### 1.3 Average Path Length and Diameter

Six-degrees / small-world: distances are O(log n) or smaller.

| Network | Avg path length | Effective diameter (90th pct) | Diameter |
|---|---|---|---|
| Facebook (full) | ~4.7 | ~5 | small | (Ugander 2011) |
| LiveJournal | ~6.5 | 6.5 | 18 | SNAP |
| Twitter follower | ~4.1 | 4.8 | small | Kwak 2010 |
| Pokec | ~5.0 | ~5.7 | ~11 | SNAP |

The "ultra-small world" property (distance ~ log log n) appears in scale-free networks with α < 3.

### 1.4 Assortativity

Newman's degree-degree correlation coefficient r ([Newman 2003 PRE](https://link.aps.org/doi/10.1103/PhysRevE.67.026126)). r > 0 means high-degree nodes connect to other high-degree nodes (the "rich club"); r < 0 means hubs connect to low-degree peripheries.

**Empirical pattern:**
- **Most social networks: r ∈ [0.1, 0.3] (assortative).** Friendship, co-authorship, film actor collaboration.
- **Technological / biological networks: r ∈ [-0.3, -0.1] (disassortative).** Internet AS graph, protein interaction.
- **Online social networks transition from assortative → disassortative as they grow dense** ([Hu & Wang 2014, Scientific Reports](https://www.nature.com/articles/srep04861)). Early Facebook was strongly assortative; mature Twitter is mildly disassortative.

Beyond scalar assortativity, **homophily on attributes** matters enormously: gender, age, class year, geography. Facebook 100 (Traud et al. 2012) found *class year* dominates Facebook university subnets — common high school strong at large institutions, common major variable. Any procedural social graph that ignores attribute-homophily will feel sterile.

### 1.5 Community Structure / Modularity

Real social graphs have *strong*, *overlapping*, *hierarchical* community structure. Modularity Q (Newman-Girvan) typically 0.4–0.8 for social graphs. LiveJournal communities follow a power-law size distribution (median ~20, max ~10k); the same is true of Facebook 100 and Pokec.

Communities are *nested* (departments inside universities inside cities) and *overlapping* (a person belongs to family + workplace + neighborhood). The [LFR benchmark](https://arxiv.org/abs/0805.4770) is the standard synthetic to test against (see §2.10).

---

## 2. Classical Generative Models

For each: mechanism, parameters, what it produces, what it misses.

### 2.1 Erdős–Rényi G(n, p)

**Mechanism:** every pair of n nodes is independently connected with probability p.
**Parameters:** n, p (or equivalently n, m).
**Produces:** Poisson degree distribution P(k) ~ Poisson(np). Average path length log n / log(np). Phase transition at p = 1/n (giant component emerges).
**Misses:** No clustering (C ~ p, vanishes for sparse graphs). No heavy tail. No community structure. No homophily.

Only useful as a null model. Not a serious candidate for synthetic social graphs.

### 2.2 Watts–Strogatz (Small World)

**Mechanism:** Start with a ring lattice where each node connects to k nearest neighbors; rewire each edge with probability β to a random target.
**Parameters:** n, k, β ∈ [0, 1].
**Produces:** High clustering (lattice-inherited) AND short paths (rewiring-induced) — the original "small world." Phase transition in β.
**Misses:** [Watts–Strogatz Wikipedia](https://en.wikipedia.org/wiki/Watts%E2%80%93Strogatz_model): degree distribution is approximately Poisson — *not* heavy-tailed. Fixed n (cannot model growth). The clustering comes from spatial regularity that has no semantic meaning in a social context (why are people 1.0 and 2.0 in the ring "near"?).

Useful pedagogically. Not a realistic generator on its own.

### 2.3 Barabási–Albert (Preferential Attachment)

**Mechanism:** Grow the graph by adding one node at a time; each new node attaches m edges to existing nodes with probability proportional to their current degree.
**Parameters:** n, m.
**Produces:** Power-law degree distribution P(k) ~ k^-3 (exponent is *exactly* 3, not tunable in the vanilla model). Small-world distances.
**Misses:** [BA Wikipedia](https://en.wikipedia.org/wiki/Barab%C3%A1si%E2%80%93Albert_model): clustering coefficient → 0 as n → ∞ (asymptotically zero, opposite of real social networks). Fixed exponent. Disassortative (real social is assortative). No community structure. No homophily. Sequential / stateful generation (each step depends on all prior degrees).

Useful for explaining hubs. Not realistic for social graphs.

### 2.4 Stochastic Block Model (SBM)

**Mechanism:** Partition n nodes into K blocks; edge between i and j with probability ω(b_i, b_j) depending only on block memberships.
**Parameters:** block sizes, K×K probability matrix ω.
**Produces:** Tunable community structure. Homophily via diagonal-heavy ω. Erdős–Rényi within blocks.
**Misses:** Poisson degree within each block (no heavy tail). No hierarchical structure. No spatial / geographic effects.

The foundation of modern community-detection literature ([Holland, Laskey & Leinhardt 1983](https://www.sciencedirect.com/science/article/abs/pii/0378873383900217)).

### 2.5 Degree-Corrected SBM (DC-SBM)

**Mechanism:** Karrer & Newman (2011). Same as SBM but each node carries an additional degree-propensity parameter θ_i; edge probability is θ_i θ_j ω(b_i, b_j).
**Parameters:** SBM parameters + per-node θ vector.
**Produces:** Tunable degree heterogeneity *within* each block. Communities + heavy tail.
**Misses:** Still no spatial structure. Still no clustering above what blocks provide.

Currently the most popular SBM variant for empirical work ([review, Lee & Wilkinson 2019](https://link.springer.com/article/10.1007/s41109-019-0232-2)).

### 2.6 Hierarchical SBM / Nested SBM (Peixoto)

**Mechanism:** [Peixoto 2014](https://arxiv.org/abs/1310.4377). Recursively apply SBM: the inferred block multigraph is itself generated by an SBM, producing a tree of nested blocks. graph-tool's `NestedBlockState` is the canonical implementation.
**Parameters:** depth (chosen by MDL), per-level block partitions.
**Produces:** Communities at every scale; explains right-skewed degrees, high clustering, short paths *simultaneously*. Max inferable blocks scales as B_max = O(N/log N) (vs. O(√N) for flat SBM).
**Misses:** Inference-heavy; generative form requires picking the hierarchy. Still no geographic / spatial.

Considered state-of-the-art for *inferring* community structure on real social graphs.

### 2.7 Geographic / Spatial Random Graphs

**Mechanism:** Place nodes in metric space; connect pairs with probability depending on distance (threshold or sigmoid).
**Parameters:** ambient space, density, connection radius / kernel.
**Produces:** High clustering (geometry → triangles), local structure.
**Misses:** Homogeneous degree distribution (no hubs). Diameter scales as n^(1/d) for dimension d, much larger than O(log n).

### 2.8 Hyperbolic Random Graphs (Krioukov et al.)

**Mechanism:** [Krioukov, Papadopoulos, Kitsak, Vahdat & Boguñá 2010, Phys. Rev. E 82, 036106](https://arxiv.org/abs/1006.5169). Sample n points uniformly inside a disk of radius R in the hyperbolic plane H²; connect pairs within hyperbolic distance R (or with sigmoidal probability at "temperature" T).
**Parameters:** n, R (controls density), α (controls radial density → degree exponent), T (controls clustering).
**Produces:** *All* of: power-law degrees with tunable exponent γ = 2α + 1, high clustering (geometry-driven), small-world diameter, community structure (angular sectors), assortativity tunable via T.
**Misses:** Single latent dimension (angular). Doesn't naturally handle multiplex relationships. The mapping from "person attributes" to hyperbolic coordinates is non-obvious for procedural generation — what does radius "mean"?

**Why this matters:** the first analytically-tractable model that hits the empirical fingerprints *jointly*. The community-detection followup ([Faqeeh et al. 2018](https://arxiv.org/abs/1812.03002), [Muscoloni & Cannistraci 2021, Sci Rep](https://www.nature.com/articles/s41598-021-93921-2)) confirmed that hyperbolic models reproduce empirical community size distributions on Facebook subgraphs.

### 2.9 Configuration Model & Chung–Lu (Expected Degree)

**Configuration model:** given a degree sequence (d_1, ..., d_n), create d_i "stubs" per node and pair stubs uniformly at random. Produces *exact* degree sequence (modulo self-loops / multi-edges).

**Chung–Lu (2002):** given expected degrees (w_1, ..., w_n), connect i and j with probability w_i w_j / Σ w_k.
**Parameters:** target degree sequence / weight sequence.
**Produces:** Arbitrary heavy-tailed degree distribution. Otherwise maximally random (no clustering, no communities, mostly assortativity-neutral).
**Misses:** No clustering, no homophily, no geometry. Standard null model. Generation is O(n + m) via [efficient algorithms](https://arxiv.org/abs/1910.11341); standard naive is O(n²).

Use as a building block (e.g. GIRG uses Chung–Lu weights + geometry).

### 2.10 LFR Benchmark

**Mechanism:** [Lancichinetti, Fortunato & Radicchi 2008](https://arxiv.org/abs/0805.4770). Sample degrees from a power law with exponent τ₁; sample community sizes from a power law with exponent τ₂; assign each node a community such that the node's intra-community degree is (1−μ)·k_i and inter-community is μ·k_i.
**Parameters:** n, ⟨k⟩, k_max, τ₁ (degree exponent), τ₂ (community-size exponent), μ (mixing parameter).
**Produces:** Heterogeneous degrees + heterogeneous community sizes + tunable boundary sharpness.
**Misses:** Clustering is whatever falls out of the configuration-style wiring; not directly tuned. No assortativity control. No hierarchy by default.

Standard benchmark for community-detection algorithm evaluation.

### 2.11 Exponential Random Graph Models (ERGM)

**Mechanism:** define P(G) ∝ exp(Σ_k θ_k s_k(G)), where s_k are graph statistics (edges, triangles, k-stars, attribute-homophily, etc.).
**Parameters:** vector of θ coefficients, fit by MCMC-MLE.
**Produces:** Any combination of statistics, in principle.
**Misses:** **Fundamentally does not scale.** [PLOS ONE 2020](https://journals.plos.org/plosone/article?id=10.1371/journal.pone.0227804): MLE intractable beyond ~few thousand nodes. Strongly degenerate when triangle-like terms are large (collapses to empty or complete graph). Sampling requires MCMC; deterministic construction is out of reach.

ERGMs are an *inference / hypothesis-testing* tool, not a generator for synthetic-data substrates.

### 2.12 Kronecker Graphs

**Mechanism:** [Leskovec, Chakrabarti, Kleinberg, Faloutsos & Ghahramani 2010, JMLR](https://jmlr.csail.mit.edu/papers/v11/leskovec10a.html). Start with a small "initiator" matrix K (e.g. 2×2 with entries 0–1); take Kronecker powers K^⊗k to build adjacency at scale 2^k.
**Parameters:** initiator matrix (~4 floats), depth k.
**Produces:** Multinomial / log-normal degree distribution (looks heavy-tailed), small diameter, densification over time, eigenvalue heavy tails. Mathematically tractable; KronFit estimates the initiator from real data.
**Misses:** Clustering coefficient is too low compared to real social graphs. Recursive structure visible in spectrum (telltale "fractal" eigenvalue spacing). Power of two sizes only (modulo padding).

Used at scale (Graph500 benchmark = R-MAT, a Kronecker variant).

---

## 3. State-of-the-Art for Realistic Social-Graph Synthesis

What does the 2020s research community actually use to *generate* graphs that pass empirical tests?

1. **Geometric Inhomogeneous Random Graphs (GIRGs)** — [Bringmann, Keusch & Lengler 2019](https://www.sciencedirect.com/science/article/pii/S0304397518305309). Combine Chung–Lu weights (→ tunable power-law degrees) with random positions in a geometric space (→ clustering). Generalize hyperbolic random graphs as the unit-disk case and remove the analytical awkwardness. **Generated in O(n + m) expected time** ([Bläsius et al. 2022](https://www.cambridge.org/core/journals/network-science/article/efficiently-generating-geometric-inhomogeneous-and-hyperbolic-random-graphs/EE2080A5FEC3A6C2B3AB451934A340AC)). Hit *all* the standard fingerprints simultaneously: power-law degrees, large clustering, log-diameter, ultra-small avg distance, small separators. **Currently the leading choice for synthetic-social benchmarks.**

2. **Hyperbolic random graphs proper** — still very actively used, especially in network-embedding literature where the hyperbolic latent space is also useful for representation learning ([Muscoloni & Cannistraci](https://www.nature.com/articles/s41598-021-93921-2)).

3. **Nested / hierarchical degree-corrected SBM** — [Peixoto's graph-tool](https://graph-tool.skewed.de/static/doc/demos/inference/inference.html). The dominant approach when *community structure* is the priority (e.g. computational sociology). Generative form requires you to specify or infer the hierarchy.

4. **Multiplex / multilayer models** — when relationships are typed (friend vs. family vs. coworker), single-layer generators are inadequate. [Kivelä et al. 2014](https://academic.oup.com/comnet/article/2/3/203/2841130) survey. Each layer typically gets its own SBM / hyperbolic model with correlated latent positions.

5. **Latent position / latent space models** — Hoff, Raftery, Handcock 2002; modern Bayesian variants infer per-node embeddings such that edge probability is a function of embedding distance. Hyperbolic latent space models ([2026 review](https://arxiv.org/html/2605.11340)) are now mainstream.

6. **Graph neural-network generators (GraphRNN, DiGress, etc.)** — learned generators. Excellent local statistics, expensive, non-deterministic, opaque parameters. Not useful as a *substrate* for an agent training environment (you want interpretable knobs and reproducibility).

**Consensus 2026:** for realistic synthetic social graphs at scale, the recipe is some combination of:
- a latent space (hyperbolic or Euclidean) capturing implicit similarity → clustering, communities,
- a degree-propensity per node (Chung–Lu style) → heavy-tailed degrees,
- optional attribute-homophily on top → realistic semantic mixing,
- optional hierarchical / multiplex layering → nested communities and relationship types.

GIRG = (hyperbolic latent space) ∩ (Chung–Lu weights) is the cleanest formulation that hits all the targets, and it's what I'd point Internot at as a baseline.

---

## 4. Practical Implications for Deterministic Procedural Construction

The Internot invariant is **every fact = pure function of (id, key, t)**. No stateful RNG, no persistent state. Let's grade each model on that axis.

### 4.1 Construction Cost vs. Query Cost

The relevant cost for Internot is *not* full-graph construction cost (we never materialize the graph). It's:
- **Per-neighbor query:** given node X, what edges go from X? Must be fast and stateless.
- **Per-edge query:** given (X, Y), is there an edge? Useful for ad-hoc checks.

| Model | Stateless `f(id)` neighbors? | Per-edge check | Per-node neighbor enumeration |
|---|---|---|---|
| Erdős–Rényi G(n, p) | **Yes** (trivially: hash(min(i,j), max(i,j)) < p) | O(1) | O(n) brute, O(np) expected output |
| Watts–Strogatz | Partial — lattice neighbors free, rewired ones need per-edge hash | O(1) | O(k) |
| Barabási–Albert | **No** (preferential attachment is intrinsically sequential — degree at step t depends on prior steps) | — | — |
| SBM | **Yes** — block_of(i) and block_of(j) deterministic; edge = hash(min(i,j), max(i,j)) < ω(b_i, b_j) | O(1) | O(n), but bit-pattern pushdown over block ids can prune (relevant to `procedural_core::Space::find`) |
| DC-SBM | **Yes** — θ_i = f(i); edge = hash(...) < θ_i θ_j ω(b_i, b_j) | O(1) | O(n) (with pushdown over block) |
| Hierarchical SBM | **Yes** — block_path_of(i) = chain of hashes; edge probability product across hierarchy | O(1) | O(n) with smarter pushdown across hierarchy |
| Geographic / spatial | **Yes** — coords = f(i); edge = (dist(i, j) < r) or sigmoid via hash | O(1) | O(n) brute, O(local density) if you spatial-index lazily |
| Hyperbolic RG | **Yes** — (r_i, θ_i) = f(i); edge by hyperbolic distance | O(1) | O(n) brute |
| GIRG | **Yes** — weight_i + pos_i = f(i); edge prob = min(1, w_i w_j / W · g(|x_i − x_j|)) | O(1) | O(n) brute; O(expected deg) with grid index |
| Configuration model | **No** (stub-matching is global) | — | — |
| Chung–Lu | **Yes** — edge prob f(w_i, w_j) only depends on per-node weights | O(1) | O(n) brute |
| LFR | **No** in vanilla form (community assignment is rejection-driven) | — | — |
| ERGM | **No** (sample via MCMC) | — | — |
| Kronecker | **Yes** — edge bit determined by Kronecker hierarchy of initiator probabilities applied to the binary expansion of (i, j) | O(log n) | O(n) brute, O(deg) with hierarchy walk |

**Pattern:** the *latent-variable* models (SBM family, geometric, hyperbolic, GIRG, Chung–Lu, Kronecker) are pure-functional-friendly. The *growth* models (BA), *stub-matching* models (configuration), *rejection* models (LFR), and *sample-based* models (ERGM, GNN generators) are not.

This is excellent news for Internot: **the models that win on empirical realism (GIRG, hierarchical DC-SBM, hyperbolic) are exactly the ones that compose with `f(id, key)` purity.**

### 4.2 Cost per "list neighbors of X"

Naive: O(n) — test every potential edge. For 4.3B people this is fatal.

The escape hatches:

1. **Bit-pattern pushdown via `procedural_core::Space::find`.** If block membership lives in a layout field, queries like "list everyone in same block as X" enumerate over a `BitPattern::exact`. Cost = number of intra-block neighbors, not n. The existing people-space already has `industry:6, city:6, workplace_seed:8, member_idx:12` → block-style affordances for free.

2. **Spatial indexing for geometric models.** Bucket coordinates into a coarse grid via additional layout bits (or derive a bucket id by hashing position to a small key); enumerate neighboring buckets. Cost = O(expected degree). This composes well with the `Space` abstraction: register a `position_bucket` indexable attribute and pushdown on it.

3. **Hierarchy walk for nested SBM.** Each level of the hierarchy carves more candidates away. Particularly nice if hierarchy is encoded as nested bit fields in the layout (e.g. `country:4, region:6, city:6, workplace:8`).

4. **Latent neighbor enumeration for hyperbolic / GIRG.** Sort candidates by angular sector × radial band; only pairs in overlapping bands have non-negligible edge probability. Discretize the latent space into bit-field buckets and pushdown.

**Implication:** for Internot to support `list_neighbors(person_id)` at 4.3B scale, the latent / block / spatial structure *must* be encoded in the layout's indexable bit fields, not in hashed attributes. The framework already supports this exactly (composite indexable attributes + `where_eq` pushdown). The existing people-space industry/city/workplace structure is already a coarse SBM — extending it into a hierarchical-SBM or GIRG-style model is mostly choosing the right additional bit fields.

### 4.3 Determinism: edges as hash thresholds

The general recipe: for an edge model with probability p(i, j) = f(attr_i, attr_j), produce edge presence via

    h = hash_float(min(i, j), "edge", max(i, j))
    edge_exists := (h < p(i, j))

This is bit-stable, reproducible, and per-edge stateless. The same recipe powers every latent-variable model in the table above. Note the symmetrization on `(min, max)` so the test is order-independent.

### 4.4 Stationarity and time-varying edges

Internot wants `f(id, key, t)`. Edges can be time-varying without breaking determinism:

- **Slowly drifting friendships:** add `t // T_period` as a hash key, so the edge "refreshes" once per period.
- **Lifetime-bounded edges:** existence requires `t_start(i, j) < t < t_end(i, j)`, where the endpoints themselves are derived from hashes anchored on life-event timelines (see existing `people` career-arc / life-events).
- **Composability with `step()` trajectory:** the social graph at time t = union of household edges (always on) + workplace edges (on during employment from career arc) + school edges (on during education timeline). Each is a deterministic function of `(id_pair, t)` against an event schedule.

This composes beautifully with the existing `people` realism subsystem (education profile, career arc, life events). The graph isn't a separately-sampled object — it's a *derived* function of the same primitives.

---

## 5. Design Implications for Internot

1. **Adopt a GIRG-style latent-space + Chung–Lu-weight model as the default friendship-edge generator.** This is the lowest-cost path to hitting all empirical fingerprints simultaneously (power-law degrees, high clustering, small diameter, communities, assortativity). The hyperbolic variant is mathematically prettier but the Euclidean GIRG is conceptually easier to derive from existing person attributes (city → 2D coord, industry → embedding dim, age → another dim). Edge probability becomes a deterministic function of (attr-vector, attr-vector, weight, weight), evaluated via the `hash_float` threshold trick. Per-edge stateless. Per-neighbor enumeration via bit-pattern pushdown on the bucketed coordinates.

2. **Encode community / spatial structure in indexable bit fields, not hashed attributes.** The current `people` layout (`industry:6, city:6, workplace_seed:8, member_idx:12`) is already an SBM scaffold. Extend it with a coarse-grained position-bucket field if a continuous latent space is desired (a 6-bit "social-cluster" id derived from a small hash of personality + life events, then declared as an indexable layout field, would give 64 communities each enumerable in O(community size) via `Space::find`). The single load-bearing principle: anything you want to push down through `find()` must live in the layout, last-declared.

3. **Make degree heterogeneity a per-node `weight` derived from existing attributes, not a separate sample.** Big Five extraversion + age + occupation tier are exactly the kind of signal that should set a person's expected degree. `weight_i = f_existing_attrs(person_i)` keeps determinism and gets log-normal-like heterogeneity for free. Tune the weight curve so the resulting degree distribution lies in the empirical α ∈ [2.0, 2.5] band; verify against the Facebook 100 / Pokec / LiveJournal numbers above.

4. **Treat edges as time-windowed, not static.** A workplace edge is a function of overlapping employment intervals from career arcs; a school edge is a function of overlapping education intervals; a household edge is a function of the marriage/cohabitation lifecycle. Family / friendship persists; workplace / school turns over. This is what makes the substrate feel *alive* rather than frozen, and it leverages the realism subsystem already built. The composition rule is: an edge exists at time `t` iff at least one *relationship channel* (household, workplace, school, neighborhood, online) currently has it active, where each channel is a separate `f((i, j), t)` predicate.

5. **Validate against the empirical battery once, then lock it in.** Pick a fixed seed; sample a 100k-person subgraph; measure: degree distribution (KS test against log-normal / power-law-with-cutoff), clustering coefficient (target 0.15–0.35), avg path length (target 4–6), assortativity (target 0.1–0.3), community-size distribution (target power-law-like). Drop the script in `internot/examples/social_graph_validation.rs`. Re-run whenever the generator parameters change. This is the single guard against the cohort-style sterility incident in `learning.md`.

6. **Reject ERGM, configuration-model, BA, and LFR as direct generators — they're not pure-functional.** They can still be useful as *targets* for validation (e.g. "does our generator pass an LFR-style community-detection sanity check?") but never as the construction mechanism. The construction mechanism must be latent-variable. The Stage Manager paradigm already in CLAUDE.md (procedural floor → typed AVMs → constrained renderer) extends naturally: the social graph is just another typed lens on the same procedural floor, computed by `f(person_pair, t)` from primitives. Cross-service coherence (Alice texting Bob in `chat` ⇔ Alice/Bob co-workers in `people`) becomes automatic.

---

## Sources

Empirical real-graph properties:
- [Broido & Clauset 2019 — Scale-free networks are rare (Nat Comms)](https://www.nature.com/articles/s41467-019-08746-5)
- [Voitalov et al. 2019 — How rare are power-law networks really? (Proc Roy Soc A)](https://royalsocietypublishing.org/doi/10.1098/rspa.2019.0742)
- [Lee 2024 — Degree distributions: beyond the power law (Stat Neer)](https://onlinelibrary.wiley.com/doi/full/10.1111/stan.12355)
- [Traud, Mucha & Porter 2012 — Social Structure of Facebook Networks (arXiv:1102.2166)](https://arxiv.org/abs/1102.2166)
- [Ugander et al. 2011 — The Anatomy of the Facebook Social Graph (arXiv:1111.4503)](https://arxiv.org/pdf/1111.4503)
- [Onnela et al. 2007 — Structure and tie strengths in mobile communication networks (PNAS)](https://www.pnas.org/doi/10.1073/pnas.0610245104)
- [Kwak et al. 2010 — What is Twitter? (WWW '10)](https://www.cs.cornell.edu/courses/cs6241/2019sp/readings/Kwak-2010-Twitter.pdf)
- [SNAP soc-LiveJournal1 dataset](https://snap.stanford.edu/data/soc-LiveJournal1.html)
- [SNAP soc-Pokec dataset](https://snap.stanford.edu/data/soc-Pokec.html)
- [Hu & Wang 2014 — From sparse to dense and from assortative to disassortative (Sci Rep)](https://www.nature.com/articles/srep04861)
- [Newman 2003 — Mixing patterns in networks (PRE)](https://link.aps.org/doi/10.1103/PhysRevE.67.026126)

Classical models:
- [Watts–Strogatz model — Wikipedia](https://en.wikipedia.org/wiki/Watts%E2%80%93Strogatz_model)
- [Barabási–Albert model — Wikipedia](https://en.wikipedia.org/wiki/Barab%C3%A1si%E2%80%93Albert_model)
- [Karrer & Newman 2011 — Stochastic blockmodels and community structure (PRE)](https://arxiv.org/abs/1008.3926)
- [Peixoto 2014 — Hierarchical block structures (Phys. Rev. X)](https://arxiv.org/abs/1310.4377)
- [Krioukov et al. 2010 — Hyperbolic Geometry of Complex Networks (PRE)](https://arxiv.org/abs/1006.5169)
- [Chung & Lu 2002 — Connected components in random graphs with given expected degree sequences](https://www.math.cmu.edu/~af1p/Teaching/MCC17/Papers/ChungLuVu.pdf)
- [Lancichinetti, Fortunato & Radicchi 2008 — LFR benchmark (arXiv:0805.4770)](https://arxiv.org/abs/0805.4770)
- [Leskovec et al. 2010 — Kronecker Graphs (JMLR)](https://jmlr.csail.mit.edu/papers/v11/leskovec10a.html)
- [Configuration model — Wikipedia](https://en.wikipedia.org/wiki/Configuration_model)
- [ERGMs — Wikipedia](https://en.wikipedia.org/wiki/Exponential_family_random_graph_models)
- [Stivala et al. 2020 — ERGM parameter estimation for very large directed networks (PLOS ONE)](https://journals.plos.org/plosone/article?id=10.1371/journal.pone.0227804)

State of the art:
- [Bringmann, Keusch & Lengler 2019 — Geometric inhomogeneous random graphs (Theor. Comput. Sci.)](https://www.sciencedirect.com/science/article/pii/S0304397518305309)
- [Bläsius et al. 2022 — Efficiently generating GIRGs and HRGs (Network Science, Cambridge)](https://www.cambridge.org/core/journals/network-science/article/efficiently-generating-geometric-inhomogeneous-and-hyperbolic-random-graphs/EE2080A5FEC3A6C2B3AB451934A340AC)
- [Faqeeh et al. 2018 — Scale-free network clustering in hyperbolic and other random graphs (arXiv:1812.03002)](https://arxiv.org/abs/1812.03002)
- [Muscoloni & Cannistraci 2021 — Inherent community structure of hyperbolic networks (Sci Rep)](https://www.nature.com/articles/s41598-021-93921-2)
- [Lee & Wilkinson 2019 — Review of SBMs and extensions (Applied Network Science)](https://link.springer.com/article/10.1007/s41109-019-0232-2)
- [Peixoto — graph-tool documentation on inference](https://graph-tool.skewed.de/static/doc/demos/inference/inference.html)
