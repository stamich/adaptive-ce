//! "Limit ⇒ error before allocation" (ACE 0.5.1 resource-limit contract).
//!
//! A forged size in a file header, block header or trailer must be rejected *before* a buffer
//! of that size is allocated. This test binary installs a counting global allocator (the same
//! forwarding design as the harness allocator, `docs/UNSAFE-AUDIT.md`) and asserts that the
//! peak live allocation while decoding a forged file stays far below the forged size.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::Cursor;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use ace_core::{AceConfig, AceError, BlockSizePolicy, DecodeLimits};
use ace_engine::{AceEngine, AceIndexedDecoder};
use ace_format::{checksum, FILE_HEADER_SIZE};

/// Forwarding allocator that tracks live and peak bytes.
struct PeakAllocator;

/// Bytes currently live.
static LIVE: AtomicU64 = AtomicU64::new(0);
/// High-water mark of [`LIVE`] since the last [`reset_peak`].
static PEAK: AtomicU64 = AtomicU64::new(0);
/// Serializes the measurements (the test harness runs tests on several threads).
static MEASURE: Mutex<()> = Mutex::new(());

#[global_allocator]
static GLOBAL: PeakAllocator = PeakAllocator;

/// Records `size` newly allocated bytes.
fn grow(size: usize) {
    let live = LIVE.fetch_add(size as u64, Ordering::Relaxed) + size as u64;
    PEAK.fetch_max(live, Ordering::Relaxed);
}

/// Records `size` freed bytes.
fn shrink(size: usize) {
    LIVE.fetch_sub(size as u64, Ordering::Relaxed);
}

// SAFETY: every method forwards its arguments unchanged to `System`, which upholds the
// `GlobalAlloc` contract; the bookkeeping only touches atomics and never allocates.
unsafe impl GlobalAlloc for PeakAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        grow(layout.size());
        // SAFETY: the caller upholds `GlobalAlloc::alloc`'s contract for `layout`.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        grow(layout.size());
        // SAFETY: the caller upholds `GlobalAlloc::alloc_zeroed`'s contract for `layout`.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        shrink(layout.size());
        // SAFETY: `ptr` was returned by `System` (via this allocator) for `layout`.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        shrink(layout.size());
        grow(new_size);
        // SAFETY: `ptr`/`layout` come from this allocator and the caller upholds
        // `GlobalAlloc::realloc`'s contract for `new_size`.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

/// Peak live bytes above the starting level while `run` executes.
fn peak_during(run: impl FnOnce()) -> u64 {
    let _guard = MEASURE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let start = LIVE.load(Ordering::Relaxed);
    PEAK.store(start, Ordering::Relaxed);
    run();
    PEAK.load(Ordering::Relaxed).saturating_sub(start)
}

/// Upper bound on what decoding a forged 4-block fixture may allocate (the real data is
/// 16 KiB; the forged sizes are ≥ 4 GiB).
const BUDGET: u64 = 1 << 20;

/// A valid indexed file: four 4 KiB blocks of text.
fn fixture() -> Vec<u8> {
    let input: Vec<u8> = b"allocation fixture "
        .iter()
        .copied()
        .cycle()
        .take(4 * 4096)
        .collect();
    let config = AceConfig {
        block_size: 4096,
        block_size_policy: BlockSizePolicy::Fixed,
        ..AceConfig::default()
    };
    AceEngine::new(config).unwrap().compress(&input).unwrap()
}

/// `file` with the first block header's `u32` field at `offset` set to `value` (header CRC
/// recomputed, so only the size itself is wrong).
fn forge_block_field(file: &[u8], offset: usize, value: u32) -> Vec<u8> {
    let mut forged = file.to_vec();
    let header = FILE_HEADER_SIZE;
    forged[header + offset..header + offset + 4].copy_from_slice(&value.to_le_bytes());
    let crc = checksum(&forged[header..header + 28]);
    forged[header + 28..header + 32].copy_from_slice(&crc.to_le_bytes());
    forged
}

/// `file` with the file header's original size set to `value` (CRC recomputed).
fn forge_original_size(file: &[u8], value: u64) -> Vec<u8> {
    let mut forged = file.to_vec();
    forged[12..20].copy_from_slice(&value.to_le_bytes());
    let crc = checksum(&forged[..28]);
    forged[28..32].copy_from_slice(&crc.to_le_bytes());
    forged
}

/// Decodes `file` through the in-memory, streamed and indexed readers and asserts that each
/// one fails within [`BUDGET`].
fn assert_rejected_within_budget(file: &[u8], what: &str) {
    let engine = AceEngine::default_engine();
    let mut outcomes = Vec::new();
    let in_memory = peak_during(|| outcomes.push(engine.decompress(file).map(drop)));
    let streamed = peak_during(|| {
        outcomes.push(engine.decompress_from(Cursor::new(file), std::io::sink()));
    });
    let indexed = peak_during(|| {
        outcomes.push(
            AceIndexedDecoder::open(Cursor::new(file), DecodeLimits::default())
                .and_then(|mut decoder| decoder.decode_block(0).map(drop)),
        );
    });
    for outcome in &outcomes {
        assert!(outcome.is_err(), "{what}: forged file decoded");
    }
    for (reader, peak) in [
        ("in-memory", in_memory),
        ("streamed", streamed),
        ("indexed", indexed),
    ] {
        assert!(
            peak < BUDGET,
            "{what}: {reader} reader allocated {peak} bytes"
        );
    }
}

/// The unforged fixture decodes, and its decode stays within the budget too.
#[test]
fn fixture_decodes_within_budget() {
    let file = fixture();
    let peak = peak_during(|| {
        AceEngine::default_engine().decompress(&file).unwrap();
    });
    assert!(peak < BUDGET, "fixture decode allocated {peak} bytes");
}

/// Block original size (offset 8), encoded size (12) and metadata size (16) at `u32::MAX`.
#[test]
fn forged_block_sizes_fail_before_allocation() {
    let file = fixture();
    for (offset, what) in [
        (8, "original_size"),
        (12, "encoded_size"),
        (16, "metadata_size"),
    ] {
        assert_rejected_within_budget(&forge_block_field(&file, offset, u32::MAX), what);
    }
}

/// A file header declaring 2^40 output bytes reserves at most the preallocation cap, and a
/// limit below the declared size rejects the file before any block is decoded.
#[test]
fn forged_original_size_fails_before_allocation() {
    let file = forge_original_size(&fixture(), 1 << 40);
    assert_rejected_within_budget(&file, "file original_size");
    let limits = DecodeLimits {
        max_output_size: 1024,
        ..DecodeLimits::default()
    };
    let engine = AceEngine::default_engine().with_decode_limits(limits);
    let file = fixture();
    let mut outcome = Ok(());
    let peak = peak_during(|| outcome = engine.decompress(&file).map(drop));
    assert!(
        matches!(outcome, Err(AceError::ResourceLimitExceeded(_))),
        "{outcome:?}"
    );
    assert!(peak < BUDGET, "limited decode allocated {peak} bytes");
}
