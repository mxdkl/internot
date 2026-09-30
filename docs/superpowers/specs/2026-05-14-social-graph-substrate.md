# Social-graph substrate — v1 (household-only)

**Date:** 2026-05-14
**Status:** Design (pre-implementation)
**Supersedes:** ad-hoc `family.rs` / `cohort.rs` / pair-thread / pair-DM correspondent logic in `internot/src/{people, mail, chat}/`.
**Research dossier:** [`docs/superpowers/research/2026-05-14-*.md`](../research/) (5 files, ~22k words).

## 1 — Motivation

The current Internot graph is structurally wrong in four ways that compound:

1. **Edges between strangers exist.** `mail::thread_for_pair(a, b)` and `chat::dm_for_pair(a, b)` return a valid thread/DM for *any* `(a, b)` of mail_ids. Real social graphs are sparse: ~150 stable acquaintances per person at most (Dunbar, Hill & Dunbar 2003); ~5 strong ties (Mac Carron, Kaski & Dunbar 2016 measured 4.1 in 6 B mobile-call records). Today Internot offers 4.3 B × 4.3 B / 2 ≈ 10¹⁹ potential threads. Nineteen orders of magnitude is the gap between observed reality and current substrate.

2. **Per-person edge seeds break reciprocity.** `internot/src/people/family.rs` admits this in its docstring: "A's spouse search resolves to B, but B's spouse search may resolve to C." Each person carries their own seed for spouse/child identity; a brute-force compatibility loop (256 trials, cached) finds a real person who fits. Two independent searches → reciprocity is not guaranteed. The same problem exists everywhere edges are derived from per-endpoint seeds.

3. **Topics are random hashes of participant pairs.** `topic_seed = hash_int(pair_key, "thread_topic_seed_v1", ...)` produces topic indices unrelated to *why* the two people communicate. Real comms topics emerge from shared context — workplace project, family logistics, school. The Stage Manager paradigm (CLAUDE.md, validated 2026-04-25) requires every fact to derive from procedural-floor primitives; topic-from-pair-hash violates this.

4. **No cross-service coherence.** Each service independently derives correspondents. Someone in your mail inbox is not necessarily in your chat, calendar, money flows, or shared files. Real life is the opposite: people you mail also chat with, meet, share files with.

These compound: random edges → random topics → no coherence → no realism. Patching any one service in isolation can't fix the substrate failure.

The fix is a graph layer with one load-bearing principle:

> **Edges are shared objects, not per-endpoint seeds.**
> Two people share a household (or workplace, or school, in later versions); both query the same shared object; both see each other. Reciprocity is structural by construction.

## 2 — Empirical anchors

Numbers the v1 implementation should hit, sourced from `docs/superpowers/research/2026-05-14-real-social-graph-structure.md`. These are sanity-check targets, not exact specs:

| Property | Real (mobile-call / FB / NSS data) | v1 (household-only) expectation |
|---|---|---|
| Mean strong-tie count per person (Dunbar inner layer) | 4.1 (Mac Carron 2016) | 3–5 (varies with household size + life stage) |
| Kin share of strong ties (adult Western data) | 44–54% (McPherson 2006, Bidart 2018) | ~100% in v1 (household-only — v2 introduces non-kin) |
| Reciprocity ratio (out-edges that are mutual) | ≥ 0.95 in symmetric-tie data | 1.0 (structurally guaranteed) |
| Clustering coefficient (whole graph) | 0.10–0.35 (Onnela 2007 phone, Ugander 2011 FB) | Higher in v1 (cliques: households are fully connected) — track but expect ~0.6+ |
| Mean strong-tie strength, cohabiting partner age 40 | ~0.9 normalized (Roberts & Dunbar 2011) | 0.9 ± 0.05 |
| Burstiness B in inter-event times | 0.4–0.7 (Karsai et al. 2011) | 0.4–0.6 (cascading-Poisson model) |
| Heavy-tailed inter-event time exponent α | ~3/2 (Barabási 2005 priority-queue model) | ~3/2 ± 0.2 |

`internot/tests/social_validation.rs` will assert these via property-style checks on a sampled population.

## 3 — Goals and non-goals

### Goals

- **Realistic social graphs that are deterministic.** Every tie and every communication event is `f(id, key, t)`; no stored state on the procedural floor; reciprocity automatic.
- **One graph, all services consume it.** Mail, chat, calendar, files, money, tasks all read from `internot::social`. Cross-service coherence is automatic — they share the same edge set and the same comm-volume function.
- **Monoplex with typed edges.** A single graph; each edge carries `(kind, strength, since, ...)` rather than `N` parallel layers. Endorsed by `2026-05-14-real-social-graph-structure.md` — real-world layers (family / coworker / classmate) co-occur on the same pairs ~70–80% of the time; storing them separately wastes state and breaks integration.
- **Framework graph primitives in `procedural_core`; ontology in `internot`.** Pattern matches the existing `procedural_core::space` vs `internot/src/people` split.
- **Household-only for v1.** Nail the substrate on one venue class before extending. The framework primitives (`VenueSpace`, tie-strength trajectories, deterministic event-enumeration) get exercised end-to-end on household and are then reusable verbatim for workplace / school / neighborhood in v2+.
- **Clean separation: `people` = individuals only.** All family/social logic leaves `people` and consolidates in `internot::social`. The dependency arrow is `social → people`, never reversed.

### Non-goals (explicit list)

- Workplace, school, neighborhood, online-community, friend, vendor venues — **v2+, additive**, on the same framework primitives.
- Extended family beyond parents / partner / children / siblings (no grandparents / in-laws / cousins in v1).
- Minors (under-18) as full population members — they remain household-attached stubs (name + age + gender). The `MIN_AGE=18` invariant is preserved.
- Group threads / group DMs / shared rooms — pair-only DMs within household ties stay; group comms are v2.
- Hyperbolic latent-space for weak/elective ties — **v2+ direction**, documented in §10.
- LLM-as-judge, agent convenience tools — out of scope per `depth-over-breadth` (CLAUDE.md).
- Backwards compatibility with current mail/chat/calendar/files/money/tasks views — none. All six services are deleted and rewritten.

## 4 — Architecture

Three layers; dependency arrows point downward only:

```
internot/src/{mail, chat, calendar, files, money, tasks}/    (rewritten as graph consumers)
                          │
                          ▼
internot/src/social/                                         (v1 household model)
                          │
                          ├──► internot/src/people/          (cleaned: no family/cohort code)
                          │
                          ▼
procedural_core/src/graph/                                   (NEW framework primitives)
                          │
                          ▼
procedural_core/{bits, hash, sampler, trajectory, edge, space, world}
```

`people` does not depend on `social`. `social` reads life-event *dates* and personality from `people`. Comms services read `social` only — they do not reach into `people` for graph facts.

The framework / ontology split mirrors today's `procedural_core::space` (framework) vs `internot/src/people` (ontology) division. What graduates to core: the abstract graph machinery any procedural world could reuse. What stays in internot: which venue types exist, what tie kinds matter, the demographic numbers for our universe.

## 5 — `procedural_core::graph` (framework primitives)

Domain-free machinery. Every consumer plugs in its own venue types, role enums, and tie-strength profiles; the core provides the deterministic-procedural mechanism.

### 5.1 `VenueSpace<W>`

Generalization of today's `workplace_members_of(industry, city, workplace_seed)` cohort pattern. Registers a `Space<W>` whose entities are *(member_id, venue_id, role, intra_order, window_start, window_end)* tuples, with `venue_id` and `member_id` both indexable. Exposes:

```rust
fn members_of(world: &World<W>, venue_id: u64, t: DateTime<Utc>) -> Vec<u32>;
fn venues_of(world: &World<W>, member_id: u32, t: DateTime<Utc>) -> Vec<u64>;
fn role_of(world: &World<W>, member_id: u32, venue_id: u64, t: DateTime<Utc>) -> Option<Role>;
```

Member enumeration uses `Space::find()` bit-pattern pushdown on the indexable `venue_id` — O(|members in venue|), not O(|population|). Reverse lookup is symmetric.

The window fields (`window_start`, `window_end`) carry the active interval for each membership entity. Life-event transitions (marriage forming a new household, divorce dissolving one) close one entity and open another rather than mutating. Both old and new memberships remain queryable at their respective times.

### 5.2 `Tie`

```rust
pub struct Tie {
    pub peer_id: u32,
    pub kind: u8,                 // venue-specific role encoding
    pub venue_id: u64,
    pub strength: f64,            // [0.0, 1.0]
    pub since: DateTime<Utc>,
    pub last_proc_contact: DateTime<Utc>,
}
```

`kind` is `u8`-encoded; the consumer (`social::HouseholdRole`) defines what each value means. The core type stays domain-free.

### 5.3 Tie-strength trajectories

`tie_strength(profile, t) -> f64` returns the procedural strength of a tie at time t, composing existing `procedural_core::trajectory::{step, smooth}` primitives so stability radii stack via `stability::min_of`. The profile carries the parameters specific to a tie kind; consumers supply it.

Composable shape (consumer-instantiated):

```
s_ij(t) = clamp01(
    s_base
    + s_kin_step(t)              // step-on at relationship start, step-off at end
    + s_cohabit_pulse(t)          // peak during cohabitation window, decay after
    + Σ venue_decay_k(t)          // additional decay terms per shared past venue
)
```

Decay terms follow `s_peak · exp(-(t - venue_end) / τ_kind)`, with empirical τ from the temporal-networks research:

| Tie kind | Peak | Floor | τ (years) | Source |
|---|---|---|---|---|
| Partner, cohabiting | 1.0 | 0 (post-divorce) | — | by construction |
| Partner, post-divorce | 0.3 → 0.1 | 0.1 | 0.5 | Roberts & Dunbar 2011 |
| Parent ↔ child, post move-out | 0.9 → 0.5 | 0.5 | 5 | Roberts & Dunbar 2011 |
| Sibling, post move-out | 0.7 → 0.4 | 0.4 | 10 | Saramäki et al. 2014 |
| Ex-coworker (v2) | 0.6 → 0.1 | 0.1 | 1.5 | Burt 2000 |
| Ex-classmate (v2) | 0.5 → 0.2 | 0.2 | 4 | Bidart-Degenne-Grossetti 2011 |

### 5.4 Deterministic event-stream enumeration

The load-bearing primitive that makes comms procedural. Given a tie `(A, B)` with continuous-time intensity function `λ(A, B, t)` derived from tie strength + personality + circadian + weekly modulation, enumerate events in `[T₁, T₂]` deterministically via the **time-rescaling theorem** (Brown et al. 2002) and **hash-derived uniforms**:

```text
canonical (A, B) so that comm_events(A, B, ·) is symmetric in arguments
Λ(t) = ∫_{birth(A) ∨ birth(B)}^{t} λ(A, B, s) ds      // cumulative intensity, monotone
For k = 0, 1, 2, ...:
    u_k = hash_float((A, B, "iet", k), namespace)     // iid uniform via hash
    τ_k = -ln(u_k)                                    // exponential, rate 1
    Λ_event_k = Λ(birth) + Σ_{j ≤ k} τ_j
    t_k = Λ⁻¹(Λ_event_k)                              // back to real time
    if t_k ∉ [T₁, T₂]: skip
    emit (t_k, sender, mode, topic_seed)              // sender/mode/topic from hash subkeys
```

Properties:

- **Pure function.** Same `(A, B, T₁, T₂)` → same event list, every call, every process.
- **Window-local cost.** `O(events in [T₁, T₂])`, not `O(events since birth)`. Binary-search on Λ to find the first event past T₁.
- **Slice-recombinable.** Querying `[T₁, T₂] ∪ [T₂, T₃]` yields the same event set as `[T₁, T₃]`.
- **Overlay-composable.** Session writes (compose, reply) layer over procedural events without disturbing the procedural enumeration.

Cumulative-rate inversion `Λ⁻¹` is done via piecewise-linear interpolation on a precomputed grid (cached per `(A, B)` pair at coarse granularity). Discontinuities in `λ` (life events: marriage start, divorce, partner death) are explicit grid knots.

### 5.5 Deterministic stable-roommates pairing

For pair-formation passes (most importantly, marriage partner assignment within a cohort), the core provides Irving's **stable-roommates algorithm** (Irving 1985) — the gender-agnostic generalization of Gale-Shapley. Given:

- An ordered candidate cohort (a list of marriage-eligible person ids at time t in a region),
- A preference function `pref(a, b) -> f64` over (age compatibility, geographic proximity, education compatibility, personality compatibility),

returns `Vec<(u32, u32)>` of stably-matched pairs, canonical small-id-first. The algorithm is O(n²); cohort sizes are capped by region+year filtering, so the absolute cost is bounded. Output is cached process-wide on `(cohort_id, t)`.

Symmetry property: `(a, b) ∈ output ↔ (b, a) ∈ output` by construction. **This is what fixes the family.rs reciprocity bug** — there are no two independent searches.

Irving stable-roommates over Gale-Shapley: chosen specifically to eliminate the heteronormative-binary assumption baked into current `family.rs`. Same-sex pairing falls out of gender-agnostic preference — no special-cased branch.

## 6 — `internot/src/social/` (v1 household model)

### 6.1 Modules

- `social/mod.rs` — `SocialService` impl; `views()` exporter; `Service` trait integration.
- `social/household.rs` — household derivation; `households` Space registration; `current_household_id_of(person_id, t)`.
- `social/cohort_matching.rs` — parent-generation stable-roommates pass; cached.
- `social/membership.rs` — `household_members_of(world, household_id, t)` via `VenueSpace`.
- `social/role.rs` — `HouseholdRole` enum; role derivation from age + intra-order.
- `social/tie.rs` — `ties_of(person_id, t)` — the canonical query consumed by every comms service.
- `social/strength.rs` — profile + `tie_strength(profile, t)` per tie kind.
- `social/intensity.rs` — `comm_intensity(tie, personality_pair, t) -> CommIntensity`.
- `social/events.rs` — deterministic per-mode event enumeration consumed by mail / chat / calendar / money / tasks.
- `social/minors.rs` — household-attached stubs for children < MIN_AGE.
- `social/views.rs` — `list_household`, `read_tie`, `read_family_tree`, `find_ties`.
- `social/tests/` — reciprocity, household-closure, monotonic-partner, validation-target property tests.

### 6.2 Household lifecycle

At any time t, a person belongs to exactly one *active* household, derived deterministically from their `person_id` and the dates in `people::life_events`:

- **Birth household.** Active from `birth_date(person_id)` to either `marriage_date(person_id)` or, if unmarried, indefinitely (until death in a future temporal scope). Siblings share it. Defined by their parents' marriage.
- **Adulthood household.** Formed at `marriage_date(person_id)`. Both partners' `household_id` becomes `household_for_marriage(self_id, partner_id, marriage_date)` — a symmetric function in `(self_id, partner_id)`. Children born to this couple inherit it as their birth household.
- **Post-divorce household.** If `divorce_date` is present in life-events, the adulthood household ends at `divorce_date`. Each partner forms a single-adult household. Minor children attach to the primary-custody parent (deterministic from a `custody_bit` derived from child's mail_id).

### 6.3 Birth-household derivation — the load-bearing function

Given a person X with birth-date `B`, find X's parents (and therefore household) deterministically. The chain:

1. **Identify parent generation.** Parents have birth-years in `[B - 45, B - 22]` — the range where they could plausibly be adults of parenting age at X's birth. Concretely, anchor on a *parent cohort year* P = `B - 28` (median parenting age) and the geographic region R derived from X's existing geography bits.

2. **Stable-roommates pass over the parent cohort.** At time t₀ = P + 28 (the cohort's median marriage age), all marriage-eligible adults of cohort × region run through `procedural_core::graph::stable_roommates_match`. Preference function: weighted sum over (age proximity, geographic proximity, education compatibility, personality compatibility — Big Five trait alignment). Output: stable pair-matching of the cohort. Cached process-wide on `(cohort_year, region)`.

3. **Fertility flow from each pair.** Each matched couple has a procedurally-determined child count via regional fertility distribution (lognormal anchored on US 2020 total fertility rate ≈ 1.78, region-tabled if data available). Each child's mail_id is derived from `(couple_canonical_id, child_index, child_index_in_year_cohort)` — must land in the right `(birth_year, region)` slot bucket so existing `people` derivations (name, personality, education, career, etc.) work unchanged.

4. **Reverse lookup: `birth_household_id_of(person_id)`.** Given arbitrary X, extract X's `(birth_year, region)` from existing `people` attributes, look up the cached parent-cohort matching, find the couple whose children-derivation includes X, return their household_id. Cached.

This chain is order-of-operations-sensitive: pairing happens *before* fertility derivation; both happen lazily on first query (any query that triggers them populates the cache for that `(cohort_year, region)`). Cold-start cost is one stable-roommates pass per touched cohort × region; subsequent queries within the same cohort are O(log) lookups.

**The 32-bit person slot stays intact** — `(industry:6, city:6, workplace_seed:8, member_idx:12)` is unchanged. Workplace_seed is retained for v2 workplace-cohort ties. Household-membership data lives in the registered `households` Space, not in the person slot.

### 6.4 The `households` Space

A new `Space<W>` registered alongside `people`. Entities are *(member_mail_id, household_id, role, intra_order, window_start_day, window_end_day)* tuples. Representative bit layout (exact widths finalized in the implementation plan):

```text
[member_mail_id:32 | household_id:32 | role:4 | intra_order:4 | window_start_day:16 | window_end_day:16]
```

- `household_id` registered as indexable → `Space::find(where_eq("household_id", H))` is pushdown-fast enumeration.
- `member_mail_id` registered as indexable → reverse lookup ("which households is X in") is equally fast.
- 32-bit household_id gives 4 G unique households globally across all cohorts — comfortably exceeds any plausible population.
- Window fields carry the membership's active interval. Life-event transitions close one entity and open another.

At API level `household_id: u64` for ergonomics; the registered field stores 32 bits.

### 6.5 `HouseholdRole`

```rust
pub enum HouseholdRole { Self_, Partner, Parent, Child, Sibling }
```

Roles derive from age relative to household founding year (parents' marriage_date) and intra-household birth-order. Members older than `marriage_date - 18y` are Parents; younger by ≥18y are Children of those parents; same generation as Self is Sibling (if not Partner). Birth-order disambiguates sibling sequence.

### 6.6 `ties_of(person_id, t)`

The canonical query. Returns the active `Tie` list. Per-tie strength uses the trajectory machinery + the empirical τ table from §5.3. v1 implementation walks current household + closes-out tie strengths from prior households (parents, ex-partner, ex-siblings).

```rust
pub fn ties_of(world: &World<W>, person_id: u32, t: DateTime<Utc>) -> Vec<Tie> {
    let mut out = vec![];
    // Active household (where you currently live)
    let current = current_household_id_of(person_id, t);
    for peer in household_members_of(world, current, t) {
        if peer == person_id { continue; }
        let role = role_in_household(person_id, peer, current, t);
        out.push(make_tie(person_id, peer, role, current, t));
    }
    // Persistent kin ties from prior households (parents you've moved out from,
    // siblings who've moved out): walk past household memberships, emit
    // ties with decayed strength.
    for past_hh in past_households_of(person_id, t) {
        for peer in household_members_of(world, past_hh, past_hh.window_end) {
            if already_emitted(peer) || peer == person_id { continue; }
            out.push(make_decayed_tie(person_id, peer, past_hh, t));
        }
    }
    out
}
```

### 6.7 Reciprocity invariant

For every `(A, B, t)`: `B ∈ peer_ids(ties_of(A, t))  ↔  A ∈ peer_ids(ties_of(B, t))`. Enforced as a property-style test on random samples.

Reciprocity is structural because: (a) household_id is a shared object; (b) past_households_of is symmetric — if A's past household includes B, then B's past household symmetrically includes A; (c) tie-strength `s_ij(t)` is symmetric in `(i, j)` by construction. No independent searches; no asymmetric outcomes.

## 7 — Service rebuilds

All six comms services are deleted and rewritten as graph consumers. Each consumes `social::ties_of(self_id, t)` to determine correspondents; `social::events::*_events_in_window(...)` for event streams. Nothing exists between strangers.

### 7.1 `mail`

- Threads exist only between tied people. `thread_for_pair(a, b)` returns `Some(thread)` only if `(a, b)` is a tie at `t`; otherwise `None`.
- Topic distribution per tie kind (anchored on `2026-05-14-communication-patterns.md` topic-by-kind percentages):
  - **Partner threads**: household-logistics 35% + finances 15% + kids 25% + relationship 15% + other 10%.
  - **Parent ↔ adult-child**: family-events 30% + health 20% + finances 15% + advice 25% + other 10%.
  - **Sibling**: family-events 35% + nostalgia-catchup 30% + shared-parents 15% + other 20%.
  - **Parent ↔ minor-child**: mostly stub renders (logistics: pickup, school).
- Volume from `social::events::mail_events_in_window` per `comm_intensity(tie, ...).mail_per_day`. Inter-event burstiness from Barabási-2005 cascading-Poisson — α ≈ 3/2.
- Mutating views unchanged in surface: `compose`, `reply`, `mark_read`, `archive`, `delete`. Constrained to operate over tied correspondents; non-tied attempts return a typed `ViewError::NoSuchCorrespondent`.

### 7.2 `chat`

- DMs only over strong ties (strength ≥ 0.6 at `t`, configurable). Subset of mail edges.
- Short-form; volume from `comm_intensity(tie, ...).chat_per_day`. Heavier-tailed bursts than mail (chat-mode α slightly < 3/2 per Karsai 2011).
- Topic distribution skewed transactional/immediate vs mail (logistics + quick coordination).
- Pair-only in v1 (no group rooms).

### 7.3 `calendar`

- Personal events (work appts, doctor) stay personal — outside the household-tie scope.
- **Household-shared events** become first-class: anniversary (anchored on marriage_date day-of-year), birthdays (each member's), family dinner (procedurally weekly, evening), kids' school events (procedurally seasonal), joint travel (procedurally seasonal + work-vacation aligned).
- Per-event surface unchanged: `get_schedule`, `rsvp`, `book_meeting`, `get_busy`, `find_meeting_slot`. `book_meeting` to a non-tied attendee is `NoSuchCorrespondent`.

### 7.4 `files`

- Owner files stay personal — owner-only.
- **Shared-with-household** becomes a flag on a file's slot bit; visibility extends to all cohabiting members at time t.
- New view: `list_household_drive(now)` returns the union of self-owned + currently-cohabited-shared files.

### 7.5 `money`

- Per-person accounts/subs unchanged.
- **Joint accounts** for cohabiting partners — derived from marriage_date + household. Specific accounts (joint checking, shared savings) appear at marriage, disappear at divorce.
- **Intra-household transfers** as procedural transactions: allowance to child (recurring, weekly/monthly), gift to parent (occasional, anchored on birthdays / anniversaries), shared rent contribution.
- Subscription overlap: streaming services on a joint plan are listed once at household level, not twice per partner.

### 7.6 `tasks`

- Personal tasks unchanged.
- **Shared household tasks** (groceries, chores, kids' school logistics) visible to all cohabiting members.
- Status semantics (time-varying) unchanged.
- Mutating views: `create_task` for household — task visible to all current household members; `mark_done` requires task be self-owned or household-shared.

### 7.7 Cross-service coupling

For coherence, certain events on one tie ride other modes:

- A file share between A and B → optionally accompanied by a mail or chat event referencing the file.
- A money transfer between A and B → optionally accompanied by a chat memo.
- A calendar invite from A to B → optionally accompanied by a mail invitation.

These are procedural, not stored — the deterministic event-stream enumeration emits both events with the same `(tie, hash_subkey)` derivation so they're linkable.

## 8 — `people` cleanup

Files and what happens to them:

| File | Action |
|---|---|
| `internot/src/people/family.rs` | DELETE |
| `internot/src/people/cohort.rs` | DELETE (workplace-cohort logic moves to `procedural_core::graph::VenueSpace`; consumers re-point) |
| `internot/src/people/slot/tier3.rs` (XOR-encoded neighbor mail_ids) | DELETE; bits 256..384 of the U512 freed |
| `internot/src/people/slot/populate.rs` family-seed lines | DELETE |
| `internot/src/people/life_events.rs` | KEEP — partner/child *identity* fields removed; only *dates* stay (marriage_date, divorce_date, child_birth_dates) |
| `internot/src/people/{names, education, career, geography, languages, daily_life, timezones, tables}` | UNTOUCHED |
| `internot/src/people/views.rs` | TRIM — family-touching views move to `social/views.rs`; `read_career_history` etc. stay |
| `internot/tests/people_u512_round_trip.rs` | UPDATE — family round-trip assertions now go through `social::` |

The 32-bit person slot stays exactly as today. The freed Tier 3 bits (256..384, ~128 bits) are reserved for future people-internal use (no immediate consumer); not used to encode household state. Household state lives entirely in the registered `households` Space.

## 9 — Cross-service coherence guarantees

Concretely the v1 design guarantees:

1. **Existence reciprocity.** For every cohabiting partner pair (A, B), all six services show cross-references in both directions: A's mail/chat/calendar/files-shared/money/tasks contain B; B's contain A; at the same `t`.

2. **Temporal coherence.** Crossing `divorce_date(A, B)`, all six services drop cross-references at the same `t`. Tie-strength function decays continuously; service-level visibility cuts off at the household-membership-window boundary.

3. **Topic coherence per tie kind.** Mail and chat topics on a tie share the same distribution profile derived from tie kind, age cohort, life-stage. They co-vary; they don't independently sample.

4. **Comm-volume coherence.** Daily mail count + chat count on a tie respect the same `comm_intensity(tie, ...)` source; mail and chat split mode-shares without independently drifting.

5. **Validation test.** `internot/tests/social_cross_service.rs` will:
   - Pick a sample of married couples at various marriage_year vintages.
   - For each, walk t across marriage_date / first-child-birth / divorce_date.
   - Assert all 6 services reflect the cross-references identically.

## 10 — Future direction: v2+ hyperbolic latent space for weak ties

The v1 venue-cohort model (household, future workplace, future school) handles **strong communal ties** — people you share a stable shared context with. It does not produce **weak elective ties**: friend-of-friend, mutual-interest, "we met at a conference 5 years ago" connections. These are critical for realism beyond v1.

The 2020s state-of-the-art for procedural weak-tie generation is **Geometric Inhomogeneous Random Graphs (GIRG)** built on **hyperbolic latent space** (Krioukov et al. 2010; Bringmann, Keusch, Lengler 2019; Papadopoulos et al. 2012 PSO):

- Every person has a hyperbolic latent position `(θ, r) = f(person_id)` — 18 bits encode angle (12) and radial distance (6). Pure hash-derived; point-deterministic.
- Pair-edge probability `≈ 1 / (1 + (d_hyp(A, B) / R)^β)` where `d_hyp` is hyperbolic distance and `R, β` are global parameters.
- Yields scale-free degree (Barabási-style), high clustering (small-world), and short paths — simultaneously, the trifecta real social graphs show.
- Hash-threshold edge predicate: `edge(A, B) iff hash_float(min(A,B), "girg", max(A,B)) < p(A, B)`.
- Enumeration `weak_ties_of(X)` uses bit-pattern pushdown on a coarse cell-discretization of the latent space (O(|cells near X| × density), bounded).

**v1 does not use this.** Household ties are 100% within-venue, dense, fully connected. The framework primitives (`procedural_core::graph::*`) are designed so that adding hyperbolic-latent-space queries in v2 is a new function family alongside `VenueSpace`, not a refactor. Concretely, `procedural_core::graph::latent::hyperbolic::{position_of, pair_edge_prob, neighbors_in_radius}` would land in v2.

This is documented here so v1 implementation choices preserve compatibility (no bit-layout decisions in v1 prevent v2 latent positions from going somewhere).

## 11 — Error handling

The procedural graph is total: every person has a household, every tie kind is well-defined. There are no "missing" graph queries to fail on. Edge cases handled in code:

- `now < birth_date(person_id)` → `ties_of` returns empty; debug-only assertion.
- Marriage cohort odd-sized → one person unmatched at that tick → stays in birth household indefinitely (single-adult outcome). Not an error.
- Same-sex pairings → falls out of gender-agnostic stable-roommates preference function. No special case.
- Mail/chat/calendar attempts to write to a non-tied correspondent → `ViewError::NoSuchCorrespondent` (new variant). Sessions can still record the *attempt* in `MutationTrace.invalid_correspondent_attempts` for scenario verdicts that test refusal behavior.

## 12 — Testing strategy

### Unit/property tests at `procedural_core::graph`

- `VenueSpace::members_of` × `venues_of` round-trip (X is in venue V ↔ V is in venues_of(X)).
- Stable-roommates output symmetry: `(a, b) ∈ out ↔ (b, a) ∈ out`; preference monotonicity sanity-check.
- Event-enumeration determinism: same `(A, B, T₁, T₂)` → bitwise-identical event list across processes.
- Slice-recombinability: `events([T₁, T₂]) ∪ events([T₂, T₃]) == events([T₁, T₃])`.

### Property tests at `internot/src/social/`

- Reciprocity: random sample of (A, B, t) — `B ∈ peers(A, t) ↔ A ∈ peers(B, t)`.
- Household closure: for all members M of household H at t, `current_household_id_of(M, t) == H`.
- Monotonic partner identity: between `marriage_date(A)` and `divorce_date(A)`, partner doesn't change.
- Tie-strength bounds: `tie_strength` ∈ `[0, 1]` always; monotone-non-increasing post-cohabit-end for kin ties.

### Empirical validation suite — `internot/tests/social_validation.rs`

Sample 10 k random persons; for each, compute statistics; assert within tolerance bands derived from §2:

- Mean strong-tie count (kin only in v1): 3–5.
- Reciprocity ratio: 1.0 ± floating-point noise.
- Tie strength at partner-cohabiting peak: 0.9 ± 0.05.
- Burstiness B in synthesized mail events: 0.4 ± 0.15.

### Integration tests at `internot/tests/`

- `<service>_round_trip.rs` per service: writes survive reads, mutations restricted to tied correspondents.
- `social_cross_service.rs`: walks `t` across marriage / first-child / divorce for a sample of couples; asserts all 6 services reflect cross-references at the same `t`.

### Adversarial-scenario rebuild

After service rebuilds land, replay `mcp_harness/scenarios/`. Scenarios that depended on pair-existence-everywhere semantics get rebuilt around household-scoped failure modes (e.g., "agent attempts to email a non-tied person" should be a `NoSuchCorrespondent` refusal in v1).

## 13 — Phasing

One spec; one implementation plan per phase. Phases 0–2 are sequential; 3–8 are parallelizable.

1. **Phase 0** — `procedural_core::graph` primitives: `VenueSpace`, `Tie`, tie-strength trajectories, deterministic event enumeration, `stable_roommates_match`. Unit tests at the framework level.
2. **Phase 1** — `internot/src/social/` v1: household derivation (parent-cohort matching, fertility flow), member enumeration via `VenueSpace`, `ties_of`, role derivation, minors stubs, view registrations, property tests.
3. **Phase 2** — `people` cleanup: delete `family.rs`, `cohort.rs`, Tier 3 family-seed bits; rewire consumers; update `people_u512_round_trip` tests.
4. **Phase 3** — `mail` rebuild on graph.
5. **Phase 4** — `chat` rebuild on graph.
6. **Phase 5** — `calendar` rebuild on graph (with household events).
7. **Phase 6** — `files` rebuild on graph (with household shares).
8. **Phase 7** — `money` rebuild on graph (with joint accounts + intra-household transfers).
9. **Phase 8** — `tasks` rebuild on graph (with shared household tasks).
10. **Phase 9** — Adversarial-scenario review + empirical validation suite.

Phases 3–8 touch disjoint files; they can be parallelized.

## 14 — Open implementation details

These are *sizing* decisions, not architectural. Resolved in the writing-plans phase:

- Exact bit allocation in the `households` Space layout (`window_start_day` / `window_end_day` widths, role + intra_order encoding, headroom for v2 venues).
- Region granularity for the parent-cohort matching pass — `city_idx` (6 bits) vs a coarser country/region bucket. Affects cohort size and matching cost.
- Fertility distribution per region (uniform Poisson λ ≈ 1.78, lognormal, or region-tabled from real data).
- Sibling birth-spacing distribution (lognormal mean 2.5 years per Mathews & Hamilton 2016 — or simpler uniform).
- Whether minor-children stubs carry a personality projection or only name/age/gender (probably v2).
- Whether `last_proc_contact` lives per-tie or per-household.
- Cumulative-intensity grid granularity for `Λ⁻¹` interpolation (probably 1 day; refinement at life-event knots).
- Partner-preference function specific weights (age weight, geography weight, education weight, personality weight).

## 15 — Out of scope (explicit)

- Workplace / school / neighborhood / online-community / friend / vendor venues — v2+.
- Hyperbolic latent space for weak ties — v2+ (§10).
- Group threads / DMs / shared rooms — v2.
- Extended family (grandparents, in-laws, cousins, aunts/uncles) — v2.
- Minors as full population members — v2.
- LLM-as-judge, agent convenience tools — never (per `depth-over-breadth`).
- Backwards compatibility with current mail/chat/calendar/files/money/tasks views — none.

## 16 — References

Primary sources cited above; full bibliography per research file:

- Barabási, A.-L. (2005). "The origin of bursts and heavy tails in human dynamics." *Nature*, 435, 207–211.
- Bidart, C., Degenne, A., Grossetti, M. (2011, 2018). *La vie en réseau* — French longitudinal panel data on tie composition.
- Brown, E. N., Barbieri, R., Ventura, V., Kass, R. E., Frank, L. M. (2002). "The time-rescaling theorem and its application to neural spike train data analysis." *Neural Computation* 14, 325–346.
- Bringmann, K., Keusch, R., Lengler, J. (2019). "Geometric inhomogeneous random graphs." *Theor. Comput. Sci.* 760, 35–54.
- Burt, R. S. (2000). "Decay functions." *Social Networks* 22(1), 1–28.
- Dunbar, R. I. M. (1992). "Neocortex size as a constraint on group size in primates." *J. Human Evolution* 22, 469–493.
- Granovetter, M. S. (1973). "The strength of weak ties." *Am. J. Sociology* 78, 1360–1380.
- Hill, R. A., Dunbar, R. I. M. (2003). "Social network size in humans." *Human Nature* 14, 53–72.
- Irving, R. W. (1985). "An efficient algorithm for the 'stable roommates' problem." *J. Algorithms* 6(4), 577–595.
- Karsai, M., Kaski, K., Barabási, A.-L., Kertész, J. (2011). "Universal features of correlated bursty behaviour." *Sci. Rep.* 2, 397.
- Kivelä, M. et al. (2014). "Multilayer networks." *J. Complex Networks* 2(3), 203–271.
- Krioukov, D., Papadopoulos, F., Kitsak, M., Vahdat, A., Boguñá, M. (2010). "Hyperbolic geometry of complex networks." *Phys. Rev. E* 82(3), 036106.
- Lewis, P. A. W., Shedler, G. S. (1979). "Simulation of nonhomogeneous Poisson processes by thinning." *Naval Res. Logist. Quart.* 26, 403–413.
- Liben-Nowell, D., Kleinberg, J. (2007). "The link-prediction problem for social networks." *J. Am. Soc. Inf. Sci. Technol.* 58, 1019–1031.
- Mac Carron, P., Kaski, K., Dunbar, R. (2016). "Calling Dunbar's numbers." *Social Networks* 47, 151–155.
- McPherson, M., Smith-Lovin, L., Cook, J. M. (2001). "Birds of a feather: Homophily in social networks." *Annu. Rev. Sociol.* 27, 415–444.
- Onnela, J.-P., Saramäki, J., Hyvönen, J., Szabó, G., Lazer, D., Kaski, K., Kertész, J., Barabási, A.-L. (2007). "Structure and tie strengths in mobile communication networks." *PNAS* 104, 7332–7336.
- Perra, N., Gonçalves, B., Pastor-Satorras, R., Vespignani, A. (2012). "Activity driven modeling of time varying networks." *Sci. Rep.* 2, 469.
- Roberts, S. G. B., Dunbar, R. I. M. (2011). "The costs of family and friends: An 18-month longitudinal study of relationship maintenance and decay." *Evol. Hum. Behav.* 32, 186–197.
- Saramäki, J., Leicht, E. A., López, E., Roberts, S. G. B., Reed-Tsochas, F., Dunbar, R. I. M. (2014). "Persistence of social signatures in human communication." *PNAS* 111, 942–947.
- Ugander, J., Karrer, B., Backstrom, L., Marlow, C. (2011). "The anatomy of the Facebook social graph." *arXiv:1111.4503*.
