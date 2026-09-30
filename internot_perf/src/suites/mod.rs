//! The registered suites. [`ALL`] is the single list `perf-gate` walks;
//! adding a suite is a new module here plus one line in [`ALL`].

use crate::harness::Harness;

mod core_hash;
mod kinship;
mod primitives;

/// A named group of benchmarks.
pub struct Suite {
    /// Name used by `--suite`, in benchmark names (`<suite>/<bench>`) and
    /// in results file names.
    pub name: &'static str,
    /// One line on what the suite covers.
    pub about: &'static str,
    /// Builds inputs and registers benchmarks on the harness.
    pub run: fn(&mut Harness),
}

/// Every suite, in the order `perf-gate` runs them.
pub const ALL: &[Suite] = &[core_hash::SUITE, primitives::SUITE, kinship::SUITE];

/// The suite with this name.
pub fn find(name: &str) -> Option<&'static Suite> {
    ALL.iter().find(|s| s.name == name)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn suite_names_are_unique_and_path_safe() {
        let mut seen = BTreeSet::new();
        for suite in ALL {
            assert!(seen.insert(suite.name), "duplicate suite {}", suite.name);
            assert!(
                suite
                    .name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                "suite name {:?} must be lowercase snake_case",
                suite.name
            );
        }
    }
}
