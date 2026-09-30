//! Structured derivation keys: allocation-free, integer-only randomness.
//!
//! Every random draw in the world is a pure function of a [`Key`]. Keys form
//! a tree: a root comes from the world seed, and children are derived from
//! integers (stream labels, entity ids, ordinals, absolute time buckets).
//! Deriving a child is one short xxh3 call over a stack buffer; no string
//! formatting and no heap allocation. That is what makes the time model
//! window-independent: a draw is keyed on *what* it is (bucket 7,412,
//! ordinal 3), never on the query that asked for it.
//!
//! Children derived with different arities ([`Key::with`], [`Key::with2`],
//! [`Key::with3`]) hash different input lengths, so they can never alias
//! each other.

use xxhash_rust::xxh3::xxh3_64_with_seed;

/// A 64-bit derivation key. Cheap to copy and to derive from.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct Key(u64);

/// Compile-time label hash (64-bit FNV-1a) for naming streams in `const`s.
///
/// ```
/// use procedural_core::key::{label, Key};
/// const BIRTHS: u64 = label("births");
/// let k = Key::from_seed(7).with(BIRTHS);
/// # let _ = k;
/// ```
pub const fn label(s: &str) -> u64 {
    let bytes = s.as_bytes();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut i = 0;
    while i < bytes.len() {
        h ^= bytes[i] as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
        i += 1;
    }
    h
}

impl Key {
    /// The root key for a world seed.
    #[inline]
    pub fn from_seed(seed: u64) -> Key {
        Key(xxh3_64_with_seed(b"internot-root", seed))
    }

    /// Wrap raw bits as a key. Prefer deriving from a root; this exists for
    /// keys that were stored or transmitted as integers.
    #[inline]
    pub const fn from_bits(bits: u64) -> Key {
        Key(bits)
    }

    /// The key's raw 64 bits (also a uniform random `u64`).
    #[inline]
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Child key derived from one integer.
    #[inline]
    pub fn with(self, a: u64) -> Key {
        Key(xxh3_64_with_seed(&a.to_le_bytes(), self.0))
    }

    /// Child key derived from two integers.
    #[inline]
    pub fn with2(self, a: u64, b: u64) -> Key {
        let mut buf = [0u8; 16];
        buf[..8].copy_from_slice(&a.to_le_bytes());
        buf[8..].copy_from_slice(&b.to_le_bytes());
        Key(xxh3_64_with_seed(&buf, self.0))
    }

    /// Child key derived from three integers.
    #[inline]
    pub fn with3(self, a: u64, b: u64, c: u64) -> Key {
        let mut buf = [0u8; 24];
        buf[..8].copy_from_slice(&a.to_le_bytes());
        buf[8..16].copy_from_slice(&b.to_le_bytes());
        buf[16..].copy_from_slice(&c.to_le_bytes());
        Key(xxh3_64_with_seed(&buf, self.0))
    }

    /// Uniform draw in `[0, 1)` from the top 53 bits.
    #[inline]
    pub fn unit(self) -> f64 {
        (self.0 >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Uniform draw in `(0, 1]`. Safe to pass to `ln`.
    #[inline]
    pub fn unit_open0(self) -> f64 {
        ((self.0 >> 11) + 1) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Uniform integer in `[0, n)` (Lemire's multiply-shift; no modulo bias
    /// beyond 2^-64). Panics if `n == 0`.
    #[inline]
    pub fn below(self, n: u64) -> u64 {
        assert!(n > 0, "Key::below bound must be positive");
        ((self.0 as u128 * n as u128) >> 64) as u64
    }

    /// Bernoulli draw with probability `p`.
    #[inline]
    pub fn chance(self, p: f64) -> bool {
        self.unit() < p
    }

    /// An endless stream of independent uniforms keyed by this key. The
    /// i-th value is always `self.with(i).unit()`, so rejection samplers
    /// that consume a variable number of draws stay pure functions of the
    /// key.
    #[inline]
    pub fn uniforms(self) -> Uniforms {
        Uniforms { key: self, i: 0 }
    }
}

/// Counter-based stream of uniforms in `[0, 1)`. See [`Key::uniforms`].
#[derive(Clone, Debug)]
pub struct Uniforms {
    key: Key,
    i: u64,
}

impl Uniforms {
    /// Next uniform in `[0, 1)`.
    #[inline]
    #[allow(clippy::should_implement_trait)] // infinite stream; `Iterator` adds nothing but Option noise
    pub fn next(&mut self) -> f64 {
        let u = self.key.with(self.i).unit();
        self.i += 1;
        u
    }

    /// Next uniform in `(0, 1]`.
    #[inline]
    pub fn next_open0(&mut self) -> f64 {
        let u = self.key.with(self.i).unit_open0();
        self.i += 1;
        u
    }

    /// How many uniforms have been consumed.
    #[inline]
    pub fn consumed(&self) -> u64 {
        self.i
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_is_fnv1a() {
        assert_eq!(label(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(label("a"), 0xaf63_dc4c_8601_ec8c);
        assert_ne!(label("births"), label("deaths"));
    }

    #[test]
    fn derivation_is_deterministic_and_distinct() {
        let k = Key::from_seed(42);
        assert_eq!(k.with(7), k.with(7));
        assert_ne!(k.with(7), k.with(8));
        assert_ne!(k.with(7), Key::from_seed(43).with(7));
        // Arity separation: equal integers at different arities differ.
        assert_ne!(k.with(0), k.with2(0, 0));
        assert_ne!(k.with2(0, 0), k.with3(0, 0, 0));
        assert_ne!(k.with2(1, 2), k.with2(2, 1));
    }

    #[test]
    fn unit_ranges() {
        let k = Key::from_seed(1);
        for i in 0..10_000 {
            let u = k.with(i).unit();
            assert!((0.0..1.0).contains(&u));
            let v = k.with(i).unit_open0();
            assert!(v > 0.0 && v <= 1.0);
        }
        assert_eq!(Key::from_bits(0).unit(), 0.0);
        assert_eq!(Key::from_bits(u64::MAX).unit_open0(), 1.0);
    }

    #[test]
    fn below_is_in_range_and_roughly_uniform() {
        let k = Key::from_seed(3);
        let mut counts = [0u32; 10];
        for i in 0..100_000 {
            let v = k.with(i).below(10);
            counts[v as usize] += 1;
        }
        for c in counts {
            assert!((9_500..10_500).contains(&c), "bucket count {c}");
        }
    }

    #[test]
    fn uniforms_stream_matches_with() {
        let k = Key::from_seed(9);
        let mut s = k.uniforms();
        for i in 0..5 {
            assert_eq!(s.next(), k.with(i).unit());
        }
        assert_eq!(s.consumed(), 5);
    }

    #[test]
    fn unit_mean_and_variance() {
        let k = Key::from_seed(11);
        let n = 200_000u64;
        let (mut s, mut s2) = (0.0, 0.0);
        for i in 0..n {
            let u = k.with(i).unit();
            s += u;
            s2 += u * u;
        }
        let mean = s / n as f64;
        let var = s2 / n as f64 - mean * mean;
        assert!((mean - 0.5).abs() < 0.003, "mean {mean}");
        assert!((var - 1.0 / 12.0).abs() < 0.002, "var {var}");
    }
}
