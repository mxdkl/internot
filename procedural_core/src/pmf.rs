//! Operations on discrete distributions given as weight slices: normalizing,
//! flooring, mixing, shifting, spreading mass over bins, and cumulative
//! tables.
//!
//! Each operation has one fixed arithmetic (sums left to right, the stated
//! expression per entry), so a world built on it keeps its bits.

/// `values` divided by their sum.
pub fn normalized(values: impl Iterator<Item = f64>) -> Vec<f64> {
    let out: Vec<f64> = values.collect();
    let sum: f64 = out.iter().sum();
    out.iter().map(|m| m / sum).collect()
}

/// Every share raised to at least `floor`, then renormalized: a mix in
/// which no part is rarer than about `floor`.
pub fn floored(mix: &[f64], floor: f64) -> Vec<f64> {
    normalized(mix.iter().map(|m| m.max(floor)))
}

/// The mix `a·(1 − w) + b·w`, entry by entry, written to `out`.
pub fn mix_into(a: &[f64], b: &[f64], w: f64, out: &mut [f64]) {
    for ((o, &x), &y) in out.iter_mut().zip(a).zip(b) {
        *o = x * (1.0 - w) + y * w;
    }
}

/// A count distribution thinned by one: with probability `keep`, one fewer
/// event than `pmf` says (none stays none); otherwise no event. Index `k` is
/// the count.
pub fn one_fewer_or_none<const N: usize>(pmf: &[f64; N], keep: f64) -> [f64; N] {
    let mut shifted = [0.0; N];
    for (k, &pk) in pmf.iter().enumerate() {
        shifted[k.saturating_sub(1)] += pk;
    }
    let mut out = shifted.map(|x| keep * x);
    out[0] += 1.0 - keep;
    out
}

/// Spread `mass` evenly over the bins `lo..hi` of `out` (each gets
/// `mass / (hi − lo)`).
#[inline]
pub fn spread_evenly(out: &mut [f64], lo: usize, hi: usize, mass: f64) {
    for o in &mut out[lo..hi] {
        *o = mass / (hi - lo) as f64;
    }
}

/// The cumulative table of a density's positive part, normalized:
/// `cum[0] = 0` and `cum[a + 1] = cum[a] + max(d[a], 0) / total`, where
/// `total` is the positive part's sum (`dens.len() + 1` entries).
pub fn cumulative_normalized(dens: &[f64], total: f64) -> Vec<f64> {
    let mut cum = Vec::with_capacity(dens.len() + 1);
    let mut acc = 0.0;
    cum.push(acc);
    for &d in dens {
        acc += d.max(0.0) / total;
        cum.push(acc);
    }
    cum
}

/// The sum of a density's positive part.
#[inline]
pub fn positive_mass(dens: &[f64]) -> f64 {
    dens.iter().map(|d| d.max(0.0)).sum()
}

/// The quantile at `q ∈ [0, 1)` of a density uniform within each band
/// `bands[k] = (lo, hi)`, band `k` carrying share `share(k)` (shares summing
/// to 1): the band where the cumulative share passes `q`, then linear
/// within it. `default` when the shares run out first.
pub fn band_quantile(bands: &[(f64, f64)], share: impl Fn(usize) -> f64, q: f64, default: f64) -> f64 {
    let mut acc = 0.0;
    for (k, &(lo, hi)) in bands.iter().enumerate() {
        let w = share(k);
        if q < acc + w {
            return lo + (hi - lo) * (q - acc) / w;
        }
        acc += w;
    }
    default
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizing_and_flooring() {
        assert_eq!(normalized([1.0, 3.0].into_iter()), vec![0.25, 0.75]);
        let f = floored(&[0.9, 0.1, 0.0], 0.05);
        assert!((f.iter().sum::<f64>() - 1.0).abs() < 1e-15);
        assert!(f[2] > 0.04);
    }

    #[test]
    fn mixing_thinning_and_spreading() {
        let mut out = [0.0; 3];
        mix_into(&[1.0, 0.0, 0.0], &[0.0, 0.5, 0.5], 0.25, &mut out);
        assert_eq!(out, [0.75, 0.125, 0.125]);
        let p = one_fewer_or_none(&[0.1, 0.2, 0.3, 0.4], 0.5);
        assert!((p.iter().sum::<f64>() - 1.0).abs() < 1e-15);
        assert_eq!(p, [0.5 + 0.15, 0.15, 0.2, 0.0]);
        let mut bins = [0.0; 6];
        spread_evenly(&mut bins, 1, 5, 2.0);
        assert_eq!(bins, [0.0, 0.5, 0.5, 0.5, 0.5, 0.0]);
    }

    #[test]
    fn cumulative_of_the_positive_part() {
        let d = [0.5, -1.0, 1.5, 0.0];
        let total = positive_mass(&d);
        assert_eq!(total, 2.0);
        assert_eq!(
            cumulative_normalized(&d, total),
            vec![0.0, 0.25, 0.25, 1.0, 1.0]
        );
    }

    #[test]
    fn golden() {
        let n = normalized([0.3, 0.11, 0.07, 0.52].into_iter());
        let f = floored(&[0.62, 0.3, 0.008, 0.072], 0.025);
        let bits: Vec<u64> = n.iter().chain(&f).map(|x| x.to_bits()).collect();
        assert_eq!(bits, GOLDEN);
    }

    const GOLDEN: [u64; 8] = [
        4599075939470750515,
        4592590756007337001,
        4589708452245819884,
        4602858963157741732,
        4603666334135187346,
        4598985601779110045,
        4582742530556696104,
        4589765843249920888,
    ];
}

#[cfg(test)]
mod band_tests {
    use super::*;

    #[test]
    fn band_quantiles_are_monotone_and_land_in_bands() {
        let bands = [(1.0, 4.0), (4.0, 8.0), (8.0, 13.0)];
        let w = [0.2, 0.5, 0.3];
        let mut last = 0.0;
        for i in 0..1000 {
            let q = i as f64 / 1000.0;
            let x = band_quantile(&bands, |k| w[k], q, 40.0);
            assert!((1.0..13.0).contains(&x) && x >= last, "{q}");
            last = x;
        }
    }

    #[test]
    fn golden() {
        let bands = [(1.0, 4.0), (4.0, 8.0)];
        assert_eq!(band_quantile(&bands, |k| [0.25, 0.75][k], 0.5, 40.0), 4.0 + 4.0 * (0.25 / 0.75));
        assert_eq!(band_quantile(&bands, |k| [0.25, 0.5][k], 0.9, 40.0), 40.0);
    }
}
