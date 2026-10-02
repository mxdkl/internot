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

#[cfg(test)]
mod tests {
    use super::*;

    /// Golden values: exact bit patterns. If a `libm` upgrade or compiler
    /// change alters any of these, every world built from a published seed
    /// would silently change. Update only as a deliberate world-version bump.
    #[test]
    fn golden_bits() {
        let cases: [(&str, f64, u64); 10] = [
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
    fn agrees_with_reference_values() {
        assert!((ln(std::f64::consts::E) - 1.0).abs() < 1e-15);
        assert!((exp(0.0) - 1.0).abs() == 0.0);
        assert!((ln_gamma(5.0) - 24f64.ln()).abs() < 1e-13);
        assert!((pow(9.0, 0.5) - 3.0).abs() < 1e-15);
    }
}
