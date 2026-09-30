//! Heap-allocation counting, for "zero allocations on hot paths" gates.
//!
//! Install [`CountingAllocator`] as the global allocator of a binary or
//! test crate:
//!
//! ```ignore
//! #[global_allocator]
//! static ALLOCATOR: internot_perf::alloc::CountingAllocator =
//!     internot_perf::alloc::CountingAllocator;
//! ```
//!
//! then wrap the code under test in [`count_allocs`]. Counts are kept per
//! thread, so allocations made by other threads (the test runner, a
//! background logger) never leak into a measurement. The flip side:
//! allocations made by threads the closure spawns are not counted either.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;

use serde::{Deserialize, Serialize};

/// Heap allocations made on one thread.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllocCount {
    /// Calls to `alloc`, `alloc_zeroed` and `realloc`. A `realloc` counts
    /// because it may move the block: it is a heap allocation event.
    pub allocations: u64,
    /// Bytes requested by those calls (the new size, for `realloc`).
    pub bytes: u64,
}

impl AllocCount {
    fn since(self, earlier: AllocCount) -> AllocCount {
        AllocCount {
            allocations: self.allocations - earlier.allocations,
            bytes: self.bytes - earlier.bytes,
        }
    }
}

thread_local! {
    // `const` initialisation and a `Copy` payload: no lazy registration and
    // no destructor, so the allocator can touch it without recursing.
    static COUNTS: Cell<AllocCount> = const {
        Cell::new(AllocCount { allocations: 0, bytes: 0 })
    };
}

fn record(bytes: usize) {
    // `try_with` fails only during thread teardown; such allocations go
    // uncounted rather than aborting the process.
    let _ = COUNTS.try_with(|counts| {
        let c = counts.get();
        counts.set(AllocCount {
            allocations: c.allocations + 1,
            bytes: c.bytes + bytes as u64,
        });
    });
}

/// Allocations made on the current thread since it started (zero if
/// [`CountingAllocator`] is not the global allocator).
pub fn thread_alloc_count() -> AllocCount {
    COUNTS.try_with(Cell::get).unwrap_or_default()
}

/// Runs `f` and returns its result together with the heap allocations it
/// made on the current thread. Scopes nest.
pub fn count_allocs<R>(f: impl FnOnce() -> R) -> (R, AllocCount) {
    let before = thread_alloc_count();
    let result = f();
    (result, thread_alloc_count().since(before))
}

/// Whether [`CountingAllocator`] is this process's global allocator. When
/// it is not, every count reads zero, so harnesses check this before
/// trusting a zero.
pub fn counting_installed() -> bool {
    let (_, count) = count_allocs(|| black_box(Box::new(black_box(1u8))));
    count.allocations > 0
}

/// A global allocator that forwards to [`System`] and counts allocations
/// per thread. See the [module docs](self) for how to install it.
pub struct CountingAllocator;

// SAFETY: every method forwards to `System` with the caller's arguments
// unchanged; the only addition is a thread-local counter update, which does
// not allocate.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        System.alloc(layout)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        System.alloc_zeroed(layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record(new_size);
        System.realloc(ptr, layout, new_size)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}

#[cfg(test)]
mod tests {
    // The crate's unit-test binary installs `CountingAllocator` (see lib.rs).
    use super::*;

    #[test]
    fn the_test_binary_counts_allocations() {
        assert!(counting_installed());
    }

    #[test]
    fn counts_a_known_allocation() {
        let (_, count) = count_allocs(|| black_box(vec![0u8; 100]));
        assert_eq!(
            count,
            AllocCount {
                allocations: 1,
                bytes: 100
            }
        );
    }

    #[test]
    fn a_non_allocating_closure_counts_zero() {
        let (sum, count) = count_allocs(|| (0..black_box(1_000u64)).sum::<u64>());
        assert_eq!(sum, 499_500);
        assert_eq!(count, AllocCount::default());
    }

    #[test]
    fn realloc_counts_as_an_allocation() {
        let mut v: Vec<u64> = Vec::with_capacity(1);
        v.push(1);
        let (_, count) = count_allocs(|| {
            v.reserve_exact(15);
            black_box(&v);
        });
        assert_eq!(
            count,
            AllocCount {
                allocations: 1,
                bytes: 16 * 8
            }
        );
    }

    #[test]
    fn scopes_nest() {
        let ((_, inner), outer) = count_allocs(|| {
            let first = black_box(Box::new(1u32));
            let inner = count_allocs(|| black_box(Box::new(2u32)));
            drop(first);
            inner
        });
        assert_eq!(inner.allocations, 1);
        assert_eq!(outer.allocations, 2);
    }
}
