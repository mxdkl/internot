//! Deterministic math: transcendental functions that return the same bits
//! on every machine.
//!
//! Rust documents `f64::ln`, `exp`, `powf`, `sin` and the other
//! transcendentals as having *unspecified precision*: results may vary by
//! platform, Rust version, and even between calls. A world built from a
//! published seed must be bit-identical everywhere, so substrate code calls
//! these wrappers instead. They delegate to the pinned pure-Rust `libm`
//! (FreeBSD/musl ports), whose only architecture-specific paths are for
//! operations IEEE 754 already defines exactly.
//!
//! `+ - * /`, `sqrt`, `floor`, `ceil`, `trunc` and `mul_add` are exact under
//! IEEE 754 and may be used directly.
//!
//! See `docs/superpowers/research/2026-09-29-deterministic-numerics.md`.

/// Natural logarithm.
#[inline]
pub fn ln(x: f64) -> f64 {
    libm::log(x)
}

/// `ln(1 + x)`, accurate for small `x`.
#[inline]
pub fn ln_1p(x: f64) -> f64 {
    libm::log1p(x)
}

/// `e^x`.
#[inline]
pub fn exp(x: f64) -> f64 {
    libm::exp(x)
}

/// `e^x - 1`, accurate for small `x`.
#[inline]
pub fn exp_m1(x: f64) -> f64 {
    libm::expm1(x)
}

/// `x^y`.
#[inline]
pub fn pow(x: f64, y: f64) -> f64 {
    libm::pow(x, y)
}

/// Sine.
#[inline]
pub fn sin(x: f64) -> f64 {
    libm::sin(x)
}

/// Cosine.
#[inline]
pub fn cos(x: f64) -> f64 {
    libm::cos(x)
}

/// Arcsine, in `[-π/2, π/2]`.
#[inline]
pub fn asin(x: f64) -> f64 {
    libm::asin(x)
}

/// Four-quadrant arctangent of `y / x`, in `[-π, π]`.
#[inline]
pub fn atan2(y: f64, x: f64) -> f64 {
    libm::atan2(y, x)
}

/// Hyperbolic cosine.
#[inline]
pub fn cosh(x: f64) -> f64 {
    libm::cosh(x)
}

/// Hyperbolic sine.
#[inline]
pub fn sinh(x: f64) -> f64 {
    libm::sinh(x)
}

/// Inverse hyperbolic cosine.
#[inline]
pub fn acosh(x: f64) -> f64 {
    libm::acosh(x)
}

/// Inverse hyperbolic tangent.
#[inline]
pub fn atanh(x: f64) -> f64 {
    libm::atanh(x)
}

/// Natural log of the gamma function, `ln Γ(x)`, for `x > 0`.
#[inline]
pub fn ln_gamma(x: f64) -> f64 {
    libm::lgamma(x)
}

/// Complementary error function, `erfc(x) = 1 − erf(x)`, accurate in the
/// tails.
#[inline]
pub fn erfc(x: f64) -> f64 {
    libm::erfc(x)
}

/// Standard normal CDF, `Φ(x) = erfc(−x/√2) / 2`, accurate in both tails.
#[inline]
pub fn norm_cdf(x: f64) -> f64 {
    0.5 * erfc(-x * std::f64::consts::FRAC_1_SQRT_2)
}

/// Standard normal quantile `Φ⁻¹(p)` for `p` in `[0, 1]` (±∞ at the ends):
/// Wichura's AS 241 (`PPND16`, Applied Statistics 37:477, 1988), relative
/// error about 1e-16. Rational functions in `p − ½` near the centre and
/// in `√(−ln min(p, 1 − p))` in the tails.
pub fn norm_quantile(p: f64) -> f64 {
    #[inline]
    fn poly(c: &[f64; 8], r: f64) -> f64 {
        c.iter().rev().fold(0.0, |acc, &k| acc * r + k)
    }
    const A: [f64; 8] = [
        3.387_132_872_796_366_6,
        1.331_416_678_917_843_8e2,
        1.971_590_950_306_551_4e3,
        1.373_169_376_550_946_1e4,
        4.592_195_393_154_987e4,
        6.726_577_092_700_87e4,
        3.343_057_558_358_813e4,
        2.509_080_928_730_122_7e3,
    ];
    const B: [f64; 8] = [
        1.0,
        4.231_333_070_160_091e1,
        6.871_870_074_920_579e2,
        5.394_196_021_424_751e3,
        2.121_379_430_158_659_7e4,
        3.930_789_580_009_271e4,
        2.872_908_573_572_194_3e4,
        5.226_495_278_852_854_5e3,
    ];
    const C: [f64; 8] = [
        1.423_437_110_749_683_6,
        4.630_337_846_156_545,
        5.769_497_221_460_691,
        3.647_848_324_763_204_5,
        1.270_458_252_452_368_4,
        2.417_807_251_774_506e-1,
        2.272_384_498_926_918_4e-2,
        7.745_450_142_783_414e-4,
    ];
    const D: [f64; 8] = [
        1.0,
        2.053_191_626_637_758_8,
        1.676_384_830_183_803_8,
        6.897_673_349_851e-1,
        1.481_039_764_274_800_8e-1,
        1.519_866_656_361_645_7e-2,
        5.475_938_084_995_345e-4,
        1.050_750_071_644_416_8e-9,
    ];
    const E: [f64; 8] = [
        6.657_904_643_501_103_8,
        5.463_784_911_164_114,
        1.784_826_539_917_291_3,
        2.965_605_718_285_049e-1,
        2.653_218_952_657_612_4e-2,
        1.242_660_947_388_078_4e-3,
        2.711_555_568_743_487_6e-5,
        2.010_334_399_292_288_1e-7,
    ];
    const F: [f64; 8] = [
        1.0,
        5.998_322_065_558_879e-1,
        1.369_298_809_227_358e-1,
        1.487_536_129_085_061_5e-2,
        7.868_691_311_456_133e-4,
        1.846_318_317_510_054_8e-5,
        1.421_511_758_316_446e-7,
        2.044_263_103_389_939_7e-15,
    ];
    if p <= 0.0 {
        return f64::NEG_INFINITY;
    }
    if p >= 1.0 {
        return f64::INFINITY;
    }
    let q = p - 0.5;
    if q.abs() <= 0.425 {
        let r = 0.180_625 - q * q;
        return q * poly(&A, r) / poly(&B, r);
    }
    let r = (-ln(if q < 0.0 { p } else { 1.0 - p })).sqrt();
    let v = if r <= 5.0 {
        let r = r - 1.6;
        poly(&C, r) / poly(&D, r)
    } else {
        let r = r - 5.0;
        poly(&E, r) / poly(&F, r)
    };
    if q < 0.0 {
        -v
    } else {
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Golden values: exact bit patterns. If a `libm` upgrade or compiler
    /// change alters any of these, every world built from a published seed
    /// would silently change. Update only as a deliberate world-version bump.
    #[test]
    fn golden_bits() {
        let cases: [(&str, f64, u64); 15] = [
            ("ln(2)", ln(2.0), 0x3FE62E42FEFA39EF),
            ("ln(0.1)", ln(0.1), 0xC0026BB1BBB55515),
            // libm's exp(1) is 1 ulp from the correctly rounded e
            // (0x4005BF0A8B145769). Reproducibility, not perfect rounding, is
            // the property pinned here.
            ("exp(1)", exp(1.0), 0x4005BF0A8B14576A),
            ("exp(-3.7)", exp(-3.7), 0x3F99511FC6871044),
            ("ln_1p(1e-9)", ln_1p(1e-9), 0x3E112E0BE801F1D9),
            ("pow(2.5, 1.7)", pow(2.5, 1.7), 0x4012FDCF53F3E6F3),
            ("sin(1)", sin(1.0), 0x3FEAED548F090CEE),
            ("ln_gamma(10.5)", ln_gamma(10.5), 0x402BE199A0F64393),
            ("asin(0.3)", asin(0.3), 0x3FD380159E14F6FF),
            ("atan2(1, -2)", atan2(1.0, -2.0), 0x40056C6E7397F5AE),
            ("erfc(0.7)", erfc(0.7), 0x3FD49EE7BDD1D374),
            ("norm_cdf(-1.3)", norm_cdf(-1.3), 0x3FB8C7EAA3883AE1),
            ("norm_quantile(0.975)", norm_quantile(0.975), 0x3FFF5C0331EEFF82),
            ("norm_quantile(0.02)", norm_quantile(0.02), 0xC0006E13E8AADFDB),
            ("norm_quantile(1e-20)", norm_quantile(1e-20), 0xC022865170B43A4C),
        ];
        for (name, got, want) in cases {
            assert_eq!(
                got.to_bits(),
                want,
                "{name}: got {got:e} (0x{:016X})",
                got.to_bits()
            );
        }
    }

    #[test]
    fn normal_quantile_and_cdf_match_references_and_invert() {
        // Reference values (R: qnorm, pnorm).
        let q = [
            (0.975, 1.959_963_984_540_054),
            (0.5, 0.0),
            (0.1, -1.281_551_565_544_600_5),
            (1e-10, -6.361_340_902_404_056),
            (1e-6, -4.753_424_308_822_899),
            (1e-300, -37.047_096_299_361_2),
        ];
        for (p, want) in q {
            let got = norm_quantile(p);
            assert!((got - want).abs() <= 2e-15 * want.abs().max(1.0), "q({p}) = {got}");
        }
        assert!((norm_cdf(1.959_963_984_540_054) - 0.975).abs() < 1e-16);
        assert!((norm_cdf(-6.361_340_902_404_056) / 1e-10 - 1.0).abs() < 1e-13);
        assert_eq!(norm_quantile(0.0), f64::NEG_INFINITY);
        assert_eq!(norm_quantile(1.0), f64::INFINITY);
        // Round trip over the centre and both tails.
        for i in 1..2000 {
            let p = i as f64 / 2000.0;
            assert!((norm_cdf(norm_quantile(p)) - p).abs() < 4e-16, "p {p}");
        }
        for k in 1..300 {
            let p = 10f64.powi(-k);
            assert!((norm_cdf(norm_quantile(p)) / p - 1.0).abs() < 1e-12, "p 1e-{k}");
        }
        // Symmetry, exact where `1 − p` is (dyadic p).
        for i in 1..512 {
            let p = i as f64 / 1024.0;
            assert_eq!(norm_quantile(p), -norm_quantile(1.0 - p), "p {p}");
        }
    }

    #[test]
    fn agrees_with_reference_values() {
        assert!((ln(std::f64::consts::E) - 1.0).abs() < 1e-15);
        assert!((exp(0.0) - 1.0).abs() == 0.0);
        assert!((ln_gamma(5.0) - 24f64.ln()).abs() < 1e-13);
        assert!((pow(9.0, 0.5) - 3.0).abs() < 1e-15);
    }
}
