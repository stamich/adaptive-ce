//! NEON kernels for aarch64. Every function is `unsafe` and must only be called after runtime
//! detection (NEON is part of the aarch64 baseline, but the dispatch still honours the
//! `ACE_SIMD=scalar` override and keeps one call pattern for both architectures).
//!
//! The kernels are the 16-byte counterparts of the AVX2 ones in `avx2.rs` and return exactly the
//! same values: `tests/scan.rs` compares them with the scalar reference on every length around
//! the vector boundary, and the golden tests run the whole engine with the NEON backend on the
//! native ARM64 CI runner.
#![cfg(target_arch = "aarch64")]

use std::arch::aarch64::*;

use crate::scan::{common_prefix_len_scalar, count_zeroes_scalar};

/// Width of one NEON register in bytes.
const LANE_BYTES: usize = 16;

/// Number of vector additions after which a byte lane of the accumulator could overflow.
const DRAIN_INTERVAL: u32 = 255;

/// Counts zero bytes with 16-byte comparisons.
///
/// A byte that equals zero turns into `0xFF` (that is `-1`), so subtracting the comparison result
/// from a byte-lane accumulator adds one per match. Each lane holds at most 255 matches, so the
/// accumulator is folded into the total every [`DRAIN_INTERVAL`] vectors.
///
/// # Safety
/// The CPU must support NEON.
#[target_feature(enable = "neon")]
pub(crate) unsafe fn count_zeroes(input: &[u8]) -> usize {
    let mut offset = 0usize;
    let mut total = 0usize;
    let zero = vdupq_n_u8(0);
    let mut accumulator = zero;
    let mut pending = 0u32;
    while offset + LANE_BYTES <= input.len() {
        // SAFETY: NEON is enabled for this function; the 16-byte load starts at `offset` with
        // `offset + 16 <= input.len()`, so it stays inside `input` (loads need no alignment).
        let vector = unsafe { vld1q_u8(input.as_ptr().add(offset)) };
        accumulator = vsubq_u8(accumulator, vceqq_u8(vector, zero));
        pending += 1;
        if pending == DRAIN_INTERVAL {
            total += usize::from(vaddlvq_u8(accumulator));
            accumulator = zero;
            pending = 0;
        }
        offset += LANE_BYTES;
    }
    total += usize::from(vaddlvq_u8(accumulator));
    total + count_zeroes_scalar(&input[offset..])
}

/// Returns the first differing position of `left[..limit]` and `right[..limit]`.
///
/// The comparison mask of a 16-byte block is narrowed to one nibble per byte (`0xF` equal, `0`
/// different) and read as a 64-bit integer, so the first difference is the lowest set bit of the
/// inverted mask divided by four.
///
/// # Safety
/// The CPU must support NEON and `limit` must not exceed either slice length.
#[target_feature(enable = "neon")]
pub(crate) unsafe fn common_prefix_len(left: &[u8], right: &[u8], limit: usize) -> usize {
    let mut offset = 0usize;
    while offset + LANE_BYTES <= limit {
        // SAFETY: the caller guarantees `limit <= min(left.len(), right.len())`, so both loads of
        // `offset..offset + 16` with `offset + 16 <= limit` stay inside their slices.
        let (a, b) = unsafe {
            (
                vld1q_u8(left.as_ptr().add(offset)),
                vld1q_u8(right.as_ptr().add(offset)),
            )
        };
        let equal = vceqq_u8(a, b);
        if vminvq_u8(equal) != u8::MAX {
            let nibbles = vshrn_n_u16::<4>(vreinterpretq_u16_u8(equal));
            let mask = vget_lane_u64::<0>(vreinterpret_u64_u8(nibbles));
            return offset + (!mask).trailing_zeros() as usize / 4;
        }
        offset += LANE_BYTES;
    }
    common_prefix_len_scalar(left, right, offset, limit)
}
