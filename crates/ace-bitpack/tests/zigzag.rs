//! Tests of `ace_bitpack` `zigzag` (formerly the inline unit tests of `src`).

use ace_bitpack::*;

/// ZigZag is a bijection over the whole 16-bit domain and maps small magnitudes first.
#[test]
fn zigzag_is_bijective_and_ordered() {
    for value in i16::MIN..=i16::MAX {
        assert_eq!(u16::zigzag(value).unzigzag(), value);
    }
    assert_eq!(
        [
            u32::zigzag(0),
            u32::zigzag(-1),
            u32::zigzag(1),
            u32::zigzag(-2)
        ],
        [0, 1, 2, 3]
    );
    for value in [i64::MIN, -1, 0, 1, i64::MAX] {
        assert_eq!(unzigzag_u64(zigzag_i64(value)), value);
    }
}
