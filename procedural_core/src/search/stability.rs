//! Composition helpers for analytic stability radii.
//!
//! When a value function is a sum of trajectory primitives, each with its
//! own `stability_radius(id, t, ε)`, the sum's conservative stability
//! radius is obtained by splitting ε evenly and taking the minimum δ.

use crate::word::BitWord;
use chrono::{DateTime, Duration, Utc};

/// Conservative stability radius for a value function of the form
/// `f(id, t) = Σ f_i(id, t)` where each `f_i` has a known stability radius
/// `stab_i(id, t, ε) → δ`. Returns `min_i stab_i(id, t, ε/n)` where `n` is
/// the number of components.
///
/// Rationale: if `|f_i(t') − f_i(t)| ≤ ε/n` for each `i` when `|t' − t| ≤ δ`,
/// then by triangle inequality `|f(t') − f(t)| ≤ n · (ε/n) = ε`. Splitting ε
/// evenly is conservative; sensitivity-weighted splitting could yield tighter
/// δ but is out of scope for v0.1.
///
/// Empty `components` returns a 10-year sentinel (no skip constraint).
pub fn min_of<W: BitWord>(
    components: &[&dyn Fn(W, DateTime<Utc>, f64) -> Duration],
    id: W,
    t: DateTime<Utc>,
    epsilon: f64,
) -> Duration {
    if components.is_empty() {
        return Duration::days(3650);
    }
    let per_component = epsilon / components.len() as f64;
    components
        .iter()
        .map(|f| f(id, t, per_component))
        .min()
        .expect("components is non-empty — checked above")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn t() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap()
    }

    #[test]
    fn min_of_empty_returns_sentinel() {
        let components: [&dyn Fn(u64, DateTime<Utc>, f64) -> Duration; 0] = [];
        let delta = min_of(&components, 1u64, t(), 1.0);
        assert_eq!(delta, Duration::days(3650));
    }

    #[test]
    fn min_of_single_component_passes_full_epsilon() {
        use std::cell::Cell;
        let seen: Cell<f64> = Cell::new(-1.0);
        let f = |_id: u64, _t: DateTime<Utc>, eps: f64| {
            seen.set(eps);
            Duration::hours(1)
        };
        let components: &[&dyn Fn(u64, DateTime<Utc>, f64) -> Duration] = &[&f];
        let delta = min_of(components, 1u64, t(), 1.0);
        assert_eq!(delta, Duration::hours(1));
        assert_eq!(seen.get(), 1.0);
    }

    #[test]
    fn min_of_two_components_splits_epsilon_evenly() {
        use std::cell::RefCell;
        let seen: RefCell<Vec<f64>> = RefCell::new(Vec::new());
        let f = |_id: u64, _t: DateTime<Utc>, eps: f64| {
            seen.borrow_mut().push(eps);
            Duration::hours(1)
        };
        let g = |_id: u64, _t: DateTime<Utc>, eps: f64| {
            seen.borrow_mut().push(eps);
            Duration::hours(2)
        };
        let components: &[&dyn Fn(u64, DateTime<Utc>, f64) -> Duration] = &[&f, &g];
        let delta = min_of(components, 1u64, t(), 2.0);
        assert_eq!(delta, Duration::hours(1));
        let passed = seen.borrow();
        assert_eq!(passed.len(), 2);
        assert!((passed[0] - 1.0).abs() < 1e-12);
        assert!((passed[1] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn min_of_takes_minimum_across_components() {
        let f = |_id: u64, _t: DateTime<Utc>, _eps: f64| Duration::hours(5);
        let g = |_id: u64, _t: DateTime<Utc>, _eps: f64| Duration::hours(1);
        let h = |_id: u64, _t: DateTime<Utc>, _eps: f64| Duration::hours(3);
        let components: &[&dyn Fn(u64, DateTime<Utc>, f64) -> Duration] = &[&f, &g, &h];
        let delta = min_of(components, 1u64, t(), 3.0);
        assert_eq!(delta, Duration::hours(1));
    }

    #[test]
    fn min_of_with_zero_epsilon_propagates_zero() {
        use std::cell::Cell;
        let seen: Cell<f64> = Cell::new(-1.0);
        let f = |_id: u64, _t: DateTime<Utc>, eps: f64| {
            seen.set(eps);
            Duration::zero()
        };
        let components: &[&dyn Fn(u64, DateTime<Utc>, f64) -> Duration] = &[&f];
        let _ = min_of(components, 1u64, t(), 0.0);
        assert_eq!(seen.get(), 0.0);
    }
}
