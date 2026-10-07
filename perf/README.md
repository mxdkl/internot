# perf/: performance budgets and the gate

Every performance number in spec §16 is a gate, not an estimate. `perf-gate`
(crate `internot_perf`) measures each benchmark's latency distribution, checks
it against `budgets.toml` and this machine's committed baseline, and exits
non-zero on any failure.

```
perf/
  budgets.toml                           budgets (committed)
  baselines/<machine>/<suite>.json       per-machine baselines (committed)
  results/latest/<suite>.json            last run (git-ignored)
  flamegraphs/<suite>/<bench>.svg        flamegraphs (git-ignored)
  check.sh                               local CI: workspace tests + gate
```

## Run the gate

```sh
perf/check.sh                 # cargo test --workspace, then the full gate (release)
perf/check.sh --quick         # same, with a tenth of the samples (smoke run)
cargo run --release -p internot_perf --bin perf-gate -- --suite core_hash   # one suite
cargo run --release -p internot_perf --bin perf-gate -- --list              # suites
```

Exit status: `0` pass, `1` gate failed, `2` usage or I/O error. Debug builds
are refused.

Output per suite: a latency table (p50, p90, p99, p99.9, max, mean,
throughput, allocations per call), a hardware-counter table, then the gate
table: benchmark, p50, p99, p99 budget, baseline p99, delta, status. The full
numbers, including each benchmark's slowest inputs (by `Debug` value, for
replay), go to `perf/results/latest/<suite>.json`.

A benchmark **fails** when it:
- has no budget (every measured number is gated);
- was measured warm but budgeted cold, or vice versa;
- exceeds `p99_ns`, `p50_ns` or `max_allocs`;
- has p99 more than 10% above the baseline (see "The 10% rule" below).

A budget whose benchmark its suite no longer measures also fails: a rename
must not quietly drop a gate. A missing baseline is not a failure; the table
says `no baseline`.

### How a benchmark is measured

0. Thermal settle (full runs): before each benchmark the harness waits until
   the CPU is at or below 65 °C (`internot_perf::thermal`, reading k10temp or
   coretemp). It prints the wait when it takes over 0.5 s. `PERF_NO_COOL=1`
   skips this. The reference laptop goes from 63 °C to 93 °C within about 7 s
   of load, and a hot run is slower.
1. Warm-up: warm mode makes one full pass over the inputs (filling memo
   caches), then keeps calling for at least 0.5 s so the clock ramps up.
2. Batch calibration (warm mode): each timed sample repeats one input enough
   times to last about 2 µs, so clock-read overhead stays near 1%. **Calls
   of 250 ns or more (mean) are timed one per sample.** A batch repeats one
   input, so its later calls run from cache and hide the DRAM misses that
   dominate a lookup. On the old kinship suite, batching made p99 flip by
   ±40% depending on whether calibration picked a batch of 1 or 2. A suite
   can force single calls with `.batch(1)`; `society` does for every lookup.
3. Sampling: 100,000 samples (10,000 with `--quick`), one input each, visited
   in a fixed shuffled order. Allocations are counted in the same region.
4. Summary: the samples are split, in time order, into up to 10 rounds.
   Percentiles are the **median over rounds** of each round's percentile, so a
   few-millisecond clock dip moves one round, not the number. `max` and
   `mean` use every sample. `round_p99_ns` in the JSON shows each round's p99.
5. Hardware counters: counted in a separate, untimed pass.

**Cold mode** (`.cold(reset)`) runs `reset` before every call, untimed, and
times single calls: the "empty caches" column of §16.2.

**Re-measurement:** when a benchmark fails only on latency (a budget or a
regression), the gate measures it again, up to 3 attempts in all, and keeps
the fastest. A real regression reproduces; a background job does not. The
report's `attempts` field records this.

## Add a suite and a budget

The full recipe, with a code example, is in the crate docs of
`internot_perf/src/lib.rs`. In short:

1. Write `internot_perf/src/suites/<name>.rs` exporting
   `pub const SUITE: Suite`. Its `run(h: &mut Harness)` builds inputs
   (`InputRng` with a fixed seed, uniform over the whole id space, plus the
   §16.4 worst cases) and calls `h.dist("bench", &inputs, |x| f(x))`, or
   `h.bench("bench").cold(|| clear_caches()).samples(n).run(&inputs, f)`.
2. Add `mod <name>;` and `<name>::SUITE` to `ALL` in
   `internot_perf/src/suites/mod.rs`.
3. Add a budget per benchmark (names are `<suite>/<bench>`):

   ```toml
   [budget."feistel/permute"]
   mode = "warm"      # required: "warm" or "cold"
   p99_ns = 100       # required
   p50_ns = 40        # optional
   max_allocs = 0     # optional: most heap allocations in any single call
   ```

   Unknown keys are errors, so a typo cannot disable a gate. Budgets come from
   §16.2; a budget is never raised silently.
4. Run the suite, then record a baseline.

## Record a baseline

```sh
sudo cpupower frequency-set -g performance      # if you can (see Noise control)
cargo run --release -p internot_perf --bin perf-gate -- --record-baseline
git add perf/baselines/
```

Baselines are per machine: `perf/baselines/<cpu-model>-<threads>t/<suite>.json`,
for example `amd-ryzen-9-5900hx-with-radeon-graphics-16t`. Other machines
see `no baseline`. Every file records the CPU, governor, rustc, commit and
timestamp under `run`.

Recording is refused for `--quick` runs, and when any budget fails. Recording
can accept a regression; it can never paper over a budget. Record after an
intentional performance change, and say why in the commit message.

## Flamegraphs

```sh
# One benchmark (debug info makes frames resolve best):
cargo run --profile profiling -p internot_perf --bin perf-gate -- \
    --suite core_hash --flamegraph core_hash/lognormal
# The three benchmarks with the worst p99:
cargo run --release -p internot_perf --bin perf-gate -- --flamegraphs-top 3
```

This writes `perf/flamegraphs/<suite>/<bench>.svg`; open it in a browser to
zoom and search. The profiler is pprof's in-process sampler, so no root and no
`perf` binary are needed. Profiling runs in a separate pass, after
measurement, so it never skews the numbers. Each graph samples for 5 s (2 s
with `--quick`). The effective rate is capped by the kernel tick (about
250 Hz on the reference machine), so graphs have roughly 1,250 samples.

From code, `internot_perf::flamegraph::profile_to_svg(path, title, duration, || ...)`
profiles any closure.

## Hardware counters

Every suite prints, per call: cycles, instructions, IPC, cache references,
cache misses and branch misses. The same numbers are stored under `counters`
in the results JSON. They come from `perf_event_open` for the calling thread,
user space only, which works unprivileged at `perf_event_paranoid <= 2`. The
cache events are the kernel's generic ones; on AMD Zen they count L2 accesses
and misses.

Cycles and instructions per call do not depend on the clock speed. When
latency moves but cycles per call do not, the machine changed, not the code.
On the reference machine a warm laptop measured `hash_int` at 26 ns instead
of 22 ns, both at 100.5 cycles per call.

If counters are unavailable (paranoid level 3, a VM without a PMU), the header
says `hardware counters: unavailable (<reason>)`, the fields are `null` with a
`note`, and the gate still runs. For ad-hoc use from Rust:

```rust
let (result, counters) = internot_perf::counters::measure(|| work());
println!("{:?} IPC {:?}", counters.instructions, counters.ipc());
```

## Allocations

`perf-gate` installs `internot_perf::alloc::CountingAllocator`, which counts
per thread. The `allocs/call` column is the most allocations any single call
made, and `max_allocs = 0` enforces the §16.2 rule of zero heap allocations
on hot paths. In your own test crate:

```rust
#[global_allocator]
static A: internot_perf::alloc::CountingAllocator = internot_perf::alloc::CountingAllocator;

let (_, count) = internot_perf::alloc::count_allocs(|| feistel(42));
assert_eq!(count.allocations, 0);
```

## Noise control

The reference machine (Ryzen 9 5900HX laptop) runs the `performance`
governor since 2026-09-30, and all baselines were recorded under it.
`perf-gate` warns when the governor is not `performance`, and records it in
every results file (`run.machine.governor`).

- Record baselines with `sudo cpupower frequency-set -g performance`
  (restore with `-g powersave`), on AC power, on a quiet machine: no builds,
  no browser.
- Back-to-back runs heat a laptop, and p99 drifts up 3-7%. Let it cool
  before recording.
- Built-in defences: time-sliced rounds with a median (above), and up to 3
  attempts for latency failures.

Measured on the reference machine under `powersave`: pooling all samples into
one p99 jumped 20-25% in about a third of runs. With rounds, 10 back-to-back
full runs stayed within +9% of the baseline.

### A/B runs, for changes under ~10%

Separate gate runs of a DRAM-bound lookup still vary by about ±10%, so the gate cannot see a 5% change. To compare two versions of a source file, run them alternately with `perf/ab.sh`. Alternating cancels drift in temperature and background load:

```sh
cp internot_society/src/mono.rs .scratch/mono_before.rs   # before the change
# ... edit mono.rs ...
perf/ab.sh internot_society mono_report internot_society/src/mono.rs .scratch/mono_before.rs 3
```

- **What it does:** it builds the example with the current file (B) and with the saved one (A), and restores the file afterwards. It then prints A and B alternately, cooling the CPU to 60 °C before each run.
- **Checksums:** `mono_report` prints `p50/p99/mean` per lookup and a checksum of every answer. A change that should not alter any answer must print the same checksum for A and B.
- **Two saved binaries:** `perf/society_ab.py BASE NEW ROUNDS` runs two built `mono_report` binaries alternately (×1, then ×1000 with `BOUNDS_ONLY`) and prints medians, changes and checksum agreement for every metric.
- **Reading the output:** trust a difference only when it holds in every pair. If one run is an outlier on every query, that run was disturbed; rerun it.

## The 10% rule

A benchmark fails when its p99 is more than 10% above the committed baseline
for this machine (`[gate] max_regression = 0.10` in `budgets.toml`). An
improvement larger than 10% passes, and the status suggests re-recording so
the gain is locked in. The regression check compares p99 only; budgets gate
p99, and optionally p50 and allocations.

## Phase gates

A phase of `specs/2026-09-29-society-as-a-function.md` is done only when its benchmarks pass the gate and flamegraphs of its three most expensive queries are committed (§19). `perf/flamegraphs/` is regenerated on every run and ignored by git, so copy a phase's top three into `perf/phase-gates/<phase>/` when the phase closes:

```
cargo run --profile profiling -p internot_perf --bin perf-gate -- --suite <suite> --flamegraphs-top 3
cp perf/flamegraphs/<suite>/*.svg perf/phase-gates/<phase>/
```

Phase 0 closed on 2026-09-30. Its top three were `count_tree_select_24` (binomial splits, BTPE), `poisson_tree_last_before` and `poisson_buckets_30d` (event placement by exponential spacings).
