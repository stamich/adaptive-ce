//! Tests of `ace_bitpack` `width` (formerly the inline unit tests of `src`).

use ace_bitpack::*;

/// Bit widths of edge values.
#[test]
fn bit_widths() {
    assert_eq!(bits_required_u64(0), 0);
    assert_eq!(bits_required_u64(1), 1);
    assert_eq!(bits_required_u64(u64::MAX), 64);
    assert_eq!(max_bit_width::<u16>(&[0, 3, 200]), 8);
    assert_eq!(max_bit_width::<u32>(&[]), 0);
    assert_eq!(max_bit_width(&[1u64 << 40, 5]), 41);
}
