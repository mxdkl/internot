//! Thermal settling before each benchmark.
//!
//! A laptop CPU under sustained load reaches its thermal limit within
//! seconds (the reference laptop goes from 63 °C to 93 °C in about 7 s at
//! 4.5 GHz) and then throttles. Benchmarks late in a suite would run slower
//! than early ones, and the same benchmark would swing ±40% between runs
//! depending on what ran before it. Waiting until the CPU has cooled to a
//! fixed temperature before each benchmark makes every one start from the
//! same thermal state.

use std::path::PathBuf;
use std::time::{Duration, Instant};

/// The CPU package temperature sensor: `k10temp` (AMD) or `coretemp`
/// (Intel) under hwmon, else the first ACPI thermal zone.
fn sensor() -> Option<PathBuf> {
    if let Ok(dir) = std::fs::read_dir("/sys/class/hwmon") {
        for entry in dir.flatten() {
            let name = std::fs::read_to_string(entry.path().join("name")).unwrap_or_default();
            if matches!(name.trim(), "k10temp" | "coretemp") {
                return Some(entry.path().join("temp1_input"));
            }
        }
    }
    let zone = PathBuf::from("/sys/class/thermal/thermal_zone0/temp");
    zone.exists().then_some(zone)
}

/// Current CPU temperature in °C, if a sensor is readable.
pub fn cpu_temp_c() -> Option<f64> {
    let raw = std::fs::read_to_string(sensor()?).ok()?;
    raw.trim().parse::<f64>().ok().map(|milli| milli / 1000.0)
}

/// What a settle did: temperature before and after, and the time waited.
#[derive(Clone, Copy, Debug)]
pub struct Settled {
    pub from_c: f64,
    pub to_c: f64,
    pub waited: Duration,
}

/// Wait until the CPU is at or below `limit_c`, polling every 200 ms, for
/// at most `timeout`. `None` if no sensor is readable (nothing to wait on).
pub fn settle(limit_c: f64, timeout: Duration) -> Option<Settled> {
    let start = Instant::now();
    let from_c = cpu_temp_c()?;
    let mut now_c = from_c;
    while now_c > limit_c && start.elapsed() < timeout {
        std::thread::sleep(Duration::from_millis(200));
        now_c = cpu_temp_c().unwrap_or(now_c);
    }
    Some(Settled {
        from_c,
        to_c: now_c,
        waited: start.elapsed(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settle_returns_at_once_when_already_cool() {
        // Any real CPU is below 150 °C, so this never waits.
        if let Some(s) = settle(150.0, Duration::from_secs(1)) {
            assert!(s.waited < Duration::from_millis(100));
            assert!(s.from_c > 0.0 && s.from_c < 150.0);
        }
    }
}
