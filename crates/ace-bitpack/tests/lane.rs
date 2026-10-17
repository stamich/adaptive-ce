//! Tests of `ace_bitpack` `lane` (formerly the inline unit tests of `src`).

use ace_bitpack::*;

/// Constants and conversions agree for every lane.
#[test]
fn lane_constants_and_conversions() {
    assert_eq!((u16::BITS, u16::BYTES), (16, 2));
    assert_eq!((<u32 as Lane>::BITS, u32::BYTES), (32, 4));
    assert_eq!((<u64 as Lane>::BITS, u64::BYTES), (64, 8));
    assert_eq!(u16::from_signed(-1i16), u16::MAX);
    assert_eq!(u32::MAX.to_signed(), -1i32);
    assert_eq!(u16::from_u64_truncating(0x1_2345), 0x2345);
    assert_eq!(<u32 as Lane>::signed_to_i64(-5), -5i64);
}

/// Lane I/O is little-endian and round-trips.
#[test]
fn lane_io_roundtrip() {
    let values = vec![1u32, 0xAABB_CCDD, u32::MAX];
    let bytes = write_lanes(&values);
    assert_eq!(&bytes[4..8], &[0xDD, 0xCC, 0xBB, 0xAA]);
    assert_eq!(read_lanes::<u32>(&bytes), values);
    assert_eq!(
        read_lanes::<u64>(&bytes[..11]).len(),
        1,
        "partial lanes are ignored"
    );
}
