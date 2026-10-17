//! Tests of `ace_core` `numeric` (formerly the inline unit tests of `src`).

use ace_core::*;

/// Width helpers must agree with each other for every supported lane.
#[test]
fn width_helpers_are_consistent() {
    for (width, bytes) in [
        (NumericWidth::U16, 2usize),
        (NumericWidth::U32, 4),
        (NumericWidth::U64, 8),
    ] {
        assert_eq!(width.bytes(), bytes);
        assert_eq!(width.bits() as usize, bytes * 8);
        assert_eq!(NumericWidth::from_byte_width(bytes as u8), Some(width));
        assert_eq!(width.small_delta_bits() as usize, bytes * 4);
    }
}

/// Lane-ring arithmetic must wrap at the lane boundary instead of at 64 bits.
#[test]
fn lane_delta_wraps_in_the_lane_ring() {
    assert_eq!(NumericWidth::U16.lane_delta(65_535, 0), 1);
    assert_eq!(NumericWidth::U16.lane_delta(0, 65_535), -1);
    assert_eq!(NumericWidth::U32.lane_delta(u32::MAX as u64, 2), 3);
    assert_eq!(NumericWidth::U64.lane_delta(u64::MAX, 4), 5);
    assert_eq!(NumericWidth::U16.mask(), 0xFFFF);
    assert_eq!(NumericWidth::U32.mask(), 0xFFFF_FFFF);
    assert_eq!(NumericWidth::U64.mask(), u64::MAX);
    assert_eq!(NumericWidth::U16.sign_extend(0x8000), -32_768);
}

/// The const-generic helpers must agree with the run-time `NumericWidth` methods.
#[test]
fn const_helpers_match_runtime_helpers() {
    let bytes = [0x34u8, 0x12, 0xFF, 0xEE, 0xDD, 0xCC, 0xBB, 0xAA];
    assert_eq!(read_lane::<2>(&bytes), 0x1234);
    assert_eq!(read_lane::<4>(&bytes), 0xEEFF_1234);
    assert_eq!(read_lane::<8>(&bytes), 0xAABB_CCDD_EEFF_1234);
    for (a, b) in [(0u64, 1u64), (65_535, 0), (0, 65_535), (123, 99_999)] {
        assert_eq!(
            lane_delta_const::<2>(a, b),
            NumericWidth::U16.lane_delta(a, b)
        );
        assert_eq!(
            lane_delta_const::<4>(a, b),
            NumericWidth::U32.lane_delta(a, b)
        );
        assert_eq!(
            lane_delta_const::<8>(a, b),
            NumericWidth::U64.lane_delta(a, b)
        );
    }
}

/// Unsupported serialized widths must be rejected rather than guessed.
#[test]
fn unsupported_byte_widths_are_rejected() {
    for bad in [0u8, 1, 3, 5, 6, 7, 9, 16, 255] {
        assert_eq!(NumericWidth::from_byte_width(bad), None);
    }
}
