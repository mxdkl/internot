//! Session-id sentinel convention.
//!
//! Session-allocated ids must never collide with procedurally-derivable
//! ids. The convention is a high-bit sentinel:
//!
//! ```text
//! u64:  [1 : 1bit][session_id : 15bit][counter : 48bit]
//! u128: [1 : 1bit][session_id : 15bit][counter : 112bit]
//! ```
//!
//! Procedural ids must keep the top bit clear. Application bit layouts
//! must therefore reserve the top bit (`total_width() ≤ W::BITS - 1`).
//! See spec `procedural-overlay.md` §"Session id convention".

// Wired up by session.rs in Step 6; until then these are unreferenced from
// any consuming code, but the test suite exercises them directly.
#![allow(dead_code)]

use procedural_core::word::BitWord;

/// True iff `id` carries the session sentinel bit (top bit of `W`).
pub fn is_session_allocated<W: BitWord>(id: W) -> bool {
    id.extract_bits(W::BITS - 1, 1) == 1
}

/// Width in bits of the session-id field (between sentinel and counter).
pub const SESSION_ID_BITS: u32 = 15;

/// Bit position where the session_id begins (above the counter).
fn session_id_offset<W: BitWord>() -> u32 {
    W::BITS - 1 - SESSION_ID_BITS
}

/// Compose a session-allocated id from `session_id` (15 bits) and
/// `counter` (`W::BITS - 16` bits). The sentinel bit is set automatically.
pub fn compose_session_id<W: BitWord>(session_id: u16, counter: u128) -> W {
    let sid = (session_id as u64) & ((1u64 << SESSION_ID_BITS) - 1);
    let counter_bits = W::BITS - 1 - SESSION_ID_BITS;
    let counter_mask: u128 = if counter_bits >= 128 {
        u128::MAX
    } else {
        (1u128 << counter_bits) - 1
    };
    let counter_masked = counter & counter_mask;
    // Build via insert_bits on a zero word: counter low, session_id high, sentinel highest.
    let mut id = W::zero();
    // Counter occupies bits [0, counter_bits). Each `insert_bits` call is
    // capped at 64 per the BitWord trait, so we split into low and high
    // halves. Since `counter` is u128, the high half is at most 64 bits
    // even when `counter_bits` is much larger (e.g. 240 for U256).
    if counter_bits > 0 {
        if counter_bits <= 64 {
            id = id.insert_bits(0, counter_bits, counter_masked as u64);
        } else {
            let low = counter_masked as u64;
            let high = (counter_masked >> 64) as u64;
            id = id.insert_bits(0, 64, low);
            let high_width = (counter_bits - 64).min(64);
            id = id.insert_bits(64, high_width, high);
        }
    }
    id = id.insert_bits(session_id_offset::<W>(), SESSION_ID_BITS, sid);
    id = id.insert_bits(W::BITS - 1, 1, 1);
    id
}

/// Extract the session_id (15 bits) from a session-allocated id.
/// Returns `None` if the sentinel bit is clear (i.e., procedural id).
pub fn extract_session_id<W: BitWord>(id: W) -> Option<u16> {
    if !is_session_allocated(id) {
        return None;
    }
    let sid = id.extract_bits(session_id_offset::<W>(), SESSION_ID_BITS) as u16;
    Some(sid)
}

/// Mutable counter that emits successive session-allocated ids for a fixed
/// `(session_id, W)`.
#[derive(Debug)]
pub(crate) struct Allocator<W: BitWord> {
    session_id: u16,
    next: u128,
    _word: std::marker::PhantomData<W>,
}

impl<W: BitWord> Allocator<W> {
    pub(crate) fn new(session_id: u16) -> Self {
        Allocator {
            session_id,
            next: 0,
            _word: std::marker::PhantomData,
        }
    }

    /// Returns the next session-allocated id and bumps the counter.
    pub(crate) fn allocate(&mut self) -> W {
        let id = compose_session_id::<W>(self.session_id, self.next);
        self.next = self.next.saturating_add(1);
        id
    }

    /// Bump the counter past `id` so subsequent `allocate()` calls do
    /// not collide with it. Used by `replay` to reconstruct allocator
    /// state from a trajectory.
    pub(crate) fn observe(&mut self, id: W) {
        if let Some(sid) = extract_session_id::<W>(id) {
            if sid != self.session_id {
                return; // foreign session — ignore
            }
            // Counter occupies the low (W::BITS - 1 - SESSION_ID_BITS) bits.
            let counter_bits = W::BITS - 1 - SESSION_ID_BITS;
            let low = if counter_bits.min(64) > 0 {
                id.extract_bits(0, counter_bits.min(64))
            } else {
                0
            };
            let high = if counter_bits > 64 {
                // Counter source is u128, so the high half is at most 64 bits
                // even when counter_bits exceeds 128 (e.g. 240 for U256).
                id.extract_bits(64, (counter_bits - 64).min(64))
            } else {
                0
            };
            let observed = (high as u128) << 64 | (low as u128);
            if observed.saturating_add(1) > self.next {
                self.next = observed.saturating_add(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn procedural_ids_are_not_session_allocated_u64() {
        // Top bit clear — covers everything an application bit layout produces
        // when total_width ≤ 63 (the documented constraint).
        for id in [0u64, 1, 0xDEAD_BEEF, 0x7FFF_FFFF_FFFF_FFFF] {
            assert!(!is_session_allocated(id), "id {id:#x} should not be session-allocated");
        }
    }

    #[test]
    fn sentinel_bit_means_session_allocated_u64() {
        for id in [
            0x8000_0000_0000_0000u64,
            0xFFFF_FFFF_FFFF_FFFF,
            0x8000_0000_0000_0001,
        ] {
            assert!(is_session_allocated(id), "id {id:#x} should be session-allocated");
        }
    }

    #[test]
    fn procedural_ids_are_not_session_allocated_u128() {
        for id in [0u128, 1, 0x0123_4567_89AB_CDEF_FEDC_BA98_7654_3210] {
            assert!(!is_session_allocated(id));
        }
    }

    #[test]
    fn sentinel_bit_means_session_allocated_u128() {
        let id: u128 = 1u128 << 127;
        assert!(is_session_allocated(id));
    }

    #[test]
    fn compose_then_extract_round_trips_session_id_u64() {
        for sid in [0u16, 1, 0x1234, 0x7FFF] {
            let id: u64 = compose_session_id(sid, 42);
            assert!(is_session_allocated(id), "sentinel bit lost");
            assert_eq!(extract_session_id::<u64>(id), Some(sid));
        }
    }

    #[test]
    fn compose_then_extract_round_trips_session_id_u128() {
        for sid in [0u16, 1, 0x7FFF] {
            let id: u128 = compose_session_id(sid, 42);
            assert_eq!(extract_session_id::<u128>(id), Some(sid));
        }
    }

    #[test]
    fn extract_session_id_returns_none_for_procedural() {
        assert_eq!(extract_session_id::<u64>(0x1234_5678), None);
        assert_eq!(extract_session_id::<u128>(0x1234_5678), None);
    }

    #[test]
    fn allocator_emits_successive_distinct_ids_u64() {
        let mut a = Allocator::<u64>::new(7);
        let ids: Vec<u64> = (0..100).map(|_| a.allocate()).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "allocator emitted duplicate ids");
        for id in &ids {
            assert!(is_session_allocated(*id));
            assert_eq!(extract_session_id::<u64>(*id), Some(7));
        }
    }

    #[test]
    fn allocator_emits_successive_distinct_ids_u128() {
        let mut a = Allocator::<u128>::new(123);
        let ids: Vec<u128> = (0..50).map(|_| a.allocate()).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len());
        for id in &ids {
            assert_eq!(extract_session_id::<u128>(*id), Some(123));
        }
    }

    #[test]
    fn allocators_with_distinct_session_ids_dont_collide() {
        let mut a = Allocator::<u64>::new(1);
        let mut b = Allocator::<u64>::new(2);
        let from_a: std::collections::HashSet<u64> = (0..100).map(|_| a.allocate()).collect();
        let from_b: std::collections::HashSet<u64> = (0..100).map(|_| b.allocate()).collect();
        assert!(from_a.is_disjoint(&from_b), "different session ids leaked into each other");
    }

    #[test]
    fn observe_advances_counter_past_seen_id() {
        let sid = 5u16;
        let mut a = Allocator::<u64>::new(sid);
        // Generate a few ids, then create a fresh allocator that observes the last one.
        let ids: Vec<u64> = (0..10).map(|_| a.allocate()).collect();
        let last = ids[9];
        let mut b = Allocator::<u64>::new(sid);
        b.observe(last);
        let next = b.allocate();
        assert!(!ids.contains(&next), "observe failed to advance past observed id");
    }

    #[test]
    fn observe_ignores_foreign_session_ids() {
        let mut a = Allocator::<u64>::new(1);
        let foreign = compose_session_id::<u64>(2, 999);
        a.observe(foreign);
        // Counter unaffected: first allocation is still counter=0.
        let first = a.allocate();
        assert_eq!(first, compose_session_id::<u64>(1, 0));
    }

    #[test]
    fn observe_ignores_procedural_ids() {
        let mut a = Allocator::<u64>::new(1);
        a.observe(0x1234u64); // procedural; no sentinel bit
        let first = a.allocate();
        assert_eq!(first, compose_session_id::<u64>(1, 0));
    }
}
