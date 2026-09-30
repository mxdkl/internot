//! `perf-gate`: runs the registered suites, writes results, and gates them
//! against `perf/budgets.toml` and this machine's committed baselines.
//!
//! Exit status: 0 when the gate passes, 1 when it fails, 2 on a usage or
//! I/O error. See `perf/README.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::ExitCode;

use internot_perf::alloc::CountingAllocator;
use internot_perf::budget::Budgets;
use internot_perf::gate;
use internot_perf::meta::RunInfo;
use internot_perf::report::render_measurements;
use internot_perf::results::{workspace_root, PerfDir, SuiteResults};
use internot_perf::suites::{self, Suite};
use internot_perf::{format_ns, Error, Harness, HarnessConfig, Result};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Measurements a benchmark gets before a latency failure counts.
const MAX_ATTEMPTS: u32 = 3;

const USAGE: &str = "\
Usage: perf-gate [OPTIONS]

Runs the benchmark suites, writes perf/results/latest/<suite>.json, and
checks every benchmark against perf/budgets.toml and this machine's
baseline in perf/baselines/<machine>/.

Options:
  --suite <name>          Run only this suite (repeatable). Default: all.
  --quick                 A tenth of the samples: a smoke run, not a gate run.
  --record-baseline       Save this run as the machine's baseline. Refused for
                          --quick runs and when any budget fails.
  --flamegraph <bench>    Write perf/flamegraphs/<bench>.svg (repeatable).
  --flamegraphs-top <n>   Flamegraphs for the n benchmarks with the worst p99.
  --list                  List suites and exit.
  -h, --help              Show this help.";

#[derive(Debug, Default)]
struct Args {
    suites: Vec<String>,
    quick: bool,
    record_baseline: bool,
    flamegraphs: Vec<String>,
    flamegraphs_top: usize,
    list: bool,
    help: bool,
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Args> {
    let mut parsed = Args::default();
    while let Some(arg) = args.next() {
        let mut value = |flag: &str| {
            args.next()
                .ok_or_else(|| Error::new(format!("{flag} needs a value")))
        };
        match arg.as_str() {
            "--suite" => parsed.suites.push(value("--suite")?),
            "--quick" => parsed.quick = true,
            "--record-baseline" => parsed.record_baseline = true,
            "--flamegraph" => parsed.flamegraphs.push(value("--flamegraph")?),
            "--flamegraphs-top" => {
                let n = value("--flamegraphs-top")?;
                parsed.flamegraphs_top = n.parse().map_err(|_| {
                    Error::new(format!("--flamegraphs-top needs a count, got {n:?}"))
                })?;
            }
            "--list" => parsed.list = true,
            "-h" | "--help" => parsed.help = true,
            other => return Err(Error::new(format!("unknown argument {other:?}\n\n{USAGE}"))),
        }
    }
    if parsed.record_baseline && parsed.quick {
        return Err(Error::new(
            "--record-baseline needs a full run: baselines from --quick runs are too noisy",
        ));
    }
    Ok(parsed)
}

fn main() -> ExitCode {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("perf-gate: {e}");
            return ExitCode::from(2);
        }
    };
    match run(&args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("perf-gate: {e}");
            ExitCode::from(2)
        }
    }
}

/// Runs the gate; `Ok(false)` means it ran and failed.
fn run(args: &Args) -> Result<bool> {
    if args.help {
        println!("{USAGE}");
        return Ok(true);
    }
    if args.list {
        for suite in suites::ALL {
            println!("{:<16} {}", suite.name, suite.about);
        }
        return Ok(true);
    }
    if cfg!(debug_assertions) {
        return Err(Error::new(
            "refusing to measure a debug build: use `cargo run --release` (or --profile profiling)",
        ));
    }

    let perf = PerfDir::in_workspace();
    let budgets = Budgets::load(&perf.budgets())?;
    let selected = select_suites(&args.suites)?;
    let config = if args.quick {
        HarnessConfig::quick()
    } else {
        HarnessConfig::full()
    };
    let run_info = RunInfo::collect(workspace_root(), args.quick);
    print_header(&run_info);

    // 1. Measure.
    let mut latest = Vec::new();
    for suite in &selected {
        println!("\n== {} ({}) ==", suite.name, suite.about);
        let mut harness = Harness::new(suite.name, config.clone());
        (suite.run)(&mut harness);
        let results = SuiteResults {
            suite: suite.name.to_owned(),
            run: run_info.clone(),
            benchmarks: harness.into_reports(),
        };
        print!("{}", render_measurements(&results.benchmarks));
        latest.push(results);
    }

    // 2. Flamegraphs, in a second pass so profiling never perturbs timings.
    let targets = flamegraph_targets(args, &latest, &perf)?;
    if !targets.is_empty() {
        write_flamegraphs(&selected, &targets, &config)?;
    }

    // 3. Gate. A latency failure is re-measured before it counts: a real
    //    regression reproduces, a clock dip or a background job does not.
    let slug = &run_info.machine.slug;
    let baselines = load_baselines(&latest, &perf, slug, args.record_baseline)?;
    let mut report = evaluate(&budgets, &latest, &baselines);
    for attempt in 2..=MAX_ATTEMPTS {
        let suspects: BTreeSet<String> = report
            .rows
            .iter()
            .filter(|r| r.failed_on_latency_only())
            .map(|r| r.name.clone())
            .collect();
        if suspects.is_empty() {
            break;
        }
        println!(
            "\nre-measuring {} benchmark(s) that failed on latency (attempt {attempt} of {MAX_ATTEMPTS})",
            suspects.len()
        );
        remeasure(&selected, &suspects, &config, &mut latest, attempt);
        report = evaluate(&budgets, &latest, &baselines);
    }

    println!();
    for results in &latest {
        let path = perf.latest(&results.suite);
        results.write(&path)?;
        println!("results and slowest inputs: {}", path.display());
    }
    println!(
        "\n== gate (p99 regression limit {:.0}%) ==",
        report.max_regression * 100.0
    );
    print!("{report}");

    if args.record_baseline {
        return record_baselines(&report, &latest, &perf, slug);
    }
    if report.passed() {
        println!("\nPASS: {} benchmarks within budget", report.rows.len());
        Ok(true)
    } else {
        println!(
            "\nFAIL: {} of {} benchmarks",
            report.failed_rows(),
            report.rows.len()
        );
        Ok(false)
    }
}

/// Each suite's committed baseline for this machine, if any.
fn load_baselines(
    latest: &[SuiteResults],
    perf: &PerfDir,
    slug: &str,
    replacing: bool,
) -> Result<Vec<Option<SuiteResults>>> {
    latest
        .iter()
        .map(
            |results| match SuiteResults::read(&perf.baseline(slug, &results.suite)) {
                Ok(baseline) => Ok(baseline),
                // Recording replaces an unreadable (e.g. older-format) baseline.
                Err(e) if replacing => {
                    println!("warning: {e}; it will be replaced");
                    Ok(None)
                }
                Err(e) => Err(Error::new(format!(
                    "{e} (re-record it with --record-baseline)"
                ))),
            },
        )
        .collect()
}

fn evaluate(
    budgets: &Budgets,
    latest: &[SuiteResults],
    baselines: &[Option<SuiteResults>],
) -> gate::GateReport {
    let pairs: Vec<(&SuiteResults, Option<&SuiteResults>)> = latest
        .iter()
        .zip(baselines.iter().map(Option::as_ref))
        .collect();
    gate::evaluate(budgets, &pairs)
}

/// Measures the named benchmarks again and keeps, for each, the attempt
/// with the lower p99. `latest` is in the same order as `selected`.
fn remeasure(
    selected: &[&'static Suite],
    names: &BTreeSet<String>,
    config: &HarnessConfig,
    latest: &mut [SuiteResults],
    attempt: u32,
) {
    for (suite, results) in selected.iter().zip(latest.iter_mut()) {
        let mine: BTreeSet<String> = names
            .iter()
            .filter(|name| results.bench(name).is_some())
            .cloned()
            .collect();
        if mine.is_empty() {
            continue;
        }
        let mut harness = Harness::new(suite.name, config.clone()).only(mine);
        (suite.run)(&mut harness);
        for fresh in harness.into_reports() {
            let Some(kept) = results.benchmarks.iter_mut().find(|b| b.name == fresh.name) else {
                continue;
            };
            println!(
                "  {}: p99 {} -> {}",
                fresh.name,
                format_ns(kept.latency.p99_ns),
                format_ns(fresh.latency.p99_ns)
            );
            if fresh.latency.p99_ns < kept.latency.p99_ns {
                *kept = fresh;
            }
            kept.attempts = attempt;
        }
    }
}

fn select_suites(names: &[String]) -> Result<Vec<&'static Suite>> {
    if names.is_empty() {
        return Ok(suites::ALL.iter().collect());
    }
    names
        .iter()
        .map(|name| {
            suites::find(name).ok_or_else(|| {
                let known: Vec<&str> = suites::ALL.iter().map(|s| s.name).collect();
                Error::new(format!(
                    "unknown suite {name:?}; known: {}",
                    known.join(", ")
                ))
            })
        })
        .collect()
}

fn print_header(run: &RunInfo) {
    let m = &run.machine;
    let governor = m.governor.as_deref().unwrap_or("unknown");
    println!(
        "perf-gate{}: {} ({} threads), governor {governor}, {}, commit {}",
        if run.quick { " --quick" } else { "" },
        m.cpu_model,
        m.logical_cores,
        run.rustc,
        run.git_commit.as_deref().unwrap_or("unknown"),
    );
    match &run.hw_counters_note {
        None => println!("hardware counters: available"),
        Some(note) => println!("hardware counters: unavailable ({note})"),
    }
    if !run.alloc_counting {
        println!("allocation counting: unavailable (counting allocator not installed)");
    }
    if governor != "performance" {
        println!(
            "warning: CPU governor is {governor:?}, so numbers are noisy. Before recording a \
             baseline run `sudo cpupower frequency-set -g performance`."
        );
    }
}

/// Benchmarks to profile: those named with `--flamegraph`, plus the
/// `--flamegraphs-top` worst by p99. Maps full name to SVG path.
fn flamegraph_targets(
    args: &Args,
    latest: &[SuiteResults],
    perf: &PerfDir,
) -> Result<BTreeMap<String, PathBuf>> {
    let mut measured: Vec<(&str, f64)> = latest
        .iter()
        .flat_map(|s| &s.benchmarks)
        .map(|b| (b.name.as_str(), b.latency.p99_ns))
        .collect();
    let mut targets = BTreeMap::new();
    for name in &args.flamegraphs {
        if !measured.iter().any(|(n, _)| n == name) {
            let known: Vec<&str> = measured.iter().map(|(n, _)| *n).collect();
            return Err(Error::new(format!(
                "--flamegraph {name:?} matches no benchmark that ran; known: {}",
                known.join(", ")
            )));
        }
        targets.insert(name.clone(), perf.flamegraph(name));
    }
    measured.sort_by(|a, b| b.1.total_cmp(&a.1));
    for (name, _) in measured.into_iter().take(args.flamegraphs_top) {
        targets.insert(name.to_owned(), perf.flamegraph(name));
    }
    Ok(targets)
}

fn write_flamegraphs(
    selected: &[&'static Suite],
    targets: &BTreeMap<String, PathBuf>,
    config: &HarnessConfig,
) -> Result<()> {
    println!("\n== flamegraphs ({:?} each) ==", config.flamegraph_time);
    for suite in selected {
        let prefix = format!("{}/", suite.name);
        let mine: BTreeMap<String, PathBuf> = targets
            .iter()
            .filter(|(name, _)| name.starts_with(&prefix))
            .map(|(name, path)| (name.clone(), path.clone()))
            .collect();
        if mine.is_empty() {
            continue;
        }
        let mut harness = Harness::for_flamegraphs(suite.name, config.clone(), mine);
        (suite.run)(&mut harness);
        for (name, outcome) in harness.into_flamegraphs() {
            match outcome {
                Ok(path) => println!("{name}: {}", path.display()),
                Err(e) => return Err(Error::new(format!("flamegraph for {name}: {e}"))),
            }
        }
    }
    Ok(())
}

/// Saves the run as the baseline unless a budget failed. Regressions
/// against the old baseline do not block: accepting them is the point.
fn record_baselines(
    report: &gate::GateReport,
    latest: &[SuiteResults],
    perf: &PerfDir,
    slug: &str,
) -> Result<bool> {
    let blocking = report.budget_failures();
    if blocking > 0 {
        println!(
            "\nnot recording a baseline: {blocking} budget failure(s). Fix them first; a budget \
             is never raised by recording over it."
        );
        return Ok(false);
    }
    for results in latest {
        let path = perf.baseline(slug, &results.suite);
        results.write(&path)?;
        println!("baseline recorded: {}", path.display());
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Args> {
        parse_args(args.iter().map(|a| a.to_string()))
    }

    #[test]
    fn parses_every_flag() {
        let args = parse(&[
            "--suite",
            "core_hash",
            "--flamegraph",
            "core_hash/hash_int",
            "--flamegraphs-top",
            "3",
            "--record-baseline",
        ])
        .unwrap();
        assert_eq!(args.suites, ["core_hash"]);
        assert_eq!(args.flamegraphs, ["core_hash/hash_int"]);
        assert_eq!(args.flamegraphs_top, 3);
        assert!(args.record_baseline);
        assert!(!args.quick);
    }

    #[test]
    fn rejects_bad_arguments() {
        assert!(parse(&["--suite"]).is_err());
        assert!(parse(&["--flamegraphs-top", "many"]).is_err());
        assert!(parse(&["--bogus"]).is_err());
        assert!(parse(&["--quick", "--record-baseline"]).is_err());
    }
}
