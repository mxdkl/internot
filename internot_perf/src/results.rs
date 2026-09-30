//! Results files and the `perf/` directory layout.
//!
//! ```text
//! perf/
//!   budgets.toml                          budgets, committed
//!   baselines/<machine-slug>/<suite>.json baselines, committed
//!   results/latest/<suite>.json           last run, git-ignored
//!   flamegraphs/<suite>/<bench>.svg       flamegraphs, git-ignored
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::meta::RunInfo;
use crate::report::BenchReport;

/// One suite's results from one run. Latest results and baselines share
/// this format.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SuiteResults {
    /// Suite name.
    pub suite: String,
    /// Machine and run metadata.
    pub run: RunInfo,
    /// One report per benchmark, in registration order.
    pub benchmarks: Vec<BenchReport>,
}

impl SuiteResults {
    /// The report for a benchmark, by full name.
    pub fn bench(&self, name: &str) -> Option<&BenchReport> {
        self.benchmarks.iter().find(|b| b.name == name)
    }

    /// Writes pretty-printed JSON, creating parent directories.
    pub fn write(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| Error::new(format!("serialising {}: {e}", self.suite)))?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)
                .map_err(|e| Error::new(format!("creating {}: {e}", dir.display())))?;
        }
        fs::write(path, json + "\n")
            .map_err(|e| Error::new(format!("writing {}: {e}", path.display())))
    }

    /// Reads a results file; `Ok(None)` if it does not exist.
    pub fn read(path: &Path) -> Result<Option<Self>> {
        let json = match fs::read_to_string(path) {
            Ok(json) => json,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(Error::new(format!("reading {}: {e}", path.display()))),
        };
        serde_json::from_str(&json)
            .map(Some)
            .map_err(|e| Error::new(format!("parsing {}: {e}", path.display())))
    }
}

/// The workspace root this crate was built in.
pub fn workspace_root() -> &'static Path {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.parent().unwrap_or(manifest_dir)
}

/// Paths inside a `perf/` directory.
#[derive(Clone, Debug)]
pub struct PerfDir {
    root: PathBuf,
}

impl PerfDir {
    /// A `perf/` directory at `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// `perf/` in the workspace this crate was built in.
    pub fn in_workspace() -> Self {
        Self::new(workspace_root().join("perf"))
    }

    /// The directory itself.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `budgets.toml`.
    pub fn budgets(&self) -> PathBuf {
        self.root.join("budgets.toml")
    }

    /// Where a suite's latest results go.
    pub fn latest(&self, suite: &str) -> PathBuf {
        self.root
            .join("results")
            .join("latest")
            .join(format!("{suite}.json"))
    }

    /// Where a suite's baseline for a machine lives.
    pub fn baseline(&self, machine_slug: &str, suite: &str) -> PathBuf {
        self.root
            .join("baselines")
            .join(machine_slug)
            .join(format!("{suite}.json"))
    }

    /// Where a benchmark's flamegraph goes: `flamegraphs/<suite>/<bench>.svg`.
    pub fn flamegraph(&self, bench: &str) -> PathBuf {
        self.root.join("flamegraphs").join(format!("{bench}.svg"))
    }
}
