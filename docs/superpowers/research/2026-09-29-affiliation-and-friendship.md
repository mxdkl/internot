# Affiliation models and friendship formation: math and data for a foci-based social graph

**Date:** 2026-09-29
**Status:** Research
**Builds on:** [`2026-05-14-real-social-graph-structure.md`](2026-05-14-real-social-graph-structure.md) (Dunbar layers, GSS, multiplexity, homophily ordering, Granovetter/Onnela), [`2026-05-14-graph-theory-foundations.md`](2026-05-14-graph-theory-foundations.md) (classical generators, dataset fingerprints), [`2026-05-14-deterministic-graph-generation.md`](2026-05-14-deterministic-graph-generation.md) (point-determinism, venue cohorts, hyperbolic weak ties). Nothing from those notes is repeated here except as a pointer.

**Working idea under review.** People belong to several overlapping groups ("foci": household, team/workplace, school cohort, neighbourhood, hobby groups, former workplaces). Each focus type is a keyed bijection on the id space, cut into cells. A tie between `a` and `b` exists with probability `1 − Π_{g ∈ G(a)∩G(b)} (1 − p_g)`, decided by a hash of the unordered pair.

**Method note.** Every number below comes from a source I opened (paper text, PMC full text, or the publisher/author page). Items marked **(inferred)** are my derivations or design proposals. Items marked **(not verified here)** are commonly cited but I could not open the primary text. §2.8 reports a small numpy simulation (n = 10⁵, PRNG-based, script not committed) that I used to check the formulas against the working idea.

---

## 0. Summary: what is directly usable

1. **The working idea already has a name and a theory.** It is the Community-Affiliation Graph Model (AGM; Yang & Leskovec 2012, eq. 1: `p(u,v) = 1 − Π_{k∈C_uv}(1 − p_k)`). Its random version is the "superposition of Bernoulli random graphs" (Bloznelis & Leskelä 2023), which has exact limit formulas for degree distribution, transitivity and the clustering spectrum C(k).
2. **Degree formula.** Degree is compound Poisson with `E[D] = μ·E[(X)₂Y]` and `Var[D] = μ(E[(X)₂Y] + E[(X)₃Y²])`, where X is group size, Y is within-group tie probability and μ is groups per person. Tail exponent: `δ = 1 + (α−2)/(1−β)` when group sizes are ~x^−α and p_g ~ x^−β.
3. **Clustering formula.** For partitions per type (the working idea): `C = Σ_g E[p_g·(d_g)₂] / E[(d)₂]` **(derived; §2.4)**. Locally, `c_v ≈ Σ_g p_g·(d_g/d)²`. Clustering is density times the Herfindahl index of a person's degree across foci.
4. **The main naivety is the reverse of the one expected.** Clustering is too high only inside a small dense focus. Globally it is **too low**: with about 6 independent foci of comparable weight, T ≈ 0.05 against a target of 0.10–0.15 at degree about 100. Simulation matches the formula (0.053 measured, 0.055 predicted).
5. **Fix for clustering (BTER principle).** Put most of each person's ties into 3–5 dense sub-foci ("circles") inside the big foci. Large foci should contribute only sparse ties with `p_g = κ/(s_g−1)`. In simulation this gives T ≈ 0.13, local C(100) ≈ 0.14 (Facebook: 0.14) and a decreasing C(k).
6. **Degree is far too narrow if sizes and memberships are fixed** (CV 0.08, target about 0.6–0.8). Fix: lognormal sociability weights (σ about 0.5–0.7), heavy-tailed group sizes, and Poisson membership counts for elective foci.
7. **Degree assortativity is zero** with independent permutations and a product kernel. Real social graphs are 0.1–0.3 (Facebook 0.226). Fix: stratify the bijection by a coarse sociability band and by age, with age-dependent sociability. This gives r = 0.26–0.31 in simulation, and it is pure-function friendly.
8. **Long-range ties can be foci too.** A hierarchy of geographic "buckets", with equal expected ties per scale, reproduces rank-based friendship `P ∝ 1/rank` (Liben-Nowell et al. 2005) and Kleinberg-navigability, using the same keyed-bijection machinery **(inferred)**. Former foci in former cities add "friend in another city" for free.
9. **Targets to hit:** active-network degree about 125–155; layers about 4 / 11 / 30 / 130 with ratio about 3.3 (Mac Carron 2016); kin about 21% of the 150 layer (Hill & Dunbar 2003); 62.8% of ties within 100 miles (Bailey et al.); average path length about 4.3–4.7 (Facebook). The full table is in §7.
10. **Validation is cheap because ids are enumerable.** Uniform ego sampling is free (no random-walk bias). Transitivity needs `k = ⌈0.5ε⁻²ln(2/δ)⌉` wedges (38k for ±0.01 at 99.9%), independent of graph size. Path lengths come from bidirectional BFS on sampled pairs plus extrapolation across N.

---

## 1. Affiliation and overlapping-community graph models (Q1)

### 1.1 Foundations: Breiger duality and Feld foci

- **Breiger (1974), "The Duality of Persons and Groups"**, *Social Forces* 53(2):181. Given a person × group incidence matrix A, the person graph is `P = A·Aᵀ`, where entry (i,j) is the number of shared groups. The group graph is `G = Aᵀ·A`. Every model below is a randomised version of P, sometimes thresholded or thinned. The working idea is exactly "P > 0, thinned per group".
- **Feld (1981), "The Focused Organization of Social Ties"**, *AJS* 86(5). Ties are organised around foci, and shared foci induce ties and homophily. The earlier notes already cover this. The quantitative consequence, that edge probability rises with the number of shared foci, is measured by AGM (§1.3).

### 1.2 Bipartite random graphs (Newman, Strogatz & Watts 2001; Newman & Park 2003)

Let groups-per-person have moments μₙ and persons-per-group have moments νₙ, and assume every co-member is tied (p = 1). NSW 2001, eq. (82):

```
1/C − 1 = (μ₂ − μ₁)(ν₂ − ν₁)² / ( μ₁ ν₁ (ν₃ − 3ν₂ + 2ν₁) )
```

- Poisson memberships and Poisson sizes give `C = 1/(μ+1)` (eq. 88). One Poisson(1) membership per person gives C = 0.5. Two memberships give C ≈ 0.33.
- Empirical check (NSW Table I): company directors, C predicted 0.590 vs 0.588 measured. Movie actors 0.084 vs 0.199. Physics (arXiv) 0.192 vs 0.452. MEDLINE 0.042 vs 0.088. For actors and scientists, real clustering is **about 2× the bipartite prediction**. This is extra closure beyond shared groups ("scientists introduce collaborators"). Real mean degree is also lower than predicted, because pairs repeat collaborations (they share several groups).
- **Newman & Park (2003)** add within-group tie probability p. They prove the model's degree assortativity r is **always ≥ 0**. With one Poisson-size group per person, `r = p`. With Poisson memberships (mean μ) and Poisson sizes (mean ν):

```
r = p / (1 + μ + ν μ p)
```

  More memberships dilute assortativity. Coauthorship example: p = 0.178, predicted r = 0.145, measured 0.174 ± 0.045.

### 1.3 AGM and BigCLAM (Yang & Leskovec 2012, 2013)

- **Definition (AGM, ICDM 2012, eq. 1):** `p(u,v) = 1 − Π_{k∈C_uv} (1 − p_k)`. Each community creates edges independently, and duplicate edges collapse. An **ε-community** containing everyone links any pair with `ε = 2|E|/(|V|(|V|−1))` (about 10⁻⁸ in their data).
- **Empirical basis:** across LiveJournal, Friendster, Orkut, YouTube, DBLP and Amazon, edge probability *increases* with the number of shared ground-truth communities. The background rate is about 10⁻⁵, and sharing 2 communities lifts it to about 10⁻¹. In LiveJournal, 8 shared groups give about 80%. Growth is diminishing-returns in online social networks and threshold-like in DBLP. Overlaps are denser than non-overlapping parts, and hubs sit in the overlaps. This directly supports the OR-rule in the working idea.
- AGM is noted to be "very similar" to Lattanzi–Sivakumar, **except that L–S make edge probability decrease with community size**. That is the κ/(s−1) fix in §8.
- The AGM paper reports, citing the authors' companion evaluation, that AGM generates heavy-tailed degrees, high clustering, and overlapping, nested and hierarchical communities. These are experimental results, not theorems. The Proc. IEEE 2014 follow-up shows that overlaps explain core–periphery structure.
- **BigCLAM (WSDM 2013)** relaxes memberships to non-negative strengths F_uc: `P(u,v) = 1 − exp(−F_u·F_vᵀ)`. This is AGM with `p_c = 1 − exp(−F_uc F_vc)`. For Internot it is the natural parameterisation of "affinity-weighted" membership: F_uc = sociability × engagement in focus c **(inferred)**.

### 1.4 Lattanzi & Sivakumar, "Affiliation Networks" (STOC 2009)

An evolving bipartite graph B(Q,U) of actors Q and societies U. At each step, with probability β a new actor copies c_q edges from a preferentially chosen prototype. With probability 1−β a new society copies c_u edges. The folded graph G links actors that share a society. A variant adds s preferential edges per new actor.

- Thm 1: both sides of B have power-law degrees, with exponent `α = −2 − c_qβ/(c_u(1−β))` (and the mirror expression), for degrees below n^γ.
- Thm 5 / Prop 6: the folded graph is heavy-tailed. All but o(n) vertices have bounded degree.
- Under a mild condition: superlinear edge count (**densification**), and the **effective diameter stabilises to a constant** (shrinking diameter).
- The bipartite backbone is sparse (constant average degree), which they exploit for shortest paths.

**Relevance:** this proves that affiliation plus heavy-tailed group sizes explains densification and small diameter. It is a growth model, however, and not point-deterministic. Use it as a target, not a mechanism.

### 1.5 BTER (Seshadhri, Kolda & Pinar 2012; Kolda, Pinar, Plantenga & Seshadhri 2014)

Inputs are a degree distribution {n_d} and a clustering-by-degree target {c_d}.

- **Phase 1:** group d+1 nodes of degree d into an "affinity block" and make it Erdős–Rényi with density `ρ_b = c_{d_b}^{1/3}`. A degree-d node in a (d+1)-block has about `C(d,2)·ρ³` triangles, so ρ³ = c.
- **Phase 2:** Chung–Lu on the "excess degree" `e_i = d_i − ρ_b·d_b`.
- Reproduces the degree distribution and C(d) by construction. Needs O(d_max) memory, generates edges independently, and has been demonstrated at 4.6 B edges.
- Limitation, as stated by the authors: no communities beyond affinity blocks, no hierarchy, no time dimension.
- Suggested idealised inputs: generalised log-normal degrees `n_d ∝ exp(−(log d / α)^δ)`, and clustering `c̄_d = c_max·exp(−(d−1)ξ)`. For example, c_max = 0.5 with target global clustering 0.10 gives ξ = 0.01.

**Lesson for Internot:** a node's clustering comes from sitting in a dense block of roughly its own degree. §2.8 shows this is exactly what the naive foci model lacks.

### 1.6 Mixed-membership SBM (Airoldi, Blei, Fienberg & Xing, JMLR 2008)

Each node has a membership vector π_i. For each pair, each endpoint samples a role from its π, and the edge probability is B[z_i→j, z_j→i]. Overlap therefore **averages** block probabilities instead of OR-ing them. Sharing two communities does not raise tie probability, which contradicts the AGM data. The model targets inference, not realism, and has no degree or clustering guarantees. **Not recommended** as a generator.

### 1.7 Random intersection graphs (RIG)

- **Karoński, Scheinerman & Singer-Cohen (1999)**, *CPC* 8(1–2): vertices get random subsets of an attribute set, and are adjacent iff their subsets intersect. This is the p = 1 case of the working idea.
- **Deijfen & Kets (2009)**, *PEIS* 23: each vertex has a random weight, and heavier vertices get larger subsets. Power-law weights give a power-law degree distribution, with tunable clustering.
- **Bloznelis (2013)**, *Ann. Appl. Probab.* 23(3): asymptotic degree distribution in sparse RIGs. Proves that clustering correlates negatively with degree, with local clustering `C(k) ~ k⁻¹`.
- **van der Hofstad, Komjáthy & Vadon (2021)**, *Adv. Appl. Probab.* 53: "RIG with communities". Each community can be an arbitrary graph, not a clique, and memberships come from a bipartite configuration model. They prove local weak convergence, the degree distribution and local clustering.
- **Karjalainen, van Leeuwaarden & Leskelä (2018)**: the "thinned" RIG (co-members linked with probability q), with moment estimators. See §2.6.
- **Bloznelis & Leskelä (2023)**, *RSA* 63(2): the general superposition, analysed in §2.

### 1.8 Scorecard

| Model | Degree | Clustering | C(k) | Assortativity | Overlap | Densification | Point-deterministic? |
|---|---|---|---|---|---|---|---|
| Bipartite/RIG (NSW, KSS) | compound; tunable | 1/C−1 formula; Poisson case 1/(μ+1) | ~k⁻¹ (Bloznelis 2013) | ≥ 0 always (Newman–Park) | yes | no (static) | **yes** |
| Thinned RIG / superposition | CPoi; power law δ=1+(α−2)/(1−β) | exact τ (Thm 3.2) | power law exponent in [0,2] | ≥ 0 (inferred via N–P) | yes | no | **yes** |
| AGM / BigCLAM | heavy-tailed (experiments) | high (experiments) | n/a | n/a | dense overlaps | no | **yes** (given memberships) |
| Lattanzi–Sivakumar | heavy-tailed (proved) | n/a | n/a | n/a | yes | **yes** (proved) | no (growth) |
| BTER | exact by construction | c_d exact by construction | exact | not controlled | no | no | mostly (block assignment by degree rank needs global sort) |
| MMSB | Poisson-like | low unless B diagonal | n/a | n/a | averaged, not OR | no | yes, but unrealistic |

---

## 2. Random intersection graphs in depth (Q2)

### 2.1 Model (Bloznelis & Leskelä 2023)

There are n nodes and m layers. Layer k has size X_k and strength Y_k. Its node set is a uniform random X_k-subset, and each pair inside is linked with probability Y_k. The overlay G is the union of the layers. Write `(P)_rs = E[(X)_r Y^s]` with falling factorials `(x)_r`, and `μ = lim m/n`.

**Mapping to the working idea:**

| Model term | Working-idea term |
|---|---|
| layer | group (a focus cell) |
| X | group size |
| Y | p_g |
| μ(P)₁₀ | mean number of groups per person |

One difference matters. B&L use uniformly random node sets, so a node's number of layers is Poisson. The working idea is a partition per type, so the count is exactly 1 per type (see §2.4).

### 2.2 Degree

**Theorem 3.1.** Degree → `CPoi(μ(P)₁₀, Bin₁₀(P))`, so `D = Σ_{k=1}^{Λ} D_k`. Here Λ ~ Poisson(μ(P)₁₀) is the number of layers covering the node, and D_k ~ Bin(X̃−1, Y), where X̃ is **size-biased**: a person sees large groups more often than their share of groups.

```
E[D]   = μ (P)₂₁                        (= Σ over groups of s(s−1)p_s, per person)
Var[D] = μ ((P)₂₁ + (P)₃₂)
E[D^r] < ∞  ⇔  (P)_{r+1,r} < ∞
```

**Power laws (Thm 4.1).** Take `P(X = x) ~ a x^−α` (α > 2) and `Y = q(X) ~ b x^−β`:

- For β ∈ (0,1), degree `f(t) ~ d·t^−δ` with `δ = 1 + (α−2)/(1−β)`.
- For β = 0, the same holds if b < 1 (or q = 1 eventually).
- **For β ≥ 1, the degree is light-tailed.** β = 1 is "layers of bounded average degree", p ∝ 1/size.

The degree tail is inherited from the size-biased group-size tail unless p_g shrinks with size. A Zipf-like employer-size distribution with constant p_g therefore produces giant-degree employees. Choosing `p_g = κ/(s−1)` (β = 1) caps within-focus degree at about Poisson(κ). The heavy tail must then come from elsewhere: sociability weights and the number of memberships, as in Deijfen–Kets.

### 2.3 Transitivity and clustering spectrum (B&L Thm 3.2, 3.3, 4.2)

```
τ = (P)₃₃ / ( (P)₃₂ + μ (P)₂₁² )          (m/n → μ)
constant strength q:  τ = q (p)₃ / ( (p)₃ + μ (p)₂² )
```

- The first denominator term counts wedges inside one layer, and the second counts wedges across two layers. Only the first kind closes, in the sparse, locally tree-like regime.
- Clustering spectrum for the power-law case: `σ(t) ~ t^{−β/(1−β)}` for β < 2/3, and `~ t^−2` for β ≥ 2/3. With constant q (β = 0), σ(t) → b, which is flat.
- The network is "strongly clustered" (σ ≫ t⁻¹) for β < 1/2.
- Percolation (Thm 3.6) multiplies τ by the retention probability θ, so clustering scales linearly with thinning.

### 2.4 Exactly one membership per type (the keyed-bijection case) (derived)

Let each person belong to exactly one group per type t, with the types independent. Write d_t for the within-type degree and q_t for that group's tie probability. Under the same tree-like assumption (a pair shares at most one group; cross-group triangles are negligible):

```
E[D]     = Σ_t E_{s~size-biased}[ (s−1) q_t(s) ]
E[(D)₂]  = Σ_t E[(d_t)₂] + Σ_{t≠u} E[d_t] E[d_u]
closed   = Σ_t E[ q_t · (d_t)₂ ]                     (ER inside a group closes a wedge w.p. q)
C        = Σ_t E[q_t (d_t)₂] / E[(D)₂]
local:   c_v ≈ Σ_g q_g · d_g(d_g−1) / (d(d−1)) ≈ Σ_g q_g · (d_g/d)²
```

This matches B&L when memberships are Poisson, where the `Σ_{t≠u}` term becomes `μ(P)₂₁²`. It also matches NSW when q = 1.

**The rule of thumb that follows:** a person's clustering is at most (typical within-focus density) × (Herfindahl index of their degree across foci). If ties are spread evenly over F foci with density q, then c ≈ q/F. Hitting c ≈ 0.15 needs, for example, q ≈ 0.5 with an effective focus count 1/H ≈ 3.

### 2.5 Assortativity

Newman–Park (§1.2): `r = p/(1+μ+νμp) ≥ 0`. Assortativity comes from **variation in group sizes**, since everyone in a big dense group has high degree. Many memberships per person dilute it toward 0. §2.8 shows that independent random permutations with a product kernel w_i·w_j give r ≈ 0, and that sorting or stratifying groups by sociability (or by age, when sociability depends on age) is needed for r ≈ 0.2–0.3.

### 2.6 Inverse design: from targets to parameters (Karjalainen, van Leeuwaarden & Leskelä 2018)

In the thinned RIG in the balanced sparse regime (m/n and size moments converge):

```
λ  ~ (m/n)(π)₂ q
σ² ~ λ (1 + q (π)₃/(π)₂)
τ  ~ (π)₃ q / ( (π)₃ + (m/n)(π)₂² )
```

For the Bernoulli special case (binomial sizes, Poisson memberships with mean μ), `μ = λ²/(σ²−λ)` and `q = τ(1+μ)`. The attainable region is `τ ≤ (1 + λ²/(σ²−λ))⁻¹`.

Estimators from an induced sample of n₀ nodes (consistent if n₀ ≫ n^{2/3}) use N_K2 (links), N_S2 (2-stars) and N_K3 (triangles):

```
λ̂ = (n−1) N_K2 / C(n₀,2)
μ̂ = 2 N_K2² / (n₀ N_S2 − 2 N_K2²)
q̂ = 3 n₀ N_K3 / (n₀ N_S2 − 2 N_K2²)
```

**Worked example (inferred).** Target λ = 130 and SD = 90, so σ² = 8100:

- μ = 16900/7970 ≈ **2.1 effective groups per person**.
- The maximum attainable τ is 1/3.1 ≈ 0.32.
- τ = 0.12 needs q ≈ 0.37.

Read this as saying that realistic dispersion plus clustering implies each person's ties come from **about 2–3 effective dense groups**, not from 6–10 equal foci. §2.8 finds the same thing independently.

### 2.7 Choosing group-size distributions

- **Size-biasing is the trap.** The mean within-focus degree is `E[(S)₂ q(S)]/E[S]`, not `E[S]·q`. For lognormal employer sizes (median 20, σ = 1.3), the size-biased mean is about 20·e^{1.5σ²} ≈ 250. A typical employee sees a large employer.
- For **sparse "context" foci** (whole workplace, neighbourhood, school cohort, former workplace), use `p = 1 − exp(−κ·η_g·w_a w_b/(s−1))`. Within-focus degree is then about κ regardless of size (B&L β = 1, light tail). Pick κ per type from the "where did you meet" shares (§3.2).
- For **dense "circle" foci** (household, extended kin, team, friendship circle, small hobby group), use sizes of 5–40 and `p = min(1, ρ·η_g·w_a w_b)` with ρ ≈ 0.5–0.9. BTER's ρ = c^{1/3} tells you the density a circle needs to give its members clustering c from that circle alone. For example, c = 0.3 needs ρ = 0.67.
- **Heavy but bounded tail:** lognormal sociability w (σ ≈ 0.5–0.7; McCormick et al. fit σ̂ = 0.68 to "know" network size), Poisson(λ·w) memberships for elective foci, and a hard cap on w. Group-level intensity η_g (lognormal, σ ≈ 0.5–1) adds between-group variation.

### 2.8 Simulation check of the working idea (n = 10⁵)

I used a numpy PRNG, not Internot code. Foci are made by permuting ids and cutting them into blocks, which is the keyed-bijection analogue. Ties use the OR-rule. T is global transitivity from 40k uniform wedges (±0.010 at 99.9%). C_loc is exact average local clustering over 3,000 sampled egos. r is Pearson degree correlation over all edges.

| Config | mean deg | CV | T | C_loc | C(k) | r |
|---|---|---|---|---|---|---|
| **Naive**: fixed sizes (household 3, kin 25, work 100, neighbourhood 200, cohort 200, 2 hobbies of 50), constant p per type (1, .8, .3, .1, .1, .2) | 110.2 | **0.08** | **0.053** | 0.056 | flat | **0.00** |
| + heterogeneous sizes, lognormal w, κ/(s−1), nested teams, some circles, Chung–Lu weak ties | 83.0 | 0.69 | 0.044 | 0.057 | 0.074 → 0.030 | 0.01 |
| + "introduction" closure among strong ties (p = 0.06) | 74.0 | 0.56 | 0.058 | 0.076 | decreasing | 0.03 |
| **Circles**: most ties inside dense sub-foci of big foci (ρ = 0.8), sparse remainder κ = 2–3 | 59.8 | 0.57 | 0.124 | 0.156 | 0.18 → 0.065 | 0.015 |
| **Final**: circles + age-dependent sociability (peaks at 25) + age-sorted cohorts + sociability-sorted hobby/work circles | 87.5 | 0.58 | **0.128** | 0.142 | 0.16 at k 50–100 → 0.08 at 250+ | **0.306** |
| Same, but stratified by sociability **terciles** (bijection-friendly) instead of a full sort | 88.6 | 0.61 | 0.127 | 0.142 | same | **0.260** |

**Checks:**

- Naive analytic prediction (§2.4): E[D] = 110.3 (measured 110.2). C = 663/12132 = 0.055 (measured 0.053 / 0.056).
- Final config vs. Facebook: local C at degree about 100 is 0.14–0.16 (Facebook C(100) = 0.14). T = 0.128 (LiveJournal global 0.124). r = 0.26–0.31 (Facebook 0.226).
- Adding closure edges alone does not fix clustering when sparse foci dominate the wedge count. The degree mix has to change.

---

## 3. Friendship formation: mechanisms and data (Q3)

### 3.1 Triadic and focal closure

- **Share of new ties that close a triangle.** Leskovec, Backstrom, Kumar & Tomkins (KDD 2008): Flickr **65.6%** (2003–05), LinkedIn **49.6%** (2003–06), Delicious **27.7%**, Yahoo Answers **23.4%**. Edge counts to nodes h hops away decay exponentially in h. The best simple closure model is "random-random": pick a random neighbour, then a random neighbour of it. Weighting the intermediary by common friends improves the likelihood further.
- **Kossinets & Watts (2006, *Science* 311):** 43,553 students, faculty and staff at a large US university, from one academic year of e-mail headers matched with affiliations and attributes. Tie formation is dominated by network topology (mutual acquaintances, i.e. cyclic closure) combined with organisational structure (shared classes, i.e. focal closure). Aggregate properties approach equilibrium while individual properties stay unstable. (The frequently quoted multipliers for shared class and mutual friend are **not verified here**.)
- **Closure beyond foci is real and large.** NSW 2001 found real clustering about 2× the bipartite prediction for actors and physicists (§1.2). In the 2021 American Perspectives Survey, **40%** of Americans with close friends made one through existing friends.

### 3.2 Where friends come from and how long it takes

- **American Perspectives Survey (Cox 2021)**, n = 2,019, Ipsos KnowledgePanel, May 2021. Among Americans with close friends, the share who made **at least one** close friend in each place (multiple answers allowed): workplace (own or spouse's) **54%**, school **47%**, through friends **40%**, neighbourhood **35%**, religious organisation 21%, club/organisation 19%, child's school 10%, online 8%. Use these as relative weights for κ per focus type **(inferred mapping)**.
- **Hours to friendship (Hall 2019, *J. Soc. Pers. Relat.*):** about **50 h** of time together to go from acquaintance to casual friend, **90 h** to friend, **>200 h** to close friend. The sample was 355 recently relocated adults plus 112 first-year students. "Hours spent working together just don't count as much." This gives a mechanistic tie-strength model: accumulated shared leisure hours per focus, with work hours discounted.
- **Settings of daily contact (POLYMOD, Mossong et al. 2008):** 7,290 participants and 97,904 contacts across 8 European countries. Mean **13.4 contacts/day** (Germany 7.95, Italy 19.77). Contacts occur at home 23%, work 21%, school 14%, leisure 16% and travel 3%. About 75% of first-time contacts last under 15 min.

### 3.3 Homophily (quantitative additions to the McPherson ordering in the earlier notes)

- **Age (Facebook, Ugander et al. 2011):** a random neighbour's age peaks at the ego's own age. It falls off "nearly exponentially" toward older ages, and more slowly toward younger ages before levelling off. The spread widens with ego age. POLYMOD age mixing is strongly assortative, most so at ages 5–24 and least at 55–69.
- **Generational peak (Bhattacharya et al. 2016, 3.2 M phone users, 2007):** the alter-age distribution is bimodal, with peaks about **25 years apart** (peers plus parent/child generation). Kin ties create a second age mode, and the age kernel should be a mixture.
- **Gender:** on whole Facebook, effectively none. p(F|M) = 0.5131 and p(F|F) = 0.5178, against p(F) = 0.5156 at the end of a random edge. In inner layers it is strong: "each sex exhibits a strong preference for members of their own sex" in the support clique (Dunbar & Spoors 1995). Gender homophily should therefore scale with tie strength **(inferred)**.
- **Country:** 84.2% of Facebook edges are within-country.
- **Race/ethnicity and education:** the ordering (race > age > religion > education > occupation > gender) is in the 2026-05-14 notes. I found no verified effect sizes for this survey. Mostly induce these through stratified foci (neighbourhood, school, occupation), per Feld.

### 3.4 Geography

- **Rank-based friendship (Liben-Nowell et al. 2005, *PNAS*):** 495,836 LiveJournal users with US locations.
  - The fit is `P(δ) ∝ ε + 1/δ^α` with α ≈ 1 and ε ≈ 5.0 × 10⁻⁶. About **1/3 of friendships are independent of geography**, and the rest follow `Pr[u→v] ∝ 1/rank_u(v)`, where rank_u(v) is the number of people closer to u than v.
  - On a uniform 2-D grid rank ≈ d², which gives d⁻², Kleinberg's exponent.
  - Greedy geographic routing: 12.78% of chains reached the target city, median 4 hops, mean 4.12.
  - Theorem: in any rank-based network, greedy paths have expected length O(log³ n).
- **Social Connectedness Index (Bailey, Cao, Kuchler, Stroebel & Wong 2017/2018):** "for the population of the average county, **62.8% of friends live within 100 miles**". Connectedness is also associated with county-to-county migration.
- **Neighbours:** 35% of Americans with close friends have made one in their neighbourhood (Cox 2021). Neighbour and coworker networks are "important only in specific age ranges" (Wrzus et al. 2013).

### 3.5 Persistence of school, college and old-focus ties

- **Network size across life (Wrzus et al. 2013 meta-analysis, 277 studies, N = 177,635):**
  - Global network size rises until young adulthood and then declines steadily.
  - Friendship and personal networks decline throughout adulthood.
  - The family network is stable from adolescence to old age.
  - Life events (parenthood, job entry, widowhood) produce matching network changes.
  - Personal and friendship networks have shrunk over the last 35 years (period effect).
- The earlier notes cover Roberts & Dunbar 2011 (friendships decay without contact; kin buffered) and Saramäki 2014 (layer identity turns over while the signature persists).
- I found **no verified persistence rates** for school or college friendships. Treat per-type half-lives as calibration knobs, checked against Wrzus's age curve and Cox's "47% made a close friend at school" **(inferred)**.

---

## 4. Ego-network numbers (Q4)

| Quantity | Value | Source (population, year) |
|---|---|---|
| Cumulative layers (4-cluster Jenks on call frequency) | **4.1, 11.0, 29.8, 128.9** | Mac Carron, Kaski & Dunbar 2016. European operator, 2007, 34.9 M users, ~6 B calls. 26,680 egos with ≥100 reciprocated alters (mean 129.9, SD 37.7; mean 3,553.8 calls/yr) |
| Same, forced to 4 clusters | 3.5, 10.6, 31.1, 129.9; **scaling ratio 3.3** | same |
| Egos with 50–100 alters (N = 301,190) | 3 clusters: 3.9, 11.9, 63.9 | same |
| Layer-size distribution | **log-normal within each layer** | same |
| Support clique / sympathy group (survey) | **4.1 / 13.6** | Dunbar 2016, UK Sample 1 (N = 2000) |
| Literature norms | 3.8 ± 2.29 / 11.3 ± 6.19 | Hill & Dunbar 2003, via Dunbar 2016 |
| Facebook friends | mean 155.2 (social media users), 182.8 (professionals, N = 1375) | Dunbar 2016 |
| Christmas-card network | max 153.5, contacted 124.9; **kin about 21%** (constant) | Hill & Dunbar 2003 (Western sample) |
| Preferred group sizes | 3–5, 9–15, 30–45 …, ratio about 3 | Zhou, Sornette, Hill & Dunbar 2005 |
| "Know" network (by sight/name, contact in 2 years) | mean **611**, median 472, **lognormal μ̂ = 6.2, σ̂ = 0.68** | McCormick, Salganik & Zheng 2010 (US, n = 1,370) |
| Close friends, distribution | 0: **12%**; 1–3: **49%**; 10+: **13%** (1990 Gallup: 3% / 27% / 33%) | Cox 2021 (US) |

**Variance by age and sex.**
- Monthly phone alters peak at **about age 25**, decline to about 45, plateau until about 55, then decline. Men have more alters than women below age 39, and the order reverses afterwards (Bhattacharya 2016).
- Women list more friends than men (165.5 vs 145.0; 196.2 vs 156.6), and network size declines with age class.
- Support-clique and sympathy-group sizes **hardly vary with age** (Dunbar 2016). Age should therefore scale the outer layers, not the inner ones **(inferred)**.

**Personality.** Extraverts have larger layers but not closer relationships (Pollet, Roberts & Dunbar 2011, *J. Individual Differences*; **not verified here**). Model this as extraversion → sociability w → more memberships and higher tie probability in outer foci.

**Kin share by layer.**
- About 21% of the 150 layer is kin (Hill & Dunbar 2003).
- Kin are over-represented relative to chance, and **people from larger families have proportionately fewer non-kin** (kin displace friends). Kin do *not* make up a larger share of the support clique than of the wider monthly-contact network (Dunbar & Spoors 1995).
- In GSS "discuss important matters" data, about half of confidants are kin (earlier notes).
- The two measures differ by definition, so calibrate both: kin about 20–25% of the 150 layer and about 40–55% of confidants.

**Contact frequency by layer.**
- The ~15 layer is "the number of alters contacted at least once a month" (Bhattacharya 2016). In phone data, "people focus their (phone-based) social effort each month on around 15 people". Dunbar & Spoors measured the sympathy group as adults contacted at least monthly.
- The 150 layer is at least yearly (the Christmas-card method).
- The ~5 layer is at least weekly (commonly stated; **not verified here**).
- Costs of maintaining a relationship are roughly linear in the number of ties per layer, while benefits are asymptotic (Sutcliffe et al. 2012).

---

## 5. Weak ties and long-range structure (Q5)

- **Kleinberg (STOC 2000).** On an n×n grid, each node has local contacts plus q long-range contacts with `P ∝ d(u,v)^−r`:
  - r = 0: any decentralised algorithm needs ≥ α₀·n^{2/3} steps.
  - r = 2: greedy routing takes ≤ α₂(log n)² steps.
  - r < 2: ≥ n^{(2−r)/3}. r > 2: ≥ n^{(r−2)/(r−1)}.
  - Only the exponent equal to the dimension is navigable.
- **Rank-based friendship generalises this to non-uniform density** (§3.4). It is the right target for Internot, whose population density comes from real cities.
- **Hierarchical identity (Watts, Dodds & Newman 2002, *Science* 296).**
  - Individuals belong to groups of size g ≈ 100 nested in a hierarchy with branching b. Distance x_ij is the height of the lowest common ancestor, and ties are drawn with `p(x) ∝ exp(−αx)`.
  - There are H independent hierarchies (e.g. geography and occupation), and social distance is `y = min_h x_h`.
  - Networks are searchable over a broad region with α > 0 and H > 1, best at **H = 2–3**. With α = 2 and H = 1, only 0.4% of searches succeed; adding a second hierarchy gives 14.4%.
  - Milgram-consistent parameters: N = 10⁸, H = 2, α = 1, b = 10, g = 100, z = 300.
  - **This is a foci model**: a person has a group in each hierarchy, and tie probability depends on the smallest shared group.
- **Global social search (Dodds, Muhamad & Watts 2003):** more than 60,000 senders, 18 targets in 13 countries. Median **5–7 steps** after correcting for attrition. Successful chains used intermediate-to-weak ties and professional relationships, not hubs.
- **Measured distances.**
  - Facebook, May 2011: 721 M active users and about 69 B links. Average distance **4.74** (Backstrom et al. 2012), US 4.3 (Ugander et al.). 99.91% of users are in the giant component.
  - A user with 100 friends has **27,500 unique friends-of-friends** (40,300 non-unique), because friends have more friends than you.
- **GIRG (Bringmann, Keusch & Lengler).** Each node has a weight w_v (power law β > 2) and a position x_v on the torus T^d, with `p_uv = Θ(min{1, (w_u w_v/W)^α / ‖x_u−x_v‖^{αd}})`. Results:
  - Degree follows a power law with exponent β.
  - Clustering is Θ(1).
  - For 2 < β < 3, there is a giant component and average distance (2 ± o(1))·log log n / |log(β−2)|.
  - Sparse separators exist, O(n) expected sampling time, and O(n)-bit storage with O(1) degree and i-th-neighbour queries.
  - Hyperbolic random graphs are a special case. Scale-free percolation on Z^d is the lattice analogue.
  - Caveat: log log n distances need β < 3. Social degree is closer to lognormal, so expect ordinary log n / log k̄ small-world distances instead.

**What is locally computable (inferred design).**
1. **Geographic hierarchy as foci.** For scales ℓ = 1..L (block → neighbourhood → city → metro → region → country → world), partition each scale-ℓ cell's residents into buckets of size b with a keyed bijection. Link within a bucket with `q_ℓ` chosen so the expected ties per scale c_ℓ are roughly constant.
   - Since `∫_r^{2r} dr'/r'` is constant, equal ties per doubling of rank is exactly rank-based friendship. With cells nested by population, each level is a rank doubling (or constant factor).
   - This is the Watts–Dodds–Newman/Kleinberg group construction implemented with the same partition primitive as every other focus. Enumeration costs O(b) per level, and ties are symmetric by construction.
2. **Mobility.** Career-arc and education moves put people into former foci in former cities, with ties decaying but persisting. This creates "friend of friend in another city" without any random edges, consistent with SCI's association with migration.
3. **A small ε-community** (Chung–Lu on w, a few ties per person) for the geography-independent third. In simulation it costs about 4–6 degree.
4. **Sanity estimate:** a random-graph approximation ℓ ≈ ln N / ln k̄ = ln(4.3·10⁹)/ln(130) ≈ **4.6** at Internot's maximum population (inferred). Clustering lengthens this and friendship-paradox branching shortens it. Measure it (§6).

---

## 6. Validation at scale with local queries (Q6)

**Internot's advantage:** ids are enumerable, so **uniform ego sampling is free**. Real-network studies must use crawls. BFS and plain random walks are "substantially biased", and only Metropolis–Hastings or re-weighted random walks are approximately uniform (Gjoka et al. 2010). Also avoid induced-subgraph sampling from random nodes (Leskovec & Faloutsos 2006 evaluate its distortions). If you use it for the §2.6 estimators, n₀ must be ≫ n^{2/3}. Query full egos instead.

| Metric | Estimator on local queries | Sample size |
|---|---|---|
| Reciprocity and determinism | For sampled u, every v ∈ N(u,t) must have u ∈ N(v,t), identical strength and kind-mask, and bit-identical results across runs and threads | 10⁴ egos (zero tolerance) |
| Degree distribution | Uniform ids → exact degree. Mean, CV, quantiles; KS against lognormal. Compare layer sizes separately | n = 10⁴ gives mean ± 1.4% at CV 0.7 and KS D_crit = 1.36/√n = 0.014. Tail P(D > k) = p to ±10% relative needs n ≈ (1−p)/(0.01p), i.e. 10⁴ for p = 1% |
| Global transitivity T | Wedge sampling: centre v ∝ C(d_v,2), two random neighbours, test adjacency. With uniform sampling, weight by C(d_v,2) (ratio estimator) or reject with d_v²/d_max² | Hoeffding: k = ⌈0.5ε⁻²ln(2/δ)⌉. ε = 0.01, δ = 0.001 gives about 38k wedges, independent of N (Seshadhri, Pinar & Kolda 2013) |
| Average local C and C(k) | Uniform centres, one wedge each (same bound), or exact c_v for about 3k egos (d² adjacency tests each) binned by degree | about 38k wedges, or about 500 egos per degree bin for ±0.01 |
| Degree assortativity r | Uniform edges: uniform ego u, uniform neighbour v, importance weight ∝ d_u. Pearson on (d_u, d_v). Bootstrap over egos for the CI | 10⁵ edge samples (heavy tails inflate variance; bootstrap) |
| Homophily | Newman attribute assortativity on sampled edges, per attribute (age, education, ethnicity, sex). Age-gap histogram vs random pairs. p(age′ \| age) curves vs Ugander Fig. 8 shape. Same-sex share by layer | 10⁵ edges; per-layer shares from 10⁴ egos |
| Geography | Share of ties < 100 miles (target 62.8%). P(tie \| distance) fit to ε + 1/δ. Tie rate vs rank_u(v) (target ∝ 1/rank) | 10⁴ egos |
| Dunbar layers | Per ego, sort alters by strength. Counts above calibrated thresholds, or Jenks/k-means on log strength as in Mac Carron. Check means about 4/11/30/130, ratio about 3, lognormal per layer, age trend only in outer layers | 10⁴ egos |
| Path length / small world | Bidirectional BFS between sampled pairs using N(·). Also: unique FoF count vs degree (Ugander: about 27,500 at k = 100). Build the generator at N = 10⁵, 10⁶, 10⁷ (same local parameters) and fit ℓ(N) = a + b·ln N; extrapolate to 4.3·10⁹ and check 4–5 | 200–1000 pairs gives mean ± about 0.05 |
| Giant component | Fraction of sampled egos whose BFS reaches a fixed hub set within 8 hops | 10³ egos |
| Model-parameter recovery | Estimate λ̂, μ̂, q̂ with the Karjalainen et al. estimators on a materialised sub-population and compare to the configured parameters (a regression test for the formulas) | one city-sized sub-population |

**Snowball samples** are unnecessary for estimation, because uniform sampling and full ego queries dominate them. They remain useful only as a performance test of `neighbors()` fan-out (2-hop balls of about 10⁴ nodes).

---

## 7. Realism targets

| Metric | Target | Source |
|---|---|---|
| Mean degree, active network (150 layer) | 125–155 | Hill & Dunbar 2003 (124.9 / 153.5); Mac Carron 2016 (129.9 ± 37.7, filtered); Dunbar 2016 (FB 155–183) |
| "Know" network (optional outer tier) | mean about 600, median about 470, lognormal σ ≈ 0.68 | McCormick et al. 2010 |
| Degree dispersion | CV 0.6–0.8; log-normal-ish; p99/mean about 3 | McCormick σ̂ = 0.68 gives CV 0.76 (inferred); Mac Carron lognormal |
| Support clique / sympathy / affinity / active | about 4 / 11–14 / 30 / 130; ratio 3–3.3 | Mac Carron 2016; Dunbar 2016; Zhou 2005 |
| Inner-layer SD | support 2.3, sympathy 6.2 | Hill & Dunbar via Dunbar 2016 |
| Kin share | about 21% of 150 layer; about 50% of confidants | Hill & Dunbar 2003; GSS (earlier notes) |
| Zero close friends | about 3–12% (period dependent) | Cox 2021 |
| Global transitivity T | 0.10–0.15 at degree about 100 | LiveJournal 0.124; Flickr 0.112 (Seshadhri et al. Table 2) |
| Average local clustering | 0.14–0.35; Facebook C(100) = 0.14 | Ugander 2011; LiveJournal 0.345 |
| C(k) | decreasing (about k⁻¹ in RIGs) | Bloznelis 2013; Ugander 2011 |
| Degree assortativity | +0.1 to +0.3 (FB 0.226) | Ugander 2011; Newman–Park |
| New ties closing triangles | 25–65% | Leskovec et al. 2008 |
| Age homophily | mode at own age; faster decay toward older; second mode about 25 yr apart (kin) | Ugander 2011; Bhattacharya 2016 |
| Gender homophily | about 0 overall; strong same-sex in support clique | Ugander 2011; Dunbar & Spoors 1995 |
| Geography | 62.8% of ties within 100 mi; about 1/3 geography-independent; P ∝ ε + 1/δ; ∝ 1/rank | Bailey et al.; Liben-Nowell 2005 |
| Average distance | 4.3 (national) – 4.74 (global); giant component > 99.9% | Ugander 2011; Backstrom 2012 |
| Unique friends-of-friends at k = 100 | about 27,500 | Ugander 2011 |
| Monthly active contacts | about 15 | Bhattacharya 2016 |
| Daily contacts (all, incl. strangers) | about 13 (8–20 by country) | Mossong 2008 |
| Age profile of outer layers | peak about 25, decline to 45, plateau to 55, decline; inner layers flat | Bhattacharya 2016; Dunbar 2016; Wrzus 2013 |
| Where close friends were met (≥1) | work 54%, school 47%, via friends 40%, neighbourhood 35%, religion 21%, clubs 19% | Cox 2021 |

---

## 8. Construction recommendations

### 8.1 Where the working idea is naive, and the fix for each

1. **Degree is too narrow.** Fixed sizes, one membership per type and constant p give CV 0.08 (§2.8). *Fix:* lognormal sociability w (σ about 0.5–0.7, age-dependent, correlated with extraversion); heavy-tailed sizes for workplaces; Poisson(λ·w) memberships for elective foci; group intensity η_g.
2. **Clustering is locally too high, globally too low.** A household is a clique (c = 1). But with about 6 independent foci, cross-focus wedges dominate and T ≈ 0.05, following `c ≈ Σ q_g (d_g/d)²`. *Fix:* route most degree through **3–5 dense circles** (kin, team, friend circle inside school cohort, friend circle inside neighbourhood/hobby) with ρ ≈ 0.6–0.8. Let the big foci contribute sparse `κ/(s−1)` ties only.
3. **Mega-groups explode degree.** Constant p in a 3,000-person employer gives about 900 ties. *Fix:* `p = 1 − exp(−κ η_g w_a w_b/(s−1))` for context foci (B&L β = 1; Lattanzi–Sivakumar size-decreasing edge probability).
4. **No assortativity** (r ≈ 0). *Fix:* stratify the bijection domain by a coarse sociability band and by age. Make outer-layer sociability age-dependent (peak about 25).
5. **Foci are uncorrelated.** Independent permutations mean coworkers of any age and cohorts unrelated to hometown, so there is no induced homophily. *Fix:* each focus type permutes within a **stratum** (cohort: birth year × hometown; neighbourhood: city cell; workplace: city × industry; hobby: city × interest × sociability band). Homophily is then induced by foci, which Feld and the earlier notes identify as the dominant channel.
6. **No closure beyond foci.** Real clustering is about 2× the bipartite prediction, and 40% of people made close friends via friends. *Fix:* circles provide most of it. Optionally add "introductions" among strong ties (below), keeping its degree share small.
7. **One pair hash loses provenance.** *Fix:* hash per (pair, focus) and tie iff any `h(pair, g) < p_g`. This has the same distribution as the OR-rule and yields the kind-mask and "met via" labels for the typed-edge design in the earlier notes.
8. **Foci are static.** *Fix:* memberships are time intervals (from career arc, education and moves). Ties persist after the focus ends with a deterministic death time `τ* = −τ_T·ln h(pair, "survive")`. Since h is fixed, "alive at t" is monotone, which keeps the graph consistent across queries.
9. **Existence is conflated with strength.** Dunbar layers need a strength, not a second graph. *Fix:* §8.3.

### 8.2 Focus catalogue (starting parameters, inferred; calibrate against §7)

| Focus type | Stratum (bijection domain) | Size | Memberships | Within-group tie p | Strength base | Lifetime after exit |
|---|---|---|---|---|---|---|
| Household | from social spec (households Space) | US-like 1–7 | 1 | 1 | very high | kin: ∞; ex-partner decays |
| Extended kin | lineage (spec) | about 1+Poi(20–25) | 1 | 0.7·w_a w_b·η | high | ∞ (slow decay) |
| Team | inside workplace | about 5–10 | 0–1 | 0.7–0.8·w w η | high while active | τ ≈ 2–5 yr |
| Workplace (context) | city × industry | lognormal, median 20, σ 1.3, cap | 0–1 | κ ≈ 3–6 over s−1 | low | τ ≈ 1–3 yr |
| Work circle | inside workplace, sociability band | about 1+Poi(30–40) | 0–1 | ρ ≈ 0.6–0.7 | medium | τ ≈ 3–10 yr |
| School cohort (context) | birth year × hometown | about 1+Poi(120–150) | 1 per level attended | κ ≈ 3–4 | low | τ ≈ 5–10 yr |
| School/college circle | inside cohort, sociability band | about 1+Poi(30–40) | 1 per level | ρ ≈ 0.6–0.7 | medium-high | τ ≈ 10–30 yr |
| Neighbourhood (context) | city cell | about 1+Poi(150) | 1 | κ ≈ 2–3 | low | τ ≈ 1–2 yr |
| Local circle | inside neighbourhood, age band | about 1+Poi(12–20) | 1 | ρ ≈ 0.6–0.7 | medium | τ ≈ 3–5 yr |
| Hobby/interest | city × interest × sociability band | about 1+Poi(15–25) | Poi(1.0–1.2·w) | 0.3–0.5·w w η | medium | τ ≈ 1–3 yr |
| Former workplaces | as workplace, past intervals | as workplace | Poi(about 1.5) over career | κ ≈ 2 | low | decaying |
| Geographic buckets ℓ = 1..L | cell at scale ℓ | b ≈ 50 | 1 per scale | q_ℓ so Σ_ℓ ≈ 5–10 ties | low | re-drawn on move |
| ε-community | global | – | – | Chung–Lu on w, about 2–4 ties | low | – |

Expected composition at defaults: about 20–25 kin (about 21% of the 150 layer), about 30–40 each from work and school, about 10–20 local and hobby, about 10 long-range. Adjust κ and ρ until E[D] ≈ 130 and T ≈ 0.12, using §2.4 analytically first, then simulating.

### 8.3 Tie strength and emergent Dunbar layers (inferred design)

- **Strength.** `strength(a,b,t) = Σ_{g shared} base_T × hours_T(g, overlap) × affinity(a,b) × decay_T(t − exit_g)`, plus kin terms. Here affinity(a,b) is a symmetric hash-and-homophily term (age gap, sex, education). Hall's thresholds (50 / 90 / 200 hours of leisure time, with work hours discounted) map accumulated hours to casual friend / friend / close friend.
- **Layers are symmetric absolute thresholds** on strength, calibrated so that population means are about 4 / 11 / 30 / 130. Do not use per-ego top-k ranks. Thresholds keep "A is in B's 15" symmetric and let layer sizes vary lognormally across egos, as in Mac Carron.
- **Expected emergence.**
  - Support clique ≈ household (about 1.5) + 1–2 close kin + 1–2 circle cores.
  - Sympathy group adds parents, siblings and the rest of the circle cores.
  - The 50 layer adds circles.
  - The 150 layer is everything above the floor.
  - Inner-layer size should be flat in age. Outer layers follow the sociability age curve.
- **Contact frequency.** Map strength to weekly (≈5), monthly (≈15) and yearly (≈150) contact rates via the existing `graph::comm_intensity`.

### 8.4 Pseudocode (primitive shapes; not tied to current APIs)

```rust
// A focus type = keyed bijection over a stratum's "stub line", cut into variable-size groups.
struct FocusType { key: u64, max_m: u32, size: SizeDist, kernel: Kernel, life: Lifetime }

// Stub line for stratum S: slots [0, |S| * max_m). Stub (rank_in_S(p), k) is active iff k < m(p, t).
fn slot_of(ft: &FocusType, s: Stratum, p: Person, k: u32) -> u64 {
    feistel(ft.key ^ s.id, s.len() * ft.max_m as u64, rank_in(s, p) * ft.max_m as u64 + k as u64)
}

// Variable sizes without global state: fixed super-blocks of B slots, segmented by hash.
// (A group larger than B spans consecutive super-blocks: store sizes as "continues" flags.)
fn segments(ft: &FocusType, s: Stratum, block: u64) -> Vec<Range<u64>> {
    let mut out = vec![]; let mut pos = 0; let mut j = 0;
    while pos < B { let len = ft.size.sample(hash3(ft.key, s.id, block, j)).min(B - pos);
                    out.push(block * B + pos .. block * B + pos + len); pos += len; j += 1; }
    out  // cacheable per (ft, s, block)
}

fn group_of(ft, s, p, k) -> GroupId { let x = slot_of(ft, s, p, k);
    let seg = segments(ft, s, x / B).into_iter().position(|r| r.contains(&x)).unwrap();
    GroupId { ft: ft.key, stratum: s.id, block: x / B, seg } }

fn members(g: GroupId) -> impl Iterator<Item = Person> {   // O(|g|) Feistel inversions
    g.slots().map(|x| feistel_inv(..)).filter(|stub| stub.k < m(stub.person, t)).map(|s| s.person) }

// Tie: OR over shared groups, one hash per (pair, group) => provenance + kind mask.
fn tie(a: Person, b: Person, t: Time) -> Option<Tie> {
    let (lo, hi) = (a.min(b), a.max(b));
    let shared = groups_at(a, t).intersect(groups_at(b, t));   // O(#memberships), ~15-30
    let mut kinds = 0u16; let mut strength = 0.0;
    for g in shared {
        let p = g.kernel.prob(g.size, g.eta(), w(a), w(b), affinity(a, b));
        if unit(hash4(g.id(), lo, hi, KEY_TIE)) < p && alive(g, lo, hi, t) {
            kinds |= g.kind_bit(); strength += g.strength_contrib(lo, hi, t); }
    }
    (kinds != 0).then(|| Tie { kinds, strength, layer: layer_of(strength) })
}

fn neighbors(a: Person, t: Time) -> Vec<Tie> {       // O(Σ |g| over a's groups) ≈ 10^3–10^4 hashes
    groups_at(a, t).flat_map(|g| members(g)).filter(|b| b != a).dedup()
        .filter_map(|b| tie(a, b, t)).collect()      // dedup handles pairs sharing several groups
}

// Optional closure layer ("introductions"): v introduces pairs among its strong ties.
// Symmetric because strong(v, a) is symmetric; one round only (never iterate over closure edges).
fn intro_tie(a, b, t) -> bool {
    strong_ties(a, t).intersect(strong_ties(b, t))
        .any(|v| unit(hash4(v, a.min(b), a.max(b), KEY_INTRO)) < P_INTRO) }
```

**Cost notes (inferred).**
- Stratum-local ranks (`rank_in(s, p)`) need the stratum fields in an indexable layout. That is the "put narrowly-constrained fields last" rule. Cohort, city cell and industry must be bit fields of whatever Space holds focus memberships, not hashed attributes.
- `neighbors()` enumerates about Σ|g| over a person's groups. That is about 150 × 3 + 1,000 per big context focus, so pre-filter each large sparse focus with a sub-bucket trick (split into √s buckets, κ ties per bucket) to avoid O(s) scans at 3,000-person employers.

### 8.5 Calibration loop

1. Choose types and sizes. Compute `E[D]` and `C = Σ E[q(d)₂]/E[(D)₂]` analytically per age/sociability class (§2.4). Adjust κ and ρ.
2. Run the §6 battery on 10⁴ egos at N = 10⁶ and N = 10⁸ (it is local, so N only matters for path length).
3. Lock the seed and parameters. Make the §7 table the assertions of `internot/tests/social_validation.rs`, with tolerances set from the §6 sample-size column.

---

## 9. Sources

Affiliation and RIG models
- Breiger 1974, The Duality of Persons and Groups, *Social Forces* 53(2) — https://doi.org/10.2307/2576011
- Feld 1981, The Focused Organization of Social Ties, *AJS* 86(5) — https://doi.org/10.1086/227352
- Newman, Strogatz & Watts 2001, Random graphs with arbitrary degree distributions, *PRE* 64 — https://arxiv.org/abs/cond-mat/0007235
- Newman & Park 2003, Why social networks are different, *PRE* 68, 036122 — https://arxiv.org/abs/cond-mat/0305612
- Lattanzi & Sivakumar 2009, Affiliation Networks, STOC — https://www.cs.cornell.edu/courses/cs6241/2019sp/readings/Lattanzi-2009-affiliation.pdf
- Yang & Leskovec 2012, Community-Affiliation Graph Model, ICDM — https://cs.stanford.edu/people/jure/pubs/agmfit-icdm12.pdf
- Yang & Leskovec 2014, Overlapping communities explain core–periphery organization, *Proc. IEEE* 102(12) — https://cs.stanford.edu/people/jure/pubs/communities-pieee14.pdf
- Yang & Leskovec 2013, BigCLAM, WSDM — https://cs.stanford.edu/people/jure/pubs/bigclam-wsdm13.pdf
- Kolda, Pinar, Plantenga & Seshadhri 2014, A Scalable Generative Graph Model with Community Structure (BTER), *SIAM J. Sci. Comput.* — https://arxiv.org/abs/1302.6636
- Airoldi, Blei, Fienberg & Xing 2008, Mixed Membership Stochastic Blockmodels, *JMLR* 9 — https://jmlr.org/papers/v9/airoldi08a.html
- Karoński, Scheinerman & Singer-Cohen 1999, On random intersection graphs: the subgraph problem, *CPC* 8 — https://doi.org/10.1017/S0963548398003459
- Deijfen & Kets 2009, Random intersection graphs with tunable degree distribution and clustering, *PEIS* 23 — https://www.cambridge.org/core/journals/probability-in-the-engineering-and-informational-sciences/article/abs/random-intersection-graphs-with-tunable-degree-distribution-and-clustering/32954CFB72F5B470A207042D05492991
- Bloznelis 2013, Degree and clustering coefficient in sparse random intersection graphs, *Ann. Appl. Probab.* 23(3) — https://doi.org/10.1214/12-AAP874
- van der Hofstad, Komjáthy & Vadon 2021, Random intersection graphs with communities, *Adv. Appl. Probab.* 53 — https://arxiv.org/abs/1809.02514
- Karjalainen, van Leeuwaarden & Leskelä 2018, Parameter estimators of sparse random intersection graphs with thinned communities — https://arxiv.org/abs/1802.01171
- Bloznelis & Leskelä 2023, Clustering and percolation on superpositions of Bernoulli random graphs, *RSA* 63(2) — https://arxiv.org/abs/1912.13404
- Bloznelis & Karoński 2013, Random intersection graph process — https://arxiv.org/abs/1301.5579

Formation, homophily, geography
- Leskovec, Backstrom, Kumar & Tomkins 2008, Microscopic evolution of social networks, KDD — https://cs.stanford.edu/people/jure/pubs/microEvol-kdd08.pdf
- Kossinets & Watts 2006, Empirical analysis of an evolving social network, *Science* 311 — https://doi.org/10.1126/science.1116869 (abstract via Europe PMC)
- Cox 2021, The State of American Friendship, American Perspectives Survey — https://www.americansurveycenter.org/research/the-state-of-american-friendship-change-challenges-and-loss/
- Hall 2019, How many hours does it take to make a friend?, *J. Soc. Pers. Relat.* — https://news.ku.edu/2018/03/06/study-reveals-number-hours-it-takes-make-friend (DOI 10.1177/0265407518761225)
- Mossong et al. 2008, Social contacts and mixing patterns (POLYMOD), *PLoS Med* 5 — https://pmc.ncbi.nlm.nih.gov/articles/PMC2270306/
- Ugander, Karrer, Backstrom & Marlow 2011, The Anatomy of the Facebook Social Graph — https://arxiv.org/abs/1111.4503
- Liben-Nowell, Novak, Kumar, Raghavan & Tomkins 2005, Geographic routing in social networks, *PNAS* 102 — https://pmc.ncbi.nlm.nih.gov/articles/PMC1187977/
- Bailey, Cao, Kuchler, Stroebel & Wong 2017/2018, Social Connectedness: Measurement, Determinants, and Effects, NBER w23608 / *JEP* 32(3) — https://www.nber.org/papers/w23608
- Wrzus, Hänel, Wagner & Neyer 2013, Social network changes and life events across the life span, *Psych. Bull.* — https://doi.org/10.1037/a0028601

Ego networks
- Mac Carron, Kaski & Dunbar 2016, Calling Dunbar's numbers, *Social Networks* 47 — https://arxiv.org/abs/1604.02400
- Dunbar 2016, Do online social media cut through the constraints…, *R. Soc. Open Sci.* 3:150292 — https://pmc.ncbi.nlm.nih.gov/articles/PMC4736918/
- Hill & Dunbar 2003, Social network size in humans, *Human Nature* 14 — https://doi.org/10.1007/s12110-003-1016-y
- Dunbar & Spoors 1995, Social networks, support cliques, and kinship, *Human Nature* 6 — https://doi.org/10.1007/BF02734142
- Zhou, Sornette, Hill & Dunbar 2005, Discrete hierarchical organization of social group sizes, *Proc. R. Soc. B* — https://pmc.ncbi.nlm.nih.gov/articles/PMC1634986/
- Sutcliffe, Dunbar, Binder & Arrow 2012, Relationships and the social brain, *Br. J. Psychol.* — https://doi.org/10.1111/j.2044-8295.2011.02061.x
- Bhattacharya, Ghosh, Monsivais, Dunbar & Kaski 2016, Sex differences in social focus across the life cycle, *R. Soc. Open Sci.* 3:160097 — https://pmc.ncbi.nlm.nih.gov/articles/PMC4852646/
- McCormick, Salganik & Zheng 2010, How many people do you know?, *JASA* 105 — https://pmc.ncbi.nlm.nih.gov/articles/PMC3666355/
- Pollet, Roberts & Dunbar 2011, Extraverts have larger social network layers…, *J. Individual Differences* 32 — https://doi.org/10.1027/1614-0001/a000048 (**not verified here**)

Small world, long-range ties
- Kleinberg 2000, The small-world phenomenon: an algorithmic perspective, STOC — https://www.cs.cornell.edu/home/kleinber/swn.pdf
- Watts, Dodds & Newman 2002, Identity and search in social networks, *Science* 296 — https://arxiv.org/abs/cond-mat/0205383
- Dodds, Muhamad & Watts 2003, An experimental study of search in global social networks, *Science* 301 — https://doi.org/10.1126/science.1081058
- Backstrom, Boldi, Rosa, Ugander & Vigna 2012, Four degrees of separation — https://arxiv.org/abs/1111.4570
- Bringmann, Keusch & Lengler, Geometric inhomogeneous random graphs / sampling GIRGs in linear time — https://arxiv.org/abs/1511.00576
- Kumpula, Onnela, Saramäki, Kaski & Kertész 2007, Emergence of communities in weighted networks, *PRL* 99 — https://arxiv.org/abs/0708.0925

Sampling and validation
- Seshadhri, Pinar & Kolda 2013, Triadic measures on graphs: the power of wedge sampling, SDM — https://www.cs.cornell.edu/courses/cs6241/2019sp/readings/Sesh-2013-wedges.pdf
- Gjoka, Kurant, Butts & Markopoulou 2010, Walking in Facebook: uniform sampling of users in OSNs, INFOCOM — https://arxiv.org/abs/0906.0060
- Leskovec & Faloutsos 2006, Sampling from large graphs, KDD — https://cs.stanford.edu/people/jure/pubs/sampling-kdd06.pdf
