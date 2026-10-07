//! ZigZag mapping between signed deltas and small unsigned integers.
//!
//! The generic form is [`Lane::zigzag`] / [`Lane::unzigzag`]; the 64-bit helpers below are kept
//! because the allocation-free NUM1 estimator works on sign-extended `i64` deltas of every lane.

use crate::Lane;

/// ZigZag-encodes a signed 64-bit value (`0→0, -1→1, 1→2, …`).
#[inline(always)]
pub fn zigzag_i64(value: i64) -> u64 {
    u64::zigzag(value)
}

/// Reverses [`zigzag_i64`].
#[inline(always)]
pub fn unzigzag_u64(value: u64) -> i64 {
    value.unzigzag()
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
