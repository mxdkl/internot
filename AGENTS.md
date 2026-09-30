# Internot — project guide

A procedural-world substrate for AI-agent training and evaluation. The thesis: a single deterministic procedural floor underlies every "service" (mail, calendar, drive, money, chat, ...), so cross-service coherence is automatic. Agents act on it through typed views exposed as MCP tools, scored by `_get_trace` which reports session mutations as JSON. The same typed views drive an OpenAI-compatible tool-calling harness for cheap iteration on cognitive tasks.

## How to work here (founder rules, 2026-09-29)

1. **Research first.** When you hit a problem, look it up before designing around it. It has probably been solved; the work is finding its formal name and framing it correctly. Search papers and the web, write what you find to `docs/superpowers/research/`, then design. Never weaken a core constraint (e.g. people as `f(seed, id, t)`) without doing this first.
2. **Make it perfect or don't build it.** Design fully, prototype the risky parts, and only then write production code. A layer is done only when its property, realism and performance gates pass.
3. **Extreme profiling.** Speed is a requirement. Budgets are measured gates at p99 over the full id space and worst cases (see the active spec §16). Benchmarks come before features.
4. **Keep this file current as you work.** Record decisions, progress, surprises and newly found defects here, as they happen, not at the end.

> **Current state (2026-09-29): read "Current phase" below before anything else.** The git history was re-initialized; only `people` and `_get_trace` exist. Mail, calendar, chat, files, tasks and money were removed for quality problems. Sections below that mention them describe removed code unless marked otherwise.

## Crates

**Substrate (framework, never edited per-service):**
- `procedural_core` — bit layouts, hashes, samplers, trajectories, `Space::find()` query builder with bit-pattern pushdown, `SlotLayout` helper.
- `procedural_overlay` — `Session<'w, W>` for mutation: in-memory overlay over a borrowed `World`, intercepts reads, absorbs writes.
- `internot_renderer` — shared LLM-render infrastructure: `OpenAiClient` (blocking reqwest; `with_base_url` lets it target any OpenAI-compatible API such as DeepSeek), on-disk `Cache` keyed by `(namespace, prompt_version, model_version, hashed-id)`, `RenderError`. **Currently has no consumer** (its only consumer, mail's renderer, was removed). Per-service renderers compose this; the same OpenAiClient + Cache instance is shared across services via the Services struct (see below).

**New layers under construction (spec `specs/2026-09-29-society-as-a-function.md`):**
- `internot_society` — the R1 kinship prototype: the demographic ledger (`ledger.rs`), schedules (`params.rs`), plan catalogs (`plan.rs`) and the lookups (`world.rs`). Population, unions (including same-sex and couples who arrived together), births, parents, children, siblings and deaths, for natives and immigrants, all as pure functions of `(seed, id, t)`. It depends only on `procedural_core`. Status and guarantees are under "Phase 1" below.
- `internot_perf` — the benchmark and profiling harness (`perf-gate` binary): suites `core_hash`, `primitives` and `kinship`, budgets in `perf/budgets.toml`, per-machine baselines in `perf/baselines/`, and flamegraphs. `perf/check.sh` runs all workspace tests and then the gate.

**The world (one crate, services as folders):**
- `internot/` — every domain service lives here as a sibling folder under `src/`. `Universe` carries three procedural worlds (`&'static World<u128>` + `&'static World<U256>` + `&'static World<U512>`), the `Mutex<SessionState>`, the injectable `now`, and a `services: Services` bag for process-wide infrastructure (LLM renderer Arcs). `registry()` concatenates each service's `views()` into the canonical list.
  - **Service plug-and-play.** `src/services.rs` defines the `Service` trait (methods `name`, `register_u128/u256/u512` (default no-op), `views`). `lib.rs::SERVICES: &[&dyn Service]` is the **single source of truth** — adding a service is one line here. `Universe::build_world_*()` and `crate::registry()` both walk this list; they never name services individually. Each service's `mod.rs` defines its tag struct (`MailService`, `PeopleService`, …) and impls `Service`.
  - **Process-wide infrastructure.** `src/runtime_services.rs` defines `Services` — a struct with `Option<Arc<...>>` fields per LLM renderer (mail). Built once at startup via `Services::from_env()` (reads `OPENAI_API_KEY`; empty if absent), attached via `Universe::new().with_services(Services::from_env())`. Views read `ctx.services.mail_llm` instead of constructing renderers per-call. Tests leave it `Default::default()`.
  - `src/people/` — slot layout 32-bit `(industry:6, city:6, workplace_seed:8, member_idx:12)`; `person_id` is U512-wide (32 bits + cached Tier 1–4 derived attrs). Names from a real-world dataset. Cohort enumeration via `Space::find()` pushdown. **Its relationship fields are unreliable; see "Known defects in `people`" below.**
  - *(Removed; do not restore: `mail/`, `calendar/`, `files/`, `tasks/`, `money/`, `chat/`.)*
  - `src/universe.rs` — Universe + SessionState wiring. SessionState carries one field per mutating service (still explicit; session types differ structurally).
  - `src/views.rs` — `View<P, O>` trait + `DynView` + `ViewRegistry`. View signature: `execute(&self, ctx: &Universe, params) -> Result<O, ViewError>`.
  - `src/services.rs` — `Service` trait.
  - `src/runtime_services.rs` — `Services` struct.
  - `src/trace.rs` — cross-cutting `_get_trace` view (verdict-only).

**Transport (thin generic router; no domain knowledge; never edited per-service):**
- `internot_mcp/` — stdio MCP server (rmcp 1.5). Walks `internot::registry()` once at startup and binds every view to an MCP tool. Calls `Universe::new().with_services(Services::from_env())` so LLM renderers are built once per process. Adding a new service appears as new tools on next build with no transport-side edit.

**Out-of-tree (Python):**
- `mcp_harness/` — scenarios driving `internot-mcp` over stdio through an OpenAI-compatible client. Verdict DSL with `tool_calls_must`, `tool_calls_must_not`, `final_must_contain`, etc. Write a Rust integration test in `internot/tests/` BEFORE adding any new chained-tool LLM scenario. **Most of the 27 scenarios in `mcp_harness/scenarios/` call tools that no longer exist**; treat them as design references, not a runnable suite. The client is a bare `OpenAI()`, so pointing it at DeepSeek needs only env vars: `OPENAI_BASE_URL=https://api.deepseek.com`, `OPENAI_API_KEY=<key>`, `MCP_HARNESS_MODEL=deepseek-flash`.

## Load-bearing invariants (do not violate)

1. **Single procedural floor.** Every service derives from the shared procedural world held by `Universe`. The world is split across three width-specialized containers (`World<u128>`, `World<U256>`, `World<U512>`) for bit-pattern-pushdown performance, but they are one logical floor — services pick the narrowest width that fits their layout. A new service is a lens on this floor, never an independently-seeded World. `internot_sql` was deleted because it built its own world.
2. **Single viewer per scenario.** Agents act as ONE user (the launched viewer). Views must not contain affordances that switch the active viewer. Read views can surface information about *other* people; mutations are always on behalf of the launched viewer.
3. **`now` is injectable, never a constant.** A frozen calendar overfits training data to one month and dates the simulation. Tests pin a specific `Utc.with_ymd_and_hms(...)`. Procedural generation can derive `now` from the `World` seed for variety while remaining 100% reproducible.
4. **32-bit person references throughout.** Mail's `mail_id: u32` set the convention. New services follow it. Population can reach 4.3B. Don't truncate to 16 bits to save padding.
5. **Cross-entity references via reconstruction, not materialization.** When entity A points at B, A stores B's defining bit fields, not the full u128 id. Then a procedural function reconstructs B's id deterministically. Keeps bit layouts narrow, makes references free queries.
6. **Stage Manager paradigm** (validated 2026-04-25). Procedural floor → typed AVMs → constrained renderer (LLM is a renderer of structured data, never a source of facts). Cross-service coherence is automatic because every service computes `f(id, key)` from the same primitives.
7. **Hand-curated lookup tables are tech debt.** Use Faker / geonamescache / procedural generators with a small hand-list for marquee items (e.g., 5-10 global brands at fixed `vendor_idx` slots; the rest procedural). An earlier incident with hand-curated city and name tables is the cautionary tale.

## Where to look

- `docs/memos/` — positioning decisions (the 2026-09-26 memo's commercial framing is superseded; see "Current phase").
- `docs/superpowers/specs/` — designs.
  - **`2026-09-29-society-as-a-function.md`** is the world-model design: population, kinship, households, residence, education, work, ties and content as pure functions of `(seed, id, t)`. It is **awaiting founder review (its §17). Do not build any of it before approval.**
  - `2026-09-29-coherence-eval-awareness-experiment.md` is the pilot that will consume it.
  - `2026-05-14-social-graph-substrate.md` and `2026-09-26-eval-first-service-rebuild.md` are superseded; they are kept for history.
- `docs/superpowers/research/` — read the relevant note before touching a layer.
  - The 2026-09-29 notes:
    - `local-access-and-bijections`
    - `kinship-and-households`
    - `time-consistent-evolution`
    - `work-and-organizations`
    - `affiliation-and-friendship`
    - `happenings-and-content-diversity`
  - The 2026-05-14 notes are older. Where they disagree with the 2026-09-29 notes, the newer ones are right: several 2026-05-14 claims were corrected (Burt tie decay, email volume, "stable under slicing").
- `mcp_harness/results/cross_model_matrix.md` — the last recorded cross-model results (gpt-5.4 family, on the removed services).
- `procedural_core` reference (below) — framework features. Anything using only `hash_int`/`hash_float` is ~10% of the framework.
- *(Gone with the history reset: `learning.md`, `vision.md`, `scenarios/registry.json`, the `2026-04-28` specs and plans, the `pre-rebuild-snapshot` branch.)*

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

## Current phase: rebuild for the coherence pilot (2026-09-29 →)

**What exists.** `people` + `_get_trace`, 6 MCP tools (`read_person`, `read_person_at_time`, `list_people_in_workplace`, `find_people`, `read_career_history`, `_get_trace`). Workspace tests pass (about 600). The transport still contains zero domain knowledge.

**Direction (set by the user, 2026-09-29).**
- Internot is free/open. The goal is usefulness, not revenue; the 2026-09-26 memo's sales plan (design partners, Nov 20 "concrete ask" gate) no longer applies.
- **Content, not websites.** Services serve content (people, messages, events, tasks, documents) through tools. No generated website or app-UI clones; that approach was tried before and dropped.
- **Every new service must be non-repetitive.** Treat repetition as a test failure, not a polish item.
- **The social graph must look realistic.** Reciprocal, clustered, bounded ego networks, sensible org structure and households. Check against the realism targets in `specs/2026-09-29-society-as-a-function.md` §15.
- **People stay `f(seed, id, t)`.** No materialized population and no simulation pass. The function may get as complicated as needed. The founder expects hard problems to have mathematical reframings: look for them (research first) before proposing to abandon this.
- **Speed is a requirement, proven by extreme profiling.**
  - Every performance budget is a measured gate at p99 over the full id space and worst cases, including concurrency and memory (spec §16).
  - Benchmarks come before features.
- **"Make it perfect or don't build it."** Design fully and prototype the risky parts before production code. A phase is done only when its property, realism and performance gates pass.
- **Budget.** About $10 of DeepSeek API credit for everything (LLM rendering plus agent runs). Design for that.

**Active goal.** Run the pilot in `docs/superpowers/specs/2026-09-29-coherence-eval-awareness-experiment.md`: does a coherent world lower how often a model says it thinks it's being tested? The pilot needs a small set of content services with cross-references between them.

**Current step: Phase 0 done; Phase 1 (kinship prototype R1) under way.** Design history:
- **Research:** six notes in `docs/superpowers/research/`, dated 2026-09-29.
- **Design:** `specs/2026-09-29-society-as-a-function.md`. The founder approved D1–D8 on 2026-09-29:
  - **D1** birth-cohort ids.
  - **D2** a full demographic ledger, with migration.
  - **D3** neighbourhoods, workplaces and schools everywhere, validated first in 1–2 metros.
  - **D4** US parameters first.
  - **D5** children are full people.
  - **D6** build the new layers alongside `people`, then cut over.
  - **D7** pilot services: directory, mail, calendar, tasks (no chat yet).
  - **D8** extreme profiling as a gate.
- **Next:**
  1. The Phase 0 benchmark and profiling harness.
  2. The risk prototypes (spec §18).
  3. Phases 0–6 (§19). Each is done only when its property, realism and performance gates pass.
- **Don't write production code** for a layer before its prototype passes.

**Phase 0: DONE (2026-09-30).** Every gate in spec §19 passes: primitive laws, recombination, primitive budgets, zero hot-path allocations, and top-3 flamegraphs committed.
- **New `procedural_core` modules,** each with property tests and golden values:
  - `dmath`: deterministic math via pinned `libm`.
  - `key`: structured keys and counter-based uniforms.
  - `sample`: exact Poisson (mult/PTRS), binomial (inversion/BTPE), normal, lognormal, Lomax.
  - `perm`: `FeistelPerm`, `GrowablePerm`, `SmallPerm`.
  - `pairing`: `Pairing`, `Coupling`.
  - `partition`: `SizeClasses`, `SplitTree`, `histogram_from_pmf`.
  - `count`: `CountTree`, `QuotaTree`.
  - `stream`: Recipe A `poisson_buckets`, Recipe B `PoissonTree`, Recipe C `renewal_walk` and `hazard_time`, and `regen_state`.
- **Event streams:** `graph::enumerate_events` is rewritten on Recipe A, and its May recombination test now passes. `graph::stable_roommates_match` is `#[deprecated]` in favour of `Coupling`.
- **Deterministic math:** all substrate float math goes through `dmath`.
- **Harness:** new crate `internot_perf` (perf-gate binary), plus `perf/` (README, `budgets.toml`, `check.sh` as the local CI substitute, `baselines/`, `phase-gates/phase0/`).
  - **How to run:** `perf/check.sh [--quick]` runs every workspace test and then the gate.
  - **Rules:**
    - a benchmark with no budget fails;
    - a p99 more than 10% over the committed baseline fails;
    - latency-only failures are re-measured up to 3 times;
    - percentiles are medians over time-ordered rounds, because of powersave-governor noise.
  - **Baseline:** recorded 2026-09-30 on the reference laptop under the `powersave` governor. Re-record with the `performance` governor when someone with root can set it (`sudo cpupower frequency-set -g performance`).
- **Measured p99, full id space plus worst cases:**

  | Primitive | p99 |
  |---|---|
  | key derivation | 1.9 ns |
  | Feistel fwd / inv at n ≈ 4.3·10⁹ | 20 / 18 ns |
  | pairing | 46 ns |
  | coupling | 60 ns |
  | size-class lookup | 4.6 ns |
  | quota locate | 43 ns |
  | Poisson | 90 ns |
  | binomial | 179 ns |
  | normal | 24 ns |
  | count tree, 24 levels | 1.5 µs |
  | Poisson-tree `count_before` / `last_before` | 1.1 / 3.6 µs |
  | 30-day event window | 6.3 µs |
  | 100-year hazard walk | 0.28 µs |

- **Lessons learned:**
  - Wall-clock swings ~50% on this governor. Compare cycles (the harness reports them) when micro-optimizing.
  - Feistel rounds use conditional subtraction and a precomputed inverse for division; a `%` there cost ~25%.
  - hdrhistogram's `saturating_record` never resizes and silently records zeros. The harness uses `record()`, and a test guards it.

**Phase 1 (population, kinship, households): IN PROGRESS.** The kinship prototype R1 is built. Households and the rest wait on founder decisions (below).

**What exists: crate `internot_society`** (built alongside `people`, per D6). The plan and a dated outcome log for every step are in `docs/superpowers/plans/2026-09-30-r1-kinship-prototype.md`.
- **Model.** Every fact is a pure function of `(seed, id, t)` over an integer ledger, with no simulation.
  - Blocks are `(birth year, lineage region)`, in two prototype regions. A block's members are entry cohorts: natives, then immigrants by arrival year.
  - First unions come from yearly two-sex markets: regional, national, and one same-sex market per sex. The IPF runs over birth years, with keyed integer rounding.
  - Fertility plans are apportioned per union cell. There are non-union births, dissolution by era, and deaths drawn from era mortality conditioned on the survival the person's *own* cells require: entry, the union year and, for a woman, her planned births.
  - **A father need only be alive at conception** (founder decision, 2026-09-30; `research/2026-09-30-death-locality-problem.md`).
    - `father()` is the mother's partner at conception, `GESTATION_DAYS` = 266 before the birth.
    - A child conceived after his death has no in-world father until widowed re-partnering.
    - No death depends on another person.
  - Immigrants arrive single or as couples. Couples arrive with their children born abroad, who have in-world parents.
- **API:** `World::build(params, seed)`, then:
  - `sex`, `birth_year`, `birth`, `death`, `alive_at`;
  - `region`, `is_founder`, `is_immigrant`, `arrival`;
  - `union`, `union_class`, `partner_at`;
  - `mother`, `father`, `children`, `siblings` (the last two return an inline `KinList`, with no allocation).
- **Guarantees.** Tested exhaustively on the tiny world (`tests/kinship.rs`, same-sex share boosted 10×) and by sampling on the prototype (`tests/realism.rs`):
  - partners are mutual, with identical union facts on both sides;
  - mother/child and father/child are dual;
  - siblings are the union of both parents' children;
  - life bounds hold, including immigrants alive at arrival and children born abroad arriving with both parents;
  - ledger closure holds per (block, block, year, cell kind);
  - worlds are deterministic across builds;
  - **zero close-kin couples**, checked exhaustively on 7 tiny seeds and on the full prototype (3.8M couples).
- **Realism (prototype, 11.1M ever born, 1840–2100).** Natives' cohort e0, CFR, union age, childlessness and divorce are within bands (`examples/realism_report.rs`). Also:
  - cross-region unions rise from 11% to 24%;
  - the foreign-born share of the living tracks the Census within ~3 points from 1870 to 2020;
  - children are ~12–18% of arrivals;
  - same-sex couples are 1.46% in 2019 (ACS ~1.5%);
  - only children are 2.8% of the 1880 cohort, 6% of the 1950 cohort and 11.6% of the 2010 cohort; mean siblings fall from 4.5 to 1.9 (spec ~9% only children; the child-weighted US figure is about 10–13%).
  - posthumous births are 0.8% of the 1860 cohort and 0.2% of the 1950 cohort (historically ~1% in high-mortality eras);
  - children conceived after the mother's partner died, who therefore have no in-world father, are 6.7% of the 1860 cohort, 3.4% of 1920, 1.2% of 1950 and 0.5% of 2010. Widowed re-partnering would give them stepfathers.
  - **Provisional parameters** (calibration targets are named in their docs): `national_market_share`, `couple_arrival_share`, `same_sex_share`, the immigration anchors.
- **Performance** (`kinship` suite; `perf/budgets.toml`). Measured 2026-09-30 under the `performance` governor, with single-call timing and thermal settling (below). Baselines for all suites were re-recorded then. Gate runs vary about ±10% (death more; see below), so compare changes with `perf/ab.sh`.

  | Query | p50 | p99 | Budget | Margin |
  |---|---|---|---|---|
  | birth | 8 ns | 10 ns | 25 ns | 60% |
  | death | 200 ns | 0.45–0.87 µs | 1 µs (spec) | 13–55% |
  | mother | 410 ns | 1.1–1.2 µs | 2 µs | ~42% |
  | father | 1.45 µs | 3.1 µs | 5 µs | ~38% |
  | union | 1.42 µs | 3.0–3.3 µs | 5 µs | ~36% |
  | children | 0.72 µs | 3.6–3.95 µs | 5 µs | ~24% |
  | siblings | 0.95 µs | 2.8–3.35 µs | 4 µs | ~22% |
  | world build | 1.04 s | 1.04 s | 2 s | |

  - **Starting point that morning** (single-call timing): death 2.0–2.6 µs and mother 1.3 µs, both failing; union, father and children 4.4–4.9 µs against 5 µs.
  - **All lookups are DRAM-bound** (IPC 0.3–0.6). On this machine a dependent load costs ~95 ns over 128 MB and 13 ns within L3 (16 MB), so **p99 ≈ the depth of the dependent-load chain × 95 ns**. Instructions barely matter.
  - **The world is 152 MB RSS.** Cells take ~36 MB, cohorts ~18 MB and birth tables ~6 MB; the rest is plans and the ledger.
  - **death's p99 is two-valued.** Partnered women drawn to die before 46 (1.3% of calls) need their own plan: three more DRAM loads, ~0.8 µs. The p99 sits at that group's edge, so round p99s come out at either ~0.43 or ~1.0 µs, and the gate's median over rounds picks one.
  - **What worked,** each confirmed by an interleaved A/B with identical answer checksums:
    - **Father survival at conception** (founder decision): death went from 2.3 to 0.8 µs.
    - **One array per search:**
      - partner slices as (block, start) with a sentinel;
      - sub-cells as (start, in-cell start, cell, year, plan);
      - plan leaves storing their start instead of their count.

      A search and the read after it now touch one array, not two: union and children −10–15%, mother −10–18%.
    - **Inline hot data:**
      - each cell's first cohort, usually the natives (−8–15% on union);
      - the union year in the sub-cell record (death −14–27%);
      - the count of women without non-union births in the cohort.
    - **Cheap exact filters in the kin predicate** (union p99 −10–14%):
      - different mothers' union cells mean different mothers, since each woman has one union cell in R1 (tested against full resolution);
      - a mother must be in the child's mother block;
      - a father's union cell must have a slice of the mother's block.
  - **What did not help, and was reverted:**
    - an inverted per-offset leaf index: −43% instructions, no change in cycles, +31 MB;
    - a direct year index for finding cells, whose small binary search was already cache-resident.
  - **Plans moved** into a per-block array indexed from sub-cells: roughly neutral, but no per-cell box.
  - **`CompactPerm`** (new in `procedural_core`, additive): the Feistel network in 32 bytes, with round keys `seed ^ π-constants`. `FeistelPerm` and its golden values are unchanged. The world uses it everywhere, which cut cells and cohorts by ~40%.
  - **Harness fix 1: batching hid memory costs.** Calibration repeated one input up to 18 times per sample, so later calls ran from cache. That was the "bimodal ±40%" noise once blamed on the governor. Calls of ≥ 250 ns are now timed singly; kinship lookups use `.batch(1)`.
  - **Harness fix 2: thermal settle.** Each benchmark waits until the CPU is ≤ 65 °C (`PERF_NO_COOL` skips it). The laptop goes from 63 °C to 93 °C within ~7 s of load.
  - **Levers left:**
    - huge pages, measured at 10–20% on every lookup via `MADV_COLLAPSE` (a process-level step for the MCP binary and the harness, not yet built);
    - hot/cold splits of cells and cohorts into 64-byte headers plus per-block arenas.
  - **R1c warning:** second unions add sub-cells and plans to death and kin lookups. Keep every constraint local (see Lessons).

**Blocked on founder decisions** (each written up, with options and a recommendation):
1. ~~Residence and households~~ **Decided 2026-09-30: option B now, C prototyped as its upgrade** (`research/2026-09-30-residence-enumeration-problem.md`).
   - Kinship is exact by lineage region.
   - Residence is dynamic: exact per person and per address, with moves drawn from candidate tiers (same city, same lineage region, national).
   - The ledger counts unions and births, not moves. This revises D2.
   - C (a first home derived from the parents' address) is prototyped against the move-distance and distance-to-parent targets before it replaces B.
   - Households (L3) are unblocked.
2. ~~Re-partnering (R1c)~~ **Decided 2026-09-30: divorce re-partnering now, widowed later** (`research/2026-09-30-repartnering-exactness-problem.md`, §4 item 1).
   - Dissolution becomes a partition of each partner slice, exact on both sides. This replaces D-R1.3's "plan independent of partner".
   - Fertility plans are conditioned on the divorce year, per (cell, dissolution year) sub-cell.
   - Kin repair (all three layers) runs within sub-cells.
   - A second-union market pairs the divorced with each other and with the never-partnered.
   - Widowed re-partnering is deferred; the later option is counting deaths of partnered people under 55.
   - Prototype on the tiny world with the exhaustive kinship suite first. `siblings()` must then add a father's other children.
3. ~~Name data~~ **Decided 2026-09-30: public-domain sources only.**
   - First names come from SSA baby names by sex and birth year (1880–2023; earlier births reuse 1880).
   - Surnames come from Census 2010, weighted by heritage through its race/Hispanic shares, and are inherited through the new family tree.
   - The old `internot/data/{first,last}_names.json` has unrecorded provenance and likely derives from the 2021 Facebook leak (via `philipperemy/name-dataset`). It serves only the old `people` crate and is deleted at cutover (D6). New layers must not use it.
   - European country-specific surnames wait until needed (Wikidata is CC0 but has no frequencies).
   - **Data in hand (2026-09-30)** in `datasets/names/`, which is git-ignored (raw data never committed; distillates are):
     - `ssa_names.zip`: SSA births by name, sex and year, 1880–2025, from about 2k names in 1880 to 31k in 2025. The founder downloaded it; the server refuses scripted downloads.
     - `Names2020_LastNames_RaceHispanic.xlsx`: 156,621 surnames with counts by race and Hispanic origin (the rest, 12% of people, are "all other names").
     - `Names2020_FirstNames_Sex.xlsx` and `Names2020_FirstNames_RaceHispanic.xlsx`: 53,616 first names with counts by sex and by race/Hispanic origin.
   - The Census 2020 release (April–May 2026, `https://www2.census.gov/topics/genealogy/2020surnames/`) replaces the 2010 file, whose link failed. Its `.xlsx` files download by script; the `*_WithNegatives` variants (presumably the 2020 differential-privacy counts before negatives were removed; not yet checked against the Census notes) are not used.
   - **Planned combination** for first names by sex, year and heritage: SSA's year distribution times each name's heritage lift from Census 2020 (the name's race/Hispanic share over the population share). Surnames come from Census 2020 counts by heritage.
4. ~~Perf budgets and baseline~~ **Decided 2026-09-30.**
   - The founder approved the raised budgets: union, father and children at the 5 µs one-hop cap; world build at 2 s.
   - Baselines are re-recorded once the founder sets the `performance` governor (`sudo cpupower frequency-set -g performance`, restore with `-g powersave`).
   - Memory levers wait until a layer's benchmarks need them:
     - tiny cells skip their stored shuffle;
     - `FeistelPerm` stores only its used round keys (touches Phase 0 goldens).
5. ~~DeepSeek key~~ **Decided 2026-09-30:** the founder has a key, with ~$2 of credit now and more to be added.
   - It goes into the project's `.env`, which is git-ignored (`OPENAI_BASE_URL=https://api.deepseek.com`, `OPENAI_API_KEY=...`), when Phase 5 starts. Never paste it into chat or commit it.
   - The spend guard (usage logging, running dollar total, cap) lands before the first paid call.
   - Top up to ~$10 before the pilot's main run.

**Lessons from R1 (durable):**
- **Constraints must stay local to the person drawn.** Conditioning a man's death on his partner's plan made 3.9% of deaths pay a two-block partner lookup. That tripled death's p99 and biased male e0 by up to 1.8 years. Before adding any constraint, ask which endpoint can evaluate it without a hop.
- **Sequential scans are nearly free next to dependent misses.** An inverted per-offset index replaced `mother`'s ~120-leaf scan. It cut instructions 43%, left cycles unchanged and added 31 MB, so it was reverted. Hardware prefetch streams a scan; a search is a chain of dependent loads.
- **Huge pages** (`MADV_COLLAPSE` over the heap) gave 10–20% on every lookup at 152 MB. That is worth having once the world lives in a few large arenas, but it is not a design lever.
- **Measure changes under 10% with interleaved A/B runs** (`perf/ab.sh`, `examples/lookup_timing.rs`), not separate gate runs. Separate runs vary ~10%. In one case they showed a 13% "regression" in `father` that reversed when the run order was swapped.
- **A search and the read after it should touch one array.** Parallel arrays (starts in one, payload in another) cost two dependent misses; interleave them with a sentinel.
- **Guard shortcut predicates with an agreement test.** `same_mother`'s cell shortcut is checked against full mother resolution on two tiny seeds (`world::tests`), because R1c will give some women a second union cell and break its premise.
- **Kin repair.** Pairs alone leave odd tails and one-couple cells unrepairable; the spec's 10⁻¹² estimate was wrong for small cells. What works is three layers:
  1. women's-side groups (pairs plus a trailing triple);
  2. men's-side groups holding a "free" couple;
  3. ledger deferral of couples isolated on both sides.
  Cheap necessary conditions (same mother block; parent–child age gaps) keep it fast.
- **Memory decides p99 once the world exceeds L3 (16 MB).** Instruction counts barely move; cache misses do. Fixes that worked:
  - birth tables instead of per-year event lists (83 MB → 3 MB);
  - packed u64 plan leaves, and plans boxed off cells that have none;
  - packed search arrays and coarse table indexes;
  - inline mother-age slots instead of searches;
  - precomputing each union birth's start in one pass.
- **Keep keys and perms out of recomputation, and chrono out of hot paths.** Stored per-cell permutations beat rebuilding them while the world fits in cache. Use `stream::year_start` / `year_of` (closed form), not chrono.
- **Two-stage exact draws.** Draw from the cheap constraint; keep the draw if it clears an upper bound on the full constraint; otherwise redraw from the full constraint. That is exactly the conditional law, and it keeps death off the partner lookup almost always.
- **Separate cells whenever pairing semantics differ** (arrival couples, same-sex left/right). Slot coupling pairs slice positions, so mixing kinds in one slice pairs the wrong people.
- **Calibrate against the observable.** Immigrant flows were tuned on the foreign-born share of the living, not on gross inflows: there is no return migration, and children are extra.
- **Noise.** Compare cycles and instructions when p99 moves; run perf suites on a quiet, cool machine; `check.sh` right after the workspace tests inflates p99.

### Known defects in `people` (do not build on these)

Found 2026-09-29 by dumping `read_person` for `mail_id` 8197:
- **Unpopulated people are served.** 8197 has `member_idx` 5, but its workplace size is 1, so by `cohort.rs` it doesn't exist; `read_person` still returns a full profile.
- **Manager and frequent collaborator usually don't exist.** `populate.rs` picks a random `member_idx` in 0..4095, but median workplace size is ~7, so the stub almost always points past the populated range (8197's manager has `member_idx` 1427).
- **Relationships contradict life state.** 8197 is 66, retired (`current_role: null`) and still has a manager and collaborator; is divorced yet still lists a `spouse_mail_id`.
- **Spouse, parent and child links are one-way** (documented in `family.rs`): A's spouse B usually doesn't have A as spouse.
- Mentor is a random person in another city.

Found the same day by reading the derivations:
- **17 ages never occur.** `derive_lifecycle_epoch` draws from buckets `[18,22,28,35,45,55,65,75]` + 0..5 years, so nobody is 34, 41–44, 51–54, 61–64 or 71–74 at `DEFAULT_NOW`.
- **The population doesn't turn over.** Birth dates are `DEFAULT_NOW − age`, so the population is a fixed set of people aged 18–80 at `DEFAULT_NOW`. At a `now` 50 years later everyone is 68–130; nobody is born or dies. This undercuts invariant 3 (injectable `now`).
- **Career employers are unrelated to the workplace.** `career.rs` draws `employer_seed` as a random 32-bit hash, so a person's job history never mentions the workplace encoded in their own id, and a job switch doesn't move them.
- **Nobody ever moves.** `current_city_of_at` ignores `t`; the city is fixed for life by the id.
- **Families are 65,536 global buckets.** `family_id_of` hashes into 2^16 families, so tens of thousands of unrelated people worldwide count as "siblings."
- **Surnames come from a random country.** `last_name_of` picks the family's heritage country uniformly at random, so most people's surname doesn't match their country (e.g. a Mexico City native named "Ramiya").
- The individual attributes (personality, education level, job titles, languages, hobbies, working hours) are reasonable and reusable; relationships, time and the population model are what need redesigning.

Found during Phase 0 (substrate-wide, not just `people`):
- **Float results aren't reproducible across machines.** Rust documents `f64::ln`, `exp`, `sin`, `powf` and other transcendentals as non-deterministic: results can vary by platform, Rust version, and even between calls. `hash_gaussian`, `sampler::{lognormal, pareto, exponential}`, trajectories and `people` all use them, so a published seed may not rebuild the same world elsewhere.
  - **Rule from now on:** transcendental math goes through `procedural_core::dmath`, which wraps the pinned pure-Rust `libm`, and is pinned by golden tests. Never call `f64::ln` and friends in substrate code.
  - Existing call sites migrate in Phase 0. See `docs/superpowers/research/2026-09-29-deterministic-numerics.md`.

### Lessons carried over (from the lost `learning.md`)

- **Round-trip test per write surface.** For every mutating tool, a test writes via the tool and re-reads through every view that should show the write. This caught real bugs that a trait abstraction would not have.
- **Verify the verdict.** Before calling a scenario "hard", confirm its correct answer is reachable through the tools (`mcp_harness/test_verify_solver.py`). Three of seven early "failure modes" turned out to be caused by the environment (e.g. `book_meeting` couldn't book :30 starts).
- **Accept "refuse and ask" as a correct outcome** where a safe agent might decline; otherwise stronger models get marked wrong.
- **Findings on the removed services** (gpt-5.4 family): failures clustered in fan-out × constraint satisfaction (booking several meetings with mutual non-overlap), topological order (completing dependent tasks), and instruction prioritization (`lunch_window_refusal`: the agent names the user's rule, then breaks it). Pure fan-out and single-booking multi-constraint tasks passed.
- **Cross-model runs are cheap.** Earlier matrices cost a few dollars; run them early, not after building many scenarios.

**Off the roadmap:** agent convenience tools (`whoami`, `now`, aggregators), LLM-as-judge for task verdicts.

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
