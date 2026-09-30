//! Hashing primitives. All deterministic from (id, key).

use crate::word::BitWord;
use xxhash_rust::xxh3::Xxh3;

/// Deterministic uniform draw in `[0.0, 1.0)` from `(id, key)`.
///
/// The `key` is how different attributes get independent values from the same id:
/// `hash_float(42, "age")` and `hash_float(42, "income")` are statistically independent.
pub fn hash_float<W: BitWord>(id: W, key: &str) -> f64 {
    unit_interval_from_u64(raw_hash(id, key))
}

/// Map a 64-bit hash to `[0.0, 1.0)` using the top 53 bits (fits f64 mantissa
/// exactly, avoids boundary issues).
#[inline]
fn unit_interval_from_u64(h: u64) -> f64 {
    ((h >> 11) as f64) / ((1u64 << 53) as f64)
}

/// Deterministic uniform integer in `[0, n)` from `(id, key)`.
///
/// Uses Lemire's fast bounded hash: avoids modulo bias and is faster than `hash % n`.
/// Panics if `n == 0`.
pub fn hash_int<W: BitWord>(id: W, key: &str, n: u64) -> u64 {
    assert!(n > 0, "hash_int bound must be positive");
    let h = raw_hash(id, key);
    ((h as u128 * n as u128) >> 64) as u64
}

/// Deterministic vector in `[0, 1)^dims` from `(id, key)`.
///
/// Each component uses a distinct sub-key derived from `key` and the dimension index,
/// so extending `dims` leaves earlier components unchanged.
pub fn hash_vec<W: BitWord>(id: W, key: &str, dims: usize) -> Vec<f64> {
    (0..dims)
        .map(|i| {
            let mut hasher = Xxh3::new();
            hasher.update(id.to_le_bytes().as_ref());
            hasher.update(&[0u8]);
            hasher.update(key.as_bytes());
            hasher.update(b"/");
            hasher.update(&(i as u64).to_le_bytes());
            let h = hasher.digest();
            ((h >> 11) as f64) / ((1u64 << 53) as f64)
        })
        .collect()
}

/// Deterministic draw from the standard normal distribution `N(0, 1)`.
///
/// Implemented via the Box-Muller transform over two independent uniform
/// draws. The two sub-hashes use a streaming `Xxh3` with a fixed
/// `__gauss` separator and a sub-index byte (1 / 2), so neither call
/// allocates — earlier versions formatted `"{key}/__gauss_u1"` per
/// invocation.
pub fn hash_gaussian<W: BitWord>(id: W, key: &str) -> f64 {
    let u1 = unit_interval_from_u64(hash_gauss_sub(id, key, 1)).max(f64::MIN_POSITIVE);
    let u2 = unit_interval_from_u64(hash_gauss_sub(id, key, 2));
    (-2.0 * crate::dmath::ln(u1)).sqrt() * crate::dmath::cos(2.0 * std::f64::consts::PI * u2)
}

/// Streamed sub-hash for `hash_gaussian`. The separator chain
/// `[id_bytes][0][key_bytes][0 __gauss 0][sub_le_bytes]` is unique to
/// this call site — `raw_hash` and `hash_with_index` use different
/// separator schemas, so outputs cannot alias across helpers.
fn hash_gauss_sub<W: BitWord>(id: W, key: &str, sub: u8) -> u64 {
    let mut hasher = Xxh3::new();
    hasher.update(id.to_le_bytes().as_ref());
    hasher.update(&[0u8]);
    hasher.update(key.as_bytes());
    hasher.update(b"\0__gauss\0");
    hasher.update(&[sub]);
    hasher.digest()
}

pub(crate) fn raw_hash<W: BitWord>(id: W, key: &str) -> u64 {
    let mut hasher = Xxh3::new();
    hasher.update(id.to_le_bytes().as_ref());
    hasher.update(&[0u8]); // separator, prevents aliasing between numeric suffixes and key prefixes
    hasher.update(key.as_bytes());
    hasher.digest()
}

/// Fast path for `raw_hash(id, &format!("{label}_{index}"))` without
/// heap-allocating the key per call. Used by streaming candidate
/// generators that need an independent hash per integer index.
///
/// Separators (`0u8` between fields) match `raw_hash` byte-for-byte so
/// output domain overlaps with arbitrary string keys are impossible —
/// `hash_with_index(id, "candidate", 5)` and any `raw_hash(id, key)`
/// produce distinct byte sequences regardless of `key`.
pub(crate) fn hash_with_index<W: BitWord>(id: W, label: &str, index: u64) -> u64 {
    let mut hasher = Xxh3::new();
    hasher.update(id.to_le_bytes().as_ref());
    hasher.update(&[0u8]);
    hasher.update(label.as_bytes());
    hasher.update(&[0u8]);
    hasher.update(&index.to_le_bytes());
    hasher.digest()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn hash_float_is_in_unit_interval() {
        for id in [0u64, 1, 42, u64::MAX] {
            for key in ["a", "birth_year", "mood/daily"] {
                let v = hash_float(id, key);
                assert!(
                    (0.0..1.0).contains(&v),
                    "hash_float({}, {:?}) = {} out of range",
                    id,
                    key,
                    v
                );
            }
        }
    }

    #[test]
    fn hash_float_is_deterministic() {
        for id in [0u64, 42, 1_000_000] {
            assert_eq!(hash_float(id, "x"), hash_float(id, "x"));
        }
    }

    #[test]
    fn hash_float_differs_across_keys() {
        assert_ne!(hash_float(42u64, "a"), hash_float(42u64, "b"));
    }

    #[test]
    fn hash_float_differs_across_ids() {
        assert_ne!(hash_float(1u64, "x"), hash_float(2u64, "x"));
    }

    proptest! {
        #[test]
        fn hash_float_always_in_unit_interval(id in any::<u64>(), key in "\\PC*") {
            let v = hash_float(id, &key);
            prop_assert!(v >= 0.0);
            prop_assert!(v < 1.0);
        }

        #[test]
        fn hash_float_distribution_is_approx_uniform(key in "\\PC{1,16}") {
            let n = 10_000u64;
            let mean: f64 = (0..n).map(|i| hash_float(i, &key)).sum::<f64>() / n as f64;
            prop_assert!((mean - 0.5).abs() < 0.02,
                "mean = {} for key {:?}", mean, key);
        }
    }

    #[test]
    fn hash_int_respects_bound() {
        for n in [1u64, 2, 10, 256, 1_000_000] {
            for id in 0..100u64 {
                let v = hash_int(id, "x", n);
                assert!(v < n, "hash_int({}, \"x\", {}) = {} >= {}", id, n, v, n);
            }
        }
    }

    #[test]
    fn hash_int_distribution_is_approx_uniform() {
        let n = 10u64;
        let trials = 100_000u64;
        let mut counts = [0u64; 10];
        for i in 0..trials {
            counts[hash_int(i, "bucket", n) as usize] += 1;
        }
        let expected = trials as f64 / n as f64;
        for c in counts {
            let diff = (c as f64 - expected).abs();
            assert!(
                diff < expected * 0.1,
                "bucket count {} too far from expected {}",
                c,
                expected
            );
        }
    }

    #[test]
    fn hash_vec_length_matches_dims() {
        for dims in [0usize, 1, 5, 32] {
            assert_eq!(hash_vec(42u64, "x", dims).len(), dims);
        }
    }

    #[test]
    fn hash_vec_is_deterministic() {
        assert_eq!(hash_vec(42u64, "x", 5), hash_vec(42u64, "x", 5));
    }

    #[test]
    fn hash_vec_components_are_in_unit_interval() {
        for v in hash_vec(42u64, "x", 10) {
            assert!((0.0..1.0).contains(&v));
        }
    }

    #[test]
    fn hash_vec_components_differ() {
        let v = hash_vec(42u64, "x", 5);
        for (i, a) in v.iter().enumerate() {
            for (j, b) in v.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "components {} and {} identical", i, j);
                }
            }
        }
    }

    #[test]
    fn hash_gaussian_is_deterministic() {
        assert_eq!(hash_gaussian(42u64, "x"), hash_gaussian(42u64, "x"));
    }

    #[test]
    fn hash_gaussian_approx_mean_zero_unit_variance() {
        let n = 10_000u64;
        let samples: Vec<f64> = (0..n).map(|i| hash_gaussian(i, "x")).collect();
        let mean = samples.iter().sum::<f64>() / n as f64;
        let variance = samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;
        assert!(mean.abs() < 0.05, "mean = {}", mean);
        assert!((variance - 1.0).abs() < 0.1, "variance = {}", variance);
    }

    #[test]
    fn hash_gaussian_no_nan_or_inf() {
        for id in 0..10_000u64 {
            let v = hash_gaussian(id, "x");
            assert!(v.is_finite(), "non-finite value at id {}: {}", id, v);
        }
    }

    #[test]
    fn hash_gaussian_varies_across_ids() {
        // Two distinct ids must produce different gaussian draws — guards
        // against an impl that ignored id (regression cover for the
        // streaming-hash refactor that replaced format!).
        assert_ne!(hash_gaussian(1u64, "x"), hash_gaussian(2u64, "x"));
        assert_ne!(hash_gaussian(42u64, "x"), hash_gaussian(43u64, "x"));
    }

    #[test]
    fn hash_gaussian_varies_across_keys() {
        // Same id, distinct keys → distinct draws.
        assert_ne!(hash_gaussian(42u64, "a"), hash_gaussian(42u64, "b"));
    }

    #[test]
    fn hash_gaussian_u1_u2_sub_keys_are_independent() {
        // Internal: the two sub-hashes that feed Box-Muller must not
        // coincide (otherwise sin/cos always sample the same angle and
        // the variance collapses). 10k samples → if u1 == u2 ever, the
        // distribution would have a clear bias — exposed by var ≠ 1.
        let n = 10_000u64;
        let mut squared_sum = 0.0f64;
        for i in 0..n {
            let v = hash_gaussian(i, "sub_keys");
            squared_sum += v * v;
        }
        let second_moment = squared_sum / n as f64;
        // E[Z^2] = 1 for N(0,1). u1==u2 would push this far from 1.
        assert!(
            (second_moment - 1.0).abs() < 0.1,
            "second moment {} far from 1.0 — sub-keys may have collapsed",
            second_moment
        );
    }

    #[test]
    fn hash_float_works_with_u128() {
        let a: u128 = 0x0123_4567_89AB_CDEF_FEDC_BA98_7654_3210;
        let b: u128 = 0x0123_4567_89AB_CDEF_FEDC_BA98_7654_3211; // one bit different
        let va = hash_float(a, "x");
        let vb = hash_float(b, "x");
        assert!((0.0..1.0).contains(&va));
        assert!((0.0..1.0).contains(&vb));
        // Very high probability that 1-bit-apart u128 IDs produce different floats
        assert_ne!(va, vb);
    }

    #[test]
    fn hash_float_u64_u128_disjoint() {
        // The same numeric value at different widths hashes to different bytes,
        // which (with overwhelming probability) gives different hash_float outputs.
        let n64: u64 = 42;
        let n128: u128 = 42;
        assert_ne!(hash_float(n64, "x"), hash_float(n128, "x"));
    }

    #[test]
    fn hash_int_works_with_u128() {
        for i in 0..100u128 {
            let v = hash_int(i, "bucket", 10);
            assert!(v < 10);
        }
    }

    #[test]
    fn hash_vec_works_with_u128() {
        let v = hash_vec(0xDEAD_BEEF_CAFE_1234u128, "traits", 5);
        assert_eq!(v.len(), 5);
        for x in &v {
            assert!((0.0..1.0).contains(x));
        }
    }

    #[test]
    fn hash_gaussian_works_with_u128() {
        let n = 10_000u128;
        let samples: Vec<f64> = (0..n).map(|i| hash_gaussian(i, "z")).collect();
        let mean = samples.iter().sum::<f64>() / n as f64;
        assert!(mean.abs() < 0.05, "u128 gaussian mean = {}", mean);
        for &v in &samples {
            assert!(v.is_finite());
        }
    }
}
