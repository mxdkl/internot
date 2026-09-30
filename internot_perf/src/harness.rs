//! The distribution harness: runs a function over many inputs and records
//! the per-call latency of each into an HDR histogram.
//!
//! For every benchmark the harness:
//! 1. **warms up**: in warm mode, one full pass over the inputs (so memo
//!    caches hold every input), then more calls until `warmup` has elapsed,
//!    which also gives a `powersave` governor time to raise the clock;
//! 2. **calibrates a batch** (warm mode): the number of back-to-back calls
//!    per timed sample that makes a sample last about `target_sample`, so
//!    the ~20–30 ns cost of reading the clock stays around 1%. Calls of
//!    250 ns or more are not batched: repeats of one input run from cache
//!    and would hide memory costs;
//! 3. **samples**: each sample is one input called `batch` times, recorded
//!    as per-call latency. Heap allocations are counted around the same
//!    region. Batching repeats the *same* input, so a sample is one input's
//!    latency and the slowest inputs can be replayed; the price is that a
//!    warm sample runs with that input's branches already predicted.
//!    Inputs are visited in a fixed shuffled order, so any cluster of
//!    inputs (say, a worst-case set appended at the end) is spread over
//!    every round of the [`LatencySummary`];
//! 4. **counts hardware events** in a separate untimed pass.
//!
//! Cold mode calls a `reset` closure (untimed) before every sample and
//! times single calls, for "empty caches" numbers.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::alloc::{self, AllocCount};
use crate::counters::{CounterGroup, HwCounters, PerCallCounters};
use crate::error::Result;
use crate::flamegraph;
use crate::inputs::InputRng;
use crate::report::{AllocSummary, BenchReport, LatencySummary, Mode, SlowInput, PS_PER_NS};

/// Upper bound on calls per timed sample.
const MAX_BATCH: u64 = 1 << 16;

/// Calls at least this long (mean, ns) are timed one per sample: the clock
/// read (~20–30 ns) is then under about 10% of a call.
const UNBATCHED_NS: f64 = 250.0;

/// Upper bound on inputs the hardware-counter pass visits.
const COUNTER_PASS_MAX_INPUTS: usize = 10_000;

/// Seed of the shuffled order in which samples visit inputs.
const INPUT_ORDER_SEED: u64 = 0x5EED_0F0D;

/// How much to measure.
#[derive(Clone, Debug)]
pub struct HarnessConfig {
    /// Timed samples per benchmark.
    pub samples: usize,
    /// Minimum warm-up time per benchmark.
    pub warmup: Duration,
    /// Target duration of one warm sample; sets the batch size.
    pub target_sample: Duration,
    /// Sampling stops after this long even if `samples` is not reached
    /// (the report then says `hit_time_limit`).
    pub time_limit: Duration,
    /// How many of the slowest distinct inputs to keep for replay.
    pub slowest: usize,
    /// How long one flamegraph samples its benchmark.
    pub flamegraph_time: Duration,
    /// Before each benchmark, wait until the CPU has cooled to this
    /// temperature (°C), so every benchmark starts from the same thermal
    /// state (see [`crate::thermal`]). `None` skips it; setting
    /// `PERF_NO_COOL` in the environment does too.
    pub cool_to_c: Option<f64>,
}

impl HarnessConfig {
    /// Settings for gate runs and baselines.
    pub fn full() -> Self {
        Self {
            samples: 100_000,
            warmup: Duration::from_millis(500),
            target_sample: Duration::from_micros(2),
            time_limit: Duration::from_secs(20),
            slowest: 10,
            flamegraph_time: Duration::from_secs(5),
            cool_to_c: std::env::var_os("PERF_NO_COOL").is_none().then_some(65.0),
        }
    }

    /// Settings for smoke runs (`--quick`): a tenth of the samples.
    pub fn quick() -> Self {
        Self {
            samples: 10_000,
            warmup: Duration::from_millis(100),
            time_limit: Duration::from_secs(3),
            slowest: 5,
            flamegraph_time: Duration::from_secs(2),
            cool_to_c: None,
            ..Self::full()
        }
    }
}

/// Collects measurements for one suite. A suite's `run` function receives
/// it and registers benchmarks with [`Harness::dist`] or
/// [`Harness::bench`].
pub struct Harness {
    suite: String,
    config: HarnessConfig,
    alloc_counting: bool,
    names: BTreeSet<String>,
    /// `Some`: only these benchmarks run; the suite's others are skipped.
    only: Option<BTreeSet<String>>,
    /// `Some` in flamegraph mode: only these benchmarks run, and they are
    /// profiled instead of measured.
    flamegraph_targets: Option<BTreeMap<String, PathBuf>>,
    reports: Vec<BenchReport>,
    flamegraphs: Vec<(String, Result<PathBuf>)>,
}

impl Harness {
    /// A harness that measures every benchmark of `suite`.
    pub fn new(suite: &str, config: HarnessConfig) -> Self {
        Self {
            suite: suite.to_owned(),
            config,
            alloc_counting: alloc::counting_installed(),
            names: BTreeSet::new(),
            only: None,
            flamegraph_targets: None,
            reports: Vec::new(),
            flamegraphs: Vec::new(),
        }
    }

    /// A harness that measures nothing and instead writes a flamegraph for
    /// each target: full benchmark name → SVG path. Other benchmarks of the
    /// suite are skipped without running.
    pub fn for_flamegraphs(
        suite: &str,
        config: HarnessConfig,
        targets: BTreeMap<String, PathBuf>,
    ) -> Self {
        Self {
            flamegraph_targets: Some(targets),
            ..Self::new(suite, config)
        }
    }

    /// Restricts the run to these benchmarks (full names); the suite's
    /// other benchmarks are skipped without running.
    pub fn only(mut self, names: BTreeSet<String>) -> Self {
        self.only = Some(names);
        self
    }

    /// Whether the benchmark with this full name should run.
    fn selects(&self, name: &str) -> bool {
        self.only.as_ref().is_none_or(|only| only.contains(name))
            && self
                .flamegraph_targets
                .as_ref()
                .is_none_or(|t| t.contains_key(name))
    }

    /// Samples per benchmark in this run. Generate at least this many
    /// inputs so every sample sees a distinct one.
    pub fn samples(&self) -> usize {
        self.config.samples
    }

    /// Measures `f` over `inputs` in warm mode with default settings.
    /// Samples visit the inputs in a fixed shuffled order, cycling if there
    /// are fewer than [`samples`](Self::samples). The benchmark's full name
    /// is `<suite>/<name>`.
    pub fn dist<I: Debug, O>(&mut self, name: &str, inputs: &[I], f: impl FnMut(&I) -> O) {
        self.bench(name).run(inputs, f);
    }

    /// Starts a benchmark with non-default settings; finish with
    /// [`Bench::run`].
    ///
    /// # Panics
    ///
    /// Panics if the suite already registered a benchmark with this name.
    pub fn bench<'r>(&mut self, name: &str) -> Bench<'_, 'r> {
        let full_name = format!("{}/{}", self.suite, name);
        assert!(
            self.names.insert(full_name.clone()),
            "benchmark {full_name} is registered twice"
        );
        Bench {
            harness: self,
            name: full_name,
            samples: None,
            batch: None,
            reset: None,
        }
    }

    /// The measured reports, in registration order.
    pub fn into_reports(self) -> Vec<BenchReport> {
        self.reports
    }

    /// Flamegraph outcomes, by benchmark name (flamegraph mode only).
    pub fn into_flamegraphs(self) -> Vec<(String, Result<PathBuf>)> {
        self.flamegraphs
    }
}

/// A benchmark being configured. Created by [`Harness::bench`].
#[must_use = "a benchmark does nothing until .run() is called"]
pub struct Bench<'h, 'r> {
    harness: &'h mut Harness,
    name: String,
    samples: Option<usize>,
    batch: Option<u64>,
    reset: Option<Box<dyn FnMut() + 'r>>,
}

impl<'r> Bench<'_, 'r> {
    /// Caps this benchmark's samples below the harness setting, for
    /// expensive calls. p99 needs a few hundred samples to mean anything.
    pub fn samples(mut self, samples: usize) -> Self {
        self.samples = Some(samples);
        self
    }

    /// Fixes the calls per timed sample instead of calibrating it (warm
    /// mode only).
    pub fn batch(mut self, batch: u64) -> Self {
        self.batch = Some(batch);
        self
    }

    /// Cold mode: `reset` runs, untimed, before every call, and calls are
    /// timed one at a time. Use it to empty the caches under test.
    pub fn cold(mut self, reset: impl FnMut() + 'r) -> Self {
        self.reset = Some(Box::new(reset));
        self
    }

    /// Runs the benchmark over `inputs`.
    ///
    /// # Panics
    ///
    /// Panics if `inputs` is empty, or if a batch above 1 was set in cold
    /// mode.
    pub fn run<I: Debug, O>(self, inputs: &[I], mut f: impl FnMut(&I) -> O) {
        let Bench {
            harness,
            name,
            samples,
            batch,
            reset,
        } = self;
        assert!(!inputs.is_empty(), "benchmark {name} has no inputs");
        let mode = if reset.is_some() {
            Mode::Cold
        } else {
            Mode::Warm
        };
        assert!(
            mode == Mode::Warm || batch.is_none_or(|b| b == 1),
            "benchmark {name}: cold mode times single calls, so it cannot batch"
        );
        if !harness.selects(&name) {
            return;
        }
        let flamegraph_out = harness
            .flamegraph_targets
            .as_ref()
            .and_then(|targets| targets.get(&name).cloned());

        let config = &harness.config;
        let samples = samples
            .map_or(config.samples, |s| s.min(config.samples))
            .max(1);
        let mut subject = Subject {
            inputs,
            order: shuffled_indices(inputs.len()),
            reset,
            call: move |input: &I| {
                black_box(f(input));
            },
        };
        let first_pass = match mode {
            Mode::Warm => samples.min(inputs.len()),
            Mode::Cold => 1,
        };
        if let Some(limit) = config.cool_to_c {
            if let Some(s) = crate::thermal::settle(limit, Duration::from_secs(60)) {
                if s.waited > Duration::from_millis(500) {
                    eprintln!(
                        "  {name}: cooled {:.0} -> {:.0} C in {:.1} s",
                        s.from_c,
                        s.to_c,
                        s.waited.as_secs_f64()
                    );
                }
            }
        }
        subject.warm_up(first_pass, config.warmup);
        let batch = match mode {
            Mode::Cold => 1,
            Mode::Warm => batch.unwrap_or_else(|| subject.calibrate_batch(config.target_sample)),
        };

        match flamegraph_out {
            Some(out) => {
                let mut sample = 0;
                let result =
                    flamegraph::profile_to_svg(&out, &name, config.flamegraph_time, || {
                        subject.run_sample(sample, batch);
                        sample += 1;
                    })
                    .map(|()| out);
                harness.flamegraphs.push((name, result));
            }
            None => {
                let report =
                    subject.measure(name, mode, samples, batch, config, harness.alloc_counting);
                harness.reports.push(report);
            }
        }
    }
}

/// Indices `0..len` in a fixed pseudo-random order (Fisher–Yates).
fn shuffled_indices(len: usize) -> Vec<usize> {
    let mut rng = InputRng::new(INPUT_ORDER_SEED);
    let mut order: Vec<usize> = (0..len).collect();
    for i in (1..len).rev() {
        order.swap(i, rng.below(i as u64 + 1) as usize);
    }
    order
}

/// The function under test, its inputs, and the optional cold-mode reset.
struct Subject<'a, 'r, I, F> {
    inputs: &'a [I],
    /// The order samples visit inputs in: sample `s` uses
    /// `inputs[order[s % len]]`.
    order: Vec<usize>,
    reset: Option<Box<dyn FnMut() + 'r>>,
    /// Calls the benchmarked function and `black_box`es its output.
    call: F,
}

impl<I: Debug, F: FnMut(&I)> Subject<'_, '_, I, F> {
    fn reset(&mut self) {
        if let Some(reset) = &mut self.reset {
            reset();
        }
    }

    /// The input index sample number `sample` uses.
    fn input_index(&self, sample: usize) -> usize {
        self.order[sample % self.order.len()]
    }

    /// One untimed sample: a reset in cold mode, then `batch` back-to-back
    /// calls on the input of sample number `sample`.
    fn run_sample(&mut self, sample: usize, batch: u64) {
        let inputs = self.inputs;
        let input = &inputs[self.input_index(sample)];
        self.reset();
        for _ in 0..batch {
            (self.call)(black_box(input));
        }
    }

    fn warm_up(&mut self, first_pass: usize, min_time: Duration) {
        let inputs = self.inputs;
        let start = Instant::now();
        let mut k = 0;
        while k < first_pass || start.elapsed() < min_time {
            self.reset();
            (self.call)(black_box(&inputs[k % inputs.len()]));
            k += 1;
        }
    }

    /// Calls per sample needed for a sample to last about `target`.
    fn calibrate_batch(&mut self, target: Duration) -> u64 {
        let inputs = self.inputs;
        let mut calls: u64 = 1;
        loop {
            let start = Instant::now();
            for k in 0..calls {
                let input = &inputs[self.input_index(k as usize)];
                (self.call)(black_box(input));
            }
            let elapsed = start.elapsed();
            if elapsed >= target || calls >= MAX_BATCH {
                let per_call_ns = (elapsed.as_nanos() as f64 / calls as f64).max(0.01);
                // A batch repeats one input, so every call after the first
                // finds its data in cache. For calls long enough that the
                // clock read is noise, batching only hides memory costs
                // (and a batch flipping between 1 and 2 swung p99 by ~40%
                // between runs), so they are timed one call at a time.
                if per_call_ns >= UNBATCHED_NS {
                    return 1;
                }
                let batch = (target.as_nanos() as f64 / per_call_ns).ceil() as u64;
                return batch.clamp(1, MAX_BATCH);
            }
            calls *= 2;
        }
    }

    fn measure(
        &mut self,
        name: String,
        mode: Mode,
        samples: usize,
        batch: u64,
        config: &HarnessConfig,
        alloc_counting: bool,
    ) -> BenchReport {
        let sampled = self.sample(samples, batch, config.time_limit, config.slowest);
        let counter_inputs = (sampled.taken / 10)
            .clamp(1, COUNTER_PASS_MAX_INPUTS)
            .min(self.inputs.len());
        let counters = self.count_hw(counter_inputs, batch);
        let calls = sampled.taken as u64 * batch;
        let slowest = sampled
            .slowest
            .into_sorted()
            .into_iter()
            .map(|(ps, index)| SlowInput {
                ns: ps as f64 / PS_PER_NS,
                input: format!("{:?}", self.inputs[index]),
            })
            .collect();
        BenchReport {
            name,
            mode,
            attempts: 1,
            batch,
            calls,
            distinct_inputs: sampled.taken.min(self.inputs.len()) as u64,
            hit_time_limit: sampled.taken < samples,
            latency: LatencySummary::from_samples(&sampled.per_call_ps),
            slowest,
            allocs: alloc_counting.then(|| AllocSummary {
                max_per_call: sampled.max_allocs_per_call,
                mean_per_call: sampled.allocs.allocations as f64 / calls as f64,
                mean_bytes_per_call: sampled.allocs.bytes as f64 / calls as f64,
            }),
            counters,
        }
    }

    fn sample(
        &mut self,
        samples: usize,
        batch: u64,
        time_limit: Duration,
        keep_slowest: usize,
    ) -> Sampled {
        let inputs = self.inputs;
        let mut sampled = Sampled {
            per_call_ps: Vec::with_capacity(samples),
            slowest: Slowest::new(keep_slowest),
            taken: 0,
            allocs: AllocCount::default(),
            max_allocs_per_call: 0,
        };
        let deadline = Instant::now() + time_limit;
        for s in 0..samples {
            let index = self.input_index(s);
            let input = &inputs[index];
            self.reset();
            let ((start, end), allocs) = alloc::count_allocs(|| {
                let start = Instant::now();
                for _ in 0..batch {
                    (self.call)(black_box(input));
                }
                (start, Instant::now())
            });
            let per_call_ps = u64::try_from((end - start).as_nanos() * 1_000 / u128::from(batch))
                .unwrap_or(u64::MAX);
            sampled.per_call_ps.push(per_call_ps);
            sampled.slowest.offer(per_call_ps, index);
            sampled.allocs.allocations += allocs.allocations;
            sampled.allocs.bytes += allocs.bytes;
            sampled.max_allocs_per_call = sampled
                .max_allocs_per_call
                .max(allocs.allocations.div_ceil(batch));
            sampled.taken += 1;
            if end >= deadline {
                break;
            }
        }
        sampled
    }

    fn count_hw(&mut self, inputs: usize, batch: u64) -> PerCallCounters {
        match self.try_count_hw(inputs, batch) {
            Ok((totals, calls)) => totals.per_call(calls),
            Err(e) => PerCallCounters::unavailable(e.to_string()),
        }
    }

    /// Counter totals over the inputs of the first `count` samples, and the
    /// calls they cover. Warm mode counts whole batches in one region; cold
    /// mode switches counters on around each call so resets are excluded.
    fn try_count_hw(&mut self, count: usize, batch: u64) -> Result<(HwCounters, u64)> {
        let inputs = self.inputs;
        let mut group = CounterGroup::open()?;
        if self.reset.is_none() {
            group.enable()?;
            for s in 0..count {
                let input = &inputs[self.input_index(s)];
                for _ in 0..batch {
                    (self.call)(black_box(input));
                }
            }
            group.disable()?;
            Ok((group.read()?, count as u64 * batch))
        } else {
            for s in 0..count {
                let input = &inputs[self.input_index(s)];
                self.reset();
                group.enable()?;
                (self.call)(black_box(input));
                group.disable()?;
            }
            Ok((group.read()?, count as u64))
        }
    }
}

/// Raw output of the sampling loop.
struct Sampled {
    /// Per-call latency of each sample in picoseconds, in time order.
    per_call_ps: Vec<u64>,
    slowest: Slowest,
    taken: usize,
    allocs: AllocCount,
    max_allocs_per_call: u64,
}

/// The `k` slowest distinct inputs seen, as (per-call ps, input index).
struct Slowest {
    k: usize,
    entries: Vec<(u64, usize)>,
}

impl Slowest {
    fn new(k: usize) -> Self {
        Self {
            k,
            entries: Vec::with_capacity(k),
        }
    }

    fn offer(&mut self, ps: u64, index: usize) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.1 == index) {
            entry.0 = entry.0.max(ps);
        } else if self.entries.len() < self.k {
            self.entries.push((ps, index));
        } else if let Some(fastest) = self.entries.iter_mut().min_by_key(|e| e.0) {
            if ps > fastest.0 {
                *fastest = (ps, index);
            }
        }
    }

    /// Slowest first.
    fn into_sorted(mut self) -> Vec<(u64, usize)> {
        self.entries.sort_by_key(|&(ps, _)| std::cmp::Reverse(ps));
        self.entries
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    fn tiny() -> HarnessConfig {
        HarnessConfig {
            samples: 300,
            warmup: Duration::from_millis(1),
            target_sample: Duration::from_micros(2),
            time_limit: Duration::from_secs(10),
            slowest: 3,
            flamegraph_time: Duration::from_millis(10),
            cool_to_c: None,
        }
    }

    #[test]
    fn warm_mode_reports_a_consistent_distribution() {
        let inputs: Vec<u64> = (0..100).collect();
        let mut h = Harness::new("t", tiny());
        h.dist("sum", &inputs, |&n| (0..n).sum::<u64>());
        let reports = h.into_reports();
        let r = &reports[0];
        assert_eq!(r.name, "t/sum");
        assert_eq!(r.mode, Mode::Warm);
        assert_eq!(r.latency.samples, 300);
        assert_eq!(r.calls, 300 * r.batch);
        assert_eq!(r.distinct_inputs, 100);
        assert!(!r.hit_time_limit);
        let l = &r.latency;
        assert!(0.0 < l.p50_ns && l.p50_ns <= l.p99_ns && l.p99_ns <= l.max_ns);
        assert_eq!(r.slowest.len(), 3);
        assert!(r.slowest.windows(2).all(|w| w[0].ns >= w[1].ns));
        for slow in &r.slowest {
            let input: u64 = slow.input.parse().expect("Debug rendering of the input");
            assert!(input < 100);
        }
        assert_eq!(r.allocs.as_ref().map(|a| a.max_per_call), Some(0));
    }

    #[test]
    fn measured_latency_matches_a_known_duration() {
        let spin = |&micros: &u64| {
            let start = Instant::now();
            while start.elapsed() < Duration::from_micros(micros) {}
        };
        let mut h = Harness::new(
            "t",
            HarnessConfig {
                samples: 100,
                ..tiny()
            },
        );
        h.dist("spin_3us", &[3u64], spin);
        h.bench("spin_3us_cold").cold(|| {}).run(&[3u64], spin);
        for r in h.into_reports() {
            let l = &r.latency;
            assert!(
                l.p50_ns >= 3_000.0,
                "{}: p50 {} below the 3 µs floor",
                r.name,
                l.p50_ns
            );
            assert!(
                l.p50_ns < 30_000.0,
                "{}: p50 {} implausibly high",
                r.name,
                l.p50_ns
            );
            assert!(l.mean_ns >= 3_000.0 && l.max_ns >= l.p99_ns);
        }
    }

    #[test]
    fn an_appended_worst_case_set_still_moves_p99() {
        // The last 2% of inputs are slow. Visited in order they would all
        // land in the final round and the median over rounds would hide
        // them; the shuffled order spreads them over every round.
        let inputs: Vec<u64> = (0..10_000).collect();
        let mut h = Harness::new(
            "t",
            HarnessConfig {
                samples: 10_000,
                ..tiny()
            },
        );
        h.bench("tail").batch(1).run(&inputs, |&n| {
            if n >= 9_800 {
                let start = Instant::now();
                while start.elapsed() < Duration::from_micros(20) {}
            }
        });
        let latency = &h.into_reports()[0].latency;
        assert_eq!(latency.rounds, 10);
        assert!(latency.p50_ns < 5_000.0, "p50 {}", latency.p50_ns);
        assert!(
            latency.p99_ns >= 20_000.0,
            "p99 {} misses the slow inputs",
            latency.p99_ns
        );
    }

    #[test]
    fn cold_mode_resets_before_every_call_and_does_not_batch() {
        let fresh = Cell::new(false);
        let resets = Cell::new(0u64);
        let mut h = Harness::new("t", tiny());
        h.bench("cold")
            .cold(|| {
                fresh.set(true);
                resets.set(resets.get() + 1);
            })
            .run(&[1u8, 2, 3], |_| {
                assert!(fresh.replace(false), "called without a reset")
            });
        let r = &h.into_reports()[0];
        assert_eq!(r.mode, Mode::Cold);
        assert_eq!(r.batch, 1);
        assert_eq!(r.calls, 300);
        assert!(resets.get() >= 300);
    }

    #[test]
    fn allocations_are_attributed_per_call() {
        let inputs: Vec<u64> = (0..10).collect();
        let mut h = Harness::new("t", tiny());
        h.dist("boxed", &inputs, |&n| Box::new(n));
        let allocs = h.into_reports()[0]
            .allocs
            .clone()
            .expect("counting allocator installed");
        assert_eq!(allocs.max_per_call, 1);
        assert_eq!(allocs.mean_per_call, 1.0);
        assert_eq!(allocs.mean_bytes_per_call, 8.0);
    }

    #[test]
    fn sample_cap_and_time_limit_bound_the_run() {
        let mut h = Harness::new("t", tiny());
        h.bench("capped").samples(50).run(&[1u8], |&x| x);
        assert_eq!(h.into_reports()[0].latency.samples, 50);

        let mut h = Harness::new(
            "t",
            HarnessConfig {
                time_limit: Duration::ZERO,
                ..tiny()
            },
        );
        h.dist("limited", &[1u8], |&x| x);
        let r = &h.into_reports()[0];
        assert_eq!(r.latency.samples, 1);
        assert!(r.hit_time_limit);
    }

    #[test]
    fn flamegraph_mode_skips_benchmarks_it_does_not_target() {
        let calls = Cell::new(0);
        let mut h = Harness::for_flamegraphs("t", tiny(), BTreeMap::new());
        h.dist("skipped", &[1u8], |_| calls.set(calls.get() + 1));
        assert_eq!(calls.get(), 0);
        assert!(h.into_flamegraphs().is_empty());
    }

    #[test]
    fn only_runs_the_selected_benchmarks() {
        let calls = Cell::new(0);
        let mut h = Harness::new("t", tiny()).only(BTreeSet::from(["t/kept".to_owned()]));
        h.dist("skipped", &[1u8], |_| calls.set(calls.get() + 1));
        h.dist("kept", &[1u8], |&x| x);
        assert_eq!(calls.get(), 0);
        let names: Vec<String> = h.into_reports().into_iter().map(|r| r.name).collect();
        assert_eq!(names, ["t/kept"]);
    }

    #[test]
    #[should_panic(expected = "registered twice")]
    fn duplicate_benchmark_names_panic() {
        let mut h = Harness::new("t", tiny());
        h.dist("same", &[1u8], |&x| x);
        h.dist("same", &[1u8], |&x| x);
    }

    #[test]
    fn slowest_keeps_the_k_slowest_distinct_inputs() {
        let mut slowest = Slowest::new(2);
        slowest.offer(10, 0);
        slowest.offer(30, 1);
        slowest.offer(20, 0); // same input again: keeps its worst sample
        slowest.offer(5, 2); // faster than both kept: dropped
        slowest.offer(25, 3); // evicts input 0 (20)
        assert_eq!(slowest.into_sorted(), vec![(30, 1), (25, 3)]);
    }
}
