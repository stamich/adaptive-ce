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
