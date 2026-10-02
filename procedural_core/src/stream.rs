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
//! - Regeneration: [`regen_state`] (a state redrawn at a stream's events is
//!   the draw at the last one) and its nested form, [`nested_regen`] and
//!   [`nested_regen_state`] (a move at level `j` redraws levels `≥ j`), each
//!   one `last_before` per level, with no replay.
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

/// Where a level's state was last regenerated: the event, and the level of
/// the move that caused it (at most the level itself).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Regen {
    pub event: Event,
    pub from: usize,
}

/// Nested regeneration times (hierarchical "hold or redraw").
///
/// Levels `0..levels` are nested, 0 the coarsest (region ⊃ county ⊃ tract).
/// A move at level `j` redraws every level `≥ j` and keeps the coarser ones.
/// `last_before(j, t)` is the last move of level `j` strictly before `t`
/// (for example [`PoissonTree::last_before`] on level `j`'s stream).
/// Writes to `out[k]` the last regeneration of level `k`: the latest move of
/// any level `≤ k`, so `out` is nondecreasing in time. Moves at the same
/// instant count the finer level as the later one. Costs one `last_before`
/// per level, whatever the number of moves: no replay.
///
/// Panics if `out.len() < levels`.
pub fn nested_regen(
    levels: usize,
    t: i64,
    mut last_before: impl FnMut(usize, i64) -> Option<Event>,
    out: &mut [Option<Regen>],
) {
    assert!(out.len() >= levels, "one output per level");
    let mut best: Option<Regen> = None;
    for (k, o) in out.iter_mut().enumerate().take(levels) {
        if let Some(e) = last_before(k, t) {
            if best.map_or(true, |b| e.t >= b.event.t) {
                best = Some(Regen { event: e, from: k });
            }
        }
        *o = best;
    }
}

/// The nested state at `t`: level `k`'s state is the draw made at its last
/// regeneration ([`nested_regen`]), under the level-`(k−1)` state.
///
/// That parent is the one in force at `t`, since no coarser move happened
/// after level `k`'s regeneration: the regeneration lemma ([`regen_state`])
/// applied level by level. `initial(k, parent)` gives a level's state before
/// any regeneration; `draw(k, regen, key, parent)` draws it at a
/// regeneration, with `key = regen.event.key.with(k)` so the levels redrawn
/// by one move draw independently. `parent` is `None` at level 0. Writes the
/// states, coarsest first, to `out` (cleared first).
pub fn nested_regen_state<S>(
    levels: usize,
    t: i64,
    mut last_before: impl FnMut(usize, i64) -> Option<Event>,
    mut initial: impl FnMut(usize, Option<&S>) -> S,
    mut draw: impl FnMut(usize, Regen, Key, Option<&S>) -> S,
    out: &mut Vec<S>,
) {
    out.clear();
    let mut best: Option<Regen> = None;
    for k in 0..levels {
        if let Some(e) = last_before(k, t) {
            if best.map_or(true, |b| e.t >= b.event.t) {
                best = Some(Regen { event: e, from: k });
            }
        }
        let parent = out.last();
        let s = match best {
            Some(r) => draw(k, r, r.event.key.with(k as u64), parent),
            None => initial(k, parent),
        };
        out.push(s);
    }
}

/// The nested state at `t` when a level's own moves are drawn relative to
/// an **anchor**: the state that level took at the last regeneration from a
/// coarser level (its "home" there), or its initial state.
///
/// A move that redraws from level `j` sets level `j` by
/// `draw_move(j, regen, key, parent, anchor_j)` (for example, a destination
/// drawn by distance from home rather than from the current place), and
/// every finer level `k > j` by `draw_fresh(k, regen, key, parent)`, which
/// also becomes `k`'s new anchor. Anchors depend only on coarser
/// regenerations, so the regeneration lemma still holds: one `last_before`
/// per level, no replay. It matches a full replay exactly (tests). Keys are
/// `regen.event.key.with(k)` as in [`nested_regen_state`].
#[allow(clippy::type_complexity)]
pub fn nested_regen_anchored<S: Clone>(
    levels: usize,
    t: i64,
    mut last_before: impl FnMut(usize, i64) -> Option<Event>,
    mut initial: impl FnMut(usize, Option<&S>) -> S,
    mut draw_fresh: impl FnMut(usize, Regen, Key, Option<&S>) -> S,
    mut draw_move: impl FnMut(usize, Regen, Key, Option<&S>, &S) -> S,
    out: &mut Vec<S>,
) {
    out.clear();
    // The latest regeneration from any level coarser than k (`coarser`),
    // and from any level up to k (`best`).
    let mut coarser: Option<Regen> = None;
    for k in 0..levels {
        let own = last_before(k, t);
        let parent = out.last();
        let anchor = match coarser {
            Some(r) => draw_fresh(k, r, r.event.key.with(k as u64), parent),
            None => initial(k, parent),
        };
        let state = match own {
            // Its own move is the latest (ties go to the finer level).
            Some(e) if coarser.map_or(true, |c| e.t >= c.event.t) => {
                let r = Regen { event: e, from: k };
                let s = draw_move(k, r, e.key.with(k as u64), parent, &anchor);
                coarser = Some(r);
                s
            }
            _ => anchor,
        };
        out.push(state);
    }
}

/// Fixed-length epochs over absolute time with a keyed phase: epoch `e`
/// is `[origin + e·span, origin + (e + 1)·span)`. Keying the phase per
/// group keeps groups from all turning over on the same day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Epochs {
    pub origin: i64,
    pub span: i64,
}

impl Epochs {
    /// Epochs of `span` seconds whose first starts `key.below(span)` after
    /// `start`.
    #[inline]
    pub fn keyed(start: i64, span: i64, key: Key) -> Self {
        assert!(span > 0, "epochs need a positive length");
        Self {
            origin: start + key.below(span as u64) as i64,
            span,
        }
    }

    /// The epoch containing `t` (negative before the origin).
    #[inline]
    pub fn index(&self, t: i64) -> i64 {
        (t - self.origin).div_euclid(self.span)
    }

    /// The start of epoch `e`.
    #[inline]
    pub fn start(&self, e: i64) -> i64 {
        self.origin + e * self.span
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

    /// Three nested levels, coarse moves rare and fine moves frequent.
    fn level_trees(seed: u64) -> Vec<PoissonTree> {
        [400.0, 120.0, 30.0]
            .iter()
            .enumerate()
            .map(|(j, &days)| {
                PoissonTree::new(
                    Key::from_seed(seed).with(j as u64),
                    0,
                    DAY,
                    15,
                    move |a: i64, b: i64| (b - a) as f64 / (days * DAY as f64),
                )
            })
            .collect()
    }

    fn mix(h: u64, v: u64) -> u64 {
        (h ^ v).wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(29)
    }

    /// The nested state by replaying every move in time order (finer level
    /// later at equal times).
    fn replay(trees: &[PoissonTree], t: i64) -> Vec<u64> {
        let levels = trees.len();
        let mut moves: Vec<(i64, usize, usize, Event)> = Vec::new();
        for (j, tree) in trees.iter().enumerate() {
            let mut evs = Vec::new();
            tree.events(0, t.max(0), &mut evs);
            moves.extend(evs.into_iter().enumerate().map(|(i, e)| (e.t, j, i, e)));
        }
        moves.sort_by_key(|m| (m.0, m.1, m.2));
        let mut state: Vec<u64> = Vec::new();
        for k in 0..levels {
            let parent = state.last().copied().unwrap_or(0);
            state.push(mix(parent, 1000 + k as u64));
        }
        for (_, j, _, e) in moves {
            for k in j..levels {
                let parent = if k == 0 { 0 } else { state[k - 1] };
                state[k] = mix(parent, e.key.with(k as u64).bits());
            }
        }
        state
    }

    fn nested(trees: &[PoissonTree], t: i64) -> Vec<u64> {
        let mut out = Vec::new();
        nested_regen_state(
            trees.len(),
            t,
            |j, t| trees[j].last_before(t),
            |k, parent: Option<&u64>| mix(parent.copied().unwrap_or(0), 1000 + k as u64),
            |_, _, key, parent| mix(parent.copied().unwrap_or(0), key.bits()),
            &mut out,
        );
        out
    }

    #[test]
    fn nested_regeneration_matches_a_full_replay() {
        for seed in 0..6 {
            let trees = level_trees(seed);
            let span = (1i64 << 15) * DAY;
            let mut fine = Vec::new();
            trees[2].events(0, span, &mut fine);
            assert!(fine.len() > 500);
            let mut times: Vec<i64> = vec![-5, 0, 1, span - 1, span, span + 99];
            times.extend((0..60).map(|i| Key::from_seed(seed).with(i).below(span as u64) as i64));
            // At and just after moves of every level.
            for tree in &trees {
                let mut evs = Vec::new();
                tree.events(0, span, &mut evs);
                for e in evs.iter().step_by(7) {
                    times.extend([e.t, e.t + 1]);
                }
            }
            for &t in &times {
                assert_eq!(nested(&trees, t), replay(&trees, t), "seed {seed}, t {t}");
            }
        }
    }

    /// The anchored nested state by replaying every move in time order: a
    /// move at level j draws j around its anchor and refreshes (and
    /// re-anchors) every finer level.
    fn replay_anchored(trees: &[PoissonTree], t: i64) -> Vec<u64> {
        let levels = trees.len();
        let mut moves: Vec<(i64, usize, usize, Event)> = Vec::new();
        for (j, tree) in trees.iter().enumerate() {
            let mut evs = Vec::new();
            tree.events(0, t.max(0), &mut evs);
            moves.extend(evs.into_iter().enumerate().map(|(i, e)| (e.t, j, i, e)));
        }
        moves.sort_by_key(|m| (m.0, m.1, m.2));
        let mut state: Vec<u64> = Vec::new();
        for k in 0..levels {
            let parent = state.last().copied().unwrap_or(0);
            state.push(mix(parent, 1000 + k as u64));
        }
        let mut anchor = state.clone();
        for (_, j, _, e) in moves {
            for k in j..levels {
                let parent = if k == 0 { 0 } else { state[k - 1] };
                let key = e.key.with(k as u64).bits();
                if k == j {
                    state[k] = mix(mix(parent, key), anchor[k]);
                } else {
                    state[k] = mix(parent, key);
                    anchor[k] = state[k];
                }
            }
        }
        state
    }

    fn nested_anchored(trees: &[PoissonTree], t: i64) -> Vec<u64> {
        let mut out = Vec::new();
        nested_regen_anchored(
            trees.len(),
            t,
            |j, t| trees[j].last_before(t),
            |k, parent: Option<&u64>| mix(parent.copied().unwrap_or(0), 1000 + k as u64),
            |_, _, key, parent| mix(parent.copied().unwrap_or(0), key.bits()),
            |_, _, key, parent, anchor: &u64| {
                mix(mix(parent.copied().unwrap_or(0), key.bits()), *anchor)
            },
            &mut out,
        );
        out
    }

    #[test]
    fn anchored_regeneration_matches_a_full_replay() {
        for seed in 0..6 {
            let trees = level_trees(seed);
            let span = (1i64 << 15) * DAY;
            let mut times: Vec<i64> = vec![-5, 0, 1, span - 1, span, span + 99];
            times.extend((0..60).map(|i| Key::from_seed(seed).with(i).below(span as u64) as i64));
            for tree in &trees {
                let mut evs = Vec::new();
                tree.events(0, span, &mut evs);
                for e in evs.iter().step_by(7) {
                    times.extend([e.t, e.t + 1]);
                }
            }
            for &t in &times {
                assert_eq!(
                    nested_anchored(&trees, t),
                    replay_anchored(&trees, t),
                    "seed {seed}, t {t}"
                );
            }
        }
    }

    #[test]
    fn anchored_ties_go_to_the_finer_level() {
        // Levels 0 and 1 move at the same instant: level 1's own move wins
        // (drawn around its anchor, refreshed by level 0's move).
        let e = |t: i64, s: u64| Event {
            t,
            key: Key::from_seed(s),
        };
        let moves = [Some(e(50, 1)), Some(e(50, 2))];
        let mut out = Vec::new();
        nested_regen_anchored(
            2,
            100,
            |j, _| moves[j],
            |k, _| 100 + k as u64,
            |k, r, _, _| 10 * (k as u64) + r.from as u64,
            |k, r, _, _, a: &u64| 1000 * (k as u64 + 1) + 100 * r.from as u64 + a,
            &mut out,
        );
        // Level 0: its own move around its initial anchor (100).
        // Level 1: its own move (from 1) around the anchor drawn fresh at
        // level 0's move (10·1 + 0 = 10).
        assert_eq!(out, vec![1100, 2110]);
    }

    #[test]
    fn nested_regeneration_times_are_nested() {
        let trees = level_trees(9);
        let mut out = [None; 3];
        for i in 0..200 {
            let t = i * 160 * DAY + 12_345;
            nested_regen(3, t, |j, t| trees[j].last_before(t), &mut out);
            for k in 0..3 {
                let own = trees[k].last_before(t);
                // The level's regeneration is at least as recent as its own
                // moves and as the coarser level's regeneration.
                if let Some(e) = own {
                    assert!(out[k].unwrap().event.t >= e.t);
                }
                if k > 0 {
                    assert!(out[k].map(|r| r.event.t) >= out[k - 1].map(|r| r.event.t));
                }
                if let Some(r) = out[k] {
                    assert!(r.from <= k && r.event.t < t);
                    assert_eq!(trees[r.from].last_before(t), Some(r.event));
                }
            }
        }
    }

    #[test]
    fn nested_ties_go_to_the_finer_level() {
        let e = |t: i64, s: u64| Event {
            t,
            key: Key::from_seed(s),
        };
        let moves = [Some(e(50, 1)), Some(e(50, 2)), Some(e(40, 3))];
        let mut out = [None; 3];
        nested_regen(3, 100, |j, _| moves[j], &mut out);
        assert_eq!(
            out.map(|r| r.map(|r| (r.event.t, r.from))),
            [Some((50, 0)), Some((50, 1)), Some((50, 1))]
        );
        nested_regen(
            3,
            100,
            |j, _| if j == 2 { moves[2] } else { None },
            &mut out,
        );
        assert_eq!(out.map(|r| r.map(|r| r.from)), [None, None, Some(2)]);
    }

    #[test]
    fn nested_regeneration_golden() {
        let trees = level_trees(77);
        let t = 20_000 * DAY;
        let mut out = [None; 3];
        nested_regen(3, t, |j, t| trees[j].last_before(t), &mut out);
        let got = out.map(|r| r.map(|r| (r.event.t, r.from)));
        assert_eq!(got, GOLDEN_NESTED);
        assert_eq!(nested(&trees, t), GOLDEN_NESTED_STATE);
    }

    const GOLDEN_NESTED: [Option<(i64, usize)>; 3] = [
        Some((1_707_161_077, 0)),
        Some((1_709_302_508, 1)),
        Some((1_720_574_874, 2)),
    ];
    const GOLDEN_NESTED_STATE: [u64; 3] = [
        12_910_264_497_775_185_185,
        7_323_164_947_375_710_727,
        5_422_628_283_462_481_694,
    ];

    #[test]
    fn epochs_tile_time() {
        let e = Epochs::keyed(1000, 7 * DAY, Key::from_seed(4));
        assert!((1000..1000 + 7 * DAY).contains(&e.origin));
        for t in [
            -50 * DAY,
            0,
            e.origin - 1,
            e.origin,
            e.origin + 3 * DAY,
            400 * DAY,
        ] {
            let i = e.index(t);
            assert!(e.start(i) <= t && t < e.start(i + 1), "{t}");
        }
        assert_eq!(e.index(e.origin), 0);
        assert_eq!(e.index(e.origin - 1), -1);
    }
}
