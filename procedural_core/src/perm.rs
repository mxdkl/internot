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

/// `x mod n` for `n ≥ 1`, given `n_inv = ⌊(2⁶⁴ − 1)/n⌋`: a multiply
/// and one correction ([`divmod`]) instead of a divide instruction. Store
/// `n_inv` where `n` repeats.
#[inline(always)]
pub fn rem_by_inverse(x: u64, n: u64, n_inv: u64) -> u64 {
    divmod(x, n, n_inv).1
}

/// `(x / n, x mod n)` likewise: compute `n_inv` early (off a dependency
/// chain) and the division itself becomes two multiplies.
#[inline(always)]
pub fn divmod_by_inverse(x: u64, n: u64, n_inv: u64) -> (u64, u64) {
    divmod(x, n, n_inv)
}

/// Round function `F(r)` for round key `k`, reduced to `[0, modulus)`.
#[inline(always)]
fn round(k: u64, r: u64, modulus: u64) -> u64 {
    ((mix64(r ^ k) as u128 * modulus as u128) >> 64) as u64
}

/// [`round`] for `r < 2³⁰`, given `kk = k ^ (k >> 30)`: then
/// `(r ^ k) >> 30 = k >> 30`, so mix64's first xorshift is the key's alone
/// and is folded into `kk`. The same value, a shift and an xor shorter.
#[inline(always)]
fn round_small(kk: u64, r: u64, modulus: u64) -> u64 {
    debug_assert!(r < 1 << 30);
    let mut z = (r ^ kk).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    (((z ^ (z >> 31)) as u128 * modulus as u128) >> 64) as u64
}

/// One pass of the generalized Feistel network over `[0, a·b)` with
/// `rounds` rounds and round keys `key(j)`. Each round adds two values below
/// the modulus, so the reduction is a conditional subtraction rather than a
/// 64-bit division.
#[inline(always)]
fn encrypt(m: u64, a: u64, b: u64, a_inv: u64, rounds: usize, key: impl Fn(usize) -> u64) -> u64 {
    let (mut r, mut l) = divmod(m, a, a_inv);
    // Halves below 2³⁰ (n ≤ 2⁶⁰): the shorter round, same values.
    let small = a <= 1 << 30 && b <= 1 << 30;
    for j in 0..rounds {
        let modulus = if j % 2 == 0 { a } else { b };
        let k = key(j);
        let f = if small { round_small(k ^ (k >> 30), r, modulus) } else { round(k, r, modulus) };
        let mut next = l + f;
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
    let small = a <= 1 << 30 && b <= 1 << 30;
    for j in (0..rounds).rev() {
        let modulus = if j % 2 == 0 { a } else { b };
        let prev_r = l;
        let k = key(j);
        let f = if small { round_small(k ^ (k >> 30), l, modulus) } else { round(k, l, modulus) };
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
pub type CompactPerm = CompactPermR<6>;

/// The [`CompactPerm`] network with 4 rounds: the fewest the Luby–Rackoff
/// construction needs for a strong pseudorandom permutation
/// ([`MIN_ROUNDS`]), at two thirds of the cost. For a bijection on a hot
/// path whose job is only to decorrelate two orders. A different
/// permutation from [`CompactPerm`] for the same key.
pub type CompactPerm4 = CompactPermR<4>;

/// The compact Feistel network with `R` rounds (`4 ≤ R ≤ 6`): see
/// [`CompactPerm`] and [`CompactPerm4`]. (Three rounds leave a serial
/// correlation of about −0.02 between the images of consecutive inputs.)
#[derive(Clone, Copy, Debug)]
pub struct CompactPermR<const R: usize> {
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

impl<const R: usize> CompactPermR<R> {
    const ROUNDS_OK: () = assert!(R >= MIN_ROUNDS as usize && R <= ROUND_TWEAKS.len());

    /// A permutation of `[0, n)` keyed by `key`.
    pub fn new(n: u64, key: Key) -> Self {
        let () = Self::ROUNDS_OK;
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
        encrypt(m, self.a as u64, self.b as u64, self.a_inv, R, |j| {
            seed ^ ROUND_TWEAKS[j]
        })
    }

    #[inline(always)]
    fn decrypt(&self, y: u64) -> u64 {
        let seed = self.seed;
        decrypt(y, self.a as u64, self.b as u64, self.a_inv, R, |j| {
            seed ^ ROUND_TWEAKS[j]
        })
    }
}

impl<const R: usize> Bijection for CompactPermR<R> {
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
            assert_bijection(&CompactPerm4::new(n, Key::from_seed(n)));
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

    /// Four rounds: near-uniform from a few hundred elements on (on a
    /// handful, four rounds over a 3 × 2 grid are visibly biased, ~9%), and
    /// no linear trace of the input order in the output (correlation of x
    /// and π(x), and of π(x) and π(x + 1), within a few standard errors of 0).
    #[test]
    fn compact4_is_uniform_and_decorrelates() {
        uniform_and_decorrelated(|n, k| CompactPerm4::new(n, k));
    }


    fn uniform_and_decorrelated<P: Bijection>(make: impl Fn(u64, Key) -> P) {
        const N: usize = 256;
        let mut counts = vec![[0u32; N]; N];
        for s in 0..40_000 {
            let p = make(N as u64, Key::from_seed(1000 + s));
            for (x, row) in counts.iter_mut().enumerate() {
                row[p.fwd(x as u64) as usize] += 1;
            }
        }
        // Each count: mean 156.25, sd 12.5; every one of 65,536 within 5.5 sd.
        for row in &counts {
            for &c in row {
                assert!((87..226).contains(&c), "{c}");
            }
        }
        for seed in 0..8 {
            let n = 1_000_003u64;
            let p = make(n, Key::from_seed(seed));
            let m = (n as f64 - 1.0) / 2.0;
            let (mut xy, mut yy, mut xx) = (0.0f64, 0.0f64, 0.0f64);
            for x in 0..n - 1 {
                let (a, b) = (p.fwd(x) as f64 - m, p.fwd(x + 1) as f64 - m);
                xy += (x as f64 - m) * a;
                yy += a * b;
                xx += a * a;
            }
            // Standard error of a correlation over n pairs: 1/√n ≈ 0.001.
            assert!((xy / xx).abs() < 0.005, "{seed}: order kept {}", xy / xx);
            assert!((yy / xx).abs() < 0.005, "{seed}: serial {}", yy / xx);
        }
    }


    #[test]
    fn rem_by_inverse_is_exact() {
        for n in [1u64, 2, 3, 7, 1000, 999_983, 1 << 31, (1 << 40) + 13, u64::MAX / 3, u64::MAX] {
            let inv = u64::MAX / n;
            for x in [0u64, 1, n - 1, n, n.wrapping_add(1), 12_345_678_901, u64::MAX - 1, u64::MAX] {
                assert_eq!(rem_by_inverse(x, n, inv), x % n, "{x} mod {n}");
                assert_eq!(divmod_by_inverse(x, n, inv), (x / n, x % n), "{x} / {n}");
            }
        }
    }

    #[test]
    fn compact4_golden_values() {
        let p = CompactPerm4::new(1_000_000, Key::from_seed(2026));
        let got: Vec<u64> = [0u64, 1, 2, 999_999, 123_456].iter().map(|&x| p.fwd(x)).collect();
        assert_eq!(got, [886_829, 880_353, 25_121, 216_212, 495_995], "CompactPerm4 output changed: {got:?}");
        let q = CompactPerm4::from_parts(1_000_000, p.parts());
        assert!((0..1000).all(|x| q.inv(q.fwd(x)) == x && q.fwd(x) == p.fwd(x)));
    }

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

/// A bijection of `0..n` that spreads items within neighbourhoods of about
/// `block`: a keyed [`CompactPerm`] inside each block of `block` items,
/// then inside blocks staggered by half a block (`key(level, block start)`
/// keys each). Forward applies level 0 then 1; inverse undoes them.
pub fn staggered_blocks(n: u64, block: u64, k: u64, inverse: bool, key: impl Fn(u64, u64) -> Key) -> u64 {
    let block = block.max(1);
    let level = |k: u64, lvl: u64| -> u64 {
        let off = if lvl == 0 { 0 } else { (block / 2).min(n) };
        let (start, len) = if k < off {
            (0, off)
        } else {
            let s = off + (k - off) / block * block;
            (s, block.min(n - s))
        };
        let perm = CompactPerm::new(len, key(lvl, start));
        start + if inverse { perm.inv(k - start) } else { perm.fwd(k - start) }
    };
    if inverse { level(level(k, 1), 0) } else { level(level(k, 0), 1) }
}


/// A Feistel domain shape `a × b` with `a`'s division inverse: what
/// [`CompactPerm::new`] derives from the size (a square root and a
/// division). Precompute it for a size used often.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PermShape {
    a_inv: u64,
    a: u32,
    b: u32,
}

impl PermShape {
    /// The shape of `[0, n)`.
    pub fn of(n: u64) -> Self {
        let (a, b, a_inv) = shape(n);
        PermShape { a_inv, a: a as u32, b: b as u32 }
    }
}

impl CompactPerm {
    /// [`CompactPerm::new`] with the shape of `n` given (`PermShape::of(n)`):
    /// the same permutation.
    #[inline]
    pub fn shaped(n: u64, s: PermShape, key: Key) -> Self {
        Self { n, a_inv: s.a_inv, seed: key.bits(), a: s.a, b: s.b }
    }
}

/// [`staggered_blocks`] over a fixed `n` and block, with the full blocks'
/// shape precomputed: the same bijection, without a shape per call.
#[derive(Clone, Copy, Debug)]
pub struct StaggeredBlocks {
    n: u64,
    block: u64,
    full: PermShape,
}

impl StaggeredBlocks {
    pub fn new(n: u64, block: u64) -> Self {
        let block = block.max(1);
        StaggeredBlocks { n, block, full: PermShape::of(block) }
    }

    /// As `staggered_blocks(n, block, k, inverse, key)`.
    #[inline]
    pub fn map(&self, k: u64, inverse: bool, key: impl Fn(u64, u64) -> Key) -> u64 {
        let (n, block) = (self.n, self.block);
        let level = |k: u64, lvl: u64| -> u64 {
            let off = if lvl == 0 { 0 } else { (block / 2).min(n) };
            let (start, len) = if k < off {
                (0, off)
            } else {
                let s = off + (k - off) / block * block;
                (s, block.min(n - s))
            };
            let perm = if len == block { CompactPerm::shaped(len, self.full, key(lvl, start)) } else { CompactPerm::new(len, key(lvl, start)) };
            start + if inverse { perm.inv(k - start) } else { perm.fwd(k - start) }
        };
        if inverse { level(level(k, 1), 0) } else { level(level(k, 0), 1) }
    }
}

#[cfg(test)]
mod staggered_tests {
    use super::*;

    #[test]
    fn staggered_blocks_are_bijections_with_inverse() {
        for (n, block) in [(1u64, 1u64), (7, 3), (100, 10), (1000, 33), (257, 256), (50, 80), (15, 54), (3, 1000)] {
            let key = |l: u64, s: u64| Key::from_seed(5).with2(l, s);
            let mut seen = vec![false; n as usize];
            for k in 0..n {
                let f = staggered_blocks(n, block, k, false, key);
                assert!(f < n && !seen[f as usize], "{n} {block}");
                seen[f as usize] = true;
                assert_eq!(staggered_blocks(n, block, f, true, key), k);
                // Items move at most about one and a half blocks.
                assert!(f.abs_diff(k) < block + block / 2 + 1, "{n} {block}: {k} -> {f}");
            }
        }
    }

    #[test]
    fn precomputed_shapes_give_the_same_bijection() {
        for (n, block) in [(1u64, 1u64), (7, 3), (100, 10), (1000, 33), (257, 256), (50, 80), (15, 54), (3, 1000), (5000, 77)] {
            let key = |l: u64, s: u64| Key::from_seed(5).with2(l, s);
            let sb = StaggeredBlocks::new(n, block);
            for k in 0..n {
                assert_eq!(sb.map(k, false, key), staggered_blocks(n, block, k, false, key));
                assert_eq!(sb.map(k, true, key), staggered_blocks(n, block, k, true, key));
            }
        }
        for n in [1u64, 2, 3, 10, 99, 1 << 20, (1 << 33) + 7] {
            let k = Key::from_seed(n);
            let (a, b) = (CompactPerm::new(n, k), CompactPerm::shaped(n, PermShape::of(n), k));
            for x in [0, n / 3, n - 1] {
                assert_eq!(a.fwd(x), b.fwd(x));
                assert_eq!(a.inv(x), b.inv(x));
            }
        }
    }

    #[test]
    fn golden() {
        let key = |l: u64, s: u64| Key::from_seed(9).with2(l, s);
        let v: Vec<u64> = (0..10).map(|k| staggered_blocks(10, 4, k, false, key)).collect();
        assert_eq!(v, GOLDEN);
    }
    const GOLDEN: [u64; 10] = [0, 2, 1, 3, 5, 4, 7, 8, 6, 9];
}

/// A lattice permutation of `[0, n)`: `r ↦ (a·r + b) mod n`, with `a`
/// coprime to `n` and near `n/φ` (the golden ratio), so the points
/// `(r, π(r))` form a rank-1 lattice whose 2-D discrepancy is
/// `O(log n / n)`. Unlike a Feistel network, its graph is a lattice, so the
/// number of `r` in an interval whose image lies in an interval is two
/// floor sums ([`AffinePerm::count`], O(log n)). A world can use one inside
/// a class to decorrelate two orders and still count their joint prefixes
/// (`thinking/claude/005`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AffinePerm {
    n: u64,
    a: u64,
    b: u64,
    a_inv: u64,
}

impl AffinePerm {
    /// The permutation of `[0, n)` with the golden multiplier for `n` and an
    /// offset from `key`.
    pub fn new(n: u64, key: Key) -> Self {
        let (a, a_inv) = golden_pair(n);
        let b = if n == 0 { 0 } else { key.below(n) };
        Self { n, a, b, a_inv }
    }

    /// The permutation with stored parts: multiplier `a`, its inverse
    /// `a_inv` mod `n` and offset `b` (as [`Self::from_pair`] would derive
    /// them): no Euclid and no key hash.
    #[inline]
    pub fn from_raw(n: u64, a: u64, a_inv: u64, b: u64) -> Self {
        Self { n, a, b, a_inv }
    }

    /// [`AffinePerm::new`] from its stored [`golden_pair`]: no Euclid.
    #[inline]
    pub fn from_pair(n: u64, pair: (u64, u64), key: Key) -> Self {
        let b = if n == 0 { 0 } else { key.below(n) };
        Self { n, a: pair.0, b, a_inv: pair.1 }
    }

    /// The permutation `r ↦ (a·r + b) mod n` for a given `a` coprime to
    /// `n` (`b < n`).
    pub fn with_parts(n: u64, a: u64, b: u64) -> Self {
        assert!(n > 0 && b < n && gcd(a % n, n) == 1, "a coprime multiplier and an offset below n");
        Self { n, a: a % n, b, a_inv: mod_inverse(a % n, n) }
    }

    /// `inv(v)` for `v` in `[lo, hi)`, in order: one addition and a
    /// conditional subtraction each (`inv(v + 1) = inv(v) + a⁻¹ mod n`).
    #[inline]
    pub fn inv_range(&self, lo: u64, hi: u64) -> impl Iterator<Item = u64> + '_ {
        let mut r = if lo < hi { self.inv(lo) } else { 0 };
        let (n, step) = (self.n, self.a_inv);
        (lo..hi).map(move |_| {
            let out = r;
            r += step;
            if r >= n {
                r -= n;
            }
            out
        })
    }

    /// `a⁻¹ mod n`.
    pub fn inverse_multiplier(&self) -> u64 {
        self.a_inv
    }

    /// The multiplier `a` and offset `b`.
    pub fn parts(&self) -> (u64, u64) {
        (self.a, self.b)
    }

    /// Of the `r` in `[lo, hi)`, how many map below `d`.
    pub fn count_below(&self, lo: u64, hi: u64, d: u64) -> u64 {
        if hi <= lo || d == 0 {
            return 0;
        }
        let (n, len) = (self.n, hi - lo);
        if d >= n {
            return len;
        }
        // [v mod n < d] = 1 − (⌊(v + n − d)/n⌋ − ⌊v/n⌋), with v = a·r + b;
        // shifting b by multiples of n leaves the difference unchanged.
        let b = ((self.a as u128 * lo as u128 + self.b as u128) % n as u128) as u64;
        let over = crate::lattice::floor_sum(len, n, self.a, b + n - d) - crate::lattice::floor_sum(len, n, self.a, b);
        len - over as u64
    }

    /// Of the `r` in `[lo, hi)`, how many map into `[c, d)`.
    pub fn count(&self, lo: u64, hi: u64, c: u64, d: u64) -> u64 {
        if d <= c {
            return 0;
        }
        self.count_below(lo, hi, d) - self.count_below(lo, hi, c)
    }

    /// The `j`-th (from 0) `r` at or after `lo` whose image lies in
    /// `[c, d)`, if there is one below `n`.
    pub fn select(&self, lo: u64, c: u64, d: u64, j: u64) -> Option<u64> {
        if self.count(lo, self.n, c, d) <= j {
            return None;
        }
        // The least `hi` with `count(lo, hi) > j`, by bisection.
        let (mut a, mut b) = (lo, self.n);
        while a < b {
            let mid = a + (b - a) / 2;
            if self.count(lo, mid + 1, c, d) > j {
                b = mid;
            } else {
                a = mid + 1;
            }
        }
        Some(a)
    }
}

impl Bijection for AffinePerm {
    fn len(&self) -> u64 {
        self.n
    }

    #[inline]
    fn fwd(&self, r: u64) -> u64 {
        assert!(r < self.n, "{r} outside [0, {})", self.n);
        // a·r + b in 64 bits when it fits (one division), else 128.
        match self.a.checked_mul(r).and_then(|p| p.checked_add(self.b)) {
            Some(p) => p % self.n,
            None => ((self.a as u128 * r as u128 + self.b as u128) % self.n as u128) as u64,
        }
    }

    #[inline]
    fn inv(&self, v: u64) -> u64 {
        assert!(v < self.n, "{v} outside [0, {})", self.n);
        let d = if v >= self.b { v - self.b } else { v + self.n - self.b };
        match self.a_inv.checked_mul(d) {
            Some(p) => p % self.n,
            None => ((self.a_inv as u128 * d as u128) % self.n as u128) as u64,
        }
    }
}

/// [`golden_multiplier`] and its inverse mod `n`, from one extended Euclid
/// per candidate. Store it to build [`AffinePerm::from_pair`] without one.
pub fn golden_pair(n: u64) -> (u64, u64) {
    if n <= 2 {
        return (1, if n <= 1 { 0 } else { 1 });
    }
    let g = 0.618_033_988_749_894_9_f64;
    let c = ((n as f64 * g).round() as u64).clamp(1, n - 1);
    for d in 0..n {
        for a in [c.saturating_sub(d), c + d] {
            if a >= 1 && a < n {
                if let Some(inv) = inverse_mod(a, n) {
                    return (a, inv);
                }
            }
        }
    }
    (1, 1)
}

/// `a⁻¹ mod n`, or `None` when `a` and `n` share a factor: extended Euclid
/// with a subtraction for quotient 1.
fn inverse_mod(a: u64, n: u64) -> Option<u64> {
    // |t| stays below n, so 64-bit signed arithmetic is exact for n < 2⁶³.
    let (mut r0, mut r1) = (n as i64, a as i64);
    let (mut t0, mut t1) = (0i64, 1i64);
    while r1 != 0 {
        let q = if r0 - r1 < r1 { 1 } else { r0 / r1 };
        (r0, r1) = (r1, r0 - q * r1);
        (t0, t1) = (t1, t0 - q * t1);
    }
    (r0 == 1).then(|| t0.rem_euclid(n as i64) as u64)
}

/// The multiplier of [`AffinePerm`] for `n`: the integer nearest
/// `n·(φ − 1)` that is coprime to `n`, searching outwards (1 for `n ≤ 2`).
pub fn golden_multiplier(n: u64) -> u64 {
    if n <= 2 {
        return 1;
    }
    let g = 0.618_033_988_749_894_9_f64;
    let c = ((n as f64 * g).round() as u64).clamp(1, n - 1);
    for d in 0..n {
        for a in [c.saturating_sub(d), c + d] {
            if a >= 1 && a < n && gcd(a, n) == 1 {
                return a;
            }
        }
    }
    1
}

/// Euclid with a subtraction for quotient 1 (the common case near `n/φ`,
/// which is Euclid's worst case for steps, not for quotients).
fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let r = if a >= b && a - b < b { a - b } else { a % b };
        (a, b) = (b, r);
    }
    a
}

/// `a⁻¹ mod n` for `a` coprime to `n` (0 when `n ≤ 1`).
fn mod_inverse(a: u64, n: u64) -> u64 {
    if n <= 1 {
        return 0;
    }
    let (mut r0, mut r1) = (n as i64, a as i64);
    let (mut t0, mut t1) = (0i128, 1i128);
    while r1 != 0 {
        let q = if r0 - r1 < r1 { 1 } else { r0 / r1 };
        (r0, r1) = (r1, r0 - q * r1);
        (t0, t1) = (t1, t0 - q as i128 * t1);
    }
    debug_assert_eq!(r0, 1, "a coprime multiplier");
    t0.rem_euclid(n as i128) as u64
}

#[cfg(test)]
mod affine_tests {
    use super::*;

    #[test]
    fn affine_is_a_bijection_with_its_inverse() {
        for n in [1u64, 2, 3, 7, 10, 64, 97, 1000, 4096] {
            let p = AffinePerm::new(n, Key::from_seed(n));
            let mut seen = vec![false; n as usize];
            for r in 0..n {
                let v = p.fwd(r);
                assert!(!seen[v as usize]);
                seen[v as usize] = true;
                assert_eq!(p.inv(v), r);
            }
        }
    }

    #[test]
    fn counts_and_selects_match_brute_force() {
        for n in [1u64, 5, 13, 100, 257, 1000] {
            let p = AffinePerm::new(n, Key::from_seed(7 * n + 1));
            let step = (n / 9).max(1);
            for lo in (0..=n).step_by(step as usize) {
                for hi in (lo..=n).step_by(step as usize) {
                    for c in (0..=n).step_by(step as usize) {
                        for d in (c..=n).step_by(step as usize) {
                            let brute = (lo..hi).filter(|&r| (c..d).contains(&p.fwd(r))).count() as u64;
                            assert_eq!(p.count(lo, hi, c, d), brute, "n {n} [{lo},{hi}) → [{c},{d})");
                        }
                    }
                }
            }
            for (c, d) in [(0, n), (n / 3, n), (0, n / 2), (n / 4, 3 * n / 4)] {
                let hits: Vec<u64> = (0..n).filter(|&r| (c..d).contains(&p.fwd(r))).collect();
                for (j, &r) in hits.iter().enumerate() {
                    assert_eq!(p.select(0, c, d, j as u64), Some(r));
                }
                assert_eq!(p.select(0, c, d, hits.len() as u64), None);
            }
        }
    }

    #[test]
    fn from_pair_equals_new() {
        for n in [1u64, 2, 3, 97, 1000, 1 << 33] {
            let k = Key::from_seed(n);
            assert_eq!(AffinePerm::from_pair(n, golden_pair(n), k), AffinePerm::new(n, k));
        }
    }

    #[test]
    fn inv_range_steps_exactly() {
        for n in [1u64, 2, 7, 100, 1000, 65_537] {
            let p = AffinePerm::new(n, Key::from_seed(n + 3));
            for (lo, hi) in [(0, n), (n / 3, n), (n / 2, n / 2)] {
                let v: Vec<u64> = p.inv_range(lo, hi).collect();
                let w: Vec<u64> = (lo..hi).map(|x| p.inv(x)).collect();
                assert_eq!(v, w);
            }
        }
    }

    #[test]
    fn counts_match_hand_vectors() {
        // Tursi's vectors (`thinking/tursi/002`): `#{r < R : (a r + b) mod n
        // ≥ thr}` against brute force, over `R` past `n` too.
        for (a, b, n, thr, r, at_least) in [(3u64, 1u64, 17u64, 5u64, 20u64, 13u64), (7, 2, 31, 10, 40, 27), (5, 0, 16, 8, 16, 8)] {
            let p = AffinePerm::with_parts(n, a, b);
            let brute = (0..r).filter(|&x| (a * x + b) % n >= thr).count() as u64;
            assert_eq!(brute, at_least);
            assert_eq!(r - p.count_below(0, r, thr), at_least);
            assert_eq!(p.count_below(0, r, thr), r - at_least);
        }
    }

    #[test]
    fn large_domains_count_exactly() {
        // Against brute force over a window of a large domain.
        let n = (1u64 << 40) + 15;
        let p = AffinePerm::new(n, Key::from_seed(3));
        let (lo, hi) = (n / 3, n / 3 + 20_000);
        let (c, d) = (n / 5, n / 5 + n / 7);
        let brute = (lo..hi).filter(|&r| (c..d).contains(&p.fwd(r))).count() as u64;
        assert_eq!(p.count(lo, hi, c, d), brute);
    }

    #[test]
    fn golden() {
        let p = AffinePerm::new(1_000_003, Key::from_seed(42));
        assert_eq!(p.parts(), (golden_multiplier(1_000_003), p.parts().1));
        assert_eq!(golden_multiplier(1_000_003), 618_036);
        let v: Vec<u64> = (0..4).map(|r| p.fwd(r)).collect();
        assert_eq!(v, vec![p.parts().1, (p.parts().1 + 618_036) % 1_000_003, (p.parts().1 + 2 * 618_036) % 1_000_003, (p.parts().1 + 3 * 618_036) % 1_000_003]);
    }
}
