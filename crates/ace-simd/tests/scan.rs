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
