//! AVX2 kernels. Every function is `unsafe` and must only be called after runtime detection.
#![cfg(target_arch = "x86_64")]

use std::arch::x86_64::*;

use crate::scan::{common_prefix_len_scalar, count_zeroes_scalar};

/// Counts zero bytes with 32-byte comparisons.
///
/// # Safety
/// The CPU must support AVX2.
#[target_feature(enable = "avx2")]
pub(crate) unsafe fn count_zeroes(input: &[u8]) -> usize {
    let mut offset = 0usize;
    let mut total = 0usize;
    let zero = _mm256_setzero_si256();
    while offset + 32 <= input.len() {
        // SAFETY: AVX2 is enabled for this function; the 32-byte load starts at `offset` with
        // `offset + 32 <= input.len()`, so it stays inside `input` (unaligned loads are allowed).
        let vector = unsafe { _mm256_loadu_si256(input.as_ptr().add(offset) as *const __m256i) };
        let equal = _mm256_cmpeq_epi8(vector, zero);
        total += (_mm256_movemask_epi8(equal) as u32).count_ones() as usize;
        offset += 32;
    }
    total + count_zeroes_scalar(&input[offset..])
}

/// Returns the first differing position of `left[..limit]` and `right[..limit]`.
///
/// # Safety
/// The CPU must support AVX2 and `limit` must not exceed either slice length.
#[target_feature(enable = "avx2")]
pub(crate) unsafe fn common_prefix_len(left: &[u8], right: &[u8], limit: usize) -> usize {
    let mut offset = 0usize;
    while offset + 32 <= limit {
        // SAFETY: the caller guarantees `limit <= min(left.len(), right.len())`, so both loads of
        // `offset..offset + 32` with `offset + 32 <= limit` stay inside their slices.
        let (a, b) = unsafe {
            (
                _mm256_loadu_si256(left.as_ptr().add(offset) as *const __m256i),
                _mm256_loadu_si256(right.as_ptr().add(offset) as *const __m256i),
            )
        };
        let mask = _mm256_movemask_epi8(_mm256_cmpeq_epi8(a, b)) as u32;
        if mask != u32::MAX {
            return offset + (!mask).trailing_zeros() as usize;
        }
        offset += 32;
    }
    common_prefix_len_scalar(left, right, offset, limit)
}
