# Internot — project guide

A procedural-world substrate for AI-agent training and evaluation. The thesis: a single deterministic procedural floor underlies every "service" (mail, calendar, drive, money, chat, ...), so cross-service coherence is automatic. Agents act on it through typed views exposed as MCP tools, scored by `_get_trace` which reports session mutations as JSON. The same typed views drive an OpenAI-compatible tool-calling harness for cheap iteration on cognitive tasks.

## How to work here (founder rules, 2026-09-29)

1. **Math first, then research.** When you hit a problem (founder, 2026-10-01):
   1. **Frame it as math.** State it formally and find its name. It has probably been solved.
   2. **Check `procedural_core`** for a primitive that already answers it.
   3. **Otherwise, research online** (papers and the web), and write what you find to `docs/superpowers/research/`.
   4. **Add the answer to `procedural_core`** as a generic primitive with property tests and golden values, so other consumers can reuse it.

   **All math lives in `procedural_core`; that is what it is for** (founder, 2026-10-01).
   - This includes samplers, fitting, rounding, interpolation, hazards and life tables, streams and bijections.
   - Domain crates hold only their ontology (what a union or a household is), the wiring from pack data to core primitives, and their data layouts.
   - Math already written in a consumer moves to `procedural_core`.

   Then design. Never weaken a core constraint (e.g. people as `f(seed, id, t)`) without doing this first.
2. **Make it perfect or don't build it.** Design fully, prototype the risky parts, and only then write production code. A layer is done only when its property, realism and performance gates pass.
3. **Extreme profiling.** Speed is a requirement. Budgets are measured gates at p99 over the full id space and worst cases (see the active spec §16). Benchmarks come before features.
4. **Keep this file current as you work.** Record decisions, progress, surprises and newly found defects here, as they happen, not at the end.

> **Current state: read "Current phase" below before anything else.** The git history was re-initialized on 2026-09-29. Mail, calendar, chat, files, tasks and money were removed for quality problems; sections below that mention them describe removed code unless marked otherwise. Since 2026-10-01 the MCP tools are the `directory` service on the society world (`internot_society`) plus `_get_trace`; the old `people` service was cut over and deleted.

## Crates

**Substrate (framework, never edited per-service):**
- `procedural_core` — bit layouts, hashes, samplers, trajectories, `Space::find()` query builder with bit-pattern pushdown, `SlotLayout` helper.
- *(Removed 2026-10-01, founder: "outright remove them and rebuild something exact that we need when we need": `procedural_overlay` (a session overlay for mutation), `internot_renderer` (an LLM render client and cache), the `mcp_harness/` Python scenario harness, and `internot`'s empty `Services` bag. All are in git history; rebuild what a service needs when it needs it.)*

**World packs (spec `specs/2026-10-01-world-packs.md`):**
- `internot_def`: world packs, i.e. RON files in `worlds/<pack>/`. It provides:
  - loading from directories or from packs embedded in the binary;
  - `extends`, merging field by field and lists by `id`;
  - errors that name the file and the line or field;
  - a fingerprint;
  - the value vocabulary (`Series`, `VecSeries`, `Steps`, `Bands`, `Ranges`, `BySex`), evaluated by `procedural_core::interp` (its one dependency inside the workspace).
- **Every statistic lives in a pack** (`worlds/us/`), never in code. `worlds/README.md` is the guide.

**New layers under construction (spec `specs/2026-09-29-society-as-a-function.md`):**
- `internot_society` — the R1 kinship prototype: the demographic ledger (`ledger.rs`), schedules (`params.rs`), plan catalogs (`plan.rs`) and the lookups (`world.rs`). Population, unions (including same-sex and couples who arrived together), births, parents, children, siblings and deaths, for natives and immigrants, all as pure functions of `(seed, id, t)`. It depends on `procedural_core`, plus `rayon` for a parallel `World::build`. Status and guarantees are under "Phase 1" below.
- `internot_perf` — the benchmark and profiling harness (`perf-gate` binary): suites `core_hash`, `primitives` and `kinship`, budgets in `perf/budgets.toml`, per-machine baselines in `perf/baselines/`, and flamegraphs. `perf/check.sh` runs all workspace tests and then the gate.

**The world (one crate, services as folders):**
- `internot/` — every domain service lives here as a sibling folder under `src/`. `Universe` carries the society world (`society`), three procedural worlds (`&'static World<u128>` + `&'static World<U256>` + `&'static World<U512>`, empty since the cutover), the `Mutex<SessionState>` and the injectable `now`. `registry()` concatenates each service's `views()` into the canonical list.
  - **Service plug-and-play.** `src/services.rs` defines the `Service` trait (methods `name`, `register_u128/u256/u512` (default no-op), `views`). `lib.rs::SERVICES: &[&dyn Service]` is the **single source of truth** — adding a service is one line here. `Universe::build_world_*()` and `crate::registry()` both walk this list; they never name services individually. Each service's `mod.rs` defines its tag struct (`DirectoryService`, …) and impls `Service`.
  - `src/directory/`: people of the society world through MCP (`read_person`, `read_household`): names, family, unions, households and addresses at any time. Spec `specs/2026-10-01-directory.md`.
  - `src/society.rs`: the society world (`internot_society::World` plus `Residence`), built once per process from the embedded pack `INTERNOT_PACK` (default `us`) and seed `INTERNOT_SEED` (default 42); `Universe::society` holds it, and tests use `Universe::for_pack("us-tiny", now)`.
  - *(Removed 2026-10-01: `src/people/`, the old slot-layout people, with its leaked name data. Its individual attributes (personality, education, languages, hobbies, working hours) are in git history, to re-key to the new ids when needed.)*
  - *(Removed; do not restore: `mail/`, `calendar/`, `files/`, `tasks/`, `money/`, `chat/`.)*
  - `src/universe.rs` — Universe + SessionState wiring. SessionState carries one field per mutating service (still explicit; session types differ structurally).
  - `src/views.rs` — `View<P, O>` trait + `DynView` + `ViewRegistry`. View signature: `execute(&self, ctx: &Universe, params) -> Result<O, ViewError>`.
  - `src/services.rs` — `Service` trait.
  - `src/trace.rs` — cross-cutting `_get_trace` view (verdict-only).

**Transport (thin generic router; no domain knowledge; never edited per-service):**
- `internot_mcp/` — stdio MCP server (rmcp 1.5). Builds `Universe::new()` (and so the society world) once at startup, walks `internot::registry()` and binds every view to an MCP tool. Adding a new service appears as new tools on next build with no transport-side edit.

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
- *(The cross-model results on the removed services, `mcp_harness/results/cross_model_matrix.md`, went with the harness on 2026-10-01; their findings are summarized under "Lessons carried over".)*
- `procedural_core` reference (below) — framework features. Anything using only `hash_int`/`hash_float` is ~10% of the framework.
- *(Gone with the history reset: `learning.md`, `vision.md`, `scenarios/registry.json`, the `2026-04-28` specs and plans, the `pre-rebuild-snapshot` branch.)*

## Using internot core (the consumer recipe)

Symmetric to the authoring recipe below. Whether you're driving the world from a Rust integration test, a binary, or another transport, the shape is always the same.

**1. Build the Universe once.**
```rust
use internot::Universe;
let u = Universe::new();                      // the INTERNOT_PACK world (default `us`) at DEFAULT_NOW
let u = Universe::for_pack("us-tiny", now);   // tests
```
`Universe::new()` builds the society world once per process (per pack and seed) and the leaked `&'static World<W>` containers via `OnceLock`; later calls share them. `Universe::with_now(t)` pins the simulated clock; `set_now(t)` adjusts it later.

**2. Call views, two ways.**

*Typed (in-process).* Best for tests and Rust consumers — no JSON round-trip, full type checking:
```rust
use internot::View;
use internot::directory::views::{ReadPerson, ReadPersonParams};
let rec = View::execute(&ReadPerson, &u, ReadPersonParams { person_id: 9_000_000, at: None })?;
```

*Untyped (transport-style).* Best for any code that walks the registry generically:
```rust
let reg = internot::registry();
let view = reg.get("read_person").expect("registered");
let out: serde_json::Value = view.execute(&u, serde_json::json!({"person_id": 9000000, "at": "1990-01-01"}))?;
```
`view.input_schema()` returns the JSON Schema for the params; `view.read_only()` flags mutating views. This is exactly what `internot_mcp` does.

**3. Mutate through the session mutex.** No service mutates yet. A mutating view locks `ctx.sessions`, changes its service's session state (a field of `SessionState`) and appends to `MutationTrace`; the mutation layer is rebuilt when the first mutating service needs it.

**4. Read the verdict.**
`_get_trace` is the canonical scoring surface — every mutating session method appends to one of `MutationTrace`'s typed lists. Scenarios call `view.execute(&u, json!({}))` on the trace view and pattern-match the JSON. For in-process tests just inspect `u.sessions.lock().trace` directly.

**5. Test the chain in Rust before paying an API.**
For any new chained-tool flow, write a Rust integration test in `internot/tests/` first (`internot/tests/directory.rs` is the model). Agent runs over MCP come after.

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
5. **If `foo` needs an LLM renderer or other process-wide infrastructure**, build exactly that when it's needed (the old renderer crate and `Services` bag were removed unused).

Adding the service automatically: registers spaces on the right world, exposes views as MCP tools (`internot_mcp` rebuilds and they appear).

## Current phase: rebuild for the coherence pilot (2026-09-29 →)

**What exists.** The `directory` service on the society world plus `_get_trace`: 3 MCP tools (`read_person`, `read_household`, `_get_trace`). The transport still contains zero domain knowledge.

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
- **Statistics, not rules (2026-09-30).** "Don't make global rules for people; theoretically anything can happen, it's just statistics." Also: "do what you think is realistic."
  - Model behaviour with rates by age, era and circumstance, never with caps, exclusions or labels forced on people. Examples:
    - immigrants arrive as they are, and re-partner if they want;
    - no "at most two unions";
    - no "never returns home".
  - A hard rule kept for technical reasons is debt: name it, measure its realism cost, and replace it when it matters.

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

**R1c (divorce re-partnering): IN PROGRESS, design written 2026-09-30** (`plans/2026-09-30-r1c-divorce-repartnering.md`; targets in `research/2026-09-30-remarriage-targets.md`).
- **Mechanism:**
  - dissolution years are exact classes on each partner slice, via keyed systematic apportionment;
  - sub-cells are (cell, class): kin repair, coupling and plans work within them;
  - isolated couples move to the nearest class, and one pass suffices;
  - markets have a status dimension (never partnered or divorced), giving four kinds of opposite-sex cell;
  - second-union membership comes from each first-union sub-cell's remarriage parts;
  - every life constraint stays local.
- **Scope:** at most two unions per person; no same-sex re-partnering. The founder is informed and it proceeds unless told otherwise.
- **Build order:** primitive → ledger → world → realism → performance, tiny world first.
- **Stage A DONE (2026-09-30): dissolution classes and class-cells, without remarriage.**
  - Every test passes: the exhaustive kinship suite, closure per class, and zero close kin.
  - Realism bands pass.
  - **The layout that works:** one cell per (year, kind, class), in one cache line (64-byte `CellLayout`, arrays in per-block arenas), with coarse-indexed keys, cohort lines and birth rows.
  - **Cost against pre-R1c:**
    - union p99 +41% (4.4 µs) and father +55% (4.8 µs) against 5 µs budgets;
    - children +20%;
    - world build 1.52 s.
  - Details and the three layouts tried are in the R1c plan, "Stage A outcome".
- **Stage B BUILT, gates FAILING (2026-09-30): divorced pools, status markets, second unions.**
  - Every test passes (845 in the workspace), including the exhaustive kinship suite with second unions: reciprocity on both unions, closure per (year, kind, class, block, block, sex), zero close kin, duality and life bounds.
  - **Realism, calibrated** (targets in `research/2026-09-30-remarriage-targets.md`):

    | Measure | Model | Target |
    |---|---|---|
    | remarried within 1 / 3 / 5 / 10 years of divorce | 17 / 39 / 52 / 69% | NSFG 15 / 39 / 54 / 75% |
    | median age at second union, women / men | 34 / 36 | 33 / 36 |
    | remarriages with the husband 10+ years older | 17.2% | 16% |
    | second unions broken within 10 years | 36% | 39% |
    | new unions with both / one partner previously partnered | 15 / 16% | Pew 20 / 20%; the gap is the widowed, deferred |

    - Couples moved by de-isolation: 2.4%.
    - CFR of the 1950 cohort: 2.31.
    - Ever born: 12.06M.
    - Provisional parameters: `SECOND_UNION_FERTILE` = 0.45, the remarriage hazard by duration, men ×1.2, status affinity 3.5 for two divorced partners, divorce ×1.35 in second unions, and remarriage age gaps ×0.6.
  - **World build: 4.28 s → 1.45 s** (min of 3; 3.44 s on one thread, 1.64 s on four). Every change was checked bit-identical, by a checksum of the whole ledger and of 600k lookup answers:
    - the IPF fuses its passes and sums eight rows side by side (a float sum is a serial add chain);
    - market rounding skips the draw on zero cells;
    - `procedural_core::partition::SystematicShares` (new, additive) computes shares once, with a sparse `for_each_part` for small splits. It is used by class splits, plan partitions (`plan::PlanShares`, `PlanTables`) and arrival classes;
    - births are a dense year × block table;
    - free couples come from one pass over single-member cells, replacing a search into another block's ledger cells for every one of 4M slices (20% of the build);
    - **parallel, with `rayon`** (new dependency of `internot_society`): per-block layouts and tables, each block's side of a market group, all of a year's market solves (caps then apply in order), class splits and the year-end sort. Each task touches only its own block, and births merge as sums, so results don't depend on scheduling.
  - **Gate, 2026-09-30** (performance governor). world_build 1.59 s, within the 2 s budget; its baseline (1.04 s) predates R1c. **Lookups fail:**

    | Query | p99 | Budget |
    |---|---|---|
    | death | 1.77 µs | 1 µs |
    | mother | 1.93 µs | 2 µs |
    | father | 12.3 µs | 5 µs |
    | union | 11.4 µs | 5 µs |
    | children | 15.2 µs | 5 µs |
    | siblings | 22.4 µs | 4 µs |

  - **Memory: 890 MB peak RSS** (R1: 152 MB).
    - Union cells: 1.30M (stage A: 592k).
    - Partner slices: 4.77M, 4.0M of them a single couple.
    - Cohort or source parts: 2.93M.
    - Divorced sources: 656k, with 1.64M remarriage parts.
    - The ledger's per-cell `Vec`s stay alive inside `World`.
  - **Founder decision (2026-09-30): performance is good enough for now; the work is deferred.**
    - The bar is "decent enough not to get in the way of high workloads", not extreme p99s.
    - In absolute terms, lookups are fine: a family-heavy view call is a few ms, and bulk kin generation is about 20 s per million people on one thread.
    - Deferred, in priority order:
      1. **Memory**: 890 MB per process, so ~14 GB for 16 parallel MCP servers.
      2. **Sharing one world across processes**: a memory-mapped build, or one server for many sessions.
      3. **Full-scale estimates**: memory and build time grow with population, and the national market's rounding with regions².
      4. **Lookup budgets sized to workloads**: about 25 µs for one-hop lookups and 2 µs for death, in place of the 4–5 µs caps.
    - The kinship gate's lookup failures are known and accepted until then. Do not re-record baselines; that would hide them.

**L3 households: BUILT, calibrated (2026-09-30)** (`plans/2026-09-30-l3-households.md`, including §13; targets in `research/2026-09-30-household-targets.md`).
- **Module:** `internot_society::household`: `World::household(x, t)`, `World::members(h, t)`, `World::kin_host`.
  - Household kinds: `Union`, `Solo` (with anyone who lives with them), `Roommates`.
  - Rules:
    1. dependents live with a parent or guardian (minors whose union ended go back);
    2. a unit (single or couple) seeks kin with a propensity by age and era: moving back with a parent, other relatives, an elder with a child, a young couple with a parent. It joins the first **anchor** among its kin (anchors never seek kin), so it's one step and never cycles;
    3. partners live together;
    4. single adults live alone or with roommates (ages 18 to 64, no child under 18; keyed frames of 12 over birth-year bands).
- **Tests** (`tests/households.rs`, exhaustive on the tiny world at 15 dates): exact reciprocity both ways; partners together; minors never alone except the measured residuals; no next-day reversion.
  - The residuals: founder minors until 18 (1840 to 1857); kinless orphans, 0.15% of minors in 1900 and none by 2025.
- **Calibrated:**
  - at home at 18 to 24;
  - adults with other relatives (12.0% against 12.3%);
  - household size and couple share for 1980 and 2000 (2.75 against 2.76; 53.2% against 52.8%);
  - 65+ with an adult child (18.5% in 2000).
  - **Report:** `examples/household_report.rs` (about 80 s), including partner status by age.
- **Still off (2025):**

  | Measure | Model | Census |
  |---|---|---|
  | adults living alone | 22.5% | 14.8% |
  | one-person households | 40.6% | 29.5% |
  | 65+ living alone | 39% | 28% |

  This is mostly the kinship partner deficit at older ages; see R1d debt.
- **Speed:** `household` 17 µs p50 / 176 µs p99; `members` 167 µs / 731 µs. No gate yet.
- **Out of scope:** dorms, boarders and servants.
- **Founder decision (2026-10-01): accepted as is.** "We can drift from the census a little." Phase 1 (population, kinship, households) is done.
  - Still open, as debt: widowed re-partnering, mortality by partnership, the young-union timing, and the deferred speed items (lookup budgets, memory, the 2.4 s world build).

**R1d (statistics, not rules): steps 1 to 4 DONE (2026-09-30)** (`plans/2026-09-30-r1d-statistics-not-rules.md`).
- Rules removed:
  - the age cutoffs on first unions and re-partnering;
  - "at most two unions" (now any number; `MAX_UNIONS` = 16 is a bound on lookups, and the most reached is 9);
  - single immigrants partnering on the never-partnered first-union schedule (now at their single native peers' rate).
- Recalibrated to living with a spouse or partner by age (Census A1 plus UC3):
  - re-partnering;
  - dissolution, with cohabitation included, so recent unions end sooner;
  - fertility compensation.
- The 1970 cohort has 2.02 children; 14.0M ever born.
- **Open debt:** no widowed re-partnering (widowed singles at 75+: 39% against 33%); mortality ignores partnership; no same-sex re-partnering. World build is 2.4 s.

**World packs: steps 1 to 4 DONE (2026-10-01)** (`specs/2026-10-01-world-packs.md`; research `research/2026-10-01-world-definitions.md`).
- **Founder decision (2026-10-01):** "everything configurable through those rust json-like files". That covers races, names, birth rates, countries, places and, later, services, "easily extendible by anyone"; the specifics are delegated.
- **The design:**
  - Mechanisms are code; every number, list and name is data.
  - Packs are RON files in `worlds/<pack>/`, and `us` and `us-tiny` are embedded in the binary.
  - `extends` merges a child pack into its parent field by field. Lists of records with an `id` merge by id.
  - Unknown fields are errors. Every number sits next to a comment naming its source.
  - The merged pack is compiled once at build. A fingerprint identifies the pack.
  - A new crate, `internot_def`, does the loading and merging and holds the value vocabulary (`Series`, `Steps`, `BySex`, ...).
- **Migration is bit-identical,** checked by `examples/world_fingerprint.rs`. Baseline: tiny `b19c209ec97e8c4c` / `f3bd18ec8c5afb2e`, prototype `e47e257ad076f57d` / `bc764f24987f394f`.
- **Done:**
  - `internot_def` crate.
  - Packs `worlds/us` (nine sections) and `worlds/us-tiny` (one file extending `us`).
  - `worlds/README.md`, the guide for anyone.
  - `internot_society::params` is typed sections plus `Params::{prototype, tiny, embedded, load, from_pack}`.
  - `build.rs` embeds `worlds/`.
  - Heritage groups are pack-defined: `Heritage` is an index, and there is no enum.
- **Checks:**
  - The fingerprint is unchanged (re-recorded baseline with heritage as an index: tiny `830e0ddfaf6b1702` / `f3bd18ec8c5afb2e`, prototype `fc1cc62ffcc638bb` / `bc764f24987f394f`).
  - All 860 workspace tests pass.
- **How to work from now on:**
  - New statistics go in the pack, never in code.
  - A new section is a typed struct in its consumer's crate, read with `pack.section(name)`, checked in `validate`, and documented in `worlds/README.md`.
  - Structural bounds stay in code and are listed in the README.
  - Check refactors that must not change the world with `examples/world_fingerprint.rs`.
- **Next:** N1 resumes on the packs: group fertility and mortality rates, calibration, then names.

**Names and heritage (N1): heritage DONE, names BUILT (2026-10-01)** (plan outcome for details).
- **Names:** `internot_society::names`, with data and rules in `worlds/us/names.ron` and `data/names.bin`.
  - First names come from SSA by year, split by group with Census 2020 and raked to the world's own births.
  - Surnames are inherited, change at weddings by era, and can revert after a separation.
  - Marriage is now a fact of each union (`World::marriage_date`). 86.5% of couples living together are married in 2023, against 86.7%.
- **Speed:** `first_name` 5.5 / 24 µs and `surname` 51 / 286 µs (p50 / p99).
- **Debt:** Asian names mix origins (needs origin countries); no women's middle-name pool; names frozen after 2025.
- **Heritage is in the ledger and calibrated** (plan outcome):
  - composition 1850–2020 within about 2–3 points of the Census;
  - intermarriage by group and sex matches Pew for 1980 and 2015;
  - group fertility and mortality are pack factors, with adult mortality solved to the e0 gaps.
- **Debt:** children join the mother's group (Hispanic 16.7% against 19.6%); same union rates for every group; no generation effect in intermarriage.
- **Build:** 4.9 s.
- **Why heritage first.** First names and surnames depend strongly on race and Hispanic origin (Census 2020 name files). For a family's names to make sense, partners must mostly share a heritage, at real intermarriage rates by era (Pew: 3% of newlyweds in 1967, 17% in 2015).
  - A heritage-blind ledger can't give that. Any labelling of a heritage-blind union graph either mixes families at random within a few generations or lets one label take over.
  - So heritage must shape the markets, not only the names.
- **Founder decision (2026-10-01): heritage goes in the ledger.**
  - Lineage groups become region × heritage.
  - Five groups follow the Census 2020 name files: non-Hispanic White, Black, Asian and Pacific Islander, AIAN, and Hispanic of any race.
  - "Two or more races" is not a group; it comes from mixed parents.
  - The founder asked whether heritage affects the couple or only the children. The answer is both: it shapes who partners with whom, and children inherit it.
- **Cost measured before design (10 groups against 2, uniform mixing, the worst case):**
  - world build 10.4 s against 2.7 s;
  - peak RSS during the build 2.6 GB against 1.6 GB.
  - The time is the cross-group market's dense rounding, which grows with groups² (the known regions² debt).
  - Sparse rounding (per row group: cumulative shares and systematic placement, O(couples · log) instead of O(rows · cols)) is part of the plan.
- **v1 scope (debt from the start):**
  - every group has the same fertility, mortality and union rates; real groups differ, so composition will drift;
  - children join their mother's group in the ledger (as with regions, D-R1.1), and their names draw on both parents.

**Phase 2, residence: research DONE, design A chosen (2026-10-01).**
- **Notes:** `research/2026-10-01-residence-data-and-targets.md` (data in `datasets/geo/`, mobility and proximity targets) and `research/2026-10-01-residence-algorithms.md` (the obstruction, P1/P2).
- **Data in hand** (public domain):
  - tracts with population and centres (2000/2010/2020), the Gazetteer, GNIS neighbourhoods and CBSA metros;
  - Forstall county populations 1800–1990 plus estimates to 2025, and WP27 largest cities 1840–1990;
  - a street-name frequency list computed from TIGER.
- **Binding targets:**
  - at 26, 30% are in the same tract as at 16, and 58% within 10 miles;
  - 59.8% of adults have their nearest parent within 30 miles;
  - about 76% of domestic moves are under 50 miles;
  - mover rate 20% (1948–70) → 7.8% (2023).
- **Finding:** an exact roster must evaluate every household that could have entered N.
  - Each move channel is either static-indexed (cost ∝ |N|, no dependence on the current location) or confined to a closed unit (cost ∝ that unit's history, once, then memoized).
  - Option B as decided can't put new households near parents (it fails the targets above).
- **Recommended: P1.** About county-sized closed basins; formation near parents or partner via kin edges; local moves relative to the current address; long moves as static itinerary flows.
  - Exact both ways. Forward is about 0.1 ms; a basin's first roster takes seconds at full scale, then is memoized.
- **Open coupling:** partner geography. The ledger pairs within lineage regions, which don't follow residence, so realistic partner distance needs a multiregional (counted-residence) ledger.
- **Founder, 2026-10-01:** P1's forward replays a household's moves, which "does not sound very f(time, id)". Find a direct construction for both questions, and move all the math into `procedural_core`.
- **Answer** (`research/2026-10-01-residence-closed-form.md`):
  - **Forward is direct by hierarchical regeneration.** A move at level k redraws levels ≥ k, so the level-k place at t is the draw at the last level-≤k move. That is one `last_before` per level, O(L · log T), with no replay.
  - **Rosters are direct for static channels**, through bijective draws.
  - **"Near the parents" can't be both direct and exact** (the catchment argument). It needs a closed unit (A, cached), place counted in the ledger (B), or dropping it (C).
  - **Recommended: A**, with B at state level later for partner geography.
- **Founder decision (2026-10-01): A, closed units.**
  - Forward by hierarchical regeneration.
  - Rosters exact, by enumerating a unit's history once and caching it.
  - Channels that cross a unit's boundary are static (seeds, itinerary bijections).
  - "Near the parents" and local moves work inside units.
  - Debt from the start: partner geography (B at state level is the later fix), and first long moves that can't depend on an inherited place.
  - **Next:** design and prototype on a synthetic geography (forward cost, first-touch roster cost, the proximity targets) before production code.
- **Design A written (2026-10-01):** `specs/2026-10-01-residence.md`.
  - **Units:** unions and single spells, each with a position process over its whole span, latent while nobody lives there. Households live at their anchor unit's position.
  - **Forward:** nested regeneration over basin ⊃ county ⊃ cluster ⊃ tract. New units start near their source (leaving home: the parent's unit; a union: one partner's unit; after a separation, the keeper stays). Formation never leaves the basin.
  - **Long moves:** a counted flow, per (birth block, year), with a keyed selection. Destinations are tiered around the lineage region, exactly invertible.
  - **Rosters:** each basin's closure E(B) is the least fixed point from its static entries through sources, proved equal to "every unit ever in B", and cached.
  - **New core primitives:** `geo` (haversine), `fixpoint::closure`, a tiered (ultrametric) destination kernel, and possibly a segmented keyed permutation.
  - **Open:** move hazards, distances and commuting-zone data. A research pass is writing `research/2026-10-01-residence-moves-and-basins.md`.
  - **Next:** the core primitives, then a prototype on a synthetic geography (exactness on the tiny world, then costs on the prototype world), then real geography and calibration.
- **Core primitives added (2026-10-01):** `dmath::{asin, atan2}`, `geo` (haversine), `fixpoint::closure`, `perm::SegmentedPerm`, each with property tests and golden values (553 core tests pass).
- **Prototype `internot_society::residence` (2026-10-01): exactness PASSES.**
  - Pack section `residence.ron`, with provisional rates.
  - `Places`: a tree with weights by decade, and a synthetic builder.
  - Units, sources, `pos` (nested regeneration), counted long-move and seed flows (`SegmentedPerm`), per-basin closures, `roster`, `address`.
  - **Exactness:** `tests/residence.rs` checks the tiny world (2 seeds × 7 dates, 6 basins × 36 tracts): every tract's roster equals brute force over every person, and county rosters equal the union of their tracts.
  - Bug found on the way: a union starting exactly at the world's start was not treated as seeded. Units that start by the time their people enter the world are seeds.
  - **Timelines:** `Residence::timeline` replays a unit's events into stretches. It agrees exactly with the direct `pos` (property test, 1,062 units and 5,878 stretches). Histories store the stretches with a per-tract index, so a roster is a range scan plus occupancy checks.
  - **Speed work, in order:**
    1. Memoized per-person facts and per-unit info: 3.8 → 0.75 s per tiny basin.
    2. A fast path for first independence.
    3. Candidate pruning (descend through a child only if they were still a dependent at 31, or died while the unit lasted).
    4. `fixpoint::closure_layers` (new in core), with rayon expanding each layer and sharded memo locks.
    5. Caches cleared after each history.
    6. Parallel occupancy checks in rosters.
  - **Bug found by the exactness test while pruning:** grandchildren are orphaned into a grandparent's care whenever the parent dies before they leave, at any age. Fixed.
  - **Measured on the prototype world** (13.8M people ever born; synthetic tree of 120 basins, 11,520 tracts; `examples/residence_report.rs`, release):

    | Measure | Result | Budget |
    |---|---|---|
    | `address_of`, cold | p50 259 µs, p99 619 µs | about 1 ms ✓ |
    | `address_of`, warm | p50 24 µs, p99 279 µs | ✓ |
    | first touch of a basin (210k–420k units) | 6–12 s | ≤ 10 s for the largest ✗ for the biggest |
    | warm tract roster | p50 0.9–1.5 ms, p99 1.5–2.8 ms | about 10 ms ✓ |
    | memory per cached basin history | about 100–140 MB | |

    - The closure's cost is mostly the kinship layer's union lookups (kin repair inside `repaired_partner`): the deferred kinship performance debt.
    - Large real basins must be split.
    - The world itself is now 2.5 GB RSS after build (heritage multiplied cells; debt).
- **Real geography (2026-10-01):** `internot_society/data/distill_places.py` writes `worlds/us/data/places.bin` (5.6 MB).
  - Contents: 2020 tracts with population centres; ERS 2020 commuting zones; clusters of at most 16 tracts; weights by decade 1840–2100 from Forstall county censuses scaled to state totals, 2000/2010/2020 counts and the 2025 estimates. Puerto Rico is dropped.
  - Census-region shares match CPH-2-1 within 0.3 points from 1850 to 2020, and the 2020 total is exact.
  - `places.ron` maps states to the pack's lineage regions: east is the Atlantic seaboard, 62.6% of people in 1840, against the 0.6 founder weight.
  - `Places::from_params` parses it.
- **First realism report** (`examples/residence_realism.rs`, provisional rates): families scatter far too widely.

  | Measure | Model | Target |
  |---|---|---|
  | nearest parent under 30 mi | 17.8% | 59.8% |
  | nearest parent 500+ mi | 41.7% | 9.2% |
  | same tract at 26 as at 16 | 16.9% | 30% |
  | 500+ mi from where they were at 16 | 29% | 10% |
  | West's share in 2020 | 12% | 23.7% |

  - The cause: every move between zones is static and tiered only around a half-continent lineage region.
- **Research note done:** `research/2026-10-01-residence-moves-and-basins.md`.
  - Keep zones whole: splitting turns 30–50% of local moves static.
  - A static key works for long moves only if fine: the birth state's overlap is 0.62, the current state's 0.56, national 0.42.
  - Rates by age and era with gamma frailty and no duration term; level shares; formation channels; the cross-zone kernel (piecewise power law, median 240 mi).
- **Restructure decided (within design A):**
  - The closed unit becomes the **area**: whole zones grouped by state, capped, with a zone too big for the cap as its own area. Tree: area ⊃ zone ⊃ county ⊃ cluster ⊃ tract.
  - Moves between zones inside an area draw by gravity around the unit's home zone (its first zone). That keeps the regeneration lemma, and per research §4d a home key predicts as well as the origin.
  - Only moves between areas are static counted flows.
  - Rates, frailty, level shares and formation channels follow the research.
- **Restructure built (2026-10-01), still exact** (all three residence tests pass).
  - Five levels, real places; `places.bin` v2 has 62 areas of whole zones, with counties over 2M cut into 1M parts.
  - Seeds and own-region shares use each region's tract weights per area.
  - Core gained `sample::{gamma, frailty}` and `curve::piecewise_power` (556+ core tests).
- **Realism is blocked by residence-blind kinship** (spec §7, `examples/residence_realism.rs`, `examples/residence_trace.rs`):

  | Measure | Model | Target |
  |---|---|---|
  | natives outside their birth state | 43–58% | 21–34% |
  | adults with a parent under 30 mi | 16% | 59.8% |

  - Traces show people jump when their household composition changes. The main cause is partners paired from a half-continent market: one partner relocates at every union.
  - Roommates (region-wide frames) and custody add to it.
  - Turning roommates off, lowering move rates, or making county and zone moves rare barely helps.
  - **Experiment** `worlds/us-states` (one lineage region per state): better (500+ mi from parents 30% against 45%), but the world build is 46 s against 5 s, and pairing is by birth state, stale for migrants.
- **Founder decision (2026-10-01): B at area level.** The ledger tracks each person's residence area and pairs partners by it (spec §8).
  - **Design:** `specs/2026-10-01-ledger-areas.md`.
    - Blocks by (birth year, upbringing area, heritage).
    - Single migration classes before a first union.
    - Couple move classes on cells.
    - Markets per area.
    - Residence uses exactly these classes for moves between areas.
  - **Step 1 done** (regions = areas, as a variant pack):
    - `clear_year` indexes each market's members instead of scanning every block: bit-identical, and 53 regions now build in 36 s against 46 s.
    - `places.ron` can set `by_area: true`.
    - `internot_society/data/make_area_regions.py` generates `worlds/us-areas` (62 area regions from the 1840 shares; immigrant weights a proxy).
    - The `us-areas` build takes 40 s and 5.2 GB (two regions: 5 s and 2.5 GB). The cost is mostly one class IPF per market per year (310 local markets).
  - **Anchored gravity:**
    - core `stream::nested_regen_anchored`: moves at a level draw around the level's anchor, its node at the last coarser regeneration; it matches a full replay exactly;
    - residence uses it at the zone, county and cluster levels (`LevelGravity`; `local_gravity` in the pack), formation draws its first level by gravity around the source, and long-move rates are ×0.4;
    - the residence tests stay exact.
  - **Realism of `us-areas` with a 6–10% inter-area marriage share** (experiment):

    | Measure | Model | Target |
    |---|---|---|
    | natives outside their birth state, 1900 / 1930 / 1960 / 2000 / 2020 | 19.8 / 24.3 / 28.9 / 34.6 / 37.4% | 20.9 / 23.8 / 29.7 / 32.5 / 33.7% |
    | adults with a parent under 30 mi | 24% (+12.6% coresident) | 59.8% (+5.9%) |
    | adults 500+ mi from a parent | 21.8% | 9.2% |
    | born 1990–94, under 10 / 100 mi at 26 against 16 | 39.6 / 65.5% | 58 / 80% |
    | moving to another area per year, 1950 → 2019 | 1.4 → 2.4% | 3.1 → 1.5% |

    - Leaving home lands a median 4 mi from the mother. After that, the child's own moves spread it: median 17 mi, 90th percentile 680 mi.
    - The rising cross-area rate comes from the national open (intermarriage) and same-sex markets.
  - **Local open and same-sex markets** (`unions.ron` `local_open_markets`, default off, so `us` is bit-identical). `us-areas` turns them on, with a 6–10% inter-area share.

    | Measure | Model | Target |
    |---|---|---|
    | natives outside their birth state, 1900 / 1930 / 1960 / 2000 / 2020 | 19.9 / 23.6 / 28.5 / 32.5 / 32.7% | 20.9 / 23.8 / 29.7 / 32.5 / 33.7% |
    | nearest parent or parent-in-law (PSID counts in-laws): under 30 mi + coresident | 33.7 + 14.1 = 47.8% | 59.8 + 5.9 = 65.7% |
    | nearest parent or parent-in-law, 500+ mi | 12.1% | 9.2% |
    | born 1990–94, under 10 / 100 / over 500 mi at 26 against 16 | 39.3 / 68.9 / 13.8% | 58 / 80 / 10% |

    - Coresidence with parents is overcounted by L3 (14% against 6%).
  - **Founder decision (2026-10-01): finish B (steps 2–4), then calibrate, fix build performance, and make `us-areas` the default.** Partners paired across a whole area stays a known gap (zone-level pairing was not chosen).
  - **The question, as it stood** (2026-10-01): what's left is mostly *within* areas.
    - Partners are paired across a state-sized area, so one partner usually relocates 100+ mi; research §5d says 75% of couples lived within 50 km.
    - Steps 2–4 (migration classes, couple moves, upbringing area) improve cross-area consistency, where realism is already close.
    - Within-area partner locality needs pairing at zone level (588 zones), which multiplies ledger cost again.
  - **Debt:**
    - the ledger build at 62 areas is 40 s and 5.2 GB;
    - the cross-area move rate rises over time (1.3 → 1.9%) where CPS falls (3.1 → 1.5%); calibration pending.
  - **Stages 2b and 2c built (2026-10-01):** births by area with cross-group parent lines, and migration classes as native cohorts (spec §6). Details and numbers are in spec §7.
  - **Ledger defects found at full scale, fixed (2026-10-01).** They were mostly already in the ledger and showed once 62 areas × 5 heritages made blocks thin. Before the fixes, `us-areas` had 5.2M people against 13.8M, and deferred 21% of pairs:
    1. **Caps:** the cap was `⌊expected never-partnered⌋` per market, so small pools were starved. It is now rounded up, keeping the integer bound on members available. `us-tiny` had lost 18% of its solved couples.
    2. **Founders' first year:** that market was capped by the *never*-partnered, so in `us` only 25% of women born 1800–1819 were ever partnered. Founder pools are now `founding` during it.
    3. **Deferral:** an isolated couple's want is now carried to next year, so the union is delayed, not lost. Before, `us-tiny` lost a quarter of its pairs this way.
    4. **Arrival couples:** rounded with core `partition::round_unbiased` (new: `⌊x + u⌋`, keyed).
    5. **Residual deaths:** second-union members were subtracted twice (a source index read as a cohort index), so the never-partnered died too young. In area mode, 84.5% of women born 1980 reached 50, against 94.5% in `us`.
    - Area mode also needed: `settle` takes each market group's exact count; divorced sources record each remarriage part's area; the first year's cells are sorted.
    - **Default worlds changed** (realism report diffed against the old behaviour; temporary `LEGACY_*` env switches reproduce the old fingerprints exactly):
      - population 13.8M → 17.3M (the founders now have their unions);
      - ever partnered, 1970 cohort: 89.1 → 91.1%;
      - men's e0, 1970–2010 cohorts: +0.8 to 1.4 years (the residual fix);
      - same-sex couples in 2019: 1.43 → 1.68% (ACS about 1.5%; recalibrate);
      - other measures move within noise.
    - **New fingerprints** (after the build work below): tiny `f6eb885c416edd0c` / `1f61e7c1386778d9` / names `d894734870db2d8f`; prototype `2e34fbc85c3bc774` / `be7cd490145a4249` / names `4e44d4de2914f815`.
    - **`us-areas` now matches `us`:** women born 1980 reaching 50, 94.7 against 94.5%; ever partnered among them, 89.2 against 89.6%; 16.5M people.
  - **`us-areas-tiny` is four areas** (PA 1, PA 2, OH 1, OH 2), from `internot_society/data/make_area_test_pack.py`: a subset of the place tree, extending `us-tiny`.
    - With all 62 areas, blocks held a few people, and pairwise kin repair could not avoid siblings: two full siblings ended up in a two-couple cell whose other pairing was also related.
    - The exhaustive kinship suite passes on it (`TEST_PACK=us-areas-tiny`) and on `us-tiny`.
  - **Close kin at prototype scale is rare but not zero** (`examples/kin_debug.rs` checks every couple in parallel, about 35–50 s):
    - `us`: 1 sibling couple in 9.0M (old behaviour: 0 in 7.1M);
    - `us-areas`: 5 in 8.7M, all full siblings born 1910–1920 in small groups (AIAN, Hispanic) in small areas.
    - Each is a repair pair in which both pairings are related; in `us` the pair sat in a six-couple cell. R1's "zero on the full prototype" was luck, not a guarantee: pairwise repair fails at about 10⁻⁷ per couple, more where blocks are thin.
    - **Founder decision needed:** accept it as measured debt; or repair groups of three (much rarer failures; about +50% `related` calls per partner lookup); or check small cells at build time and re-key failing ones (year order, since kinship depends on earlier repairs).
  - **Build time: `us-areas` 475 s → 63 s** (`us` 5.5 s), each step but two checked bit-identical by fingerprint. Phase timers, not pprof (it misattributed twice), found the costs:
    - **Caps** rescanned a pool's cohorts on every call (about 1,100 markets a year, each capping every row and column). Pools now cache each area's availability and wants once a year (`Pool::refresh`, sorted per area), kept current by `settle`, which works on each area's cohort list.
    - **Empty rows** (most seekers get no couple in a given market) were capped and hashed anyway; they are skipped.
    - **`record`** walked every block for each market group's side and allocated arrays sized by the largest block id; it now splits the slices at the group's blocks and sorts.
    - **Rounding** built a `classes × columns` cumulative table per market. Core `GroupedIpf::{columns_by_class, class_weights_cumulative, round_row}` (new, with tests and golden values) rounds in two levels, over column classes and then within a class: the same law, different bits (one of the two world-changing steps; the other is the founding-capacity fix).
    - In parallel now: rates, availability and seekers per pool; each market's participants; mortality (the living still summed in pool order). The open-market shares and the age-gap kernels are tabled once.
    - **Left:** the class IPF is about 190 s of CPU (272k solves, 48 passes on average, 63% stopping at the 60-pass cap; the row-margin error is 5e-5 of the mass at 60 passes, 0.24% at 20, 1.9% at 10, so the passes stay). Warm starts from last year's factors, or over-relaxed Sinkhorn, would reach the same fixed point faster; later.
  - **Memory is the next blocker for making `us-areas` the default:** peak RSS 9.3 GB against 2.9 GB for `us`. The ledger alone is 4.9 GB (1.6 GB for `us`) and the world's layouts add 4.4 GB.
    - The ledger holds 9.5M union cells (61% of one couple), 16.4M partner slices, 2.9M cohorts and 5.9M divorced sources, each cell with three small heap vectors, and the world keeps the whole ledger.
    - Fixes to weigh: arenas instead of per-cell vectors; dropping what the world doesn't read after its build.
  - **L3 in area mode:** `World::migration(x)` gives a native's single long move, `(destination, time)` at a keyed time in the class's move year, if they are alive and never partnered by then. Leaving home is the earliest of independence, first union and that move.
  - **Tests on other packs:** `TEST_PACK=<pack>` runs the kinship and household suites on any pack in `worlds/` (default `us-tiny`). Both pass on `us-areas-tiny`.
    - The household suite's "kinless orphan" check now matches the guardian rule: an adult sibling counts only if on their own. A paternal half-sibling still living with his own mother is not a guardian.
  - **Stage 3 BUILT (2026-10-01): couple moves in the woman's plan leaf** (spec §8).
    - `ledger::CoupleMoves` splits each women's plan leaf over "stays" and `(move offset, destination)`, by keyed systematic apportionment. The move year's chance is the long-move rate at the woman's age; destinations follow `Migration`.
    - The ledger records each birth in the area the mother lives in that year. The world splits the same leaves (`move_key`), builds its birth-line area runs per leaf, and keeps a sorted table of moving leaves.
    - `World::couple_moves(x)` gives each union's move, aligned with `unions(x)`; a widow still moves. `World::union_birth(x)` tells plan births from non-union ones.
    - **Tests:** the exhaustive suite passes on both packs, now 12 tests. The new test checks that both partners see the same move and that this union's later plan births are in the destination. On `us-areas-tiny`, 25.5% of unions move during the union (1900–1990, before calibration).
    - **v1 debt:**
      - one move per union;
      - same-sex couples don't move;
      - the divorced re-partner in the formation area;
      - a non-union birth to a woman whose union moved is placed in her cohort's area, not where she lives.
    - **Cost:** `us-areas` build 64.6 s, 9.4 GB; default worlds bit-identical.
  - **Stage 4 BUILT (2026-10-01): residence on the ledger's areas** (spec §9). Invariant: everyone independent lives in their ledger area, `World::area_at(x, t)`.
    - **Upbringing area:** a child's block is the area of their upbringing, where the family lives at 18 (`ledger::upbringing_moved`): the couple's destination if the move comes by then and the couple hasn't separated by then.
    - **Couple moves:** a couple's move is made by whichever partner is alive (the family moves).
    - **Residence** (area mode) takes every move between areas from the ledger: the change points of `area_at`. These are a single's migration, the couple's move, and a widowed partner's return to the formation area at the couple's planned separation. Its own long-move flows are off.
    - **Sources:**
      - a union keeps or forms near a partner's household only if the household is in the cell area, otherwise it is fresh there;
      - a first single spell starts near the household only if the household is in the person's ledger area, otherwise fresh there;
      - a separated couple that had moved returns near where it formed (`Source::Back`).
    - **Closures** take ledger entries, a superset that is cheap to enumerate:
      - first spells of the area's blocks (natives and immigrants) and of migrants into it;
      - `World::unions_formed_in`;
      - `World::couple_movers_into`, either partner;
      - widowed partners returning at the planned separation.
    - **L3:**
      - leaving home also happens when the parents' household moves after the child's upbringing year;
      - kin hosting only joins kin in the same ledger area;
      - roommates must be counted in their frame's area (`roommate_ok`).
    - **Tests:**
      - `TEST_PACK=us-areas-tiny`: residence rosters equal brute force (2 seeds × 7 years);
      - a new test checks that every independent person lives in their ledger area: 109k in their own homes, 364 roommates, 5,160 kin guests, zero elsewhere;
      - the kinship test checks that every union forms in the woman's ledger area except the move-year window (1.1%: a migrant partnering before her move date);
      - every suite passes on both packs; default fingerprints unchanged.
    - **v1 debt:**
      - divorced and widowed singles don't move between areas;
      - the divorced return to the formation area;
      - no moves to kin in another area;
      - migrants have no roommates in their new area;
      - the household suite on the area world runs 66 s against 32 s (`couple_moves` in `leave_time`).
  - **Realism of `us-areas` after stage 4** (`examples/residence_realism.rs`, uncalibrated):

    | Measure | Model | Target |
    |---|---|---|
    | West's share, 2020 | 8.8% | 23.7% |
    | natives outside their birth state, 1900 / 1960 / 2020 | 33 / 40 / 36% | 21 / 30 / 34% |
    | nearest parent or in-law under 30 mi, coresident included | 44.0% | 65.7% |
    | born 1990–94, under 10 / 100 mi at 26 against 16 | 39 / 72% | 58 / 80% |
    | moving to another area per year, 1950 → 2019 | 1.5 → 0.8% | 3.1 → 1.5% (now falling, as it should) |

  - **Founder decision (2026-10-01): calibration only needs to be close; focus on performance and features.** Fix gross defects with cheap levers; no calibration machinery (such as a doubly constrained gravity fit) unless asked.
  - **Memory work (2026-10-01), every step bit-identical (fingerprints and the `lookup_timing` checksum):**

    | | `us-areas` RSS / peak | `us` RSS / peak |
    |---|---|---|
    | before | 9.6 / 9.6 GB | 2.95 / 2.95 GB |
    | after | 5.6 / 6.3 GB | 1.5 / 2.2 GB |

    - `World::memory_report()` (printed by `examples/build_time.rs`) gives heap bytes and allocations by component.
    - **Ledger detail freed:** `World::build` hands each block's ledger detail (union cells, divorced sources, mothers), which only layouts read, to its layout build and frees it after. `World::build_keeping_ledger` keeps it, for the kinship suite, `world_fingerprint` and `realism_report`.
    - **Shared life tables:** one per (birth year, heritage), not per block.
    - **Cohort arenas:** each cohort's sub-cells, coarse starts, non-union plans and parent line now live in per-block arenas. This took allocations from 19M to 2.5M.
    - **Tight arrays:** layout arrays are shrunk to fit, and glibc's `malloc_trim` runs after the build.
    - **Tried and dropped:** mimalloc as the global allocator. It gave higher RSS (`us` 2.8 GB, `us-areas` 6.3 GB) and only an 8% faster build.
    - **Left in `us-areas`:** about 1.5 GB of build-time fragmentation, plus the biggest components: birth rows 620 MB (dense ages × plan columns), cells 609 MB (64 B each, by design), cohorts 492 MB (2.9M, from migration classes), sources 367 MB, sub-cells 308 MB, ledger cohorts 271 MB.
  - **Lookups in area mode, about twice as slow as `us`** (p50/p99, µs): father 10/31, union 10/30, children 4/54, siblings 5/69, household 1.5/275, `area_at` 11/65. `couple_move_of` resolves one union's move without the partner's full union list, which halved `area_at` and the household tail.
  - **Open:** build time (68 s for `us-areas`); lookups; making `us-areas` the default.

**Directory cutover: v1 DONE (2026-10-01)** (founder chose it as the next feature; spec `specs/2026-10-01-directory.md`).
- `internot` serves the society world. `Universe::society` is built once per process (`INTERNOT_PACK`, default `us`; `INTERNOT_SEED`, default 42).
- **The `directory` service:**
  - `read_person`: names, with surname changes; birth, death and age; heritage; immigration; partner; unions with marriage dates; parents, children and siblings; the household with members, relations and address.
  - `read_household`.
  - Both take `at` (ISO 8601), defaulting to the Universe's `now`.
- **Removed:** `people` (with its tests and the leaked name files). Its other data files (NAICS, SOC, CIP, languages, time zones, hobbies) stay in `internot/data/`, unused for now.
- **Tests:** `internot/tests/directory.rs` runs every view through the JSON registry on `us-tiny`. Partners, parents and children, and household members agree both ways; time travel and errors are covered.
- **MCP smoke test over stdio:** the server is ready in 6.9 s (`us`); `read_person` takes 22–32 ms.
- **Debt:**
  - no rosters ("who lives here") and no name search (needs an index);
  - residence's memo caches grow without bound in a long-running server;
  - no individual attributes yet.
- **The proposal, as it stood** (spec §8):
  - A person's ledger area is static: their block's area, or the destination of their last counted long move.
  - The ledger counts residents per (block, area, year) and pairs by ledger area.
  - Long moves become origin-keyed, and roommate frames go local.
  - Design A stays inside areas.
- **Kinship lookups have slowed since the R1c gate** (median p99, measured by the core-migration A/B, the same before and after the migration):

  | Query | Now | At the R1c gate |
  |---|---|---|
  | death | 3.3 µs | 1.77 µs |
  | father | 16.6 µs | 12.3 µs |
  | children | 28.2 µs | 15.2 µs |
  | siblings | 37.6 µs | 22.4 µs |

  The likely cause, not yet profiled, is the five heritage groups multiplying blocks and cells. This joins the deferred performance debt; size the budgets to workloads when it is picked up.
- **Math moved to `procedural_core`: DONE (2026-10-01)** (founder request). APIs are in the reference below ("Layer 0.6").
  - `fit` (new): `ipf`, `GroupedIpf`, `rake_columns` / `raked_weight`, `tilt_mean`.
  - `partition`: `round_systematic_cumulative`, `SparseCounts`, `sweep_capped`, `apportion_largest_remainder` (+ `_capped`), `trim_largest_first`.
  - `table` (new): `CumTable<T>`.
  - `stream`: `nested_regen`, `nested_regen_state` (new; tested against a full replay).
  - `internot_society` now calls these. Its own copies are gone: `ledger::{ipf, scale_cols_and_sum_rows, Sparse, sweep, apportion_capped, trim}`, `plan::apportion`, `params::tilt`, `names::Table` and the names raking loop.
  - **Bit-identical, checked:** `world_fingerprint` (now also hashing first and middle names, marriage dates and surnames at four dates) gives tiny `4734ebc318bcef8c` / `ed3c73a58212a955` / names `e894610a479cd2e5` and prototype `4b4b0fc1e69824f0` / `824be8ce7c830c39` / names `91447aced082ca14`, before and after. The `lookup_timing` sum is `200ecbd68ce20f73` both times.
  - **World build:** 4.59 s after against 4.67 s before (min of 3).
  - **Not unified:** names raking and dense IPF reach the same fixed point but iterate differently (factor form with a fixed 60 passes, against matrix form with a tolerance), so merging them would change the worlds. Both are kept, and the docs say how they relate.
- **Round 2, "all math in procedural_core" (founder, 2026-10-01): DONE, bit-identical.** Every generic formula in `internot_society` and `internot_def` moved; the world fingerprints, names hash and `lookup_timing` sum are unchanged.
  - **New core modules:**
    - `interp`: `bracket`, `lerp`, `log_lerp`, `piecewise_linear`, `piecewise_log_linear`, `step_at_most`, `step_below`, `interval_value`, `first_above`;
    - `life`: `survival`, `survivorship`, `first_event_pmf`, `cumulative_incidence`, `cure_hazard`, `Siler`, `stable_age_weight`, `add_conditional_deaths`, `invert_survival`, `invert_cumulative`;
    - `curve`: `log_logistic_cdf`, `logistic_rise`, `logistic_floor_quantile`, `gaussian_bump`, `gaussian_kernel`, `ramp`, `rogers_castro_labour`, `symmetrized`, `dilated`;
    - `pmf`: `normalized`, `floored`, `mix_into`, `one_fewer_or_none`, `spread_evenly`, `cumulative_normalized`, `positive_mass`.
  - **Additions to existing core modules:**
    - `table`: `Coarse`, `CoarseRow` (`first_fail`, `count_le`), `coarse_index`, `coarse_len`, `BucketIndex`;
    - `sample`: `exp1_by_inversion`, `pick_linear`, `lazy_conditional`;
    - `partition`: `pair_group`, `even_parts`, `segment_offset`, `locate_in_segments`;
    - `perm`: `least_cost_assignment`;
    - `stream`: `Epochs`;
    - `bits`: `ones`.
  - **Where each came from:**
    - `internot_def` `Series`/`VecSeries`/`Steps`/`Bands`/`Ranges::at`. `internot_def` now depends on `procedural_core`.
    - `params`:
      - `interp_with`;
      - mortality's log-linear multipliers, the Siler hazard and its year integral;
      - first-union log-logistic and cure hazard;
      - gap-kernel symmetrizing and dilation;
      - parity shift, offset mix, non-union Gaussian, dissolution lerp and band spreading;
      - Rogers–Castro;
      - `normalized` and `floored`;
      - the independence logistic quantile and the households' logistic, Gaussian and ramp curves.
    - `ledger`: `period_survival`, `never_partnered_share`, stable-population weights, arrivals' ever-partnered share, `bits`.
    - `plan`: `union_age_density`.
    - `world`:
      - `Coarse`, `Indexed`, `coarse_index`, `invert_survival`, `invert_cumulative`;
      - the id buckets, `repair_group` and `PERMS` / `least_related`'s search;
      - life-table construction, the residual's conditional deaths and cumulative;
      - the two-stage death draw, the birth-row coarse search, the same-sex band pick.
    - `household`: the epoch grid, band offsets and `CHUNKS` (now `even_parts`).
    - `names`: the exponential marriage delay, the legal-year crossing, the group shares.
  - **What stayed, and why:**
    - **Ledger projection pools** (`CohortPool`, `DivPools`, `Pool::want/cap/settle`): bookkeeping of expected and counted members per cohort; no formula beyond sums and floors.
    - **Market structure, `clear_year`, `class_cells`, `split_classes`, `deisolate`, `defer_isolated`**: ledger ontology (who meets whom, which cells exist).
    - **Plan partitions** (`PlanShares`, `leaf_births`, `arrival_plans`, `nonunion_plans`): domain compositions of core apportionment.
    - **Kin repair's predicates** (`related`, `same_mother`, group walks): kinship ontology. Only the permutation search moved.
    - **`Repartnering::hazard/base`, `nonunion_count_pmf`, `class_pmf`'s share scaling, `status_affinity`**: products and caps of pack factors, the wiring from pack to core.
    - **The surname "rare tail" lift**: a correction for the Census file's unlisted 12%, a data quirk rather than a method.
    - **Rounding of expected counts** (`(rate · alive).round()`): one rounding per pack-scaled count.
    - **`partition_point` searches over the crate's own arrays** (slices, leaves, cohorts): std binary search on data layouts.
    - **`Leaf` bit packing and the other layouts**: data layouts.
    - **`internot_def` validation**: checks, not math.
    - **Timestamp placement within a year** (`year_start + key.below(...)`): a keyed uniform offset.
  - **Speed:**
    - `lookup_timing`, interleaved A/B against HEAD (3 runs each, median p99): death 3.0 against 3.3 µs, mother 1.78 against 1.75, father 16.7 against 16.6, union 15.4 against 15.2, children 28.3 against 28.2, siblings 37.9 against 37.6. All within run-to-run noise.
    - World build, interleaved, min of 4: 4.63 s against 4.64 s.

**Blocked on founder decisions** (each written up, with options and a recommendation):
0. ~~Kinship current-status calibration~~ **Decided 2026-09-30: do what is realistic, statistics not rules** (see Direction). Immigrants arrive as they are and partner at rates for people like them. Being worked on (R1d, below). The original question: Too many people are divorced and single at 45+ (Census A1), and older single immigrants count as never partnered. Recommendation:
   - (a) recalibrate R1c's re-partnering hazard to current-status targets (A1 divorced by age, plus cohabitation). It was calibrated to remarriage only, while unions include cohabitation.
   - (b) give older single arrivals a previous union abroad (divorced or widowed), so they enter the re-partnering markets.

   Then reassess widowed re-partnering and third unions (now capped at two unions).
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
   - The old `internot/data/{first,last}_names.json` had unrecorded provenance and likely derived from the 2021 Facebook leak (via `philipperemy/name-dataset`). It served only the old `people` crate and was deleted at the cutover (2026-10-01). Never restore it.
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
- **Finer structure costs lookups even when memory stays flat.** R1c's exact-year classes made about six times as many cells.
  - Sub-cells nested inside cells doubled instructions.
  - Flattening them into cells and then compacting to one cache line still left union at +41%: longer sorted arrays and colder cells.
  - Index long sorted arrays (`Coarse`, every 16th value).
  - Don't group columns to save memory if lookups must then scan the group.
- **Largest remainder is biased on small partitions:** a one-member split always goes to the modal class. Use keyed systematic apportionment (`partition::apportion_systematic`) wherever cells can be small.
- **Build-time work (2026-09-30):**
  - **Prove refactors bit-identical with checksums**: a hash of the ledger's `Debug` output plus `examples/lookup_timing.rs`'s answer sum. All ten build changes passed this way, so no realism rerun was needed.
  - **pprof line attribution is unreliable for inlined code.** It blamed `trim` for 18% when timers showed 64 ms. Mark candidates `#[inline(never)]` temporarily, or time them.
  - **A search into another block's cells is a cache miss per step, even at build time.** Precompute cross-block facts in one pass instead.
  - **Parallel tasks run ~1.7× slower each on this laptop** (turbo drops, memory contention), even at 8 threads. Parallelism pays only where the work is large. Keep glibc allocations and frees on the same thread; cross-thread frees lock the owner's arena.
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

### Known defects of the removed `people` service (history; it was cut over on 2026-10-01)

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
- **Verify the verdict.** Before calling a scenario "hard", confirm its correct answer is reachable through the tools (the removed harness's `test_verify_solver.py` did this). Three of seven early "failure modes" turned out to be caused by the environment (e.g. `book_meeting` couldn't book :30 starts).
- **Accept "refuse and ask" as a correct outcome** where a safe agent might decline; otherwise stronger models get marked wrong.
- **Findings on the removed services** (gpt-5.4 family): failures clustered in fan-out × constraint satisfaction (booking several meetings with mutual non-overlap), topological order (completing dependent tasks), and instruction prioritization (`lunch_window_refusal`: the agent names the user's rule, then breaks it). Pure fan-out and single-booking multi-constraint tasks passed.
- **Cross-model runs are cheap.** Earlier matrices cost a few dollars; run them early, not after building many scenarios.

**Off the roadmap:** agent convenience tools (`whoami`, `now`, aggregators), LLM-as-judge for task verdicts.

---

# procedural_core — Full Feature Reference

**Keep this file in context whenever building on `procedural_core`.** The crate offers far more than hash-derived attributes; anything using only `hash_int`/`hash_float` is using ~10% of the framework.

## Layer 0 — Primitives (`word`, `bits`, `hash`, `sampler`, `trajectory`, `edge`)

### `word::BitWord` (trait)
Implemented by `u64` and `u128`. Bounds include `Hash` (so word values can key `HashMap`s, as a mutation overlay does). Methods: `BITS`, `extract_bits(offset, width) -> u64`, `insert_bits(offset, width, value) -> Self`, `from_hash_u64(h)`. **`extract_bits` panics if `width > 64`** — the framework returns u64 from extraction, so any single field wider than 64 bits is unreachable. Split such fields into `_lo: 64` + `_hi: ≤63` and reconstruct with raw shifts (see `internot_mail::messages::compose_message_id` for an example).

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

## Layer 0.6 — Exact counts, fits and streams (Phase 0 and 2026-10-01)

Every function here is a pure function of its inputs, with golden tests. Float math goes through `dmath`.

**`fit`: fitting tables and distributions to targets.**
- `ipf(k, cols, r, c, IpfStop) -> IpfOutcome`: dense IPF in place (row-major `r.len() × cols`). It fuses a column scaling with the next row sums, eight rows side by side, and is bit-identical to separate passes. `IpfStop::DEFAULT` is 1e-10 relative or 60 passes.
- `GroupedIpf::new(row_keys, r, col_keys, c, kernel(&KR, &KC), stop)`: IPF when the kernel depends only on row and column classes. The fit runs over the classes, and each item takes its share of its class margin.
  - `entry(i, j)` gives one fitted entry.
  - `class_cumulative(a)` gives a class's cumulative weights over all columns, the input to `round_systematic_cumulative`.
  - `round_row(i, &columns_by_class(), class_weights_cumulative(a, ..), u, f)` rounds row `i` in two levels (column class, then column within it), without a `classes × columns` table. It has the same law as one-level rounding along the columns ordered by class. `f` receives counts by class, not in column order.
  - `Classes<K>` holds the grouping.
- `rake_columns(rows, cols, row_total, q, targets, passes) -> b` and `raked_weight(total, q_row, &b, h)`: IPF in factor form when each row keeps its total (names split over groups).
- `tilt_mean(&mut pmf, factor)`: exponential tilt to `factor ×` the mean (200 bisection steps on ln θ ∈ [−20, 20]).

**`partition`: integer splits.**
- Already there: `apportion_systematic`, `SystematicShares` and `contingency_systematic`.
- `round_unbiased(x, u)`: `⌊x + u⌋`, the floor or the ceiling, exact on average; with the same `u`, `x − k` rounds to `round(x) − k`.
- `round_systematic_cumulative(cum, scale, u, f(col, count))` rounds one real-valued row, given by its cumulative weights, to sparse integers.
  - Each entry gets the floor or ceiling of its expectation, unbiased; the row total is within one.
  - Cost `O(points · log cols)`.
- `SparseCounts` stores integer rows in CSR form: `from_rows`, `row(i)`, and `column_index(cols)` (a counting sort by column).
- Capped splits:
  - `sweep_capped(n, len, item(k) -> (w, cap), u, give)`: a keyed systematic split under caps;
  - `apportion_largest_remainder(n, w)` and `apportion_largest_remainder_capped(n, w, caps)`: deterministic, but biased on small splits;
  - `trim_largest_first(cells, cap)`.

- `pair_group(q, n)`: pairs with a trailing triple.
- `even_parts(n, min, max)`: the fewest even parts.
- `segment_offset` / `locate_in_segments`: index ↔ (segment, offset) over consecutive segments.

**`table`: drawing and searching.**
- `CumTable<T>::new(iter of (item, weight)) -> Option`, with `draw(key)` (top 32 bits) and `at(u32)`. The cumulative thresholds are `u32` fixed point, the last exactly `u32::MAX`.
- `Coarse<T>`: a sorted array with an index of every 16th value, giving `count_le`, `count_lt` and `find`.
- `CoarseRow { row, coarse }`: a borrowed row plus its stored index, with `first_fail(from, pred)` for monotone predicates and `count_le` for sorted rows. Build the index with `coarse_index` and `coarse_len`.
- `BucketIndex::new(starts)` / `segment(starts, x)`: which consecutive segment holds `x`, through a direct-address bucket table.

**`interp`: functions given by knots.**
- `bracket(n, x(i), at) -> Bracket { lo, hi, f }`, plus `lerp` and `log_lerp`.
- `piecewise_linear` and `piecewise_log_linear`: clamped outside the knots.
- `step_at_most` and `step_below`: piecewise-constant by inclusive or exclusive bounds.
- `interval_value`: the value of the first interval containing x.
- `first_above(lo, hi, f, v)`: the first integer where `f` exceeds `v`.

**`life`: life tables and hazards.**
- `survival(n, h)`: Π(1 − h).
- `survivorship(max, q, &mut l)`: the l column, with a closing 0.
- `first_event_pmf` and `cumulative_incidence`.
- `cure_hazard(ever, F(a), F(a + 1))`: the discrete hazard of a defective distribution.
- `Siler { infant, decay, background, old, slope }`, with `hazard(x)` and `year_death_prob(age)` in closed form.
- `stable_age_weight(growth, age)`.
- `add_conditional_deaths(l, from, scale, out)`.
- `invert_survival` / `invert_cumulative(row, req, u)`: an age conditioned on reaching `req`.

**`curve`: named schedule shapes.**
- `log_logistic_cdf`, `logistic_rise`, and `logistic_floor_quantile` (a logistic survival with a floor, inverted).
- `gaussian_bump`, `gaussian_kernel` and `ramp`.
- `rogers_castro_labour`.
- Kernel combinators: `symmetrized` and `dilated`.

**`pmf`: weight vectors.**
- `normalized` and `floored`.
- `mix_into`: a·(1−w) + b·w.
- `one_fewer_or_none`: thinning a count distribution.
- `spread_evenly`, `positive_mass` and `cumulative_normalized`.

**`sample` additions.**
- `exp1_by_inversion(u)`: −ln(1−u). The same law as `exp1_from_unit` but different bits, so a world must keep the one it was built with.
- `pick_linear(weights, u)`: a linear-scan categorical draw.
- `lazy_conditional(own, bound, required, draw, (k1, k2))`: an exact draw conditioned on a costly constraint, with a cheap first stage.

**Small helpers.**
- `perm::least_cost_assignment(k ≤ 3, cost)`: the first-minimum permutation.
- `stream::Epochs::keyed(start, span, key)`, with `index(t)` and `start(e)`: fixed epochs with a keyed phase.
- `bits::ones(mask)`: the set-bit positions.

**`stream`: regeneration.**
- `regen_state` is the single-level form (Phase 0).
- `nested_regen(levels, t, last_before(j, t), &mut [Option<Regen>])` handles nested levels, 0 the coarsest. A move at level j redraws levels ≥ j, so level k's regeneration is the latest move of any level ≤ k (at equal times the finer level counts as later).
- `nested_regen_state(levels, t, last_before, initial(k, parent), draw(k, regen, key, parent), &mut out)` gives each level's state: the draw at its last regeneration, under the parent level's state.
- Each costs one `last_before` per level, with no replay, and matches a full replay exactly (`stream::tests`).

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

**Introspection helpers:** `attribute_names() -> Vec<String>`, `relation_names()`, `context_names()`, `is_temporal(name) -> Option<bool>`, `attribute_type_id(name) -> Option<TypeId>` and `attribute_type_name(name) -> Option<&'static str>` (so external crates, such as a mutation overlay, can validate writes against the registered attribute type), `attribute_indexable_fields(name) -> Option<Vec<String>>` (own + deps for composites; empty Vec for non-indexable; `None` for unknown), `relation_arity_kind(name) -> Option<&'static str>` (`"fixed"` | `"dynamic"`).

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
6. **For mutation, keep the core read-only:** agent actions live in a session-scoped overlay, rebuilt when the first mutating service needs one (the old `procedural_overlay` crate is in git history). If the overlay allocates ids, the bit layout must reserve the top bit (`total_width() ≤ W::BITS - 1`) so session-allocated ids carry a sentinel.

## Stage Manager paradigm (for consumer crates)

Service crates built on `procedural_core` (e.g., `internot_mail`, `internot_calendar`) should adopt the **Stage Manager** split: every fact a renderer / UI / agent might consume must be derivable as a pure function of `(id, key)` from the procedural floor *before* prose generation. An LLM, when added, is a constrained renderer of structured AVMs (Attribute-Value Matrices), never the source of any fact. This is what makes cross-service coherence automatic: every service that asks `f(id, key)` gets the same answer without runtime coordination. Concrete shape: build a per-message/per-entity AVM struct populated entirely from procedural attributes + shared `internot_facts` functions; pass that AVM to the renderer; cache on `(entity_id, prompt_version, model_version)`.

**Empirically validated as of 2026-04-25.** The pattern that worked across mail and calendar:
1. The AVM carries a **typed specifier enum** (`TopicSpecifier { AboutMeeting | AboutFile | AboutPhrase }`, `EventSpecifier { Standard | MeetingFromThread | AdHocAbout }`). The LLM dispatches on the kind and uses the companion fields. No stringly-typed "subject" field — the model has nowhere to insert an invented one.
2. The AVM serializes via a **descriptive-key flat JSON wrapper** (`MessageAvmJson`, `EventAvmJson`) — long, semantic key names (`sender_voice_formality_baseline`, `event_specifier_kind`) so the model can disambiguate fields with no fine-tuning.
3. The OpenAI call uses **`response_format: json_schema, strict: true`** (rejection-sampled server-side); the system prompt instructs on per-kind dispatch and explicitly forbids invention.
4. **Cross-service knowledge gaps are exposed as structural truth, not hidden** — calendar's `MeetingFromThread { thread_id }` lets the renderer say "the calendar can't view the original thread" when appropriate. The agent navigates services because the AVMs *can't* answer cross-service questions on their own.
5. **Shared renderer infrastructure** (an OpenAI-compatible client with an on-disk cache keyed by `(namespace, prompt_version, model_version, hashed id)`) is rebuilt when the first renderer needs it; the old `internot_renderer` is in git history. Each service keeps its own typed renderer trait + AVM JSON wrapper + system prompt. No generic `Renderer<I, O>` trait — mail's `MessageAvm` and calendar's `EventAvm` have different structural relationships with time and context; forcing them through one trait pushes toward `Box<dyn Any>` context plumbing. See `internot_calendar/examples/cross_coherence.rs` for the working proof.

When designing a new service crate's AVM + renderer, copy mail's pattern: typed specifier enum, descriptive-key serde wrapper, structured-output OpenAI call, side-by-side template + LLM demo. The architecture has earned the prescription.

## Known sharp edges

- The search builder `panic`s on misuse (unknown field, duplicate-field predicate, anchor/filter mismatch); attribute / context / relation registration returns `Result`. Different error contracts in similar API positions — do not assume `find()` is `Result`-safe.
- **`BitLayout::extract` and `BitLayout::compose` go through `u64`.** Declaring a single field wider than 64 bits succeeds at construction time but panics at `extract`. If you need a wider logical field (e.g., embedding a 96-bit thread_id inside a 127-bit message_id), split it into `_lo` (≤64) and `_hi` (≤63) and pack/unpack via raw `u128` shifts. `BitLayout::compose` also accepts only `u64` per field for the same reason.
- `Metric::Hyperbolic` is registered cleanly but panics inside `candidates(...).take()` (v0.1). The other validation gaps (dim type, `Normalization`) are now caught at `Space::context()` registration.
- The `EvalFn` dispatch ladder is open-coded in 5 places (`Space::entity`, `Space::attribute_value`, `World::entity`, `World::attribute_value`, `candidates::eval_attribute`). Adding a new variant means editing all five — easy to miss one.
