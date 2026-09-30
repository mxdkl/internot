//! Typestate query builder for `Space::find()`.
//!
//! `Space::find()` returns [`Query<W, Unbounded>`]. A `where_*` predicate
//! transitions to [`Query<W, Bounded>`]. Only `Bounded` exposes `execute()`,
//! enforcing "at least one indexable predicate" at compile time.

use crate::bits::BitLayout;
use crate::search::{
    range_to_prefixes, BitPattern, Filter, Op, Quantifier, StabilityMode, TimeAnchor,
};
use crate::space::Space;
use crate::word::BitWord;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::marker::PhantomData;

/// Marker: no indexable predicate yet. `execute()` is not callable.
pub struct Unbounded;

/// Marker: at least one indexable predicate added. `execute()` is callable.
pub struct Bounded;

/// Builder that collects predicates and composes them into a `FindIter`.
pub struct Query<'a, W: BitWord, State> {
    pub(crate) space: &'a Space<W>,
    pub(crate) per_field: HashMap<String, Vec<BitPattern>>,
    pub(crate) filters: Vec<(String, Filter<'a, W>)>,
    pub(crate) budget: Option<usize>,
    pub(crate) time_anchor: Option<TimeAnchor>,
    _state: PhantomData<State>,
}

impl<'a, W: BitWord> Query<'a, W, Unbounded> {
    pub(crate) fn new(space: &'a Space<W>) -> Self {
        Query {
            space,
            per_field: HashMap::new(),
            filters: Vec::new(),
            budget: None,
            time_anchor: None,
            _state: PhantomData,
        }
    }

    pub fn where_eq(self, field: &str, value: u64) -> Query<'a, W, Bounded> {
        add_eq(self, field, value)
    }

    pub fn where_range(self, field: &str, lo: u64, hi: u64) -> Query<'a, W, Bounded> {
        add_range(self, field, lo, hi)
    }

    pub fn where_in(self, field: &str, values: &[u64]) -> Query<'a, W, Bounded> {
        add_in(self, field, values)
    }
    // .execute() intentionally NOT defined here — typestate guard.
}

impl<'a, W: BitWord> Query<'a, W, Bounded> {
    pub fn where_eq(self, field: &str, value: u64) -> Self {
        add_eq(self, field, value)
    }

    pub fn where_range(self, field: &str, lo: u64, hi: u64) -> Self {
        add_range(self, field, lo, hi)
    }

    pub fn where_in(self, field: &str, values: &[u64]) -> Self {
        add_in(self, field, values)
    }

    /// Attach a post-enumeration filter. The closure receives candidate IDs
    /// that passed all indexable predicates; only IDs for which the closure
    /// returns `true` are yielded.
    ///
    /// Filters apply in the order they were added (no selectivity reordering
    /// in v0.1). Multiple filters are combined with AND.
    pub fn filter_static<F>(mut self, name: &str, pred: F) -> Self
    where
        F: Fn(W) -> bool + 'a,
    {
        self.filters
            .push((name.to_string(), Filter::Static(Box::new(pred))));
        self
    }

    /// Cap the number of candidates drawn from the underlying enumeration.
    /// After `n` candidates have been drawn (filter pass or fail), the
    /// iterator stops and reports `Termination::Budgeted { evaluated: n }`.
    pub fn scan_budget(mut self, n: usize) -> Self {
        self.budget = Some(n);
        self
    }

    /// Set the time anchor used by later `filter_temporal` evaluation.
    /// Only the last call takes effect. `at(t)` alone has no effect on
    /// the query result unless a `filter_temporal` is also attached.
    pub fn at(mut self, t: DateTime<Utc>) -> Self {
        self.time_anchor = Some(TimeAnchor::At(t));
        self
    }

    /// Attach a post-enumeration filter whose closure takes both the
    /// candidate ID and the time anchor set by `at(t)`. Evaluated in
    /// insertion order with any other filters (static or temporal).
    ///
    /// Panics at `execute()` if `at(t)` was not called before then.
    pub fn filter_temporal<F>(mut self, name: &str, pred: F) -> Self
    where
        F: Fn(W, DateTime<Utc>) -> bool + 'a,
    {
        self.filters
            .push((name.to_string(), Filter::Temporal(Box::new(pred))));
        self
    }

    /// Set the time anchor to a half-open range `[start, end)`. Pairs with
    /// `filter_exists_in_range` / `filter_forall_in_range`. Last call wins
    /// across `at()` and `at_any()`.
    pub fn at_any(mut self, start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        self.time_anchor = Some(TimeAnchor::AtAny(start, end));
        self
    }

    /// Attach a temporal range filter. Id passes iff there exists `t` in
    /// the `at_any` range where `value_fn(id, t) op threshold`.
    ///
    /// Panics at `execute()` if `at_any(...)` was not called before then.
    pub fn filter_exists_in_range<F>(
        mut self,
        name: &str,
        value_fn: F,
        op: Op,
        threshold: f64,
        stability: StabilityMode<'a, W>,
    ) -> Self
    where
        F: Fn(W, DateTime<Utc>) -> f64 + 'a,
    {
        self.filters.push((
            name.to_string(),
            Filter::TemporalRange {
                value_fn: Box::new(value_fn),
                op,
                threshold,
                quantifier: Quantifier::Exists,
                stability,
            },
        ));
        self
    }

    /// Attach a temporal range filter. Id passes iff for all `t` sampled
    /// in the `at_any` range, `value_fn(id, t) op threshold`.
    ///
    /// Panics at `execute()` if `at_any(...)` was not called before then.
    pub fn filter_forall_in_range<F>(
        mut self,
        name: &str,
        value_fn: F,
        op: Op,
        threshold: f64,
        stability: StabilityMode<'a, W>,
    ) -> Self
    where
        F: Fn(W, DateTime<Utc>) -> f64 + 'a,
    {
        self.filters.push((
            name.to_string(),
            Filter::TemporalRange {
                value_fn: Box::new(value_fn),
                op,
                threshold,
                quantifier: Quantifier::Forall,
                stability,
            },
        ));
        self
    }

    /// Consume the query and return a `Cohort<W>` — a bitmask-described
    /// set of IDs supporting `contains()` and `sample()` without
    /// enumeration. Filters attached to the query are NOT carried into
    /// the Cohort in v1; cohorts are pure bit-pattern sets. For
    /// post-pattern filtering use `.execute().into_iter().filter(...)`
    /// instead.
    ///
    /// Multi-pattern fields (from `where_in`) are also not yet
    /// supported by `into_cohort` — use the smallest pattern slot
    /// (the first pattern in the cursor) or restructure the query.
    pub fn into_cohort(self) -> crate::search::Cohort<W> {
        let layout = self.space.layout();
        let mut patterns_per_field: Vec<(u32, u32, BitPattern)> =
            Vec::with_capacity(self.per_field.len());
        for (field_name, patterns) in &self.per_field {
            let (offset, width) = layout
                .field_offset_width(field_name)
                .expect("query field exists in layout (constructor would have panicked otherwise)");
            assert!(
                patterns.len() <= 1,
                "Cohort v1 supports at most one BitPattern per field; \
                 multi-pattern (where_in / where_range with disjoint prefixes) \
                 cohorts not yet implemented for field '{field_name}'"
            );
            let pat = patterns
                .first()
                .copied()
                .unwrap_or_else(|| BitPattern::any(width as u8));
            patterns_per_field.push((offset, width, pat));
        }
        crate::search::Cohort {
            patterns_per_field,
            _word: std::marker::PhantomData,
        }
    }

    /// Consume the query and return an iterator over matching IDs.
    pub fn execute(self) -> crate::search::Results<'a, W> {
        let has_temporal_instant = self
            .filters
            .iter()
            .any(|(_, f)| matches!(f, Filter::Temporal(_)));
        let has_temporal_range = self
            .filters
            .iter()
            .any(|(_, f)| matches!(f, Filter::TemporalRange { .. }));
        match &self.time_anchor {
            None => {
                if has_temporal_instant {
                    panic!(
                        "filter_temporal requires a time anchor — call .at(t) before .execute()"
                    );
                }
                if has_temporal_range {
                    panic!(
                        "filter_*_in_range requires a time range — call .at_any(start, end) before .execute()"
                    );
                }
            }
            Some(TimeAnchor::At(_)) => {
                if has_temporal_range {
                    panic!(
                        ".at_any(start, end) required for filter_*_in_range — .at(t) is only for filter_temporal"
                    );
                }
            }
            Some(TimeAnchor::AtAny(start, end)) => {
                if start >= end {
                    panic!(".at_any requires start < end");
                }
                if has_temporal_instant {
                    panic!(
                        ".at(t) required for filter_temporal — .at_any(start, end) is only for filter_*_in_range"
                    );
                }
            }
        }
        let inner = crate::search::FindIter::new(self.space.layout(), self.per_field);
        crate::search::Results {
            inner,
            filters: self.filters,
            budget: self.budget,
            time_anchor: self.time_anchor,
            evaluated: 0,
            termination: crate::search::Termination::Pending,
        }
    }
}

// Generic helpers that add a predicate and transition/preserve state.
fn add_eq<'a, W: BitWord, S, NewS>(
    q: Query<'a, W, S>,
    field: &str,
    value: u64,
) -> Query<'a, W, NewS> {
    let (_offset, width) = field_width_or_panic(q.space.layout(), field);
    let pattern = BitPattern::exact(value, width as u8);
    add_pattern(q, field, vec![pattern])
}

fn add_range<'a, W: BitWord, S, NewS>(
    q: Query<'a, W, S>,
    field: &str,
    lo: u64,
    hi: u64,
) -> Query<'a, W, NewS> {
    let (_offset, width) = field_width_or_panic(q.space.layout(), field);
    let patterns = range_to_prefixes(lo, hi, width as u8);
    add_pattern(q, field, patterns)
}

fn add_in<'a, W: BitWord, S, NewS>(
    q: Query<'a, W, S>,
    field: &str,
    values: &[u64],
) -> Query<'a, W, NewS> {
    let (_offset, width) = field_width_or_panic(q.space.layout(), field);
    // Use one exact pattern per distinct value rather than minimize_patterns.
    // Quine-McCluskey can emit overlapping prime implicants (e.g., {0,4,8}
    // produces patterns covering {0,4} and {0,8} — both containing 0), which
    // would make FindIter yield duplicate IDs. Exact patterns are disjoint
    // by construction at the cost of more cursor chaining.
    let mut seen = std::collections::HashSet::new();
    let mut patterns = Vec::new();
    for &v in values {
        if seen.insert(v) {
            patterns.push(BitPattern::exact(v, width as u8));
        }
    }
    add_pattern(q, field, patterns)
}

fn add_pattern<'a, W: BitWord, S, NewS>(
    mut q: Query<'a, W, S>,
    field: &str,
    patterns: Vec<BitPattern>,
) -> Query<'a, W, NewS> {
    if q.per_field.contains_key(field) {
        panic!(
            "field {field:?} already has a predicate in this query (v0.1 only supports one predicate per field)"
        );
    }
    q.per_field.insert(field.to_string(), patterns);
    Query {
        space: q.space,
        per_field: q.per_field,
        filters: q.filters,
        budget: q.budget,
        time_anchor: q.time_anchor,
        _state: PhantomData,
    }
}

fn field_width_or_panic<W: BitWord>(layout: &BitLayout<W>, field: &str) -> (u32, u32) {
    layout
        .field_offset_width(field)
        .unwrap_or_else(|| panic!("unknown field: {field:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bits::BitLayout;
    use chrono::TimeZone;

    fn make_space() -> Space<u64> {
        let layout = BitLayout::<u64>::new(vec![("locale", 8), ("age", 8), ("hash", 48)]).unwrap();
        Space::<u64>::new("people", layout)
    }

    #[test]
    fn where_eq_records_exact_pattern() {
        let space = make_space();
        let q = space.find().where_eq("locale", 5);
        assert_eq!(q.per_field.len(), 1);
        assert_eq!(q.per_field["locale"].len(), 1);
        assert_eq!(q.per_field["locale"][0], BitPattern::exact(5, 8));
    }

    #[test]
    fn where_range_records_prefix_cover() {
        let space = make_space();
        let q = space.find().where_range("age", 20, 30);
        assert_eq!(q.per_field.len(), 1);
        assert!(!q.per_field["age"].is_empty());
        for v in 20..=30u64 {
            assert!(q.per_field["age"].iter().any(|p| p.matches(v)));
        }
    }

    #[test]
    fn where_in_records_minimized_cover() {
        let space = make_space();
        let q = space.find().where_in("locale", &[1, 3, 5, 7]);
        assert_eq!(q.per_field.len(), 1);
        for v in [1u64, 3, 5, 7] {
            assert!(q.per_field["locale"].iter().any(|p| p.matches(v)));
        }
    }

    #[test]
    fn chained_where_preserves_bounded_state() {
        let space = make_space();
        // This compiles → Bounded state supports chaining.
        let q = space
            .find()
            .where_eq("locale", 5)
            .where_range("age", 20, 30);
        assert_eq!(q.per_field.len(), 2);
    }

    #[test]
    #[should_panic(expected = "unknown field")]
    fn unknown_field_panics() {
        let space = make_space();
        let _ = space.find().where_eq("nope", 5);
    }

    #[test]
    #[should_panic(expected = "already has a predicate")]
    fn duplicate_field_predicate_panics() {
        let space = make_space();
        let _ = space.find().where_eq("locale", 5).where_eq("locale", 6);
    }

    #[test]
    #[should_panic(expected = "lo must be ≤ hi")]
    fn where_range_inverted_panics() {
        let space = make_space();
        let _ = space.find().where_range("age", 30, 20);
    }

    #[test]
    #[should_panic(expected = "hi must fit")]
    fn where_range_out_of_domain_panics() {
        let space = make_space();
        // age is 8 bits → max 255
        let _ = space.find().where_range("age", 0, 256);
    }

    #[test]
    #[should_panic(expected = "bits outside")]
    fn where_in_out_of_domain_panics() {
        let space = make_space();
        let _ = space.find().where_in("age", &[1, 256]);
    }

    #[test]
    fn where_eq_on_unbounded_transitions_to_bounded() {
        let space = make_space();
        let q = space.find().where_eq("locale", 5);
        // Fact: next line would not compile if q were Unbounded; typestate
        // enforces execute()'s absence there. Here we only check that
        // chaining to Bounded still works (self-returning where_eq).
        let q = q.where_eq("age", 30);
        assert_eq!(q.per_field.len(), 2);
    }

    #[test]
    fn execute_on_bounded_query_yields_matching_ids() {
        let space = make_space();
        let ids: Vec<u64> = space
            .find()
            .where_eq("locale", 3)
            .where_eq("age", 42)
            .execute()
            .take(5)
            .collect();
        assert_eq!(ids.len(), 5);
        for id in &ids {
            assert_eq!(space.layout().extract(*id, "locale"), 3);
            assert_eq!(space.layout().extract(*id, "age"), 42);
        }
    }

    #[test]
    fn filter_static_attaches_filter_and_yields_only_matching_ids() {
        // Use a small layout so the unconstrained bits don't explode enumeration.
        let layout = BitLayout::<u64>::new(vec![("a", 4), ("b", 4)]).unwrap();
        let space = Space::<u64>::new("tiny", layout);
        let ids: Vec<u64> = space
            .find()
            .where_eq("a", 3)
            .filter_static("b_high", |id| space.layout().extract(id, "b") >= 10)
            .execute()
            .collect();
        // a=3 pinned, b in 0..16 → 16 candidates. Filter passes for b ∈ {10..=15} → 6 results.
        assert_eq!(ids.len(), 6);
        for id in &ids {
            assert_eq!(space.layout().extract(*id, "a"), 3);
            assert!(space.layout().extract(*id, "b") >= 10);
        }
    }

    #[test]
    fn scan_budget_caps_evaluated_count() {
        let space = make_space();
        let mut results = space.find().where_eq("locale", 3).scan_budget(10).execute();
        let _ids: Vec<u64> = (&mut results).collect();
        assert_eq!(results.evaluated(), 10);
        assert_eq!(
            results.termination(),
            &crate::search::Termination::Budgeted { evaluated: 10 }
        );
    }

    #[test]
    fn multiple_filter_static_are_conjunctive() {
        let layout = BitLayout::<u64>::new(vec![("a", 4), ("b", 4)]).unwrap();
        let space = Space::<u64>::new("tiny", layout);
        let ids: Vec<u64> = space
            .find()
            .where_eq("a", 3)
            .filter_static("b_gt_5", |id| space.layout().extract(id, "b") > 5)
            .filter_static("b_lt_12", |id| space.layout().extract(id, "b") < 12)
            .execute()
            .collect();
        // a=3 pinned, b in 0..16. Filter passes for b ∈ {6, 7, 8, 9, 10, 11} → 6 results.
        assert_eq!(ids.len(), 6);
        for id in &ids {
            let b = space.layout().extract(*id, "b");
            assert!(b > 5);
            assert!(b < 12);
        }
    }

    #[test]
    fn scan_budget_before_where_is_preserved() {
        let space = make_space();
        let mut results = space
            .find()
            .where_eq("locale", 3)
            .scan_budget(5)
            .where_range("age", 10, 50)
            .execute();
        let _ids: Vec<u64> = (&mut results).collect();
        assert_eq!(results.evaluated(), 5);
    }

    #[test]
    fn at_sets_time_anchor() {
        let space = make_space();
        let anchor = chrono::Utc.with_ymd_and_hms(2026, 5, 1, 9, 0, 0).unwrap();
        let q = space.find().where_eq("locale", 3).at(anchor);
        assert_eq!(q.time_anchor, Some(TimeAnchor::At(anchor)));
    }

    #[test]
    fn at_last_call_wins() {
        let space = make_space();
        let t1 = chrono::Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let t2 = chrono::Utc
            .with_ymd_and_hms(2026, 12, 31, 23, 59, 59)
            .unwrap();
        let q = space.find().where_eq("locale", 3).at(t1).at(t2);
        assert_eq!(q.time_anchor, Some(TimeAnchor::At(t2)));
    }

    #[test]
    #[should_panic(expected = "filter_temporal requires a time anchor")]
    fn filter_temporal_without_at_panics_at_execute() {
        let space = make_space();
        let _ = space
            .find()
            .where_eq("locale", 3)
            .filter_temporal("always_true", |_id, _t| true)
            .execute();
    }

    #[test]
    fn filter_temporal_with_at_evaluates_closure_with_anchor() {
        let layout = BitLayout::<u64>::new(vec![("a", 4), ("b", 4)]).unwrap();
        let space = Space::<u64>::new("tiny", layout);
        let anchor = chrono::Utc.with_ymd_and_hms(2026, 5, 1, 9, 0, 0).unwrap();
        let ids: Vec<u64> = space
            .find()
            .where_eq("a", 3)
            .at(anchor)
            .filter_temporal("anchor_match", move |_id, t| t == anchor)
            .execute()
            .collect();
        // a=3 pinned, b in 0..16; closure always true at the anchor → 16 ids.
        assert_eq!(ids.len(), 16);
    }

    #[test]
    fn filter_temporal_yields_only_matching_ids() {
        let layout = BitLayout::<u64>::new(vec![("a", 4), ("b", 4)]).unwrap();
        let space = Space::<u64>::new("tiny", layout);
        let anchor = chrono::Utc.with_ymd_and_hms(2026, 5, 1, 9, 0, 0).unwrap();
        let ids: Vec<u64> = space
            .find()
            .where_eq("a", 3)
            .at(anchor)
            .filter_temporal("b_gt_5_at_anchor", move |id, t| {
                assert_eq!(t, anchor);
                (id >> 4) > 5
            })
            .execute()
            .collect();
        // a=3 pinned, b in 0..16, keep b ∈ {6..=15} → 10 ids.
        assert_eq!(ids.len(), 10);
        for id in &ids {
            assert!((id >> 4) > 5);
        }
    }

    #[test]
    fn mixed_static_and_temporal_filters_compose_in_insertion_order() {
        let layout = BitLayout::<u64>::new(vec![("a", 4), ("b", 4)]).unwrap();
        let space = Space::<u64>::new("tiny", layout);
        let anchor = chrono::Utc.with_ymd_and_hms(2026, 5, 1, 9, 0, 0).unwrap();
        let ids: Vec<u64> = space
            .find()
            .where_eq("a", 3)
            .at(anchor)
            .filter_static("b_even", |id| (id >> 4) & 1 == 0)
            .filter_temporal("b_ge_8_at_anchor", move |id, t| {
                assert_eq!(t, anchor);
                (id >> 4) >= 8
            })
            .filter_static("b_lt_14", |id| (id >> 4) < 14)
            .execute()
            .collect();
        // a=3 pinned, b even AND b≥8 AND b<14 → b ∈ {8, 10, 12} → 3 ids.
        assert_eq!(ids.len(), 3);
        for id in &ids {
            let b = id >> 4;
            assert_eq!(b & 1, 0);
            assert!(b >= 8);
            assert!(b < 14);
        }
    }

    #[test]
    fn scan_budget_caps_temporal_evaluations() {
        let space = make_space();
        let anchor = chrono::Utc.with_ymd_and_hms(2026, 5, 1, 9, 0, 0).unwrap();
        let mut results = space
            .find()
            .where_eq("locale", 3)
            .at(anchor)
            .scan_budget(7)
            .filter_temporal("always_true", move |_id, t| {
                assert_eq!(t, anchor);
                true
            })
            .execute();
        let _ids: Vec<u64> = (&mut results).collect();
        assert_eq!(results.evaluated(), 7);
        assert_eq!(
            results.termination(),
            &crate::search::Termination::Budgeted { evaluated: 7 }
        );
    }

    #[test]
    fn at_any_sets_time_anchor_at_any() {
        let space = make_space();
        let s = chrono::Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
        let e = chrono::Utc.with_ymd_and_hms(2026, 5, 8, 0, 0, 0).unwrap();
        let q = space.find().where_eq("locale", 3).at_any(s, e);
        assert_eq!(q.time_anchor, Some(TimeAnchor::AtAny(s, e)));
    }

    #[test]
    fn at_any_then_at_overwrites_to_at() {
        let space = make_space();
        let s = chrono::Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
        let e = chrono::Utc.with_ymd_and_hms(2026, 5, 8, 0, 0, 0).unwrap();
        let t = chrono::Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap();
        let q = space.find().where_eq("locale", 3).at_any(s, e).at(t);
        assert_eq!(q.time_anchor, Some(TimeAnchor::At(t)));
    }

    #[test]
    #[should_panic(expected = ".at_any requires start < end")]
    fn at_any_with_start_ge_end_panics_at_execute() {
        let space = make_space();
        let s = chrono::Utc.with_ymd_and_hms(2026, 5, 8, 0, 0, 0).unwrap();
        let e = chrono::Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
        let _ = space
            .find()
            .where_eq("locale", 3)
            .at_any(s, e)
            .filter_exists_in_range(
                "noop",
                |_id, _t| 0.0,
                Op::Gt,
                0.0,
                StabilityMode::BinarySearch { probes: 4 },
            )
            .execute();
    }

    #[test]
    #[should_panic(expected = "filter_*_in_range requires a time range")]
    fn filter_exists_in_range_without_at_any_panics_at_execute() {
        let space = make_space();
        let _ = space
            .find()
            .where_eq("locale", 3)
            .filter_exists_in_range(
                "noop",
                |_id, _t| 0.0,
                Op::Gt,
                0.0,
                StabilityMode::BinarySearch { probes: 4 },
            )
            .execute();
    }

    #[test]
    #[should_panic(expected = "filter_*_in_range requires a time range")]
    fn filter_forall_in_range_without_at_any_panics_at_execute() {
        let space = make_space();
        let _ = space
            .find()
            .where_eq("locale", 3)
            .filter_forall_in_range(
                "noop",
                |_id, _t| 0.0,
                Op::Gt,
                0.0,
                StabilityMode::BinarySearch { probes: 4 },
            )
            .execute();
    }

    #[test]
    #[should_panic(expected = ".at(t) required for filter_temporal")]
    fn filter_temporal_with_at_any_panics_at_execute() {
        let space = make_space();
        let s = chrono::Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
        let e = chrono::Utc.with_ymd_and_hms(2026, 5, 8, 0, 0, 0).unwrap();
        let _ = space
            .find()
            .where_eq("locale", 3)
            .at_any(s, e)
            .filter_temporal("always_true", |_id, _t| true)
            .execute();
    }

    #[test]
    #[should_panic(expected = ".at_any(start, end) required for filter_*_in_range")]
    fn filter_exists_in_range_with_at_panics_at_execute() {
        let space = make_space();
        let t = chrono::Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
        let _ = space
            .find()
            .where_eq("locale", 3)
            .at(t)
            .filter_exists_in_range(
                "noop",
                |_id, _t| 0.0,
                Op::Gt,
                0.0,
                StabilityMode::BinarySearch { probes: 4 },
            )
            .execute();
    }

    #[test]
    fn filter_exists_in_range_binary_search_mode_yields_expected_ids() {
        let layout = BitLayout::<u64>::new(vec![("a", 4), ("b", 4)]).unwrap();
        let space = Space::<u64>::new("tiny", layout);
        let s = chrono::Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
        let e = chrono::Utc.with_ymd_and_hms(2026, 5, 2, 0, 0, 0).unwrap();
        // value_fn returns b (upper nibble). With a=3 pinned, b ∈ 0..16.
        // Filter: ∃ t where b > 7 → passes for b ∈ {8..=15} = 8 ids.
        let ids: Vec<u64> = space
            .find()
            .where_eq("a", 3)
            .at_any(s, e)
            .filter_exists_in_range(
                "b_gt_7",
                |id, _t| ((id >> 4) & 0xF) as f64,
                Op::Gt,
                7.0,
                StabilityMode::BinarySearch { probes: 2 },
            )
            .execute()
            .collect();
        assert_eq!(ids.len(), 8);
        for id in &ids {
            assert!((id >> 4) > 7);
        }
    }

    #[test]
    fn filter_exists_in_range_analytic_mode_yields_expected_ids() {
        use chrono::Duration;
        let layout = BitLayout::<u64>::new(vec![("a", 4), ("b", 4)]).unwrap();
        let space = Space::<u64>::new("tiny", layout);
        let s = chrono::Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
        let e = chrono::Utc.with_ymd_and_hms(2026, 5, 2, 0, 0, 0).unwrap();
        let ids: Vec<u64> = space
            .find()
            .where_eq("a", 3)
            .at_any(s, e)
            .filter_exists_in_range(
                "b_gt_7",
                |id, _t| ((id >> 4) & 0xF) as f64,
                Op::Gt,
                7.0,
                // Constant value fn → any Analytic giving a non-zero skip is fine.
                StabilityMode::Analytic(Box::new(|_id, _t, _eps| Duration::hours(6))),
            )
            .execute()
            .collect();
        assert_eq!(ids.len(), 8);
        for id in &ids {
            assert!((id >> 4) > 7);
        }
    }

    #[test]
    fn filter_forall_in_range_yields_only_always_matching_ids() {
        let layout = BitLayout::<u64>::new(vec![("a", 4), ("b", 4)]).unwrap();
        let space = Space::<u64>::new("tiny", layout);
        let s = chrono::Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
        let e = chrono::Utc.with_ymd_and_hms(2026, 5, 2, 0, 0, 0).unwrap();
        // value_fn is constant = b. Forall: b > 7 holds iff b > 7 at all sample points;
        // since value is constant, same ids as exists (b ∈ {8..=15}) → 8 ids.
        let ids: Vec<u64> = space
            .find()
            .where_eq("a", 3)
            .at_any(s, e)
            .filter_forall_in_range(
                "b_gt_7",
                |id, _t| ((id >> 4) & 0xF) as f64,
                Op::Gt,
                7.0,
                StabilityMode::BinarySearch { probes: 4 },
            )
            .execute()
            .collect();
        assert_eq!(ids.len(), 8);
        for id in &ids {
            assert!((id >> 4) > 7);
        }
    }

    #[test]
    fn mixed_static_and_temporal_range_filters_compose_in_order() {
        let layout = BitLayout::<u64>::new(vec![("a", 4), ("b", 4)]).unwrap();
        let space = Space::<u64>::new("tiny", layout);
        let s = chrono::Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
        let e = chrono::Utc.with_ymd_and_hms(2026, 5, 2, 0, 0, 0).unwrap();
        let ids: Vec<u64> = space
            .find()
            .where_eq("a", 3)
            .at_any(s, e)
            .filter_static("b_even", |id| (id >> 4) & 1 == 0)
            .filter_exists_in_range(
                "b_ge_8",
                |id, _t| ((id >> 4) & 0xF) as f64,
                Op::Ge,
                8.0,
                StabilityMode::BinarySearch { probes: 2 },
            )
            .execute()
            .collect();
        // a=3 pinned, b even AND b ≥ 8 → b ∈ {8, 10, 12, 14} → 4 ids.
        assert_eq!(ids.len(), 4);
        for id in &ids {
            let b = id >> 4;
            assert_eq!(b & 1, 0);
            assert!(b >= 8);
        }
    }

    #[test]
    fn scan_budget_caps_temporal_range_evaluations() {
        let space = make_space();
        let s = chrono::Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
        let e = chrono::Utc.with_ymd_and_hms(2026, 5, 2, 0, 0, 0).unwrap();
        let mut results = space
            .find()
            .where_eq("locale", 3)
            .at_any(s, e)
            .scan_budget(7)
            .filter_exists_in_range(
                "always_below",
                |_id, _t| 0.0,
                Op::Gt,
                1.0,
                StabilityMode::BinarySearch { probes: 2 },
            )
            .execute();
        let _ids: Vec<u64> = (&mut results).collect();
        assert_eq!(results.evaluated(), 7);
        assert_eq!(
            results.termination(),
            &crate::search::Termination::Budgeted { evaluated: 7 }
        );
    }
}
