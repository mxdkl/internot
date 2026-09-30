//! Budgets: the §16.2 numbers every benchmark is gated against, loaded
//! from `perf/budgets.toml`.
//!
//! ```toml
//! [gate]
//! max_regression = 0.10        # optional; fail when p99 exceeds baseline by >10%
//!
//! [budget."core_hash/hash_int"]
//! mode = "warm"                # required: "warm" or "cold", must match the benchmark
//! p99_ns = 50                  # required: maximum p99
//! p50_ns = 20                  # optional: maximum p50
//! max_allocs = 0               # optional: maximum heap allocations in any one call
//! ```
//!
//! Unknown keys are errors, so a typo cannot silently disable a gate.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::report::Mode;

/// The p99 regression tolerated against a baseline when `budgets.toml`
/// does not set `[gate] max_regression`.
pub const DEFAULT_MAX_REGRESSION: f64 = 0.10;

/// Limits for one benchmark.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    /// The mode the benchmark must be measured in.
    pub mode: Mode,
    /// Maximum p99 latency.
    pub p99_ns: f64,
    /// Maximum median latency.
    #[serde(default)]
    pub p50_ns: Option<f64>,
    /// Maximum heap allocations in any single call.
    #[serde(default)]
    pub max_allocs: Option<u64>,
}

/// Every budget, plus the regression threshold.
#[derive(Clone, Debug, PartialEq)]
pub struct Budgets {
    /// Fractional p99 increase over the baseline that fails the gate.
    pub max_regression: f64,
    /// Budgets by full benchmark name (`<suite>/<bench>`).
    pub benches: BTreeMap<String, Budget>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BudgetsFile {
    #[serde(default)]
    gate: GateSettings,
    #[serde(default)]
    budget: BTreeMap<String, Budget>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GateSettings {
    #[serde(default = "default_max_regression")]
    max_regression: f64,
}

impl Default for GateSettings {
    fn default() -> Self {
        Self {
            max_regression: DEFAULT_MAX_REGRESSION,
        }
    }
}

fn default_max_regression() -> f64 {
    DEFAULT_MAX_REGRESSION
}

impl Budgets {
    /// Parses and validates `budgets.toml` contents.
    pub fn parse(source: &str) -> Result<Self> {
        let file: BudgetsFile =
            toml::from_str(source).map_err(|e| Error::new(format!("invalid budgets: {e}")))?;
        let max_regression = file.gate.max_regression;
        if !(max_regression.is_finite() && max_regression >= 0.0) {
            return Err(Error::new(format!(
                "invalid budgets: max_regression must be a non-negative fraction, got {max_regression}"
            )));
        }
        for (name, budget) in &file.budget {
            let positive = |v: f64| v.is_finite() && v > 0.0;
            if !positive(budget.p99_ns) || !budget.p50_ns.is_none_or(positive) {
                return Err(Error::new(format!(
                    "invalid budget {name:?}: latency limits must be positive"
                )));
            }
        }
        Ok(Self {
            max_regression,
            benches: file.budget,
        })
    }

    /// Reads and parses a budgets file.
    pub fn load(path: &Path) -> Result<Self> {
        let source = std::fs::read_to_string(path)
            .map_err(|e| Error::new(format!("reading {}: {e}", path.display())))?;
        Self::parse(&source).map_err(|e| Error::new(format!("{}: {e}", path.display())))
    }

    /// The budget for a benchmark, by full name.
    pub fn get(&self, name: &str) -> Option<&Budget> {
        self.benches.get(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_budgets_with_optional_fields() {
        let budgets = Budgets::parse(
            r#"
            [gate]
            max_regression = 0.05

            [budget."core_hash/hash_int"]
            mode = "warm"
            p99_ns = 50
            p50_ns = 20.5
            max_allocs = 0

            [budget."kin/partner_of"]
            mode = "cold"
            p99_ns = 5000
            "#,
        )
        .unwrap();
        assert_eq!(budgets.max_regression, 0.05);
        assert_eq!(
            budgets.get("core_hash/hash_int"),
            Some(&Budget {
                mode: Mode::Warm,
                p99_ns: 50.0,
                p50_ns: Some(20.5),
                max_allocs: Some(0),
            })
        );
        let cold = budgets.get("kin/partner_of").unwrap();
        assert_eq!(cold.mode, Mode::Cold);
        assert_eq!(cold.p50_ns, None);
        assert_eq!(cold.max_allocs, None);
    }

    #[test]
    fn regression_threshold_defaults_to_ten_percent() {
        let budgets = Budgets::parse("[budget.\"a/b\"]\nmode = \"warm\"\np99_ns = 1\n").unwrap();
        assert_eq!(budgets.max_regression, DEFAULT_MAX_REGRESSION);
        assert_eq!(Budgets::parse("").unwrap().benches.len(), 0);
    }

    #[test]
    fn rejects_typos_missing_fields_and_nonsense() {
        let typo = "[budget.\"a/b\"]\nmode = \"warm\"\np99_ns = 1\np99 = 2\n";
        assert!(Budgets::parse(typo).is_err(), "unknown key must fail");
        let no_mode = "[budget.\"a/b\"]\np99_ns = 1\n";
        assert!(Budgets::parse(no_mode).is_err(), "mode is required");
        let bad_mode = "[budget.\"a/b\"]\nmode = \"hot\"\np99_ns = 1\n";
        assert!(Budgets::parse(bad_mode).is_err());
        let zero = "[budget.\"a/b\"]\nmode = \"warm\"\np99_ns = 0\n";
        assert!(Budgets::parse(zero).is_err());
        let negative = "[gate]\nmax_regression = -0.1\n";
        assert!(Budgets::parse(negative).is_err());
    }
}
