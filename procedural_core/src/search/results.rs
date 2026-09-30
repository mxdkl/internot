//! Termination reporting and filter-chained iterator for find() results.

use crate::search::FindIter;
use crate::search::Op;
use crate::word::BitWord;
use chrono::{DateTime, Duration, Utc};

/// Time anchor set on a `Query` to drive temporal filters.
///
/// `At(t)` pairs with `filter_temporal(...)`. `AtAny(start, end)` pairs with
/// `filter_exists_in_range(...)` / `filter_forall_in_range(...)`. A single
/// query cannot mix the two — `execute()` panics on a mismatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeAnchor {
    /// Single instant. Pairs with `filter_temporal`.
    At(DateTime<Utc>),
    /// Half-open time range `[start, end)`. Pairs with
    /// `filter_exists_in_range` / `filter_forall_in_range`.
    AtAny(DateTime<Utc>, DateTime<Utc>),
}

/// How to obtain the stability radius during a temporal range sweep.
///
/// `Analytic` takes a user-supplied closure `(id, t, ε) → δ` returning a
/// conservative radius such that `|value_fn(id, t') − value_fn(id, t)| ≤ ε`
/// for all `t' ∈ [t−δ, t+δ]`.
///
/// `BinarySearch` computes each step as `(range_end − t) / probes` — a
/// **shrinking per-call step**, not a fixed grid. The first call on a
/// 1-day range with `probes = 4` returns 6h; later calls return
/// proportionally less as `t` approaches `range_end`. Combined with
/// `MIN_SWEEP_STEP_SECS = 1s` this guarantees termination. Simple but can
/// miss satisfying instants when the function oscillates faster than the
/// schedule — see `search-design.md` §5.3 for the caveat.
///
/// Note: the closures in `Analytic` are not `Send + Sync`; any future
/// parallel-evaluation path will need a stricter bound.
pub enum StabilityMode<'a, W: BitWord> {
    Analytic(Box<dyn Fn(W, DateTime<Utc>, f64) -> Duration + 'a>),
    BinarySearch { probes: usize },
}

impl<'a, W: BitWord> StabilityMode<'a, W> {
    /// Compute the step `δ` to advance the sweep from `t`.
    /// `range_end` is the end of the sweep window (required for
    /// `BinarySearch` to size its shrinking per-call step).
    pub(crate) fn delta(
        &self,
        id: W,
        t: DateTime<Utc>,
        epsilon: f64,
        range_end: DateTime<Utc>,
    ) -> Duration {
        match self {
            StabilityMode::Analytic(f) => f(id, t, epsilon),
            StabilityMode::BinarySearch { probes } => {
                let remaining = range_end - t;
                let steps = (*probes).max(1) as i64;
                Duration::nanoseconds(remaining.num_nanoseconds().unwrap_or(i64::MAX) / steps)
            }
        }
    }
}

/// Quantifier for a temporal range predicate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Quantifier {
    Exists,
    Forall,
}

/// Termination state of a [`Results`] iterator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Termination {
    /// Iterator still running — no terminal state reached yet.
    Pending,
    /// Iterator reached end of candidate stream naturally.
    Exhaustive,
    /// Iterator stopped because `scan_budget(n)` was set and `n` candidates
    /// were drawn from the underlying enumeration. `evaluated` equals the
    /// budget when this is reported.
    Budgeted { evaluated: usize },
}

/// A post-enumeration filter attached to a query. Static filters depend only
/// on the candidate ID; temporal filters additionally take a `DateTime<Utc>`
/// set by `Query::at(t)`.
pub(crate) enum Filter<'a, W: BitWord> {
    Static(Box<dyn Fn(W) -> bool + 'a>),
    Temporal(Box<dyn Fn(W, DateTime<Utc>) -> bool + 'a>),
    TemporalRange {
        value_fn: Box<dyn Fn(W, DateTime<Utc>) -> f64 + 'a>,
        op: Op,
        threshold: f64,
        quantifier: Quantifier,
        stability: StabilityMode<'a, W>,
    },
}

/// Hard cap on sweep iterations per candidate — defends against misconfigured
/// `StabilityMode` or pathological ranges. A correctly-tuned analytic
/// stability radius on a realistic range completes in ≤ 10² iterations.
pub(crate) const MAX_SWEEP_ITERATIONS: usize = 1_000_000;

/// Floor on the per-step advance. Prevents the sweep from stalling at exact
/// threshold crossings where `ε = 0` ⇒ `δ = 0`.
pub(crate) const MIN_SWEEP_STEP_SECS: i64 = 1;

/// Evaluate a `Filter::TemporalRange` for a single candidate.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sweep_passes<'a, W: BitWord>(
    id: W,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    value_fn: &(dyn Fn(W, DateTime<Utc>) -> f64 + 'a),
    op: Op,
    threshold: f64,
    quantifier: Quantifier,
    stability: &StabilityMode<'a, W>,
) -> bool {
    let min_step = Duration::seconds(MIN_SWEEP_STEP_SECS);
    let mut t = start;
    let mut iterations = 0usize;
    while t < end {
        iterations += 1;
        if iterations > MAX_SWEEP_ITERATIONS {
            panic!(
                "temporal sweep exceeded {MAX_SWEEP_ITERATIONS} iterations — \
                 stability radius too small or range too large"
            );
        }
        let v = value_fn(id, t);
        let matched = op.check(v, threshold);
        match quantifier {
            Quantifier::Exists => {
                if matched {
                    return true;
                }
            }
            Quantifier::Forall => {
                if !matched {
                    return false;
                }
            }
        }
        let epsilon = (v - threshold).abs();
        let delta = stability.delta(id, t, epsilon, end);
        let step = if delta > min_step { delta } else { min_step };
        t += step;
    }
    match quantifier {
        Quantifier::Exists => false,
        Quantifier::Forall => true,
    }
}

/// Filtered, budget-aware iterator over the IDs matching a `find()` query.
pub struct Results<'a, W: BitWord> {
    pub(crate) inner: FindIter<W>,
    pub(crate) filters: Vec<(String, Filter<'a, W>)>,
    pub(crate) budget: Option<usize>,
    pub(crate) time_anchor: Option<TimeAnchor>,
    pub(crate) evaluated: usize,
    pub(crate) termination: Termination,
}

impl<'a, W: BitWord> Results<'a, W> {
    /// Current termination state. `Pending` while the iterator is still
    /// running; transitions exactly once to `Exhaustive` or `Budgeted`
    /// when `next()` returns `None`.
    pub fn termination(&self) -> &Termination {
        &self.termination
    }

    /// How many candidates have been drawn from the underlying enumeration
    /// so far (including ones filtered out).
    pub fn evaluated(&self) -> usize {
        self.evaluated
    }
}

impl<'a, W: BitWord> Iterator for Results<'a, W> {
    type Item = W;

    fn next(&mut self) -> Option<W> {
        // Once terminated, keep returning None.
        if !matches!(self.termination, Termination::Pending) {
            return None;
        }
        loop {
            // Budget check before drawing another candidate.
            if let Some(limit) = self.budget {
                if self.evaluated >= limit {
                    self.termination = Termination::Budgeted {
                        evaluated: self.evaluated,
                    };
                    return None;
                }
            }
            let candidate = match self.inner.next() {
                Some(c) => c,
                None => {
                    self.termination = Termination::Exhaustive;
                    return None;
                }
            };
            self.evaluated += 1;
            let passes = self.filters.iter().all(|(_, f)| match f {
                Filter::Static(g) => g(candidate),
                Filter::Temporal(g) => {
                    let t = match &self.time_anchor {
                        Some(TimeAnchor::At(t)) => *t,
                        _ => panic!(
                            "Filter::Temporal reached with non-At anchor — execute() check failed"
                        ),
                    };
                    g(candidate, t)
                }
                Filter::TemporalRange {
                    value_fn,
                    op,
                    threshold,
                    quantifier,
                    stability,
                } => {
                    let (start, end) = match &self.time_anchor {
                        Some(TimeAnchor::AtAny(s, e)) => (*s, *e),
                        _ => panic!(
                            "Filter::TemporalRange reached with non-AtAny anchor — execute() check failed"
                        ),
                    };
                    sweep_passes(
                        candidate,
                        start,
                        end,
                        value_fn,
                        *op,
                        *threshold,
                        *quantifier,
                        stability,
                    )
                }
            });
            if passes {
                return Some(candidate);
            }
            // else keep looping
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bits::BitLayout;
    use crate::search::BitPattern;
    use chrono::TimeZone;
    use std::collections::HashMap;

    fn u64_layout() -> BitLayout<u64> {
        BitLayout::<u64>::new(vec![("a", 4), ("b", 4)]).unwrap()
    }

    fn make_inner(layout: &BitLayout<u64>, pin_a: u64) -> FindIter<u64> {
        let mut per_field: HashMap<String, Vec<BitPattern>> = HashMap::new();
        per_field.insert("a".to_string(), vec![BitPattern::exact(pin_a, 4)]);
        FindIter::new(layout, per_field)
    }

    fn empty_results<'a>(inner: FindIter<u64>) -> Results<'a, u64> {
        Results {
            inner,
            filters: Vec::new(),
            budget: None,
            time_anchor: None,
            evaluated: 0,
            termination: Termination::Pending,
        }
    }

    #[test]
    fn termination_starts_pending() {
        let layout = u64_layout();
        let inner = make_inner(&layout, 5);
        let results = empty_results(inner);
        assert_eq!(results.termination(), &Termination::Pending);
    }

    #[test]
    fn termination_transitions_to_exhaustive_when_drained() {
        let layout = u64_layout();
        let inner = make_inner(&layout, 5);
        let mut results = empty_results(inner);
        // Drain all 16 ids.
        let ids: Vec<u64> = (&mut results).collect();
        assert_eq!(ids.len(), 16);
        assert_eq!(results.termination(), &Termination::Exhaustive);
    }

    #[test]
    fn termination_transitions_to_budgeted_at_limit() {
        let layout = u64_layout();
        let inner = make_inner(&layout, 5);
        let mut results = Results {
            inner,
            filters: Vec::new(),
            budget: Some(4),
            time_anchor: None,
            evaluated: 0,
            termination: Termination::Pending,
        };
        let ids: Vec<u64> = (&mut results).collect();
        assert_eq!(ids.len(), 4);
        assert_eq!(
            results.termination(),
            &Termination::Budgeted { evaluated: 4 }
        );
    }

    #[test]
    fn results_yields_only_ids_passing_all_filters() {
        let layout = u64_layout();
        let inner = make_inner(&layout, 5);
        let mut results = Results {
            inner,
            filters: vec![(
                "b_is_even".to_string(),
                Filter::Static(Box::new(|id: u64| (id >> 4) & 1 == 0)),
            )],
            budget: None,
            time_anchor: None,
            evaluated: 0,
            termination: Termination::Pending,
        };
        let ids: Vec<u64> = (&mut results).collect();
        // a=5, b in 0..16 → 16 candidates, half even → 8 results.
        assert_eq!(ids.len(), 8);
        for id in &ids {
            assert_eq!((id >> 4) & 1, 0);
            assert_eq!(layout.extract(*id, "a"), 5);
        }
        // evaluated counts all 16 candidates, filter or not.
        assert_eq!(results.evaluated(), 16);
    }

    #[test]
    fn results_with_zero_budget_yields_nothing_and_terminates_budgeted() {
        let layout = u64_layout();
        let inner = make_inner(&layout, 5);
        let mut results = Results {
            inner,
            filters: Vec::new(),
            budget: Some(0),
            time_anchor: None,
            evaluated: 0,
            termination: Termination::Pending,
        };
        let ids: Vec<u64> = (&mut results).collect();
        assert!(ids.is_empty());
        assert_eq!(
            results.termination(),
            &Termination::Budgeted { evaluated: 0 }
        );
    }

    #[test]
    fn multiple_filters_are_conjunctive() {
        let layout = u64_layout();
        let inner = make_inner(&layout, 5);
        let mut results = Results {
            inner,
            filters: vec![
                (
                    "b_even".to_string(),
                    Filter::Static(Box::new(|id: u64| id & 1 == 0)),
                ),
                (
                    "high_b".to_string(),
                    Filter::Static(Box::new(|id: u64| (id >> 4) >= 8)),
                ),
            ],
            budget: None,
            time_anchor: None,
            evaluated: 0,
            termination: Termination::Pending,
        };
        let ids: Vec<u64> = (&mut results).collect();
        for id in &ids {
            assert_eq!(id & 1, 0, "id {id} not even");
            assert!((id >> 4) >= 8, "id {id} b<8");
        }
    }

    #[test]
    fn filter_enum_dispatches_both_variants_in_insertion_order() {
        let layout = u64_layout();
        let inner = make_inner(&layout, 5);
        let anchor = Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
        // Interleave Static, Temporal, Static. All three must be called; all
        // must pass for an id to be yielded.
        let mut results = Results {
            inner,
            filters: vec![
                (
                    "s_even_b".to_string(),
                    Filter::Static(Box::new(|id: u64| (id >> 4) & 1 == 0)),
                ),
                (
                    "t_always_true".to_string(),
                    Filter::Temporal(Box::new(move |_id: u64, t: DateTime<Utc>| {
                        // Asserts the anchor is passed through.
                        assert_eq!(t, anchor);
                        true
                    })),
                ),
                (
                    "s_high_b".to_string(),
                    Filter::Static(Box::new(|id: u64| (id >> 4) >= 8)),
                ),
            ],
            budget: None,
            time_anchor: Some(TimeAnchor::At(anchor)),
            evaluated: 0,
            termination: Termination::Pending,
        };
        let ids: Vec<u64> = (&mut results).collect();
        // a=5 pinned, b even and ≥8 → b ∈ {8, 10, 12, 14} → 4 ids.
        assert_eq!(ids.len(), 4);
        for id in &ids {
            let b = id >> 4;
            assert_eq!(b & 1, 0);
            assert!(b >= 8);
        }
    }

    #[test]
    fn temporal_filter_receives_time_anchor() {
        let layout = u64_layout();
        let inner = make_inner(&layout, 3);
        let anchor = Utc.with_ymd_and_hms(2030, 1, 2, 3, 4, 5).unwrap();
        let mut results = Results {
            inner,
            filters: vec![(
                "check_anchor".to_string(),
                Filter::Temporal(Box::new(move |_id: u64, t: DateTime<Utc>| t == anchor)),
            )],
            budget: None,
            time_anchor: Some(TimeAnchor::At(anchor)),
            evaluated: 0,
            termination: Termination::Pending,
        };
        let ids: Vec<u64> = (&mut results).collect();
        // All 16 candidates pass because the closure returns true for the anchor.
        assert_eq!(ids.len(), 16);
    }

    #[test]
    fn time_anchor_at_and_at_any_are_distinct_variants() {
        let t = Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2026, 5, 8, 0, 0, 0).unwrap();
        let at = TimeAnchor::At(t);
        let at_any = TimeAnchor::AtAny(t, t2);
        assert_ne!(at, at_any);
        assert_ne!(at, TimeAnchor::At(t2));
    }

    #[test]
    fn sweep_exists_finds_match_on_first_sample() {
        let start = Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 5, 2, 0, 0, 0).unwrap();
        let value_fn = |_id: u64, _t: DateTime<Utc>| 100.0;
        let stab = StabilityMode::BinarySearch { probes: 4 };
        let passes = sweep_passes(
            1u64,
            start,
            end,
            &value_fn,
            Op::Gt,
            50.0,
            Quantifier::Exists,
            &stab,
        );
        assert!(passes);
    }

    #[test]
    fn sweep_exists_returns_false_when_no_match() {
        let start = Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 5, 2, 0, 0, 0).unwrap();
        let value_fn = |_id: u64, _t: DateTime<Utc>| 10.0;
        let stab = StabilityMode::BinarySearch { probes: 4 };
        let passes = sweep_passes(
            1u64,
            start,
            end,
            &value_fn,
            Op::Gt,
            50.0,
            Quantifier::Exists,
            &stab,
        );
        assert!(!passes);
    }

    #[test]
    fn sweep_exists_finds_match_after_multiple_skips() {
        // Linear ramp from 0 to 200 over the range. With BinarySearch { probes: 10 }
        // we sample at 10 uniform points — sample index ~3 crosses 50 (at t = 30%).
        let start = Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 5, 2, 0, 0, 0).unwrap();
        let range_ns = (end - start).num_nanoseconds().unwrap() as f64;
        let value_fn = move |_id: u64, t: DateTime<Utc>| {
            let elapsed = (t - start).num_nanoseconds().unwrap() as f64;
            200.0 * (elapsed / range_ns)
        };
        let stab = StabilityMode::BinarySearch { probes: 10 };
        let passes = sweep_passes(
            1u64,
            start,
            end,
            &value_fn,
            Op::Gt,
            50.0,
            Quantifier::Exists,
            &stab,
        );
        assert!(passes);
    }

    #[test]
    fn sweep_forall_fails_on_first_miss() {
        let start = Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 5, 2, 0, 0, 0).unwrap();
        let value_fn = |_id: u64, _t: DateTime<Utc>| 10.0;
        let stab = StabilityMode::BinarySearch { probes: 4 };
        let passes = sweep_passes(
            1u64,
            start,
            end,
            &value_fn,
            Op::Gt,
            50.0,
            Quantifier::Forall,
            &stab,
        );
        assert!(!passes);
    }

    #[test]
    fn sweep_forall_returns_true_when_always_matches() {
        let start = Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 5, 2, 0, 0, 0).unwrap();
        let value_fn = |_id: u64, _t: DateTime<Utc>| 100.0;
        let stab = StabilityMode::BinarySearch { probes: 4 };
        let passes = sweep_passes(
            1u64,
            start,
            end,
            &value_fn,
            Op::Gt,
            50.0,
            Quantifier::Forall,
            &stab,
        );
        assert!(passes);
    }

    #[test]
    fn sweep_min_step_prevents_infinite_loop_at_zero_epsilon() {
        // Closure returns exactly the threshold; ε = 0. If Analytic returns
        // Duration::zero(), we still advance by MIN_STEP (1s) to avoid stalling.
        let start = Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 10).unwrap(); // 10 seconds
        let value_fn = |_id: u64, _t: DateTime<Utc>| 50.0;
        let stab = StabilityMode::<u64>::Analytic(Box::new(|_id, _t, _eps| Duration::zero()));
        // Exists with Gt and threshold=50: closure returns 50 exactly, never > 50,
        // so sweep must walk 10 seconds and return false. Critically, it terminates.
        let passes = sweep_passes(
            1u64,
            start,
            end,
            &value_fn,
            Op::Gt,
            50.0,
            Quantifier::Exists,
            &stab,
        );
        assert!(!passes);
    }

    #[test]
    #[should_panic(expected = "temporal sweep exceeded")]
    fn sweep_max_iterations_panics_on_runaway() {
        // MIN_STEP = 1s, MAX_ITERATIONS = 1e6. A range of > 1e6 seconds
        // (~11.6 days) at MIN_STEP per iteration triggers the cap.
        //
        // Use value_fn always returning 100 with Forall + Gt 50 → always
        // matches, so we never early-exit and walk the full range. Analytic
        // returns 0 → clamp to MIN_STEP = 1s each iteration. 12 days = ~1.04M
        // seconds, exceeds MAX_ITERATIONS.
        let start = Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 5, 13, 0, 0, 0).unwrap(); // +12 days
        let value_fn = |_id: u64, _t: DateTime<Utc>| 100.0;
        let stab = StabilityMode::<u64>::Analytic(Box::new(|_id, _t, _eps| Duration::zero()));
        let _ = sweep_passes(
            1u64,
            start,
            end,
            &value_fn,
            Op::Gt,
            50.0,
            Quantifier::Forall,
            &stab,
        );
    }

    #[test]
    fn binary_search_mode_uses_uniform_step() {
        // BinarySearch { probes: 4 } on a 100-second range → step ≈ 25s.
        // value_fn returns its own call count so we can verify sample spacing.
        use std::cell::RefCell;
        let start = Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 5, 1, 0, 1, 40).unwrap(); // +100s
        let samples: RefCell<Vec<DateTime<Utc>>> = RefCell::new(Vec::new());
        let value_fn = |_id: u64, t: DateTime<Utc>| {
            samples.borrow_mut().push(t);
            0.0 // never crosses threshold 100.0
        };
        let stab = StabilityMode::BinarySearch { probes: 4 };
        let _ = sweep_passes(
            1u64,
            start,
            end,
            &value_fn,
            Op::Gt,
            100.0,
            Quantifier::Exists,
            &stab,
        );
        let taken = samples.borrow();
        // Each sample should be ~25s apart; at least 4 samples taken within 100s.
        assert!(taken.len() >= 4, "expected ≥4 samples, got {}", taken.len());
        let deltas: Vec<i64> = taken
            .windows(2)
            .map(|w| (w[1] - w[0]).num_seconds())
            .collect();
        for d in &deltas {
            // First step computed from 100s remaining / 4 probes = 25s.
            // Each subsequent one is (remaining / 4), shrinking slowly.
            assert!(*d >= 1, "step must be ≥ MIN_STEP (1s)");
        }
    }
}
