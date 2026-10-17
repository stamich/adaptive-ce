//! Tests of `ace_bitpack` `scalar` (formerly the inline unit tests of `src`).
#![allow(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)] // test fixtures

use ace_bitpack::*;

/// Mask with the low `bit_width` bits set (`bit_width` in `1..=64`).
fn low_bits_mask(bit_width: u8) -> u64 {
    u64::MAX >> (64 - u32::from(bit_width))
}

/// Largest value representable in `width` bits.
fn max_for(width: u8) -> u64 {
    if width == 0 {
        0
    } else {
        low_bits_mask(width)
    }
}

/// Packing round-trips at edge widths for every lane.
#[test]
fn pack_roundtrip_all_lanes() {
    for width in [0u8, 1, 4, 7, 15, 16] {
        let m = max_for(width) as u16;
        let values = vec![0, m / 3, m / 2, m];
        assert_eq!(
            unpack::<u16>(&pack(&values, width).unwrap(), 4, width).unwrap(),
            values
        );
    }
    for width in [0u8, 1, 4, 7, 16, 31, 32] {
        let m = max_for(width) as u32;
        let values = vec![0, m / 3, m / 2, m];
        assert_eq!(
            unpack::<u32>(&pack(&values, width).unwrap(), 4, width).unwrap(),
            values
        );
    }
    for width in [0u8, 1, 9, 21, 33, 63, 64] {
        let m = max_for(width);
        let values = vec![0, m / 3, m / 2, m, m];
        assert_eq!(
            unpack::<u64>(&pack(&values, width).unwrap(), 5, width).unwrap(),
            values
        );
    }
}

/// Widths beyond the lane, values beyond the width and truncation are errors, not panics.
#[test]
fn invalid_inputs_are_rejected() {
    assert!(pack::<u16>(&[1], 17).is_err());
    assert!(pack::<u32>(&[8], 3).is_err());
    assert!(pack::<u32>(&[1], 0).is_err());
    let packed = pack::<u32>(&[1, 2, 3, 4], 7).unwrap();
    assert!(unpack::<u32>(&packed[..packed.len() - 1], 4, 7).is_err());
    assert!(unpack::<u32>(&packed, 5, 7).is_err());
    assert!(unpack::<u32>(&packed, 4, 33).is_err());
}

/// The u16 packer produces exactly the bytes the pre-buildfix2 u32 packer produced for
/// widened u16 values (NUM1 u16 payloads must stay byte-identical).
#[test]
fn u16_pack_matches_widened_u32_pack() {
    let values: Vec<u16> = (0..1000u32).map(|i| (i * 37 % 1021) as u16).collect();
    let widened: Vec<u32> = values.iter().map(|&v| v as u32).collect();
    assert_eq!(pack(&values, 10).unwrap(), pack(&widened, 10).unwrap());
}
