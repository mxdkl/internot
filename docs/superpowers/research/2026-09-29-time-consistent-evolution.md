# Time-consistent evolution: memberships, ties and events as pure functions of (id, t)

**Date:** 2026-09-29
**Status:** Research
**Builds on:** `2026-05-14-temporal-networks.md` (tie-strength model, time-rescaling, burstiness) and `2026-05-14-communication-patterns.md` (per-dyad rates, mode mix). Those notes are not repeated here. §0 corrects three claims in them.
**Question:** how to let people be born, die, move house, change jobs, partner, and make and lose friends, so that (a) every fact is `f(id, t, seed)`, (b) the person-side and group-side views agree at every `t`, and (c) any `t` in 1800–2300 is answerable directly and cheaply. The team's proposal is keyed bijections as coordinate systems, evolved by sparse per-epoch transpositions; it is assessed in §3.4.

---

## Summary: what is directly usable

1. **Key every random draw on an absolute coordinate, never on the query window.** The coordinate is an absolute time bucket, a dyadic tree node, or an ordinal counted from a fixed anchor such as a birth date. This alone fixes the `enumerate_events` recombination defect (§2.1).
2. **Poisson and Cox streams → absolute-time buckets** (§2.2). A Poisson count per bucket plus in-bucket inverse-CDF times gives exact, bitwise window recombination, because Poisson counts on disjoint sets are independent.
3. **Sparse streams over centuries → a dyadic count tree with binomial splitting** (§2.3). "Count before t" costs 18 draws for daily resolution over 1800–2300, about 1 µs at the measured 30–50 ns per draw.
4. **Life-course processes → hazard inversion walked from the entity's anchor** (§2.4). This covers jobs, moves, unions, births and death. Each has ≤ ~30 events per life, so a walk costs a few µs, and it supports any tenure law and any dependence on age or history.
5. **Markov on/off states → the regeneration lemma** (§1.1). This covers edge-Markovian, DAR(1) and "stay-or-redraw" labels. The state at `t` is the fresh draw at the last regeneration before `t`, so the expected cost is `1/ρ` steps, or O(1) buckets in continuous time.
6. **Two-sided memberships → static keyed bijections with dynamic indices** (§3.3). Person `x` in spell `j` sits at `π_{k_j}(x) = (group, slot)`. The spell boundaries and the index `k_j` come from `x`'s own hazard walk. `roster(G, t)` enumerates the `K` preimages of each slot and keeps those whose active index matches. Consistency is exact by construction, and tenure laws and age effects are unrestricted.
7. **Per-epoch transpositions are a slow swap-or-not shuffle.** They are consistent but naive: each query composes O(#epochs) rounds, the swap decision cannot depend on who is being swapped (age, tenure) without a backward light-cone blow-up, tenure is geometric, flows are forced to be reciprocal, and group sizes and population are frozen. Use permutations only for rare, group-scoped bulk events such as a team reorg (§3.4).
8. **Joint events (unions, births, household moves, reorgs) are one object keyed on the joint entity**, and participants read it through membership (§4). Pairwise exclusivity comes from attempt-indexed fixed-point-free involutions `μ_j`, so the recursion is bounded by 2^J with J ≤ 4 attempts.
9. **Ties decay as a power law, not exponentially** (§5.3). Burt (2000) gives survival `(T+1)^γ` with γ = −0.716 for non-kin, −0.466 for kin and −1.842 for work colleagues. That is a Lomax lifetime with closed-form inverse CDF `T = U^{1/γ} − 1`, and hazards fall with tie age, as they do for jobs (Farber 1994; NLSY79).
10. **Calibration targets** are tabulated in §7, with sources, and the construction and property tests are in §8.

---

## 0. Current state and corrections to earlier notes

**Code facts that the design must replace** (read from the tree on 2026-09-29):
- **The event index resets per window.** `procedural_core::graph::enumerate_events` numbers its gaps `k` from `t_start`. It also evaluates `λ` at `t_start + 24h·day + 12h`, so the daily grid is window-relative too. There are therefore two sources of non-recombination, not one.
- **Measured cost:** 518 µs per call for a 100-year window at λ = 0.01/day (363 events). Nearly all of it is the 36,525 daily `λ` evaluations. `hash_float` costs 29 ns with a static key and 52 ns with a `format!`-built key, measured on this machine against `procedural_core` in release mode.
- **Workplace rosters ignore career changes.** Career events re-seed `employer_seed` (`people/career.rs`), but rosters come from the static `workplace_seed` bits (`people/cohort.rs::workplace_members_of`), so a person who switches jobs never appears on the new employer's roster.
- **Spouse links are one-way** (`people/family.rs`). `reciprocal_spouse_of` filters them after the fact.
- **The population never turns over.** Birth dates are `DEFAULT_NOW − age` (`people/slot/temporal.rs`), nobody is born or dies, and `current_city_of_at` ignores `t` (coordinator note).
- **`graph::VenueSpace`** already has the right shape: a membership is one tuple `(member, venue, role, window)` and both views query it. But enumerating a venue leaves the 32 member bits free, so it only works when the tuple space is dense and small. §3.3 is the way to make it dense.

**Corrections to the 2026-05-14 notes**, from sources verified for this survey:
- **Burt (2000).** The notes say Burt reports "close friends ~7 years". He does not.
  - His data are 345 bankers over 4 annual waves. Only 24.7% of 12,655 colleague relations were re-cited after 1 year, 10.1% after 2 years and 8.0% after 3 years.
  - His pooled decay function is a power law, `Y = (T+1)^(γ + κ·KIN + λ·WORK)`.
  - The "7 years" figure probably comes from Mollenhorst et al.'s 7-year Dutch panel (§5.3).
- **Roberts & Dunbar.** The notes give a friend half-life of 6–12 months. That is not stated in any abstract we could verify. The verified numbers for the same cohort (Roberts & Dunbar 2015) are:
  - 48.6% of inner-layer friends were still inner after 18 months, against 70.3% of kin.
- **"Window-monotone" / "stable under slicing".** Both notes claim this. It does not hold for the implemented code (the `#[ignore]`d test). It holds only after the fix in §2.

---

## 1. Evolving-graph formalisms: what is locally computable?

"Locally computable" here means that the state of one edge or one membership at `t` is available without simulating other entities, and ideally without simulating from time zero.

### 1.1 The regeneration lemma (the key tool)

Suppose a discrete-time Markov kernel has "hold-or-redraw" form, `P(s, ·) = (1−ρ)·δ_s + ρ·ν`. Then:

```
X_t = Y_{τ(t)},   τ(t) = last step ≤ t at which Bernoulli(ρ) keyed H(key,"regen",step) fired,
                  Y_s ~ ν keyed H(key,"draw",s);   X_t = X_{t0} if no regeneration in (t0, t].
```

- **Cost.** The expected backward scan is `1/ρ` steps. In continuous time the regenerations are a Poisson(ρ) stream, so with Recipe A and buckets of width ≈ `1/ρ` the scan touches O(1) buckets.
- **General kernels.** Any kernel with a minorisation `P(s,·) ≥ ρ·ν` splits as `ρ·ν + (1−ρ)·R`. You then simulate forward with the residual `R` from the last regeneration. This is the standard Markov-chain splitting construction; applying it to hash-keyed lookup is our inference.
- **Edge-Markovian graphs.** Birth probability `p` and death probability `q` give `P = (1−p−q)·I + (p+q)·1·π^T` with `π_on = p/(p+q)`, which is exactly hold-or-redraw with `ρ = p+q` (derivation: from on, `1−p−q + (p+q)·p/(p+q) = 1−q`; from off, `p`).
  - The marginal is `P(on at t | on at 0) = π_on + π_off·(1−p−q)^t`.
- **DAR(1) networks.** `A_ij(t) = V_ij(t)·A_ij(t−1) + (1−V_ij(t))·Y_ij(t)` with `V ~ B(α_ij)`, `Y ~ B(χ_ij)` (Mazzarisi et al. 2020, eq. 1.2) is the same form with `ρ = 1−α_ij`.

### 1.2 Model-by-model

| Model | One edge / one membership at t | Neighbours / roster at t | Verdict for Internot |
|---|---|---|---|
| **Edge-Markovian** (Clementi, Macci, Monti, Pasquale, Silvestri, PODC 2008; SIAM J. Discrete Math. 24(4) 2010): each edge is an independent 2-state chain, birth `p`, death `q` | Regeneration lookback, E = `1/(p+q)` steps. Sparse graphs have `p ≪ q`, so the lookback is about the mean edge lifetime | Every pair is a candidate, so O(n). Viable only on a candidate set (venue co-members, latent-space cells) | Usable per candidate pair. On-durations are geometric (memoryless), which contradicts the liability of newness (§5.3) |
| **DAR(1) / link persistence** (Mazzarisi, Barucca, Lillo, Tantari, EJOR 281, 2020) | Same, `ρ = 1−α` | Same | Same |
| **Activity-driven** (Perra et al., Sci. Rep. 2:469, 2012): `a_i = η·x_i`, `x ~ F`. Each step node `i` activates w.p. `a_i·Δt` and links to `m` random nodes; links vanish at the next step. Integrated degree `P_T(k) ~ F(k/(mηT))` | Out-links: O(m) per activation. In-links with hashed uniform targets: O(n). **Fix (inferred):** make target `r` of `j` in bucket `b` equal `π_{b,r}(j)` for a keyed bijection. Then the in-neighbours of `i` are `π_{b,r}^{-1}(i)` filtered by "active in b", giving O(m) per bucket | Same trick | Good for instantaneous contacts. No memory: ties are not persistent |
| **Activity-driven with memory** (Karsai, Perra, Vespignani, Sci. Rep. 4:4001, 2014): the probability that the next contact is a new tie falls with the number `n` of ties already contacted, `p(n) = c/(n+c)` (as recalled; full text not fetched) | Depends on the whole contact history, so you must walk every activation since birth: O(lifetime activations) | Same | Not local. Emulate the strong/weak-tie split with tie objects (§8.6) |
| **Dynamic SBM** (Xu & Hero, IEEE JSTSP 8, 2014; Matias & Miele, JRSS-B 79, 2017): node labels follow independent Markov chains; edges are independent given labels | Label: regeneration if the kernel is "stay w.p. η or redraw", else an O(t) walk. Edge: O(1) given the two labels | Block roster: O(n) scan | Label side fine; roster side needs §3.3 |
| **Stochastic block transition model** (Xu, AISTATS 2015) | Adds edge persistence given the previous state, so a per-edge lookback | Same | Same |
| **STERGM** (Krivitsky & Handcock, arXiv 1011.1937; JRSS-B 2014): separate models for formation and dissolution | With dyad-independent terms only, it reduces to per-dyad edge-Markovian or semi-Markov (inferred) | Structural terms (triangles, degree) need global MCMC | Only the dyad-independent core is usable |
| **Link memory** (Vestergaard, Génois, Barrat, PRE 90:042805, 2014): long-term memory in link creation and deletion | Duration-dependent hazards make the process semi-Markov, so use the anchor walk (§2.4) | Candidate-set | Use §2.4 |
| **Alternating renewal on/off**: sojourn laws `F_on`, `F_off`, means `μ_on`, `μ_off` | P(on) → `μ_on/(μ_on+μ_off)`. Stationary start: on w.p. `μ_on/(μ_on+μ_off)`, residual sojourn density `(1−F(x))/μ`. Locating `t` needs a walk over the cycles since the anchor | Candidate-set | Anchor at tie birth, which is cheap because there are few cycles per tie |

### 1.3 Takeaways

- **Independent per-edge processes make edge state easy and neighbour lists impossible.** Every usable design restricts candidates first: venue co-membership, a coordinate preimage, or a latent cell.
- **Regeneration makes Markov state O(1).** Realism wants non-exponential sojourns, and those push you to anchor walks, which are cheap because life-course event counts are small.
- **Rosters are the hard direction** in every model above (§3).

---

## 2. Deterministic, window-independent event streams

### 2.1 The rule

A stream is a pure function `events(key, window)`. Recombination (`events(k,[a,b)) ⊎ events(k,[b,c)) = events(k,[a,c))`, bitwise) holds if and only if every draw is keyed on coordinates fixed before any query. There are three kinds:

1. absolute bucket indices,
2. dyadic tree nodes `(level, index)`,
3. ordinals counted from a fixed anchor (birth, tie start, household formation).

This is the counter-based RNG pattern, `u = H(seed, stream, counter)` (Salmon, Moraes, Dror, Shaw, SC11 — Philox/Threefry). `procedural_core::hash` is already such a generator (xxh3). What is missing is choosing counters that are absolute.

### 2.2 Recipe A: absolute buckets (Poisson and Cox processes)

```
EPOCH = 1800-01-01, Δ = 1 day (or ≈ 1/λ̄ for sparse streams)
Λ_d   = ∫_{EPOCH+dΔ}^{EPOCH+(d+1)Δ} λ(s) ds            // λ evaluated in absolute time
N_d   = PoissonInv(Λ_d, H(key,"N",d))
t_d,j = F_d^{-1}(V_(j)),  V_(j) sorted uniforms via exponential spacings keyed H(key,"e",d,j)
events(key,[T1,T2)) = ⋃_{d ∩ [T1,T2) ≠ ∅} { t_d,j } ∩ [T1,T2)
```

- **Why it is exact.** Poisson counts on disjoint sets are independent, and given `N_d` the points are iid with density `λ/Λ_d` (Devroye 1986, ch. VI). Exponential spacings give sorted uniforms without a sort (Devroye ch. V). Each bucket's content depends only on `(key, d)`, so recombination is bitwise.
- **Cox processes stay exact.** If `λ` is itself a keyed random envelope (the `burst_state` factor in the earlier notes), the result is a Cox process and remains exact as long as the envelope is keyed on absolute time.
- **Cost:** O(#buckets + #events).
- **Precedent:** this is the same trick as per-cell feature points in Worley noise (Worley 1996): a variable number of points per grid cell, seeded by the cell's coordinates.

**Fix for `enumerate_events`:**
- Replace the gap index `k` with `(absolute day d, j)`.
- Place events within a day by inverting the within-day cumulative profile (circadian × weekly).
- Clip at the window edges.
- Un-ignore `event_enumeration_slice_recombinability`.

The cost stays O(days + events), but the result is now window-independent.

### 2.3 Recipe B: dyadic count tree (sparse streams, "count before t")

```
root  = [EPOCH, EPOCH + 2^L·Δ),  L = 18  (262,144 days ≈ 718 y ⊇ 1800–2300)
N(root)                    = PoissonInv(Λ(root), H(key,"N",L,0))
N(left) | N(node) = n     ~ Binomial(n, Λ(left)/Λ(node)),   keyed H(key,"B",ℓ,i)
N(right)                   = n − N(left)
```

- **Validity.** Given `N(A∪B) = n`, `N(A) ~ Bin(n, Λ(A)/Λ(A∪B))`, which is the conditional property of the Poisson process (Devroye ch. VI).
- **Count before t:** one root-to-leaf path, `L` binomial draws.
- **Window enumeration:** descend only into nodes that overlap the window and have `N > 0`, for O(L + k·L).
- **Not interchangeable with Recipe A.** The law is the same but the output is not bitwise-identical, so choose one recipe per stream and freeze it.
- **Brownian analogue.** The virtual Brownian tree (Li, Wong, Chen, Duvenaud, AISTATS 2020, §4) evaluates a Wiener path at any `t` by bisection with Brownian-bridge sampling. Its RNG keys derive from the path, so it has O(1) memory and time logarithmic in the tolerance. The same pattern gives smooth latent trajectories, such as career satisfaction, that are exact at any `t`.
- **Renewal with gamma gaps (inferred application).** `S_n ~ Gamma(n·k)` and `S_{n/2}/S_n ~ Beta(nk/2, nk/2)`, independent of `S_n` (Devroye ch. IX), so the k-th event time costs O(log n) draws.

### 2.4 Recipe C: hazard inversion walked from an anchor (life-course processes)

```
t_0 = anchor(entity, stream)                  // birth, union start, household formation, tie start
for k = 1, 2, ...
    E_k = −ln H(key, "E", k)
    t_k = inf{ t > t_{k−1} : ∫_{t_{k−1}}^{t} h(s | past ≤ t_{k−1}, age(s), s − t_{k−1}, cov(s)) ds ≥ E_k }
    mark_k = draws keyed H(key, "mark", k, ·)  // destination index, reason, ...
```

- **Exactness.** This is exact for any conditional intensity by time-rescaling (see the 05-14 note §8). The anchor never moves, so recombination and `t`-independence are automatic.
- **Unrestricted hazards.** `h` may be non-exponential in duration (Lomax, lognormal, Weibull), piecewise in age (Rogers–Castro, §5.1), and calendar-dependent (a recession multiplier).
- **Cost:** O(#events + #hazard pieces crossed). With annual age pieces and ≤ 30 events per life, that is ≤ ~150 draws, about 5 µs.

**Dependency rules** (needed for termination and cross-view consistency):
- **(i)** `cov(s)` may read only the entity's own earlier events, other streams' events **strictly** before `s`, and joint objects (§4). It must never read another person's state unless that person is reachable through a joint object.
- **(ii)** A triggered event, such as "job change with relocation → household move", is keyed on the triggering event (`H(job_event_key, "reloc")`). It is never sampled independently by the other stream.
- **(iii)** Streams form a DAG at each instant. Tie-break simultaneous events by a fixed stream order.

### 2.5 Recipe D: Hawkes via its cluster representation

Hawkes & Oakes (1974) show that a Hawkes process with baseline `μ(t)` and kernel `φ` (branching ratio `n = ∫φ < 1`) is a Poisson cluster process. Møller & Rasmussen (2005) use this for simulation. The recipe:

- **Immigrants** are a Poisson(`μ`) stream (Recipe A/B).
- **Offspring.** Each event has Poisson(`n`) children with iid delays of density `φ/n`.
- **Keys.** Every offspring is keyed on `(immigrant bucket d, immigrant index j, child-index path)`, which is the same path-derived keying as the virtual Brownian tree.
- **Window `[T1,T2)`.** Enumerate immigrants in `[T1 − H, T2)` and keep the descendants that land in the window.
- **Recombination** is exact, because clusters are keyed by immigrant, not by window.
- **Truncation error.** Truncating at `G` generations misses `n^{G+1}/(1−n)` expected events per immigrant (derived). For a kernel with support ≤ `H_φ`, a look-back of `H = G·H_φ` is exact up to that tail.
- **Cost:** O(immigrants in `[T1−H, T2)` × `1/(1−n)`).
- **When to use it.** Only if the measured `B`/`M` burstiness statistics fall outside their bands (the 05-14 note §6 already prefers a Cox envelope first).

### 2.6 Cost at 100 years, daily resolution

Using measured draw costs of 29–52 ns:

| Query | Recipe | Draws | Time |
|---|---|---|---|
| Messages on a 5/day dyad, 1-month window | A, daily | 30 buckets + ~150 events × 2 | ~15 µs |
| "Job changes before t" for one person | C | ≤ ~30 events + ≤ ~100 age pieces | 3–6 µs |
| "Count before t", sparse Poisson over 1800–2300 | B | 18 binomials | ~1 µs |
| Same, daily buckets | A | ~66k buckets to 1980 | ~2–3 ms |
| Current `enumerate_events`, 100 y, λ = 0.01/day | (v1) | 36,525 daily λ evaluations | 518 µs (measured) |
| Edge on/off at t (Markov) | §1.1 | O(1) buckets | < 1 µs |

---

## 3. Two-sided consistency for memberships

### 3.1 The object

A membership is one object, `m = (x, G, [a, b), role)`. The two views are projections of the same set of objects:

- `timeline(x) = { m : m.x = x }`
- `roster(G, t) = { m.x : m.G = G, a ≤ t < b }`

Any design in which the person side and the group side sample their own facts fails. The question is only which side drives the timing, and how the other side enumerates.

### 3.2 Formulations compared

| Formulation | Consistency | Tenure law | Moves depend on age/tenure? | Group size | Person query | Roster query |
|---|---|---|---|---|---|---|
| (a) Person-driven spells + roster = filter over a candidate pool | exact (by definition) | any | yes | emergent | O(E) | O(\|Pool\|·E): infeasible unless the pool is small and enumerable |
| (b) Seat-driven: each seat is an alternating renewal of occupant/vacant spells | exact only if the occupant is picked from timelines that are already consistent; otherwise needs a global matching | any per seat | only through the pool | fixed seats | needs the inverse of the seat's choice | O(seats·E) |
| (c) **Keyed coordinate systems + person-driven spells** (§3.3) | exact | any | yes | emergent, thinned by capacity | O(E) | O(K·S_G·E) |
| (d) Per-epoch transpositions (§3.4) | exact | geometric only | no | frozen | O(#epochs) | O(S_G·#epochs) |
| (d′) Event-driven interchange (Poisson clock per position pair) | exact | exponential only | no | frozen | O(#swaps touching the path) | same |
| (e) Vacancy chains / matching markets (White 1970; Chase 1991) | exact after a global simulation | any | yes | any | global | global |

(b) remains useful for **single, exclusive seats whose pool is already consistent**. For example, "manager of team T" is a seat whose occupancy spells pick from `roster(T, t_start)`. It is a derived seat and two-sided for free.

### 3.3 Recommended: static bijections, dynamic indices

**Definition.**
- Fix `K` keyed bijections per membership type and block, `π_k : block → coords(block)`, with `coords = ⋃_G {G}×[0, S_G)`. Build them from a 4–6-round Feistel with cycle-walking, or from swap-or-not over `Z_N` (Hoang, Morris, Rogaway 2012), which handles any domain size directly.
- Person `x` has spells `j = 0, 1, …` from Recipe C. Each spell has a candidate index sequence `k_{j,0}, k_{j,1}, …`, keyed `H(x, "idx", j, r)`.
- The **effective** index `k*(x, t)` is the first candidate whose coordinate lands in a live group with spare capacity at spell start: `slot < cap_G(t_{j,start})` and `alive_G(t_{j,start})`.
- `G(x, t) = grp(π_{k*(x,t)}(x))` while `x` is eligible, i.e. alive, in the right age band and not retired.

```rust
fn group_at(x, t) -> Option<GroupId> {
    if !eligible(x, t) { return None }                     // alive, age band, not retired
    let (j, start) = spell_at(x, t);                       // Recipe C walk from anchor
    let k = effective_index(x, j, start);                  // candidate list, rejection on cap/alive
    let (g, _slot) = PI[k].forward(x);
    if t >= group_death(g) { return None }                 // spell truncated; next spell starts later
    Some(g)
}

fn roster(g, t) -> Vec<PersonId> {
    let mut out = vec![];
    for k in 0..K { for s in 0..cap_at(g, t) {
        let x = PI[k].inverse((g, s));
        if eligible(x, t) && effective_index_at(x, t) == Some(k) && group_at(x, t) == Some(g) { out.push(x) }
    }}
    out
}
```

**Consistency proof (one line each way).**
- (⇒) If `x ∈ roster(G,t)`, then there are `k` and `c ∈ coords(G)` with `x = π_k^{-1}(c)` and `k*(x,t) = k`. So `π_{k*}(x) = c` and `G(x,t) = G`.
- (⇐) If `G(x,t) = G`, then `c = π_{k*(x,t)}(x) ∈ coords(G)`, and the loop visits `(k*(x,t), c)` and keeps `x`.
- `x` appears exactly once, because only `k = k*(x,t)` passes the filter.
- `cap_G(t)`, `alive_G` and `group_death` are functions of `(G, t)` only, so there is no circularity.

**Realism.**
- **Tenure and age.** Any tenure law and any age dependence live in `x`'s hazard (Recipe C). Examples: job exit hazard peaking at 3 months (Farber 1994), falling with age at start (NLSY79, §5.2), retirement at a person-level age.
- **Choice of next employer.** The next index can be weighted by group attributes (growth, size, industry), which are public functions of `(G, t)`. The roster filter recomputes the same choice.
- **Headcount.** Expected headcount is `E|roster(G,t)| ≈ cap_G(t)·P(eligible)` for a uniform index choice (inferred). It fluctuates binomially.
- **Group growth and death.** Growth, decline and firm death come from `cap_G(t)` and `alive_G`, applied by rejection at spell start and truncation at death.
- **Boomerangs.** With `K` small (e.g. 8), each person has a local market of `K` candidate groups per block, and returning to a former group happens naturally.

**Cost.**
- Person query: O(E), about 5–10 µs including residence and eligibility (inferred from the measured draw cost).
- Roster query: `K·cap` person evaluations. With `K = 8`, a 64-slot team costs ~512 evaluations, ~4 ms; a 1,000-slot establishment costs ~60 ms.
- Serve large rosters with a budget (the `scan_budget` pattern) and memoise per-request (`x`, stream) event lists.

**Aging and memberships.**
- Eligibility is person-side: age ≥ 16/18, age < `retire_age(x)`, alive.
- **Schools need no bijection.** They are derived: `school(x,t) = catchment(residence(x,t))[grade(age(x,t))]`, and the class roster is residents of the catchment with the matching birth cohort. Aging out at 18 and moving house both fall out of the definitions.
- **Retirement** is the final job spell's end, drawn from a person-level retirement-age distribution. Rosters drop `x` automatically.

### 3.4 Per-epoch transpositions: what they are, and where they are naive

**What the proposal is, formally.** At epoch `e`, pair each position `p` with `p' = p ⊕ K_e` (a keyed involution) and swap iff `F_e(max(p, p')) = 1`. This is exactly one round of the **swap-or-not** shuffle (Hoang, Morris, Rogaway, CRYPTO 2012). There, `X' = K_i ⊕ X`, `X̂ = max(X, X')`, and `X ← X'` iff `F_i(X̂) = 1`; the inverse runs the rounds in reverse. Full cryptographic mixing needs ~`6·lg N` rounds at swap probability ½. With a low swap probability, the composition is a lazy random-transposition walk.

The continuous-time version is the **interchange process**: each edge `(i, j)` rings at a rate and swaps the two occupants. Watching one particle gives a continuous-time random walk (Aldous; Caputo, Liggett, Richthammer 2010 proved the two spectral gaps are equal).

**What it gets right.** A bijection at every epoch gives exact two-sided consistency. This is the right insight, and §3.3 keeps it.

**Where it is naive:**

1. **Query cost grows with time.** The image of `x` after `e` epochs requires tracking `x` through every round, because its future partners depend on its current position. There is no closed form for sparse compositions (inferred).
   - Daily epochs over 100 years mean 36,525 rounds ≈ 1–2 ms per lookup and ≈ 70–130 ms per 64-slot roster.
   - This is fixable by moving to (d′), per-pair Poisson clocks on absolute buckets, which cuts the cost to O(swaps touching the path). The other problems below remain.
2. **Swaps are blind to occupants.** `F_e` may depend only on positions and time.
   - To make a swap depend on *who* is swapped (age, tenure, retirement, marital status), you must know the other position's occupant at `e`. That requires tracing its history backwards, and each earlier swap there depends on its own occupants.
   - The dependency cone grows geometrically with the number of past swaps in the light cone, which is effectively a global simulation (inferred).
   - Mobility is dominated by age: the CPS mover rate runs from 18.9% at 25–29 to 2.8% at 65+ (§5.1). So this is disqualifying for individual mobility.
3. **Tenure is geometric.** Swap probability `r` per epoch makes tenure memoryless. Empirically, the job-ending hazard peaks at 3 months and then declines (Farber 1994), and ties show a liability of newness (Burt 2000).
4. **Flows are forced to be reciprocal.** Every move `x: G→H` is paired with `y: H→G` at the same instant. Real flows are directional and chained: in a vacancy chain the vacancy moves opposite to the units that fill it (White 1970; Chase 1991). Adding "vacancy tokens" turns the transpositions into a vacancy-chain model, with the same sequential cost.
5. **Sizes are frozen.** A bijection conserves the count per coordinate block. That rules out births, deaths, firm entry and exit, and urban growth, unless you add reservoirs of "unborn", "dead" and "vacant" tokens, which inherits problems 1 and 2.
6. **Households cannot move together.** Swapping individuals splits households. Swapping household blocks requires equal household sizes, and unions and divorces change those sizes.
7. **Geometry is accidental.** XOR pairing produces hypercube geometry: the destination depends on bit distance, not on geographic distance. Gravity-like decay (distance exponent ≈ 1, §5.1) needs structured pairings designed for it.

**Where permutations are the right tool.** Use them for rare, group-scoped bulk events. A **team reorganisation** at date `t_r` applies a keyed permutation to the org's internal coordinates. Both sides apply the same permutation, so consistency is exact, and there are a handful of events per org, not one per epoch. The rule to adopt: **static bijections, dynamic indices; permutations only for bulk events.**

### 3.5 Residence, households and migration

**Household is the unit of residence.** The id is `h = (founder x, ordinal j)`, which is dense (32 + 4 bits). A household is founded by one of:
- leaving home (a solo household keyed on `x`),
- a union (founder = canonical low partner),
- a divorce split (each ex-partner founds one).

Children are members of their parents' union household until they leave home. Members are a membership stream per person (Recipe C) with joint-event triggers (§4). `residence(x, t) = residence(household(x, t), t)`.

**Moves are joint events on the household.**
- **Timing.** The move hazard is `h_move(age of head, duration at address, triggers)` from a Rogers–Castro age profile (§5.1), multiplied by triggers keyed on member events: union formation, a birth, or a job change marked as a relocation.
- **Who moves.** Every member of `h` at `t_move` moves together, and all compute the same date from `H(h, "move", m)`.
- **Split-off moves.** Leaving home, divorce and cohabitation are not household moves. They are exits into a new household, whose first event is its move-in.
- **Evidence for whole-household moves.** CPS 2023 has 2.39 movers per moving householder against a mean household size of 2.50, so most moves are whole-household (derived, §5.1). About 22% of movers cite split-off reasons.

**Destinations with enumerable city rosters.**
- **Within-county moves** (54% of movers) change the address and neighbourhood coordinate, not the city. They use a city-local coordinate system keyed on `(household city slot, local move ordinal)`.
- **Inter-county moves** (46%) choose the next city among `I` (e.g. 4) household-level candidate coordinate systems `ψ_i(h) → (city, household slot)`.
- **Roster.** `roster(city C, t)` enumerates `ψ_i^{-1}` of C's slots and filters by `h`'s active city index. This is §3.3 applied to households.
- **Workplaces use the same structure one level down.** The job coordinate of a member is `π_{C,k}(household slot at job-spell start, member ordinal)`. This keeps within-city moves from forcing job changes.

**The realism compromise to measure.**
- Bijections need a dense domain fixed in advance. So candidate destinations can depend on fixed attributes of `h`, such as the founder's native region encoded in id bits, but **not on `h`'s current location**.
- Distance decay is therefore approximated by making the candidate systems hierarchical relative to the founder's native region: `ψ_0` within the native county, `ψ_1` within the native state, `ψ_2`, `ψ_3` national and population-proportional.
- This produces some return migration by construction.
- Validate against the CPS inter-county distance distribution (40.4% under 50 miles; §7).
- **Alternative if origin-dependence matters more.** Use structured many-to-one maps, `dest = F_k(h) mod A_S` within the current state `S`. The preimage of an address is `{F_k^{-1}(a + j·A_S)}`, which costs ~`1/share(S)` candidates per address and ordinal. That is affordable for "who lives at this address" but not for whole-city rosters (inferred).

### 3.6 Birth, death and "alive at t"

- **Lifespan.**
  - `b(x)` comes from id cohort bits plus a hash, so birth cohorts are bit-pushdown enumerable.
  - `d(x) = b(x) + LifeTableInv(cohort, sex, H(x, "death"))`.
  - `alive(x, t) ⇔ b(x) ≤ t < d(x)`.
  - Use a life-table inverse CDF, which captures infant and old-age mortality, rather than pure Gompertz. As a sanity check, the adult hazard doubling time is ≈ 8 years, and a fit to 2024 US survivorship gives `b ≈ 0.082/yr` (§5.4).
- **Id budget.** 2024 US births were 3.63M. Over 200 years that is ≈ 7×10⁸ ids, well inside 2³² (derived), and ≈ 3.3×10⁸ are alive at any `t`.
- **Everything is clipped to `[b(x), d(x))`.** That covers spells, memberships, unions (which end at the first death), ties and events. Rosters drop the dead through `eligible`.
- **Kinship uses the same machinery**, with the child side driving (inferred design; prototype first). The child's birth date comes from its id. The parent union is found by a keyed lookup into the parent cohort. `children(U)` enumerates preimages over parity slots and filters on "the child's lookup resolves to `U` and `U` is active at `b(child)`". Fertility then emerges from how the cohort blocks are sized. Recursion depth is bounded by the generations since the founder cohort (~7 over 1800–2000 at ~28-year generations).

---

## 4. Joint events

**Principles.**
- **J1. One key per joint event.** Participants never sample independently. The key is `(joint object, kind, ordinal)`:
  - union `U = (canonical pair, attempt j)`,
  - household move `(h, m)`,
  - birth `(U, parity)`,
  - reorg `(org, r)`.
- **J2. Participants are determined by membership at the event time.** Every member of `h` at `t_move`; every member of the org at `t_r`. A person who left or died before `t` is not a participant, because the membership filter excludes them.
- **J3. Exclusivity for pairwise events comes from involutions.** Define `μ_j = σ_j^{-1} ∘ (⊕1) ∘ σ_j` over a pool block. The pool is keyed on native region, birth-year band (with band boundaries shifted per `j`) and sex. `σ_j` is a keyed bijection, so `μ_j` is a fixed-point-free involution: each person has exactly one candidate partner per attempt `j`, and that candidate's attempt-`j` candidate is the person.
  - For opposite-sex pools, put the sex bit in the id so pool halves are balanced, and pair index `i` of the women's bijection with index `i` of the men's.
- **J4. Timing is symmetric.**
  - `free_x(j)` = end of `x`'s last successful union before attempt `j`, or age 18.
  - `t_j(x, y) = max(free_x(j), free_y(j)) + gap(H(pair, j))`.
  - The union forms iff both are alive at `t_j`, the age gap is plausible, and `H(pair, j, "accept") < p_accept(ages, j)`.
  - The dissolution date comes from the pair key and the marriage-survival targets (§5.2), and is truncated at the first death.
  - **Cost:** `free_x(j)` recurses into `x`'s attempts before `j` and each partner's attempts before `j`. The tree is bounded by `2^J`, so ≤ 16 union evaluations at `J = 4` (memoised). Calibrate `gap` and `p_accept` to the median first-marriage age, the never-married-at-40 share and the cohabitation share (§7).
  - This is preferred over calendar-round matching, whose availability recursion is unbounded, and over per-bucket stable matching (`graph::stable_roommates_match`), which is an O(n²) global pass per bucket that becomes invalid once pool membership changes with migration.
- **J5. Cascades are keyed, not re-sampled.** A union triggers a new household (founder = low partner), with move-in dated `t_j`. Its members' old households record exits at `t_j`. A birth `(U, k)` adds a member to `U`'s household at `b(child)`. A death ends unions and memberships at `d(x)`. Every derived date is a function of the triggering object's key and date.
- **J6. Group-scoped bulk events.** Reorgs and firm closures are keyed on the group. At a closure, all spells in `G` end at `death_G`. The person side computes `group_death(g)` and ends its spell, so no roster-side action is needed.

**Ties as joint objects.**
- A friendship or acquaintance tie is keyed on `(canonical pair, context id, overlap ordinal)`.
- It can only form during a co-membership overlap `[a, b)`: workplace, school class, neighbourhood or household. Formation probability and delay are keyed on the pair.
- It persists at low hazard during the overlap. After exit, survival is `S(τ) = (1 + τ)^γ_type`, with `τ` measured from `b`.
- Both people compute the same overlap, because memberships are consistent, and so the same tie.
- This matches the evidence:
  - About half of new confidants come from newly entered contexts, and 52.9% of discontinued relationships ended for lack of meeting opportunities (Mollenhorst et al. 2014, as quoted by Small et al. 2015).
  - Among confidant losses, 23.4% were due to moving and distance and 17.1% to death (Cornwell et al. 2014).

---

## 5. Empirical rates

All figures below were checked against the fetched source, except where marked "derived" (our arithmetic on the source's figures).

### 5.1 Residential mobility and migration

- **Moved in the past year, CPS ASEC 2023: 7.8%** (25.6M of 327.2M aged 1+), the series low. Earlier years: 2022 8.7%, 2019 9.8%, 2010 12.5%, 2000 16.1%. From 1950 to 1970 it was 19.1–21.2% every year (Census Table A-1).
  - ACS 2024 is higher by design: 11.8% moved, 8.9% within state, 2.1% from another state.
- **Mover rate by age, CPS 2023** (derived from Table 1-S counts):

  | Age | Rate | Age | Rate |
  |---|---|---|---|
  | 1–4 | 10.7% | 25–29 | 18.9% (peak) |
  | 5–9 | 7.9% | 30–34 | 11.4% |
  | 10–14 | 5.9% | 35–44 | 7.5% |
  | 15–17 | 5.4% | 45–54 | 5.4% |
  | 18–24 | 15.7% (20–24: 18.1%) | 55–64 | 4.6% |
  | | | 65+ | 2.8% (85+: 2.2%) |

- **Lifetime moves ≈ 6 at 2023 age rates** (derived by summing the rates to age 84). CPS counts at most one move per year, so this is a floor. At 1950–1970 rates the figure is roughly 2.5× higher.
- **Type of move, share of movers, CPS 2023:** same county 54.1%, different county in the same state 23.4%, different state 17.5%, from abroad 5.0%. The 2019 split was 60.0 / 21.2 / 15.1 / 3.6.
- **Main reason for moving, CPS 2023 (Table A-5):**
  - Housing 41.3%.
  - Family 22.7%, of which "establish own household" 9.7% and "change in marital status" 5.1%.
  - Job 21.0%, of which "new job or transfer" 12.5% and "retired" 1.3%.
  - Other 15.0%, of which "unmarried partner" 4.5% and "college" 2.8%.
- **Distance of moves between counties, CPS 2020–21:** under 50 miles 40.4%, 50–199 miles 22.7%, 200–499 miles 12.8%, 500+ miles 24.1%. The under-50-mile share has been stable at about 38–42% since 2006 (Table A-6).
  - No national median move distance was found. Local cohort studies report ~4 miles.
- **Distance decay after controlling for where destinations are:** the pull of a destination falls as `d^−s`, with `s = 0.98` for Denmark (39.3M moves, 1986–2020, holding from 10 m to 500 km). Other fits: France 1.07, San Francisco 0.94, Houston 0.95, Singapore 0.89 (Boucherie, Maier, Lehmann, arXiv 2405.08746).
- **Gravity model:** `M_ij = G·P_i^α·P_j^β / D_ij^γ`. The distance exponent is "generally less than 2": UK γ ≈ 1.5–1.6, New Zealand γ ≈ 0.8–0.9 (Poot et al. 2016).
- **Radiation model** (Simini, González, Maritan, Barabási, Nature 484, 2012): `T_ij = T_i · m_i·n_j / ((m_i + s_ij)(m_i + n_j + s_ij))`, where `s_ij` is the population within radius `r_ij` of `i`, excluding `i` and `j`.
- **Rogers–Castro model migration schedule** (IIASA RR-81-30, 500+ schedules): the migration rate at age `x` is
  `M(x) = a1·e^{−α1·x} + a2·exp{−α2(x−μ2) − e^{−λ2(x−μ2)}} + a3·exp{−α3(x−μ3) − e^{−λ3(x−μ3)}} + c`.
  - Sweden 1974 values: μ2 ≈ 19–21.5, α2 ≈ 0.10–0.13, λ2 ≈ 0.38–0.53, retirement peak μ3 ≈ 72–75.
  - Use this as the age factor of `h_move`, fitted to the CPS age table above.

### 5.2 Jobs and partnering

- **Jobs held, NLSY79** (born 1957–64; BLS release of 2025-08-26):
  - 12.9 jobs from ages 18 to 58.
  - By age band: 18–24: 5.6; 25–34: 4.5; 35–44: 2.9; 45–54: 2.2; 55–58: 1.3.
  - Jobs started at 18–24: 61% ended within 1 year, 87% within 5 years.
  - Jobs started at 45–54: 21% ended within 1 year, 56% within 5 years.
  - Derived hazards for starts at 18–24: ≈ 0.94/yr in the first year, ≈ 0.28/yr over years 1–5. For starts at 45–54: ≈ 0.24/yr, then ≈ 0.15/yr. The hazard falls with tenure and with age at start.
- **Jobs held, NLSY97** (born 1980–84): 9.4 jobs from ages 18 to 38.
- **Job-ending hazard by month:** it rises to a maximum at 3 months of tenure and declines thereafter (Farber, NBER w4262, 1994).
- **Median tenure with current employer, January 2026:** 4.1 years.
  - By age: 20–24: 1.5; 25–34: 3.0; 35–44: 4.7; 45–54: 7.0; 55–64: 9.6; 65+: 9.9.
  - 20.6% had a year or less with their employer.
- **JOLTS 2025, average monthly rates:** hires 3.3%, separations 3.3%, quits 2.0%, layoffs 1.1%.
- **Median age at first marriage, 2025:** men 30.8, women 28.4 (Census MS-2).
- **Never married at 40:** 25% in 2021, against 6% in 1980 (Pew).
- **NSFG 2006–10 (NHSR 49):**
  - Women's probability of a first marriage by 25: 44%; by 35: 78%; by 40: 84%.
  - First marriage still intact, women: 0.80 at 5 years, 0.68 at 10, 0.60 at 15, 0.52 at 20. Men: 0.70 at 10 years, 0.56 at 20.
  - Derived annual hazard for women: 0.045 (years 0–5), 0.033 (5–10), 0.025 (10–15), 0.029 (15–20).
- **Cohabitation (NHSR 64):** 48% of women's first unions were cohabitations. A first premarital cohabitation lasts a median of 22 months. After 3 years: 40% had married, 32% were still cohabiting and 27% had split up.
- **Durations (SIPP 2009, P70-125):** a first marriage that ends in divorce lasts a median of 8.0 years. Separation to divorce takes 0.8–0.9 years, and divorce to remarriage a median of 3.7–3.8 years.
- **Crude rates, 2023 (provisional):** divorce 2.4 per 1,000, marriage 6.1 per 1,000.

### 5.3 Ties: persistence and decay

- **Burt (2000), Social Networks 22** (345 bankers, 4 annual waves):
  - Of the initial 12,655 relations, 24.7% were re-cited after 1 year, 10.1% after 2 years and 8.0% after 3 years.
  - Liability of newness: the annual hazard is 0.753 for 1-year-old ties and 0.529 for older ties. Of 883 relations cited in each of the first 3 surveys, 47.1% were cited in the 4th.
  - Pooled across 19 study rows (R² ≈ 95%): `Y = (T+1)^(γ + κ·KIN + λ·WORK)` with γ = −.716, κ = .250, λ = −1.126.
  - Half-lives: kin 3.42 y, colleagues 0.46 y. For non-kin the paper prints 2.63 y, but the equation gives 1.63 y; 2.63 is `T+1` (derived).
- **Wellman et al. (1997):** 33 Torontonians interviewed a decade apart. Only 27% of intimate ties persisted.
- **Mollenhorst, Volker, Flap (2014)** (Dutch panel: 1,007 people, 604 re-interviewed after 7 years):
  - 48% of discussion partners and practical helpers were still in the network; 30% still held the same position.
  - The average number of confidants barely changed.
- **Cornwell et al. (2014), NSHAP** (2,261 older adults, 5 years apart):
  - 52.5% of Wave-1 confidants were kept and 41.3% lost (derived).
  - 93% of respondents saw some change in who their confidants were.
  - Reasons for losses: moved or distance 23.4%, death 17.1%, drifted apart 14.8%.
- **Saramäki et al. (2014), PNAS 111** (24 students, 18 months, three 6-month intervals):
  - Membership overlap between intervals (Jaccard): whole networks 0.22 and 0.27; top-20 alters 0.36 and 0.44.
  - New alters in the top 20: 41%, then 21%.
  - Signature stability: Jensen–Shannon divergence to one's own earlier signature `d_self = 0.036`, against `d_ref = 0.086` to others.
- **Roberts & Dunbar (2015)**, same cohort: inner-layer friends still inner at 18 months 48.6%, kin 70.3%. On a 1–10 closeness scale, friends lost closeness over time (b = −0.62) and kin gained it (b = +0.27).
- **Miritello et al. (2013), Sci. Rep. 3:1950** (~20M phone users, 7-month window):
  - The number of active ties stays roughly constant: ties gained ≈ ties lost.
  - About 75% of the ties active at the start stay active through the window (random model ≈ 50%).
  - Among ties with fewer than 10 calls, only ~20% stay active.
- **Roy, Bhattacharya, Dunbar, Kaski (2022):** among mutual top-5 pairs, 1–4% of alters change per year; turnover is higher at ages 17–21.

### 5.4 Mortality, fertility, households

- **Life expectancy at birth, 2024:** 79.0 years (men 76.5, women 81.4); 19.7 years at 65 (NCHS Data Brief 548).
- **Share of births surviving to each age, 2024 life table:** 65: 84.1%; 85: 43.8%; 100: 2.1% (NVSR 75-5).
- **Gompertz:** the mortality doubling time is "consistently around 8 years" (Kirkwood 2015). Our fit to 2024 survivorship at ages 45–85 gives `a ≈ 6.1e−5`, `b ≈ 0.082/yr` (derived).
- **Births, 2024:** 3,628,934 births; birth rate 10.7 per 1,000; mean age of mother at first birth 27.6. Total fertility rate 1.63 (provisional).
- **Households, 2025:** mean size 2.50 people; one-person households 29.5% (derived from HH-4).
- **Living in the parental home, 2025:** at 18–24, men 58.8% and women 56.4% (college dorms count as home). At 25–34, men 19.2% and women 13.6%.

---

## 6. Answering "state at time t" cheaply

| Technique | Applies to | Cost for 100 y at daily resolution (T = 36,525; lg T ≈ 15.2) |
|---|---|---|
| Anchor walk, memoised per request (§2.4) | life-course streams: jobs, moves, unions, births, household membership | O(E): ≤ 30 events plus ≤ 100 age pieces, 3–6 µs |
| Dyadic count tree (§2.3) | sparse Poisson or Cox counts; "n-th event"; regeneration streams | L = 16 over 100 y, 18 over 1800–2300; ~1 µs |
| Gamma–beta bridge | renewal with gamma gaps, n-th event | O(log n) |
| Regeneration lookback (§1.1) | Markov on/off states, labels | E = 1/ρ steps; O(1) buckets if Δ ≈ 1/ρ |
| Absolute buckets (§2.2) | dense streams (messages) | O(buckets in window + events) |
| Closed-form marginals (`P(on at t)`, Poisson count law) | calibration and expected roster sizes only | O(1). **Never** use them to produce individual facts: they are not consistent along a sample path |
| Stability radius | step-valued attributes | Radius = next event time minus t, read off the event list. Feeds `trajectory::step_stability_radius` and view caching |
| Epoch checkpoints | valid **only** where the process regenerates at the checkpoint (Markov with minorisation) or the state is a closed-form count (Poisson via the tree) | For semi-Markov processes a checkpoint is just memoisation (a pure cache). Never re-draw state at a checkpoint: that breaks continuity |
| Per-epoch composition (§3.4) | transposition models | 36,525 rounds, about 1–2 ms per point and ×S_G per roster: the cost to avoid |

---

## 7. Realism targets

| Metric | Target | Source |
|---|---|---|
| Moved in past year, age 1+ | 7.8% (CPS 2023); 11.8% (ACS 2024) | Census A-1; ACS |
| Mover rate by age | peak 18.9% at 25–29; 11.4% at 30–34; 5.4% at 45–54; 2.8% at 65+ | CPS 2023 Table 1-S (derived) |
| Move type (share of movers) | 54.1 / 23.4 / 17.5 / 5.0% (county / state / interstate / abroad) | Census A-1 |
| Inter-county move distance | 40.4% under 50 mi; 22.7% 50–199; 12.8% 200–499; 24.1% 500+ | Census A-6 (2020–21) |
| Destination distance decay | `d^−s`, s ≈ 0.9–1.07 | Boucherie et al. 2024 |
| Whole-household moves | ~2.39 movers per moving householder (mean household size 2.50) | CPS 2023 (derived) |
| Jobs held, ages 18–58 | 12.9 (5.6 at 18–24; 1.3 at 55–58) | BLS NLSY79 |
| Jobs ending within 1 year | 61% (started at 18–24); 21% (45–54) | BLS NLSY79 |
| Job-ending hazard shape | peaks at 3 months of tenure, then declines | Farber 1994 |
| Median tenure | 4.1 y; 1.5 (20–24) to 9.9 (65+) | BLS Jan 2026 |
| Monthly quits / separations | 2.0% / 3.3% | JOLTS 2025 |
| Median age at first marriage | men 30.8, women 28.4 | Census MS-2 (2025) |
| Never married at 40 | 25% | Pew (2021) |
| First marriage intact (women) | 0.80 / 0.68 / 0.60 / 0.52 at 5 / 10 / 15 / 20 y | NSFG, NHSR 49 |
| First cohabitation | median 22 months; after 3 y: 40% married, 27% dissolved | NHSR 64 |
| Divorce → remarriage | median 3.7–3.8 y | Census P70-125 |
| Life expectancy at birth | 79.0 (2024) | NCHS DB 548 |
| Survival to 65 / 85 | 84.1% / 43.8% | NVSR 75-5 |
| Adult mortality doubling time | ≈ 8 y (b ≈ 0.082/yr) | Kirkwood 2015; our fit |
| Births per year; total fertility rate | 3.63M; 1.63 | NCHS 2024 |
| Mean household size; one-person share | 2.50; 29.5% | Census HH-6, HH-4 |
| Colleague ties re-cited after 1 / 2 / 3 y | 24.7 / 10.1 / 8.0% | Burt 2000 |
| Tie survival exponent | −0.466 kin, −0.716 non-kin, −1.842 colleague | Burt 2000 |
| Intimate ties persisting 10 y | 27% | Wellman et al. 1997 |
| Personal network still present after 7 y | 48% (30% in the same role) | Mollenhorst et al. 2014 |
| Confidants kept over 5 y (older adults) | ~52.5% | Cornwell et al. 2014 |
| Top-20 alter overlap per 6 months | Jaccard 0.36–0.44 | Saramäki et al. 2014 |
| Active ties surviving 7 months | ~75% (~20% for ties with fewer than 10 calls) | Miritello et al. 2013 |

---

## 8. Construction recommendations

### 8.1 Clock and ids

- **Clock.** `Day = i32` days since `EPOCH = 1800-01-01`, in absolute time. Sub-day times are `(Day, seconds)`. Views convert at the boundary, and `Universe.now` only selects which `t` a view asks about. **No fact may depend on `now`**, which extends invariant 3.
- **Person id bits.** Add birth-cohort band, native region and sex as indexable fields, so cohort, pool and region enumeration is `where_eq` pushdown. Keep the 32-bit width (invariant 4). This requires re-laying out the people slot, which is the one breaking change.
- **Lifespan.** `b(x)`, `d(x)` and `alive(x,t)` as in §3.6.

### 8.2 Streams API (`procedural_core`)

```rust
pub trait EventStream {
    /// Events in [t1, t2), window-independent: every draw keyed on absolute coordinates.
    fn events(&self, key: StreamKey, t1: Day, t2: Day) -> Vec<Event>;
    fn count_before(&self, key: StreamKey, t: Day) -> u64;
}
pub struct PoissonBuckets { lambda: fn(Day) -> f64, width: u32 }         // Recipe A
pub struct PoissonTree    { lambda_integral: fn(Day, Day) -> f64, levels: u8 } // Recipe B
pub struct HazardWalk<H>  { anchor: fn(u64) -> Day, hazard: H }          // Recipe C
pub fn regen_state<S>(key: StreamKey, t: Day, rho: f64, draw: impl Fn(u64) -> S) -> S; // §1.1
```

Keys are structured integers `(seed, stream_id: u16, entity: u64, a: u32, b: u32)` hashed with xxh3. That drops the `format!` keys and saves ~23 ns per draw (measured).

### 8.3 Memberships (`CoordinateSystem`)

```rust
pub struct CoordinateSystem { block_bits: u8, k: u8, rounds: u8 }  // K keyed bijections per block
impl CoordinateSystem {
    fn forward(&self, k: u8, block: u32, local: u32) -> u32;       // Feistel + cycle-walk
    fn inverse(&self, k: u8, block: u32, coord: u32) -> u32;
}
// Membership type = (CoordinateSystem, spell HazardWalk, eligibility, group capacity/alive fns)
```

Apply this to:
- **workplace:** person-level, domain = (household city slot at spell start, member ordinal);
- **household city:** household-level, `I = 4` hierarchical systems;
- **neighbourhood:** derived from (city slot, local move ordinal);
- **school:** derived from age and residence, with no bijection;
- **clubs and teams:** as for workplaces.

### 8.4 Households and residence

A household has an id, `h = (founder, ordinal)`, a membership stream, a move stream (Recipe C with Rogers–Castro × duration × triggers), and destination index draws. `residence(x,t) = residence(household(x,t), t)`. Split-offs create new households. Unions merge members into a new household at `t_union`.

### 8.5 Unions and births

- **Unions:** attempt-indexed involutions `μ_j`, symmetric timing and pair-keyed dissolution, as in §4 J3–J4.
- **Births:** child-driven lookup into the parent union, with `children(U)` enumerated by preimages and filtered (§3.6). Prototype this and measure the recursion depth before committing.

### 8.6 Ties

- Candidates for `x`'s ties are the co-members over `x`'s past and present contexts.
- A tie is keyed `(pair, context, overlap ordinal)`.
- After context exit, its lifetime is Lomax: `T = U^{1/γ} − 1`, `U = H(tie, "life")`, with γ by type (Burt). Cap it at the first death.
- Strength follows the 05-14 note's component model. Communication events use Recipe A on `λ_ij(t)`, with `λ` evaluated in absolute time.
- Enumeration cost: ≤ ~30 contexts × roster sizes, with a budget; memoise per request.

### 8.7 Guarantees to property-test (`internot/tests/`)

1. **Duality.** For sampled `(x, t)`: `x ∈ roster(group_at(x,t), t)`, and `∀ y ∈ roster(G,t): group_at(y,t) = G`. The same holds for households, cities, schools and unions: `partner(x,t) = y ⇔ partner(y,t) = x`.
2. **Uniqueness.** At every `t` an alive person has exactly one household, one residence and ≤ 1 union. Each roster lists `x` at most once.
3. **Recombination.** `events(k,[a,b)) ⊎ events(k,[b,c)) == events(k,[a,c))` bitwise, for random `a < b < c` including times that are not day-aligned, for every stream kind (A, B, C, D).
4. **Timeline/state agreement.** `state_at(x, t)` equals the state reconstructed from `events(x, [b(x), t))` for all `t`, and `count_before` agrees with the enumeration.
5. **Joint-event symmetry.** Union start and end dates are identical from both sides. Every member of `h` at `t_move` has the same move date and destination. A reorg permutation is applied identically person-side and group-side.
6. **Life bounds.** No membership, tie, union or event exists for `x` outside `[b(x), d(x))`. No joint object outlives its required members.
7. **Age guards.** No job before 16 or after the retirement date. School grade matches age. Mothers are aged 15–49 at birth. Nobody enters a union before 18.
8. **Order and `now` independence.** Results are identical whatever the query order and with memo caches cleared. Facts at `t` do not depend on `Universe.now`.
9. **Distribution checks** (tolerance bands from §7): mover rate by age, tenure by age, survival of first marriages, tie survival at 1, 2 and 3 years, and the inter-county distance histogram.

### 8.8 Open problems (prototype before committing)

- **Origin-dependent destinations** conflict with dense bijection domains (§3.5). Measure how far the native-region hierarchy is from the CPS distance distribution before building the many-to-one variant.
- **Kinship recursion depth and cost** under child-driven lookup (§3.6).
- **Roster cost for large groups.** `K·cap·E` evaluations is about 60 ms at `cap = 1000`. Decide which views need complete rosters and which can be budgeted samples.
- **Calibrating `p_accept` and `gap`** for unions against three targets at once: median first-marriage age, never-married at 40, and the cohabitation share.

---

## Sources

The math sources were opened directly during this survey. The empirical sources were opened by research subagents, which recorded the exact URLs. "Derived" marks our arithmetic; "(inferred)" marks design claims that no source states.

**Models and algorithms**
- Clementi, Macci, Monti, Pasquale, Silvestri. *Flooding time in edge-Markovian dynamic graphs*, PODC 2008; SIAM J. Discrete Math. 24(4) 2010. https://dl.acm.org/doi/abs/10.1145/1400751.1400781
- Perra, Gonçalves, Pastor-Satorras, Vespignani. *Activity driven modeling of time varying networks*, Sci. Rep. 2:469 (2012). https://arxiv.org/abs/1203.5351
- Karsai, Perra, Vespignani. *Time varying networks and the weakness of strong ties*, Sci. Rep. 4:4001 (2014). https://arxiv.org/abs/1303.5966
- Xu & Hero. *Dynamic stochastic blockmodels for time-evolving social networks*, IEEE JSTSP 8 (2014). https://arxiv.org/abs/1403.0921
- Xu. *Stochastic block transition models for dynamic networks*, AISTATS 2015. http://proceedings.mlr.press/v38/xu15.pdf
- Matias & Miele. *Statistical clustering of temporal networks through a dynamic stochastic block model*, JRSS-B 79(4) (2017). https://arxiv.org/abs/1506.07464
- Mazzarisi, Barucca, Lillo, Tantari. *A dynamic network model with persistent links and node-specific latent variables*, EJOR 281(1) (2020). https://arxiv.org/abs/1801.00185
- Krivitsky & Handcock. *A separable model for dynamic networks* (STERGM). https://arxiv.org/abs/1011.1937
- Vestergaard, Génois, Barrat. *How memory generates heterogeneous dynamics in temporal networks*, PRE 90:042805 (2014). https://link.aps.org/doi/10.1103/PhysRevE.90.042805
- Salmon, Moraes, Dror, Shaw. *Parallel random numbers: as easy as 1, 2, 3*, SC11. https://www.thesalmons.org/john/random123/papers/random123sc11.pdf
- Hoang, Morris, Rogaway. *An enciphering scheme based on a card shuffle* (swap-or-not), CRYPTO 2012. https://arxiv.org/abs/1208.1176
- Aldous. *Spectral gap for the interchange process* (open-problem page). https://www.stat.berkeley.edu/~aldous/Research/OP/sgap.html
- Caputo, Liggett, Richthammer. *Proof of Aldous' spectral gap conjecture*. https://arxiv.org/abs/0906.1238
- Møller & Rasmussen. *Perfect simulation of Hawkes processes*, Adv. Appl. Prob. 37(3) (2005), which uses Hawkes & Oakes (1974), J. Appl. Prob. https://www.cambridge.org/core/journals/advances-in-applied-probability/article/perfect-simulation-of-hawkes-processes/BCC3A80DEF4895F0E8F78A6716910AC8
- Li, Wong, Chen, Duvenaud. *Scalable gradients for stochastic differential equations* (virtual Brownian tree), AISTATS 2020. https://arxiv.org/abs/2001.01328
- Devroye. *Non-Uniform Random Variate Generation* (1986), chapters V, VI, IX. http://luc.devroye.org/rnbookindex.html
- Worley noise and Worley (1996), *A cellular texture basis function*. https://en.wikipedia.org/wiki/Worley_noise
- Vacancy chains: White, *Chains of Opportunity* (1970); Chase, *Vacancy chains*, Annu. Rev. Sociol. 17 (1991). https://en.wikipedia.org/wiki/Vacancy_chain
- Farber. *The analysis of inter-firm worker mobility*, NBER w4262 (1994). https://www.nber.org/papers/w4262

**Mobility, jobs, family, mortality**
- Census CPS geographic mobility:
  - Table A-1: https://www2.census.gov/programs-surveys/demo/tables/geographic-mobility/time-series/historic/hst_mig_a_1.xlsx
  - Table A-5: https://www2.census.gov/programs-surveys/demo/tables/geographic-mobility/time-series/historic/hst_mig_a_5.xlsx
  - Table A-6: https://www2.census.gov/programs-surveys/demo/tables/geographic-mobility/time-series/historic/hst_mig_a_6.xlsx
  - 2023 Table 1-S: https://www2.census.gov/programs-surveys/demo/tables/geographic-mobility/2023/cps-2023/mig_01-S_2023_1yr.xlsx
- ACS 1-year migration guidance: https://www.census.gov/topics/population/migration/guidance/acs-1yr.html
- Boucherie, Maier, Lehmann. *Decoupling geographical constraints from human mobility*. https://arxiv.org/abs/2405.08746
- Poot, Alimi, Cameron, Maré (2016), gravity-model survey. https://www.aecr.org/old/images/ImatgesArticles/2016/12/03_Poot.pdf
- Simini, González, Maritan, Barabási. *A universal model for mobility and migration patterns*, Nature 484 (2012). https://arxiv.org/abs/1111.0586
- Rogers & Castro. *Model Migration Schedules*, IIASA RR-81-30 (1981). https://pure.iiasa.ac.at/id/eprint/1543/1/RR-81-030.pdf
- BLS NLSY79 jobs: https://www.bls.gov/news.release/nlsoy.nr0.htm
- BLS NLSY97: https://www.bls.gov/news.release/nlsyth.htm
- BLS employee tenure: https://www.bls.gov/news.release/tenure.nr0.htm
- BLS JOLTS 2025: https://www.bls.gov/news.release/archives/jolts_03132026.htm
- Census marital status table MS-2: https://www2.census.gov/programs-surveys/demo/tables/families/time-series/marital/ms2.xls
- Pew, never-married 40-year-olds: https://www.pewresearch.org/short-reads/2023/06/28/a-record-high-share-of-40-year-olds-in-the-us-have-never-been-married/
- NSFG first marriages (NHSR 49): https://www.cdc.gov/nchs/data/nhsr/nhsr049.pdf
- NSFG cohabitation (NHSR 64): https://www.cdc.gov/nchs/data/nhsr/nhsr064.pdf
- Kreider & Ellis, P70-125: https://www2.census.gov/library/publications/2011/demo/p70-125.pdf
- CDC marriage and divorce FastStats: https://www.cdc.gov/nchs/fastats/marriage-divorce.htm
- NCHS life expectancy, Data Brief 548: https://www.cdc.gov/nchs/products/databriefs/db548.htm
- US life tables, NVSR 75-5: https://www.cdc.gov/nchs/data/nvsr/nvsr75/nvsr75-05.pdf
- Kirkwood (2015), mortality doubling time: https://pmc.ncbi.nlm.nih.gov/articles/PMC4360127/
- Births FastStats: https://www.cdc.gov/nchs/fastats/births.htm
- Total fertility rate, VSRR 38: https://www.cdc.gov/nchs/data/vsrr/vsrr038.pdf
- Census households HH-6: https://www2.census.gov/programs-surveys/demo/tables/families/time-series/households/hh6.xls
- Census households HH-4: https://www2.census.gov/programs-surveys/demo/tables/families/time-series/households/hh4.xls
- Census young adults living at home, AD-1: https://www2.census.gov/programs-surveys/demo/tables/families/time-series/adults/ad1.xls

**Ties**
- Burt, *Decay functions* (2000 preprint): http://ronaldsburt.com/research/files/DF.pdf
- Roberts & Dunbar (2015), Human Nature 26: https://pmc.ncbi.nlm.nih.gov/articles/PMC4626528/
- Roberts & Dunbar (2011), Personal Relationships: https://ora.ox.ac.uk/objects/uuid:41f6a45d-9d7d-4bb7-8ac2-cc13dd04a103
- Saramäki et al. (2014), PNAS 111: https://arxiv.org/pdf/1204.5602
- Wellman, Wong, Tindall, Nazer (1997): https://api.semanticscholar.org/graph/v1/paper/DOI:10.1016/S0378-8733(96)00289-4?fields=title,abstract,year
- Mollenhorst, Volker, Flap (2014): https://dspace.library.uu.nl/handle/1874/307264 and https://www.sciencedaily.com/releases/2009/05/090527111907.htm
- Small et al. (2015), Social Networks: https://vontrese.com/wp-content/uploads/2025/07/Small-et-al_2015_How-stable-is-the-core-discussion-network_Social-Networks.pdf
- Cornwell et al. (2014), NSHAP network change: https://pmc.ncbi.nlm.nih.gov/articles/PMC4303098/
- Miritello et al. (2013), Sci. Rep. 3:1950: https://www.nature.com/articles/srep01950.pdf
- Roy, Bhattacharya, Dunbar, Kaski (2022), *Turnover in close friendships*: https://arxiv.org/abs/2203.14854
