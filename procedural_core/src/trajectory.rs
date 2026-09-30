//! Trajectory primitives — shapes that vary over time.

use crate::word::BitWord;
use chrono::{DateTime, Duration, Utc};

/// Smooth value-noise trajectory: amplitude-bounded, continuous in time.
///
/// Produces a deterministic value in `[-amplitude, amplitude]` for `(id, t)`.
/// Values at times within `timescale` of each other vary smoothly;
/// values at times separated by much more than `timescale` are essentially independent.
pub fn smooth<W: BitWord>(id: W, t: DateTime<Utc>, amplitude: f64, timescale: Duration) -> f64 {
    let t_seconds = t.timestamp() as f64 + t.timestamp_subsec_nanos() as f64 * 1e-9;
    let ts_seconds = timescale.num_seconds().max(1) as f64;
    let x = t_seconds / ts_seconds;

    // Sample value-noise at integer grid points, then smoothstep-interpolate.
    let x_floor = x.floor();
    let i0 = x_floor as i64;
    let i1 = i0 + 1;
    let frac = x - x_floor;
    let s = frac * frac * (3.0 - 2.0 * frac);

    let v0 = grid_sample(id, i0);
    let v1 = grid_sample(id, i1);
    amplitude * (v0 * (1.0 - s) + v1 * s)
}

/// Step trajectory: returns the magnitude of the latest event with time <= t.
///
/// Useful for discrete life events — salary after job changes, current residence
/// after moves, current partner after marriages. Events must be given in ascending
/// time order; in debug builds, unsorted events will panic.
///
/// The `id` parameter is unused — events/magnitudes fully determine the output.
/// It exists so all trajectory primitives share a uniform `(id, t, ...)` signature.
///
/// Returns 0.0 if no events have occurred by time `t`.
///
/// # Panics
///
/// Panics if `events.len() != magnitudes.len()`. In debug builds, also panics if
/// `events` is not in ascending order.
pub fn step<W: BitWord>(
    _id: W,
    t: DateTime<Utc>,
    events: &[DateTime<Utc>],
    magnitudes: &[f64],
) -> f64 {
    assert_eq!(
        events.len(),
        magnitudes.len(),
        "events and magnitudes must have equal length"
    );
    debug_assert!(
        events.windows(2).all(|w| w[0] <= w[1]),
        "step: events must be in ascending time order"
    );
    let mut latest = 0.0;
    for (event_time, magnitude) in events.iter().zip(magnitudes.iter()) {
        if *event_time <= t {
            latest = *magnitude;
        } else {
            break;
        }
    }
    latest
}

/// Oscillating trajectory: `amplitude * sin(2π * t / period + phase)`.
///
/// Good for daily/weekly/seasonal rhythms. The `phase` is in radians:
/// `phase = 0` starts at `sin(0) = 0` and rises; `phase = π/2` starts at
/// the peak; `phase = π` starts at zero and falls.
///
/// The `id` parameter is unused — `(t, period, amplitude, phase)` fully determine
/// the output. It exists so all trajectory primitives share a uniform
/// `(id, t, ...)` signature.
pub fn oscillate<W: BitWord>(
    _id: W,
    t: DateTime<Utc>,
    period: Duration,
    amplitude: f64,
    phase: f64,
) -> f64 {
    let t_seconds = t.timestamp() as f64 + t.timestamp_subsec_nanos() as f64 * 1e-9;
    let period_seconds = period.num_seconds().max(1) as f64;
    let theta = 2.0 * std::f64::consts::PI * t_seconds / period_seconds + phase;
    amplitude * theta.sin()
}

/// Shared stability-radius formula — Taylor-quadratic with 0.9 safety factor.
///
/// Returns `δ` such that `|f(t+d) − f(t)| ≤ ε` for `|d| ≤ δ`, given the
/// absolute first derivative `|f'(t)|` and absolute second derivative `|f''(t)|`.
///
/// - When `f''=0`: collapses to `ε / |f'|` (classical Lipschitz jump).
/// - When `f'=0`: collapses to `√(2ε / |f''|)` (quadratic plateau at extremum).
/// - When both are zero: returns `f64::INFINITY` (fully flat region).
pub fn stability_radius_quadratic(abs_df: f64, abs_d2f: f64, epsilon: f64) -> f64 {
    const SAFETY: f64 = 0.9;
    const TINY: f64 = 1e-12;
    if abs_d2f < TINY {
        if abs_df < TINY {
            f64::INFINITY
        } else {
            SAFETY * epsilon / abs_df
        }
    } else {
        let disc = abs_df * abs_df + 2.0 * abs_d2f * epsilon;
        SAFETY * (-abs_df + disc.sqrt()) / abs_d2f
    }
}

/// Stability radius for `smooth`. Uses the closed-form first and second derivatives
/// of the value-noise trajectory within the current grid cell.
pub fn smooth_stability_radius<W: BitWord>(
    id: W,
    t: DateTime<Utc>,
    amplitude: f64,
    timescale: Duration,
    epsilon: f64,
) -> Duration {
    let t_seconds = t.timestamp() as f64 + t.timestamp_subsec_nanos() as f64 * 1e-9;
    let ts_seconds = timescale.num_seconds().max(1) as f64;
    let x = t_seconds / ts_seconds;
    let x_floor = x.floor();
    let i0 = x_floor as i64;
    let frac = x - x_floor;
    // smoothstep'(s) = 6s(1-s); smoothstep''(s) = 6(1-2s)
    let sstep_d = 6.0 * frac * (1.0 - frac);
    let sstep_d2 = 6.0 * (1.0 - 2.0 * frac);
    // Within one cell, f(t) = amp · [v0·(1-s) + v1·s] where v0, v1 are grid samples in [-1, 1)
    // f'(t) = (amp/ts_sec) · sstep'(frac) · (v1 - v0)
    // f''(t) = (amp/ts_sec²) · sstep''(frac) · (v1 - v0)
    let v0 = grid_sample(id, i0);
    let v1 = grid_sample(id, i0 + 1);
    let dv = v1 - v0;
    let abs_df = (amplitude / ts_seconds) * sstep_d.abs() * dv.abs();
    let abs_d2f = (amplitude / (ts_seconds * ts_seconds)) * sstep_d2.abs() * dv.abs();
    let delta_seconds = stability_radius_quadratic(abs_df, abs_d2f, epsilon);
    duration_from_seconds_f64(delta_seconds)
}

/// Stability radius for `oscillate`. Closed-form derivatives of `amp·sin(ωt + φ)`.
pub fn oscillate_stability_radius<W: BitWord>(
    _id: W,
    t: DateTime<Utc>,
    period: Duration,
    amplitude: f64,
    phase: f64,
    epsilon: f64,
) -> Duration {
    let t_seconds = t.timestamp() as f64 + t.timestamp_subsec_nanos() as f64 * 1e-9;
    let period_seconds = period.num_seconds().max(1) as f64;
    let omega = 2.0 * std::f64::consts::PI / period_seconds;
    let theta = omega * t_seconds + phase;
    let abs_df = amplitude.abs() * omega * theta.cos().abs();
    let abs_d2f = amplitude.abs() * omega * omega * theta.sin().abs();
    let delta_seconds = stability_radius_quadratic(abs_df, abs_d2f, epsilon);
    duration_from_seconds_f64(delta_seconds)
}

/// Stability radius for `step`. Returns the distance to the nearest event whose
/// magnitude change exceeds `epsilon` — or near-infinity if all events are within tolerance.
pub fn step_stability_radius<W: BitWord>(
    _id: W,
    t: DateTime<Utc>,
    events: &[DateTime<Utc>],
    magnitudes: &[f64],
    epsilon: f64,
) -> Duration {
    assert_eq!(
        events.len(),
        magnitudes.len(),
        "events and magnitudes must have equal length"
    );
    let mut current_value = 0.0;
    for (event_time, magnitude) in events.iter().zip(magnitudes.iter()) {
        if *event_time <= t {
            current_value = *magnitude;
        } else {
            break;
        }
    }
    // Distance to the nearest event (past or future) whose change vs. current_value exceeds epsilon
    let mut min_distance = Duration::try_days(365 * 10_000).unwrap(); // effectively "infinity" for our purposes
                                                                      // Previous events: if we go back past an event, the value changes from current_value
                                                                      // to the value that was active before that event. Find the nearest such event where
                                                                      // that change exceeds epsilon.
    let mut prior_value = 0.0;
    for (event_time, magnitude) in events.iter().zip(magnitudes.iter()) {
        if *event_time > t {
            break;
        }
        // At event_time, value changed from prior_value to *magnitude.
        // Going back past this event means the value would change from current_value to prior_value.
        if (prior_value - current_value).abs() > epsilon {
            let distance = t - *event_time;
            if distance < min_distance && distance >= Duration::zero() {
                min_distance = distance;
            }
        }
        prior_value = *magnitude;
    }
    // Future events
    for (event_time, magnitude) in events.iter().zip(magnitudes.iter()) {
        if *event_time > t && (*magnitude - current_value).abs() > epsilon {
            let distance = *event_time - t;
            if distance < min_distance {
                min_distance = distance;
            }
        }
    }
    min_distance
}

fn duration_from_seconds_f64(s: f64) -> Duration {
    if !s.is_finite() || s > 1e15 {
        // "essentially infinite" — return a very large duration
        return Duration::try_days(365 * 10_000).unwrap();
    }
    let nanos = (s * 1e9).round() as i64;
    Duration::nanoseconds(nanos.max(0))
}

fn grid_sample<W: BitWord>(id: W, i: i64) -> f64 {
    // Hash (id, "smooth", i) to a [-1, 1) value.
    let mut hasher = xxhash_rust::xxh3::Xxh3::new();
    hasher.update(id.to_le_bytes().as_ref());
    hasher.update(b"\0smooth\0");
    hasher.update(&i.to_le_bytes());
    let h = hasher.digest();
    let u = ((h >> 11) as f64) / ((1u64 << 53) as f64);
    u * 2.0 - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Duration, TimeZone, Utc};

    fn t(year: i32, month: u32, day: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, 12, 0, 0).unwrap()
    }

    #[test]
    fn smooth_is_deterministic() {
        let a = smooth(42u64, t(2026, 4, 21), 1.0, Duration::days(365));
        let b = smooth(42u64, t(2026, 4, 21), 1.0, Duration::days(365));
        assert_eq!(a, b);
    }

    #[test]
    fn smooth_stays_within_amplitude_envelope() {
        let amplitude = 2.5;
        for day in 0..365 * 10 {
            let t = t(2020, 1, 1) + Duration::days(day);
            let v = smooth(42u64, t, amplitude, Duration::days(30));
            assert!(
                v.abs() <= amplitude + 1e-9,
                "value {} exceeds amplitude {} on day {}",
                v,
                amplitude,
                day
            );
        }
    }

    #[test]
    fn smooth_is_continuous_within_timescale() {
        // Two nearby times (1 day apart) within a 365-day timescale should produce close values
        let t1 = t(2026, 4, 21);
        let t2 = t1 + Duration::days(1);
        let a = smooth(42u64, t1, 1.0, Duration::days(365));
        let b = smooth(42u64, t2, 1.0, Duration::days(365));
        // Maximum change over 1/365 of timescale should be small
        assert!((a - b).abs() < 0.1, "too much change: {} -> {}", a, b);
    }

    #[test]
    fn smooth_varies_across_time() {
        let t1 = t(2020, 1, 1);
        let t2 = t(2040, 1, 1);
        let a = smooth(42u64, t1, 1.0, Duration::days(30));
        let b = smooth(42u64, t2, 1.0, Duration::days(30));
        assert_ne!(a, b);
    }

    #[test]
    fn smooth_varies_across_ids() {
        let now = t(2026, 4, 21);
        let a = smooth(1u64, now, 1.0, Duration::days(30));
        let b = smooth(2u64, now, 1.0, Duration::days(30));
        assert_ne!(a, b);
    }

    #[test]
    fn step_returns_zero_before_first_event() {
        let events = vec![t(2030, 1, 1), t(2035, 1, 1)];
        let magnitudes = vec![5.0, 10.0];
        let v = step(42u64, t(2020, 1, 1), &events, &magnitudes);
        assert_eq!(v, 0.0);
    }

    #[test]
    fn step_returns_latest_magnitude() {
        let events = vec![t(2030, 1, 1), t(2035, 1, 1)];
        let magnitudes = vec![5.0, 10.0];
        assert_eq!(step(42u64, t(2032, 1, 1), &events, &magnitudes), 5.0);
        assert_eq!(step(42u64, t(2040, 1, 1), &events, &magnitudes), 10.0);
    }

    #[test]
    fn step_exact_event_boundary_counts_as_applied() {
        let events = vec![t(2030, 1, 1)];
        let magnitudes = vec![5.0];
        assert_eq!(step(42u64, t(2030, 1, 1), &events, &magnitudes), 5.0);
    }

    #[test]
    fn step_handles_empty_events() {
        let v = step(42u64, t(2030, 1, 1), &[], &[]);
        assert_eq!(v, 0.0);
    }

    #[test]
    #[should_panic(expected = "events and magnitudes must have equal length")]
    fn step_panics_on_length_mismatch() {
        step(42u64, t(2030, 1, 1), &[t(2030, 1, 1)], &[1.0, 2.0]);
    }

    #[test]
    #[should_panic(expected = "events must be in ascending time order")]
    #[cfg(debug_assertions)]
    fn step_panics_on_unsorted_events_in_debug() {
        step(
            42u64,
            t(2040, 1, 1),
            &[t(2035, 1, 1), t(2030, 1, 1)],
            &[10.0, 5.0],
        );
    }

    #[test]
    fn oscillate_is_bounded_by_amplitude() {
        for day in 0..365 * 3 {
            let time = t(2020, 1, 1) + Duration::days(day);
            let v = oscillate(42u64, time, Duration::days(7), 0.5, 0.0);
            assert!(v.abs() <= 0.5 + 1e-9);
        }
    }

    #[test]
    fn oscillate_is_periodic() {
        let period = Duration::days(7);
        let t0 = t(2020, 1, 1);
        let t1 = t0 + period;
        let a = oscillate(42u64, t0, period, 1.0, 0.0);
        let b = oscillate(42u64, t1, period, 1.0, 0.0);
        assert!((a - b).abs() < 1e-9, "not periodic: {} vs {}", a, b);
    }

    #[test]
    fn oscillate_is_deterministic() {
        let time = t(2026, 4, 21);
        let a = oscillate(42u64, time, Duration::days(7), 1.0, 0.3);
        let b = oscillate(42u64, time, Duration::days(7), 1.0, 0.3);
        assert_eq!(a, b);
    }

    #[test]
    fn oscillate_phase_shift_matters() {
        let time = t(2026, 4, 21);
        let a = oscillate(42u64, time, Duration::days(7), 1.0, 0.0);
        let b = oscillate(42u64, time, Duration::days(7), 1.0, 1.0);
        assert_ne!(a, b);
    }

    #[test]
    fn smooth_works_with_u128() {
        let id: u128 = 0xDEAD_BEEF_CAFE_1234_5678_9ABC_DEF0_1234;
        let time = t(2026, 4, 21);
        let v = smooth(id, time, 1.0, Duration::days(365));
        assert!(v.abs() <= 1.0 + 1e-9);
        assert_eq!(
            v,
            smooth(id, time, 1.0, Duration::days(365)),
            "should be deterministic"
        );
    }

    #[test]
    fn step_works_with_u128() {
        let events = vec![t(2030, 1, 1), t(2035, 1, 1)];
        let magnitudes = vec![5.0, 10.0];
        let id: u128 = 42;
        assert_eq!(step(id, t(2020, 1, 1), &events, &magnitudes), 0.0);
        assert_eq!(step(id, t(2040, 1, 1), &events, &magnitudes), 10.0);
    }

    #[test]
    fn oscillate_works_with_u128() {
        let id: u128 = 42;
        let time = t(2026, 4, 21);
        let v = oscillate(id, time, Duration::days(7), 1.0, 0.0);
        assert!(v.abs() <= 1.0 + 1e-9);
        assert_eq!(
            v,
            oscillate(id, time, Duration::days(7), 1.0, 0.0),
            "should be deterministic"
        );
    }

    #[test]
    fn quadratic_helper_linear_case() {
        // When f'' ≈ 0, delta should be ≈ epsilon / |f'| with 0.9 safety factor
        let delta = stability_radius_quadratic(10.0, 0.0, 1.0);
        assert!(
            (delta - 0.09).abs() < 1e-9,
            "expected 0.09 (0.9 * 1.0/10.0), got {}",
            delta
        );
    }

    #[test]
    fn quadratic_helper_flat_case() {
        // f'=0, f''=0 -> infinite radius
        let delta = stability_radius_quadratic(0.0, 0.0, 1.0);
        assert!(delta.is_infinite());
    }

    #[test]
    fn quadratic_helper_extremum_case() {
        // f'=0, f''=8 -> delta = 0.9 * sqrt(2 * 1.0 / 8) = 0.9 * 0.5 = 0.45
        let delta = stability_radius_quadratic(0.0, 8.0, 1.0);
        assert!((delta - 0.45).abs() < 1e-9, "expected 0.45, got {}", delta);
    }

    #[test]
    fn oscillate_stability_radius_at_zero_crossing() {
        // At t such that sin(omega*t + phi) = 0, f' is max (cos=1).
        // delta should be governed mostly by |f'|.
        let time = t(2020, 1, 1); // phase=0, so sin(2*pi*t/period) near 0 only at t=0
                                  // Use phase to place us at a known zero-crossing: phase = -omega*t (mod pi)
                                  // Simpler: use t=t0 + half-period offset and pick phase so sin = 0
                                  // For now, just sanity-check that stability_radius returns a positive Duration
        let delta = oscillate_stability_radius(42u64, time, Duration::days(7), 1.0, 0.0, 0.1);
        assert!(delta > Duration::zero());
        assert!(delta < Duration::days(7)); // smaller than the period
    }

    #[test]
    fn oscillate_stability_radius_conservative() {
        // For any t, the returned delta should ACTUALLY keep |f(t') - f(t)| <= epsilon
        // Sample several points in [t-delta, t+delta] and verify.
        let time = t(2026, 4, 21);
        let period = Duration::days(7);
        let amp = 1.0;
        let phase = 0.3;
        let eps = 0.05;
        let delta = oscillate_stability_radius(42u64, time, period, amp, phase, eps);
        let v0 = oscillate(42u64, time, period, amp, phase);
        // Sample 10 points within [-delta, delta]
        for i in -5..=5 {
            let offset = Duration::nanoseconds((delta.num_nanoseconds().unwrap() as i64) * i / 5);
            let t_probe = time + offset;
            let v_probe = oscillate(42u64, t_probe, period, amp, phase);
            assert!(
                (v_probe - v0).abs() <= eps + 1e-9,
                "oscillate violated stability radius: |{} - {}| > {}",
                v_probe,
                v0,
                eps
            );
        }
    }

    #[test]
    fn smooth_stability_radius_conservative() {
        let time = t(2026, 4, 21);
        let amp = 1.0;
        let ts = Duration::days(30);
        let eps = 0.05;
        let delta = smooth_stability_radius(42u64, time, amp, ts, eps);
        let v0 = smooth(42u64, time, amp, ts);
        for i in -5..=5 {
            let offset = Duration::nanoseconds((delta.num_nanoseconds().unwrap() as i64) * i / 5);
            let t_probe = time + offset;
            let v_probe = smooth(42u64, t_probe, amp, ts);
            assert!(
                (v_probe - v0).abs() <= eps + 1e-9,
                "smooth violated stability radius: |{} - {}| > {}",
                v_probe,
                v0,
                eps
            );
        }
    }

    #[test]
    fn step_stability_radius_is_exact_event_distance() {
        // Step function: plateau is the gap to the next event exceeding epsilon
        let events = vec![t(2030, 1, 1), t(2035, 1, 1), t(2040, 1, 1)];
        let magnitudes = vec![5.0, 10.0, 10.5]; // jumps of 5, 5, 0.5
        let query_t = t(2032, 1, 1);
        // At t=2032, current value is 5.0. Next event at 2035 jumps to 10 (change = 5 > eps)
        // Previous event at 2030 jumped from 0 to 5 (change = 5 > eps, but it's in the past)
        // For epsilon = 1.0, radius = min(2035 - 2032, 2032 - 2030) = 2 years
        let delta = step_stability_radius(42u64, query_t, &events, &magnitudes, 1.0);
        let _expected = Duration::days(2 * 365); // approximate, actual is slightly more
                                                 // Allow some calendar slop: delta should be ~730 days
        assert!(
            delta >= Duration::days(730) && delta <= Duration::days(732),
            "expected ~2 years, got {:?}",
            delta
        );
    }

    #[test]
    fn step_stability_radius_small_jumps_ignored() {
        // Event with magnitude change < epsilon should be "crossable"
        let events = vec![t(2030, 1, 1), t(2035, 1, 1)];
        let magnitudes = vec![5.0, 5.3]; // jump of 0.3
        let query_t = t(2025, 1, 1); // before first event, value = 0
                                     // At t=2025, value is 0. Next event jumps to 5 (change = 5 > eps).
                                     // For epsilon = 1.0, radius = 2030 - 2025 = 5 years
                                     // For epsilon = 10.0, even the 5-jump is within tolerance, so radius = infinity-ish
                                     //   (or the distance to the LAST event, after which nothing changes by > 10)
        let delta_tight = step_stability_radius(42u64, query_t, &events, &magnitudes, 1.0);
        assert!(delta_tight >= Duration::days(5 * 365 - 1));
        let delta_loose = step_stability_radius(42u64, query_t, &events, &magnitudes, 10.0);
        // All events absorbed by tolerance; no relevant event changes value by > eps
        // Distance to furthest relevant event is infinity (or some large value)
        assert!(delta_loose > Duration::days(365 * 100));
    }
}
