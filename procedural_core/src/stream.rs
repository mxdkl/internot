//! Window-independent event streams over absolute time.
//!
//! The rule: every draw is keyed on an absolute coordinate (a time bucket, a
//! dyadic tree node, or an ordinal counted from a fixed anchor), never on the
//! query window. Then any window can be enumerated independently and
//! `events([a,b)) ⊎ events([b,c)) == events([a,c))` holds bit for bit.
//!
//! - Recipe A, [`poisson_buckets`]: a Poisson count per absolute bucket, with
//!   times from exponential spacings. Exact for intensities that are constant
//!   within a bucket. Cost `O(buckets + events)`.
//! - Recipe B, [`PoissonTree`]: a dyadic count tree over centuries; "how many
//!   before t" and "last event before t" in `O(levels)` draws.
//! - Recipe C, [`renewal_walk`] and [`hazard_time`]: life-course processes as
//!   a walk from a fixed anchor, one keyed step per event, any duration law.
//!
//! Time is `i64` seconds since 1800-01-01T00:00:00Z ([`EPOCH_1800_UNIX`]).
//!
//! See `docs/superpowers/research/2026-09-29-time-consistent-evolution.md` §2.

use chrono::{DateTime, TimeZone, Utc};

use crate::count::CountTree;
use crate::key::{label, Key};
use crate::sample::{exp1_from_unit, poisson};

/// Unix timestamp of 1800-01-01T00:00:00Z, the substrate's time origin.
pub const EPOCH_1800_UNIX: i64 = -5_364_662_400;

/// Seconds in a day.
pub const DAY: i64 = 86_400;

/// Seconds since 1800-01-01 for a UTC datetime.
#[inline]
pub fn to_secs(t: DateTime<Utc>) -> i64 {
    t.timestamp() - EPOCH_1800_UNIX
}

/// UTC datetime for seconds since 1800-01-01.
#[inline]
pub fn from_secs(s: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(s + EPOCH_1800_UNIX, 0)
        .single()
        .expect("time within chrono's range")
}

/// Days from 1970-01-01 to the proleptic Gregorian date `(y, m, d)`, in
/// closed form (H. Hinnant, "chrono-Compatible Low-Level Date Algorithms",
/// `days_from_civil`). Integer-only, so it is exact and costs a few ns
/// where a `chrono` round trip costs tens.
#[inline]
pub const fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = (y - era * 400) as u64; // [0, 399]
    let mp = ((m + 9) % 12) as u64; // March = 0
    let doy = (153 * mp + 2) / 5 + d as u64 - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe as i64 - 719_468
}

/// Seconds since 1800 at 00:00 UTC on January 1 of `year`.
#[inline]
pub const fn year_start(year: i32) -> i64 {
    days_from_civil(year as i64, 1, 1) * DAY - EPOCH_1800_UNIX
}

/// Calendar year (UTC) of `t`, seconds since 1800 (Hinnant's
/// `civil_from_days`, keeping only the year).
#[inline]
pub const fn year_of(t: i64) -> i32 {
    let z = (t + EPOCH_1800_UNIX).div_euclid(DAY) + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11], March = 0
    let y = yoe as i64 + era * 400;
    (if mp >= 10 { y + 1 } else { y }) as i32
}

/// One event from a stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    /// Seconds since 1800-01-01.
    pub t: i64,
    /// Key for the event's marks (who initiated, what kind, …). Derived from
    /// the event's absolute identity, so it is window-independent too.
    pub key: Key,
}

const TAG_COUNT: u64 = label("stream/count");
const TAG_SPACING: u64 = label("stream/spacing");
const TAG_EVENT: u64 = label("stream/event");

/// One absolute bucket of a stream: `n` events in `[start, start + width)`.
#[derive(Clone, Copy)]
struct Bucket {
    id: u64,
    n: u64,
    start: i64,
    width: i64,
}

/// Visit a bucket's event times in order, uniformly placed by exponential
/// spacings (sorted without sorting). `f(i, t, key)` receives the event's
/// index in the bucket, its time and its key.
#[inline]
fn for_each_in_bucket(key: Key, b: Bucket, mut f: impl FnMut(u64, i64, Key)) {
    let sk = key.with2(TAG_SPACING, b.id);
    // The n + 1 spacings' total normalises positions.
    let mut total = 0.0;
    for i in 0..=b.n {
        total += exp1_from_unit(sk.with(i).unit());
    }
    let mut acc = 0.0;
    for i in 0..b.n {
        acc += exp1_from_unit(sk.with(i).unit());
        let t = b.start + ((b.width as f64) * (acc / total)) as i64;
        f(i, t, key.with3(TAG_EVENT, b.id, i));
    }
}

/// Append a bucket's events that fall in `[t1, t2)`.
#[inline]
fn place_in_bucket(key: Key, b: Bucket, (t1, t2): (i64, i64), out: &mut Vec<Event>) {
    for_each_in_bucket(key, b, |_, t, k| {
        if t >= t1 && t < t2 {
            out.push(Event { t, key: k });
        }
    });
}

/// Recipe A: events of a Poisson process with bucket-wise constant intensity.
///
/// `mass(d)` is the expected number of events in absolute bucket `d`
/// (bucket `d` covers `[d·width, (d+1)·width)` seconds since 1800). Appends
/// events in `[t1, t2)` to `out` in time order.
pub fn poisson_buckets(
    key: Key,
    width: i64,
    mut mass: impl FnMut(i64) -> f64,
    t1: i64,
    t2: i64,
    out: &mut Vec<Event>,
) {
    assert!(width > 0, "bucket width must be positive");
    if t1 >= t2 {
        return;
    }
    let first = t1.div_euclid(width);
    let last = (t2 - 1).div_euclid(width);
    for d in first..=last {
        let lambda = mass(d);
        let n = poisson(key.with2(TAG_COUNT, d as u64), lambda);
        if n > 0 {
            place_in_bucket(
                key,
                Bucket {
                    id: d as u64,
                    n,
                    start: d * width,
                    width,
                },
                (t1, t2),
                out,
            );
        }
    }
}

/// Recipe B: a Poisson process over `2^levels` leaves of `leaf_width`
/// seconds starting at `origin`, with counts on a dyadic [`CountTree`].
///
/// `mass(t0, t1)` is the expected number of events in `[t0, t1)`; it must be
/// additive. The root total is Poisson(mass of the whole span). Queries do
/// not allocate (except [`Self::events`], which appends to its output).
pub struct PoissonTree {
    key: Key,
    origin: i64,
    leaf_width: i64,
    tree: CountTree<Box<dyn Fn(u64, u64) -> f64>>,
}

impl PoissonTree {
    /// Build the tree. `levels` leaves span `2^levels · leaf_width` seconds.
    pub fn new(
        key: Key,
        origin: i64,
        leaf_width: i64,
        levels: u8,
        mass: impl Fn(i64, i64) -> f64 + 'static,
    ) -> Self {
        assert!(leaf_width > 0, "leaf width must be positive");
        let span = (1i64 << levels) * leaf_width;
        let total = poisson(key.with(TAG_COUNT), mass(origin, origin + span));
        let weight: Box<dyn Fn(u64, u64) -> f64> = Box::new(move |lo, hi| {
            mass(
                origin + lo as i64 * leaf_width,
                origin + hi as i64 * leaf_width,
            )
        });
        Self {
            key,
            origin,
            leaf_width,
            tree: CountTree::new(key.with(label("stream/tree")), levels, total, weight),
        }
    }

    /// Leaf containing `t`, clamped: `None` before the origin, `leaves()` past the end.
    #[inline]
    fn leaf_index(&self, t: i64) -> Option<u64> {
        if t < self.origin {
            return None;
        }
        Some((((t - self.origin) / self.leaf_width) as u64).min(self.tree.leaves()))
    }

    #[inline]
    fn leaf_bucket(&self, leaf: u64, n: u64) -> Bucket {
        Bucket {
            id: leaf,
            n,
            start: self.origin + leaf as i64 * self.leaf_width,
            width: self.leaf_width,
        }
    }

    /// Total events over the whole span.
    pub fn total(&self) -> u64 {
        self.tree.total()
    }

    /// Number of events strictly before `t`.
    pub fn count_before(&self, t: i64) -> u64 {
        let Some(leaf) = self.leaf_index(t) else {
            return 0;
        };
        if leaf >= self.tree.leaves() {
            return self.tree.total();
        }
        let (before, n) = self.tree.prefix_and_leaf(leaf);
        let mut inside = 0u64;
        for_each_in_bucket(self.key, self.leaf_bucket(leaf, n), |_, te, _| {
            if te < t {
                inside += 1;
            }
        });
        before + inside
    }

    /// Append events in `[t1, t2)` to `out`, in time order.
    pub fn events(&self, t1: i64, t2: i64, out: &mut Vec<Event>) {
        if t1 >= t2 {
            return;
        }
        let lo = self.leaf_index(t1).unwrap_or(0);
        let hi = match self.leaf_index(t2 - 1) {
            None => return,
            Some(l) => (l + 1).min(self.tree.leaves()),
        };
        self.tree.for_each_nonempty(lo, hi, |leaf, n| {
            place_in_bucket(self.key, self.leaf_bucket(leaf, n), (t1, t2), out);
        });
    }

    /// The last event strictly before `t`, if any. `O(levels)` draws.
    pub fn last_before(&self, t: i64) -> Option<Event> {
        let k = self.count_before(t);
        if k == 0 {
            return None;
        }
        let (leaf, pos) = self.tree.select(k - 1);
        let n = self.tree.leaf_count(leaf);
        let mut found = None;
        for_each_in_bucket(self.key, self.leaf_bucket(leaf, n), |i, te, key| {
            if i == pos {
                found = Some(Event { t: te, key });
            }
        });
        found
    }
}

/// Markov "hold or redraw" state at `t` (the regeneration lemma).
///
/// A state that, at the events of `regen`, is redrawn fresh (and otherwise
/// holds) equals the draw made at the last regeneration before `t`, so it
/// costs one `last_before` query, `O(levels)`, whatever `t` is. This covers
/// edge-Markovian ties and "stay or redraw" labels (Clementi et al.;
/// Mazzarisi et al. 2020). `initial` gives the state before any
/// regeneration; `draw` makes a state from a regeneration's key.
pub fn regen_state<S>(
    regen: &PoissonTree,
    t: i64,
    initial: impl FnOnce() -> S,
    draw: impl FnOnce(Key) -> S,
) -> S {
    match regen.last_before(t) {
        Some(e) => draw(e.key),
        None => initial(),
    }
}

/// Recipe C helper: time at which a piecewise-constant hazard, starting at
/// `t0`, accumulates `e` units of integrated hazard (an Exponential(1)
/// draw). `rate(t)` returns `(rate per second, end of this constant piece)`.
/// Returns `None` if the hazard hasn't accumulated `e` by `horizon`.
pub fn hazard_time(
    mut e: f64,
    t0: i64,
    horizon: i64,
    mut rate: impl FnMut(i64) -> (f64, i64),
) -> Option<i64> {
    let mut t = t0;
    while t < horizon {
        let (r, piece_end) = rate(t);
        let end = piece_end.min(horizon).max(t + 1);
        let mass = r * (end - t) as f64;
        if r > 0.0 && mass >= e {
            return Some(t + (e / r).round() as i64);
        }
        e -= mass;
        t = end;
    }
    None
}

/// Recipe C: walk a semi-Markov process from a fixed anchor.
///
/// `step(state, t, k, key_k)` returns the next event's time and state, or
/// `None` to stop; `key_k` is `key.with(k)`, so step `k`'s randomness never
/// depends on the query. `visit(k, t, &state)` sees every event up to
/// `until` (exclusive), then the walk stops. Returns the state in force at
/// `until`.
pub fn renewal_walk<S: Clone>(
    key: Key,
    anchor: i64,
    init: S,
    until: i64,
    mut step: impl FnMut(&S, i64, u64, Key) -> Option<(i64, S)>,
    mut visit: impl FnMut(u64, i64, &S),
) -> S {
    let (mut t, mut state, mut k) = (anchor, init, 0u64);
    while let Some((t_next, s_next)) = step(&state, t, k, key.with(k)) {
        debug_assert!(t_next >= t, "renewal_walk: time went backwards");
        if t_next >= until {
            break;
        }
        t = t_next;
        state = s_next;
        visit(k, t, &state);
        k += 1;
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_matches_chrono() {
        use chrono::Datelike;
        assert_eq!(year_start(1800), 0);
        assert_eq!(year_start(1970), -EPOCH_1800_UNIX);
        for year in -400..=3000 {
            let t = Utc.with_ymd_and_hms(year, 1, 1, 0, 0, 0).unwrap();
            assert_eq!(year_start(year), to_secs(t), "year_start({year})");
            assert_eq!(year_of(year_start(year)), year);
            assert_eq!(year_of(year_start(year) - 1), year - 1);
        }
        for (y, m, d) in [(2000, 2, 29), (1900, 3, 1), (2024, 12, 31), (1600, 2, 29)] {
            let t = Utc.with_ymd_and_hms(y, m, d, 0, 0, 0).unwrap();
            assert_eq!(days_from_civil(y as i64, m, d) * DAY, t.timestamp());
        }
        let mut t = -3_000_000_000_000i64;
        while t < 30_000_000_000 {
            assert_eq!(year_of(t), from_secs(t).year(), "year_of({t})");
            t += 7_777_777;
        }
    }

    #[test]
    fn epoch_constant_is_1800() {
        let t = Utc.with_ymd_and_hms(1800, 1, 1, 0, 0, 0).unwrap();
        assert_eq!(t.timestamp(), EPOCH_1800_UNIX);
        assert_eq!(to_secs(t), 0);
        let t2 = Utc.with_ymd_and_hms(2026, 9, 29, 12, 0, 0).unwrap();
        assert_eq!(from_secs(to_secs(t2)), t2);
    }

    #[test]
    fn buckets_recombine_bit_for_bit_at_any_boundary() {
        let key = Key::from_seed(10);
        let mass = |d: i64| 0.3 + 0.2 * ((d % 7) as f64);
        let (a, c) = (1_000_000i64, 1_000_000 + 200 * DAY);
        let mut full = Vec::new();
        poisson_buckets(key, DAY, mass, a, c, &mut full);
        assert!(full.len() > 50);
        for b in [a + 1, a + DAY, a + 17 * DAY + 12_345, c - 1] {
            let (mut lo, mut hi) = (Vec::new(), Vec::new());
            poisson_buckets(key, DAY, mass, a, b, &mut lo);
            poisson_buckets(key, DAY, mass, b, c, &mut hi);
            lo.extend(hi);
            assert_eq!(lo, full, "split at {b}");
        }
    }

    #[test]
    fn buckets_are_sorted_and_rate_correct() {
        let key = Key::from_seed(11);
        let mut evs = Vec::new();
        poisson_buckets(key, DAY, |_| 2.0, 0, 5_000 * DAY, &mut evs);
        assert!(evs.windows(2).all(|w| w[0].t <= w[1].t));
        let n = evs.len() as f64;
        assert!((n - 10_000.0).abs() < 400.0, "{n}");
        // Event keys are unique.
        let mut keys: Vec<u64> = evs.iter().map(|e| e.key.bits()).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), evs.len());
    }

    #[test]
    fn negative_times_work() {
        let key = Key::from_seed(12);
        let mut full = Vec::new();
        poisson_buckets(key, DAY, |_| 1.0, -30 * DAY, 30 * DAY, &mut full);
        let (mut lo, mut hi) = (Vec::new(), Vec::new());
        poisson_buckets(key, DAY, |_| 1.0, -30 * DAY, 5, &mut lo);
        poisson_buckets(key, DAY, |_| 1.0, 5, 30 * DAY, &mut hi);
        lo.extend(hi);
        assert_eq!(lo, full);
    }

    #[test]
    fn tree_counts_match_enumeration_and_recombine() {
        let key = Key::from_seed(13);
        // One event per ~100 days over 2^18 days (~718 years).
        let tree = PoissonTree::new(key, 0, DAY, 18, |a: i64, b: i64| {
            (b - a) as f64 / (100.0 * DAY as f64)
        });
        let span = (1i64 << 18) * DAY;
        let mut all = Vec::new();
        tree.events(0, span, &mut all);
        assert!(all.windows(2).all(|w| w[0].t <= w[1].t));
        let expect = (1u64 << 18) as f64 / 100.0;
        assert!(
            (all.len() as f64 - expect).abs() < 5.0 * expect.sqrt(),
            "{}",
            all.len()
        );
        for &t in &[0i64, 12_345, 50_000 * DAY + 77, span - 1, span] {
            let direct = all.iter().filter(|e| e.t < t).count() as u64;
            assert_eq!(tree.count_before(t), direct, "t = {t}");
        }
        let mid = 123_456 * DAY + 999;
        let (mut lo, mut hi) = (Vec::new(), Vec::new());
        tree.events(0, mid, &mut lo);
        tree.events(mid, span, &mut hi);
        lo.extend(hi);
        assert_eq!(lo, all);
        // last_before agrees with the enumeration.
        let t = 200_000 * DAY;
        let want = all.iter().rev().find(|e| e.t < t).copied();
        assert_eq!(tree.last_before(t), want);
    }

    #[test]
    fn regen_state_holds_between_regenerations() {
        let key = Key::from_seed(15);
        let regen = PoissonTree::new(key, 0, DAY, 16, |a: i64, b: i64| {
            (b - a) as f64 / (50.0 * DAY as f64)
        });
        let mut evs = Vec::new();
        regen.events(0, (1 << 16) * DAY, &mut evs);
        assert!(evs.len() > 100);
        let state = |t: i64| regen_state(&regen, t, || u64::MAX, |k| k.below(1000));
        assert_eq!(state(0), u64::MAX);
        for w in evs.windows(2) {
            // Constant on (e_i, e_{i+1}], equal to the draw at e_i.
            let (a, b) = (w[0].t, w[1].t);
            if b > a + 1 {
                let want = w[0].key.below(1000);
                assert_eq!(state(a + 1), want);
                assert_eq!(state(b), want);
            }
        }
    }

    #[test]
    fn hazard_time_inverts_piecewise_rates() {
        // Rate 1/day for 10 days, then 0.1/day.
        let r = |t: i64| {
            if t < 10 * DAY {
                (1.0 / DAY as f64, 10 * DAY)
            } else {
                (0.1 / DAY as f64, i64::MAX)
            }
        };
        assert_eq!(hazard_time(2.0, 0, 1000 * DAY, r), Some(2 * DAY));
        // 10 units used in the first piece, 1 more at 0.1/day takes 10 days.
        assert_eq!(hazard_time(11.0, 0, 1000 * DAY, r), Some(20 * DAY));
        assert_eq!(hazard_time(1e9, 0, 1000 * DAY, r), None);
    }

    #[test]
    fn renewal_walk_is_prefix_consistent() {
        let key = Key::from_seed(14);
        let step = |s: &u32, t: i64, _k: u64, kk: Key| {
            let gap = (exp1_from_unit(kk.unit()) * 365.0 * DAY as f64) as i64 + 1;
            Some((t + gap, s + 1))
        };
        let mut times_full = Vec::new();
        let end_state = renewal_walk(key, 0, 0u32, 50 * 365 * DAY, step, |_, t, _| {
            times_full.push(t)
        });
        assert_eq!(end_state as usize, times_full.len());
        let mut times_short = Vec::new();
        renewal_walk(key, 0, 0u32, 20 * 365 * DAY, step, |_, t, _| {
            times_short.push(t)
        });
        assert_eq!(&times_full[..times_short.len()], &times_short[..]);
    }
}
