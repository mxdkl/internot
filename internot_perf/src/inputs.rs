//! Deterministic input generation for suites.

/// A small deterministic generator (SplitMix64) for benchmark inputs.
/// Suites seed it with a constant so every run measures the same inputs;
/// the slowest inputs are also recorded by value, so they can be replayed
/// without the generator.
#[derive(Clone, Debug)]
pub struct InputRng {
    state: u64,
}

impl InputRng {
    /// A generator with the given seed.
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Uniform over all `u64`.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform over all `u128`.
    pub fn next_u128(&mut self) -> u128 {
        (u128::from(self.next_u64()) << 64) | u128::from(self.next_u64())
    }

    /// Uniform in `[0, n)`, for sampling ids from a counted space. Uses
    /// Lemire's multiply-shift, whose bias is below 2⁻³² for any `n` that
    /// fits in 32 bits.
    ///
    /// # Panics
    ///
    /// Panics if `n == 0`.
    pub fn below(&mut self, n: u64) -> u64 {
        assert!(n > 0, "InputRng::below needs a positive bound");
        ((u128::from(self.next_u64()) * u128::from(n)) >> 64) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let a: Vec<u64> = (0..5)
            .scan(InputRng::new(7), |r, _| Some(r.next_u64()))
            .collect();
        let b: Vec<u64> = (0..5)
            .scan(InputRng::new(7), |r, _| Some(r.next_u64()))
            .collect();
        assert_eq!(a, b);
        assert_ne!(a[0], a[1]);
    }

    #[test]
    fn below_stays_in_range_and_covers_it() {
        let mut rng = InputRng::new(1);
        let mut seen = [false; 10];
        for _ in 0..1_000 {
            let v = rng.below(10);
            assert!(v < 10);
            seen[v as usize] = true;
        }
        assert!(seen.iter().all(|&s| s));
    }
}
