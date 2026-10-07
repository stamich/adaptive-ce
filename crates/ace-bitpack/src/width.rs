//! Bit-width helpers used to choose the packed width of a stream.

use crate::Lane;

/// Returns the minimum number of bits required to represent `value` (`0` for zero).
#[inline(always)]
pub fn bits_required_u64(value: u64) -> u8 {
    (u64::BITS - value.leading_zeros()) as u8
}

/// Returns the largest bit width required by any value of the slice (`0` for an empty slice).
pub fn max_bit_width<T: Lane>(values: &[T]) -> u8 {
    // OR-ing all values has the same highest set bit as the maximum, without a compare per item.
    let combined = values.iter().fold(0u64, |acc, value| acc | value.to_u64());
    bits_required_u64(combined)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
