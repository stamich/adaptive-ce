//! Safe byte-scanning primitives with runtime SIMD dispatch.

use crate::backend::avx2_available;
#[cfg(target_arch = "aarch64")]
use crate::backend::neon_available;

/// Counts zero bytes.
pub fn count_zeroes(input: &[u8]) -> usize {
    if avx2_available() {
        #[cfg(target_arch = "x86_64")]
        // SAFETY: AVX2 support was verified at runtime; the kernel only performs unaligned
        // loads inside `input`.
        return unsafe { crate::avx2::count_zeroes(input) };
    }
    #[cfg(target_arch = "aarch64")]
    if neon_available() {
        // SAFETY: NEON support was verified at runtime; the kernel only performs unaligned
        // loads inside `input`.
        return unsafe { crate::neon::count_zeroes(input) };
    }
    count_zeroes_scalar(input)
}

/// Returns the length of the common prefix of two slices (bounded by the shorter one).
pub fn common_prefix_len(left: &[u8], right: &[u8]) -> usize {
    let limit = left.len().min(right.len());
    if avx2_available() {
        #[cfg(target_arch = "x86_64")]
        // SAFETY: AVX2 support was verified; all loads are guarded by `offset + 32 <= limit`.
        return unsafe { crate::avx2::common_prefix_len(left, right, limit) };
    }
    #[cfg(target_arch = "aarch64")]
    if neon_available() {
        // SAFETY: NEON support was verified; all loads are guarded by `offset + 16 <= limit`
        // and `limit` is the shorter of the two slices.
        return unsafe { crate::neon::common_prefix_len(left, right, limit) };
    }
    common_prefix_len_scalar(left, right, 0, limit)
}

/// Scalar zero count (also used for the AVX2 remainder).
pub(crate) fn count_zeroes_scalar(input: &[u8]) -> usize {
    input.iter().filter(|&&byte| byte == 0).count()
}

/// Scalar common-prefix search over `start..limit` (also used for the AVX2 remainder).
pub(crate) fn common_prefix_len_scalar(
    left: &[u8],
    right: &[u8],
    start: usize,
    limit: usize,
) -> usize {
    start
        + left[start..limit]
            .iter()
            .zip(&right[start..limit])
            .take_while(|(a, b)| a == b)
            .count()
}
