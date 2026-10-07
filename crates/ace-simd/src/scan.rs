//! Safe byte-scanning primitives with runtime SIMD dispatch.

use crate::backend::avx2_available;

/// Counts zero bytes.
pub fn count_zeroes(input: &[u8]) -> usize {
    if avx2_available() {
        #[cfg(target_arch = "x86_64")]
        // SAFETY: AVX2 support was verified at runtime; the kernel only performs unaligned
        // loads inside `input`.
        return unsafe { crate::avx2::count_zeroes(input) };
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Dispatching and scalar paths agree on lengths around the 32-byte vector boundary.
    #[test]
    fn primitives_match_scalar_reference() {
        assert_eq!(count_zeroes(&[0, 1, 0, 2, 0]), 3);
        assert_eq!(common_prefix_len(b"abcdef", b"abcxyz"), 3);
        for len in [0usize, 1, 31, 32, 33, 64, 100] {
            let a: Vec<u8> = (0..len).map(|i| (i % 3) as u8).collect();
            assert_eq!(count_zeroes(&a), count_zeroes_scalar(&a));
            for split in 0..len {
                let mut b = a.clone();
                b[split] ^= 0xFF;
                assert_eq!(common_prefix_len(&a, &b), split);
            }
            assert_eq!(common_prefix_len(&a, &a), len);
        }
    }
}
