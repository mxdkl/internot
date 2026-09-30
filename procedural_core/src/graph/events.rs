//! Deterministic communication-event enumeration for a pair of people.
//!
//! Given a pair `(a, b)` and an intensity `λ(t)` in events per day, events
//! come from the window-independent Poisson stream in
//! [`crate::stream::poisson_buckets`] over absolute **days**: each day's
//! count and times are keyed on the absolute day index, never on the query
//! window.
//!
//! Properties:
//! - **Reproducible:** same inputs, bit-identical output, on every machine.
//! - **Symmetric:** keyed on the canonical pair, so `(A, B)` and `(B, A)`
//!   see the same events.
//! - **Recombinable:** `enumerate([t1, t2)) ++ enumerate([t2, t3))` equals
//!   `enumerate([t1, t3))` exactly, for any boundaries.
//! - **Cost:** `O(days + events)` in the window; `λ` is evaluated once per
//!   day at the day's midpoint, and events are uniform within a day. Use an
//!   hourly stream directly from [`crate::stream`] when sub-day shape matters.
//!
//! This replaced a v1 enumerator whose event index and daily grid both
//! restarted at the window start, so slices did not recombine.

use chrono::{DateTime, Utc};

use crate::graph::util::canonical_pair;
use crate::key::{label, Key};
use crate::stream::{from_secs, poisson_buckets, to_secs, DAY};

/// One emitted communication event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CommEvent {
    pub at: DateTime<Utc>,
    /// Which canonical endpoint initiated the event (0 = lo, 1 = hi).
    pub initiator: u8,
}

/// Enumerate events for `(a, b)` in `[t_start, t_end)` from intensity `λ`
/// (events/day), keyed under world seed 0. See
/// [`enumerate_events_keyed`] to key on a specific world.
///
/// `namespace` keys the stream so different modes (mail / chat / calendar)
/// get independent events over the same pair. Returns events in
/// chronological order.
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
    enumerate_events_keyed(
        Key::from_seed(0),
        a,
        b,
        namespace,
        intensity,
        t_start,
        t_end,
    )
}

/// [`enumerate_events`] under an explicit world key.
pub fn enumerate_events_keyed<F>(
    world: Key,
    a: u32,
    b: u32,
    namespace: &str,
    intensity: F,
    t_start: DateTime<Utc>,
    t_end: DateTime<Utc>,
) -> Vec<CommEvent>
where
    F: Fn(DateTime<Utc>) -> f64,
{
    assert!(t_start <= t_end, "t_start must be <= t_end");
    let key = world.with2(canonical_pair(a, b), label(namespace));
    let mut raw = Vec::new();
    poisson_buckets(
        key,
        DAY,
        |d| intensity(from_secs(d * DAY + DAY / 2)).max(0.0),
        to_secs(t_start),
        to_secs(t_end),
        &mut raw,
    );
    const INITIATOR: u64 = label("initiator");
    raw.into_iter()
        .map(|e| CommEvent {
            at: from_secs(e.t),
            initiator: if e.key.with(INITIATOR).unit() < 0.5 {
                0
            } else {
                1
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};

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
