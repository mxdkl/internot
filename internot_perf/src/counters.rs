//! Hardware performance counters for the calling thread, via
//! `perf_event_open` (the same source `perf stat` reads).
//!
//! Counters cover user-space execution of the calling thread only: kernel
//! and hypervisor time are excluded, which is what lets an unprivileged
//! process measure itself at `perf_event_paranoid = 2`.
//!
//! Nothing here panics. When counters cannot be opened (paranoid level 3,
//! a VM without a virtual PMU, a CPU that lacks an event) the affected
//! fields are `None` and [`HwCounters::note`] says why.
//!
//! "Cache references" and "cache misses" are the kernel's generic events;
//! which cache level they count is CPU-specific (on AMD Zen they are L2
//! accesses and misses, on most Intel parts last-level cache).

use std::time::Duration;

use perf_event::events::Hardware;
use perf_event::{Builder, Counter, Group};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Counter totals for one measured region.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HwCounters {
    /// CPU cycles.
    pub cycles: Option<u64>,
    /// Retired instructions.
    pub instructions: Option<u64>,
    /// Cache accesses (see the module docs for which level).
    pub cache_references: Option<u64>,
    /// Cache misses (see the module docs for which level).
    pub cache_misses: Option<u64>,
    /// Mispredicted branches.
    pub branch_misses: Option<u64>,
    /// Why some or all fields are missing, or a caveat on their values.
    pub note: Option<String>,
}

impl HwCounters {
    /// All fields missing, for the given reason.
    pub fn unavailable(note: impl Into<String>) -> Self {
        Self {
            note: Some(note.into()),
            ..Self::default()
        }
    }

    /// Instructions per cycle.
    pub fn ipc(&self) -> Option<f64> {
        match (self.instructions, self.cycles) {
            (Some(instructions), Some(cycles)) if cycles > 0 => {
                Some(instructions as f64 / cycles as f64)
            }
            _ => None,
        }
    }

    /// These totals divided over `calls` calls.
    pub fn per_call(&self, calls: u64) -> PerCallCounters {
        if calls == 0 {
            return PerCallCounters::unavailable("no calls were counted");
        }
        let per = |total: Option<u64>| total.map(|t| t as f64 / calls as f64);
        PerCallCounters {
            cycles: per(self.cycles),
            instructions: per(self.instructions),
            ipc: self.ipc(),
            cache_references: per(self.cache_references),
            cache_misses: per(self.cache_misses),
            branch_misses: per(self.branch_misses),
            note: self.note.clone(),
        }
    }
}

/// Hardware counters averaged per call, as stored in a benchmark report.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PerCallCounters {
    /// CPU cycles per call.
    pub cycles: Option<f64>,
    /// Retired instructions per call.
    pub instructions: Option<f64>,
    /// Instructions per cycle.
    pub ipc: Option<f64>,
    /// Cache accesses per call.
    pub cache_references: Option<f64>,
    /// Cache misses per call.
    pub cache_misses: Option<f64>,
    /// Mispredicted branches per call.
    pub branch_misses: Option<f64>,
    /// Why some or all fields are missing, or a caveat on their values.
    pub note: Option<String>,
}

impl PerCallCounters {
    /// All fields missing, for the given reason.
    pub fn unavailable(note: impl Into<String>) -> Self {
        Self {
            note: Some(note.into()),
            ..Self::default()
        }
    }
}

/// An open set of counters on the calling thread that can be switched on
/// and off around several disjoint regions and read once at the end. Use
/// [`measure`] for the common single-region case.
pub struct CounterGroup {
    group: Group,
    cycles: Option<Counter>,
    instructions: Option<Counter>,
    cache_references: Option<Counter>,
    cache_misses: Option<Counter>,
    branch_misses: Option<Counter>,
    unsupported: Vec<&'static str>,
}

impl CounterGroup {
    /// Opens the counters, disabled. Events the CPU does not support are
    /// skipped (and named in the eventual note); an error means none could
    /// be opened at all.
    pub fn open() -> Result<Self> {
        let mut group = Group::new().map_err(|e| {
            Error::new(format!(
                "perf_event_open failed: {e} (check /proc/sys/kernel/perf_event_paranoid; \
                 values above 2 block unprivileged counters)"
            ))
        })?;
        let mut unsupported = Vec::new();
        let mut add = |event: Hardware, label: &'static str| match group.add(&Builder::new(event)) {
            Ok(counter) => Some(counter),
            Err(_) => {
                unsupported.push(label);
                None
            }
        };
        let cycles = add(Hardware::CPU_CYCLES, "cycles");
        let instructions = add(Hardware::INSTRUCTIONS, "instructions");
        let cache_references = add(Hardware::CACHE_REFERENCES, "cache_references");
        let cache_misses = add(Hardware::CACHE_MISSES, "cache_misses");
        let branch_misses = add(Hardware::BRANCH_MISSES, "branch_misses");
        if unsupported.len() == 5 {
            return Err(Error::new(
                "no hardware events could be opened (no PMU access: VM without a virtual PMU?)",
            ));
        }
        Ok(Self {
            group,
            cycles,
            instructions,
            cache_references,
            cache_misses,
            branch_misses,
            unsupported,
        })
    }

    /// Starts counting.
    pub fn enable(&mut self) -> Result<()> {
        self.group
            .enable()
            .map_err(|e| Error::new(format!("enabling hardware counters: {e}")))
    }

    /// Stops counting. Totals accumulate across enable/disable pairs.
    pub fn disable(&mut self) -> Result<()> {
        self.group
            .disable()
            .map_err(|e| Error::new(format!("disabling hardware counters: {e}")))
    }

    /// Totals so far. If the kernel had to time-share the PMU with other
    /// users, values are scaled up to the enabled time and the note says so.
    pub fn read(&mut self) -> Result<HwCounters> {
        let data = self
            .group
            .read()
            .map_err(|e| Error::new(format!("reading hardware counters: {e}")))?;
        let enabled = data.time_enabled().unwrap_or(Duration::ZERO);
        let running = data.time_running().unwrap_or(Duration::ZERO);
        let mut notes = Vec::new();
        let scale = if running.is_zero() {
            if !enabled.is_zero() {
                return Ok(HwCounters::unavailable(
                    "hardware counters were never scheduled (PMU in use by another profiler?)",
                ));
            }
            1.0
        } else if running < enabled {
            let fraction = running.as_secs_f64() / enabled.as_secs_f64();
            notes.push(format!(
                "PMU was shared: counted {:.0}% of the time, values scaled",
                fraction * 100.0
            ));
            1.0 / fraction
        } else {
            1.0
        };
        if !self.unsupported.is_empty() {
            notes.push(format!(
                "not supported here: {}",
                self.unsupported.join(", ")
            ));
        }
        let value = |counter: &Option<Counter>| {
            counter
                .as_ref()
                .and_then(|c| data.get(c))
                .map(|entry| (entry.value() as f64 * scale).round() as u64)
        };
        Ok(HwCounters {
            cycles: value(&self.cycles),
            instructions: value(&self.instructions),
            cache_references: value(&self.cache_references),
            cache_misses: value(&self.cache_misses),
            branch_misses: value(&self.branch_misses),
            note: (!notes.is_empty()).then(|| notes.join("; ")),
        })
    }
}

/// Runs `f` once with hardware counters on and returns its result with the
/// counter totals. Never fails: unavailable counters come back as `None`
/// fields with a note.
pub fn measure<R>(f: impl FnOnce() -> R) -> (R, HwCounters) {
    let mut group = match CounterGroup::open() {
        Ok(group) => group,
        Err(e) => return (f(), HwCounters::unavailable(e.to_string())),
    };
    if let Err(e) = group.enable() {
        return (f(), HwCounters::unavailable(e.to_string()));
    }
    let result = f();
    let counters = group
        .disable()
        .and_then(|()| group.read())
        .unwrap_or_else(|e| HwCounters::unavailable(e.to_string()));
    (result, counters)
}

/// Checks that counters work on this machine by counting a small loop.
/// `Err` carries the reason they do not.
pub fn probe() -> Result<()> {
    let (_, counters) = measure(|| (0..std::hint::black_box(10_000u64)).sum::<u64>());
    match counters.instructions.or(counters.cycles) {
        Some(n) if n > 0 => Ok(()),
        _ => {
            Err(Error::new(counters.note.unwrap_or_else(|| {
                "hardware counters read zero".to_owned()
            })))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn per_call_divides_totals_and_keeps_ipc() {
        let totals = HwCounters {
            cycles: Some(2_000),
            instructions: Some(5_000),
            cache_references: Some(100),
            cache_misses: None,
            branch_misses: Some(10),
            note: Some("caveat".to_owned()),
        };
        let per = totals.per_call(10);
        assert_eq!(per.cycles, Some(200.0));
        assert_eq!(per.instructions, Some(500.0));
        assert_eq!(per.ipc, Some(2.5));
        assert_eq!(per.cache_references, Some(10.0));
        assert_eq!(per.cache_misses, None);
        assert_eq!(per.branch_misses, Some(1.0));
        assert_eq!(per.note.as_deref(), Some("caveat"));
    }

    #[test]
    fn zero_calls_is_unavailable_not_a_division_by_zero() {
        let per = HwCounters::default().per_call(0);
        assert_eq!(per.cycles, None);
        assert!(per.note.is_some());
    }

    #[test]
    fn measure_never_panics_and_explains_missing_counters() {
        let (value, counters) = measure(|| 21 * 2);
        assert_eq!(value, 42);
        let all_missing = counters.cycles.is_none()
            && counters.instructions.is_none()
            && counters.cache_references.is_none()
            && counters.cache_misses.is_none()
            && counters.branch_misses.is_none();
        if all_missing {
            assert!(
                counters.note.is_some(),
                "missing counters must carry a note"
            );
        }
    }
}
