//! Keyed bijections on `[0, n)`: the coordinate systems of the world.
//!
//! A coordinate system maps a dense index space onto (group, slot)
//! positions. Group members are preimages, so listing a group needs no
//! search, and both sides of a relation read one shared structure.
//!
//! - [`FeistelPerm`]: Black–Rogaway generalized Feistel over `Z_a × Z_b`
//!   with cycle-walking (CT-RSA 2002, Method 3). 6 rounds by default, never
//!   fewer than 4: with 2–3 rounds, sequential inputs keep visible structure
//!   (adjacency chi²/df ≈ 19 measured). Integer-only, no allocation.
//! - [`CompactPerm`]: the same network with 6 rounds in 32 bytes, for
//!   permutations stored by the thousands in tables whose lookups are
//!   bound by memory traffic.
//! - [`GrowablePerm`]: cycle-walks a fixed-capacity permutation into
//!   `[0, n)`, so growing `n` by one changes exactly one existing image.
//! - [`SmallPerm`]: exact uniform permutation for `n ≤ 64` by sorting keyed
//!   hashes (Black–Rogaway Method 1), on the stack.
//! - [`SegmentedPerm`]: a keyed bijection from a dense domain onto labelled
//!   segments of given sizes, with the inverse (the static half of a
//!   counted flow).
//! - [`least_cost_assignment`]: the cheapest assignment of a group of at
//!   most three to three partners, by exhaustive search in a fixed order.
//!
//! See `docs/superpowers/research/2026-09-29-local-access-and-bijections.md` §2.

use crate::key::Key;

/// A bijection on `[0, len)`.
pub trait Bijection {
    /// Domain size.
    fn len(&self) -> u64;
    /// Forward map. Panics if `x >= len`.
    fn fwd(&self, x: u64) -> u64;
    /// Inverse map. Panics if `y >= len`.
    fn inv(&self, y: u64) -> u64;
    /// True if the domain is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Default Feistel round count.
pub const DEFAULT_ROUNDS: u8 = 6;
/// Minimum allowed Feistel round count.
pub const MIN_ROUNDS: u8 = 4;
const MAX_ROUNDS: usize = 12;

/// splitmix64 finalizer: a strong 64-bit mixer used as the round function.
#[inline(always)]
fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// `⌈√n⌉` for `n >= 1`, exact. `f64::sqrt` is correctly rounded (IEEE 754);
/// the correction loops fix the rounding of `n` to `f64`.
fn ceil_sqrt(n: u64) -> u64 {
    let mut a = (n as f64).sqrt() as u64;
    while a > 0 && a * a > n {
        a -= 1;
    }
    while (a + 1) * (a + 1) <= n {
        a += 1;
    }
    if a * a < n {
        a + 1
    } else {
        a
    }
}

/// `(x / a, x % a)` by multiply-and-correct with the precomputed inverse
/// `a_inv = ⌊(2^64 − 1) / a⌋` (Granlund–Montgomery). The estimate is at most
/// one short, so a single correction step makes it exact for every
/// `x < 2^64`.
#[inline(always)]
fn divmod(x: u64, a: u64, a_inv: u64) -> (u64, u64) {
    let mut q = ((x as u128 * a_inv as u128) >> 64) as u64;
    let mut r = x - q * a;
    if r >= a {
        q += 1;
        r -= a;
    }
    (q, r)
}

/// Round function `F(r)` for round key `k`, reduced to `[0, modulus)`.
#[inline(always)]
fn round(k: u64, r: u64, modulus: u64) -> u64 {
    ((mix64(r ^ k) as u128 * modulus as u128) >> 64) as u64
}

/// One pass of the generalized Feistel network over `[0, a·b)` with
/// `rounds` rounds and round keys `key(j)`. Each round adds two values below
/// the modulus, so the reduction is a conditional subtraction rather than a
/// 64-bit division.
#[inline(always)]
fn encrypt(m: u64, a: u64, b: u64, a_inv: u64, rounds: usize, key: impl Fn(usize) -> u64) -> u64 {
    let (mut r, mut l) = divmod(m, a, a_inv);
    for j in 0..rounds {
        let modulus = if j % 2 == 0 { a } else { b };
        let mut next = l + round(key(j), r, modulus);
        if next >= modulus {
            next -= modulus;
        }
        l = r;
        r = next;
    }
    if rounds % 2 == 1 {
        a * l + r
    } else {
        a * r + l
    }
}

/// Inverse of [`encrypt`] over `[0, a·b)`.
#[inline(always)]
fn decrypt(y: u64, a: u64, b: u64, a_inv: u64, rounds: usize, key: impl Fn(usize) -> u64) -> u64 {
    let (q, rem) = divmod(y, a, a_inv);
    let (mut l, mut r) = if rounds % 2 == 1 { (q, rem) } else { (rem, q) };
    for j in (0..rounds).rev() {
        let modulus = if j % 2 == 0 { a } else { b };
        let prev_r = l;
        let f = round(key(j), l, modulus);
        let prev_l = if r >= f { r - f } else { r + modulus - f };
        l = prev_l;
        r = prev_r;
    }
    a * r + l
}

/// The Feistel domain `a × b` for `n`: `a = ⌈√n⌉`, `b = ⌈n / a⌉`, and `a`'s
/// division inverse.
fn shape(n: u64) -> (u64, u64, u64) {
    assert!(
        n <= 1u64 << 62,
        "domain too large for a Feistel permutation: {n}"
    );
    let (a, b) = if n == 0 {
        (0, 0)
    } else {
        let a = ceil_sqrt(n);
        (a, n.div_ceil(a))
    };
    (a, b, u64::MAX.checked_div(a).unwrap_or(0))
}

/// Keyed permutation of `[0, n)` via a generalized Feistel network.
#[derive(Clone, Debug)]
pub struct FeistelPerm {
    n: u64,
    a: u64,
    b: u64,
    /// `⌊(2^64 − 1) / a⌋`, for division by `a` without a divide instruction.
    a_inv: u64,
    rounds: u8,
    keys: [u64; MAX_ROUNDS],
}

impl FeistelPerm {
    /// A permutation of `[0, n)` with the default 6 rounds.
    pub fn new(n: u64, key: Key) -> Self {
        Self::with_rounds(n, key, DEFAULT_ROUNDS)
    }

    /// A permutation of `[0, n)` with `rounds` in `4..=12`.
    pub fn with_rounds(n: u64, key: Key, rounds: u8) -> Self {
        assert!(
            (MIN_ROUNDS as usize..=MAX_ROUNDS).contains(&(rounds as usize)),
            "Feistel rounds must be in 4..=12, got {rounds}"
        );
        let (a, b, a_inv) = shape(n);
        let mut keys = [0u64; MAX_ROUNDS];
        for (j, k) in keys.iter_mut().enumerate().take(rounds as usize) {
            *k = key.with(j as u64).bits();
        }
        Self {
            n,
            a,
            b,
            a_inv,
            rounds,
            keys,
        }
    }

    #[cfg(test)]
    fn divmod_a(&self, x: u64) -> (u64, u64) {
        divmod(x, self.a, self.a_inv)
    }

    #[inline(always)]
    fn encrypt(&self, m: u64) -> u64 {
        encrypt(m, self.a, self.b, self.a_inv, self.rounds as usize, |j| {
            self.keys[j]
        })
    }

    #[inline(always)]
    fn decrypt(&self, y: u64) -> u64 {
        decrypt(y, self.a, self.b, self.a_inv, self.rounds as usize, |j| {
            self.keys[j]
        })
    }
}

impl Bijection for FeistelPerm {
    #[inline]
    fn len(&self) -> u64 {
        self.n
    }

    #[inline]
    fn fwd(&self, x: u64) -> u64 {
        assert!(x < self.n, "FeistelPerm::fwd: {x} out of [0, {})", self.n);
        let mut y = self.encrypt(x);
        while y >= self.n {
            y = self.encrypt(y);
        }
        y
    }

    #[inline]
    fn inv(&self, y: u64) -> u64 {
        assert!(y < self.n, "FeistelPerm::inv: {y} out of [0, {})", self.n);
        let mut x = self.decrypt(y);
        while x >= self.n {
            x = self.decrypt(x);
        }
        x
    }
}

/// Round-key tweaks of [`CompactPerm`]: round `j` keys with
/// `seed ^ ROUND_TWEAKS[j]`. Nothing-up-my-sleeve constants (the first
/// hexadecimal digits of π, as in Blowfish's P-array), distinct and dense in
/// bits, so the six round functions `mix64(r ^ seed ^ T_j)` differ for every
/// seed.
const ROUND_TWEAKS: [u64; 6] = [
    0x243F_6A88_85A3_08D3,
    0x1319_8A2E_0370_7344,
    0xA409_3822_299F_31D0,
    0x082E_FA98_EC4E_6C89,
    0x4528_21E6_38D0_1377,
    0xBE54_66CF_34E9_0C6C,
];

/// Keyed permutation of `[0, n)`: the [`FeistelPerm`] network with 6 rounds,
/// stored in 32 bytes instead of 136.
///
/// The round keys are `seed ^ ROUND_TWEAKS[j]`, one seed rather than six
/// stored hashes, and the domain shape is kept in 32-bit halves (`n ≤ 2^62`
/// gives `a, b ≤ 2^31`). Evaluation costs the same as [`FeistelPerm`]. Use it
/// where permutations are stored by the thousands and lookups are bound by
/// memory traffic: two fit in a cache line. It is a different permutation
/// family from [`FeistelPerm`] (see its golden values).
#[derive(Clone, Copy, Debug)]
pub struct CompactPerm {
    n: u64,
    a_inv: u64,
    seed: u64,
    a: u32,
    b: u32,
}

/// A [`CompactPerm`] without its size: 24 bytes, for callers that already
/// store the size (a table of many small permutations). Rebuild the
/// permutation with [`CompactPerm::from_parts`]; that is a copy, not a
/// recomputation.
#[derive(Clone, Copy, Debug)]
pub struct CompactParts {
    a_inv: u64,
    seed: u64,
    a: u32,
    b: u32,
}

impl CompactPerm {
    /// A permutation of `[0, n)` keyed by `key`.
    pub fn new(n: u64, key: Key) -> Self {
        let (a, b, a_inv) = shape(n);
        Self {
            n,
            a_inv,
            seed: key.bits(),
            a: a as u32,
            b: b as u32,
        }
    }

    /// The permutation without its size (see [`CompactParts`]).
    pub fn parts(&self) -> CompactParts {
        CompactParts {
            a_inv: self.a_inv,
            seed: self.seed,
            a: self.a,
            b: self.b,
        }
    }

    /// The permutation of `[0, n)` whose parts are `p`; `n` must be the size
    /// it was built with.
    #[inline]
    pub fn from_parts(n: u64, p: CompactParts) -> Self {
        Self {
            n,
            a_inv: p.a_inv,
            seed: p.seed,
            a: p.a,
            b: p.b,
        }
    }

    #[inline(always)]
    fn encrypt(&self, m: u64) -> u64 {
        let seed = self.seed;
        encrypt(m, self.a as u64, self.b as u64, self.a_inv, 6, |j| {
            seed ^ ROUND_TWEAKS[j]
        })
    }

    #[inline(always)]
    fn decrypt(&self, y: u64) -> u64 {
        let seed = self.seed;
        decrypt(y, self.a as u64, self.b as u64, self.a_inv, 6, |j| {
            seed ^ ROUND_TWEAKS[j]
        })
    }
}

impl Bijection for CompactPerm {
    #[inline]
    fn len(&self) -> u64 {
        self.n
    }

    #[inline]
    fn fwd(&self, x: u64) -> u64 {
        assert!(x < self.n, "CompactPerm::fwd: {x} out of [0, {})", self.n);
        let mut y = self.encrypt(x);
        while y >= self.n {
            y = self.encrypt(y);
        }
        y
    }

    #[inline]
    fn inv(&self, y: u64) -> u64 {
        assert!(y < self.n, "CompactPerm::inv: {y} out of [0, {})", self.n);
        let mut x = self.decrypt(y);
        while x >= self.n {
            x = self.decrypt(x);
        }
        x
    }
}

/// A permutation over a fixed capacity, cycle-walked into a live prefix
/// `[0, n)`. Growing `n` to `n + 1` changes exactly one existing image, so
/// blocks whose size is revised keep almost every assignment.
#[derive(Clone, Debug)]
pub struct GrowablePerm {
    inner: FeistelPerm,
}

impl GrowablePerm {
    /// A growable permutation with the given capacity.
    pub fn new(capacity: u64, key: Key) -> Self {
        Self {
            inner: FeistelPerm::new(capacity, key),
        }
    }

    /// Capacity (largest supported live size).
    pub fn capacity(&self) -> u64 {
        self.inner.len()
    }

    /// Forward map within the live prefix `[0, n)`.
    #[inline]
    pub fn fwd(&self, x: u64, n: u64) -> u64 {
        assert!(x < n && n <= self.capacity(), "GrowablePerm::fwd({x}, {n})");
        let mut y = self.inner.fwd(x);
        while y >= n {
            y = self.inner.fwd(y);
        }
        y
    }

    /// Inverse map within the live prefix `[0, n)`.
    #[inline]
    pub fn inv(&self, y: u64, n: u64) -> u64 {
        assert!(y < n && n <= self.capacity(), "GrowablePerm::inv({y}, {n})");
        let mut x = self.inner.inv(y);
        while x >= n {
            x = self.inner.inv(x);
        }
        x
    }
}

/// Largest domain [`SmallPerm`] supports.
pub const SMALL_PERM_MAX: usize = 64;

/// Exact uniform permutation of `[0, n)` for `n <= 64`, built by sorting
/// keyed hashes. Lives on the stack; building costs `n` hashes plus a sort.
#[derive(Clone, Copy, Debug)]
pub struct SmallPerm {
    n: u8,
    fwd: [u8; SMALL_PERM_MAX],
    inv: [u8; SMALL_PERM_MAX],
}

impl SmallPerm {
    /// Build the permutation of `[0, n)` for `key`.
    pub fn new(n: usize, key: Key) -> Self {
        assert!(n <= SMALL_PERM_MAX, "SmallPerm supports n <= 64, got {n}");
        let mut order = [(0u64, 0u8); SMALL_PERM_MAX];
        for (i, slot) in order.iter_mut().enumerate().take(n) {
            *slot = (key.with(i as u64).bits(), i as u8);
        }
        order[..n].sort_unstable();
        let mut fwd = [0u8; SMALL_PERM_MAX];
        let mut inv = [0u8; SMALL_PERM_MAX];
        for (pos, &(_, i)) in order[..n].iter().enumerate() {
            fwd[i as usize] = pos as u8;
            inv[pos] = i;
        }
        Self {
            n: n as u8,
            fwd,
            inv,
        }
    }
}

impl Bijection for SmallPerm {
    #[inline]
    fn len(&self) -> u64 {
        self.n as u64
    }

    #[inline]
    fn fwd(&self, x: u64) -> u64 {
        assert!(x < self.n as u64, "SmallPerm::fwd: {x} out of range");
        self.fwd[x as usize] as u64
    }

    #[inline]
    fn inv(&self, y: u64) -> u64 {
        assert!(y < self.n as u64, "SmallPerm::inv: {y} out of range");
        self.inv[y as usize] as u64
    }
}

/// Every permutation of `0..k` for `k ≤ 3`, in lexicographic order
/// (identity first).
const SMALL_PERMS: [&[&[u8]]; 4] = [
    &[&[]],
    &[&[0]],
    &[&[0, 1], &[1, 0]],
    &[
        &[0, 1, 2],
        &[0, 2, 1],
        &[1, 0, 2],
        &[1, 2, 0],
        &[2, 0, 1],
        &[2, 1, 0],
    ],
];

/// The assignment `a → π(a)` of `0..k` (`k ≤ 3`) minimizing
/// `Σ_a cost(a, π(a))`, the first in lexicographic order among ties (so the
/// identity whenever it is optimal). Entries past `k` stay the identity.
pub fn least_cost_assignment(k: usize, cost: impl Fn(usize, usize) -> u32) -> [u8; 3] {
    assert!(k <= 3, "groups of at most three");
    let mut perm = [0u8, 1, 2];
    let mut best = u32::MAX;
    for cand in SMALL_PERMS[k] {
        let c: u32 = (0..k).map(|a| cost(a, cand[a] as usize)).sum();
        if c < best {
            best = c;
            perm[..k].copy_from_slice(cand);
        }
    }
    perm
}

/// A keyed bijection from a dense domain `[0, n)` onto consecutive labelled
/// segments of given sizes (`n` is their sum).
///
/// It is the static half of a counted flow: a count table says how many
/// items each segment (a destination, a quota cell) receives, and a keyed
/// permutation decides which items. Forward, [`Self::assign`] gives an
/// item's segment and offset; inverse, [`Self::member`] gives the item
/// holding a segment's offset. So a segment's members are enumerated in
/// O(size), with no search.
#[derive(Clone, Debug)]
pub struct SegmentedPerm {
    perm: CompactPerm,
    /// `starts[k]` is where segment `k` begins; the last entry is `n`.
    starts: Vec<u64>,
}

impl SegmentedPerm {
    /// Segments of `sizes` over `[0, Σ sizes)`, shuffled by `key`.
    pub fn new(sizes: &[u64], key: Key) -> Self {
        let mut starts = Vec::with_capacity(sizes.len() + 1);
        let mut at = 0u64;
        for &s in sizes {
            starts.push(at);
            at += s;
        }
        starts.push(at);
        Self {
            perm: CompactPerm::new(at, key),
            starts,
        }
    }

    /// Domain size: the sum of the segment sizes.
    #[inline]
    pub fn len(&self) -> u64 {
        *self.starts.last().expect("starts ends with n")
    }

    /// True if every segment is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Number of segments.
    #[inline]
    pub fn segments(&self) -> usize {
        self.starts.len() - 1
    }

    /// Size of segment `k`.
    #[inline]
    pub fn segment_len(&self, k: usize) -> u64 {
        self.starts[k + 1] - self.starts[k]
    }

    /// The segment and offset that item `i` is assigned to. Panics if
    /// `i >= len`.
    #[inline]
    pub fn assign(&self, i: u64) -> (usize, u64) {
        let j = self.perm.fwd(i);
        // The segment holding `j` is the last whose start is at most `j`
        // (empty segments before it share its start and come first).
        let k = self.starts.partition_point(|&s| s <= j) - 1;
        (k, j - self.starts[k])
    }

    /// The item assigned to offset `off` of segment `k`. Panics if `off` is
    /// outside the segment.
    #[inline]
    pub fn member(&self, k: usize, off: u64) -> u64 {
        assert!(off < self.segment_len(k), "offset outside the segment");
        self.perm.inv(self.starts[k] + off)
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn segmented_perm_is_a_bijection_onto_its_segments() {
        let sizes = [5u64, 0, 17, 1, 0, 0, 9, 40];
        let sp = SegmentedPerm::new(&sizes, Key::from_seed(77));
        assert_eq!(sp.len(), 72);
        assert_eq!(sp.segments(), sizes.len());
        let mut filled = vec![vec![false; 0]; sizes.len()];
        for (k, &s) in sizes.iter().enumerate() {
            filled[k] = vec![false; s as usize];
        }
        for i in 0..sp.len() {
            let (k, off) = sp.assign(i);
            assert!(off < sizes[k], "offset inside its segment");
            assert!(!filled[k][off as usize], "each slot once");
            filled[k][off as usize] = true;
            assert_eq!(sp.member(k, off), i, "inverse");
        }
        assert!(filled.iter().flatten().all(|&f| f));
        let empty = SegmentedPerm::new(&[0, 0], Key::from_seed(1));
        assert!(empty.is_empty());
    }

    #[test]
    fn segmented_perm_golden() {
        let sp = SegmentedPerm::new(&[3, 4, 5], Key::from_seed(2026));
        let got: Vec<(usize, u64)> = (0..12).map(|i| sp.assign(i)).collect();
        let want = [
            (2, 4),
            (1, 3),
            (0, 0),
            (1, 1),
            (0, 1),
            (2, 0),
            (2, 3),
            (0, 2),
            (1, 0),
            (2, 1),
            (2, 2),
            (1, 2),
        ];
        assert_eq!(got, want);
    }
    use super::*;

    fn assert_bijection<B: Bijection>(p: &B) {
        let n = p.len();
        let mut seen = vec![false; n as usize];
        for x in 0..n {
            let y = p.fwd(x);
            assert!(y < n);
            assert!(!seen[y as usize], "collision at {y}");
            seen[y as usize] = true;
            assert_eq!(p.inv(y), x);
        }
    }

    #[test]
    fn divmod_by_precomputed_inverse_is_exact() {
        let k = Key::from_seed(31);
        for n in [1u64, 2, 3, 1000, 65_537, 4_000_000_000, 1 << 62] {
            let p = FeistelPerm::new(n, Key::from_seed(n));
            for i in 0..20_000u64 {
                let x = k.with(i).bits() >> 2; // up to 2^62
                assert_eq!(p.divmod_a(x), (x / p.a, x % p.a), "n={n} x={x}");
            }
            for x in [0u64, 1, p.a - 1, p.a, p.a + 1, (1 << 62) - 1] {
                assert_eq!(p.divmod_a(x), (x / p.a, x % p.a));
            }
        }
    }

    #[test]
    fn ceil_sqrt_is_exact() {
        for n in 1..100_000u64 {
            let a = ceil_sqrt(n);
            assert!(a * a >= n && (a - 1) * (a - 1) < n, "n = {n}");
        }
        for n in [(1u64 << 62) - 1, 1 << 62, 4_294_967_291 * 4_294_967_291] {
            let a = ceil_sqrt(n);
            assert!(
                (a as u128) * (a as u128) >= n as u128
                    && ((a - 1) as u128) * ((a - 1) as u128) < n as u128
            );
        }
    }

    #[test]
    fn feistel_is_a_bijection_for_every_n_up_to_2000() {
        for n in 1..=2000u64 {
            assert_bijection(&FeistelPerm::new(n, Key::from_seed(n)));
        }
    }

    #[test]
    fn feistel_all_round_counts_are_bijections() {
        for rounds in MIN_ROUNDS..=12 {
            for n in [1u64, 2, 3, 7, 64, 97, 1000, 1024, 4099] {
                assert_bijection(&FeistelPerm::with_rounds(n, Key::from_seed(9), rounds));
            }
        }
    }

    #[test]
    #[should_panic]
    fn feistel_rejects_three_rounds() {
        let _ = FeistelPerm::with_rounds(10, Key::from_seed(1), 3);
    }

    #[test]
    fn feistel_large_domain_round_trips() {
        let n = 4_294_967_291u64; // a prime near 2^32
        let p = FeistelPerm::new(n, Key::from_seed(77));
        let k = Key::from_seed(78);
        for i in 0..20_000 {
            let x = k.with(i).below(n);
            let y = p.fwd(x);
            assert!(y < n);
            assert_eq!(p.inv(y), x);
        }
    }

    #[test]
    fn feistel_keys_give_different_permutations() {
        let p1 = FeistelPerm::new(1000, Key::from_seed(1));
        let p2 = FeistelPerm::new(1000, Key::from_seed(2));
        let same = (0..1000).filter(|&x| p1.fwd(x) == p2.fwd(x)).count();
        assert!(same < 10, "{same} fixed agreements");
    }

    /// The research note's quality metric: bucket adjacency chi²/df over a
    /// 32×32 grid of (π(x), π(x+1)). Structured permutations score ≫ 1.
    #[test]
    fn feistel_sequential_inputs_look_random() {
        let n = 1_000_003u64;
        let p = FeistelPerm::new(n, Key::from_seed(5));
        let mut grid = [[0u64; 32]; 32];
        for x in 0..n - 1 {
            let a = (p.fwd(x) * 32 / n) as usize;
            let b = (p.fwd(x + 1) * 32 / n) as usize;
            grid[a][b] += 1;
        }
        let expect = (n - 1) as f64 / 1024.0;
        let chi2: f64 = grid
            .iter()
            .flatten()
            .map(|&c| (c as f64 - expect).powi(2) / expect)
            .sum();
        let per_dof = chi2 / 1023.0;
        assert!(per_dof < 1.5, "adjacency chi²/df = {per_dof}");
    }

    /// Golden values pin the exact permutation. Changing the mixer, round
    /// keys or layout would re-roll every world; this makes it deliberate.
    #[test]
    fn feistel_golden_values() {
        let p = FeistelPerm::new(1_000_000, Key::from_seed(2026));
        let got: Vec<u64> = [0u64, 1, 2, 999_999, 123_456]
            .iter()
            .map(|&x| p.fwd(x))
            .collect();
        assert_eq!(got, GOLDEN_FEISTEL, "FeistelPerm output changed: {got:?}");
    }
    const GOLDEN_FEISTEL: [u64; 5] = [933_988, 977_877, 661_497, 837_056, 647_051];

    #[test]
    fn compact_is_32_bytes_and_a_bijection_for_every_n_up_to_2000() {
        assert_eq!(std::mem::size_of::<CompactPerm>(), 32);
        for n in 0..=2000u64 {
            assert_bijection(&CompactPerm::new(n, Key::from_seed(n)));
        }
    }

    #[test]
    fn compact_parts_rebuild_the_same_permutation() {
        assert_eq!(std::mem::size_of::<CompactParts>(), 24);
        for n in [1u64, 2, 7, 1000, 123_457] {
            let p = CompactPerm::new(n, Key::from_seed(n));
            let q = CompactPerm::from_parts(n, p.parts());
            for x in (0..n).step_by((n as usize / 97).max(1)) {
                assert_eq!(p.fwd(x), q.fwd(x));
                assert_eq!(p.inv(x), q.inv(x));
            }
        }
    }

    #[test]
    fn compact_large_domains_round_trip() {
        let k = Key::from_seed(78);
        for n in [4_294_967_291u64, 1 << 40, 1 << 62] {
            let p = CompactPerm::new(n, Key::from_seed(77));
            for i in 0..20_000 {
                let x = k.with(i).below(n);
                let y = p.fwd(x);
                assert!(y < n);
                assert_eq!(p.inv(y), x);
            }
        }
    }

    #[test]
    fn compact_keys_give_different_permutations() {
        let p1 = CompactPerm::new(1000, Key::from_seed(1));
        let p2 = CompactPerm::new(1000, Key::from_seed(2));
        let same = (0..1000).filter(|&x| p1.fwd(x) == p2.fwd(x)).count();
        assert!(same < 10, "{same} fixed agreements");
        // Seeds one bit apart still give unrelated permutations.
        let p3 = CompactPerm::new(1000, Key::from_bits(Key::from_seed(1).bits() ^ 1));
        let same = (0..1000).filter(|&x| p1.fwd(x) == p3.fwd(x)).count();
        assert!(
            same < 10,
            "{same} fixed agreements for a one-bit seed change"
        );
    }

    /// The same quality bar as `feistel_sequential_inputs_look_random`, over
    /// several seeds.
    #[test]
    fn compact_sequential_inputs_look_random() {
        let n = 1_000_003u64;
        for seed in [5u64, 6, 7] {
            let p = CompactPerm::new(n, Key::from_seed(seed));
            let mut grid = [[0u64; 32]; 32];
            for x in 0..n - 1 {
                let a = (p.fwd(x) * 32 / n) as usize;
                let b = (p.fwd(x + 1) * 32 / n) as usize;
                grid[a][b] += 1;
            }
            let expect = (n - 1) as f64 / 1024.0;
            let chi2: f64 = grid
                .iter()
                .flatten()
                .map(|&c| (c as f64 - expect).powi(2) / expect)
                .sum();
            let per_dof = chi2 / 1023.0;
            assert!(per_dof < 1.5, "seed {seed}: adjacency chi²/df = {per_dof}");
        }
    }

    /// Small domains: over many keys, each element lands in each position
    /// about equally often (a Feistel family is not exactly uniform, but must
    /// be close).
    #[test]
    fn compact_small_domains_are_near_uniform() {
        let n = 5u64;
        let mut counts = [[0u32; 5]; 5];
        for s in 0..50_000 {
            let p = CompactPerm::new(n, Key::from_seed(1000 + s));
            for (x, row) in counts.iter_mut().enumerate() {
                row[p.fwd(x as u64) as usize] += 1;
            }
        }
        for row in counts {
            for c in row {
                assert!((9_400..10_600).contains(&c), "{c}");
            }
        }
    }

    #[test]
    fn compact_golden_values() {
        let p = CompactPerm::new(1_000_000, Key::from_seed(2026));
        let got: Vec<u64> = [0u64, 1, 2, 999_999, 123_456]
            .iter()
            .map(|&x| p.fwd(x))
            .collect();
        assert_eq!(got, GOLDEN_COMPACT, "CompactPerm output changed: {got:?}");
    }
    const GOLDEN_COMPACT: [u64; 5] = [849_834, 497_048, 627_643, 688_343, 59_700];

    #[test]
    fn growable_is_a_bijection_on_each_prefix() {
        let g = GrowablePerm::new(500, Key::from_seed(4));
        for n in [1u64, 2, 10, 250, 499, 500] {
            let mut seen = vec![false; n as usize];
            for x in 0..n {
                let y = g.fwd(x, n);
                assert!(y < n && !seen[y as usize]);
                seen[y as usize] = true;
                assert_eq!(g.inv(y, n), x);
            }
        }
    }

    #[test]
    fn growable_growth_changes_exactly_one_image() {
        let g = GrowablePerm::new(10_000, Key::from_seed(12));
        for n in [100u64, 1000, 9_999] {
            let changed = (0..n).filter(|&x| g.fwd(x, n) != g.fwd(x, n + 1)).count();
            assert_eq!(changed, 1, "n = {n}");
        }
    }

    #[test]
    fn small_perm_is_a_bijection_and_uniform() {
        for n in 0..=64usize {
            let p = SmallPerm::new(n, Key::from_seed(n as u64));
            assert_bijection(&p);
        }
        // Uniformity: over many keys, each element lands in each position
        // about equally often.
        let n = 5usize;
        let mut counts = [[0u32; 5]; 5];
        for s in 0..50_000 {
            let p = SmallPerm::new(n, Key::from_seed(1000 + s));
            for (x, row) in counts.iter_mut().enumerate() {
                row[p.fwd(x as u64) as usize] += 1;
            }
        }
        for row in counts {
            for c in row {
                assert!((9_500..10_500).contains(&c), "{c}");
            }
        }
    }

    #[test]
    fn least_cost_assignment_is_the_first_minimum() {
        assert_eq!(least_cost_assignment(0, |_, _| 1), [0, 1, 2]);
        assert_eq!(least_cost_assignment(1, |_, _| 1), [0, 1, 2]);
        // Identity costs nothing: kept.
        assert_eq!(least_cost_assignment(3, |a, b| (a != b) as u32), [0, 1, 2]);
        // Diagonal forbidden: the first derangement in lexicographic order.
        assert_eq!(least_cost_assignment(3, |a, b| (a == b) as u32), [1, 2, 0]);
        assert_eq!(least_cost_assignment(2, |a, b| (a == b) as u32), [1, 0, 2]);
        // Brute force over random costs.
        for seed in 0..200u64 {
            let key = Key::from_seed(seed);
            let c = |a: usize, b: usize| key.with2(a as u64, b as u64).below(3) as u32;
            let got = least_cost_assignment(3, c);
            let total = |p: &[u8]| (0..3).map(|a| c(a, p[a] as usize)).sum::<u32>();
            let min = SMALL_PERMS[3].iter().map(|p| total(p)).min().unwrap();
            assert_eq!(total(&got), min);
            let first = SMALL_PERMS[3].iter().find(|p| total(p) == min).unwrap();
            assert_eq!(&got[..], *first);
        }
    }
}
