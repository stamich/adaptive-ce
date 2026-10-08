//! Elias-gamma coding of non-negative 64-bit integers (as `gamma(v + 1)`).
//!
//! `x = v + 1` (computed in 128 bits, so `v = u64::MAX` is representable) with
//! `N = floor(log2(x))` is stored as `N` zero bits, one `1` bit and the low `N` bits of `x`:
//! `2N + 1` bits in total. Small values are cheap (`0` costs one bit) and no global bit width
//! is needed, which is what makes RunDelta robust against rare large jumps.

use ace_bitpack::BitReader;

use super::sink::BitSink;

/// Largest `N`: `x <= 2^64` needs at most 64 bits after the leading one.
const MAX_EXPONENT: u32 = 64;

/// Writes `gamma(value + 1)`.
#[inline]
pub(crate) fn put_gamma<S: BitSink>(sink: &mut S, value: u64) {
    let x = value as u128 + 1;
    let exponent = 127 - x.leading_zeros();
    sink.put(0, exponent);
    sink.put(1, 1);
    // Low `exponent` bits of `x`; for exponent 64 this is `x - 2^64 == 0`.
    sink.put(x as u64, exponent);
}

/// Reads one `gamma(value + 1)` and returns `value`; `None` on truncation or overflow.
#[inline]
pub(crate) fn read_gamma(reader: &mut BitReader<'_>) -> Option<u64> {
    let mut exponent = 0u32;
    while !reader.read_bit()? {
        exponent += 1;
        if exponent > MAX_EXPONENT {
            return None;
        }
    }
    let low = reader.read_bits(exponent)?;
    if exponent == MAX_EXPONENT {
        // x = 2^64 + low; only low == 0 maps back into u64 (value = u64::MAX).
        return (low == 0).then_some(u64::MAX);
    }
    Some(((1u64 << exponent) | low) - 1)
}
