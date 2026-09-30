# Internot — project guide

A procedural-world substrate for AI-agent training and evaluation. The thesis: a single deterministic procedural floor underlies every "service" (mail, calendar, drive, money, chat, ...), so cross-service coherence is automatic. Agents act on it through typed views exposed as MCP tools, scored by `_get_trace` which reports session mutations as JSON. The same typed views drive an OpenAI tool-calling harness for cheap iteration on cognitive tasks.

## Crates

**Substrate (framework, never edited per-service):**
- `procedural_core` — bit layouts, hashes, samplers, trajectories, `Space::find()` query builder with bit-pattern pushdown, `SlotLayout` helper.
- `procedural_overlay` — `Session<'w, W>` for mutation: in-memory overlay over a borrowed `World`, intercepts reads, absorbs writes.
- `internot_renderer` — shared LLM-render infrastructure: `OpenAiClient` (blocking reqwest), on-disk `Cache` keyed by `(namespace, prompt_version, model_version, hashed-id)`, `RenderError`. Per-service renderers (e.g. `mail::llm_render::LlmRenderer`) compose this; the same OpenAiClient + Cache instance is shared across services via the Services struct (see below).

**The world (one crate, services as folders):**
- `internot/` — every domain service lives here as a sibling folder under `src/`. `Universe` carries three procedural worlds (`&'static World<u128>` + `&'static World<U256>` + `&'static World<U512>`), the `Mutex<SessionState>`, the injectable `now`, and a `services: Services` bag for process-wide infrastructure (LLM renderer Arcs). `registry()` concatenates each service's `views()` into the canonical list.
  - **Service plug-and-play.** `src/services.rs` defines the `Service` trait (methods `name`, `register_u128/u256/u512` (default no-op), `views`). `lib.rs::SERVICES: &[&dyn Service]` is the **single source of truth** — adding a service is one line here. `Universe::build_world_*()` and `crate::registry()` both walk this list; they never name services individually. Each service's `mod.rs` defines its tag struct (`MailService`, `PeopleService`, …) and impls `Service`.
  - **Process-wide infrastructure.** `src/runtime_services.rs` defines `Services` — a struct with `Option<Arc<...>>` fields per LLM renderer (mail). Built once at startup via `Services::from_env()` (reads `OPENAI_API_KEY`; empty if absent), attached via `Universe::new().with_services(Services::from_env())`. Views read `ctx.services.mail_llm` instead of constructing renderers per-call. Tests leave it `Default::default()`.
  - `src/people/` — slot layout 32-bit `(industry:6, city:6, workplace_seed:8, member_idx:12)`; `person_id` is U512-wide (32 bits + cached Tier 1–4 derived attrs). Names from a real-world dataset. Cohort enumeration via `Space::find()` pushdown.
  - `src/mail/` — pair-thread 96-bit + messages 127-bit. `MailSession` overlay for compose/reply/mark_read/archive. `mail/llm_render.rs` is `cfg(feature = "llm")`.
  - `src/calendar/` — thread / personal / venue anchored events. `CalendarSession` overlay for RSVP + book.
  - `src/files/` — slot 48-bit; pareto count; hash-derived kind/collection/name. `FileSession` for upload.
  - `src/tasks/` — slot 44-bit; lognormal count; time-varying status via `status_of(task_id, now)`. `TaskSession` for create/done.
  - `src/money/` — accounts + transactions + subscriptions (slot-based, read-only v1).
  - `src/chat/` — DM rooms (procedural). `ChatSession` for sent DMs.
  - `src/universe.rs` — Universe + SessionState wiring. SessionState carries one field per mutating service (still explicit; session types differ structurally).
  - `src/views.rs` — `View<P, O>` trait + `DynView` + `ViewRegistry`. View signature: `execute(&self, ctx: &Universe, params) -> Result<O, ViewError>`.
  - `src/services.rs` — `Service` trait.
  - `src/runtime_services.rs` — `Services` struct.
  - `src/trace.rs` — cross-cutting `_get_trace` view (verdict-only).

**Transport (thin generic router; no domain knowledge; never edited per-service):**
- `internot_mcp/` — stdio MCP server (rmcp 1.5). Walks `internot::registry()` once at startup and binds every view to an MCP tool. Calls `Universe::new().with_services(Services::from_env())` so LLM renderers are built once per process. Adding a new service appears as new tools on next build with no transport-side edit.

**Out-of-tree (Python):**
- `mcp_harness/` — OpenAI scenarios driving `internot-mcp` over stdio. Verdict DSL with `tool_calls_must`, `tool_calls_must_not`, `final_must_contain`, etc. Per `feedback_scenario_dry_run.md`: write a Rust integration test in `internot/tests/` BEFORE adding any new chained-tool LLM scenario.

## Load-bearing invariants (do not violate)

1. **Single procedural floor.** Every service derives from the shared procedural world held by `Universe`. The world is split across three width-specialized containers (`World<u128>`, `World<U256>`, `World<U512>`) for bit-pattern-pushdown performance, but they are one logical floor — services pick the narrowest width that fits their layout. A new service is a lens on this floor, never an independently-seeded World. `internot_sql` was deleted because it built its own world.
2. **Single viewer per scenario.** Agents act as ONE user (the launched viewer). Views must not contain affordances that switch the active viewer. Read views can surface information about *other* people; mutations are always on behalf of the launched viewer.
3. **`now` is injectable, never a constant.** A frozen calendar overfits training data to one month and dates the simulation. Tests pin a specific `Utc.with_ymd_and_hms(...)`. Procedural generation can derive `now` from the `World` seed for variety while remaining 100% reproducible.
4. **32-bit person references throughout.** Mail's `mail_id: u32` set the convention. New services follow it. Population can reach 4.3B. Don't truncate to 16 bits to save padding.
5. **Cross-entity references via reconstruction, not materialization.** When entity A points at B, A stores B's defining bit fields, not the full u128 id. Then a procedural function reconstructs B's id deterministically. Keeps bit layouts narrow, makes references free queries.
6. **Stage Manager paradigm** (validated 2026-04-25, see learning.md). Procedural floor → typed AVMs → constrained renderer (LLM is a renderer of structured data, never a source of facts). Cross-service coherence is automatic because every service computes `f(id, key)` from the same primitives.
7. **Hand-curated lookup tables are tech debt.** Use Faker / geonamescache / procedural generators with a small hand-list for marquee items (e.g., 5-10 global brands at fixed `vendor_idx` slots; the rest procedural). The hand-curated cities + name tables incident in learning.md is the cautionary tale.

## Where to look

- `learning.md` — every non-obvious lesson, in chronological order. Skim recent entries before starting any new design conversation.
- `docs/superpowers/specs/` and `docs/superpowers/plans/` — past designs and implementation plans.
- `scenarios/registry.json` — the empirical test surface.
- `procedural_core` reference (below) — framework features. Anything using only `hash_int`/`hash_float` is ~10% of the framework.

## Using internot core (the consumer recipe)

Symmetric to the authoring recipe below. Whether you're driving the world from a Rust integration test, a binary, or another transport, the shape is always the same.

**1. Build the Universe once.**
```rust
use internot::{Universe, Services};
let u = Universe::new();                                  // tests
let u = Universe::new().with_services(Services::from_env()); // production
```
`Universe::new()` builds the three leaked `&'static World<W>` references via `OnceLock`; subsequent calls return the same worlds. `with_services(Services::from_env())` reads `OPENAI_API_KEY` and constructs the shared mail LLM Arc once. `Universe::with_now(t)` pins the simulated clock; `set_now(t)` adjusts it later.

**2. Call views, two ways.**

*Typed (in-process).* Best for tests and Rust consumers — no JSON round-trip, full type checking:
```rust
use internot::View;
use internot::people::views::{ReadPerson, ReadPersonParams};
let avm = View::execute(&ReadPerson, &u, ReadPersonParams { mail_id: 8197 })?;
```

*Untyped (transport-style).* Best for any code that walks the registry generically:
```rust
let reg = internot::registry();
let view = reg.get("read_person").expect("registered");
let out: serde_json::Value = view.execute(&u, serde_json::json!({"mail_id": 8197}))?;
```
`view.input_schema()` returns the JSON Schema for the params; `view.read_only()` flags mutating views. This is exactly what `internot_mcp` does.

**3. Mutate through the session mutex.**
Mutating views lock `ctx.sessions`, dispatch into the per-service overlay, and append to `MutationTrace`. From outside a view (e.g. test setup), do the same:
```rust
let mut s = u.sessions.lock();
let msg_id = s.mail.compose(viewer_id, &[recipient], "hi".into(), u.now)?;
s.trace.composed_messages.push(format!("{msg_id:#x}"));
```
Session-allocated ids carry the top sentinel bit (the `procedural_overlay::Session` invariant); read views surface the overlay-stored sender/recipient/body via the per-service side maps (`MailSession::composed_thread`, etc.).

**4. Read the verdict.**
`_get_trace` is the canonical scoring surface — every mutating session method appends to one of `MutationTrace`'s typed lists. Scenarios call `view.execute(&u, json!({}))` on the trace view and pattern-match the JSON. For in-process tests just inspect `u.sessions.lock().trace` directly.

**5. Drive the LLM renderers.**
`read_thread` / `get_inbox` accept `llm_render: true`. The view checks `ctx.services.mail_llm` and dispatches to the shared Arc; a None value falls back to the deterministic template renderer. The on-disk cache at `$XDG_CACHE_HOME/internot-renderer` is keyed by `(namespace, prompt_version, model_version, hashed_id)`, so re-runs of the same scenario are free.

**6. Test the chain in Rust before paying the API.**
For any new chained-tool flow, write a Rust integration test in `internot/tests/` first. The `mcp_harness/` Python scenarios are an external smoke test, not the first line of defense.

## Adding a new service (the canonical recipe)

The substrate is plug-and-play as of 2026-05-06. To add a new service `foo`:

1. **Create `internot/src/foo/`** with the standard module layout: `mod.rs`, `slot.rs` (or `thread.rs` etc. — whatever your bit-layout file is called), `derive.rs` (hash-derived attrs), `avm.rs` (AVM struct), `views.rs` (read views), `session.rs` (mutating overlay, if any), and a `pub fn views() -> Vec<Arc<dyn DynView>>` exporter.
2. **In `foo/mod.rs`**, define a tag struct + impl `Service`:
   ```rust
   pub struct FooService;
   impl Service for FooService {
       fn name(&self) -> &'static str { "foo" }
       fn register_u128(&self, w: &mut World<u128>) -> Result<(), WorldError> { slot::register(w) }
       fn views(&self) -> Vec<Arc<dyn DynView>> { views::views() }
   }
   ```
   (Use `register_u256` / `register_u512` instead if your layout exceeds 128 bits. Default impls are no-ops, so only override what you use.)
3. **Add `&FooService` to `internot::SERVICES`** in `lib.rs`. That's the only edit to crate-level files.
4. **If `foo` mutates state**, add a field to `SessionState` in `universe.rs` (still explicit because session types differ structurally per service) and initialize in `SessionState::new`.
5. **If `foo` has an LLM renderer**, gate the module on `cfg(feature = "llm")`, add `Option<Arc<FooRenderer>>` to `Services`, build it in `Services::from_env`, and have your views read `ctx.services.foo_llm` instead of constructing per-call.

Adding the service automatically: registers spaces on the right world, exposes views as MCP tools (`internot_mcp` rebuilds and they appear).

## Current phase: depth over breadth (2026-04-28 →)

The codebase was wiped on 2026-04-28 and rebuilt from scratch on the design in `docs/superpowers/specs/2026-04-28-service-system.md`. The previous codebase lives in branch `pre-rebuild-snapshot` for reference. Following the rebuild, the project pivoted from breadth (more services) to **depth** (more adversarial scenarios on existing services). See `docs/superpowers/plans/2026-04-28-eval-gym-phase-2.md` for the operating principle.

**Status:** 7 services + `_get_trace` (people, mail, calendar, files, tasks, money, chat). MCP tools auto-bound from the registry. Workspace tests pass (each mutating service has a `tests/<svc>_round_trip.rs` integration file enforcing "if the agent writes X via session, every relevant view surfaces X correctly on re-read"). The internot_mcp transport contains zero domain knowledge — adding a service appears as new tools on next build with no transport-side edit.

- **people:** slot 32-bit `(industry:6, city:6, workplace_seed:8, member_idx:12)`; `person_id` is U512 (32 bits + cached Tier 1–4 derived attrs: archetype, Big Five + facets, Schwartz, behavioral, attachment, dark triad, CSE, HEXACO H, self-monitor, graph stubs, lifecycle/career seeds). Real names + industries (NAICS) + cities + occupations (BLS SOC 848 detailed) + fields of study (NCES CIP 42 families). Each person has an **education profile** (degree level, field of study, graduation year), a **career arc** (~10 events from age 18 to retirement: JobSwitch / Promotion / Demotion / LateralMove / Sabbatical / FoundCompany / Retirement, each carrying role level, employer_seed, SOC title), and a **life events timeline** (HS grad → college grad → graduate degree → marriage → children → divorce, anchored on US 2020 demographic stats: ~80% marry, median age 28, ~40% divorce). Views: `read_person`, `read_person_at_time`, `list_people_in_workplace`, `find_people`, `read_career_history`. See `internot/examples/biography_demo.rs` for what a person looks like.
- **mail:** pair-thread layout 96-bit; messages 127-bit packing thread_id + day + seq. `MailSession` overlay for compose/reply/mark_read/archive. Views: `get_inbox`, `read_thread`, `compose_email`, `reply`, `mark_read`, `archive_thread`. `read_thread`/`get_inbox` accept `llm_render: true` to dispatch to the process-wide `LlmRenderer` Arc.
- **calendar:** Thread / Personal / Venue anchored events (~10/day across types). `CalendarSession` overlay for RSVP + book. Views: `get_schedule`, `rsvp`, `book_meeting`, `get_busy`, `find_meeting_slot`.
- **files:** slot-based 48-bit; pareto file count per owner. Hash-derived kind/collection/name/size. `FileSession` for upload. Views: `list_drive`, `read_file`, `upload_file`.
- **tasks:** slot-based 44-bit; lognormal count. Status is *time-varying* via `status_of(task_id, now)`. `TaskSession` for mark_done + create_task. Views: `list_tasks`, `read_task`, `mark_done`, `create_task`.
- **money (partial):** subscriptions only (slot-based 40-bit). Views: `list_accounts`, `read_account`, `list_transactions`, `list_subscriptions`. Accounts/transactions/invoices substrate not built — gated on a scenario that requires them.
- **chat (partial):** DM rooms only (procedural over correspondent pairs). `ChatSession` for sent DMs. Views: `list_dms`, `read_dm`, `send_dm`. Group rooms / presence not built — same gate.

Plus `_get_trace` (meta, verdict-only).

**What's next (depth roadmap, not new services):**
- 20-scenario target HIT (was the D1 milestone). **7 adversarial-hard** for gpt-5.4-mini across 6 distinct failure modes (see README for full breakdown + smoking-gun evidence). Key refinement: pure FAN-OUT alone PASSES (proven on tasks + files); the calendar failures are FAN-OUT × CONSTRAINT-SATISFACTION specifically. INSTRUCTION-PRIORITIZATION (`lunch_window_refusal`) is the cleanest safety-relevant signal — agent self-diagnoses the rule violation in its own final message and commits the violation anyway, every trial. Future scenario design should target conjunctions of two-or-more failure modes (already attempted in `decline_then_book` which surfaced state-divergence + multi-mutation slip simultaneously).
- Per-service edge cases (tentative RSVP, recurring events, blocking task deps, etc.) — each gated on the adversarial scenario it unlocks.
- Verdict DSL extensions (`overlay_must_satisfy`, `tool_call_order`) — added when a scenario requires them.

**Off the roadmap:** new top-level services, agent convenience tools (`whoami`, `now`, aggregators), LLM-as-judge. See plan doc D4 section.

---

# procedural_core — Full Feature Reference

**Keep this file in context whenever building on `procedural_core`.** The crate offers far more than hash-derived attributes; anything using only `hash_int`/`hash_float` is using ~10% of the framework.

## Layer 0 — Primitives (`word`, `bits`, `hash`, `sampler`, `trajectory`, `edge`)

### `word::BitWord` (trait)
Implemented by `u64` and `u128`. Bounds include `Hash` (so word values can key `HashMap`s — `procedural_overlay` relies on this). Methods: `BITS`, `extract_bits(offset, width) -> u64`, `insert_bits(offset, width, value) -> Self`, `from_hash_u64(h)`. **`extract_bits` panics if `width > 64`** — the framework returns u64 from extraction, so any single field wider than 64 bits is unreachable. Split such fields into `_lo: 64` + `_hi: ≤63` and reconstruct with raw shifts (see `internot_mail::messages::compose_message_id` for an example).

### `bits::BitLayout<W>` — the structural id
Carves an id into named bit fields (LSB-first, declaration order). **Fields you constrain narrowly via `where_eq` should be declared LAST** — last-declared varies fastest in find() enumeration.

```rust
let layout = BitLayout::new(vec![
    ("entropy", 16), ("age_idx", 6), ("country_idx", 4), ("signup_offset", 5),
])?;
let id: u64 = layout.compose(&[("age_idx", 24), ("country_idx", 1), ...]);
let age_idx = layout.extract(id, "age_idx");
let extractor = layout.extractor("age_idx"); // Fn(W) -> u64, cheap clone
```

Introspection: `total_width()`, `has_field(name)`, `field_offset_width(name) -> Option<(u32, u32)>`, `field_names()`. `compose()` **silently masks** out-of-range values to a field's declared width — range-check before calling if oversize values would indicate a bug.

### `hash::*` — deterministic value derivation
- `hash_int(id, key, n) -> u64` in `[0, n)` (Lemire bounded, no modulo bias)
- `hash_float(id, key) -> f64` in `[0.0, 1.0)`
- `hash_vec(id, key, dims) -> Vec<f64>` — independent per-dim sub-keys
- `hash_gaussian(id, key) -> f64` — N(0, 1) via Box-Muller (zero-alloc; uses two streamed sub-hashes with a fixed `__gauss` separator)

### `sampler::*` — realistic distributions
- `pareto(id, key, alpha, scale)` — power law (follower counts, org sizes)
- `lognormal(id, key, mu, sigma)` — right-skewed (income, response times)
- `exponential(id, key, rate)` — waiting times between events
- `categorical(id, key, &[weights]) -> usize` — weighted choice, auto-normalizes

### `trajectory::*` — time-varying values
- `smooth(id, t, amplitude, timescale)` — value-noise in `[-amp, amp]`, smooth within timescale (mood, focus)
- `oscillate(id, t, period, amplitude, phase)` — sine wave (activity cycles)
- `step(id, t, &events, &magnitudes)` — piecewise-constant (discrete life events). `id` is unused — events fully determine the output. Events MUST be ascending; debug-asserts otherwise.

`oscillate` also ignores `id`. To make oscillations vary per-entity, derive `phase` from `hash_float(id, key) * 2π` yourself.

**Stability radii** (for temporal range queries — tells you how far you can step before value changes by ε):
- `smooth_stability_radius(id, t, amp, ts, eps) -> Duration`
- `oscillate_stability_radius(...)`, `step_stability_radius(...)`
- `stability_radius_quadratic(abs_df, abs_d2f, eps)` — generic Taylor bound
- `stability::min_of(&[&fn], id, t, eps)` — composition helper for `f = Σ f_i`. Splits ε evenly and returns the min δ across components (conservative; triangle-inequality safe).

### `edge::*` — pairwise similarity / connection probability
- `geometric(a, b, radius, soft)` — Euclidean; hard threshold or sigmoid
- `cosine(a, b, threshold)` — cosine similarity, clamped to `[0, 1]` (negative cosine maps to 0); values below `threshold` also clamp to 0
- `hyperbolic((r_a, θ_a), (r_b, θ_b), r_disk, temperature)` — Poincaré-disk model (realistic social graphs: scale-free, small-world, clustered)
- `block(group_a, group_b, p_in, p_out)` — stochastic block model

## Layer 0.5 — `graph` (NEW 2026-05-14)

Framework primitives for social graphs. Domain-agnostic; consumers
provide the ontology (what's a venue, what kinds of ties exist).

- `graph::Tie` — edge struct (peer, kind, strength, since, last_contact).
- `graph::canonical_pair(a, b)` — symmetric pair-keying for hash-based
  procedural derivation.
- `graph::pair_hash_float(a, b, key)` — symmetric pair-keyed hash.
- `graph::VenueSpace` — cohort enumeration over a registered Space
  (members_of / venues_of / role_of, with `Space::find()` pushdown).
- `graph::TieStrengthProfile` + `graph::tie_strength(profile, t_days)` —
  composable strength function (base floor + cohabit peak +
  post-cohabit exponential decay).
- `graph::PersonalityProjection` + `graph::CommIntensity` +
  `graph::comm_intensity(strength, self_p, peer_p, t)` — per-mode
  event rates (mail/chat/calendar) with diurnal + chronotype +
  weekly modulation.
- `graph::CommEvent` + `graph::enumerate_events(a, b, namespace, λ, t1, t2)` —
  deterministic inhomogeneous-Poisson event enumeration via the
  time-rescaling theorem + hash-derived exponential gaps. Symmetric
  in `(a, b)`. v1 limitation: per-call event index resets at
  `t_start`, so slicing the window does NOT recombine; documented
  in `procedural_core/tests/graph_integration.rs::event_enumeration_slice_recombinability` (`#[ignore]`) and tracked for v2.
- `graph::stable_roommates_match(cohort, pref)` — Irving's pairing,
  gender-agnostic, symmetric in argument order by construction. v1
  is a simple greedy proposing impl (not full rotation-elimination);
  adequate for cohort sizes ≤ ~250.

Spec: `docs/superpowers/specs/2026-05-14-social-graph-substrate.md` §5.
Implementation plan: `docs/superpowers/plans/2026-05-14-phase-0-graph-primitives.md`.

## Layer 1 — `Space<W>` (domain container)

Holds a `BitLayout<W>` plus registered attributes, relations, and similarity contexts.

```rust
let mut space = Space::new("people", layout);
space.attribute("personality_o", |id: u64| hash_float(id, "personality_o"))?;
space.temporal_attribute("mood", |id, t: DateTime<Utc>| smooth(id, t, 1.0, Duration::hours(6)))?;
space.indexable_attribute("age", "age_idx", |v: u64| 18 + v as u32)?;
// Composite indexable: decoder reads its own field's bits PLUS the bits of
// each shared dependency field (in declaration order). Lets one attribute's
// value depend on multiple bit fields while every involved field stays
// independently queryable.
space.indexable_composite_attribute(
    "first_name", "name_idx", &["country_idx"],
    |own_bits, shared: &[u64]| names_for_country(shared[0])[own_bits as usize].to_string(),
)?;
space.cross_space_attribute("employer", |id, world: &World<u64>| { ... })?;
space.cross_space_temporal_attribute("inbox_count", |id, t, world| { ... })?;
```

**Relations:**
```rust
space.relation("parents", Arity::Fixed(2), |id, _t, i| parent_id_for(id, i))?;
// Note the lowercase `dynamic` helper — `Arity::Dynamic` directly takes
// an `Arc<dyn Fn>` and won't accept a bare closure.
space.relation("children", Arity::dynamic(|id, _t| num_children(id)), |id, t, i| ...)?;
```

**Contexts** (for similarity search):
```rust
use procedural_core::space::context::{ContextDim, Metric, Normalization};
space.context("lifestyle", vec![
    ContextDim { attribute: "age".into(),           weight: 1.0, normalize: Normalization::None },
    ContextDim { attribute: "personality_o".into(), weight: 0.5, normalize: Normalization::None },
], Metric::Euclidean)?;
```

`Metric` variants: `Cosine`, `Euclidean`, `Weighted` (alias for `Euclidean` — per-dim weights are applied upstream during vector build), `Hyperbolic` (**reserved — panics at `take()` in v0.1**). `Normalization` variants: only `None` is implemented.

`Space::context()` validates at registration: dim attribute types must be numeric (`f64/f32/u8..u64/i8..i64/bool`) and `normalize` must be `Normalization::None`. Errors are typed (`SpaceError::UnsupportedDimType` / `UnsupportedNormalization`) — no `take()`-time panic for these cases. The `Hyperbolic` metric panic remains at `take()` time.

**Introspection helpers:** `attribute_names() -> Vec<String>`, `relation_names()`, `context_names()`, `is_temporal(name) -> Option<bool>`, `attribute_type_id(name) -> Option<TypeId>` and `attribute_type_name(name) -> Option<&'static str>` (added so external crates like `procedural_overlay` can validate writes against the registered attribute type), `attribute_indexable_fields(name) -> Option<Vec<String>>` (own + deps for composites; empty Vec for non-indexable; `None` for unknown), `relation_arity_kind(name) -> Option<&'static str>` (`"fixed"` | `"dynamic"`).

**Queries on a space:**
- `space.entity(id, Some(t))` → `EntitySnapshot` with all attributes evaluated
- `space.attribute_value<V>(id, name, Some(t))` → single attribute
- `space.related(id, rel_name, t)` → iterator over relation members
- `space.find()` → `Query<Unbounded>` (typestate builder)
- `space.candidates(id, context_name)` → `CandidateQuery` (similarity)

## Layer 2 — `World<W>` (cross-space container)

```rust
let mut world = World::new();
world.register(people_space)?;
world.register(companies_space)?;
world.register_global("trending_topic", |t: DateTime<Utc>| topic_at(t))?;

world.attribute_value::<Company>("people", id, "employer", Some(t))?;
world.global::<String>("trending_topic", t)?;
world.related("people", id, "coworkers", t)?;
```

## The Search Engine (`search::*`) — the real magic

### `BitPattern` — compact set of ids via pinned+free bits
```rust
let pat = BitPattern::exact(value=24, width=6);      // matches one value
let pat = BitPattern::any(width=6);                   // matches everything
pat.matches(candidate) -> bool
pat.cardinality(width) -> u64                         // 2^free_bits
pat.enumerate(width) -> PatternEnumerator             // Iterator<Item = u64>
```

### `range_to_prefixes(lo, hi, bits) -> Vec<BitPattern>`
Decomposes a closed numeric range into minimal bit-prefix patterns. Worst-case 2*bits patterns.

### `minimize_patterns(&[values], width) -> Vec<BitPattern>`
Quine-McCluskey minimization of a value set into patterns. `width ≤ 26`.

### `Query<'a, W, State>` — typestate find() builder
```rust
let results = space.find()
    .where_eq("age_idx", 24)                    // Unbounded → Bounded
    .where_range("signup_offset", 10, 20)
    .where_in("country_idx", &[1, 3, 5])
    .filter_static("is_verified", |id| ...)     // post-enum filter
    .filter_temporal("mood_happy", |id, t| ...) // needs .at(t)
    .filter_exists_in_range("was_happy_this_week",
        |id, t| smooth(id, t, 1.0, hours(6)),
        Op::Gt, threshold=0.5, StabilityMode::BinarySearch { probes: 8 })
    .at_any(start, end)                          // for *_in_range filters
    .scan_budget(10_000)                         // cap
    .execute();                                  // only on Bounded

for id in results {
    ...
}
results.termination()  // Pending | Exhaustive | Budgeted { evaluated }
results.evaluated()     // candidates drawn so far
```

**`where_in` does NOT minimize.** It emits one `BitPattern::exact` per distinct value (Quine-McCluskey can produce overlapping prime implicants → duplicate IDs). Use `minimize_patterns(...)` directly + `BitPattern`s only when you can guarantee no overlap.

**Panic conditions on `find()`:**
- Unknown field name in `where_*` → panic.
- Two predicates on the same field → panic ("v0.1 only supports one predicate per field").
- Total free bits across all cursors ≥ 64 → panic at `execute()` ("query yields ≥ 2^64 candidates").
- `filter_temporal` without `.at(t)`, or `filter_*_in_range` without `.at_any(start, end)`, or mixing `at` with `*_in_range` filters / `at_any` with `filter_temporal` → panic at `execute()`.
- `at_any(start, end)` with `start >= end` → panic at `execute()`.

### `Op` — threshold operator for temporal range filters
`Lt | Le | Gt | Ge | Eq | Ne`. `op.check(value, threshold) -> bool`.

### `StabilityMode<'a, W>` — temporal sweep strategy
- `Analytic(Box<Fn(id, t, eps) -> Duration>)` — user-supplied δ
- `BinarySearch { probes: usize }` — shrinking step (works on arbitrary functions, may miss oscillating satisfying instants)

### `CandidateQuery<'a, W>` — similarity search
```rust
let results = space.candidates(id, "lifestyle")
    .budget(1024)                  // candidates to draw (default 1024)
    .envelope("country_idx")        // restrict to same country
    .at(t)                          // time anchor
    .min_score(0.5)
    .take(10);                      // top-10 by score

for (other_id, score) in results.hits { ... }
results.evaluated, results.budget, results.total_considered
```

## Typical Patterns

**Declaring a Space from scratch:**
1. Design `BitLayout` — put narrowly-constrained attributes LAST.
2. Register attributes; prefer `indexable_attribute` over `attribute` when the value lives in a bit field (enables pattern pushdown).
3. Register relations for each kind of connection.
4. Register contexts for similarity dimensions.

**Efficient equality search:** `indexable_attribute` → `where_eq` → `BitPattern::exact` → `PatternEnumerator`. O(k) where k is results drawn, not O(|keyspace|).

**Efficient range search:** `where_range(lo, hi)` → `range_to_prefixes` internally → union of `BitPattern`s.

**Similarity search:** register a `context` with weighted dimensions → `CandidateQuery` draws candidates via pattern enumeration, scores via the metric, returns top-k.

**Temporal window query:** `filter_exists_in_range` / `filter_forall_in_range` + `at_any(start, end)` + `StabilityMode`. The stability radius drives the sweep step — analytic form is tight; binary search is general.

## Design principles (don't violate)

1. **Everything is a pure function of (id, key) or (id, key, t).** No state.
2. **Bit-backed fields are free to filter on.** Hash-derived attributes require scan.
3. **Put narrowly-constrained fields last in the layout.** Odometer iteration varies last-declared fastest.
4. **Stability radii are how temporal ranges are efficient.** Don't sweep naively; use the radius or BinarySearch.
5. **Cross-space attributes go through `World`**, never `Space` directly.
6. **For mutation, use `procedural_overlay::Session<'w, W>`** — the core stays read-only; agent actions live in a session-scoped overlay. If your application needs `Session`, the bit layout must reserve the top bit (`total_width() ≤ W::BITS - 1`) so session-allocated ids carry the sentinel.

## Stage Manager paradigm (for consumer crates)

Service crates built on `procedural_core` (e.g., `internot_mail`, `internot_calendar`) should adopt the **Stage Manager** split: every fact a renderer / UI / agent might consume must be derivable as a pure function of `(id, key)` from the procedural floor *before* prose generation. An LLM, when added, is a constrained renderer of structured AVMs (Attribute-Value Matrices), never the source of any fact. This is what makes cross-service coherence automatic: every service that asks `f(id, key)` gets the same answer without runtime coordination. Concrete shape: build a per-message/per-entity AVM struct populated entirely from procedural attributes + shared `internot_facts` functions; pass that AVM to the renderer; cache on `(entity_id, prompt_version, model_version)`.

**Empirically validated as of 2026-04-25.** The pattern that worked across mail and calendar:
1. The AVM carries a **typed specifier enum** (`TopicSpecifier { AboutMeeting | AboutFile | AboutPhrase }`, `EventSpecifier { Standard | MeetingFromThread | AdHocAbout }`). The LLM dispatches on the kind and uses the companion fields. No stringly-typed "subject" field — the model has nowhere to insert an invented one.
2. The AVM serializes via a **descriptive-key flat JSON wrapper** (`MessageAvmJson`, `EventAvmJson`) — long, semantic key names (`sender_voice_formality_baseline`, `event_specifier_kind`) so the model can disambiguate fields with no fine-tuning.
3. The OpenAI call uses **`response_format: json_schema, strict: true`** (rejection-sampled server-side); the system prompt instructs on per-kind dispatch and explicitly forbids invention.
4. **Cross-service knowledge gaps are exposed as structural truth, not hidden** — calendar's `MeetingFromThread { thread_id }` lets the renderer say "the calendar can't view the original thread" when appropriate. The agent navigates services because the AVMs *can't* answer cross-service questions on their own.
5. **Shared renderer infrastructure** lives in `internot_renderer` (RenderError, FormalityBand/TensionBand, IndustryTier, OpenAiClient request wrapper). Each service keeps its own typed renderer trait + AVM JSON wrapper + system prompt. No generic `Renderer<I, O>` trait — mail's `MessageAvm` and calendar's `EventAvm` have different structural relationships with time and context; forcing them through one trait pushes toward `Box<dyn Any>` context plumbing. See `internot_calendar/examples/cross_coherence.rs` for the working proof.

When designing a new service crate's AVM + renderer, copy mail's pattern: typed specifier enum, descriptive-key serde wrapper, structured-output OpenAI call, side-by-side template + LLM demo. The architecture has earned the prescription.

## Known sharp edges

- The search builder `panic`s on misuse (unknown field, duplicate-field predicate, anchor/filter mismatch); attribute / context / relation registration returns `Result`. Different error contracts in similar API positions — do not assume `find()` is `Result`-safe.
- **`BitLayout::extract` and `BitLayout::compose` go through `u64`.** Declaring a single field wider than 64 bits succeeds at construction time but panics at `extract`. If you need a wider logical field (e.g., embedding a 96-bit thread_id inside a 127-bit message_id), split it into `_lo` (≤64) and `_hi` (≤63) and pack/unpack via raw `u128` shifts. `BitLayout::compose` also accepts only `u64` per field for the same reason.
- `Metric::Hyperbolic` is registered cleanly but panics inside `candidates(...).take()` (v0.1). The other validation gaps (dim type, `Normalization`) are now caught at `Space::context()` registration.
- The `EvalFn` dispatch ladder is open-coded in 5 places (`Space::entity`, `Space::attribute_value`, `World::entity`, `World::attribute_value`, `candidates::eval_attribute`). Adding a new variant means editing all five — easy to miss one.
