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

## Where files go (founder, 2026-10-01)

**Nothing outside this repository.** No files in `~`, `/tmp` or caches elsewhere, and no global installs: no `cargo install`, `pip install` or shell-profile edits. Use these git-ignored folders instead:
- `.scratch/` for scratch work, downloads and temporary binaries;
- `.tools/` for tools. It holds the Lean 4 toolchain (`.tools/elan`), a Lean project with mathlib (`.tools/lean`) and mathlib's download cache. Run Lean with:

  ```sh
  export ELAN_HOME=$PWD/.tools/elan PATH=$PWD/.tools/elan/bin:$PATH
  cd .tools/lean && lake env lean <file.lean>
  ```

Raw datasets stay in `datasets/`, which is also git-ignored.

## Crates

**Substrate (framework, never edited per-service):**
- `procedural_core`: deterministic math for procedural worlds: keys and hashes, samplers and laws, exact counts and splits (`partition`, `lattice` floor sums), bijections (`perm`, including `AffinePerm`), life tables (`life`, including `HazardTable`), fits and event streams. The `Space`/`World`/search framework and the old worlds' primitives were removed on 2026-10-03 (archived in `.scratch/archive/2026-10-03-old-core/`, and in git history).
- *(Removed 2026-10-01, founder: "outright remove them and rebuild something exact that we need when we need": `procedural_overlay` (a session overlay for mutation), `internot_renderer` (an LLM render client and cache), the `mcp_harness/` Python scenario harness, and `internot`'s empty `Services` bag. All are in git history; rebuild what a service needs when it needs it.)*

**World packs (spec `specs/2026-10-01-world-packs.md`):**
- `internot_def`: world packs, i.e. RON files in `worlds/<pack>/`. It provides:
  - loading from directories or from packs embedded in the binary;
  - `extends`, merging field by field and lists by `id`;
  - errors that name the file and the line or field;
  - a fingerprint;
  - the value vocabulary (`Series`, `VecSeries`, `Steps`, `Bands`, `Ranges`, `BySex`), evaluated by `procedural_core::interp` (its one dependency inside the workspace).
- **Every statistic lives in a pack** (`worlds/us/`), never in code. `worlds/README.md` is the guide.

**The society world (spec `specs/2026-09-29-society-as-a-function.md`):**
- `internot_society`: the world pack's typed sections (`params.rs`), the pack's name data (`names.rs`), and the monotone world (`mono.rs`). The monotone world holds people, first unions, separation, births, parents and deaths as pure functions of `(seed, id, t)`, built so that the counts a search asks for are closed form at any population. It depends on `procedural_core`, `internot_def` and `rayon`. Status, guarantees and measurements are under "The society world" below.
- `internot_perf`: the benchmark and profiling harness (the `perf-gate` binary), with suites `core_hash`, `primitives` and `society`. Budgets are in `perf/budgets.toml`, per-machine baselines in `perf/baselines/`; flamegraphs are generated on demand. `perf/check.sh` runs all workspace tests and then the gate; `perf/society_ab.py` A/B-tests two builds of the society world.

**The world (one crate, services as folders):**
- `internot/` — every domain service lives here as a sibling folder under `src/`. `Universe` carries the society world (`society`), the `Mutex<SessionState>` and the injectable `now`. `registry()` concatenates each service's `views()` into the canonical list.
  - **Service plug-and-play.** `src/services.rs` defines the `Service` trait (methods `name` and `views`). `lib.rs::SERVICES: &[&dyn Service]` is the **single source of truth** — adding a service is one line here. `crate::registry()` walks this list; it never names services individually. Each service's `mod.rs` defines its tag struct (`DirectoryService`, …) and impls `Service`.
  - `src/directory/`: people of the society world through MCP (`read_person`): name, sex, heritage, life dates, age, partner, union, parents, children and siblings, home and household, education and work at any time. Spec `specs/2026-10-01-directory.md` (updated 2026-10-04).
  - `src/society.rs`: the society world (`internot_society::mono::Mono`), built once per process (milliseconds) from the embedded pack `INTERNOT_PACK` (default `us`), seed `INTERNOT_SEED` (default 42) and scale `INTERNOT_SCALE` (default 1). `Universe::society` holds it, and tests use `Universe::for_pack("us-tiny", now)`.
  - *(Removed 2026-10-01: `src/people/`, the old slot-layout people, with its leaked name data. Its individual attributes (personality, education, languages, hobbies, working hours) are in git history, to re-key to the new ids when needed.)*
  - *(Removed; do not restore: `mail/`, `calendar/`, `files/`, `tasks/`, `money/`, `chat/`.)*
  - `src/universe.rs` — Universe + SessionState wiring. SessionState carries one field per mutating service (still explicit; session types differ structurally).
  - `src/views.rs` — `View<P, O>` trait + `DynView` + `ViewRegistry`. View signature: `execute(&self, ctx: &Universe, params) -> Result<O, ViewError>`.
  - `src/services.rs` — `Service` trait.
  - `src/trace.rs` — cross-cutting `_get_trace` view (verdict-only).

**Transport (thin generic router; no domain knowledge; never edited per-service):**
- `internot_mcp/` — stdio MCP server (rmcp 1.5). Builds `Universe::new()` (and so the society world) once at startup, walks `internot::registry()` and binds every view to an MCP tool. Adding a new service appears as new tools on next build with no transport-side edit.

## Load-bearing invariants (do not violate)

1. **Single procedural floor.** Every service derives from the one society world held by `Universe` (`internot_society::mono`, built once per process). A new service is a lens on this floor, never an independently seeded world. `internot_sql` was deleted because it built its own world.
2. **Single viewer per scenario.** Agents act as ONE user (the launched viewer). Views must not contain affordances that switch the active viewer. Read views can surface information about *other* people; mutations are always on behalf of the launched viewer.
3. **`now` is injectable, never a constant.** A frozen calendar overfits training data to one month and dates the simulation. Tests pin a specific `Utc.with_ymd_and_hms(...)`. Procedural generation can derive `now` from the `World` seed for variety while remaining 100% reproducible.
4. **Person references are the society world's external ids** (`u64`, `Mono::id`/`Mono::pid`: `cell·2^(9+b) + (birth year − first year)·2^b + index`, `b = 53 − 9 − cell bits`, below 2⁵³ so JSON carries them exactly). The earlier 32-bit convention (from mail's `mail_id: u32`) capped populations at 4.3B; the society world runs to trillions. Don't truncate ids to save padding.
5. **Cross-entity references via reconstruction, not materialization.** When entity A points at B, B is found by a pure function of A's facts (a mother from a child's birth-order position, a husband from a wife's interleave position), never stored. References are free queries.
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
let id = u.society.world.id(internot_society::mono::Pid { cell: 0, y: 1960, i: 1_000 });
let rec = View::execute(&ReadPerson, &u, ReadPersonParams { person_id: id, at: None })?;
```

*Untyped (transport-style).* Best for any code that walks the registry generically:
```rust
let reg = internot::registry();
let view = reg.get("read_person").expect("registered");
let out: serde_json::Value = view.execute(&u, serde_json::json!({"person_id": id, "at": "1990-01-01"}))?;
```
`view.input_schema()` returns the JSON Schema for the params; `view.read_only()` flags mutating views. This is exactly what `internot_mcp` does.

**3. Mutate through the session mutex.** No service mutates yet. A mutating view locks `ctx.sessions`, changes its service's session state (a field of `SessionState`) and appends to `MutationTrace`; the mutation layer is rebuilt when the first mutating service needs it.

**4. Read the verdict.**
`_get_trace` is the canonical scoring surface — every mutating session method appends to one of `MutationTrace`'s typed lists. Scenarios call `view.execute(&u, json!({}))` on the trace view and pattern-match the JSON. For in-process tests just inspect `u.sessions.lock().trace` directly.

**5. Test the chain in Rust before paying an API.**
For any new chained-tool flow, write a Rust integration test in `internot/tests/` first (`internot/tests/directory.rs` is the model). Agent runs over MCP come after.

## Adding a new service (the canonical recipe)

To add a new service `foo`:

1. **Create `internot/src/foo/`** with `mod.rs`, `views.rs` (read views over `ctx.society.world`), `avm.rs` (an AVM struct, when a renderer needs one) and `session.rs` (a mutating overlay, if any), and a `pub fn views() -> Vec<Arc<dyn DynView>>` exporter.
2. **In `foo/mod.rs`**, define a tag struct and implement `Service`:
   ```rust
   pub struct FooService;
   impl Service for FooService {
       fn name(&self) -> &'static str { "foo" }
       fn views(&self) -> Vec<Arc<dyn DynView>> { views::views() }
   }
   ```
3. **Add `&FooService` to `internot::SERVICES`** in `lib.rs`. That's the only edit to crate-level files.
4. **If `foo` mutates state**, add a field to `SessionState` in `universe.rs` and initialize it in `SessionState::new`.
5. **If `foo` needs an LLM renderer or other process-wide infrastructure**, build exactly that when it's needed (the old renderer crate and `Services` bag were removed unused).

Adding the service exposes its views as MCP tools automatically (`internot_mcp` rebuilds and they appear).

## Current phase: rebuild for the coherence pilot (2026-09-29 →)

**What exists.** The `directory` service on the society world (the monotone world, since 2026-10-03) plus `_get_trace`: 2 MCP tools (`read_person`, `_get_trace`). `read_person` serves names, heritage, kin, home and household, education and work (2026-10-04). The transport still contains zero domain knowledge.

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

**Phase 1 now runs on the monotone world (2026-10-03).** The founder replaced every earlier world with it: "i want to remove everything related to the cell and old world and completerly replace it with this new promising world".

### The society world: the monotone world (`internot_society::mono`)

**Why it exists.** The founder asked for searches at billions to trillions of people ("I want to do this on the scale of billions or trillions… I need it to be faster"). At 10¹² people no per-person scan survives, so every count a search asks for must be closed form, with cost proportional to the answer, not the population. Worked out with a second agent, Tursi, in `thinking/claude/` and `thinking/tursi/`; Tursi's own worktree is under `.tursi/`.

**The rules it is built on** (`thinking/claude/004`, `005`, `010`):
- **Couplings preserve order.** Every map between index spaces is monotone, so a prefix in one space is a prefix in the next. Order-scrambling is allowed only inside a class.
- **At most one decorrelating permutation per class, and it is affine** (`perm::AffinePerm`, `r ↦ (a r + b) mod n`, `a` the golden multiplier). Joint counts are then 2-D lattice counts, i.e. floor sums. Two maps on one modulus compose to one affine map.
- **Deaths are quantile slots of classes.** A class is a rank interval of one cohort and sex sharing one law; "dead by t" is one count, and "alive" is a suffix.
- **Deaths come from the cohort's life table from 16 (option A, `thinking/claude/010`, Tursi agreed in `thinking/tursi/003`).** A union whose partner is dead at its start is void for both. Wives are compensated only for the survivor's lost union (the census convention: `ever_partnered` is a share of survivors to 50).
- **Births choose mothers among the living** (survival first), so no death depends on a birth.

**What it models** (one group, one area, natives):
- **Cohorts:** sizes from the pack's births, women first; per sex, the young (die before 16), then adults.
- **First unions:** an integer table (wife cohort × husband cohort) filled in year order by the pack's age-gap kernel under each husband cohort's adult men, laid out by `lattice::ExactInterleave` over the wives in union-age order.
- **Separation:** a category in the wife class's space (a second affine map, multiplier `a²`), by the pack's dissolution classes. A union ends at the first of separation, his death and hers (competing risks).
- **Births:** blocks per (year, married or single, mother age), a capped split under each block's eligible mothers (at most one child per mother a year, Lean `one_child_per_mother`), then children choose mothers by the proportional owner. A birth's phase in the year is the mother's death slot rotated per class and turned back 0.0937 of a year per calendar year (`year_turn`, 2026-10-03: before it, every child of a mother shared her birthday), so siblings are at least 330 days apart and "born by t" is at most two slot runs per class.
- **Fathers:** the mother's husband if she was married, the union isn't void or separated by the conception, and he is alive at it.

**API** (`Mono::new(params, seed, scale)`; build ~13–20 ms; ~10 MB at ×1, 16.5 MB at ×1000, 18.3 MB at ×10⁵, growing with the log of the population (deeper death-table tails, more late union-age classes); `memory_report()` lists the components, `MEMORY=1 examples/mono_report` prints them):
- `birth`, `death`, `alive_at`, `sex`;
- `mother`, `father`, `children` (anyone), `siblings`;
- `spouse` (with the void check), `partner_seat`, `separation`, `union_end`;
- `count_alive(t)` (exact) and `alive_bounds(t)` (instant);
- `alive_in_cohort` / `for_each_alive` (enumeration);
- `Mono::id` / `Mono::pid` (external ids, below 2⁵³ so JSON numbers carry them; see "Cells" below).

**Guarantees** (`internot_society/tests/mono.rs`, exhaustive on small worlds; `MONO_SEEDS`, `MONO_MULT`):
- mother ↔ children and father ↔ children agree both ways;
- spouses and union ends are symmetric;
- every mother is alive and aged 15–46 at each birth, and every father is alive at conception;
- every child is counted once;
- `count_alive` equals brute force at four dates, within `alive_bounds`.

Passing on 12 seeds at ×0.01 and 2 at ×0.1.

**Measured** (`us` pack; `examples/mono_report.rs`; perf suite `society`):

| | ×1 (14.2M ever) | ×100000 (1.42 trillion) | ×1000000 (14.19 trillion) |
|---|---|---|---|
| build / memory | 13 ms / 10.3 MB | 20 ms / 18.3 MB | not re-measured |
| exact alive count | 0.07–0.1 ms (0.4–0.7 before the speed pass) | 1.0–2.7 s (2.6–7.1 before) | 27–77 s before the speed pass |
| instant bounds (±0.01–0.06%) | 0.04–0.13 ms | 0.13–0.18 ms | 0.5–0.75 ms before |
| list everyone alive, 1 thread | 8 ms (1.1 ns/person) | | |

At ×10⁹ (1.4·10¹⁶ ever) the bounds take 5–12 ms. The original ledger world needed 3.1 s to scan the US-scale count (33.4 s for the cell world), 1.5 GB and a 7 s build.

| Lookup (p50/p99, perf gate, ×1, after the speed pass below) | Time |
|---|---|
| birth | 90/121 ns |
| death | 80/210 ns |
| mother | 90/211 ns |
| spouse | 190/291 ns |
| father | 261/431 ns |
| children | 581 ns/1.18 µs |
| siblings | 611/912 ns |

The gate's single-call timer adds ~20 ns to every p50 (an empty call measures 20 ns). Lookups are the same at ×10⁶.

**What still grows with population:** the exact count checks one by one the young whose age at death falls within a year of `t` (≈0.025% of the alive). The closed-form part and the bounds are flat.

**Primitives added to core for it:**
- `perm::{AffinePerm, golden_pair}` (incl. `from_pair`, `inv_range`, `count`, `select`);
- `life::HazardTable` (tabulated cumulative hazard with an exact inverse, no Newton; now used only to build the quantile tables);
- `quantile::{OctaveTable, OctaveShape, slot_w, count_from_threshold}` (speed pass, below);
- `perm::{CompactPerm4, CompactPermR, rem_by_inverse, divmod_by_inverse}` and `SystematicShares::{offset, part_of_offset, end_of_offset}` (speed pass).

**Proofs:**
- `docs/superpowers/research/proofs/2026-10-03-mono-world.lean`: monotone slots, proportional-owner intervals, rotated phases, one child per mother;
- the affine count identity, from the pure-world proofs.

**How to add a feature** (founder, 2026-10-03: "slowly adding in features without sacrificing any time. back to lean"):
1. math, and a Lean lemma where it isn't definitional;
2. code;
3. the exhaustive tests;
4. the speed gate: `perf/society_ab.py BASE NEW ROUNDS`, which runs two saved `mono_report` binaries alternately (×1, and ×1000 bounds-only) and compares medians and answer checksums. Noise is p50 ±1–2%, p99 and sub-ms counts up to ±20–30%. A feature must not move existing p50s.

A speed change that must not change the world is checked with `examples/mono_checksum.rs` (every lookup's full answer on 200k people, plus alive counts, bounds and separated-and-alive; `PACK`, `MULT`): the `ALL` line must not move. `examples/mono_parts.rs` gives each internal step's mean cost (`HOT=1`: from cache), and `perf-gate --suite society` the single-call latencies with hardware counters.

**Log** (`thinking/claude/001`–`011`; `thinking/tursi/001`–`003`):
- **Tier 1, a negative result:** cheap death bounds settle 95.6% of checks but even "cheap" facts cost ~1 µs in the cell world. Per-person schemes can't scale.
- **Prototype:** exact counts in ms, flat to trillions.
- **Separation, verified:** "separated and alive" in closed form equals brute force in every class.
- **Option A:** cut men's death classes from ~3000 per cohort to one; with `HazardTable` and stored multipliers, counts went 6 → 0.4 ms and enumeration 13 → 1.1 ns/person.
- **Step 1:** children and siblings for everyone.
  - Bug found by the exhaustive test and fixed: single mothers assumed alive.
- **Step 2:** close kin.
  - Defect found and fixed: a mother could get up to 110 children in a year (blocks ignored eligible mothers). Now capped, at no time cost.
- **Women ever partnered by 50:** 92–93% (pack 92%).
- **Void seats:** 2–4% (modern), ~10% (1880), including people who die unmarried before a wedding.

**Speed pass (2026-10-03, founder goal "make it 2x as fast as now").** Perf suite `society`, p50 (the gate's timer adds ~20 ns to each lookup):

| Benchmark | Goal start | After | Speedup |
|---|---|---|---|
| death | 170 ns | 70–80 ns | 2.1–2.4× |
| father | 912 ns | 261 ns | 3.5× |
| spouse | 441 ns | 190 ns | 2.3× |
| children | 1.20 µs | 561–581 ns | 2.1× |
| siblings | 1.25 µs | 611–631 ns | 2.0× |
| count_alive | 350 µs | 68 µs | 5.1× |
| alive_bounds | 227 µs | 42 µs | 5.4× |
| world_build | 56.5 ms | 13 ms | 4.3× |
| birth | 140 ns | 90 ns | 1.56× |
| mother | 140 ns | 90 ns | 1.56× |

In bulk (mean per lookup over 1M random people, `examples/mono_parts.rs`): death 113 → 40 ns, birth 94 → 50, mother 87 → 50, spouse 289 → 125, father 464 → 217. Interleaved against the saved goal-start binary (`.scratch/mono/bin/mono_goal_base`, same timing loop): death 160 → 70 ns, mother 130 → 81, father 890 → 251, spouse 421 → 170, children 1.12 → 0.54 µs, siblings 1.21 → 0.59 µs, count 0.36 → 0.08 ms, build 60 → 10 ms. Memory: world 10.3 MB at ×1 (`memory_report`; death tables 3.9, alive prefixes 2.9, interleaves 1.1, class records 1.0, eligible rows 0.5), +13 MB resident after the build; about 7 MB before the speed pass (cache-line records and rows cost ~3 MB).
- **World changes** (statistics unchanged: the realism table, e0, ever married, CFR, the age gap and void seats move within noise; brute-force alive counts equal; exhaustive tests pass on 18 seeds at ×0.0005–×0.3):
  - **death ages from quantile tables** (`quantile::OctaveTable`): each (cohort, sex) has an adults' table (age given alive at the adult requirement) and a young table (age given death before 16), knots on octaves of `w = 1 − u` (256 cells in the body halving to 16 per octave in the tail), read from the bits of `w` and interpolated: no `ln`, no search. Exactly monotone, so every count stays exact: `count_alive` inverts the shared table once per (cohort, sex, date) (`threshold_near`) and counts each class with `count_from_threshold`;
  - **union ages from one shared two-tailed table** of the log-logistic shape `(q/(1−q))^(1/shape)` (only the median varies by cohort): no `pow`;
  - **the bridge has 4 Feistel rounds, not 6** (`perm::CompactPerm4`). Four is Luby–Rackoff's minimum for a strong pseudorandom permutation; three leave a serial correlation of −0.02 between consecutive inputs' images (20 standard errors on 1M pairs; `perm` tests), so three were rejected.
- **Answer-identical** (checked by `examples/mono_checksum.rs`, full answers on 200k people plus counts: `us` `cc70eae70beda454`, `us-tiny` and ×1000 in `.scratch/mono/`):
  - the availability passes computed the age-gap kernel 4.2M times though it ignores age: weights once, the same float sums (build 56 → 11 ms);
  - one death threshold per (cohort, sex, date) shared by all adult classes; a tight per-class loop; the band check in chunks of 64; births by the date counted per block in parallel;
  - a 64-byte record per death class (`ClassRec`: range, permutation, phase rotation, reciprocal); wife-class guides in the cohort's first cache lines; the eligibility row as one cache line with a 32-bucket guide (the early-loaded class record is right 90% of the time); the bridge parameters in a dense array; the year's block row loaded while the bridge computes; divisions by stored or early reciprocals (`perm::divmod_by_inverse`); Feistel rounds fold the key's xorshift (`round_small`, bit-identical below 2³⁰);
  - children: a mother's alive check compares her death time with the year's end, and her index needs one prefix entry.
- **Why birth and mother stop at ~1.56×.** With every line cached they still take ~215 cycles: the 4-round bridge is ~84 cycles of serial arithmetic (a 64-bit mixer per round), then a block search, a division, the class search and the phase. Cold, two dependent cache lines (the eligibility row, then the alive prefix with the class record) add ~50. Reaching 70 ns would need ~195 cycles. Tried and dropped: 3 rounds (correlated, above); an affine bridge (its rotation number against each class's golden map is arbitrary, so ~1–2% of classes would show order correlations between mother and child); prefetching the age group's rows (16 outstanding loads slowed the critical ones); inlining the Feistel (code bloat slowed children); a branchy block guide (mispredicts). A per-block table holding each bucket's class constants would cut a cache level, for ~16–33 MB more state: a founder decision (below).
- **Lessons:** measure the same lookups with all data cached (16 repeated inputs) before optimizing memory: birth was compute-bound. Prefetch only lines that will be used; a few extra outstanding loads cost more than they save. The gate's p50 moves in 10 ns steps.

**Cells: heritage groups and the open market (2026-10-03, Phase 1 of `specs/2026-10-03-global-world.md`; math in `research/2026-10-03-monotone-cells-math.md`).** `Mono::new` builds one cell per heritage group of the pack (the `us` pack: White, Black, American Indian and Alaska Native, Asian, Hispanic); `Mono::blind` builds the one-cell world. People are `Pid { cell, y, i }`.
- **Births by group, in 15-year waves.** A year's births split over groups by their *realized* eligible mothers (women alive through the year, all in cohorts of earlier waves) times the age shape and the group's fertility factor, capped by them, so a small group never gets more births than it has living mothers. Children are in their mother's cell.
- **Mortality by group: one table per country cohort, scaled per group.** Every cell shares the country cohort's death tables; a group's adults scale the table's ages about the adult requirement (accelerated failure time) so that the remaining life there is the group law's (`life::HazardTable::remaining_life`). The young's count is the group law's; their ages share the table. The scaling is folded into the seconds conversion (`aft_secs`: `[0, YEAR]` is the table itself, bit for bit).
- **Unions:** each group's own space (in-group wives × in-group men, as before) plus the open market. A cohort's open wives are an exact Beatty set of its wives; the open space's categories are (husband cohort, group), weighted by the kernel and the groups' open pools; its wives of a cohort are an exact interleave over groups (`open_mix`). A wife cohort with too few in-group men in reach sends the rest to the open market (small groups marry out more); open wives who find no man in reach (only in tiny worlds) are void seats (`open_seated`).
- **Wife death classes** are union-age years up to the last childbearing age, then one class for every later union (union times still come from the full union-age prefix). Separation of the merged class uses its first union year's dissolution shares (debt: about 3% of first unions are after 45).
- **Layout:** every per-cohort and per-year array is flat over cells (`cell · years + year`); interleave nodes are 16 bytes (`lattice::InterleaveTable` carries lengths down the walk) and locate with one division per level (`RationalBeatty::count_member`); alive prefixes are u32 below 2³² (`Prefixes`); the union tables are dropped after the build.
- **Checks:** `Mono::blind` reproduced the single-cell world's answers bit for bit through every step except deliberate changes (the merged wife classes; then birthdays turned by year, below): `us` blind `af8c5acc63c18ea3`, heritage `2a2f4c305eb8b3bf` (`examples/mono_checksum.rs`, `BLIND=1`; recheck after rebuilding the *examples*: a stale example binary once gave a wrong reference). Exhaustive tests run both worlds and pass on 18 seeds at ×0.0003–×0.01 and on 2–4 seeds at ×0.03–×0.3, adding: ids round-trip, mothers in the child's cell.
- **Measured** (×1, `perf/society_ab.py` against the speed pass's binary, 5 interleaved rounds):
  - blind world: memory 10.86 → 7.75 MB (`memory_report`; resident +22 → +10 MB); lookups equal or faster (mother −1%, spouse 0, father 0, children −16%, siblings −15%, p99s −7 to −20%);
  - heritage world (5 groups + open market): memory 20.2 MB (resident +25 MB vs +22 MB for the old single-cell world); p50 +5–14% (one 10 ns tick: death 81, mother 100, spouse 200, father 300 ns in the gate), p99 +20–43% (its hot set, ~20 MB, exceeds the 16 MB L3); `count_alive` 0.15 ms (2×), build 43 ms (3×). The perf gate's `society` baseline was re-recorded on the heritage world.
- **Build memory:** eligibility rows are written by the waves straight into per-cell flat arrays (inline arrays for the hand-off from worker threads): per-row heap vectors allocated on workers and freed on the main thread had cost +5 MB resident (glibc arena fragmentation), inline arrays held all at once +11 MB. Resident after the build: heritage +22 MB (the old single-cell world's), blind +9 MB.
- **Realism (`examples/mono_heritage.rs`):** without migration the composition drifts: alive in 2020 66% White, 28% Black, 1.2% AIAN, 0% Asian, 5% Hispanic (census 2020 ~58/12/1/6/19); e0 by cohort falls 1.5–2.5 years against the blind world (the pack's group mortality factors were calibrated on a population with immigrants). Newlyweds across groups 2000–19: White 6.5%, Black 6.4%, AIAN 45%, Hispanic 31% (Pew 2015: ~11%, 18%, 58%, 27%). Realistic composition needs migration (Phase 4: life cells and movers).

**Names (2026-10-03, Phase 2; `mono/naming.rs`, ported from the ledger world's naming; pack `names.ron`, data `names.bin`):** `first_name`, `middle_name`, `birth_surname`, `surname(x, t)`, `full_name(x, t)`, `marriage_of`/`marriage_date`/`married_at`; kin in one walk: `union_of(x)` (partners, start, end, how) and `parents(x)` (mother, birth, father's union).
- **First names:** SSA by birth year and sex, split over groups by Census 2020's `P(group | name)` raked to the world's births by group that year (`fit::rake_columns_dense`, stops at 1e-9). Drawn by exact rejection from the year's table (shared by all groups; SSA counts stored cumulative in `NameData`): a name drawn by count is kept with probability its group share over the group's largest. Only a year's rake factors and envelopes are stored (built on first use: ~7 ms per year and sex).
- **Surnames:** founders from their group's Census column (rare tail spread over rare names); children the father's, the mother's or both by whether the parents were married at the birth; Spanish double surnames; wedding changes by era, age and group; reverts after separations. A surname walks the father's line to a founder.
- **Birthdays (fixed with names):** every child of a mother shared her birthday (the phase was her death slot). Phases now turn back 0.0937 of a year per calendar year (`year_turn`): siblings differ, consecutive-year siblings stay at least 330 days apart (tested), counts stay closed form.
- **Checks:** `tests/names.rs` (both worlds: every surname from a parent's line or a founder's table; weddings take the partner's surname or hyphenate; partners agree on the wedding; middle ≠ first; >90% of 1900–39 brides take the husband's name). `examples/mono_names.rs`: top SSA names match within sampling noise (1990 girls: Jessica 2.45% vs 2.57%, Ashley 2.40/2.29; 1950 boys: James 4.82/4.94); top surnames by group follow Census (Smith, Williams, Rodriguez…); women keeping their surname 1.7% (1900s cohorts) to 30% (1990s).
- **Cost:** `full_name` 5 µs p50, 23 µs p99 warm (perf gate `society/full_name`, budget 100 µs); the first name of an unseen year adds its ~7 ms rake once.
- **Directory:** `read_person` serves `name`, `birth_surname` (when changed) and `heritage`; every person reference carries its name (`internot/examples/read_person.rs` prints one).
- **Debts:** no immigrants yet (the pack's foreign names wait for migration); Asian names unused (no Asians without migration); same-sex unions and their marriage rules wait for same-sex unions; in small cells (Hispanic before 1900 at ×1) cousins marry often enough that double surnames sometimes repeat.

**Households (2026-10-03, Phase 3 step 1; `mono/household.rs`, ported from the ledger world's L3; pack `households.ron`):** `household(x, t)` (a couple's home, or a single adult's spell), `members(h, t)`, `dependent_of`, `chain_end`, `kin_host`, `leave_time`. Rules: dependents live with the custodial parent (the father after a separation with the pack's custody share) or a guardian (grandparent, then eldest adult sibling); units seeking kin live with an anchor; couples in their home; singles alone.
- **Exact** (`tests/households.rs`, both worlds, 7 seeds × 4 dates at ×0.003–×0.01): `household` and `members` agree both ways; chains end at independent adults.
- **Cost:** household + members p50 24 µs, p99 204 µs (×0.05).
- **Realism (`examples/mono_households.rs`) is off where unions are:** 2020 mean household 2.32 people (Census 2.5), 47% of households alone (28%), 44% of 65+ alone (~28%); 1900 4.14 people (4.6), 22% alone (5%). Causes: no re-partnering (widowed and separated people stay single for good: open item 3 below is now needed), no roommates (debt until areas). 18–24 at home 64% (ACS ~55%).

**Residence (2026-10-03, Phase 3 step 2; `mono/residence.rs`, design A forward, ported; packs `residence.ron`, `places.ron`, `data/places.bin`):** `home(x, t)` (place-tree position, dwelling, mail address), `address_of`, `unit_pos`, `unit_dwelling`, `birth_place`, `places()`.
- **Places:** areas (62) ⊃ commuting zones (588) ⊃ counties or 1M parts (3,215) ⊃ clusters (8,450) ⊃ tracts (83,848). Weights by decade down to counties; clusters and tracts by static shares (the data scales 2020 tract shares by county populations), so the tree is 7.5 MB with names, ZIPs and streets, built on first use in 25 ms.
- **`places.bin` v3** (`data/distill_places.py`): adds each tract's ZIPs by land area (Census 2020 ZCTA–tract relationship), each ZIP's postal place (GeoNames postal codes, CC BY 4.0: credit www.geonames.org) and 20,000 street names by TIGER 2025 frequency (named streets with a type). The tree part is byte-identical to v2.
- **Units** are first unions (named by the wife) and single spells (0 before the union, 1 after); households live at their anchor unit's position. A unit starts near its source (the household left, one partner's household, or the union's home kept or left after a separation); positions over time by nested regeneration: local moves at four levels by gravity around the unit's anchor, long moves (to another area) by the driver's Poisson stream landing in the lineage region of the area left with the pack's share. Founders' units are seeded by weight in 1840.
- **Addresses:** a dwelling is the unit that moved in and when (a kept home keeps its source's dwelling); its house number and street are keyed by the dwelling, its ZIP drawn from the tract's by land area. "78 Robin Ln, Belmont, MA 02478" → Cambridge → Leominster → "2126 Cross St, Lunenburg, MA 01462".
- **Exact** (`tests/residence.rs`, both worlds, 6 seeds × 4 dates): everyone alive has a home; positions are tree paths; household members share the address; addresses are stable across calls and within a stay.
- **Realism (`examples/mono_residence.rs`, ×0.05):** movers in a year 20.5% (1900), 19.1% (1950), 8.6% (2023) (targets 20%, 20%, 7.8%); living outside the birth state 43–56% (21–34%); adults 25+ with a parent within 30 miles 33–44% (59.8%); born 1990–94, within 100 miles at 26 of where they lived at 16: 68% (80%). The misses are national pairing: partners come from anywhere, so one moves at every union (the design-A finding). Fix: areas as cells of the kinship world with migration (the global spec's P10–P11).
- **Cost:** `address_of` p50 ~100 µs, p99 ~190 µs (the source chain's positions; memoized per unit up to 32k units). Memory when used: places 7.5 MB, gravity kernels 0.7, move-rate tables 1.5, memos ≤ 3.2.
- **Directory:** `read_person` serves `home` (address, city, state, ZIP, county, since) and `household` (the others living there).
- **Debts:** no rosters (who lives in a place) until areas; street names national (not by county); no same-tract moves; founders who are minors in 1840 live alone; roommates.

**Education (2026-10-03, Phase 5; `mono/education.rs`, `mono/schools.rs`; pack `education.ron`, data `data/schools.bin` from `data/distill_schools.py`):** `education_level` (final), `education_at(x, t)` (completed by then), `schooling_at(x, t)` (kindergarten, grade, college or graduate year), `education_path` (dates), `school_at`, `college_of`, `graduate_school_of`, `education_history(x, until)` (stints at institutions), `institution(i)`.
- **Levels:** less than high school, high school, some college, associate, bachelor's, master's, professional, doctorate. Final attainment by cohort and sex (1940/1947 census, CPS, ACS 2023), each group's thresholds shifted by its log-odds by cohort (proportional odds), associate and graduate splits by cohort.
- **Family and partners without recursion:** `z = √own·ε_x + √parents·ε(mother's union) + √grandparents·(ε(mother's mother's union) + ε(father's mother's union)) + √union·ε(own union)`, a woman's union key her own (so a mother's children share it with her and her partner). With weights 0.10/0.35/0.05/0.45: mother–child latent 0.54 (target 0.55), partners 0.43 (0.65–0.73), siblings 0.45 (0.6). An age-only pairing can't reach all three with O(1) keys; the balance is a pack choice. (Assortative pairing in the kinship layer would need education-sorted couplings: noted as a design direction, not built.)
- **Timeline:** grade 1 from a September cutoff (later before 1930), kindergarten by era, leaving without a diploma at the era's age ± spread, college on time or late (exponential delay), programs' lengths, graduate school after a gap. Calibrated: enrollment 1910 at 13/14/15/16 = 100/88/61/32% (89/81/68/51), 2020 at 17 = 93% (95); BA at ≤23/24–29/30+ = 58/28/14% (63/21/15).
- **Institutions:** 89,220 public and 9,543 private K–12 schools (CCD 2024–25 with EDGE locations, PSS 2023–24) and 3,865 degree-granting colleges (IPEDS HD2023, EFFY2024). K–12: the nearest school offering the grade to home that September (private with the era's share); college by enrollment × `(1 + miles/15)^-1.8` from home at entry, 78% in the home state, two-year colleges for most who stop at some college; graduate school more national. Histories merge consecutive years at one school ("Waterloo Elementary K–4 → Waterloo Intermediate 5–6 → … → University of Wisconsin-Whitewater, left without a degree").
- **Exact** (`tests/education.rs`, both worlds and both packs): completed levels never fall and end at the final level; enrollment only within the path's dates; histories ordered without overlap.
- **Realism (`examples/mono_education.rs`):** BA+ by group for cohorts 1945/1975/1990: White 36.5/46.3/50.5 (35/44/47), Black 17.7/27.1/28.9 (20/29/29), Hispanic 16.2/22.2/26.5 (15/21/25), AIAN 19/22/18 (16). Overall attainment follows the world's composition (too few college graduates while the composition lacks migration).
- **Cost:** level ~1 µs, schooling at a date ~1 µs, a history 0.25–0.5 ms (a home lookup per school year); institutions 8.5 MB when first used (40 ms).
- **Directory:** `read_person` serves `education` (completed, enrolled, history with institutions).
- **Debts:** today's institutions serve every era; no grade retention; no immigrants' bimodal attainment until migration; residence's founders now live from birth (units seeded when they start) so worlds whose first year is late (us-tiny) have homes before it.

**Work (2026-10-04, Phase 6; `mono/work.rs`; pack `work.ron`, data `data/work.bin` from `data/distill_work.py`):** `career(x)` (spells from 16 to death), `work_at(x, t)`, `employer(x, job)` (on demand), `employer_info`, `job_title`, `occupation_info`, `pay_at`, `nominal`.
- **Career:** windows of schooling, keeping house and disability, then work. Students (from 16, and in college and graduate school) alternate part-time jobs with time out at the year's student share. Some women stay home before their first union, and others after a union or first birth, returning by cohort when the youngest child is about 6. Disability is a yearly hazard by age times an era factor, with returns. Work stretches walk jobs of lognormal length by starting age (NLSY79 completions), then a direct move (45%), a break (8%) or a search (lognormal weeks). Retirement age comes from each cohort's survival curve of working, by sex (`interp::inverse_decreasing`; cohorts interpolate the quantile's age).
- **Occupations and titles:** ACS 2023 PUMS shares by sex, age band and education, SOC major groups scaled by era factors (farm ×38 in 1900, factory ×3 to 1950, computing from 1950). Titles: 25,600 from the Census 2022 occupation index (coding notes, abbreviations, inverted forms such as "Flight Attendant Ramp", activities such as "Farming" and dated forms dropped), restricted to their industries, weighted by the ACS 2019 public-use write-ins (10,449 records), plus the occupation's own name made singular for a third of draws ("Registered Nurse", "Truck Driver"). A title is kept across jobs in the same occupation where it fits.
- **Employers:** K–12 teachers and anyone in the school industry at the nearest school offering a grade; college staff at a college by the education gravity; the self-employed at their own business; government workers at their county, state or the federal government; everyone else at an establishment `(county, industry, size class, index)`, the county the home's (or with 25% another of its commuting zone), the index below the class's establishment count there. Lines of business: 16,400 from the Census industry index ("Pizza Parlor", "Structural Iron Work"). Names come from the pack's patterns by NAICS prefix, after a place-like line 60% of the time ("Byrne's Diner", "Jefferson Blood Analysis Laboratory", "Blancher Freight Lines", "Hargrove & Sons").
- **Pay:** the occupation's median full-time wage (ACS) × an experience curve × the real-wage index × exp(occupation sd × (0.9 person + 0.44 job)); nominal by CPI-U (Minneapolis Fed estimates before 1913).
- **Exact** (`tests/work.rs`, both packs, several seeds): spells contiguous from the start to death, non-empty; jobs numbered in order; part-time exactly within schooling; retirement last; `work_at` is the spell in force; deterministic; employers resolve with names, and K–12 teachers work at schools. `internot/tests/directory.rs` checks the served records (current job, earlier jobs ordered without overlap, pay > 0).
- **Realism (`examples/mono_work.rs`, ×0.05), model/CPS:**
  - participation 16+, women/men: 1950 35/83 (33/87), 1970 44/75 (43/80), 2000 58/73 (60/75), 2023 54/65 (57/68);
  - by age band, mostly within 5 points; worst: women 35–44 in 1970 60 (51), women 16+ in 1900 26 (~20), men 65+ in 2023 20 (24);
  - unemployment 3–5%; median tenure 25–34/45–54 2.5–3.3/6.3–7.2 years (BLS 3.0/7.0); farm share of jobs 42/13/4/0.7% in 1900/1950/1970/2023 (38/12/4/~1);
  - jobs held, born 1957–64, at 18–24/25–34/35–44/45–54: 5.2/3.6/2.5/1.9 (NLSY79 5.6/4.5/2.9/2.2).
- **Cost** (perf gate, ×1): `career` p50 12 µs, p99 32 µs; `work_at` 6/29 µs; `employer` with its name and place 140/230 µs (a home lookup). Occupation tables 2.1 MB, loaded on first use. Kinship lookups unchanged (interleaved A/B against the names-era binary: identical answers, p50s within a 10 ns tick). The gate's committed baseline predates names; its p99s for father, children and count failed on a hot machine (88 °C) and passed the A/B, so the baseline needs re-recording on a cool machine (it will then include `full_name`, `career`, `work_at` and `employer`).
- **Establishment sizes follow the sample (fixed 2026-10-04):** counts per (county, industry, class) use the county's real weight times `sample_share(year)` (the world's alive over the places' total by decade: ×1 is 0.022 of the US), so a 1,000+ class establishment at ×1 has about 2% of a real one's staff; before, every class had ~45× too many establishments at ×1.
- **"Who works at E at t" has no index** (`examples/mono_roster_scan.rs`): a full scan at ×1 (5.8M adults, 3.4M employed, 16 threads) takes 10 s: enumerating the alive 0.05 s, `work_at` for each 9.7 s (~27 µs per person per thread), employers of the ~20k in the industry 0.3 s. Linear in population: about 7 minutes at the US's real scale. An answer in time proportional to its size needs rosters (open item 0) and a monotone job-to-establishment coupling inside them.
- **Data** (git-ignored `datasets/census_io/`, public domain, from `https://www2.census.gov/programs-surveys/demo/guidance/industry-occupation/`): `Census-2022-Occupation-Index_Final.xlsx`, `Census-2022-Industry-Index_Final.xlsx`, `pub-io-write-ins-acs2019.xlsx`. BLS refuses scripted downloads (the SOC direct-match titles were not used).
- **Debts:** no co-workers (rosters, open item 0): two workers drawn to the same establishment share it and its name, but nothing lists who works there; establishments have no open or close dates; lines of business uniform within an industry (a "Leprosy Hospital" now and then); part-time work only for students; no occupation by heritage group beyond education; pay ignores sex and region.

**Open, in order:**
0. **Areas and rosters (founder decision, `research/2026-10-04-areas-and-rosters.md`).** Coworkers, classmates, neighbours, friends and realistic residence all need a counted index of the people of a place, which the life-order index isn't. Options: (A) areas as kinship cells built lazily (~4 MB per cell: the US's 310 cells would be ~1.2 GB eager, ~20 MB per area touched); (B) areas as runs inside group cells (memory as A or on-the-fly eligibility; the cell world measured 5–20× slower lookups); (C) computed classes: per-cell state from a few numbers per cohort and shared tables (~64 B per cell cohort), lookups maybe 1.5–3× slower. Recommendation: prototype C's mother and death lookups, then A on top of C. Until decided, roster-free features are built.
1. **Birth and mother at 2×** (founder decision): accept ~1.56× (bulk ~1.8×), or add per-block tables (~16–33 MB, still flat in population) that put each birth-order bucket's mother class constants in one cache line.
2. **Sibling couples:** about 15 per world at any scale (random pairing between cohorts). Founder's choice: accept as debt, or a lookup-time check (~0.1–0.5 µs per spouse lookup).
3. **Remarriage** under option A: widow availability per (husband cohort, year) is a floor-sum count; divorcées per (wife class, duration). Design: `thinking/claude/008`–`010`.
4. **Rebuild on the monotone world** (the global world's phases): migration (life cells and movers, which the heritage composition needs), same-sex unions, areas, names, households, residence, education, work, ties.
5. **The infant band in closed form.**
6. **Calibration:** 31–37% of wives are older than their husbands (target ~22%).

### Removed worlds (history, 2026-09-30 → 2026-10-03)

Removed 2026-10-03 and archived in `.scratch/archive/2026-10-03-old-worlds/` (exact working-tree copies, many never committed) and git history (commit 3341d97 and earlier):
- the ledger world: `world.rs`, `ledger.rs`, `plan.rs`, `household.rs`, `residence.rs`, the naming in `names.rs`, with the old directory service, the perf `kinship` suite and many reports;
- the pure world, the transport world and the cell world (`zero.rs`).

Their designs, measurements and proofs stay in `docs/superpowers/plans/`, `specs/` and `research/` (including `research/2026-10-02-cell-world-math.md`, `research/2026-10-01-pure-world.md`, `research/2026-10-02-transport-world.md` and the Lean files in `research/proofs/`).

Lessons carried into the monotone world:
- kin repair needs exactness by construction;
- the census survivors convention;
- areas need residence-aware pairing (the B decision of 2026-10-01);
- the residence design A of closed units (`specs/2026-10-01-residence.md`).

**Found defect in the removed ledger world (from Tursi):** natives of the 1880 cohort were 99.9% partnered by 50, against the pack's 92% and the ledger's own hazard. Recorded in case the ledger is ever restored.

**Standing decisions that still apply:**

- **Name data (decided 2026-09-30): public-domain sources only.**
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
- **DeepSeek key (decided 2026-09-30):** the founder has a key, with ~$2 of credit now and more to be added.
   - **Update 2026-10-04 (founder):** the key is stored in Cloudflare AI Gateway's BYOK ("bring your own key") section, not locally. Rendering calls go through the gateway's DeepSeek endpoint (`https://gateway.ai.cloudflare.com/v1/<account>/<gateway>/deepseek`), which injects the key; what goes in the git-ignored `.env` is the gateway URL (and a gateway token if the gateway is authenticated), never the DeepSeek key. Ask the founder for the account and gateway ids when the renderer is built. Never paste keys or tokens into chat or commit them.
   - (Before: it was to go into `.env` as `OPENAI_BASE_URL=https://api.deepseek.com`, `OPENAI_API_KEY=...`.)
   - The spend guard (usage logging, running dollar total, cap) lands before the first paid call.
   - Top up to ~$10 before the pilot's main run.


**Lessons from R1 (durable):**
- **Constraints must stay local to the person drawn.** Conditioning a man's death on his partner's plan made 3.9% of deaths pay a two-block partner lookup. That tripled death's p99 and biased male e0 by up to 1.8 years. Before adding any constraint, ask which endpoint can evaluate it without a hop.
- **Sequential scans are nearly free next to dependent misses.** An inverted per-offset index replaced `mother`'s ~120-leaf scan. It cut instructions 43%, left cycles unchanged and added 31 MB, so it was reverted. Hardware prefetch streams a scan; a search is a chain of dependent loads.
- **Huge pages** (`MADV_COLLAPSE` over the heap) gave 10–20% on every lookup at 152 MB. That is worth having once the world lives in a few large arenas, but it is not a design lever.
- **Measure changes under 10% with interleaved A/B runs** (`perf/ab.sh` with `examples/mono_report.rs`, or `perf/society_ab.py`), not separate gate runs. Separate runs vary ~10%. In one case they showed a 13% "regression" in `father` that reversed when the run order was swapped.
- **A search and the read after it should touch one array.** Parallel arrays (starts in one, payload in another) cost two dependent misses; interleave them with a sentinel.
- **Finer structure costs lookups even when memory stays flat.** R1c's exact-year classes made about six times as many cells.
  - Sub-cells nested inside cells doubled instructions.
  - Flattening them into cells and then compacting to one cache line still left union at +41%: longer sorted arrays and colder cells.
  - Index long sorted arrays (`Coarse`, every 16th value).
  - Don't group columns to save memory if lookups must then scan the group.
- **Largest remainder is biased on small partitions:** a one-member split always goes to the modal class. Use keyed systematic apportionment (`partition::apportion_systematic`) wherever cells can be small.
- **Build-time work (2026-09-30):**
  - **Prove refactors bit-identical with checksums** (today: `mono_report`'s answer hashes): in the ledger world, a hash of the ledger's `Debug` output plus the lookups' answer sum. All ten build changes passed this way, so no realism rerun was needed.
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
- **Float results aren't reproducible across machines.** Rust documents `f64::ln`, `exp`, `sin`, `powf` and other transcendentals as non-deterministic: results can vary by platform, Rust version, and even between calls. `hash_gaussian`, the old `sampler` and trajectories (since removed) and `people` all used them, so a published seed may not rebuild the same world elsewhere.
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

# procedural_core — reference

**Keep this in context whenever building on `procedural_core`.** All math lives here (founder rule 1). Every function is a pure function of its inputs, with golden values and property tests; float math goes through `dmath` (pinned `libm`).

## Keys and hashes

- `key::Key`: structured keys and counter-based uniforms (`with`, `with2`, `with3`, `unit`, `below`). The worlds draw everything through keys.
- `hash::{hash_int, hash_float, hash_vec, hash_gaussian}`: hash-derived values of an id and a string key (xxh3), generic over `word::BitWord` (`u64`, `u128`, `U256`, `U512`).

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
- `inverse_decreasing(n, x(i), y(i), v)` (2026-10-04): where a piecewise-linear nonincreasing curve first falls to `v` (a survival curve's inverse, so a uniform `v` draws from it).

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
- `perm::AffinePerm` (2026-10-03, the monotone world's class permutation): `r ↦ (a·r + b) mod n` with `a` the golden multiplier coprime to `n`.
  - `fwd`/`inv` take 3–6 ns, and `inv_range` steps by one addition per element.
  - `count(lo, hi, c, d)` (how many `r` in an interval map into an interval) is two floor sums; `select` is a bisection.
  - `golden_pair(n)` gives `(a, a⁻¹)` once; store it and build with `from_pair` (no Euclid). `with_parts` takes an explicit `a`.
- `life::HazardTable` (2026-10-03): a cumulative hazard tabulated at fixed ages (from a `Siler`), linear between, with an exact inverse (`age_at`, one binary search) and `conditional_death_age`. No Newton steps. `remaining_life(from)`: life expectancy past `from`, exact for the piecewise-exponential law (the monotone world's group scaling).
- `fit::rake_columns_dense(row_total, q, targets, max_passes, tol)`: `rake_columns` over dense inputs, stopping at a tolerance; bit-identical to `rake_columns` for the passes it ran.
- `lattice::RationalBeatty::count_member(n)`: count and membership with one division. `InterleaveTable` stores 16 bytes per internal node (lengths carried down the walk) and locates with one division per level.
- `quantile` (2026-10-03, the speed pass): `OctaveTable` / `OctaveShape`, a monotone function of a tail probability `w = 1 − u` tabulated on geometric octaves of `w` (256 cells in the first octave, halving to 16 from the fifth), evaluated from the bits of `w` with one linear interpolation (no `ln`, no search), exactly monotone.
  - `threshold(pass)` / `threshold_near(v, pass)`: the least float `w` whose value passes a monotone predicate, exact (a knot search, then a gallop from the inverted interpolation);
  - `slot_w(ρ, n) = (2(n − ρ) − 1)/(2n)` and `count_from_threshold(n, w)`: exact counts of quantile slots on one side of a threshold;
  - `OctaveShape::tabulate_into` fills tables of one shape in one arena.
- `perm` additions (the speed pass): `CompactPerm4` (the compact Feistel network with 4 rounds; `CompactPermR<R>` is the family, `CompactPerm` is 6 rounds with its golden values unchanged); `rem_by_inverse` / `divmod_by_inverse` (division by a stored `⌊(2⁶⁴−1)/n⌋`, one correction); Feistel rounds below 2³⁰ fold mix64's first xorshift into the key (bit-identical); `AffinePerm::fwd`/`inv` use 64-bit arithmetic when the product fits.
- `partition::SystematicShares::{offset, part_of_offset, end_of_offset}`: store a split's systematic offset instead of hashing its key per call.
- `perm::least_cost_assignment(k ≤ 3, cost)`: the first-minimum permutation.
- `stream::Epochs::keyed(start, span, key)`, with `index(t)` and `start(e)`: fixed epochs with a keyed phase.

**`lattice`: exact counts over lattice patterns.**
- `floor_sum(n, m, a, b)`: `Σ⌊(a·i + b)/m⌋` in O(log) (Euclid on the line; 64-bit fast path, else 128-bit). It is how `AffinePerm::count` counts.
- `RationalBeatty { t, len, tau }`: exactly `t` members of `len` positions, evenly spread, with `count`, `member`, `select` and the complement's `select_out`.
- `ExactInterleave { categories, prefix, key }`: categories of integer sizes interleaved by a balanced tree of rational Beatty splits. Every category gets exactly its size, any prefix holds it within the tree depth of its share, and `locate`, `count` and `select` cost O(log categories). The monotone world lays out each wife cohort's husbands with it.

**Further additions (2026-10-02, still in core):**
- `partition`:
  - `proportional_owner(j, c, m) = ⌊j·m/c⌋` and its dual `proportional_range(i, c, m)` (children choose mothers);
  - `residue_count` / `residue_select`;
  - `cyclic_next` / `cyclic_run_before`;
  - `quantile_rank_count(n, f) = ⌈n·f − ½⌉`.
- `curve`:
  - `algebraic_sigmoid` and its inverse;
  - `truncated_sigmoid_cdf` / `_inv`;
  - `log_logistic_quantile`.
- `life`:
  - `Siler::{cumulative_hazard, age_at_cumulative_hazard, conditional_death_age, survival}`;
  - `remaining_share(ever, cdf)`.
- `pmf::band_quantile(bands, share, q, default)`.
- `perm`: `staggered_blocks` / `StaggeredBlocks`, `PermShape`, `CompactPerm::shaped`, `SegmentedPerm` (built for the removed worlds; unused today).

**`stream`: regeneration.**
- `regen_state` is the single-level form (Phase 0).
- `nested_regen(levels, t, last_before(j, t), &mut [Option<Regen>])` handles nested levels, 0 the coarsest. A move at level j redraws levels ≥ j, so level k's regeneration is the latest move of any level ≤ k (at equal times the finer level counts as later).
- `nested_regen_state(levels, t, last_before, initial(k, parent), draw(k, regen, key, parent), &mut out)` gives each level's state: the draw at its last regeneration, under the parent level's state.
- Each costs one `last_before` per level, with no replay, and matches a full replay exactly (`stream::tests`).

## Design principles (don't violate)

1. **Everything is a pure function of (id, key) or (id, key, t).** No state.
2. **Counts a search asks for must be closed form** (the monotone world's rules): couplings between index spaces preserve order, and at most one affine permutation acts inside a class.
3. **For mutation, keep the core read-only:** agent actions live in a session-scoped overlay, rebuilt when the first mutating service needs one (the old `procedural_overlay` crate is in git history).

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

- `AffinePerm::new(n, key)` runs an extended Euclid near `n/φ` (Euclid's worst case for steps), about 70–270 ns. On hot paths, store `golden_pair(n)` per class and build with `AffinePerm::from_pair`.
- `HazardTable` is linear in the cumulative hazard between knots (a piecewise-exponential law): close to its `Siler`, not bit-equal between knots.
