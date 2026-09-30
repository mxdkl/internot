# Temporal Networks for Internot's Procedural Social Substrate

**Date:** 2026-05-14
**Scope:** how graphs change over time, how to model that, and how to make every temporal fact a deterministic function of `(id, key, t)`.

Internot's invariant: every fact is `f(id, key, t)` — hash-derived, never stored. This note surveys the temporal-network literature bearing on the next problem: letting the social graph evolve (birth, death, marriage, friendship formation, bursty communication, tie decay) while keeping every query — including "every email A sent B during `[T1, T2]`" — answerable from hashes alone.

---

## 1. Temporal network formalisms

Canonical: Holme & Saramäki, *Temporal Networks* (Phys. Rep., 2012); updated in Holme, *Modern temporal network theory* (EPJ Data Sci., 2015). Three representational families:

**(a) Snapshot sequences.** A sequence `{G_1, ..., G_K}` of static graphs over time windows. Cheap to analyze with static-graph tooling; bad fit for bursty processes where most snapshots are empty and information lives in inter-event times. Holme 2015 §3 warns that "the choice of aggregation window is one of the most arbitrary decisions in temporal network analysis."

**(b) Continuous-time event streams (contact sequences).** A set `E = {(u, v, t)}`. The native form for email logs, calls, face-to-face proximity, mobile-phone CDRs. Every large-scale empirical study since 2005 (Eckmann et al. 2004 email, Onnela et al. 2007 mobile calls, Karsai et al. 2011 SMS/Twitter, Cattuto et al. 2010 SocioPatterns RFID) operates here. Reachability, temporal paths, time-respecting walks (Pan & Saramäki 2011) are defined directly on the stream. This is what Internot should target.

**(c) Adjacency tensors / interval graphs.** Edges annotated with `(t_start, t_end)` intervals for ties of nonzero duration. The natural representation for *relational state* (married, employed-at, lives-with) — distinct from *communication events*.

**When to use which (recommendation):**

- **Communication / contact events** → continuous-time event streams sampled lazily by `(sender_id, receiver_id, key, t)`.
- **Relational state with duration** (marriage, cohabitation, employment, friendship-active) → interval graphs with hash-derived `t_start`, `t_end`; membership is a range query.
- **Aggregated diagnostics** (degree, clustering, mixing) → snapshots derived on demand.

Continuous-time and interval-graph layers are primary; snapshots are a view.

---

## 2. Edge birth / death processes

Three threads in the empirical literature inform how social ties form and dissolve.

**Activity-driven networks** (Perra, Gonçalves, Pastor-Satorras, Vespignani, *Sci. Rep.* 2012): each node has a heterogeneous **activity rate** `a_i ∼ F(a)` (typically a truncated power law) governing the probability it initiates a contact per unit time. The instantaneous network is sparse and bursty; aggregated, it produces realistic scale-free degree distributions without invoking preferential attachment. This maps cleanly onto Internot's `f(id, key)` design: every person carries a hash-derived `activity_rate ∼ Pareto`, and contact initiations are a Poisson-like process keyed on it.

**Link-prediction features** (Liben-Nowell & Kleinberg, *JASIST* 2007): future co-authorship is well predicted by **structural proximity** (common neighbors, Adamic–Adar, Katz, rooted PageRank, hitting time) plus **attribute homophily**. Takeaway for Internot: *which* ties form is largely a function of current static structure; *when* they form is the separate temporal question driven by activity and exposure.

**Edge aging** (Hidalgo & Rodriguez-Sickert, *Physica A* 2008, mobile-phone data): per-edge event probability drops with the time since the last event, `p(new_event | gap = τ) ∝ τ^(-α)` for `α ≈ 0.7–1.2` by tie type. Edges have memory; they age out. This grounds the Hawkes reading in §6.

Combined: tie *formation* is exposure-driven (homophily + structure + activity); tie *decay* is a memoryful per-edge process whose hazard rises with gap length until effectively absorbing.

---

## 3. Tie-strength dynamics over time

Granovetter (*AJS* 1973) defines tie strength as "a combination of the amount of time, the emotional intensity, the intimacy, and the reciprocal services which characterize the tie." Marsden & Campbell (*Social Forces* 1984) empirically evaluated indicators and found **closeness** the single best predictor, with duration and contact frequency correlated but weaker.

For Internot the operational construct is a continuous `s_{ij}(t) ∈ [0, 1]` per edge, modulating both the rate and content of communication.

**Decay numbers, kin and partner ties:**

- **Roberts & Dunbar** (*Personal Relationships* 2011) tracked 30 emerging adults' egonets through the transition to university. Friend ties decayed **exponentially**, half-life ~6–12 months without investment; kin ties showed no measurable decay. Replicated in Roberts, Dunbar et al. (*Social Networks* 2009).
- **Hill & Dunbar** (*Human Nature* 2003) on Christmas-card networks: ~150 active contacts at any moment, ~5 "support clique," ~15 "sympathy group." Inner layers turn over slowly; outer layers yearly.
- **Saramäki et al.** (*PNAS* 2014, "Persistence of social signatures") used 3 years of mobile-phone data on 24 students through school-to-work transitions. Each maintained a stable rank-frequency "social signature" even as alter *identities* changed. Top-3 alter turnover: ~40% per 18 months.
- **Burt** (*Social Networks* 2000, "Decay functions") across five longitudinal datasets: `p(t) = p(0) · exp(-t/τ)` with `τ` varying by type — close friends ~7 years, acquaintances ~1 year, work colleagues ~2 years once colocation ends.

**Decay numbers, work and school ties:**

- **Wellman, Wong, Tindall & Nazer** (*Social Networks* 1997, "A decade of network change") on Toronto egonets across 10 years: half the year-0 ties were gone by year 10, but aggregate tie count was stable — turnover, not attrition. Coworker ties decayed fastest (half-life ~3 years post-job-change); kin ties slowest.
- **McPherson, Smith-Lovin & Brashears** (*ASR* 2006) on GSS "discuss-important-matters": median tie duration in the core discussion network ~7 years; ~20% of ties < 1 year old at any cross-section. Marsden 2014 reanalysis lowered headline numbers but preserved the broad timescales.
- **McKenzie & Sirianni** working papers on Facebook tie decay (cf. Dunbar *Social Networks* 2016): online ties without offline reinforcement decay with half-life ~12–18 months.

**Family / cohabitation.** Parent–child and sibling ties show no clean exponential decay; they look piecewise, driven by life events (cohabitation, geographic separation, child marriages, parental retirement). A useful approximation: baseline `s_kin` decreasing stepwise at boundaries (child leaves home: `× 0.7`; divorce partner-tie: `→ 0`; relocation `> 500 km`: `× 0.85`).

**Ex-coworker / ex-partner ties** are asymmetric: rates drop 5–10× within months but the long-run floor is non-zero. A fast exponential to a non-zero asymptote (`s(t) = s_floor + (s_0 − s_floor) · exp(-t/τ_fast)`) fits Roberts & Dunbar 2011 and Burt 2000 curves better than a single exponential to zero.

---

## 4. Burstiness in human communication

**Barabási, *The origin of bursts and heavy tails in human dynamics* (Nature 435, 207–211, 2005)** is the central reference. Empirical claim: inter-event times (IETs) `τ` in email, web browsing, library use, mobile calls follow heavy-tailed power laws:

> `P(τ) ∝ τ^(-α)`, with `α ≈ 1` for email, `α ≈ 1.5–2.5` for other streams.

Proposed mechanism: a **priority queue model**. Agents hold a queue of pending tasks each with priority `p_i ∼ U(0, 1)`, executing the highest-priority pending task at each step; tasks arrive at rate `λ`. Highest-priority-first execution yields waiting-time power laws with `α = 3/2` (or `α = 1` for strict variants). Bursts followed by long silences emerge from queue dynamics alone, no circadian or social mechanism required.

**Counter-explanations.** The priority-queue story was challenged because much observed burstiness is circadian/weekly. **Malmgren, Stouffer, Motter, Amaral** (*PNAS* 2008, "A Poissonian explanation for heavy tails in e-mail communication") showed a *cascading non-homogeneous Poisson* (circadian-modulated Poisson with within-session bursts) fits email IETs as well as the priority queue and is statistically preferred. **Jo, Karsai, Kertész, Kaski** (*New J. Phys.* 2012, "Circadian pattern and burstiness in mobile phone communication") confirmed on calls: after de-trending the circadian cycle, residual IETs are less heavy-tailed but still bursty. Both mechanisms contribute.

**Empirical exponents** (collated; cf. Karsai et al. *Sci. Rep.* 2012 "Universal features of correlated bursty behaviour"):

| Channel | `α` (IET tail exponent) | Source |
|---|---|---|
| Email (sending) | ≈ 1.0 | Eckmann et al. 2004, Barabási 2005 |
| Mobile calls | ≈ 0.7–0.9 | Karsai et al. 2011, Jo et al. 2012 |
| SMS / text | ≈ 1.5–1.9 | Wu et al. 2010 |
| Twitter | ≈ 1.5–2.5 | Karsai et al. 2012 |
| Letter correspondence (Einstein, Darwin) | ≈ 1.5 | Oliveira & Barabási 2005 |

Goh & Barabási 2008 define a dimensionless **burstiness parameter** `B = (σ_τ − μ_τ) / (σ_τ + μ_τ)`: `B = 0` for Poisson, `B → 1` as IETs heavy-tail. Empirical social channels cluster `B ∈ [0.2, 0.6]`. They also define the lag-1 IET autocorrelation `M`; together `(B, M)` characterize burstiness more completely than a single exponent.

Design point for Internot: a stream that is Poisson at the daily scale will look obviously wrong to any analysis tool measuring `B` or fitting a tail. The substrate must produce both circadian modulation and heavy-tailed residual IETs.

---

## 5. Circadian and weekly rhythms

Communication shows strong diurnal/weekly periodicity. Aledavood et al. (*PLOS ONE* 2015; *J. Biol. Rhythms* 2018) document:

- Call/SMS volume peaks 5–9 PM weekdays, near-zero 1–6 AM.
- Email peaks 10 AM–4 PM weekdays with a smaller post-dinner peak.
- Each individual has a **chronotype-stable** phase (early-bird vs. night-owl) persisting over years.
- Weekend patterns differ qualitatively (single broader afternoon peak).

The composable activity-rate model:

```
λ(person, t) = base_rate(person)
             × circadian(time_of_day, chronotype(person))
             × weekly(day_of_week)
             × burst_state(person, t)
             × tie_strength(person, other, t)
```

where:

- `base_rate(person)` — hash-derived Pareto scalar (the activity-driven networks ingredient).
- `circadian(t_od, chr)` — periodic with two Gaussian bumps whose centers/widths depend on chronotype.
- `weekly(dow)` — 7-bin lookup, `Sat/Sun ≠ Mon-Fri`.
- `burst_state(t)` — slowly varying envelope `[0.5, 2.0]` ("high-output week"); fits `procedural_core::trajectory::smooth`.
- `tie_strength(person, other, t)` — from §3.

Jo et al. 2012 showed that with circadian de-trended, residual IETs are still heavier-tailed than Poisson — burstiness is not fully explained by the cycle. So `burst_state(t)` is needed even after the circadian factor.

---

## 6. Memory effects: Hawkes processes

A **Hawkes process** (Hawkes 1971; Bacry, Mastromatteo & Muzy 2015 review) has self-exciting conditional intensity

```
λ(t) = μ + Σ_{t_i < t} φ(t − t_i)
```

with baseline `μ` and non-negative kernel `φ(τ)` (typically `α · exp(-β τ)` or `α / (τ + c)^p`), capturing how past events boost future rates. Hawkes processes naturally produce bursty IETs and explain "you just emailed Alice; you're more likely to email her again soon than a random equally-close contact."

**Masuda et al.** (2013) fit multivariate Hawkes processes to email/SMS data, reproducing empirical IETs and burst clustering. **Karsai et al.** *Sci. Rep.* 2012 offer an alternative — correlated event sequences with a memory parameter — fitting similarly.

The catch for Internot: Hawkes is *sequential* — each event's intensity depends on history. To stay in `f(id, key, t)` we cannot accumulate history during a query. Two workarounds:

1. **Derive the Hawkes intensity from the deterministic event schedule.** Since the schedule is hash-derived (§8), "history" at any `t` is well-defined and recomputable.
2. **Use a non-Hawkes approximation.** Model `burst_state(person, t)` as a slowly varying envelope; lose true self-excitation but preserve the bursty IET tail when combined with circadian modulation. Loss is modest if the burst envelope timescale matches dominant Hawkes decay (~hours to days for email).

Start with (2); revisit if measured `B`/`M` on the stream falls outside empirical bands.

---

## 7. Empirical decay timescales

Collected anchor values usable as parameter ranges:

| Tie type | Decay timescale `τ` (1/e) | Source |
|---|---|---|
| Close friend (no investment) | 6–12 months | Roberts & Dunbar 2011 |
| Acquaintance / weak friend | 3–6 months | Burt 2000 |
| Coworker (post-job-change) | 1–3 years | Wellman et al. 1997, Burt 2000 |
| Schoolmate (post-graduation) | 1–2 years | Roberts & Dunbar 2011, Saramäki et al. 2014 |
| Sibling (geographic separation) | ~no decay; piecewise drops at life events | Roberts & Dunbar 2011 |
| Parent–adult-child | ~no decay; weekly contact baseline | Hill & Dunbar 2003 |
| Ex-partner | Sharp 2–6 month drop, then nonzero floor | Saramäki et al. 2014 implicit |
| Online-only acquaintance | 12–18 months | Dunbar 2016, McKenzie & Sirianni |
| Same-employer current colleague | ∞ (while employment overlaps) | tautology |

Read as order-of-magnitude anchors with high inter-individual variance. A person-level hash-derived multiplier in `[0.5, 2.0]` reflects across-study variance.

**Onnela et al. 2007** (*PNAS* 104, 7332): 18 weeks, ~4M mobile users. Per-edge call/SMS aggregate weight is heavy-tailed and concentrated on a strong-tie minority — Granovetter at population scale. Follow-on Karsai et al. 2011 on the same data: removing top-weighted edges fragments the network faster than random, while removing weakest edges fragments faster per-edge — the "strength of weak ties" operationalized. For Internot the implication: edge-weight distribution must itself be heavy-tailed (Pareto-like), not normal.

---

## 8. Deterministic event-stream sampling

The hard question: given a continuous-time intensity `λ(t)` over `[T1, T2]`, **enumerate event times deterministically from hashes alone, without iterating from zero or storing state.**

Three relevant techniques:

**(a) Inverse-CDF via time-rescaling.** For a non-homogeneous Poisson process with rate `λ(t)`, the cumulative intensity `Λ(t) = ∫_0^t λ(s) ds` satisfies the **time-rescaling theorem** (Brown, Barbieri, Ventura, Kass, Frank, *Neural Computation* 2002): rescaled events `{Λ(t_i)}` form a homogeneous Poisson process of rate 1. In rescaled time, gaps `Δ_i = Λ(t_{i+1}) − Λ(t_i)` are iid `Exp(1)`. Sample event `i`: draw `u_i ∼ U(0, 1)`, set `Δ_i = −log(u_i)`, `t_{i+1} = Λ^{-1}(Λ(t_i) + Δ_i)`.

The event stream is a cumulative sum of independent exponentials, `S_i = Σ_{k=1}^{i} −log(u_k)`. Critically, the `u_k` can be deterministic functions of `(sender, receiver, k)` via `hash_float`:

```
S_i(A, B) = Σ_{k=1}^{i} −log(hash_float((A, B), "iet_k", k))
t_i(A, B) = Λ^{-1}(S_i(A, B))
```

Each event index `i` is a pure function of `(A, B, i)`. The time `t_i` is recovered by inverting `Λ`, itself a pure function of `t`.

**(b) Window-bounded enumeration via binary search.** To enumerate events in `[T1, T2]`:
1. Compute `S_min = Λ(T1)`, `S_max = Λ(T2)`.
2. Binary-search for the smallest `i` with `S_i ≥ S_min` (monotone in `i`).
3. Walk `i = i_min, i_min+1, ...` evaluating `t_i = Λ^{-1}(S_i)` until `S_i > S_max`.

Cost: `O(log i_min + events_in_window)`. Each `S_i` evaluation is `O(i)`. To avoid `O(i)` per binary-search step, chunk: precompute partial sums at multiples of `C = 256` by hashing `(A, B, "block", i/C)` — at the cost of exact independence at block boundaries. Or accept `O(i_min log i_min)` total; for a low-rate edge with `i_min ≈ 100` over years this is microseconds.

**(c) Thinning** (Lewis & Shedler 1979). Sample a homogeneous Poisson process at rate `λ_max = sup λ(t)` over the window, then accept each candidate with probability `λ(t_k) / λ_max`. Deterministic via a second hash stream `hash_float((A, B), "thin_k", k)`. Simpler than inverse-CDF but wasteful when `λ_max / λ_mean` is large (a 20× circadian peak-to-trough means 95% rejection).

**Recommendation.** Use **inverse-CDF with time-rescaling** for the per-edge stream, computing `Λ` analytically when factors permit and via Romberg quadrature otherwise. Fall back to thinning only when `Λ` is intractable.

The invariant: every `t_i` is a deterministic function of `(sender_id, receiver_id, i)` and the integrable rate `λ(·)`. Re-querying the same window always yields the same events. No RNG, no stored events — the substrate's promise extended to time.

---

## Tie-strength-over-time model recommendation for Internot

Per-edge `s_{ij}(t) ∈ [0, 1]` as a sum of contributions, each a pure function of `(i, j, t)` and the procedural life timelines in `internot::people`:

```
s_ij(t) = clip01(
    s_base(i, j)
  + s_kin(i, j, t)
  + s_cohabit(i, j, t)
  + s_school(i, j, t)
  + s_work(i, j, t)
  + s_partner(i, j, t)
)
```

Component forms:

- **`s_base(i, j)`** — homophily from procedural attributes (city, industry, age proximity, Big Five cosine). Order `0.05–0.20`. Pure `f(i, j)`, no `t`.

- **`s_kin(i, j, t)`** — `0.7` if `j` is parent/child/sibling of `i` (from life timelines), stepwise multiplied by `0.85` per `≥ 500 km` relocation. No exponential decay floor.

- **`s_cohabit(i, j, t)`** — `0.4` during cohabitation (marriage/partnership, not separated). Post-separation tail: `s = 0.05 + 0.35 · exp(-(t − t_sep) / τ_partner)`, `τ_partner ∼ LogNormal(μ=ln(180 d), σ=0.5)` hash-derived per ex-pair.

- **`s_school(i, j, t)`** — `0.3` during same-school cohort overlap; post-graduation `0.3 · exp(-(t − t_end_overlap) / τ_school)`, `τ_school ∼ LogNormal(μ=ln(540 d), σ=0.6)`.

- **`s_work(i, j, t)`** — `0.25` during employment overlap; post-departure `0.25 · exp(-(t − t_end_overlap) / τ_work)`, `τ_work ∼ LogNormal(μ=ln(730 d), σ=0.5)`. Reset to peak on any re-overlap (procedurally enumerable from career arcs).

- **Diurnal modulation** lives on the rate, not `s`: `λ_ij(t) = s_ij(t) · base_rate_i · circadian(t_od, chr_i) · weekly(dow) · burst_state_i(t)`. Tie strength is the carrier; the rate is the channel.

All `τ` constants and baselines are hash-derived per pair, so two "ex-coworker" pairs produce different but plausible decay curves, matching Roberts & Dunbar 2011 / Burt 2000 inter-individual variance.

Sanity check: a close friend who becomes an ex-coworker drops from `s ≈ 0.45` (base + school-no + work-yes) to `s ≈ 0.2` over 2 years, consistent with Burt 2000 banker-friendship curves.

---

## Deterministic-event-enumeration recipe for Internot

The shape of the query: "every message A sent B during `[T1, T2]`."

```rust
// All inputs are deterministic; no RNG, no stored state.
fn messages_sent(
    sender: PersonId,
    receiver: PersonId,
    t1: DateTime<Utc>,
    t2: DateTime<Utc>,
) -> impl Iterator<Item = (DateTime<Utc>, MessageSeed)> {
    // 1. Build the per-edge instantaneous rate as a pure function of t.
    let rate_fn = |t: DateTime<Utc>| -> f64 {
        let s   = tie_strength(sender, receiver, t);       // §3 model above
        let bs  = burst_state(sender, t);                  // smooth() trajectory
        let circ = circadian(t.time(), chronotype(sender));
        let wkly = weekly(t.weekday());
        let base = base_rate(sender);                      // Pareto, hash-derived
        base * circ * wkly * bs * s
    };

    // 2. Cumulative intensity Λ(t) = ∫_0^t rate_fn(s) ds, computed by
    //    composing analytic factors where possible, Romberg quadrature for
    //    the rest. Λ is monotone increasing in t.
    let cumulative = CumulativeIntensity::new(rate_fn);

    // 3. Rescale window endpoints into homogeneous Poisson time.
    let s_min = cumulative.eval(t1);
    let s_max = cumulative.eval(t2);

    // 4. Find first event index i* such that S_i >= s_min via binary search
    //    on the deterministic exponential cumulative
    //
    //      S_i = sum_{k=1..i} -ln(hash_float((sender, receiver), "iet", k))
    //
    //    using chunked partial sums cached on (edge, chunk_index) for cost.
    let i_min = find_first_index_ge(sender, receiver, s_min);

    // 5. Walk forward enumerating events until S_i > s_max.
    let mut i = i_min;
    std::iter::from_fn(move || {
        let s_i = cumulative_event_sum(sender, receiver, i);
        if s_i > s_max { return None; }
        let t_i = cumulative.invert(s_i);    // numerical root-find on Λ
        let seed = hash_int((sender, receiver), "msg_seed", i);
        i += 1;
        Some((t_i, MessageSeed(seed)))
    })
}

// Each enumerated (t_i, seed) lets you recover the message body
// deterministically: subject = topic_of(seed); body = render(seed, ...).
```

Key properties:

- **Pure determinism.** Re-running the query at any future time produces an identical sequence.
- **Window-local cost.** `O(log i_min + events_in_window)`, not `O(events_since_t=0)` once chunked partial sums are warm. A 5-msg/week edge over 1 month: ~22 events plus ~13 hash evaluations for binary search.
- **Composable with `procedural_overlay`.** Session-allocated messages live in the overlay and merge with the procedural enumeration in `read_thread`/`get_inbox`; the stream provides background history.
- **Stable under window slicing.** `[T1, T_mid] ∪ [T_mid, T2] = [T1, T2]`. The property `internot::tests::*_round_trip` should assert for any temporal view.
- **Recoverable bit layout.** Each event index `i` is the `messages` slot's `seq` field in `internot::mail::messages` (127-bit thread_id + day + seq). The current static-event layout becomes a materialized prefix; no migration needed.

Sharp edge: the **inverse of `Λ`**. If `s_ij` has step discontinuities at life events, `Λ` is piecewise-smooth — use a hybrid (analytic per smooth piece + binary search across pieces). Romberg on a 256-point/day grid handles the smooth case; the piecewise case emits hash-derivable "rate-change events" from the life timelines into a per-edge sorted list.

---

## References (selected)

- Aledavood et al. (2015, 2018). Daily/channel-specific rhythms in mobile communication. *PLOS ONE*; *J. Biol. Rhythms*.
- Bacry, Mastromatteo & Muzy (2015). Hawkes processes in finance.
- Barabási (2005). Origin of bursts and heavy tails in human dynamics. *Nature* 435, 207–211.
- Brown, Barbieri, Ventura, Kass, Frank (2002). Time-rescaling theorem. *Neural Computation*.
- Burt (2000). Decay functions. *Social Networks* 22, 1–28.
- Cattuto et al. (2010). Person-to-person interactions from RFID. *PLOS ONE*.
- Eckmann, Moses, Sergi (2004). Entropy of dialogues in email. *PNAS*.
- Goh & Barabási (2008). Burstiness and memory in complex systems. *EPL*.
- Granovetter (1973). Strength of weak ties. *AJS*.
- Hidalgo & Rodriguez-Sickert (2008). Dynamics of a mobile phone network. *Physica A*.
- Hill & Dunbar (2003). Social network size in humans. *Human Nature* 14, 53–72.
- Holme & Saramäki (2012). Temporal networks. *Physics Reports* 519, 97–125.
- Holme (2015). Modern temporal network theory. *EPJ Data Science*.
- Jo, Karsai, Kertész, Kaski (2012). Circadian pattern and burstiness. *New J. Phys.* 14, 013055.
- Karsai et al. (2011). Small but slow world. *Phys. Rev. E*.
- Karsai et al. (2012). Universal features of correlated bursty behaviour. *Sci. Rep.* 2, 397.
- Lewis & Shedler (1979). Simulation of NHPP by thinning. *Naval Research Logistics Quarterly*.
- Liben-Nowell & Kleinberg (2007). Link-prediction for social networks. *JASIST*.
- Malmgren et al. (2008). Poissonian explanation for email heavy tails. *PNAS*.
- Marsden & Campbell (1984). Measuring tie strength. *Social Forces*.
- Masuda, Takaguchi, Sato, Yano (2013). Self-exciting point processes for conversations. in *Temporal Networks*, Springer.
- McPherson, Smith-Lovin, Brashears (2006). Social isolation in America. *ASR*.
- Oliveira & Barabási (2005). Darwin and Einstein correspondence. *Nature*.
- Onnela et al. (2007). Tie strengths in mobile communication. *PNAS* 104, 7332.
- Pan & Saramäki (2011). Paths and centrality in temporal networks. *Phys. Rev. E*.
- Perra, Gonçalves, Pastor-Satorras, Vespignani (2012). Activity-driven modeling. *Sci. Rep.* 2, 469.
- Roberts & Dunbar (2011). Costs of family and friends. *Personal Relationships*.
- Saramäki et al. (2014). Persistence of social signatures. *PNAS* 111, 942–947.
- Wellman, Wong, Tindall, Nazer (1997). A decade of network change. *Social Networks*.
- Wu et al. (2010). Bimodal distribution in human communication. *PNAS*.
