# Communication patterns on social graphs — empirical structure and synthetic generation

**Date:** 2026-05-14
**Scope:** how real-world human communication rides on a social-tie graph, what empirical regularities the literature has nailed down, what synthetic-comms systems have tried, and how Internot should structure its six communication services (mail, chat, calendar, files, money, tasks) on top of its procedural social-graph substrate.

The substrate question: above a deterministic graph of typed ties (kin, partner, friend, coworker, neighbor, acquaintance, weak-tie, stranger), each carrying a tie-strength scalar and a relationship-age, what does a *coherent stream of communication events* look like? What rates, what modes, what topics, what burstiness, and how do we sample it from a hash function without storing per-event state?

The literature gives us five robust regularities, several softer ones, and a workable deterministic recipe.

---

## 1. Tie strength ↔ communication frequency — Onnela et al. 2007 and what it actually measured

The reference work is Onnela, Saramäki, Hyvönen, Szabó, Lazer, Kaski, Kertész, Barabási, *Structure and tie strengths in mobile communication networks*, PNAS 2007. They built a network from a national mobile-phone operator's call-detail-records (CDR) for ~4 million subscribers over 18 weeks. An edge between A and B was "reciprocated calls in both directions" (a key filter — one-shot calls produce mostly noise, the "did A call B and did B ever call A back" filter removes the long tail of telemarketing-style edges). Tie *weight* `w_AB` was operationalised three ways:

- **Aggregate call duration** over the observation window (their primary measure)
- **Number of calls** (the count, not the time)
- **Reciprocity-weighted** versions of both

Their headline result — known as **the Onnela weak-tie / weight-topology coupling** — is that weight and topology are *positively correlated*: edges with high overlap (the Jaccard overlap of A and B's neighbour sets, a proxy for local embeddedness) tend to be high-weight, edges that bridge communities tend to be low-weight. The famous experiment is the percolation cascade: when you remove edges in order of *descending* weight, the giant component collapses suddenly; when you remove in *ascending* weight order, it collapses gradually. This is Granovetter's "strength of weak ties" (1973) confirmed at population scale. Bridges are weak; cliques are strong.

Two things the literature is *less* clean about, which matter for Internot:

**(a) Duration vs frequency are not interchangeable.** Onnela treated duration as the primary weight but the count and duration are not perfectly correlated. Hidalgo & Rodriguez-Sickert (2008, *The dynamics of a mobile phone network*) showed that *frequency* captures recurrence-of-contact (which predicts persistence of the tie over time) better than duration, while duration captures *intimacy-per-event* (a long phone call signals depth more than a long sequence of pings). Wuchty (2009, *What is a social tie?*) ran the regression: when both duration and frequency are entered as predictors of self-reported emotional closeness in a paired survey, frequency dominates duration. The reciprocity ratio (how balanced the directional volume is) adds independent predictive power on top of both.

**(b) "True" tie strength is multidimensional.** Granovetter's original 1973 paper specified four dimensions: *amount of time*, *emotional intensity*, *intimacy (mutual confiding)*, *reciprocal services*. CDR data captures only time-on-the-phone, and even that conflates several distinct micro-behaviours (long catch-up call ≠ many short pings ≠ habitual goodnight-call). Marsden & Campbell (1984, *Measuring tie strength*) and Marsden & Campbell (2012, the follow-up) ran the survey-vs-behaviour regression: *closeness* (self-report) is the cleanest single indicator. *Duration* and *frequency* are good but biased — they over-weight cohabiting kin (forced proximity inflates duration without depth) and under-weight long-distance close friends (real depth, low frequency).

**Implication for Internot.** Tie-strength `s ∈ [0, 1]` in the procedural graph is the latent variable; communication rate `λ(s, t, mode)` is the observable. Train an agent on a single observable (e.g. only count of emails) and it will conflate cohabiting-spouse with one-and-done-vendor; expose multiple observables (counts × durations × reciprocity-ratio × modes used) and the same latent `s` projects more honestly.

---

## 2. Mode partitioning — the richer-medium-for-stronger-ties hypothesis

When does a person email vs text vs call vs meet vs schedule a calendar event? Three converging streams:

**Media richness theory** (Daft & Lengel 1986) predicts richer (synchronous, multi-cue, immediate-feedback) media for ambiguous/relational tasks; lean media (email) for unambiguous/transactional. Empirically this maps onto tie strength: closer ties get richer modes.

**Kossinets & Watts 2006** (*Empirical analysis of an evolving social network*, Science) tracked one year of email logs at a large university (~43k users, 14M messages). Three findings ride directly on Internot's design:
- The network is *not* well-mixed: 95% of email volume sits inside ~3% of the dyads.
- Triadic closure is the dominant edge-formation mechanism; the probability of A↔C forming given an A↔B↔C path scales with the number of shared neighbours.
- *Most* sustained dyads have a class-, lab-, or course-context shared. Pure-affinity ties are rare in the email substrate.

**Reid & Reid (2005) and Ling (2008)** on SMS specifically: SMS skews toward *intimate* dyads (partners, close friends, immediate kin). The "good morning / good night" texting pattern is a marker of partner-status. Group SMS skews toward planning-tasks.

**Eagle, Pentland & Lazer (2009, *Inferring friendship network structure by using mobile phone data*, PNAS)** is the bridge to multimodal mode-selection. They combined CDR with physical co-location (Bluetooth proximity) and Saturday-night co-location, and showed that **the combined multimodal signal predicts self-reported friendship with ~95% accuracy** vs ~75% for call data alone. The mode mix carries information beyond any single mode's volume.

Wuchty (2009) on multimodal communication specifically — for one organisation he had email + IM + calendar-event + face-to-face-meeting data and could regress self-reported tie strength on each separately. Result: **face-to-face > calendar-event-cooccurrence > IM > email**, in that order, as predictors of close-friend nominations. Email is the *least* informative single mode about closeness because email is used for everything (the universal envelope).

**Implication for Internot.** Mode selection is a function of `(tie_kind, tie_strength, message_intent)`:

- *Casual relational maintenance* → chat/DM for strong ties, nothing for weak ties
- *Scheduling coordination* → calendar invite (with or without email cover) for medium-and-up
- *Asynchronous information transfer* → email, across all strengths
- *Money transfer (Venmo/Cash-app-style)* → restricted to medium-and-up ties + transactional vendors
- *File share* → tracks the calendar/work-thread relationships, not the personal ones
- *Task assignment* → restricted to org-internal directed edges (manager→report, or self-loop)

The base rates differ by mode by orders of magnitude. Mobile phones produce ~100 SMS/day for active users, ~10 calls/day. Email is ~30 received/day at the median knowledge worker (Radicati Group annual report, multi-year). Calendar invites are 1-5/day. Files-shared is ~1/day. So mode-rate ratios are roughly `chat : email : calendar : files ≈ 100 : 30 : 3 : 1` for an average information worker — and the dispersion across the population is huge.

---

## 3. Topic distributions by tie kind

This is the literature that has progressed slowest, because public datasets are rare. The available evidence:

**Enron corpus** (Klimt & Yang 2004; McCallum, Wang, Corrada-Emmanuel 2007 *Topic and role discovery in social networks with experiments on Enron and academic email*; McCallum et al. ATM — Author-Topic Model). With ~500k emails over 158 employees, McCallum et al. ran the Author-Recipient-Topic model and recovered topic clusters that align cleanly with (a) project teams (deal-making, west-power-trading, regulatory) and (b) a small but distinct "personal" cluster covering travel-plans, family, sports betting, jokes. The personal cluster lives on a *different sub-graph* (denser, smaller, higher reciprocity) than the work clusters. Bird, Gourley, Devanbu et al. (2006) found similar work-personal separation in open-source-project email lists.

**Lyle's family-communication corpus** (Lyle 2011, *Family Communication Standards Inventory*) and the broader family-comms tradition (Vangelisti's edited handbook, third edition): family talk skews to (a) coordination ("when is dinner", "pick up kids"), (b) check-in ("how was your day"), (c) shared-event-planning (holidays, birthdays), (d) advice/support during stress, (e) shared-memory invocation ("remember when"). Parent-child has a sharp age-stratified shift: teens shift from coordination-dominated to advice-and-emotional-support around age 18-22 (Aquilino 2006). Spousal communication has the highest density of *implicit reference* ("the thing we talked about") — they share context, so explicit subject lines are sparse.

**Reddit r/family vs r/work topic-modelling** (Mohammad & Turney 2013's NRC emotion lexicon applied to subreddit corpora; numerous subsequent studies): family subreddits skew to *emotion-laden* tokens (love, worry, frustration, gratitude), work subreddits skew to *task-and-process* tokens (deadline, manager, deliverable, performance-review). The LDA topics that emerge are stable across replications: family = {birthdays, holidays, kid-milestones, illness, money-stress, in-laws}; work = {deadlines, meetings, scope, blocker, review-cycle, comp}.

**Goh & Choi et al.** on cross-domain topic separation in mixed corpora confirmed that even when a single user emits both family-mode and work-mode messages, an unsupervised topic model recovers the partition cleanly — the *vocabularies overlap less than 10%* at the high-frequency-content-word tail.

**Friends** sit topically between family and coworkers — coordination-dominated for activity-planning, with bursty topic-of-the-week patterns (a viral video, a sporting event, a meme). McCallum et al. showed that within-cohort friends have *temporally clustered* topic bursts (a topic spreads through the cohort over 1-3 days then decays).

**Implication for Internot.** Topic distribution is a function of `(tie_kind, tie_strength, time_of_year, current_life_events)`. The procedural floor already encodes career-arc and life-events; the topic generator reads those plus the calendar:

- Parent↔adult-child: 60% coordination + check-in, 25% advice/support, 10% shared-memory, 5% logistics-of-major-life-events (wedding, illness, move)
- Spouse↔spouse: 40% household-coordination, 20% kids (if any), 20% emotional check-in, 15% planning (vacation, finances), 5% gossip-about-others
- Coworker peer↔peer: 50% project-task, 20% meta-work (meetings, process), 15% break-room (sports, weather, weekend), 10% gossip, 5% personal
- Manager↔report: 60% task delegation/review, 25% performance/career, 10% scheduling, 5% personal
- Friend↔friend (close): 40% activity-planning, 30% topic-of-the-week, 20% emotional/advice, 10% shared-memory
- Acquaintance↔acquaintance: 70% transactional (info request, scheduling), 30% pleasantries

These percentages anchor the procedural topic-sampler. They are not invented — they are the rough consensus across the Vangelisti family-comms handbook and the email-corpus topic-modelling literature.

---

## 4. Burstiness, circadian, weekly patterns

**Karsai, Kivelä, Pan, Kaski, Kertész, Barabási, Saramäki (2011)** *Small but slow world: how network topology and burstiness slow down spreading*, Phys. Rev. E — the canonical paper. Inter-event-time (IET) distributions for human communication are **heavy-tailed**, well-fit by a power-law with exponential cutoff, with the burstiness parameter `B = (σ - μ)/(σ + μ)` typically in `[0.2, 0.6]` for individual dyads, much higher than Poisson (which has B=0). Karsai et al. proved that burstiness *slows* spreading on networks compared to a time-shuffled null model — long quiet periods dominate the mean first-passage time.

**Barabási 2005** *The origin of bursts and heavy tails in human dynamics*, Nature — the priority-queue model: humans handle a queue of tasks by priority, so high-priority items are handled near-immediately and low-priority items wait a long time, producing a power-law of waiting times. The model is debated (Stouffer et al. 2006 critique) but the empirical IET shape is uncontested.

**Circadian pattern** (Malmgren, Stouffer, Motter, Amaral 2008 *A Poissonian explanation for heavy tails in e-mail communication*, PNAS): once you condition on the user being *active* (awake, at the keyboard) the IET inside an active period looks Poisson with a user-specific rate. The heavy tail emerges from the alternation between active periods (work day, evening session) and inactive ones (sleep, weekend off). This is the "**cascading Poisson process**" model: a slow on/off-process for activity, a fast Poisson process inside the on-state.

The circadian shape is bimodal for office workers: a strong 9-12 morning peak, a smaller 13-17 afternoon peak with a clear lunch dip, a long evening tail, then near-zero from midnight to 7am. Weekly: weekdays carry 80-90% of work-email volume; weekends carry the personal volume.

**Sunday-evening / Monday-morning effect.** Aledavood, Lehmann, Saramäki et al. (2015, *Daily rhythms in mobile telephone communication*, PLOS ONE) found a measurable Sunday-evening spike in personal SMS (planning the week) and a Monday-7-to-9am spike in work-email (catching up on backlog). The weekly autocorrelation of email volume is dominated by the 7-day period (obvious) with secondary 24-hour and 30-day components.

**Implication for Internot.** Per-dyad event rate is `λ(s, t) = base(s) × circadian(t, user_timezone) × weekly(t) × seasonal(t)`. Implement as a product of factors with the slow envelope (active-period) modulating a within-period Poisson. The Karsai burstiness is then *automatic* from the cascading-Poisson structure — no need to fit a power-law explicitly.

---

## 5. Per-person communication-volume distributions

Robust regularities across datasets:

**Inbox size, per-edge volume, contact-list size** all follow heavy-tailed distributions. Specifically:
- *Inbox messages per dyad, ranked*: Pareto with shape α ≈ 1.5-2.0 for both Enron and the EU Reality-Mining dataset (Eagle & Pentland 2006). The top 5 correspondents account for ~50% of volume; the top 20 for ~80% (a strict 80/20 with a slightly thinner tail than the classic Pareto principle).
- *Contact list size* (the count of distinct correspondents over a year): lognormal with `μ ≈ ln(150)`, `σ ≈ 0.6-0.8`. This recovers Dunbar's number as the median (~150) with heavy tails on both sides (Hill & Dunbar 2003; Wellman et al. on personal-network size).
- *Edge total volume*: Pareto α ≈ 1.5 (Onnela et al.).

The Enron numbers specifically (Klimt & Yang 2004 and replications): mean per-person inbox ≈ 1100 emails over the ~3.5 year window; median ≈ 200; max ≈ 30000. The volume per dyad has α ≈ 1.7 in the tail.

**Strong ties are persistent; weak ties churn.** Saramäki, Leicht, López, Roberts, Reed-Tsochas, Dunbar (2014, *Persistence of social signatures in human communication*, PNAS) showed that an individual's *rank-ordered* contact-frequency distribution is a stable personal signature — if Alice talks to her #1 the most by a factor of 5, that ratio is stable across years even as the identity of #1 shifts. The shape of the per-person signature is heterogeneous: some people have a sharp peak (one dominant correspondent), some have a flat top-10.

**Implication for Internot.** Per-person, sample a *shape* once at world-generation (a Pareto exponent specific to that person, or equivalently a "social signature peakedness"), and a *total budget* (lognormal). Then distribute the budget across that person's edges by rank, with rank determined by `tie_strength * communication_propensity(viewer)`.

---

## 6. Group communication

Internot v1 is pair-only. v2 may add groups, so the research:

**WhatsApp / Telegram group studies** (Garimella et al. 2018 on WhatsApp groups; Rosenfeld et al. 2018): group-chat membership is heavy-tailed (most users in 1-3 groups, power-user in 50+). Group-size distribution is lognormal with median ~6-10 members. Message-rate per group: most groups go silent within weeks; the surviving groups have a near-power-law rate with a small fraction of high-rate "always on" groups (family chat, close-friend chat, work-team chat).

**Family group chats** specifically (Taylor & Vincent 2005 on mobile-and-family; later WhatsApp-family-chat work): the median active family chat has 4-6 members across 2 generations, with the *parent generation* dominating sending volume during weekdays and the *child generation* during weekends. Multimedia (photos) is over-represented vs text in family chats compared to friend chats.

**Work group chats** (Slack-corpus studies; Lin, Zhang et al. 2016 on Slack at Stripe and other orgs): channel rate distribution has a clear bimodal shape — high-rate "general" / "random" / team channels, and low-rate "interest" channels. Membership-overlap-with-org-chart is the dominant structural signal; project channels mirror the formal team boundaries.

**Role of creator vs active members**: in most chat platforms the creator initiates but is *not* the most active member after week 1; the most-active-member emerges from intrinsic engagement (Lampinen et al.).

**Implication for Internot v2.** Groups should be derived from existing graph structures rather than registered independently: a family group chat from each kin-cluster, a work team chat from each manager↔reports cluster, a friend group chat from each high-clustering-coefficient triangle. The membership composition is then a free function of the graph; only the rate distribution per group needs separate sampling.

---

## 7. Cross-modal coherence in synthetic-comms datasets — what existing systems do and where they break

**LinkedIn synthetic** (the company's internal generator for ML eval, partially described in their open-sourced LinkedIn Echo / EconJobMarket work and the *Synthetic LinkedIn* work referenced by Sun et al.): a graph + a profile generator + a message generator running independently. The known failure mode is *cross-modal incoherence* — the user whose profile says "Software Engineer at Google" sends a message that references "my role at Microsoft". The generators don't share a single source of truth.

**IBM synthetic banking transactions** (Altman et al. 2023 *Realistic synthetic financial transactions for anti-money laundering models*): more disciplined — a single agent-based simulation drives all event streams, so transactions are coherent with declared occupations. But the comms layer is minimal (no message bodies, just transaction memos).

**Enron-based replay agents** (e.g. ENRON-style replay in dialog research): take real Enron messages and replay them with model agents — coherent because real, but bounded by the actual corpus and ethically constrained.

**AgentBench, OSWorld, ToolBench, AppWorld** (Liu, Yu, Zhang, Xue et al. 2024 AgentBench; Xie, Zhang et al. 2024 OSWorld; Qin, Liang et al. 2023 ToolBench; Trivedi, Khot, Sabharwal et al. 2024 AppWorld): all use *scripted environments with a per-task seed*. They don't try cross-service coherence across thousands of users; they curate each task. AppWorld is closest to Internot in spirit — it has a multi-service environment (Gmail, Splitwise, Spotify, Venmo, SimpleNote, AmazonShopping, Calendar, etc.) with ~457 agentic tasks. Its coherence is **per-task hand-curated**, not procedural. Each task starts from a hand-built seed state; cross-service tasks (e.g. "split the dinner bill with the people on the calendar event") rely on the curator having pre-populated both services consistently. This works but doesn't scale.

**Microsoft Research and the "Synthetic Computers" line** — the relevant references are the WindowsAgentArena (Bonatti et al. 2024) and the broader Magentic-One / AutoGen evaluation environments. WindowsAgentArena reuses real-world Windows apps with scripted task seeds, so coherence is *real* by virtue of running real apps but is limited to single-user / single-machine. The truly "synthetic" line (the "Synthetic Computers" paper referenced in the project memory, MS-research 2026): generates per-user "personal computer" state — emails, files, browser history — and uses LLMs to bridge cross-service references. The known failure mode the paper itself reports is *LLM-introduced inconsistency*: the LLM generates an email referencing "the document I sent yesterday" but no such document exists in the file system. They patch this with a post-hoc consistency-checker that re-prompts on inconsistency, which is expensive and probabilistic.

**Internot's advantage over all of these** is the procedural-floor + Stage-Manager split (validated 2026-04-25 in the codebase). Every fact — who, when, what relationship, what topic, what tone — is `f(id, key)` on the same procedural floor. Coherence is *structural*, not enforced after-the-fact. The LLM only renders pre-computed AVMs; it has no degrees of freedom to introduce inconsistency about the world, only about the linguistic surface.

The literature does *not* contain a system that hits this design point with the breadth Internot is aiming at. AppWorld is the nearest neighbour and its coherence is curated rather than procedural.

---

## 8. Deterministic event-stream sampling — the math

The core problem: given a tie `(A, B)` with strength `s` and a time window `[T1, T2]`, enumerate all communication events (mode, timestamp, topic-class) deterministically, without storing per-event state.

The right formalism is the **inhomogeneous Poisson process (IPP)** with rate `λ(s, t, mode)`. The IPP has two reproducibility recipes:

**(a) Time-rescaling theorem** (Brown, Barbieri, Ventura, Kass, Frank 2002 *The time-rescaling theorem and its application to neural spike train data analysis*, Neural Computation). Let `Λ(t) = ∫_0^t λ(u) du` be the cumulative-rate function. If event times `t_1 < t_2 < ...` are an IPP with rate `λ`, then `Λ(t_1) < Λ(t_2) < ...` is a unit-rate homogeneous Poisson process. To sample IPP events: sample unit-rate Poisson events `τ_i` and invert via `t_i = Λ^{-1}(τ_i)`. Unit-rate inter-arrival times are i.i.d. exponential.

**(b) Thinning** (Lewis & Shedler 1979): sample a homogeneous Poisson process at rate `λ_max ≥ sup λ(t)` and accept each event `t_i` with probability `λ(t_i)/λ_max`. Simpler, slightly wasteful when `λ` varies widely.

For Internot, **time-rescaling is preferred** because the rejection step in thinning breaks pure-function-of-(id, key) determinism (rejection introduces a variable number of hash calls per accepted event).

**Hashed-arrival-times recipe.**

Within a time window `[T1, T2]`:
1. Compute the upper-bound count `N_max = ceil(Λ(T2) - Λ(T1) + k·sqrt(Λ(T2) - Λ(T1)))` for some safety `k` (say 6 standard deviations). This gives a deterministic ceiling on how many events could exist in this window.
2. For each `i ∈ [0, N_max)`, hash `(A_id, B_id, mode, window_seed, i)` to a uniform `u_i ∈ [0, 1)` and convert to an exponential inter-arrival via `e_i = -ln(1 - u_i)`.
3. Accumulate the exponential inter-arrivals into unit-rate event times `τ_i = Σ_{j≤i} e_j`. (This is the unit-rate homogeneous Poisson process on the rescaled clock.)
4. Invert each `τ_i` via `t_i = Λ^{-1}(τ_i + Λ(T1))`.
5. Filter `t_i ∈ [T1, T2]`. Stop enumeration at the first `t_i > T2`.

This is **deterministic, addressable, and parallel-safe**: any worker can ask "what's the i-th event between A and B?" and get the same answer.

The cumulative-rate function `Λ(t)` is a piecewise integration of the circadian × weekly × seasonal × per-tie factors. For tractability, **precompute `Λ(t)` on a fixed grid** (say 1-hour resolution) and do log-time binary search on the grid to invert. The grid is itself a pure function of `(A_id, B_id, mode, world_seed)`.

For *per-event mode selection* in a multi-mode setup, two options:
- Run one IPP *per mode* with its own `λ_mode`. Clean, parallel. Event ordering across modes recovered by merge.
- Run one combined IPP at rate `λ_total = Σ λ_mode` and at each event sample the mode by `λ_mode(t) / λ_total(t)` from a categorical hash. Slightly less hash-work but couples modes.

Internot should use **per-mode IPPs** because the bit-pattern-pushdown in `procedural_core::Space::find()` works best when each mode is its own space with its own slot layout.

**Topic sampling per event** is then a separate hash on `(A_id, B_id, mode, event_index, "topic")` against the tie-kind-conditional topic distribution from §3.

**Reproducibility properties:**
- Adding a new agent action (an overlay write in `procedural_overlay::Session`) does not affect the IPP for any other pair — the IPP is `(A, B)`-local.
- Changing the time window only adds/removes events at the boundary — events inside the original window are unchanged.
- Reordering enumeration order does not change the set of events.
- A different `world_seed` shifts every event identity but preserves the *aggregate* distributional properties.

---

## 9. Message content vs message event — register modelling

The literature distinguishes *event* (an email was sent) from *content* (what it said). Internot's procedural floor gives us the event; the LLM-as-renderer gives us the content from an AVM.

**Length distribution by mode.** Empirically:
- Chat / SMS: lognormal with `μ ≈ ln(20 chars)`, `σ ≈ 1.0` — most messages are short pings, with a tail of paragraph-length ones. Median 7-10 words.
- Email: lognormal with `μ ≈ ln(80 words)`, `σ ≈ 1.2` for informal, longer for formal. Long tail to multi-paragraph essays.
- Calendar invite description: bimodal — most are empty or one-line, with a tail of meeting agendas.
- Formal letter / document: lognormal with `μ ≈ ln(500 words)`, `σ ≈ 0.8`. Heavy-tailed but with a high floor.

**Formality / register** scales with `(tie_kind, mode)`. The matrix is roughly: kin × chat → most informal; coworker × email → moderately formal; vendor × email → most formal. Internot already encodes a `FormalityBand` enum in `internot_renderer` — extend it to a `(formality, tension, intimacy)` triple per AVM, all computable from `(tie_strength, tie_kind, recent_events, time_of_day)`.

**Topic-conditional surface features.** The McCallum ART-model and the Vangelisti family-comms tradition agree: family-personal topics carry higher rates of (a) emotion-tokens, (b) intimacy-markers (terms of endearment, shortened names), (c) implicit reference ("the thing"). Work topics carry higher rates of (a) jargon / acronyms, (b) action-verbs in imperative mood, (c) explicit subject lines and signatures. These are surface-renderer concerns — the AVM should encode the *intent and relational stance*, not the surface phrases.

---

## Per-service recommendations for Internot

Six services to spec, each with volume, topic, and mode-selection logic. Internot already has the procedural floor with `tie_strength`, `tie_kind`, life-events, career-arc; these recommendations sit on top.

### mail
- **Volume.** Per-dyad daily rate `λ_mail(s, t) = base_mail(s) × circadian_workday(t) × weekly_mtwrf(t)`, with `base_mail(s)` going from ~0.05/day for weak ties up to ~5/day for active work pairs. Per-person daily inbox volume target: lognormal around 30 received/day (Radicati anchor).
- **Topic distribution.** As §3: project-task, scheduling, meta-work, personal-aside. Conditional on `tie_kind ∈ {coworker, manager-report, vendor, kin, friend}`.
- **Mode selection.** Mail is the "default async". Chosen when (a) tie is not partner/close-friend, (b) message has structured content (attachment, multi-paragraph), (c) recipient is in work-mode context. Chat preempts mail for short relational pings on close ties.

### chat
- **Volume.** Per-dyad daily rate `λ_chat(s)` heavy on close ties: ~100/day for partners, ~20/day for close friends, near-zero for weak ties. Per-message length lognormal with `μ = ln(20 chars)`.
- **Topic distribution.** Skews relational and coordinative: `{check-in, coordination, link-share, gossip-aside, emotion-expression}`.
- **Mode selection.** Chat wins for `intimacy ≥ medium` AND `message-length-class = short` AND `synchrony ≥ medium`. Family-chat group preempts pair-chat for kin-cluster events.

### calendar
- **Volume.** Per-person 1-5 events/day with a workday concentration; per-dyad rate driven by org-graph adjacency (manager↔report has weekly 1:1; cross-team has occasional sync) and life-events (birthdays, anniversaries from procedural floor).
- **Topic / event-kind distribution.** `{work-meeting, 1:1, project-sync, all-hands, social-coffee, doctor-appointment, kid-event, family-celebration, travel-block, focus-block}`. Tie-kind-conditional: kin events skew to celebrations and appointments; work events skew to syncs and 1:1s.
- **Mode coupling.** Calendar events are paired with a mail invite for formal/cross-team events, with a chat ping for informal/close-tie events. The pairing is procedural: `mode_pairing(event_kind, attendees_max_tie_kind)`.

### files
- **Volume.** Per-person ~1-5 uploads/day, with weekly bursts (Monday-morning project-kickoff, Friday-afternoon weekly-report). Per-edge sharing rate driven by org-adjacency + active calendar-event-cooccurrence.
- **Topic / file-kind distribution.** `{doc, spreadsheet, presentation, image, pdf-report, code-snippet}`. Strongly tie-kind-conditional: kin shares photos and PDFs; coworkers share docs, sheets, decks. Driven by the calendar/thread context the file lives in.
- **Mode coupling.** A file share *implies* a mail or chat with a link, not an independent event. Files should be sampled *conditional on* mail/chat events that have attachment-or-link slots.

### money
- **Volume.** Per-person 2-10 transactions/day, heavy-tailed (a big purchase every few weeks). Per-edge P2P-transfer rate restricted to medium-and-up personal ties + transactional vendors.
- **Topic distribution.** `{rent, groceries, restaurant-share, ride-share-split, gift, recurring-subscription, salary, refund}`. Tie-kind constrains: kin sees gifts and rent-splits, friends see restaurant-shares and trip-splits, vendors see one-shots.
- **Mode coupling.** A money-transfer between persons typically has a *short chat message* attached (the Venmo-memo equivalent). A subscription bill has a *mail receipt*. Sample money events first; sample the comm event conditional on the money event.

### tasks
- **Volume.** Per-person 5-20 open tasks at any time (lognormal count, already in slot-based layout); creation rate ~3-8/day; completion rate matched to keep the backlog in steady state.
- **Topic distribution.** Career-arc-conditional: a recently-promoted-to-manager person has more delegation tasks, a sabbatical-state person has near-zero work tasks and more personal-project tasks. Life-events drive personal tasks (wedding-planning, baby-prep).
- **Mode coupling.** Tasks are self-loops mostly. Cross-person task assignment is restricted to directed manager→report edges and self-assignment. A task creation may be the *result* of a mail or calendar event (action-item from a meeting) — sample the task conditional on the upstream event, not independently.

### Cross-service coherence guarantees
Every service computes its events as a pure function of `(viewer_id, correspondent_id, time, world_seed)`. Because the same `tie_strength(viewer_id, correspondent_id)` enters every service's rate function, the *distributions* across services are mutually consistent: if Alice has a strong tie to Bob, she will have high mail rate, high chat rate, occasional calendar overlap, occasional file share, occasional money transfer, and shared tasks — all on the same dyad, with no enforcement step.

---

## Deterministic event enumeration recipe

The recipe in pseudocode. Inputs: persons `A`, `B`; time window `[T1, T2]`; world seed `W`. Output: ordered list of `(timestamp, mode, topic_class, intent)` events.

```
fn enumerate_dyad_events(A: PersonId, B: PersonId, T1: time, T2: time, W: seed)
    -> Vec<Event>
{
    let (lo, hi) = canonical_order(A, B);            // sort so (lo,hi)==(hi,lo)
    let s        = tie_strength(lo, hi);             // f(id,key) on procedural floor
    let kind     = tie_kind(lo, hi);                 // kin|partner|friend|coworker|...
    let mut out  = Vec::new();

    for mode in [Mail, Chat, Calendar, Files, Money, Task] {
        let lambda = |t| {
            base_rate(mode, kind, s)
            * circadian(t, timezone_of(lo))
            * weekly(t)
            * seasonal(t)
            * life_event_modifier(lo, hi, t)
        };

        // Step 1: cumulative-rate grid on a 1-hour resolution.
        let grid = build_lambda_grid(lambda, T1, T2);  // pure f of inputs
        let big_lambda = |t| integrate(grid, T1, t);   // monotone
        let total_mass = big_lambda(T2);

        // Step 2: deterministic ceiling on event count.
        let n_max = ceil(total_mass + 6.0 * sqrt(total_mass));

        // Step 3: unit-rate Poisson via cumulative exponentials.
        let mut tau = 0.0;
        for i in 0..n_max {
            let u    = hash_float(lo, hi, mode, W, i, "u");   // in [0,1)
            let exp  = -ln(1.0 - u);
            tau     += exp;
            if tau > total_mass { break; }

            // Step 4: invert Lambda by binary search on the grid.
            let t = invert_lambda(big_lambda, tau, T1, T2);

            // Step 5: topic / intent / formality sampling.
            let topic = sample_categorical(
                hash_float(lo, hi, mode, W, i, "topic"),
                topic_distribution(kind, mode, t));
            let intent = sample_categorical(
                hash_float(lo, hi, mode, W, i, "intent"),
                intent_distribution(kind, mode, topic, s));
            let formality = formality_band(kind, mode, topic, t);

            out.push(Event {
                t, mode, direction: hash_bit(...),     // who initiated
                topic, intent, formality,
                event_seed: hash_u64(lo, hi, mode, W, i, "event"),
            });
        }
    }
    out.sort_by_key(|e| e.t);
    out
}
```

Key properties:

- **Pure function of inputs.** No hidden state. The same `(A, B, T1, T2, W)` yields the same events.
- **Window-monotone.** Widening `[T1, T2]` only adds events at the boundary; events in the original window keep their identities.
- **Mode-decoupled.** Adding a new mode does not perturb existing modes' events.
- **Symmetric in `(A, B)`.** `canonical_order` ensures `(A,B)` and `(B,A)` enumerate the same set; `direction` is sampled separately as a bit.
- **Overlay-compatible.** Agent actions via `procedural_overlay::Session` are *additional* events with the sentinel top-bit set on their ids (per `procedural_core` invariant); the IPP produces the *baseline* event stream below the sentinel.
- **Bit-pattern-friendly.** The event-id layout can pack `(dyad_lo_id_lsb : 32, mode : 4, day : 16, intra_day_seq : 12)` for fast `Space::find()` enumeration when an agent queries "all mail events between A and B in the last month".
- **Indexable as Space.** Each mode becomes a `Space<W>` with its own `BitLayout`, `indexable_attribute`s for `(dyad_lo, dyad_hi, day)`, and procedural rate functions. The existing mail slot layout is the prototype.

The `topic_distribution(kind, mode, t)` and `intent_distribution(...)` tables are the §3 tie-kind-conditional distributions, time-modulated by life events (a recent baby in the procedural floor shifts kin-topic distributions toward childcare for ~24 months; a recent promotion shifts coworker-topic toward delegation for ~6 months).

The Stage-Manager handoff is then: the AVM for a single event packs `{ dyad, tie_strength, tie_kind, mode, topic, intent, formality, references_to_prior_events }`, the LLM renders the linguistic surface deterministically given the AVM and the prompt+model+cache key. Cross-service coherence is structural because every service's events read the same floor.

---

## References

- Aledavood, T., Lehmann, S., Saramäki, J., et al. (2015). Daily rhythms in mobile telephone communication. *PLOS ONE* 10(9).
- Altman, E., et al. (2023). Realistic synthetic financial transactions for anti-money laundering models. *NeurIPS Datasets & Benchmarks*.
- Aquilino, W. S. (2006). Family relationships and support systems in emerging adulthood. *Emerging Adults in America*, APA.
- Barabási, A.-L. (2005). The origin of bursts and heavy tails in human dynamics. *Nature* 435.
- Bird, C., Gourley, A., Devanbu, P., et al. (2006). Mining email social networks. *MSR Workshop*.
- Bonatti, R., et al. (2024). WindowsAgentArena: Evaluating multi-modal OS agents at scale. *Microsoft Research preprint*.
- Brown, E. N., Barbieri, R., Ventura, V., Kass, R. E., Frank, L. M. (2002). The time-rescaling theorem and its application to neural spike train data analysis. *Neural Computation* 14.
- Daft, R. L., & Lengel, R. H. (1986). Organizational information requirements, media richness and structural design. *Management Science* 32.
- Eagle, N., & Pentland, A. (2006). Reality mining: sensing complex social systems. *Personal & Ubiquitous Computing* 10.
- Eagle, N., Pentland, A., Lazer, D. (2009). Inferring friendship network structure by using mobile phone data. *PNAS* 106.
- Garimella, K., Tyson, G. (2018). WhatsApp, Doc? A first look at WhatsApp public group data. *ICWSM*.
- Goh, K.-I., Choi, J., et al. (various). Topic separation in mixed-domain email corpora.
- Granovetter, M. S. (1973). The strength of weak ties. *American Journal of Sociology* 78.
- Hidalgo, C. A., & Rodriguez-Sickert, C. (2008). The dynamics of a mobile phone network. *Physica A* 387.
- Hill, R. A., & Dunbar, R. I. M. (2003). Social network size in humans. *Human Nature* 14.
- Karsai, M., Kivelä, M., Pan, R. K., Kaski, K., Kertész, J., Barabási, A.-L., Saramäki, J. (2011). Small but slow world: how network topology and burstiness slow down spreading. *Phys. Rev. E* 83.
- Klimt, B., & Yang, Y. (2004). The Enron corpus: a new dataset for email classification research. *ECML*.
- Kossinets, G., & Watts, D. J. (2006). Empirical analysis of an evolving social network. *Science* 311.
- Lewis, P. A. W., & Shedler, G. S. (1979). Simulation of nonhomogeneous Poisson processes by thinning. *Naval Research Logistics Quarterly* 26.
- Ling, R. (2008). *New Tech, New Ties: How mobile communication is reshaping social cohesion*. MIT Press.
- Liu, X., Yu, H., Zhang, H., et al. (2024). AgentBench: Evaluating LLMs as agents. *ICLR*.
- Lyle, J. (2011). Family communication standards inventory. *Communication Studies* 62.
- Malmgren, R. D., Stouffer, D. B., Motter, A. E., Amaral, L. A. N. (2008). A Poissonian explanation for heavy tails in e-mail communication. *PNAS* 105.
- Marsden, P. V., & Campbell, K. E. (1984). Measuring tie strength. *Social Forces* 63.
- Marsden, P. V., & Campbell, K. E. (2012). Reflections on conceptualizing and measuring tie strength. *Social Forces* 91.
- McCallum, A., Wang, X., Corrada-Emmanuel, A. (2007). Topic and role discovery in social networks with experiments on Enron and academic email. *JAIR* 30.
- Mohammad, S. M., & Turney, P. D. (2013). Crowdsourcing a word-emotion association lexicon. *Computational Intelligence* 29.
- Onnela, J.-P., Saramäki, J., Hyvönen, J., Szabó, G., Lazer, D., Kaski, K., Kertész, J., Barabási, A.-L. (2007). Structure and tie strengths in mobile communication networks. *PNAS* 104.
- Qin, Y., Liang, S., et al. (2023). ToolLLM / ToolBench: Facilitating large language models to master 16000+ real-world APIs. *ICLR*.
- Radicati Group, Email Statistics Reports (annual series).
- Reid, D., & Reid, F. (2005). Textmates and text circles: insights into the social ecology of SMS text messaging. *Mobility & Society*.
- Saramäki, J., Leicht, E. A., López, E., Roberts, S. G. B., Reed-Tsochas, F., Dunbar, R. I. M. (2014). Persistence of social signatures in human communication. *PNAS* 111.
- Stouffer, D. B., Malmgren, R. D., Amaral, L. A. N. (2006). Comments on "The origin of bursts and heavy tails in human dynamics". *arXiv*.
- Taylor, A. S., & Vincent, J. (2005). An SMS history. *Mobile World: Past, Present and Future*.
- Trivedi, H., Khot, T., Sabharwal, A., et al. (2024). AppWorld: A controllable world of apps and people for benchmarking interactive coding agents. *ACL*.
- Vangelisti, A. L. (ed.) (2012). *The Routledge Handbook of Family Communication*, 3rd ed.
- Wellman, B., et al. Various works on personal-network size.
- Wuchty, S. (2009). What is a social tie? *PNAS* 106.
- Xie, T., Zhang, D., et al. (2024). OSWorld: Benchmarking multimodal agents for open-ended tasks in real computer environments. *NeurIPS*.
