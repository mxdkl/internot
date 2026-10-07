//! Liability-threshold models: an ordered outcome as a latent standard
//! normal "liability" cut at thresholds (Falconer 1965; the ordered probit).
//!
//! Correlated outcomes come from liabilities that share shocks: each
//! liability is a weighted sum of independent standard-normal shocks,
//! scaled to unit variance ([`combine`]), and two liabilities correlate
//! through the shocks they share ([`correlation`]). Thresholds fitted to
//! category shares ([`Thresholds::from_shares`]) then give those shares
//! exactly, whatever the correlations: the marginals and the dependence are
//! set separately.

use crate::dmath::{norm_cdf, norm_quantile};

/// Cut points of a standard-normal liability into ordered categories:
/// category `k` is `cuts[k - 1] ≤ z < cuts[k]` (with `cuts[-1] = −∞` and
/// `cuts[n] = +∞`).
#[derive(Clone, Debug, PartialEq)]
pub struct Thresholds {
    cuts: Vec<f64>,
}

impl Thresholds {
    /// Thresholds giving categories the shares `shares` (non-negative, not
    /// necessarily normalized; at least one positive): `cuts[k] =
    /// Φ⁻¹(F_k)`, with `F_k` the cumulative share of categories `0..=k`.
    /// An empty category gets two equal cuts.
    pub fn from_shares(shares: &[f64]) -> Self {
        assert!(!shares.is_empty(), "at least one category");
        assert!(
            shares.iter().all(|&s| s >= 0.0 && s.is_finite()),
            "shares are finite and non-negative"
        );
        let total: f64 = shares.iter().sum();
        assert!(total > 0.0, "a positive share");
        let mut acc = 0.0;
        let cuts = shares[..shares.len() - 1]
            .iter()
            .map(|&s| {
                acc += s;
                norm_quantile((acc / total).min(1.0))
            })
            .collect();
        Self { cuts }
    }

    /// The category of liability `z`.
    #[inline]
    pub fn category(&self, z: f64) -> usize {
        self.cuts.partition_point(|&c| c <= z)
    }

    /// The cut points, ascending (one fewer than the categories).
    pub fn cuts(&self) -> &[f64] {
        &self.cuts
    }

    /// Number of categories.
    pub fn len(&self) -> usize {
        self.cuts.len() + 1
    }

    /// Never true: there is always at least one category.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Each category's probability under a standard-normal liability.
    pub fn shares(&self) -> Vec<f64> {
        let mut prev = 0.0;
        let mut out: Vec<f64> = self
            .cuts
            .iter()
            .map(|&c| {
                let f = norm_cdf(c);
                let s = f - prev;
                prev = f;
                s
            })
            .collect();
        out.push(1.0 - prev);
        out
    }
}

/// A unit-variance liability from independent standard-normal shocks:
/// `Σ wᵢ sᵢ / √(Σ wᵢ²)` over `(weight, shock)` terms (weights not all zero).
#[inline]
pub fn combine(terms: &[(f64, f64)]) -> f64 {
    let (mut sum, mut norm) = (0.0, 0.0);
    for &(w, s) in terms {
        sum += w * s;
        norm += w * w;
    }
    debug_assert!(norm > 0.0, "a nonzero weight");
    sum / norm.sqrt()
}

/// The correlation of two [`combine`]d liabilities, given each as
/// `(shock id, weight)` terms with distinct ids within each: the weights
/// on shared shocks, `Σ_shared wₐ w_b / √(Σ wₐ² · Σ w_b²)`.
pub fn correlation(a: &[(u64, f64)], b: &[(u64, f64)]) -> f64 {
    let na: f64 = a.iter().map(|t| t.1 * t.1).sum();
    let nb: f64 = b.iter().map(|t| t.1 * t.1).sum();
    let shared: f64 = a
        .iter()
        .filter_map(|&(id, wa)| b.iter().find(|t| t.0 == id).map(|t| wa * t.1))
        .sum();
    shared / (na * nb).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::Key;
    use crate::sample::std_normal;

    #[test]
    fn thresholds_give_their_shares() {
        let cases: [&[f64]; 5] = [
            &[0.2, 0.5, 0.3],
            &[1.0],
            &[0.0, 3.0, 1.0, 0.0],
            &[1e-9, 1.0, 1e-9],
            &[5.0, 5.0, 5.0, 5.0, 5.0, 5.0],
        ];
        for shares in cases {
            let t = Thresholds::from_shares(shares);
            assert_eq!(t.len(), shares.len());
            let total: f64 = shares.iter().sum();
            for (got, want) in t.shares().iter().zip(shares) {
                assert!(
                    (got - want / total).abs() < 1e-14,
                    "{shares:?}: {got} vs {want}"
                );
            }
            assert!(t.cuts().windows(2).all(|w| w[0] <= w[1]), "ascending");
        }
    }

    #[test]
    fn categories_follow_the_cuts_and_the_shares() {
        let shares = [0.1, 0.25, 0.4, 0.2, 0.05];
        let t = Thresholds::from_shares(&shares);
        // Boundaries: a cut belongs to the category above it.
        for (k, &c) in t.cuts().iter().enumerate() {
            assert_eq!(t.category(c), k + 1);
            assert_eq!(t.category(c - 1e-12), k);
        }
        assert_eq!(t.category(f64::NEG_INFINITY), 0);
        assert_eq!(t.category(f64::INFINITY), shares.len() - 1);
        // Empirical law over keyed normals.
        let key = Key::from_seed(41);
        let n = 400_000u64;
        let mut count = [0u64; 5];
        for i in 0..n {
            count[t.category(std_normal(key.with(i)))] += 1;
        }
        for (c, s) in count.iter().zip(shares) {
            let f = *c as f64 / n as f64;
            assert!(
                (f - s).abs() < 5.0 * (s * (1.0 - s) / n as f64).sqrt(),
                "{f} vs {s}"
            );
        }
        // An empty category is never drawn.
        let t = Thresholds::from_shares(&[0.5, 0.0, 0.5]);
        assert!((0..10_000).all(|i| t.category(std_normal(key.with2(7, i))) != 1));
    }

    #[test]
    fn combined_liabilities_are_standard_and_correlate_as_computed() {
        // Two liabilities sharing shocks 0 and 1, each with an own shock.
        let a = [(0u64, 0.6), (1, 0.3), (2, 0.5)];
        let b = [(0u64, 0.4), (1, 0.5), (3, 0.7)];
        let rho = correlation(&a, &b);
        let want =
            (0.6 * 0.4 + 0.3 * 0.5) / ((0.36 + 0.09 + 0.25f64) * (0.16 + 0.25 + 0.49f64)).sqrt();
        assert!((rho - want).abs() < 1e-15);
        assert_eq!(correlation(&a, &a), 1.0);
        assert_eq!(correlation(&[(0, 1.0)], &[(1, 1.0)]), 0.0);
        let key = Key::from_seed(5);
        let n = 200_000u64;
        let (mut sa, mut sb, mut saa, mut sbb, mut sab) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for i in 0..n {
            let s: Vec<f64> = (0..4).map(|j| std_normal(key.with2(i, j))).collect();
            let za = combine(&a.map(|(id, w)| (w, s[id as usize])));
            let zb = combine(&b.map(|(id, w)| (w, s[id as usize])));
            (sa, sb) = (sa + za, sb + zb);
            (saa, sbb, sab) = (saa + za * za, sbb + zb * zb, sab + za * zb);
        }
        let nf = n as f64;
        let (ma, mb) = (sa / nf, sb / nf);
        let (va, vb) = (saa / nf - ma * ma, sbb / nf - mb * mb);
        let r = (sab / nf - ma * mb) / (va * vb).sqrt();
        assert!(ma.abs() < 0.01 && mb.abs() < 0.01, "means {ma} {mb}");
        assert!(
            (va - 1.0).abs() < 0.01 && (vb - 1.0).abs() < 0.01,
            "variances {va} {vb}"
        );
        assert!((r - rho).abs() < 0.01, "correlation {r} vs {rho}");
    }

    /// Golden values: exact cut bits for a fixed share vector.
    #[test]
    fn golden_cuts() {
        let t = Thresholds::from_shares(&[0.12, 0.31, 0.27, 0.2, 0.1]);
        let bits: Vec<u64> = t.cuts().iter().map(|c| c.to_bits()).collect();
        assert_eq!(
            bits,
            [
                0xBFF2CCBEF3527E31,
                0xBFC6936DBACBEB27,
                0x3FE0C7E39582C5FE,
                0x3FF4813C36E26D34
            ],
            "{:?}",
            bits.iter()
                .map(|b| format!("0x{b:016X}"))
                .collect::<Vec<_>>()
        );
    }
}
