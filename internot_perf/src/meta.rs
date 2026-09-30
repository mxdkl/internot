//! Machine and run metadata stamped into every results file, so a number
//! can always be traced to the hardware, governor, toolchain and commit
//! that produced it.

use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::{alloc, counters};

/// The machine a run happened on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MachineInfo {
    /// CPU model name from `/proc/cpuinfo`.
    pub cpu_model: String,
    /// Logical cores available to this process.
    pub logical_cores: usize,
    /// cpufreq governor of cpu0 (`powersave`, `performance`, ...), if the
    /// kernel exposes it.
    pub governor: Option<String>,
    /// Directory name for this machine's baselines: CPU model plus cores.
    pub slug: String,
}

impl MachineInfo {
    /// Reads the current machine's details.
    pub fn detect() -> Self {
        let cpu_model = std::fs::read_to_string("/proc/cpuinfo")
            .ok()
            .and_then(|info| cpu_model_from_cpuinfo(&info))
            .unwrap_or_else(|| "unknown cpu".to_owned());
        let logical_cores = std::thread::available_parallelism().map_or(1, |n| n.get());
        let governor =
            std::fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor")
                .ok()
                .map(|g| g.trim().to_owned());
        let slug = machine_slug(&cpu_model, logical_cores);
        Self {
            cpu_model,
            logical_cores,
            governor,
            slug,
        }
    }
}

/// The first `model name` line of a `/proc/cpuinfo` dump.
fn cpu_model_from_cpuinfo(cpuinfo: &str) -> Option<String> {
    cpuinfo
        .lines()
        .find_map(|line| line.strip_prefix("model name"))
        .and_then(|rest| rest.split_once(':'))
        .map(|(_, model)| model.trim().to_owned())
}

/// `AMD Ryzen 9 5900HX with Radeon Graphics`, 16 →
/// `amd-ryzen-9-5900hx-with-radeon-graphics-16t`.
pub fn machine_slug(cpu_model: &str, logical_cores: usize) -> String {
    let mut slug = String::new();
    for c in cpu_model.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-');
    format!("{slug}-{logical_cores}t")
}

/// Metadata for one `perf-gate` run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunInfo {
    /// The machine.
    pub machine: MachineInfo,
    /// `rustc --version` of the compiler that built the harness.
    pub rustc: String,
    /// Cargo profile the harness was built with (`release` for release and
    /// profiles inheriting from it).
    pub build_profile: String,
    /// `git rev-parse` of the workspace, with `-dirty` for uncommitted
    /// changes to tracked files; `None` outside a git checkout.
    pub git_commit: Option<String>,
    /// When the run started, RFC 3339 UTC.
    pub timestamp: String,
    /// Whether this was a `--quick` smoke run.
    pub quick: bool,
    /// Whether hardware counters could be read.
    pub hw_counters_available: bool,
    /// Why hardware counters could not be read, if they could not.
    pub hw_counters_note: Option<String>,
    /// Whether the counting allocator was installed (allocation budgets
    /// can only be checked when it was).
    pub alloc_counting: bool,
}

impl RunInfo {
    /// Collects metadata for a run starting now in the given workspace.
    pub fn collect(workspace_root: &Path, quick: bool) -> Self {
        let hw_probe = counters::probe();
        Self {
            machine: MachineInfo::detect(),
            rustc: env!("INTERNOT_PERF_RUSTC_VERSION").to_owned(),
            build_profile: env!("INTERNOT_PERF_BUILD_PROFILE").to_owned(),
            git_commit: git_commit(workspace_root),
            timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            quick,
            hw_counters_available: hw_probe.is_ok(),
            hw_counters_note: hw_probe.err().map(|e| e.to_string()),
            alloc_counting: alloc::counting_installed(),
        }
    }
}

fn git_commit(workspace_root: &Path) -> Option<String> {
    let git = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(workspace_root)
            .args(args)
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
    };
    let commit = git(&["rev-parse", "--short=12", "HEAD"])?;
    let dirty = git(&["status", "--porcelain", "--untracked-files=no"])
        .is_some_and(|status| !status.is_empty());
    Some(if dirty {
        format!("{commit}-dirty")
    } else {
        commit
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_is_lowercase_dashed_with_core_count() {
        assert_eq!(
            machine_slug("AMD Ryzen 9 5900HX with Radeon Graphics", 16),
            "amd-ryzen-9-5900hx-with-radeon-graphics-16t"
        );
        assert_eq!(
            machine_slug("Intel(R) Core(TM) i7-8650U CPU @ 1.90GHz", 8),
            "intel-r-core-tm-i7-8650u-cpu-1-90ghz-8t"
        );
    }

    #[test]
    fn cpu_model_comes_from_the_first_model_name_line() {
        let cpuinfo = "processor\t: 0\nvendor_id\t: AuthenticAMD\n\
                       model name\t: AMD Ryzen 9 5900HX with Radeon Graphics\n\
                       processor\t: 1\nmodel name\t: ignored\n";
        assert_eq!(
            cpu_model_from_cpuinfo(cpuinfo).as_deref(),
            Some("AMD Ryzen 9 5900HX with Radeon Graphics")
        );
        assert_eq!(cpu_model_from_cpuinfo("processor\t: 0\n"), None);
    }
}
