//! # internot_perf: measured, enforced performance budgets
//!
//! Every performance number in the spec (§16 of
//! `docs/superpowers/specs/2026-09-29-society-as-a-function.md`) is a gate,
//! not an estimate. This crate measures them and enforces them.
//!
//! - [`harness`]: runs a function over many inputs and records per-call
//!   latency into an HDR histogram: p50, p90, p99, p99.9, max,
//!   throughput, the slowest inputs (for replay), warm or cold mode.
//! - [`alloc`]: a counting global allocator, so "zero heap allocations on
//!   hot paths" is checked, not assumed.
//! - [`counters`]: cycles, instructions, IPC, cache and branch misses from
//!   the CPU's performance counters.
//! - [`flamegraph`]: SVG flamegraphs from an in-process sampling profiler.
//! - [`budget`] and [`gate`]: `perf/budgets.toml`, and the comparison of a
//!   run against budgets and the committed per-machine baseline.
//! - [`suites`]: the registered benchmark suites, run by the `perf-gate`
//!   binary.
//!
//! `perf/README.md` covers running the gate, baselines, flamegraphs and
//! noise control.
//!
//! ## Adding a suite
//!
//! 1. Create `internot_perf/src/suites/<name>.rs`:
//!
//!    ```ignore
//!    use super::Suite;
//!    use crate::harness::Harness;
//!    use crate::inputs::InputRng;
//!
//!    pub const SUITE: Suite = Suite {
//!        name: "feistel",
//!        about: "Phase 0 Feistel permutation over the full id space",
//!        run,
//!    };
//!
//!    fn run(h: &mut Harness) {
//!        // Uniform inputs over the whole space, from a fixed seed, plus the
//!        // fixed worst-case set (§16.4) appended at the end.
//!        let mut rng = InputRng::new(0xFE15);
//!        let ids: Vec<u64> = (0..h.samples()).map(|_| rng.below(ID_SPACE)).collect();
//!
//!        // Warm: the common case. The closure's output is black-boxed.
//!        h.dist("permute", &ids, |&id| permute(id));
//!
//!        // Cold: `reset` runs untimed before every call.
//!        let cache = QuotaCache::new();
//!        h.bench("quota_lookup")
//!            .cold(|| cache.clear())
//!            .samples(2_000) // cap for expensive calls
//!            .run(&ids, |&id| cache.quota(id));
//!    }
//!    ```
//!
//!    Benchmarks are named `<suite>/<bench>` (here `feistel/permute`).
//!    Inputs must implement `Debug`: that rendering is what the report
//!    keeps for the slowest inputs.
//! 2. Register it in `src/suites/mod.rs`: add `mod feistel;` and put
//!    `feistel::SUITE` in [`suites::ALL`]. If it needs another crate (say
//!    `internot`), add that to `internot_perf/Cargo.toml`.
//! 3. Budget every benchmark in `perf/budgets.toml`. The gate fails any
//!    benchmark without one:
//!
//!    ```toml
//!    [budget."feistel/permute"]
//!    mode = "warm"
//!    p99_ns = 100
//!    max_allocs = 0
//!    ```
//! 4. Run `cargo run --release -p internot_perf --bin perf-gate -- --suite feistel`,
//!    then record a baseline with `--record-baseline` (on a quiet machine,
//!    ideally with the `performance` governor).
//!
//! Measuring from a test or another binary works the same way: build a
//! [`Harness`], call the suite function, and read
//! [`Harness::into_reports`]. Install [`alloc::CountingAllocator`] as the
//! global allocator there too, or allocation figures come back `None`.

pub mod alloc;
pub mod budget;
pub mod counters;
pub mod error;
pub mod flamegraph;
pub mod gate;
pub mod harness;
pub mod inputs;
pub mod meta;
pub mod report;
pub mod results;
pub mod suites;
mod table;
pub mod thermal;

pub use error::{Error, Result};
pub use harness::{Bench, Harness, HarnessConfig};
pub use report::{BenchReport, Mode};
pub use suites::Suite;
pub use table::format_ns;

// The unit-test binary counts allocations, so the harness tests can check
// allocation figures.
#[cfg(test)]
#[global_allocator]
static TEST_ALLOCATOR: alloc::CountingAllocator = alloc::CountingAllocator;
