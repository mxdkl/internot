//! Distribution samplers. Transform uniform hashes into realistic distributions.

use crate::hash::{hash_float, hash_gaussian};
use crate::word::BitWord;

/// Pareto (power-law) sample. `alpha` is the shape parameter (larger = thinner tail);
/// `scale` is the minimum value of the distribution.
///
/// Used for org sizes, follower counts, city populations.
/// Formula: `scale * (1 - u)^(-1/alpha)` where `u ∈ [0, 1)`.
pub fn pareto<W: BitWord>(id: W, key: &str, alpha: f64, scale: f64) -> f64 {
    assert!(alpha > 0.0, "pareto alpha must be positive");
    assert!(scale > 0.0, "pareto scale must be positive");
    let u = hash_float(id, key);
    scale * (1.0 - u).powf(-1.0 / alpha)
}

/// Lognormal sample: `exp(mu + sigma * Z)` where `Z ~ N(0, 1)`.
///
/// Used for income, asset values, response times. Always positive.
pub fn lognormal<W: BitWord>(id: W, key: &str, mu: f64, sigma: f64) -> f64 {
    let z = hash_gaussian(id, key);
    (mu + sigma * z).exp()
}

/// Exponential sample with given rate. Inverse CDF: `-ln(1 - u) / rate`.
///
/// Used for waiting times between rare events. Always non-negative.
pub fn exponential<W: BitWord>(id: W, key: &str, rate: f64) -> f64 {
    assert!(rate > 0.0, "exponential rate must be positive");
    let u = hash_float(id, key);
    -(1.0 - u).ln() / rate
}

/// Weighted categorical choice. Returns an index in `[0, weights.len())`,
/// chosen proportional to the weight at that index. Weights need not be normalized.
///
/// # Panics
///
/// Panics if `weights` is empty, if any weight is negative, or if the sum of
/// weights is not positive.
pub fn categorical<W: BitWord>(id: W, key: &str, weights: &[f64]) -> usize {
    assert!(!weights.is_empty(), "weights must not be empty");
    assert!(
        weights.iter().all(|&w| w >= 0.0),
        "weights must be non-negative"
    );
    let total: f64 = weights.iter().sum();
    assert!(total > 0.0, "weights must have positive sum");
    let u = hash_float(id, key) * total;
    let mut cumulative = 0.0;
    for (i, w) in weights.iter().enumerate() {
        cumulative += w;
        if u < cumulative {
            return i;
        }
    }
    // Safety net: `iter().sum()` (used for `total`) and the left-fold
    // `cumulative` can differ by a few ULPs, so `u = hash_float * total`
    // may land in a tiny gap above the final cumulative. Fall back to
    // the last bucket rather than panicking.
    weights.len() - 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pareto_is_deterministic() {
        assert_eq!(pareto(42u64, "x", 2.0, 1.0), pareto(42u64, "x", 2.0, 1.0));
    }

    #[test]
    fn pareto_respects_scale_minimum() {
        for id in 0..1000u64 {
            let v = pareto(id, "x", 2.0, 5.0);
            assert!(v >= 5.0, "pareto value {} below scale minimum 5.0", v);
        }
    }

    #[test]
    fn pareto_has_heavy_tail() {
        // With alpha=1.5 and scale=1.0, expect some values > 10
        let max = (0..10_000u64)
            .map(|i| pareto(i, "x", 1.5, 1.0))
            .fold(0.0f64, f64::max);
        assert!(max > 10.0, "expected heavy tail, max was {}", max);
    }

    #[test]
    fn lognormal_is_positive() {
        for id in 0..1000u64 {
            let v = lognormal(id, "x", 0.0, 1.0);
            assert!(v > 0.0, "lognormal produced non-positive value {}", v);
        }
    }

    #[test]
    fn lognormal_median_approx_exp_mu() {
        let n = 10_000u64;
        let mut samples: Vec<f64> = (0..n).map(|i| lognormal(i, "x", 1.5, 0.5)).collect();
        samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median = samples[(n / 2) as usize];
        let expected = 1.5f64.exp();
        assert!(
            (median - expected).abs() / expected < 0.05,
            "lognormal median {} far from exp(mu)={}",
            median,
            expected
        );
    }

    #[test]
    fn exponential_is_non_negative() {
        for id in 0..1000u64 {
            let v = exponential(id, "x", 1.0);
            assert!(v >= 0.0);
        }
    }

    #[test]
    fn exponential_mean_approx_inverse_rate() {
        let rate = 2.0;
        let n = 10_000u64;
        let mean: f64 = (0..n).map(|i| exponential(i, "x", rate)).sum::<f64>() / n as f64;
        let expected = 1.0 / rate;
        assert!(
            (mean - expected).abs() / expected < 0.05,
            "exponential mean {} far from 1/rate {}",
            mean,
            expected
        );
    }

    #[test]
    fn categorical_respects_bounds() {
        for id in 0..1000u64 {
            let idx = categorical(id, "x", &[0.1, 0.2, 0.3, 0.4]);
            assert!(idx < 4);
        }
    }

    #[test]
    fn categorical_is_deterministic() {
        let weights = [0.5, 0.3, 0.2];
        assert_eq!(
            categorical(42u64, "x", &weights),
            categorical(42u64, "x", &weights)
        );
    }

    #[test]
    fn categorical_follows_weights_approximately() {
        let weights = [0.1, 0.6, 0.3];
        let n = 100_000u64;
        let mut counts = [0u64; 3];
        for i in 0..n {
            counts[categorical(i, "x", &weights)] += 1;
        }
        for (i, w) in weights.iter().enumerate() {
            let expected = n as f64 * w;
            let actual = counts[i] as f64;
            let diff = (actual - expected).abs() / expected;
            assert!(
                diff < 0.05,
                "bucket {} count {} far from expected {} (weight {})",
                i,
                actual,
                expected,
                w
            );
        }
    }

    #[test]
    fn categorical_handles_single_weight() {
        for id in 0..100u64 {
            assert_eq!(categorical(id, "x", &[1.0]), 0);
        }
    }

    #[test]
    #[should_panic(expected = "weights must not be empty")]
    fn categorical_panics_on_empty_weights() {
        categorical(42u64, "x", &[]);
    }

    #[test]
    #[should_panic(expected = "weights must be non-negative")]
    fn categorical_panics_on_negative_weight() {
        categorical(42u64, "x", &[0.5, -0.1, 0.6]);
    }

    #[test]
    fn pareto_works_with_u128() {
        let id: u128 = 0xDEAD_BEEF_1234_5678_AABB_CCDD_EEFF_0011;
        let v = pareto(id, "size", 1.5, 1.0);
        assert!(v >= 1.0);
        assert_eq!(v, pareto(id, "size", 1.5, 1.0), "should be deterministic");
    }

    #[test]
    fn lognormal_works_with_u128() {
        for id in 0..100u128 {
            let v = lognormal(id, "income", 0.0, 1.0);
            assert!(v > 0.0);
        }
    }

    #[test]
    fn exponential_works_with_u128() {
        for id in 0..100u128 {
            let v = exponential(id, "wait", 2.0);
            assert!(v >= 0.0);
        }
    }

    #[test]
    fn categorical_works_with_u128() {
        let weights = [0.2, 0.5, 0.3];
        for id in 0..100u128 {
            let idx = categorical(id, "cat", &weights);
            assert!(idx < 3);
        }
    }
}
