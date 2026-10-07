//! Fixed-width bit packing of unsigned lanes in little-endian bit order.

use ace_core::{AceError, AceResult};

use crate::bit_io::{read_bits_validated, write_bits};
use crate::Lane;

/// Packs `values` at a fixed `bit_width` (little-endian bit order).
///
/// Fails when `bit_width` exceeds the lane or a value does not fit. A zero bit width is legal
/// only for an all-zero slice and yields an empty vector.
pub fn pack<T: Lane>(values: &[T], bit_width: u8) -> AceResult<Vec<u8>> {
    if bit_width > T::BITS {
        return Err(AceError::Malformed(T::WIDTH_TOO_LARGE));
    }
    let mut out = vec![0u8; packed_len(values.len(), bit_width)?];
    if bit_width == 0 {
        return if values.iter().all(|&value| value == T::default()) {
            Ok(out)
        } else {
            Err(AceError::Malformed(T::NONZERO_AT_ZERO_WIDTH))
        };
    }
    let mask = low_bits_mask(bit_width);
    for (index, value) in values.iter().enumerate() {
        let value = value.to_u64();
        if value & !mask != 0 {
            return Err(AceError::Malformed(T::VALUE_TOO_WIDE));
        }
        write_bits(&mut out, index * bit_width as usize, value, bit_width);
    }
    Ok(out)
}

/// Unpacks `count` values of `bit_width` bits.
///
/// The input length must equal `ceil(count * bit_width / 8)` exactly, so corrupted metadata
/// can neither read out of bounds nor be silently truncated.
pub fn unpack<T: Lane>(input: &[u8], count: usize, bit_width: u8) -> AceResult<Vec<T>> {
    Ok(unpack_iter(input, count, bit_width)?.collect())
}

/// Validates like [`unpack`] and then yields the values lazily (no intermediate vector).
pub fn unpack_iter<T: Lane>(
    input: &[u8],
    count: usize,
    bit_width: u8,
) -> AceResult<impl Iterator<Item = T> + '_> {
    if bit_width > T::BITS {
        return Err(AceError::Malformed(T::WIDTH_TOO_LARGE));
    }
    if input.len() != packed_len(count, bit_width)? {
        return Err(AceError::Malformed("bitpack payload length mismatch"));
    }
    let width = bit_width as usize;
    Ok((0..count).map(move |index| {
        if width == 0 {
            T::default()
        } else {
            T::from_u64_truncating(read_bits_validated(input, index * width, bit_width))
        }
    }))
}

/// Exact byte length of `count` packed values of `bit_width` bits (checked for overflow).
pub fn packed_len(count: usize, bit_width: u8) -> AceResult<usize> {
    count
        .checked_mul(bit_width as usize)
        .map(|bits| bits.div_ceil(8))
        .ok_or(AceError::Malformed("bitpack size overflow"))
}

/// Mask with the low `bit_width` bits set (`bit_width` in `1..=64`).
fn low_bits_mask(bit_width: u8) -> u64 {
    u64::MAX >> (64 - bit_width as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
