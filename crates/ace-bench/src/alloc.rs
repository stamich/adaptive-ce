//! Counting global allocator and process peak-RSS probe used by the memory families.
//!
//! Counting is off by default (one relaxed atomic load per allocation) and is switched on
//! only inside an [`AllocationProbe`], so timing families are not perturbed.
//!
//! This module contains the only `unsafe` code of the benchmark harness: the
//! [`GlobalAlloc`] implementation, which forwards verbatim to [`System`]
//! (audited in `docs/UNSAFE-AUDIT-0.4.6.md`).

use std::alloc::{GlobalAlloc, Layout, System};
use std::fs;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use serde::Serialize;

/// Global allocator that forwards to [`System`] and optionally counts allocations.
pub(crate) struct CountingAllocator;

/// Whether allocations are currently being counted.
static ENABLED: AtomicBool = AtomicBool::new(false);
/// Number of allocation calls (including reallocations) while enabled.
static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
/// Total bytes requested while enabled.
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);
/// Bytes currently live that were allocated while enabled (net of frees while enabled).
static LIVE_BYTES: AtomicU64 = AtomicU64::new(0);
/// High-water mark of [`LIVE_BYTES`] while enabled.
static PEAK_LIVE_BYTES: AtomicU64 = AtomicU64::new(0);

/// The harness-wide allocator instance.
#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

/// Records an allocation of `size` bytes when counting is enabled.
fn record_alloc(size: usize) {
    if ENABLED.load(Ordering::Relaxed) {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(size as u64, Ordering::Relaxed);
        let live = LIVE_BYTES.fetch_add(size as u64, Ordering::Relaxed) + size as u64;
        PEAK_LIVE_BYTES.fetch_max(live, Ordering::Relaxed);
    }
}

/// Records a deallocation of `size` bytes when counting is enabled (saturating at zero, as
/// memory allocated before enabling may be freed while enabled).
fn record_dealloc(size: usize) {
    if ENABLED.load(Ordering::Relaxed) {
        let _ = LIVE_BYTES.try_update(Ordering::Relaxed, Ordering::Relaxed, |live| {
            Some(live.saturating_sub(size as u64))
        });
    }
}

// SAFETY: every method forwards its arguments unchanged to `System`, which upholds the
// `GlobalAlloc` contract; the bookkeeping only touches atomics and never allocates.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_alloc(layout.size());
        // SAFETY: the caller upholds `GlobalAlloc::alloc`'s contract for `layout`.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        record_dealloc(layout.size());
        // SAFETY: `ptr` was returned by `System` (via this allocator) for `layout`.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_alloc(layout.size());
        // SAFETY: the caller upholds `GlobalAlloc::alloc_zeroed`'s contract for `layout`.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record_dealloc(layout.size());
        record_alloc(new_size);
        // SAFETY: `ptr`/`layout` come from this allocator and the caller upholds
        // `GlobalAlloc::realloc`'s contract for `new_size`.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

/// Allocation activity observed during one [`AllocationProbe`].
#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
pub(crate) struct AllocationReport {
    /// Allocation calls (including reallocations).
    pub(crate) allocations: u64,
    /// Total bytes requested.
    pub(crate) allocated_bytes: u64,
    /// Peak of simultaneously live bytes allocated inside the probe.
    pub(crate) peak_live_bytes: u64,
}

/// Scoped allocation counter; not re-entrant (the harness measures one case at a time).
#[derive(Debug)]
pub(crate) struct AllocationProbe {
    /// Prevents construction outside [`AllocationProbe::start`].
    _private: (),
}

/// Inherent methods of [`AllocationProbe`].
impl AllocationProbe {
    /// Resets the counters and starts counting.
    pub(crate) fn start() -> Self {
        ALLOCATIONS.store(0, Ordering::Relaxed);
        ALLOCATED_BYTES.store(0, Ordering::Relaxed);
        LIVE_BYTES.store(0, Ordering::Relaxed);
        PEAK_LIVE_BYTES.store(0, Ordering::Relaxed);
        ENABLED.store(true, Ordering::SeqCst);
        Self { _private: () }
    }

    /// Stops counting and returns what was observed.
    pub(crate) fn finish(self) -> AllocationReport {
        ENABLED.store(false, Ordering::SeqCst);
        AllocationReport {
            allocations: ALLOCATIONS.load(Ordering::Relaxed),
            allocated_bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
            peak_live_bytes: PEAK_LIVE_BYTES.load(Ordering::Relaxed),
        }
    }
}

/// Runs `operation` inside an [`AllocationProbe`] and returns its result and report.
pub(crate) fn count_allocations<T>(operation: impl FnOnce() -> T) -> (T, AllocationReport) {
    let probe = AllocationProbe::start();
    let value = operation();
    (value, probe.finish())
}

/// Process peak resident set size (`VmHWM` of `/proc/self/status`) in bytes.
pub(crate) fn peak_rss_bytes() -> Option<u64> {
    fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|text| {
            text.lines().find_map(|line| {
                line.strip_prefix("VmHWM:")
                    .and_then(|rest| rest.split_whitespace().next())
                    .and_then(|kb| kb.parse::<u64>().ok())
                    .map(|kb| kb * 1024)
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_counts_a_vector_allocation() {
        let (len, report) = count_allocations(|| {
            let buffer = std::hint::black_box(vec![0u8; 4096]);
            buffer.len()
        });
        assert_eq!(len, 4096);
        assert!(report.allocations >= 1);
        assert!(report.allocated_bytes >= 4096);
        assert!(report.peak_live_bytes >= 4096);
    }
}
