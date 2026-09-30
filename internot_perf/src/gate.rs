//! The gate: compares a run to its budgets and to the committed baseline.
//!
//! A benchmark fails when it
//! - has no budget (every measured number is a gate, §16);
//! - was measured in a different mode than its budget names;
//! - exceeds its `p99_ns`, `p50_ns` or `max_allocs` budget;
//! - regresses p99 by more than `max_regression` against the baseline.
//!
//! A budget that names a benchmark of a suite that ran, but which that
//! suite no longer measures, also fails: renaming a benchmark must not
//! quietly drop its gate. A missing baseline is not a failure.

use std::fmt;

use crate::budget::{Budget, Budgets};
use crate::report::{BenchReport, Mode};
use crate::results::SuiteResults;
use crate::table::{format_ns, Align, Table};

/// Why a benchmark failed the gate.
#[derive(Clone, Debug, PartialEq)]
pub enum Failure {
    /// No entry in `budgets.toml`.
    NoBudget,
    /// A budget exists for a benchmark its suite no longer measures.
    NotMeasured,
    /// The budget is for one mode; the benchmark ran in the other.
    ModeMismatch {
        /// Mode the budget names.
        budget: Mode,
        /// Mode the benchmark ran in.
        measured: Mode,
    },
    /// A latency percentile is over budget.
    OverBudget {
        /// `"p50"` or `"p99"`.
        metric: &'static str,
        /// Measured value.
        measured_ns: f64,
        /// Budgeted value.
        budget_ns: f64,
    },
    /// A call made more heap allocations than allowed.
    AllocsOverBudget {
        /// Most allocations in one call.
        measured: u64,
        /// Allowed allocations per call.
        budget: u64,
    },
    /// `max_allocs` is set but allocations were not counted.
    AllocsNotCounted,
    /// p99 is more than the threshold above the baseline's.
    Regression {
        /// Fractional increase over the baseline p99.
        delta: f64,
        /// The configured threshold.
        threshold: f64,
    },
}

impl Failure {
    /// Whether this is a baseline regression (as opposed to a budget
    /// failure). Only regressions may be accepted by recording a new
    /// baseline.
    pub fn is_regression(&self) -> bool {
        matches!(self, Failure::Regression { .. })
    }

    /// Whether this is about latency, which machine noise can cause, as
    /// opposed to a structural problem (missing budget, wrong mode,
    /// allocations) that re-measuring cannot change.
    pub fn is_latency(&self) -> bool {
        matches!(
            self,
            Failure::OverBudget { .. } | Failure::Regression { .. }
        )
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Failure::NoBudget => write!(f, "no budget in perf/budgets.toml"),
            Failure::NotMeasured => write!(f, "budgeted but not measured (renamed or removed?)"),
            Failure::ModeMismatch { budget, measured } => {
                write!(f, "budget is for {budget} runs, measured {measured}")
            }
            Failure::OverBudget {
                metric,
                measured_ns,
                budget_ns,
            } => write!(
                f,
                "{metric} {} > budget {}",
                format_ns(*measured_ns),
                format_ns(*budget_ns)
            ),
            Failure::AllocsOverBudget { measured, budget } => {
                write!(f, "{measured} allocs/call > budget {budget}")
            }
            Failure::AllocsNotCounted => {
                write!(
                    f,
                    "max_allocs set but the counting allocator is not installed"
                )
            }
            Failure::Regression { delta, threshold } => write!(
                f,
                "p99 regressed {:+.1}% vs baseline (limit {:.0}%)",
                delta * 100.0,
                threshold * 100.0
            ),
        }
    }
}

/// The gate's verdict on one benchmark.
#[derive(Clone, Debug, PartialEq)]
pub struct GateRow {
    /// Full benchmark name.
    pub name: String,
    /// Measured median, if the benchmark ran.
    pub p50_ns: Option<f64>,
    /// Measured p99, if the benchmark ran.
    pub p99_ns: Option<f64>,
    /// Budgeted p99, if budgeted.
    pub budget_p99_ns: Option<f64>,
    /// Baseline p99, if a baseline exists for this benchmark.
    pub baseline_p99_ns: Option<f64>,
    /// Fractional p99 change against the baseline.
    pub delta: Option<f64>,
    /// Everything wrong with it; empty means it passed.
    pub failures: Vec<Failure>,
}

impl GateRow {
    /// Failed, and only on latency: worth re-measuring before believing.
    pub fn failed_on_latency_only(&self) -> bool {
        !self.failures.is_empty() && self.failures.iter().all(Failure::is_latency)
    }

    fn status(&self, max_regression: f64) -> String {
        if !self.failures.is_empty() {
            let reasons: Vec<String> = self.failures.iter().map(ToString::to_string).collect();
            return format!("FAIL: {}", reasons.join("; "));
        }
        match self.delta {
            Some(delta) if delta < -max_regression => {
                "ok (improved: consider --record-baseline)".to_owned()
            }
            _ => "ok".to_owned(),
        }
    }
}

/// The gate's verdict on a run.
#[derive(Clone, Debug, PartialEq)]
pub struct GateReport {
    /// One row per benchmark, plus one per budget left unmeasured.
    pub rows: Vec<GateRow>,
    /// The regression threshold applied.
    pub max_regression: f64,
}

impl GateReport {
    /// True when no benchmark failed.
    pub fn passed(&self) -> bool {
        self.rows.iter().all(|r| r.failures.is_empty())
    }

    /// Number of benchmarks that failed for any reason.
    pub fn failed_rows(&self) -> usize {
        self.rows.iter().filter(|r| !r.failures.is_empty()).count()
    }

    /// Number of failures other than baseline regressions. These block
    /// recording a baseline: a baseline must never paper over a budget.
    pub fn budget_failures(&self) -> usize {
        self.rows
            .iter()
            .flat_map(|r| &r.failures)
            .filter(|f| !f.is_regression())
            .count()
    }
}

impl fmt::Display for GateReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut table = Table::new(&[
            ("benchmark", Align::Left),
            ("p50", Align::Right),
            ("p99", Align::Right),
            ("p99 budget", Align::Right),
            ("baseline p99", Align::Right),
            ("delta", Align::Right),
            ("status", Align::Left),
        ]);
        let ns = |v: Option<f64>| v.map_or_else(|| "-".to_owned(), format_ns);
        for row in &self.rows {
            table.row(vec![
                row.name.clone(),
                ns(row.p50_ns),
                ns(row.p99_ns),
                ns(row.budget_p99_ns),
                row.baseline_p99_ns
                    .map_or_else(|| "no baseline".to_owned(), format_ns),
                row.delta
                    .map_or_else(|| "-".to_owned(), |d| format!("{:+.1}%", d * 100.0)),
                row.status(self.max_regression),
            ]);
        }
        f.write_str(&table.render())
    }
}

/// Checks each suite's latest results against the budgets and, where one
/// exists, the suite's baseline.
pub fn evaluate(
    budgets: &Budgets,
    suites: &[(&SuiteResults, Option<&SuiteResults>)],
) -> GateReport {
    let mut rows = Vec::new();
    for (latest, baseline) in suites {
        for bench in &latest.benchmarks {
            let base = baseline.and_then(|b| b.bench(&bench.name));
            rows.push(check(
                bench,
                budgets.get(&bench.name),
                base,
                budgets.max_regression,
            ));
        }
        let prefix = format!("{}/", latest.suite);
        for (name, budget) in &budgets.benches {
            if name.starts_with(&prefix) && latest.bench(name).is_none() {
                rows.push(GateRow {
                    name: name.clone(),
                    p50_ns: None,
                    p99_ns: None,
                    budget_p99_ns: Some(budget.p99_ns),
                    baseline_p99_ns: None,
                    delta: None,
                    failures: vec![Failure::NotMeasured],
                });
            }
        }
    }
    GateReport {
        rows,
        max_regression: budgets.max_regression,
    }
}

fn check(
    bench: &BenchReport,
    budget: Option<&Budget>,
    baseline: Option<&BenchReport>,
    max_regression: f64,
) -> GateRow {
    let p50 = bench.latency.p50_ns;
    let p99 = bench.latency.p99_ns;
    let mut failures = Vec::new();

    match budget {
        None => failures.push(Failure::NoBudget),
        Some(budget) => {
            if budget.mode != bench.mode {
                failures.push(Failure::ModeMismatch {
                    budget: budget.mode,
                    measured: bench.mode,
                });
            }
            if p99 > budget.p99_ns {
                failures.push(Failure::OverBudget {
                    metric: "p99",
                    measured_ns: p99,
                    budget_ns: budget.p99_ns,
                });
            }
            if let Some(p50_budget) = budget.p50_ns.filter(|&b| p50 > b) {
                failures.push(Failure::OverBudget {
                    metric: "p50",
                    measured_ns: p50,
                    budget_ns: p50_budget,
                });
            }
            if let Some(max_allocs) = budget.max_allocs {
                match &bench.allocs {
                    None => failures.push(Failure::AllocsNotCounted),
                    Some(a) if a.max_per_call > max_allocs => {
                        failures.push(Failure::AllocsOverBudget {
                            measured: a.max_per_call,
                            budget: max_allocs,
                        })
                    }
                    Some(_) => {}
                }
            }
        }
    }

    let baseline_p99 = baseline.map(|b| b.latency.p99_ns).filter(|&b| b > 0.0);
    let delta = baseline_p99.map(|b| p99 / b - 1.0);
    if let Some(delta) = delta.filter(|&d| d > max_regression) {
        failures.push(Failure::Regression {
            delta,
            threshold: max_regression,
        });
    }

    GateRow {
        name: bench.name.clone(),
        p50_ns: Some(p50),
        p99_ns: Some(p99),
        budget_p99_ns: budget.map(|b| b.p99_ns),
        baseline_p99_ns: baseline_p99,
        delta,
        failures,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::DEFAULT_MAX_REGRESSION;
    use crate::counters::PerCallCounters;
    use crate::meta::{MachineInfo, RunInfo};
    use crate::report::{AllocSummary, LatencySummary};

    fn bench(name: &str, p50_ns: f64, p99_ns: f64) -> BenchReport {
        BenchReport {
            name: name.to_owned(),
            mode: Mode::Warm,
            attempts: 1,
            batch: 1,
            calls: 1_000,
            distinct_inputs: 1_000,
            hit_time_limit: false,
            latency: LatencySummary {
                samples: 1_000,
                rounds: 1,
                mean_ns: p50_ns,
                p50_ns,
                p90_ns: p99_ns,
                p99_ns,
                p999_ns: p99_ns,
                max_ns: p99_ns,
                throughput_per_sec: 1e9 / p50_ns,
                round_p99_ns: vec![p99_ns],
            },
            slowest: Vec::new(),
            allocs: Some(AllocSummary {
                max_per_call: 0,
                mean_per_call: 0.0,
                mean_bytes_per_call: 0.0,
            }),
            counters: PerCallCounters::default(),
        }
    }

    fn suite(benchmarks: Vec<BenchReport>) -> SuiteResults {
        SuiteResults {
            suite: "s".to_owned(),
            run: RunInfo {
                machine: MachineInfo {
                    cpu_model: "test cpu".to_owned(),
                    logical_cores: 1,
                    governor: None,
                    slug: "test-cpu-1t".to_owned(),
                },
                rustc: "rustc test".to_owned(),
                build_profile: "release".to_owned(),
                git_commit: None,
                timestamp: "2026-09-29T00:00:00Z".to_owned(),
                quick: false,
                hw_counters_available: false,
                hw_counters_note: None,
                alloc_counting: true,
            },
            benchmarks,
        }
    }

    fn budgets(toml: &str) -> Budgets {
        Budgets::parse(toml).unwrap()
    }

    const ONE_BUDGET: &str = "[budget.\"s/a\"]\nmode = \"warm\"\np99_ns = 100\n";

    #[test]
    fn within_budget_without_baseline_passes_and_says_so() {
        let latest = suite(vec![bench("s/a", 10.0, 50.0)]);
        let report = evaluate(&budgets(ONE_BUDGET), &[(&latest, None)]);
        assert!(report.passed());
        assert_eq!(report.rows[0].baseline_p99_ns, None);
        assert_eq!(report.rows[0].delta, None);
        assert!(report.to_string().contains("no baseline"));
    }

    #[test]
    fn exceeding_the_p99_budget_fails() {
        let latest = suite(vec![bench("s/a", 10.0, 150.0)]);
        let report = evaluate(&budgets(ONE_BUDGET), &[(&latest, None)]);
        assert!(!report.passed());
        assert_eq!(
            report.rows[0].failures,
            vec![Failure::OverBudget {
                metric: "p99",
                measured_ns: 150.0,
                budget_ns: 100.0
            }]
        );
        assert_eq!(report.budget_failures(), 1);
    }

    #[test]
    fn exceeding_p50_and_alloc_budgets_fails() {
        let mut allocating = bench("s/a", 30.0, 50.0);
        allocating.allocs.as_mut().unwrap().max_per_call = 2;
        let latest = suite(vec![allocating]);
        let strict =
            "[budget.\"s/a\"]\nmode = \"warm\"\np99_ns = 100\np50_ns = 20\nmax_allocs = 0\n";
        let report = evaluate(&budgets(strict), &[(&latest, None)]);
        assert_eq!(
            report.rows[0].failures,
            vec![
                Failure::OverBudget {
                    metric: "p50",
                    measured_ns: 30.0,
                    budget_ns: 20.0
                },
                Failure::AllocsOverBudget {
                    measured: 2,
                    budget: 0
                },
            ]
        );
    }

    #[test]
    fn alloc_budget_without_counting_fails() {
        let mut uncounted = bench("s/a", 10.0, 50.0);
        uncounted.allocs = None;
        let latest = suite(vec![uncounted]);
        let with_allocs = "[budget.\"s/a\"]\nmode = \"warm\"\np99_ns = 100\nmax_allocs = 0\n";
        let report = evaluate(&budgets(with_allocs), &[(&latest, None)]);
        assert_eq!(report.rows[0].failures, vec![Failure::AllocsNotCounted]);
    }

    #[test]
    fn regression_over_threshold_fails_but_under_passes() {
        let baseline = suite(vec![bench("s/a", 10.0, 50.0)]);
        let regressed = suite(vec![bench("s/a", 10.0, 57.5)]); // +15%
        let report = evaluate(&budgets(ONE_BUDGET), &[(&regressed, Some(&baseline))]);
        assert!(!report.passed());
        match &report.rows[0].failures[..] {
            [Failure::Regression { delta, threshold }] => {
                assert!((delta - 0.15).abs() < 1e-9);
                assert_eq!(*threshold, DEFAULT_MAX_REGRESSION);
            }
            other => panic!("expected one regression, got {other:?}"),
        }
        assert_eq!(
            report.budget_failures(),
            0,
            "a regression is not a budget failure"
        );
        assert!(report.rows[0].failed_on_latency_only());

        let noisy = suite(vec![bench("s/a", 10.0, 54.0)]); // +8%
        let report = evaluate(&budgets(ONE_BUDGET), &[(&noisy, Some(&baseline))]);
        assert!(report.passed());
        assert!((report.rows[0].delta.unwrap() - 0.08).abs() < 1e-9);
    }

    #[test]
    fn improvement_passes_and_suggests_a_new_baseline() {
        let baseline = suite(vec![bench("s/a", 10.0, 50.0)]);
        let faster = suite(vec![bench("s/a", 8.0, 40.0)]); // -20%
        let report = evaluate(&budgets(ONE_BUDGET), &[(&faster, Some(&baseline))]);
        assert!(report.passed());
        assert!((report.rows[0].delta.unwrap() + 0.20).abs() < 1e-9);
        assert!(report.to_string().contains("improved"));
    }

    #[test]
    fn a_benchmark_without_a_budget_fails() {
        let latest = suite(vec![bench("s/a", 10.0, 50.0), bench("s/new", 1.0, 2.0)]);
        let report = evaluate(&budgets(ONE_BUDGET), &[(&latest, None)]);
        assert_eq!(report.rows[1].failures, vec![Failure::NoBudget]);
        assert_eq!(report.failed_rows(), 1);
        assert!(
            !report.rows[1].failed_on_latency_only(),
            "re-measuring cannot add a budget"
        );
    }

    #[test]
    fn a_mode_mismatch_fails() {
        let latest = suite(vec![bench("s/a", 10.0, 50.0)]);
        let cold = "[budget.\"s/a\"]\nmode = \"cold\"\np99_ns = 100\n";
        let report = evaluate(&budgets(cold), &[(&latest, None)]);
        assert_eq!(
            report.rows[0].failures,
            vec![Failure::ModeMismatch {
                budget: Mode::Cold,
                measured: Mode::Warm
            }]
        );
    }

    #[test]
    fn a_budget_its_suite_no_longer_measures_fails() {
        let latest = suite(vec![bench("s/a", 10.0, 50.0)]);
        let stale = format!(
            "{ONE_BUDGET}[budget.\"s/renamed\"]\nmode = \"warm\"\np99_ns = 1\n\
             [budget.\"other/x\"]\nmode = \"warm\"\np99_ns = 1\n"
        );
        let report = evaluate(&budgets(&stale), &[(&latest, None)]);
        let names: Vec<&str> = report.rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            names,
            ["s/a", "s/renamed"],
            "budgets of suites that did not run are ignored"
        );
        assert_eq!(report.rows[1].failures, vec![Failure::NotMeasured]);
    }
}
