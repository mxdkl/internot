//! Deterministic event-stream enumeration via inhomogeneous Poisson.
//!
//! Given a pair `(a, b)` and an intensity function `λ(t)` (events/day),
//! enumerate event times in `[t_start, t_end)` purely deterministically
//! via the time-rescaling theorem (Brown et al. 2002) and hash-derived
//! exponential inter-event gaps.
//!
//! Hashing is canonical in `(a, b)` so the event list is symmetric in
//! argument order — the same events appear for `(A, B)` and `(B, A)`.
//!
//! Properties:
//! - **Reproducibility:** same `(a, b, namespace, intensity, [t1, t2])`
//!   → bitwise-identical event list.
//! - **Window-locality:** cost is `O(events in window + days in window)`,
//!   not `O(events since 0)`; the daily-grid traversal is linear.
//! - **Slice-recombinability (limited):** see the test
//!   `event_enumeration_slice_recombinability` in
//!   `tests/graph_integration.rs` — full recombinability is a v2
//!   refinement; v1 derives event indices from `t_start`, so slicing
//!   the window changes the event sequence. Use within a stable
//!   window per query.
//!
//! v1 uses a *daily grid* for `Λ` and its inverse, with sub-day events
//! linearly interpolated within each day via midpoint-rule intensity.

use chrono::{DateTime, Duration, Utc};

use crate::graph::util::pair_hash_float;

/// One emitted communication event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CommEvent {
    pub at: DateTime<Utc>,
    /// Which canonical endpoint initiated the event (0 = lo, 1 = hi).
    pub initiator: u8,
}

/// Enumerate events `(A, B)` in `[t_start, t_end]` from intensity `λ`.
///
/// `intensity` is in events/day. `namespace` keys the hash so different
/// modes (mail / chat / calendar) get independent event streams over
/// the same pair.
///
/// Returns events in chronological order, `t_start`-inclusive,
/// `t_end`-exclusive.
pub fn enumerate_events<F>(
    a: u32,
    b: u32,
    namespace: &'static str,
    intensity: F,
    t_start: DateTime<Utc>,
    t_end: DateTime<Utc>,
) -> Vec<CommEvent>
where
    F: Fn(DateTime<Utc>) -> f64,
{
    assert!(t_start <= t_end, "t_start must be <= t_end");
    let mut out = Vec::new();
    if t_start >= t_end {
        return out;
    }
    // Walk forward through days. `running` is Λ accumulated from
    // `t_start`. For each exponential gap, scan forward over days
    // adding daily mass until we exceed the next event's cumulative
    // target, then linearly interpolate to find the sub-day time.
    //
    // `day_cursor` and `day_frac_consumed` are persistent across events: after
    // placing an event within a day, the next event's gap is measured from
    // exactly where we left off in that day. `day_frac_consumed` tracks the
    // fraction of the current day's mass already used.
    let mut running = 0.0_f64;
    let mut k: u64 = 0;
    let total_days = (t_end - t_start).num_days().max(0);
    let mut day_cursor: i64 = 0;
    // Fraction of current day's mass already consumed (0.0 at start of day,
    // 1.0 at end). After placing an event at fraction `frac`, we resume
    // next gap from `frac` within the same day.
    let mut day_frac_consumed: f64 = 0.0;

    loop {
        if day_cursor > total_days {
            return out;
        }
        // Draw the next exponential gap from the current position.
        // v1: two heap-allocations per event (`iet` + `dir` keys). For
        // high-event-rate consumers, replace with a stack-allocated key
        // (TODO v2 — e.g. structured (a, b, namespace_id, kind, k) hash
        // via blake3 update without intermediate String).
        let key = format!("{namespace}:iet:{k}");
        let u = pair_hash_float(a, b, &key);
        // Guard u == 0 (cannot occur from hash_float in practice, but
        // defensive): use MIN_POSITIVE (≈5e-324) rather than EPSILON
        // (≈2.2e-16) so a zero-hash produces a gap of ~730 days rather
        // than ~36 — preserving the intended "this event never fires"
        // semantics if the impossible case ever happened.
        let u = u.max(f64::MIN_POSITIVE);
        let gap = -u.ln();
        let target = running + gap;

        // Scan forward over days until we accumulate enough mass to reach target.
        loop {
            if day_cursor > total_days {
                return out;
            }
            let day_mid = t_start + Duration::hours(24 * day_cursor + 12);
            let day_lambda = intensity(day_mid).max(0.0);
            // Remaining mass in this day (portion not yet consumed).
            let remaining_mass = day_lambda * (1.0 - day_frac_consumed);
            if running + remaining_mass >= target {
                // Event lands inside this day.
                // Fraction into this day where the event lands:
                // solve: running + day_lambda * (frac - day_frac_consumed) = target
                //    =>  frac = day_frac_consumed + (target - running) / day_lambda
                let frac = if day_lambda > 0.0 {
                    day_frac_consumed + (target - running) / day_lambda
                } else {
                    // day_lambda == 0 but remaining_mass >= gap... shouldn't
                    // happen since remaining_mass = 0, but guard anyway.
                    day_frac_consumed
                };
                let frac = frac.clamp(0.0, 1.0);
                let offset_secs = (frac * 86_400.0).clamp(0.0, 86_399.0) as i64;
                let event_at = t_start
                    + Duration::hours(24 * day_cursor)
                    + Duration::seconds(offset_secs);
                if event_at >= t_end {
                    return out;
                }
                let dir_key = format!("{namespace}:dir:{k}");
                let dir_h = pair_hash_float(a, b, &dir_key);
                let initiator = if dir_h < 0.5 { 0 } else { 1 };
                out.push(CommEvent {
                    at: event_at,
                    initiator,
                });
                // Resume next gap from this fraction within the same day.
                running = target;
                day_frac_consumed = frac;
                k += 1;
                break;
            } else {
                // Consume this day entirely and move to the next.
                running += remaining_mass;
                day_cursor += 1;
                day_frac_consumed = 0.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(year: i32, month: u32, day: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, 0, 0, 0).unwrap()
    }

    #[test]
    fn empty_window_returns_no_events() {
        let t = at(2025, 6, 1);
        let ev = enumerate_events(7, 42, "mail", |_| 0.5, t, t);
        assert!(ev.is_empty());
    }

    #[test]
    fn deterministic_same_call() {
        let t1 = at(2025, 6, 1);
        let t2 = at(2025, 6, 30);
        let ev1 = enumerate_events(7, 42, "mail", |_| 0.5, t1, t2);
        let ev2 = enumerate_events(7, 42, "mail", |_| 0.5, t1, t2);
        assert_eq!(ev1, ev2);
    }

    #[test]
    fn symmetric_in_pair() {
        let t1 = at(2025, 6, 1);
        let t2 = at(2025, 6, 30);
        let ev1 = enumerate_events(7, 42, "mail", |_| 0.5, t1, t2);
        let ev2 = enumerate_events(42, 7, "mail", |_| 0.5, t1, t2);
        assert_eq!(ev1, ev2);
    }

    #[test]
    fn different_namespaces_yield_independent_streams() {
        let t1 = at(2025, 6, 1);
        let t2 = at(2025, 6, 30);
        let ev_mail = enumerate_events(7, 42, "mail", |_| 1.0, t1, t2);
        let ev_chat = enumerate_events(7, 42, "chat", |_| 1.0, t1, t2);
        assert_ne!(ev_mail, ev_chat);
    }

    #[test]
    fn events_are_monotone_in_time() {
        let t1 = at(2025, 1, 1);
        let t2 = at(2025, 12, 31);
        let ev = enumerate_events(7, 42, "mail", |_| 1.0, t1, t2);
        for w in ev.windows(2) {
            assert!(w[0].at <= w[1].at);
        }
    }

    #[test]
    fn events_lie_in_window() {
        let t1 = at(2025, 1, 1);
        let t2 = at(2025, 12, 31);
        let ev = enumerate_events(7, 42, "mail", |_| 1.0, t1, t2);
        for e in &ev {
            assert!(e.at >= t1 && e.at < t2);
        }
    }

    #[test]
    fn mean_rate_approximates_lambda() {
        // λ = 1 event/day over 1000 days → expect ~1000 events.
        let t1 = at(2020, 1, 1);
        let t2 = t1 + Duration::days(1000);
        let ev = enumerate_events(7, 42, "mail", |_| 1.0, t1, t2);
        let n = ev.len() as f64;
        // λ=1.0 over 1000 days → ~1000 events. Hashing is deterministic
        // so the count is fixed (n=1031 at time of writing); tolerance
        // 100 is a sanity bound, not a Poisson σ.
        assert!((n - 1000.0).abs() < 100.0, "got {n} events, expected ~1000");
    }
}
