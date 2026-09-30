//! ID word abstraction.
//!
//! The framework's Layer 0 primitives are generic over `BitWord` so that
//! spaces can choose an ID width based on how many indexable attributes
//! they need. `u64` is implemented for simple use cases; `u128` for spaces
//! with many indexable bit-fields.

use std::fmt::{Debug, Display};
use std::hash::Hash;

/// Abstraction over fixed-width unsigned integer ID words.
///
/// A `BitWord` can be serialized to a fixed byte array for hashing, and
/// supports reading/writing bit-fields of up to 64 bits at a time (the
/// framework never needs fields wider than 64 bits because field values
/// round-trip through `u64`).
pub trait BitWord: Copy + Clone + Eq + Ord + Hash + Debug + Display + Default + 'static {
    /// Total width of this word in bits.
    const BITS: u32;

    /// Fixed-size byte representation, used for hashing.
    type Bytes: AsRef<[u8]>;

    fn zero() -> Self;
    fn to_le_bytes(self) -> Self::Bytes;

    /// Extract the `width` bits at `offset` as a `u64`.
    ///
    /// # Panics
    ///
    /// Panics if `offset + width > Self::BITS` or `width > 64`.
    fn extract_bits(self, offset: u32, width: u32) -> u64;

    /// Return a new value with the `width` bits at `offset` replaced by
    /// the low `width` bits of `value`. Unmentioned bits are preserved.
    ///
    /// # Panics
    ///
    /// Panics if `offset + width > Self::BITS` or `width > 64`.
    fn insert_bits(self, offset: u32, width: u32, value: u64) -> Self;

    /// Construct a `W` from a raw u64 hash seed. For 64-bit W, `h` is
    /// returned as-is. For wider W, the upper bits are filled by hashing
    /// `h` with a distinct key so the result spans the full W width.
    fn from_hash_u64(h: u64) -> Self;
}

impl BitWord for u64 {
    const BITS: u32 = 64;
    type Bytes = [u8; 8];

    fn zero() -> Self {
        0
    }
    fn to_le_bytes(self) -> [u8; 8] {
        self.to_le_bytes()
    }

    fn extract_bits(self, offset: u32, width: u32) -> u64 {
        assert!(width <= 64, "width > 64");
        assert!(
            offset.checked_add(width).is_some_and(|n| n <= 64),
            "offset + width > BITS"
        );
        if width == 0 {
            return 0;
        }
        let mask: u64 = if width == 64 {
            u64::MAX
        } else {
            (1u64 << width) - 1
        };
        (self >> offset) & mask
    }

    fn insert_bits(self, offset: u32, width: u32, value: u64) -> Self {
        assert!(width <= 64, "width > 64");
        assert!(
            offset.checked_add(width).is_some_and(|n| n <= 64),
            "offset + width > BITS"
        );
        if width == 0 {
            return self;
        }
        let mask: u64 = if width == 64 {
            u64::MAX
        } else {
            (1u64 << width) - 1
        };
        let value_masked = value & mask;
        let cleared = self & !(mask << offset);
        cleared | (value_masked << offset)
    }

    fn from_hash_u64(h: u64) -> Self {
        h
    }
}

impl BitWord for u128 {
    const BITS: u32 = 128;
    type Bytes = [u8; 16];

    fn zero() -> Self {
        0
    }
    fn to_le_bytes(self) -> [u8; 16] {
        self.to_le_bytes()
    }

    fn extract_bits(self, offset: u32, width: u32) -> u64 {
        assert!(width <= 64, "width > 64");
        assert!(
            offset.checked_add(width).is_some_and(|n| n <= 128),
            "offset + width > BITS"
        );
        if width == 0 {
            return 0;
        }
        let mask: u64 = if width == 64 {
            u64::MAX
        } else {
            (1u64 << width) - 1
        };
        ((self >> offset) as u64) & mask
    }

    fn insert_bits(self, offset: u32, width: u32, value: u64) -> Self {
        assert!(width <= 64, "width > 64");
        assert!(
            offset.checked_add(width).is_some_and(|n| n <= 128),
            "offset + width > BITS"
        );
        if width == 0 {
            return self;
        }
        let mask: u64 = if width == 64 {
            u64::MAX
        } else {
            (1u64 << width) - 1
        };
        let value_masked = (value & mask) as u128;
        let field_mask = (mask as u128) << offset;
        (self & !field_mask) | (value_masked << offset)
    }

    fn from_hash_u64(h: u64) -> Self {
        // Widen by hashing `h` with a distinct key to fill the high 64 bits.
        // Uses the same raw_hash primitive from the hash module.
        let hi = crate::hash::raw_hash::<u64>(h, "bitword_from_hash_u64_hi");
        ((hi as u128) << 64) | (h as u128)
    }
}

/// 256-bit unsigned word for very wide bit layouts. Backed by `[u64; 4]`
/// in little-endian order: limb 0 is the lowest 64 bits, limb 3 is the
/// highest. Implements `BitWord` so `Space<U256>` / `BitLayout<U256>` /
/// `find()` work transparently for layouts up to 256 bits wide.
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct U256([u64; 4]);

impl U256 {
    /// Build from raw little-endian limbs.
    pub const fn from_limbs(limbs: [u64; 4]) -> Self {
        Self(limbs)
    }

    /// Inspect raw limbs.
    pub const fn limbs(self) -> [u64; 4] {
        self.0
    }

    /// Reconstruct from a 32-byte little-endian byte array.
    pub fn from_le_bytes(bytes: [u8; 32]) -> Self {
        let mut limbs = [0u64; 4];
        for (i, limb) in limbs.iter_mut().enumerate() {
            let mut buf = [0u8; 8];
            buf.copy_from_slice(&bytes[i * 8..(i + 1) * 8]);
            *limb = u64::from_le_bytes(buf);
        }
        Self(limbs)
    }
}

impl std::fmt::Debug for U256 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Hex, high-to-low for readability.
        write!(
            f,
            "U256(0x{:016x}{:016x}{:016x}{:016x})",
            self.0[3], self.0[2], self.0[1], self.0[0]
        )
    }
}

impl std::fmt::Display for U256 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

impl BitWord for U256 {
    const BITS: u32 = 256;
    type Bytes = [u8; 32];

    fn zero() -> Self {
        Self::default()
    }

    fn to_le_bytes(self) -> [u8; 32] {
        let mut out = [0u8; 32];
        for (i, &limb) in self.0.iter().enumerate() {
            out[i * 8..(i + 1) * 8].copy_from_slice(&limb.to_le_bytes());
        }
        out
    }

    fn extract_bits(self, offset: u32, width: u32) -> u64 {
        assert!(width <= 64, "width > 64");
        assert!(
            offset.checked_add(width).is_some_and(|n| n <= 256),
            "offset + width > BITS"
        );
        if width == 0 {
            return 0;
        }
        let mask: u64 = if width == 64 {
            u64::MAX
        } else {
            (1u64 << width) - 1
        };
        let limb_lo = (offset / 64) as usize;
        let bit_off = offset % 64;
        let lo_part = self.0[limb_lo] >> bit_off;
        let crosses_limb = bit_off + width > 64;
        let raw = if crosses_limb {
            let hi_part = self.0[limb_lo + 1] << (64 - bit_off);
            lo_part | hi_part
        } else {
            lo_part
        };
        raw & mask
    }

    fn insert_bits(self, offset: u32, width: u32, value: u64) -> Self {
        assert!(width <= 64, "width > 64");
        assert!(
            offset.checked_add(width).is_some_and(|n| n <= 256),
            "offset + width > BITS"
        );
        if width == 0 {
            return self;
        }
        let mask: u64 = if width == 64 {
            u64::MAX
        } else {
            (1u64 << width) - 1
        };
        let value_masked = value & mask;
        let limb_lo = (offset / 64) as usize;
        let bit_off = offset % 64;
        let mut limbs = self.0;
        // Clear and write low part
        let lo_mask = mask << bit_off;
        limbs[limb_lo] = (limbs[limb_lo] & !lo_mask) | (value_masked << bit_off);
        if bit_off + width > 64 {
            // Spill into upper limb
            let hi_bits = bit_off + width - 64;
            let hi_mask = (1u64 << hi_bits).wrapping_sub(1);
            limbs[limb_lo + 1] =
                (limbs[limb_lo + 1] & !hi_mask) | (value_masked >> (64 - bit_off));
        }
        Self(limbs)
    }

    fn from_hash_u64(h: u64) -> Self {
        // Lower 64 = hash input directly (mirrors u64/u128 convention so the
        // input is preserved for tests / debugging).
        // Upper three limbs filled by hashing `h` with distinct keys.
        let l1 = crate::hash::raw_hash::<u64>(h, "bitword_u256_l1");
        let l2 = crate::hash::raw_hash::<u64>(h, "bitword_u256_l2");
        let l3 = crate::hash::raw_hash::<u64>(h, "bitword_u256_l3");
        Self([h, l1, l2, l3])
    }
}

/// 512-bit unsigned word for very wide bit layouts. Backed by `[u64; 8]`
/// in little-endian order: limb 0 is the lowest 64 bits, limb 7 is the
/// highest. Implements `BitWord` so `Space<U512>` / `BitLayout<U512>` /
/// `find()` work transparently for layouts up to 512 bits wide.
///
/// Used by the People space (post-2026-05 substrate widening) to carry
/// cached derived attributes across all 4 spec tiers in one id.
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct U512([u64; 8]);

impl U512 {
    /// Build from raw little-endian limbs.
    pub const fn from_limbs(limbs: [u64; 8]) -> Self {
        Self(limbs)
    }

    /// Inspect raw limbs.
    pub const fn limbs(self) -> [u64; 8] {
        self.0
    }

    /// Reconstruct from a 64-byte little-endian byte array.
    pub fn from_le_bytes(bytes: [u8; 64]) -> Self {
        let mut limbs = [0u64; 8];
        for (i, limb) in limbs.iter_mut().enumerate() {
            let mut buf = [0u8; 8];
            buf.copy_from_slice(&bytes[i * 8..(i + 1) * 8]);
            *limb = u64::from_le_bytes(buf);
        }
        Self(limbs)
    }
}

impl std::fmt::Debug for U512 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "U512(0x{:016x}{:016x}{:016x}{:016x}{:016x}{:016x}{:016x}{:016x})",
            self.0[7], self.0[6], self.0[5], self.0[4],
            self.0[3], self.0[2], self.0[1], self.0[0]
        )
    }
}

impl std::fmt::Display for U512 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

impl BitWord for U512 {
    const BITS: u32 = 512;
    type Bytes = [u8; 64];

    fn zero() -> Self {
        Self::default()
    }

    fn to_le_bytes(self) -> [u8; 64] {
        let mut out = [0u8; 64];
        for (i, &limb) in self.0.iter().enumerate() {
            out[i * 8..(i + 1) * 8].copy_from_slice(&limb.to_le_bytes());
        }
        out
    }

    fn extract_bits(self, offset: u32, width: u32) -> u64 {
        assert!(width <= 64, "width > 64");
        assert!(
            offset.checked_add(width).is_some_and(|n| n <= 512),
            "offset + width > BITS"
        );
        if width == 0 {
            return 0;
        }
        let mask: u64 = if width == 64 {
            u64::MAX
        } else {
            (1u64 << width) - 1
        };
        let limb_lo = (offset / 64) as usize;
        let bit_off = offset % 64;
        let lo_part = self.0[limb_lo] >> bit_off;
        let crosses_limb = bit_off + width > 64;
        let raw = if crosses_limb {
            let hi_part = self.0[limb_lo + 1] << (64 - bit_off);
            lo_part | hi_part
        } else {
            lo_part
        };
        raw & mask
    }

    fn insert_bits(self, offset: u32, width: u32, value: u64) -> Self {
        assert!(width <= 64, "width > 64");
        assert!(
            offset.checked_add(width).is_some_and(|n| n <= 512),
            "offset + width > BITS"
        );
        if width == 0 {
            return self;
        }
        let mask: u64 = if width == 64 {
            u64::MAX
        } else {
            (1u64 << width) - 1
        };
        let value_masked = value & mask;
        let limb_lo = (offset / 64) as usize;
        let bit_off = offset % 64;
        let mut limbs = self.0;
        let lo_mask = mask << bit_off;
        limbs[limb_lo] = (limbs[limb_lo] & !lo_mask) | (value_masked << bit_off);
        if bit_off + width > 64 {
            let hi_bits = (bit_off + width) - 64;
            let hi_mask: u64 = if hi_bits == 64 { u64::MAX } else { (1u64 << hi_bits) - 1 };
            let hi_value = value_masked >> (64 - bit_off);
            limbs[limb_lo + 1] = (limbs[limb_lo + 1] & !hi_mask) | (hi_value & hi_mask);
        }
        Self(limbs)
    }

    fn from_hash_u64(h: u64) -> Self {
        let l1 = crate::hash::raw_hash::<u64>(h, "bitword_u512_l1");
        let l2 = crate::hash::raw_hash::<u64>(h, "bitword_u512_l2");
        let l3 = crate::hash::raw_hash::<u64>(h, "bitword_u512_l3");
        let l4 = crate::hash::raw_hash::<u64>(h, "bitword_u512_l4");
        let l5 = crate::hash::raw_hash::<u64>(h, "bitword_u512_l5");
        let l6 = crate::hash::raw_hash::<u64>(h, "bitword_u512_l6");
        let l7 = crate::hash::raw_hash::<u64>(h, "bitword_u512_l7");
        Self([h, l1, l2, l3, l4, l5, l6, l7])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn u64_bits_const() {
        assert_eq!(<u64 as BitWord>::BITS, 64);
    }

    #[test]
    fn u128_bits_const() {
        assert_eq!(<u128 as BitWord>::BITS, 128);
    }

    #[test]
    fn u64_zero() {
        assert_eq!(<u64 as BitWord>::zero(), 0u64);
    }

    #[test]
    fn u128_zero() {
        assert_eq!(<u128 as BitWord>::zero(), 0u128);
    }

    #[test]
    fn u64_le_bytes_round_trip() {
        let id: u64 = 0x0123_4567_89AB_CDEF;
        let bytes = id.to_le_bytes();
        assert_eq!(bytes.as_ref().len(), 8);
        assert_eq!(u64::from_le_bytes(bytes), id);
    }

    #[test]
    fn u128_le_bytes_round_trip() {
        let id: u128 = 0x0123_4567_89AB_CDEF_FEDC_BA98_7654_3210;
        let bytes = <u128 as BitWord>::to_le_bytes(id);
        assert_eq!(bytes.as_ref().len(), 16);
        assert_eq!(u128::from_le_bytes(bytes), id);
    }

    #[test]
    fn u64_extract_insert_round_trip() {
        let layout_id = 0u64
            .insert_bits(0, 4, 0xF)
            .insert_bits(4, 28, 0x0FFF_FFFF)
            .insert_bits(32, 4, 0x7)
            .insert_bits(36, 28, 0x089A_BCDE);
        assert_eq!(layout_id.extract_bits(0, 4), 0xF);
        assert_eq!(layout_id.extract_bits(4, 28), 0x0FFF_FFFF);
        assert_eq!(layout_id.extract_bits(32, 4), 0x7);
        assert_eq!(layout_id.extract_bits(36, 28), 0x089A_BCDE);
    }

    #[test]
    fn u128_extract_insert_round_trip() {
        let id = 0u128
            .insert_bits(0, 8, 0xAB)
            .insert_bits(8, 56, 0x00FF_FFFF_FFFF_FFFF)
            .insert_bits(64, 8, 0xCD)
            .insert_bits(72, 56, 0x00AA_BBCC_DDEE_FF11);
        assert_eq!(id.extract_bits(0, 8), 0xAB);
        assert_eq!(id.extract_bits(8, 56), 0x00FF_FFFF_FFFF_FFFF);
        assert_eq!(id.extract_bits(64, 8), 0xCD);
        assert_eq!(id.extract_bits(72, 56), 0x00AA_BBCC_DDEE_FF11);
    }

    #[test]
    fn insert_preserves_other_bits() {
        // Set bits 0..4, then overwrite bits 4..8 — bits 0..4 must survive.
        let id = 0u64.insert_bits(0, 4, 0xF).insert_bits(4, 4, 0xA);
        assert_eq!(id.extract_bits(0, 4), 0xF);
        assert_eq!(id.extract_bits(4, 4), 0xA);
    }

    #[test]
    fn insert_masks_oversized_value() {
        // Passing 0xFF to a 4-bit field should keep only low 4 bits (0xF).
        let id = 0u64.insert_bits(0, 4, 0xFF);
        assert_eq!(id.extract_bits(0, 4), 0xF);
    }

    #[test]
    fn u128_can_hold_64_bit_field() {
        let id = 0u128.insert_bits(0, 64, u64::MAX);
        assert_eq!(id.extract_bits(0, 64), u64::MAX);
    }

    #[test]
    #[should_panic(expected = "offset + width")]
    fn u64_extract_past_end_panics() {
        0u64.extract_bits(60, 8);
    }

    #[test]
    #[should_panic(expected = "offset + width")]
    fn u128_extract_past_end_panics() {
        0u128.extract_bits(120, 16);
    }

    #[test]
    #[should_panic(expected = "width > 64")]
    fn extract_width_over_64_panics() {
        0u128.extract_bits(0, 65);
    }

    #[test]
    fn u64_zero_width_extract_returns_zero() {
        assert_eq!(0xDEAD_BEEFu64.extract_bits(0, 0), 0);
    }

    #[test]
    fn u64_zero_width_insert_preserves_value() {
        assert_eq!(0xDEAD_BEEFu64.insert_bits(0, 0, 0xFFFF), 0xDEAD_BEEF);
    }

    // -- U256 tests -----------------------------------------------------------

    #[test]
    fn u256_bits_const() {
        assert_eq!(<U256 as BitWord>::BITS, 256);
    }

    #[test]
    fn u256_zero() {
        assert_eq!(<U256 as BitWord>::zero(), U256::default());
    }

    #[test]
    fn u256_le_bytes_round_trip() {
        let id = U256::from_limbs([
            0x0123_4567_89AB_CDEF,
            0xFEDC_BA98_7654_3210,
            0xAAAA_BBBB_CCCC_DDDD,
            0x1111_2222_3333_4444,
        ]);
        let bytes = id.to_le_bytes();
        assert_eq!(bytes.as_ref().len(), 32);
        assert_eq!(U256::from_le_bytes(bytes), id);
    }

    #[test]
    fn u256_extract_insert_round_trip_within_one_limb() {
        let id = U256::default()
            .insert_bits(0, 8, 0xAB)
            .insert_bits(8, 56, 0x00FF_FFFF_FFFF_FFFF);
        assert_eq!(id.extract_bits(0, 8), 0xAB);
        assert_eq!(id.extract_bits(8, 56), 0x00FF_FFFF_FFFF_FFFF);
    }

    #[test]
    fn u256_extract_insert_round_trip_across_limbs() {
        // Field straddles bit 64 (limb boundary).
        let id = U256::default().insert_bits(60, 16, 0x1234);
        assert_eq!(id.extract_bits(60, 16), 0x1234);
    }

    #[test]
    fn u256_extract_insert_round_trip_high_limb() {
        let id = U256::default().insert_bits(192, 64, u64::MAX);
        assert_eq!(id.extract_bits(192, 64), u64::MAX);
    }

    #[test]
    fn u256_can_hold_64_bit_field_at_each_limb() {
        for limb in 0..4u32 {
            let id = U256::default().insert_bits(limb * 64, 64, u64::MAX);
            assert_eq!(id.extract_bits(limb * 64, 64), u64::MAX);
        }
    }

    #[test]
    fn u256_insert_preserves_other_bits() {
        let id = U256::default()
            .insert_bits(0, 4, 0xF)
            .insert_bits(4, 4, 0xA)
            .insert_bits(192, 8, 0xCD);
        assert_eq!(id.extract_bits(0, 4), 0xF);
        assert_eq!(id.extract_bits(4, 4), 0xA);
        assert_eq!(id.extract_bits(192, 8), 0xCD);
    }

    #[test]
    #[should_panic(expected = "offset + width")]
    fn u256_extract_past_end_panics() {
        U256::default().extract_bits(252, 8);
    }

    #[test]
    #[should_panic(expected = "width > 64")]
    fn u256_extract_width_over_64_panics() {
        U256::default().extract_bits(0, 65);
    }

    #[test]
    fn u256_from_hash_u64_spans_full_width() {
        let a = U256::from_hash_u64(1);
        let b = U256::from_hash_u64(2);
        assert_ne!(a, b);
        assert_eq!(a.extract_bits(0, 64), 1);
        assert_eq!(b.extract_bits(0, 64), 2);
        // Upper limbs should be non-zero for any non-trivial hash input.
        assert_ne!(a.limbs()[1], 0);
        assert_ne!(a.limbs()[2], 0);
        assert_ne!(a.limbs()[3], 0);
    }

    // -- U512 tests -----------------------------------------------------------

    #[test]
    fn u512_bits_constant_is_512() {
        assert_eq!(<U512 as BitWord>::BITS, 512);
    }

    #[test]
    fn u512_zero_matches_default() {
        assert_eq!(<U512 as BitWord>::zero(), U512::default());
    }

    #[test]
    fn u512_to_le_bytes_round_trips() {
        let id = U512::from_limbs([
            0x0102_0304_0506_0708,
            0x1112_1314_1516_1718,
            0x2122_2324_2526_2728,
            0x3132_3334_3536_3738,
            0x4142_4344_4546_4748,
            0x5152_5354_5556_5758,
            0x6162_6364_6566_6768,
            0x7172_7374_7576_7778,
        ]);
        let bytes = id.to_le_bytes();
        assert_eq!(bytes.len(), 64);
        assert_eq!(U512::from_le_bytes(bytes), id);
    }

    #[test]
    fn u512_extract_within_a_single_limb() {
        let id = U512::default()
            .insert_bits(0, 16, 0xABCD)
            .insert_bits(64, 16, 0x1234)
            .insert_bits(448, 16, 0x5678);
        assert_eq!(id.extract_bits(0, 16), 0xABCD);
        assert_eq!(id.extract_bits(64, 16), 0x1234);
        assert_eq!(id.extract_bits(448, 16), 0x5678);
    }

    #[test]
    fn u512_extract_crossing_limb_boundary() {
        let id = U512::default().insert_bits(60, 16, 0x1234);
        assert_eq!(id.extract_bits(60, 16), 0x1234);
    }

    #[test]
    fn u512_insert_full_64_bit_limb() {
        let id = U512::default().insert_bits(192, 64, u64::MAX);
        assert_eq!(id.extract_bits(192, 64), u64::MAX);
        assert_eq!(id.extract_bits(0, 64), 0);
        assert_eq!(id.extract_bits(256, 64), 0);
    }

    #[test]
    fn u512_each_limb_addressable() {
        for limb in 0..8u32 {
            let id = U512::default().insert_bits(limb * 64, 64, u64::MAX);
            assert_eq!(id.limbs()[limb as usize], u64::MAX);
            for other in 0..8u32 {
                if other != limb {
                    assert_eq!(id.limbs()[other as usize], 0);
                }
            }
        }
    }

    #[test]
    #[should_panic(expected = "offset + width > BITS")]
    fn u512_extract_panics_past_bits() {
        U512::default().extract_bits(508, 8);
    }

    #[test]
    #[should_panic(expected = "width > 64")]
    fn u512_extract_panics_on_width_65() {
        U512::default().extract_bits(0, 65);
    }

    #[test]
    fn u512_from_hash_u64_seeds_all_limbs() {
        let a = U512::from_hash_u64(1);
        let b = U512::from_hash_u64(2);
        assert_ne!(a, b);
        assert_eq!(a.limbs()[0], 1);
        assert_eq!(b.limbs()[0], 2);
        assert_ne!(a.limbs()[1], 0);
        assert_ne!(a.limbs()[7], 0);
    }
}
