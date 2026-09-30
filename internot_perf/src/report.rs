//! What one benchmark produces: its latency distribution, the slowest
//! inputs, and allocation and hardware-counter figures. These structs are
//! the schema of the results and baseline JSON files.

use std::fmt;

use hdrhistogram::Histogram;
use serde::{Deserialize, Serialize};

use crate::counters::PerCallCounters;
use crate::table::{format_ns, format_opt, format_rate, Align, Table};

/// Histograms record per-call latency in picoseconds, so batched
/// nanosecond-scale measurements keep sub-nanosecond resolution.
pub(crate) const PS_PER_NS: f64 = 1_000.0;

/// Whether a benchmark measures memo hits or empty-cache calls (§16.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// A warm-up pass populates caches; calls are timed in batches.
    Warm,
    /// A reset closure empties caches before every call; calls are timed
    /// one at a time.
    Cold,
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Mode::Warm => "warm",
            Mode::Cold => "cold",
        })
    }
}

/// Most rounds a benchmark's samples are split into.
const MAX_ROUNDS: usize = 10;

/// Fewest samples per round; runs with fewer samples use fewer rounds.
const MIN_ROUND_SAMPLES: usize = 1_000;

/// Per-call latency distribution of one benchmark.
///
/// The samples are split, in time order, into up to 10 **rounds** of at
/// least 1,000 samples. Percentiles are the median over rounds of each
/// round's percentile, so a clock dip or interrupt burst lasting a few
/// milliseconds (common under the `powersave` governor) moves one round,
/// not the reported number. A slow path hit by some inputs still shows,
/// because the harness spreads inputs evenly across rounds. `mean_ns` and
/// `max_ns` use every sample.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LatencySummary {
    /// Timed samples (one input each, timed over `batch` calls).
    pub samples: u64,
    /// Rounds the samples were split into.
    pub rounds: u64,
    /// Mean per-call latency over all samples.
    pub mean_ns: f64,
    /// Median.
    pub p50_ns: f64,
    /// 90th percentile.
    pub p90_ns: f64,
    /// 99th percentile: the gated number.
    pub p99_ns: f64,
    /// 99.9th percentile.
    pub p999_ns: f64,
    /// Slowest sample.
    pub max_ns: f64,
    /// Single-thread calls per second at the mean latency.
    pub throughput_per_sec: f64,
    /// Each round's p99, in time order: the spread shows the run's noise.
    pub round_p99_ns: Vec<f64>,
}

impl LatencySummary {
    /// Summarises per-call latencies in picoseconds, in the order they were
    /// sampled. Percentiles carry HDR's 3-significant-figure precision
    /// (≤ 0.1% error, rounded up).
    pub fn from_samples(per_call_ps: &[u64]) -> Self {
        let n = per_call_ps.len();
        let rounds = (n / MIN_ROUND_SAMPLES).clamp(1, MAX_ROUNDS);
        let all = histogram_of(per_call_ps);
        let per_round: Vec<Histogram<u64>> = (0..rounds)
            .map(|r| histogram_of(&per_call_ps[r * n / rounds..(r + 1) * n / rounds]))
            .collect();
        let ns = |ps: u64| ps as f64 / PS_PER_NS;
        let round_values = |quantile: f64| -> Vec<f64> {
            per_round
                .iter()
                .map(|h| ns(h.value_at_quantile(quantile)))
                .collect()
        };
        // Upper median: with an even number of rounds, the slower middle one.
        let median = |mut values: Vec<f64>| {
            values.sort_by(f64::total_cmp);
            values[values.len() / 2]
        };
        let mean_ns = all.mean() / PS_PER_NS;
        Self {
            samples: n as u64,
            rounds: rounds as u64,
            mean_ns,
            p50_ns: median(round_values(0.50)),
            p90_ns: median(round_values(0.90)),
            p99_ns: median(round_values(0.99)),
            p999_ns: median(round_values(0.999)),
            max_ns: ns(all.max()),
            throughput_per_sec: if mean_ns > 0.0 { 1e9 / mean_ns } else { 0.0 },
            round_p99_ns: round_values(0.99),
        }
    }
}

fn histogram_of(values_ps: &[u64]) -> Histogram<u64> {
    let mut hist = Histogram::new(3).expect("3 significant figures is a valid HDR precision");
    for &v in values_ps {
        // `record` grows the auto-resizing histogram and fails only beyond
        // ~53 days; `saturating_record` would clamp without resizing.
        if hist.record(v).is_err() {
            hist.saturating_record(v);
        }
    }
    hist
}

/// One of the slowest inputs, kept so it can be replayed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlowInput {
    /// Per-call latency of this input's slowest sample.
    pub ns: f64,
    /// The input's `Debug` rendering.
    pub input: String,
}

/// Heap allocations per call, counted during the timed samples.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AllocSummary {
    /// The most allocations any one call made (per sample, rounded up, so
    /// a single allocation anywhere in a batch shows as 1). This is what
    /// `max_allocs` budgets gate.
    pub max_per_call: u64,
    /// Mean allocations per call.
    pub mean_per_call: f64,
    /// Mean bytes allocated per call.
    pub mean_bytes_per_call: f64,
}

/// Everything measured for one benchmark.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BenchReport {
    /// `<suite>/<benchmark>`, the key into budgets and baselines.
    pub name: String,
    /// Warm or cold.
    pub mode: Mode,
    /// Times the benchmark was measured. The gate re-measures benchmarks
    /// that fail on latency, to rule out machine noise, and keeps the
    /// fastest attempt (the one reported here).
    pub attempts: u32,
    /// Calls per timed sample (always 1 in cold mode).
    pub batch: u64,
    /// Total timed calls (`samples × batch`).
    pub calls: u64,
    /// Distinct inputs the samples covered.
    pub distinct_inputs: u64,
    /// Sampling stopped at the harness time limit before reaching the
    /// requested sample count.
    pub hit_time_limit: bool,
    /// The latency distribution.
    pub latency: LatencySummary,
    /// The slowest distinct inputs, slowest first.
    pub slowest: Vec<SlowInput>,
    /// Allocation figures; `None` when the counting allocator is not
    /// installed.
    pub allocs: Option<AllocSummary>,
    /// Hardware counters per call, from a separate untimed pass.
    pub counters: PerCallCounters,
}

/// Renders the measurement tables for a suite: latency and allocations,
/// then hardware counters (or the reason they are missing).
pub fn render_measurements(reports: &[BenchReport]) -> String {
    let mut latency = Table::new(&[
        ("benchmark", Align::Left),
        ("mode", Align::Left),
        ("samples", Align::Right),
        ("batch", Align::Right),
        ("p50", Align::Right),
        ("p90", Align::Right),
        ("p99", Align::Right),
        ("p99.9", Align::Right),
        ("max", Align::Right),
        ("mean", Align::Right),
        ("throughput", Align::Right),
        ("allocs/call", Align::Right),
    ]);
    for r in reports {
        let l = &r.latency;
        let samples = if r.hit_time_limit {
            format!("{} (time limit)", l.samples)
        } else {
            l.samples.to_string()
        };
        latency.row(vec![
            r.name.clone(),
            r.mode.to_string(),
            samples,
            r.batch.to_string(),
            format_ns(l.p50_ns),
            format_ns(l.p90_ns),
            format_ns(l.p99_ns),
            format_ns(l.p999_ns),
            format_ns(l.max_ns),
            format_ns(l.mean_ns),
            format_rate(l.throughput_per_sec),
            r.allocs
                .as_ref()
                .map_or_else(|| "n/a".to_owned(), |a| a.max_per_call.to_string()),
        ]);
    }
    let mut out = latency.render();

    let counted: Vec<&BenchReport> = reports
        .iter()
        .filter(|r| r.counters.cycles.is_some() || r.counters.instructions.is_some())
        .collect();
    if counted.is_empty() {
        let reason = reports
            .iter()
            .find_map(|r| r.counters.note.as_deref())
            .unwrap_or("no benchmarks");
        out.push_str(&format!("hardware counters unavailable: {reason}\n"));
        return out;
    }
    let mut counters = Table::new(&[
        ("benchmark (per call)", Align::Left),
        ("cycles", Align::Right),
        ("instructions", Align::Right),
        ("IPC", Align::Right),
        ("cache refs", Align::Right),
        ("cache misses", Align::Right),
        ("branch misses", Align::Right),
        ("note", Align::Left),
    ]);
    for r in counted {
        let c = &r.counters;
        counters.row(vec![
            r.name.clone(),
            format_opt(c.cycles, 1),
            format_opt(c.instructions, 1),
            format_opt(c.ipc, 2),
            format_opt(c.cache_references, 3),
            format_opt(c.cache_misses, 3),
            format_opt(c.branch_misses, 3),
            c.note.clone().unwrap_or_default(),
        ]);
    }
    out.push('\n');
    out.push_str(&counters.render());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(actual: f64, expected: f64) -> bool {
        (actual - expected).abs() <= expected * 0.002
    }

    /// `count` samples of `ns` nanoseconds each, in picoseconds.
    fn repeat(ns: u64, count: usize) -> Vec<u64> {
        vec![ns * 1_000; count]
    }

    #[test]
    fn summary_matches_a_known_distribution() {
        let samples: Vec<u64> = (1..=1000u64).map(|ns| ns * 1_000).collect();
        let s = LatencySummary::from_samples(&samples);
        assert_eq!(s.samples, 1000);
        assert_eq!(s.rounds, 1);
        assert!(close(s.mean_ns, 500.5), "mean {}", s.mean_ns);
        assert!(close(s.p50_ns, 500.0), "p50 {}", s.p50_ns);
        assert!(close(s.p90_ns, 900.0), "p90 {}", s.p90_ns);
        assert!(close(s.p99_ns, 990.0), "p99 {}", s.p99_ns);
        assert!(close(s.p999_ns, 999.0), "p99.9 {}", s.p999_ns);
        assert!(close(s.max_ns, 1000.0), "max {}", s.max_ns);
        assert!(close(s.throughput_per_sec, 1e9 / 500.5));
    }

    #[test]
    fn a_tail_in_every_round_moves_p99_but_not_p50() {
        // 10 rounds, each 98% at 10 ns and 2% at 5 µs.
        let round = [repeat(10, 980), repeat(5_000, 20)].concat();
        let samples = round.repeat(10);
        let s = LatencySummary::from_samples(&samples);
        assert_eq!(s.rounds, 10);
        assert!(close(s.p50_ns, 10.0));
        assert!(close(s.p99_ns, 5_000.0), "p99 {}", s.p99_ns);
    }

    #[test]
    fn a_disturbed_round_does_not_move_the_percentiles() {
        // Round 3 of 10 ran 25% slow throughout (a clock dip): pooled, that
        // would put p99 at 12.5 ns.
        let mut samples = repeat(10, 10_000);
        samples[3_000..4_000].fill(12_500);
        let s = LatencySummary::from_samples(&samples);
        assert_eq!(s.rounds, 10);
        assert!(close(s.p99_ns, 10.0), "p99 {}", s.p99_ns);
        assert!(close(s.round_p99_ns[3], 12.5));
        assert!(close(s.max_ns, 12.5), "max still sees every sample");
        assert!(close(s.mean_ns, 10.25), "mean still sees every sample");
    }

    #[test]
    fn rounds_scale_with_samples() {
        assert_eq!(LatencySummary::from_samples(&repeat(1, 1_999)).rounds, 1);
        assert_eq!(LatencySummary::from_samples(&repeat(1, 5_000)).rounds, 5);
        assert_eq!(LatencySummary::from_samples(&repeat(1, 100_000)).rounds, 10);
    }

    #[test]
    fn sub_nanosecond_resolution_survives() {
        let s = LatencySummary::from_samples(&[2_345]); // 2.345 ns
        assert!(close(s.p99_ns, 2.345), "p99 {}", s.p99_ns);
    }

    #[test]
    fn no_samples_summarise_to_zeros() {
        let s = LatencySummary::from_samples(&[]);
        assert_eq!(s.samples, 0);
        assert_eq!(s.p99_ns, 0.0);
        assert_eq!(s.throughput_per_sec, 0.0);
    }
}
