//! Small safe SIMD dispatch layer for ACE 0.3.
//!
//! Unsafe architecture intrinsics are intentionally isolated in this crate.  All callers use
//! safe functions with scalar fallbacks, which keeps the rest of the compression engine free
//! from target-specific unsafe blocks.

/// SIMD implementation selected at runtime for diagnostic and benchmark reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimdBackend {
    /// Portable scalar implementation.
    Scalar,
    /// x86/x86_64 AVX2 implementation.
    Avx2,
}

/// Returns the SIMD backend selected on this host.
pub fn selected_backend() -> SimdBackend {
    #[cfg(target_arch = "x86_64")]
    {
        if std::arch::is_x86_feature_detected!("avx2") {
            return SimdBackend::Avx2;
        }
    }
    SimdBackend::Scalar
}

/// Counts zero bytes using runtime SIMD dispatch and a scalar fallback.
pub fn count_zeroes(input: &[u8]) -> usize {
    #[cfg(target_arch = "x86_64")]
    {
        if std::arch::is_x86_feature_detected!("avx2") {
            // SAFETY: AVX2 support is checked at runtime, and the function performs only
            // unaligned loads within the provided slice bounds.
            return unsafe { count_zeroes_avx2(input) };
        }
    }
    input.iter().filter(|&&b| b == 0).count()
}

/// Returns the number of equal bytes at the beginning of two slices, bounded by the shorter slice.
pub fn common_prefix_len(left: &[u8], right: &[u8]) -> usize {
    let limit = left.len().min(right.len());
    #[cfg(target_arch = "x86_64")]
    {
        if std::arch::is_x86_feature_detected!("avx2") {
            // SAFETY: AVX2 support is checked and all loads are guarded by `offset + 32 <= limit`.
            return unsafe { common_prefix_len_avx2(left, right, limit) };
        }
    }
    left.iter().zip(right).take_while(|(a, b)| a == b).count()
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
/// Counts zero bytes using AVX2 32-byte comparisons after runtime dispatch has validated support.
unsafe fn count_zeroes_avx2(input: &[u8]) -> usize {
    use std::arch::x86_64::*;
    let zero = _mm256_setzero_si256();
    let mut offset = 0usize;
    let mut total = 0usize;
    while offset + 32 <= input.len() {
        let ptr = input.as_ptr().add(offset) as *const __m256i;
        let v = _mm256_loadu_si256(ptr);
        let eq = _mm256_cmpeq_epi8(v, zero);
        total += (_mm256_movemask_epi8(eq) as u32).count_ones() as usize;
        offset += 32;
    }
    total + input[offset..].iter().filter(|&&b| b == 0).count()
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
/// Compares 32-byte AVX2 chunks and returns the first differing byte position.
unsafe fn common_prefix_len_avx2(left: &[u8], right: &[u8], limit: usize) -> usize {
    use std::arch::x86_64::*;
    let mut offset = 0usize;
    while offset + 32 <= limit {
        let a = _mm256_loadu_si256(left.as_ptr().add(offset) as *const __m256i);
        let b = _mm256_loadu_si256(right.as_ptr().add(offset) as *const __m256i);
        let eq = _mm256_cmpeq_epi8(a, b);
        let mask = _mm256_movemask_epi8(eq) as u32;
        if mask != u32::MAX {
            return offset + (!mask).trailing_zeros() as usize;
        }
        offset += 32;
    }
    offset
        + left[offset..limit]
            .iter()
            .zip(&right[offset..limit])
            .take_while(|(a, b)| a == b)
            .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies the safe dispatch API against simple scalar expectations.
    #[test]
    fn primitives_are_correct() {
        assert_eq!(count_zeroes(&[0, 1, 0, 2, 0]), 3);
        assert_eq!(common_prefix_len(b"abcdef", b"abcxyz"), 3);
    }
}
