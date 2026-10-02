//! Exact, deterministic samplers keyed by [`Key`].
//!
//! The substrate owns these algorithms rather than borrowing a library's:
//! a library upgrade that changed an algorithm would silently re-roll every
//! world. All transcendental math goes through [`crate::dmath`], so results
//! are bit-identical across machines. Golden tests pin representative draws.
//!
//! Algorithms (see `docs/superpowers/research/2026-09-29-deterministic-numerics.md`):
//! - Poisson: multiplication method for λ < 10; PTRS (Hörmann 1993) above.
//! - Binomial: inversion when min(p, 1−p)·n ≤ 30; BTPE (Kachitvichyanukul &
//!   Schmeiser 1988) above, with numpy's correction to the Step 52 bound.
//! - log Γ: numpy's Stirling series, for PTRS's acceptance test.
//!
//! Rejection samplers draw from [`Key::uniforms`], so the i-th attempt always
//! sees the same numbers and every sample is a pure function of its key.

use crate::dmath::{exp, ln, ln_1p, pow};
use crate::key::{Key, Uniforms};

/// Exponential(1) from a uniform in `[0, 1)`: `-ln(1 - u)`.
#[inline]
pub fn exp1_from_unit(u: f64) -> f64 {
    -ln_1p(-u)
}

/// Exponential draw with the given `rate` (mean `1 / rate`).
#[inline]
pub fn exponential(key: Key, rate: f64) -> f64 {
    debug_assert!(rate > 0.0);
    exp1_from_unit(key.unit()) / rate
}

/// Standard normal via Box–Muller on two derived uniforms.
#[inline]
pub fn std_normal(key: Key) -> f64 {
    let u1 = key.with(1).unit_open0();
    let u2 = key.with(2).unit();
    (-2.0 * ln(u1)).sqrt() * crate::dmath::cos(2.0 * std::f64::consts::PI * u2)
}

/// Lognormal draw: `exp(mu + sigma · z)`.
#[inline]
pub fn lognormal(key: Key, mu: f64, sigma: f64) -> f64 {
    exp(mu + sigma * std_normal(key))
}

/// Lomax (Pareto II) lifetime with power-law survival `S(t) = (1 + t)^gamma`,
/// `gamma < 0`. This is Burt's (2000) tie-decay law: the hazard falls with
/// age. Inverse CDF: `t = U^(1/gamma) − 1` for `U` in `(0, 1]`.
#[inline]
pub fn lomax_lifetime(key: Key, gamma: f64) -> f64 {
    debug_assert!(gamma < 0.0, "Lomax survival exponent must be negative");
    pow(key.unit_open0(), 1.0 / gamma) - 1.0
}

/// Poisson draw with mean `lambda`. Returns 0 for `lambda <= 0`.
pub fn poisson(key: Key, lambda: f64) -> u64 {
    if lambda.is_nan() || lambda <= 0.0 {
        return 0;
    }
    let mut u = key.uniforms();
    if lambda < 10.0 {
        poisson_mult(&mut u, lambda)
    } else {
        poisson_ptrs(&mut u, lambda)
    }
}

fn poisson_mult(u: &mut Uniforms, lambda: f64) -> u64 {
    let enlam = exp(-lambda);
    let mut x = 0u64;
    let mut prod = 1.0;
    loop {
        prod *= u.next();
        if prod > enlam {
            x += 1;
        } else {
            return x;
        }
    }
}

/// PTRS: transformed rejection with squeeze (Hörmann 1993), as in numpy.
fn poisson_ptrs(u: &mut Uniforms, lambda: f64) -> u64 {
    let slam = lambda.sqrt();
    let loglam = ln(lambda);
    let b = 0.931 + 2.53 * slam;
    let a = -0.059 + 0.02483 * b;
    let invalpha = 1.1239 + 1.1328 / (b - 3.4);
    let vr = 0.9277 - 3.6224 / (b - 2.0);
    loop {
        let uu = u.next() - 0.5;
        let v = u.next();
        let us = 0.5 - uu.abs();
        let k = ((2.0 * a / us + b) * uu + lambda + 0.43).floor();
        if us >= 0.07 && v <= vr {
            return k as u64;
        }
        if k < 0.0 || (us < 0.013 && v > us) {
            continue;
        }
        // ln(0) = -inf is fine here: the test then always accepts.
        if ln(v) + ln(invalpha) - ln(a / (us * us) + b) <= -lambda + k * loglam - loggam(k + 1.0) {
            return k as u64;
        }
    }
}

/// `ln Γ(x)` via numpy's Stirling series (exact at 1 and 2).
fn loggam(x: f64) -> f64 {
    // numpy's literals, kept verbatim for traceability.
    #[allow(clippy::excessive_precision)]
    const A: [f64; 10] = [
        8.333333333333333e-02,
        -2.777777777777778e-03,
        7.936507936507937e-04,
        -5.952380952380952e-04,
        8.417508417508418e-04,
        -1.917526917526918e-03,
        6.410256410256410e-03,
        -2.955065359477124e-02,
        1.796443723688307e-01,
        -1.39243221690590e+00,
    ];
    if x == 1.0 || x == 2.0 {
        return 0.0;
    }
    let n: i64 = if x < 7.0 { (7.0 - x) as i64 } else { 0 };
    let mut x0 = x + n as f64;
    let x2 = (1.0 / x0) * (1.0 / x0);
    const LG2PI: f64 = 1.8378770664093453e+00;
    let mut gl0 = A[9];
    for k in (0..9).rev() {
        gl0 *= x2;
        gl0 += A[k];
    }
    let mut gl = gl0 / x0 + 0.5 * LG2PI + (x0 - 0.5) * ln(x0) - x0;
    if x < 7.0 {
        for _ in 1..=n {
            gl -= ln(x0 - 1.0);
            x0 -= 1.0;
        }
    }
    gl
}

/// Binomial(n, p) draw.
pub fn binomial(key: Key, n: u64, p: f64) -> u64 {
    if n == 0 || p <= 0.0 || p.is_nan() {
        return 0;
    }
    if p >= 1.0 {
        return n;
    }
    let mut u = key.uniforms();
    let (q, flip) = if p <= 0.5 {
        (p, false)
    } else {
        (1.0 - p, true)
    };
    let y = if q * n as f64 <= 30.0 {
        binomial_inversion(&mut u, n, q)
    } else {
        binomial_btpe(&mut u, n, q)
    };
    if flip {
        n - y
    } else {
        y
    }
}

/// Inversion with numpy's restart bound. Requires `p <= 0.5`, `n·p <= 30`.
fn binomial_inversion(u: &mut Uniforms, n: u64, p: f64) -> u64 {
    let q = 1.0 - p;
    let qn = exp(n as f64 * ln_1p(-p));
    let np = n as f64 * p;
    let bound = (n as f64).min(np + 10.0 * (np * q + 1.0).sqrt()) as u64;
    let mut x = 0u64;
    let mut px = qn;
    let mut uu = u.next();
    while uu > px {
        x += 1;
        if x > bound {
            x = 0;
            px = qn;
            uu = u.next();
        } else {
            uu -= px;
            px = ((n - x + 1) as f64 * p * px) / (x as f64 * q);
        }
    }
    x
}

/// BTPE (Kachitvichyanukul & Schmeiser 1988) as in numpy, including numpy's
/// correction: the third and fourth Stirling error terms in Step 52 are
/// subtracted. Requires `p <= 0.5` and `n·p > 30`.
fn binomial_btpe(u: &mut Uniforms, n: u64, p: f64) -> u64 {
    let nf = n as f64;
    let r = p;
    let q = 1.0 - r;
    let fm = nf * r + r;
    let m = fm.floor();
    let p1 = (2.195 * (nf * r * q).sqrt() - 4.6 * q).floor() + 0.5;
    let xm = m + 0.5;
    let xl = xm - p1;
    let xr = xm + p1;
    let c = 0.134 + 20.5 / (15.3 + m);
    let a = (fm - xl) / (fm - xl * r);
    let laml = a * (1.0 + a / 2.0);
    let a = (xr - fm) / (xr * q);
    let lamr = a * (1.0 + a / 2.0);
    let p2 = p1 * (1.0 + 2.0 * c);
    let p3 = p2 + c / laml;
    let p4 = p3 + c / lamr;
    let nrq = nf * r * q;

    loop {
        // Step 10
        let uu = u.next() * p4;
        let mut v = u.next();
        let y: f64;
        if uu <= p1 {
            return (xm - p1 * v + uu).floor() as u64; // Step 60
        } else if uu <= p2 {
            // Step 20: parallelogram
            let x = xl + (uu - p1) / c;
            v = v * c + 1.0 - (m - x + 0.5).abs() / p1;
            if v > 1.0 {
                continue;
            }
            y = x.floor();
        } else if uu <= p3 {
            // Step 30: left exponential tail
            if v == 0.0 {
                continue;
            }
            y = (xl + ln(v) / laml).floor();
            if y < 0.0 {
                continue;
            }
            v = v * (uu - p2) * laml;
        } else {
            // Step 40: right exponential tail
            if v == 0.0 {
                continue;
            }
            y = (xr - ln(v) / lamr).floor();
            if y > nf {
                continue;
            }
            v = v * (uu - p3) * lamr;
        }

        // Step 50
        let k = (y - m).abs();
        if !(k > 20.0 && k < nrq / 2.0 - 1.0) {
            let s = r / q;
            let a = s * (nf + 1.0);
            let mut f = 1.0;
            if m < y {
                let mut i = m + 1.0;
                while i <= y {
                    f *= a / i - s;
                    i += 1.0;
                }
            } else if m > y {
                let mut i = y + 1.0;
                while i <= m {
                    f /= a / i - s;
                    i += 1.0;
                }
            }
            if v > f {
                continue;
            }
            return y as u64;
        }

        // Step 52: squeeze on log scale
        let rho = (k / nrq) * ((k * (k / 3.0 + 0.625) + 0.16666666666666666) / nrq + 0.5);
        let t = -k * k / (2.0 * nrq);
        let aa = ln(v);
        if aa < t - rho {
            return y as u64;
        }
        if aa > t + rho {
            continue;
        }
        let x1 = y + 1.0;
        let f1 = m + 1.0;
        let z = nf + 1.0 - m;
        let w = nf - y + 1.0;
        let bound = xm * ln(f1 / x1)
            + (nf - m + 0.5) * ln(z / w)
            + (y - m) * ln(w * r / (x1 * q))
            + stirling_term(f1)
            + stirling_term(z)
            - stirling_term(x1)
            - stirling_term(w);
        if aa > bound {
            continue;
        }
        return y as u64;
    }
}

/// Gamma draw with `shape` and `scale` (mean `shape · scale`), by
/// Marsaglia & Tsang (2000): squeeze-and-reject on a transformed normal,
/// attempt `i` keyed by `key.with(i)`, so the draw is a pure function of the
/// key. A shape below 1 uses the boost `Γ(shape + 1) · U^(1/shape)`.
/// Returns 0 for a non-positive shape or scale.
pub fn gamma(key: Key, shape: f64, scale: f64) -> f64 {
    if !(shape > 0.0 && scale > 0.0) {
        return 0.0;
    }
    if shape < 1.0 {
        let u = key.with(u64::MAX).unit_open0();
        return gamma(key.with(u64::MAX - 1), shape + 1.0, scale) * pow(u, 1.0 / shape);
    }
    let d = shape - 1.0 / 3.0;
    let c = 1.0 / (9.0 * d).sqrt();
    let mut i = 0u64;
    loop {
        let k = key.with(i);
        i += 1;
        let x = std_normal(k.with(1));
        let v = 1.0 + c * x;
        if v <= 0.0 {
            continue;
        }
        let v = v * v * v;
        let u = k.with(2).unit_open0();
        let x2 = x * x;
        if u < 1.0 - 0.0331 * x2 * x2 || ln(u) < 0.5 * x2 + d * (1.0 - v + ln(v)) {
            return d * v * scale;
        }
    }
}

/// A frailty: a gamma draw with mean 1 and variance `var` (1 if `var` is
/// not positive), the standard multiplicative heterogeneity of a hazard.
pub fn frailty(key: Key, var: f64) -> f64 {
    if var > 0.0 {
        gamma(key, 1.0 / var, var)
    } else {
        1.0
    }
}

/// Exponential(1) by textbook inversion, `-ln(1 - u)` for `u ∈ [0, 1)`.
///
/// The same law as [`exp1_from_unit`], which uses `ln_1p` and is more
/// accurate near 0; the two differ in the last bits, so a world keyed on one
/// must keep it.
#[inline]
pub fn exp1_by_inversion(u: f64) -> f64 {
    -ln(1.0 - u)
}

/// An index drawn by weight from a uniform `u ∈ [0, 1)` by a linear scan:
/// `u` is scaled by the weights' total, and the first index whose
/// cumulative weight passes it wins (0 if rounding leaves `u` past the end).
#[inline]
pub fn pick_linear(weights: &[f64], u: f64) -> usize {
    let u = u * weights.iter().sum::<f64>();
    let mut acc = 0.0;
    weights
        .iter()
        .position(|&w| {
            acc += w;
            u < acc
        })
        .unwrap_or(0)
}

/// An exact draw of `X` conditioned on `X ≥ r`, where `r` is costly to
/// compute but bracketed by cheap bounds `own ≤ r ≤ bound`.
///
/// `draw(req, key)` draws `X` conditioned on `X ≥ req` (by inversion, say).
/// Stage 1 draws `A0 = draw(own, k1)` and keeps it if `A0 ≥ bound`, without
/// computing `r`; otherwise `r = required()`, and `A0` is kept if `A0 ≥ r`,
/// else stage 2 redraws `draw(r, k2)`. For `a ≥ r`, `P(a) = f(a)/S(own) +
/// (1 − S(r)/S(own))·f(a)/S(r) = f(a)/S(r)`: exactly the conditional law.
#[inline]
pub fn lazy_conditional<T: PartialOrd + Copy>(
    own: T,
    bound: T,
    required: impl FnOnce() -> T,
    mut draw: impl FnMut(T, Key) -> T,
    (k1, k2): (Key, Key),
) -> T {
    let a0 = draw(own, k1);
    if a0 >= bound {
        return a0;
    }
    let r = required();
    if a0 >= r {
        a0
    } else {
        draw(r, k2)
    }
}

/// One Stirling-series error term from BTPE Step 52.
#[inline]
fn stirling_term(x: f64) -> f64 {
    let x2 = x * x;
    (13860. - (462. - (132. - (99. - 140. / x2) / x2) / x2) / x2) / x / 166320.
}

#[cfg(test)]
mod tests {

    #[test]
    fn gamma_moments() {
        for &(shape, scale) in &[(0.4, 1.5), (1.0, 1.0), (2.0, 0.5), (3.3, 0.3), (12.0, 2.0)] {
            let n = 200_000u64;
            let root = Key::from_seed(4242).with((shape * 100.0) as u64);
            let (mut s1, mut s2) = (0.0, 0.0);
            for i in 0..n {
                let x = gamma(root.with(i), shape, scale);
                assert!(x >= 0.0);
                s1 += x;
                s2 += x * x;
            }
            let mean = s1 / n as f64;
            let var = s2 / n as f64 - mean * mean;
            let (m, v) = (shape * scale, shape * scale * scale);
            assert!(
                (mean - m).abs() < 0.01 * m.max(1.0),
                "shape {shape}: mean {mean} vs {m}"
            );
            assert!(
                (var - v).abs() < 0.03 * v.max(1.0),
                "shape {shape}: var {var} vs {v}"
            );
        }
        assert_eq!(gamma(Key::from_seed(1), 0.0, 1.0), 0.0);
        assert_eq!(frailty(Key::from_seed(1), 0.0), 1.0);
    }

    #[test]
    fn gamma_golden() {
        let k = Key::from_seed(2026);
        assert_eq!(gamma(k, 2.0, 0.5).to_bits(), 0x3FFC17991EB0592B);
        assert_eq!(gamma(k.with(1), 0.4, 1.0).to_bits(), 0x3FEA1698B4E213E3);
    }
    use super::*;

    fn mean_var(xs: impl Iterator<Item = f64>) -> (f64, f64, usize) {
        let (mut n, mut s, mut s2) = (0usize, 0.0, 0.0);
        for x in xs {
            n += 1;
            s += x;
            s2 += x * x;
        }
        let m = s / n as f64;
        (m, s2 / n as f64 - m * m, n)
    }

    #[test]
    fn poisson_moments_small_and_large_lambda() {
        for &lam in &[0.01f64, 0.5, 3.0, 9.99, 10.0, 25.0, 400.0, 1e5] {
            let root = Key::from_seed(100).with(lam.to_bits());
            let (m, v, n) = mean_var((0..40_000).map(|i| poisson(root.with(i), lam) as f64));
            let se = (lam / n as f64).sqrt();
            assert!((m - lam).abs() < 5.0 * se + 1e-9, "λ={lam}: mean {m}");
            assert!((v / lam - 1.0).abs() < 0.05, "λ={lam}: var {v}");
        }
        assert_eq!(poisson(Key::from_seed(1), 0.0), 0);
        assert_eq!(poisson(Key::from_seed(1), -3.0), 0);
    }

    #[test]
    fn poisson_pmf_matches_exactly_for_small_lambda() {
        // Chi-square goodness of fit against the exact pmf, λ = 2.5 and 14.
        for &lam in &[2.5f64, 14.0] {
            let n = 200_000u64;
            let root = Key::from_seed(7).with(lam.to_bits());
            let mut counts = vec![0u64; 60];
            for i in 0..n {
                let k = poisson(root.with(i), lam) as usize;
                counts[k.min(59)] += 1;
            }
            let mut chi2 = 0.0;
            let mut dof = 0;
            let mut pk = exp(-lam);
            for (k, &obs) in counts.iter().enumerate().take(59) {
                if k > 0 {
                    pk *= lam / k as f64;
                }
                let expct = pk * n as f64;
                if expct >= 20.0 {
                    chi2 += (obs as f64 - expct).powi(2) / expct;
                    dof += 1;
                }
            }
            // Loose bound: chi2 / dof well under 2 for a correct sampler.
            assert!(
                chi2 / (dof as f64) < 2.0,
                "λ={lam}: chi2/dof = {}",
                chi2 / dof as f64
            );
        }
    }

    #[test]
    fn binomial_moments_both_regimes() {
        for &(n, p) in &[
            (10u64, 0.3f64),
            (100, 0.2),
            (100, 0.7),
            (1000, 0.5),
            (1_000_000, 0.01),
            (50_000, 0.93),
        ] {
            let root = Key::from_seed(55).with2(n, p.to_bits());
            let (m, v, cnt) = mean_var((0..40_000).map(|i| binomial(root.with(i), n, p) as f64));
            let mu = n as f64 * p;
            let var = mu * (1.0 - p);
            let se = (var / cnt as f64).sqrt();
            assert!(
                (m - mu).abs() < 5.0 * se + 1e-9,
                "n={n} p={p}: mean {m} vs {mu}"
            );
            assert!(
                (v / var - 1.0).abs() < 0.05,
                "n={n} p={p}: var {v} vs {var}"
            );
        }
    }

    #[test]
    fn binomial_edges() {
        let k = Key::from_seed(3);
        assert_eq!(binomial(k, 0, 0.5), 0);
        assert_eq!(binomial(k, 10, 0.0), 0);
        assert_eq!(binomial(k, 10, 1.0), 10);
        for i in 0..1000 {
            let x = binomial(k.with(i), 37, 0.9);
            assert!(x <= 37);
        }
    }

    #[test]
    fn binomial_btpe_pmf_chi_square() {
        // n·p = 60 → BTPE path. Compare to the exact pmf over the bulk.
        let (n, p) = (200u64, 0.3);
        let trials = 200_000u64;
        let root = Key::from_seed(99);
        let mut counts = vec![0u64; (n + 1) as usize];
        for i in 0..trials {
            counts[binomial(root.with(i), n, p) as usize] += 1;
        }
        // exact pmf via log-gamma
        let lp = |k: u64| {
            crate::dmath::ln_gamma(n as f64 + 1.0)
                - crate::dmath::ln_gamma(k as f64 + 1.0)
                - crate::dmath::ln_gamma((n - k) as f64 + 1.0)
                + k as f64 * ln(p)
                + (n - k) as f64 * ln(1.0 - p)
        };
        let (mut chi2, mut dof) = (0.0, 0);
        for k in 0..=n {
            let e = exp(lp(k)) * trials as f64;
            if e >= 20.0 {
                chi2 += (counts[k as usize] as f64 - e).powi(2) / e;
                dof += 1;
            }
        }
        assert!(
            chi2 / (dof as f64) < 2.0,
            "chi2/dof = {}",
            chi2 / dof as f64
        );
    }

    #[test]
    fn exponential_and_normal_moments() {
        let root = Key::from_seed(5);
        let (m, v, _) = mean_var((0..100_000).map(|i| exponential(root.with(i), 2.0)));
        assert!(
            (m - 0.5).abs() < 0.01 && (v - 0.25).abs() < 0.01,
            "exp: {m} {v}"
        );
        let root = Key::from_seed(6);
        let (m, v, _) = mean_var((0..100_000).map(|i| std_normal(root.with(i))));
        assert!(m.abs() < 0.01 && (v - 1.0).abs() < 0.02, "normal: {m} {v}");
    }

    #[test]
    fn lomax_survival_matches_power_law() {
        // S(t) = (1 + t)^gamma. Check at t = 1 and t = 3 for Burt's non-kin γ.
        let gamma = -0.716;
        let root = Key::from_seed(8);
        let n = 100_000;
        let draws: Vec<f64> = (0..n)
            .map(|i| lomax_lifetime(root.with(i), gamma))
            .collect();
        for &t in &[1.0f64, 3.0] {
            let emp = draws.iter().filter(|&&x| x > t).count() as f64 / n as f64;
            let want = pow(1.0 + t, gamma);
            assert!((emp - want).abs() < 0.01, "t={t}: {emp} vs {want}");
        }
    }

    /// Golden draws: any change to an algorithm, constant or `libm` version
    /// re-rolls every world, so it must fail here first.
    #[test]
    fn golden_draws() {
        let k = Key::from_seed(2026);
        let pois: Vec<u64> = [0.5f64, 7.0, 42.0, 1e6]
            .iter()
            .map(|&l| poisson(k.with(l.to_bits()), l))
            .collect();
        assert_eq!(pois, vec![1, 5, 47, 1_000_182]);
        let bin: Vec<u64> = [(10u64, 0.3f64), (1000, 0.5), (1_000_000, 0.01)]
            .iter()
            .map(|&(n, p)| binomial(k.with2(n, p.to_bits()), n, p))
            .collect();
        assert_eq!(bin, vec![3, 484, 10_158]);
        assert_eq!(std_normal(k).to_bits(), 0xBFF998051FEA170E);
        assert_eq!(lognormal(k, 1.0, 0.5).to_bits(), 0x3FF38BD4A7F01637);
        assert_eq!(lomax_lifetime(k, -0.716).to_bits(), 0x4058B581EAEC64FA);
    }

    #[test]
    fn samplers_are_pure_functions_of_the_key() {
        let k = Key::from_seed(123).with(4);
        assert_eq!(poisson(k, 37.5), poisson(k, 37.5));
        assert_eq!(binomial(k, 5000, 0.4), binomial(k, 5000, 0.4));
        assert_eq!(std_normal(k).to_bits(), std_normal(k).to_bits());
    }

    #[test]
    fn inversion_exponential_has_unit_mean() {
        let (m, v, _) = mean_var((0..200_000).map(|i| exp1_by_inversion(Key::from_seed(i).unit())));
        assert!((m - 1.0).abs() < 0.01 && (v - 1.0).abs() < 0.03, "{m} {v}");
        assert_eq!(exp1_by_inversion(0.0), 0.0);
    }

    #[test]
    fn linear_pick_follows_the_weights() {
        let w = [0.2, 0.0, 0.5, 0.3];
        let mut hits = [0u64; 4];
        let trials = 100_000;
        for i in 0..trials {
            hits[pick_linear(&w, Key::from_seed(i).unit())] += 1;
        }
        for (h, p) in hits.iter().zip(w) {
            assert!((*h as f64 / trials as f64 - p).abs() < 0.01);
        }
        assert_eq!(pick_linear(&w, 0.0), 0);
        assert_eq!(pick_linear(&[0.0, 0.0], 0.5), 0);
    }

    #[test]
    fn lazy_conditional_has_the_conditional_law() {
        // Geometric-ish ages 0..40; draw by inversion of the survival.
        let f: Vec<f64> = (0..40).map(|a| 0.9f64.powi(a) * 0.1).collect();
        let s = |a: usize| f[a..].iter().sum::<f64>();
        let draw = |req: usize, k: Key| {
            let target = k.unit() * s(req);
            let mut acc = 0.0;
            (req..40)
                .find(|&a| {
                    acc += f[a];
                    target < acc
                })
                .unwrap_or(39)
        };
        let (own, r, bound) = (3usize, 9usize, 15usize);
        let trials = 200_000u64;
        let mut hist = [0u64; 40];
        let mut computed = 0u64;
        for i in 0..trials {
            let k = Key::from_seed(i);
            let a = lazy_conditional(
                own,
                bound,
                || {
                    computed += 1;
                    r
                },
                draw,
                (k, k.with(3)),
            );
            assert!(a >= r);
            hist[a] += 1;
        }
        for a in r..40 {
            let want = f[a] / s(r);
            let got = hist[a] as f64 / trials as f64;
            assert!(
                (got - want).abs() < 4.0 * (want / trials as f64).sqrt() + 1e-4,
                "{a}"
            );
        }
        // r is computed only when the first draw falls below the bound.
        let below = 1.0 - s(bound) / s(own);
        assert!((computed as f64 / trials as f64 - below).abs() < 0.01);
    }

    #[test]
    fn sampling_helpers_golden() {
        let bits = [exp1_by_inversion(0.3), exp1_by_inversion(0.999)].map(f64::to_bits);
        assert_eq!(bits, GOLDEN_HELPERS);
        let picks: Vec<usize> = (0..6)
            .map(|i| pick_linear(&[0.1, 0.4, 0.2, 0.3], Key::from_seed(i).unit()))
            .collect();
        assert_eq!(picks, GOLDEN_PICKS);
    }

    const GOLDEN_HELPERS: [u64; 2] = [4600096904496365392, 4619463459452485535];
    const GOLDEN_PICKS: [usize; 6] = [1, 1, 3, 3, 1, 3];
}
