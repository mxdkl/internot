//! Named curve shapes for schedules by age: logistic, log-logistic,
//! Gaussian bumps, ramps, the Rogers–Castro migration profile, and kernel
//! combinators.
//!
//! Each is the exact expression a schedule evaluates, so swapping one in
//! keeps a world's bits. Transcendental math goes through [`crate::dmath`].

use crate::dmath::{exp, ln, pow};

/// The log-logistic CDF of `x` measured from `origin`, with median
/// `median` (also measured on the same axis) and shape `shape`:
/// `z^shape / (1 + z^shape)` with `z = max(x − origin, 0) / (median − origin)`.
#[inline]
pub fn log_logistic_cdf(x: f64, origin: f64, median: f64, shape: f64) -> f64 {
    let z = (x - origin).max(0.0) / (median - origin);
    let zs = pow(z, shape);
    zs / (1.0 + zs)
}

/// A logistic rise of height `level`: `level / (1 + e^(−(x − mid)/width))`.
#[inline]
pub fn logistic_rise(level: f64, x: f64, mid: f64, width: f64) -> f64 {
    level / (1.0 + exp(-(x - mid) / width))
}

/// The quantile of a logistic survival curve with a floor: survival
/// `S(x) = tail + (1 − tail) / (1 + e^((x − mid)/scale))` falls from 1 to
/// `tail`, so the event age for survival quantile `u` solves `S(x) = u`:
/// `mid + scale · ln((1 − tail)/(u − tail) − 1)`. `None` if `u < tail` (the
/// event never happens); `−∞` if `u` is at or above 1 (it happened before
/// any age).
#[inline]
pub fn logistic_floor_quantile(mid: f64, scale: f64, tail: f64, u: f64) -> Option<f64> {
    if u < tail {
        return None;
    }
    let r = (1.0 - tail) / (u - tail) - 1.0;
    Some(if r > 0.0 {
        mid + scale * ln(r)
    } else {
        f64::NEG_INFINITY
    })
}

/// An unnormalized Gaussian bump `e^(−z²)` with `z = (x − peak)/width`.
#[inline]
pub fn gaussian_bump(x: f64, peak: f64, width: f64) -> f64 {
    let z = (x - peak) / width;
    exp(-z * z)
}

/// An unnormalized Gaussian density in the offset `dx` from its centre:
/// `e^(−dx²/(2·variance))`.
#[inline]
pub fn gaussian_kernel(dx: f64, variance: f64) -> f64 {
    exp(-dx * dx / (2.0 * variance))
}

/// A linear ramp from `from` over `len`, clamped: `((x − from)/len)`
/// clamped to `[lo, hi]`.
#[inline]
pub fn ramp(x: f64, from: f64, len: f64, lo: f64, hi: f64) -> f64 {
    ((x - from) / len).clamp(lo, hi)
}

/// The labour-force component of the Rogers–Castro migration age profile:
/// `e^(−α·x − e^(−λ·x))` at `x` years from the peak's location parameter
/// (a double exponential; unnormalized).
#[inline]
pub fn rogers_castro_labour(x: f64, alpha: f64, lambda: f64) -> f64 {
    exp(-alpha * x - exp(-lambda * x))
}

/// A kernel made symmetric: `(g(x) + g(−x)) / 2`.
#[inline]
pub fn symmetrized(g: impl Fn(i32) -> f64, x: i32) -> f64 {
    0.5 * (g(x) + g(-x))
}

/// The kernel `g` read at `round(x · factor)`: a factor below 1 widens it
/// (gap `x` gets the weight of the smaller gap `x · factor`).
#[inline]
pub fn dilated(g: impl Fn(i32) -> f64, x: i32, factor: f64) -> f64 {
    g((x as f64 * factor).round() as i32)
}

/// A continuous piecewise power-law decay, the usual migration distance
/// kernel: 1 up to `flat`, then on each segment `(end, exponent)` the value
/// at the segment's start times `(x / start)^exponent`, with the last
/// exponent `beyond` continuing past the last end. Exponents are negative
/// for decay. Non-positive or flat-range `x` gives 1.
pub fn piecewise_power(x: f64, flat: f64, segments: &[(f64, f64)], beyond: f64) -> f64 {
    if x <= flat {
        return 1.0;
    }
    let (mut start, mut value) = (flat, 1.0);
    for &(end, exponent) in segments {
        if x <= end {
            return value * pow(x / start, exponent);
        }
        value *= pow(end / start, exponent);
        start = end;
    }
    value * pow(x / start, beyond)
}

/// An algebraic sigmoid, `½ + ½·z/(1 + |z|)`: no exponential, and a
/// closed-form inverse ([`algebraic_sigmoid_inv`]).
#[inline]
pub fn algebraic_sigmoid(z: f64) -> f64 {
    0.5 + 0.5 * z / (1.0 + z.abs())
}

/// The inverse of [`algebraic_sigmoid`] on `(0, 1)`.
#[inline]
pub fn algebraic_sigmoid_inv(v: f64) -> f64 {
    let w = 2.0 * v - 1.0;
    w / (1.0 - w.abs())
}

/// The quantile of the log-logistic CDF ([`log_logistic_cdf`]) at
/// `q ∈ (0, 1)`: `origin + (median − origin)·(q/(1 − q))^(1/shape)`.
#[inline]
pub fn log_logistic_quantile(q: f64, origin: f64, median: f64, shape: f64) -> f64 {
    origin + (median - origin) * pow(q / (1.0 - q), 1.0 / shape)
}

/// The CDF on `[lo, hi]` of an [`algebraic_sigmoid`] centred at `mu` with
/// scale `s`, truncated to that range.
#[inline]
pub fn truncated_sigmoid_cdf(x: f64, mu: f64, s: f64, lo: f64, hi: f64) -> f64 {
    let g = |x: f64| algebraic_sigmoid((x - mu) / s);
    let (a, b) = (g(lo), g(hi));
    ((g(x) - a) / (b - a)).clamp(0.0, 1.0)
}

/// The inverse of [`truncated_sigmoid_cdf`] at `f ∈ [0, 1]`.
#[inline]
pub fn truncated_sigmoid_inv(f: f64, mu: f64, s: f64, lo: f64, hi: f64) -> f64 {
    let g = |x: f64| algebraic_sigmoid((x - mu) / s);
    let (a, b) = (g(lo), g(hi));
    let v = (a + f * (b - a)).clamp(1e-12, 1.0 - 1e-12);
    mu + s * algebraic_sigmoid_inv(v)
}

#[cfg(test)]
mod tests {

    #[test]
    fn piecewise_power_is_continuous_and_decays() {
        let seg = [(300.0, -1.55), (1000.0, -0.93)];
        let k = |x: f64| piecewise_power(x, 60.0, &seg, -0.31);
        assert_eq!(k(10.0), 1.0);
        assert_eq!(k(60.0), 1.0);
        for &b in &[60.0, 300.0, 1000.0] {
            assert!(
                (k(b - 1e-9) - k(b + 1e-9)).abs() < 1e-9,
                "continuous at {b}"
            );
        }
        assert!((k(300.0) - pow(5.0, -1.55)).abs() < 1e-15);
        let mut last = 1.0;
        for d in 61..3000 {
            let v = k(d as f64);
            assert!(v < last && v > 0.0);
            last = v;
        }
        assert_eq!(k(2000.0).to_bits(), k(2000.0).to_bits());
    }
    use super::*;

    #[test]
    fn log_logistic_has_its_median() {
        assert!((log_logistic_cdf(25.0, 15.0, 25.0, 4.0) - 0.5).abs() < 1e-15);
        assert_eq!(log_logistic_cdf(10.0, 15.0, 25.0, 4.0), 0.0);
        let mut prev = 0.0;
        for x in 15..80 {
            let f = log_logistic_cdf(x as f64, 15.0, 25.0, 4.0);
            assert!(f >= prev && f < 1.0);
            prev = f;
        }
    }

    #[test]
    fn logistic_quantile_inverts_the_survival_curve() {
        let (mid, scale, tail) = (21.0, 2.5, 0.08);
        let s = |x: f64| tail + (1.0 - tail) / (1.0 + ((x - mid) / scale).exp());
        for u in [0.1, 0.3, 0.5, 0.77, 0.99] {
            let x = logistic_floor_quantile(mid, scale, tail, u).unwrap();
            assert!((s(x) - u).abs() < 1e-12, "{u}");
        }
        assert_eq!(logistic_floor_quantile(mid, scale, tail, 0.05), None);
        assert_eq!(
            logistic_floor_quantile(mid, scale, tail, 1.0),
            Some(f64::NEG_INFINITY)
        );
        assert!((logistic_rise(2.0, 70.0, 70.0, 5.0) - 1.0).abs() < 1e-15);
    }

    #[test]
    fn bumps_ramps_and_kernels() {
        assert_eq!(gaussian_bump(3.0, 3.0, 2.0), 1.0);
        assert!((gaussian_bump(5.0, 3.0, 2.0) - (-1.0f64).exp()).abs() < 1e-15);
        assert!((gaussian_kernel(2.0, 2.0) - (-1.0f64).exp()).abs() < 1e-15);
        assert_eq!(ramp(10.0, 20.0, 10.0, 0.0, 1.0), 0.0);
        assert_eq!(ramp(25.0, 20.0, 10.0, 0.0, 1.0), 0.5);
        assert_eq!(ramp(25.0, 20.0, 10.0, 0.6, 1.0), 0.6);
        let g = |x: i32| if x >= 0 { 1.0 / (1 + x) as f64 } else { 0.0 };
        assert_eq!(symmetrized(g, 1), 0.25);
        assert_eq!(symmetrized(g, -1), 0.25);
        assert_eq!(dilated(g, 5, 0.6), g(3));
        // The labour profile peaks after its location parameter.
        let peak =
            (0..60)
                .max_by(|&a, &b| {
                    rogers_castro_labour(a as f64 - 20.0, 0.1, 0.4)
                        .total_cmp(&rogers_castro_labour(b as f64 - 20.0, 0.1, 0.4))
                })
                .unwrap();
        assert!((20..30).contains(&peak), "{peak}");
    }

    #[test]
    fn golden() {
        let bits = [
            log_logistic_cdf(27.3, 14.0, 24.5, 5.2),
            logistic_rise(0.4, 77.0, 80.0, 6.0),
            logistic_floor_quantile(22.0, 3.0, 0.05, 0.6).unwrap(),
            gaussian_bump(23.0, 21.0, 3.5),
            gaussian_kernel(-4.0, 30.0),
            rogers_castro_labour(6.0, 0.11, 0.35),
        ]
        .map(f64::to_bits);
        assert_eq!(bits, GOLDEN);
    }

    const GOLDEN: [u64; 6] = [
        4605143930316978635,
        4594608954739548338,
        4626616756900113295,
        4604673213861313316,
        4605074088703779486,
        4601909268418673106,
    ];
}

#[cfg(test)]
mod sigmoid_tests {
    use super::*;

    #[test]
    fn inverse_round_trips_and_is_monotone() {
        let mut last = 0.0;
        for i in -400..=400 {
            let z = i as f64 / 20.0;
            let v = algebraic_sigmoid(z);
            assert!(v > 0.0 && v < 1.0 && v > last);
            assert!((algebraic_sigmoid_inv(v) - z).abs() < 1e-9 * (1.0 + z.abs() * z.abs()), "{z}");
            last = v;
        }
    }

    #[test]
    fn golden() {
        assert_eq!((algebraic_sigmoid(0.0), algebraic_sigmoid(1.0), algebraic_sigmoid(-3.0)), (0.5, 0.75, 0.125));
        assert_eq!(algebraic_sigmoid_inv(0.75), 1.0);
    }
}

#[cfg(test)]
mod quantile_tests {
    use super::*;

    #[test]
    fn quantiles_invert_their_cdfs() {
        for i in 1..200 {
            let q = i as f64 / 200.0;
            let x = log_logistic_quantile(q, 15.0, 23.0, 6.0);
            assert!((log_logistic_cdf(x, 15.0, 23.0, 6.0) - q).abs() < 1e-9, "{q}");
            let a = truncated_sigmoid_inv(q, 27.0, 3.5, 15.0, 46.0);
            assert!((15.0..=46.0).contains(&a) && (truncated_sigmoid_cdf(a, 27.0, 3.5, 15.0, 46.0) - q).abs() < 1e-9, "{q}");
        }
        assert_eq!((truncated_sigmoid_cdf(15.0, 27.0, 3.5, 15.0, 46.0), truncated_sigmoid_cdf(46.0, 27.0, 3.5, 15.0, 46.0)), (0.0, 1.0));
    }

    #[test]
    fn golden() {
        assert_eq!(log_logistic_quantile(0.5, 15.0, 23.0, 6.0), 23.0);
        assert_eq!(format!("{:.9}", truncated_sigmoid_cdf(27.0, 27.0, 3.5, 15.0, 46.0)), "0.478299380");
    }
}
