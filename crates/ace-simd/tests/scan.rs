//! Tests of `ace_simd` `scan` (formerly the inline unit tests of `src`).

use ace_simd::*;

/// Dispatching and scalar paths agree on lengths around the 32-byte vector boundary.
#[test]
fn primitives_match_scalar_reference() {
    assert_eq!(count_zeroes(&[0, 1, 0, 2, 0]), 3);
    assert_eq!(common_prefix_len(b"abcdef", b"abcxyz"), 3);
    for len in [0usize, 1, 31, 32, 33, 64, 100] {
        let a: Vec<u8> = (0..len).map(|i| (i % 3) as u8).collect();
        assert_eq!(
            count_zeroes(&a),
            a.iter().filter(|&&byte| byte == 0).count()
        );
        for split in 0..len {
            let mut b = a.clone();
            b[split] ^= 0xFF;
            assert_eq!(common_prefix_len(&a, &b), split);
        }
        assert_eq!(common_prefix_len(&a, &a), len);
    }
}

/// Long inputs: the NEON zero counter folds its byte-lane accumulator every 255 vectors (4 080
/// bytes), so inputs of several drains, of all zeroes and of alternating bytes must still count
/// exactly. Passes on every backend; on aarch64 it exercises the drain.
#[test]
fn zero_count_is_exact_across_accumulator_drains() {
    for len in [4_079usize, 4_080, 4_081, 8_160, 10_000, 65_537] {
        let zeroes = vec![0u8; len];
        assert_eq!(count_zeroes(&zeroes), len);
        let alternating: Vec<u8> = (0..len).map(|i| (i % 2) as u8).collect();
        assert_eq!(count_zeroes(&alternating), len.div_ceil(2));
        assert_eq!(count_zeroes(&vec![0xFFu8; len]), 0);
    }
}

/// The first difference is found at every offset of a long buffer, including positions inside the
/// first and the last vector, and equal buffers report their full length.
#[test]
fn common_prefix_finds_the_first_difference_in_long_buffers() {
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let base: Vec<u8> = (0..1_000)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state as u8
        })
        .collect();
    for split in (0..base.len()).step_by(7).chain([15, 16, 17, 31, 32, 33, 999]) {
        let mut other = base.clone();
        other[split] = other[split].wrapping_add(1);
        // A later difference must not hide the first one.
        if split + 3 < other.len() {
            other[split + 3] ^= 0x55;
        }
        assert_eq!(common_prefix_len(&base, &other), split, "split {split}");
        // The shorter slice bounds the result.
        assert_eq!(common_prefix_len(&base[..split], &other), split);
    }
    assert_eq!(common_prefix_len(&base, &base), base.len());
}

/// The reported backend is consistent with the CPU: NEON on aarch64, AVX2 or scalar on x86_64,
/// and scalar whenever `ACE_SIMD=scalar` forces the portable kernels.
#[test]
fn backend_matches_the_architecture() {
    let backend = selected_backend();
    if scalar_forced() {
        assert_eq!(backend, SimdBackend::Scalar);
    } else if cfg!(target_arch = "aarch64") {
        assert_eq!(backend, SimdBackend::Neon);
    } else {
        assert!(matches!(backend, SimdBackend::Avx2 | SimdBackend::Scalar));
    }
}
