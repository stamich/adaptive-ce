//! Gorilla XOR coding of IEEE-754 values (binary64 and binary32), bit-exact.
//!
//! For every value after the first (which the header stores raw):
//!
//! ```text
//! x = bits(v[i]) XOR bits(v[i-1])
//! x == 0                                   -> '0'
//! x != 0, fits the previous window         -> '1' '0' + Mw bits of x >> (W - Lw - Mw)
//! x != 0, otherwise                        -> '1' '1' + L (l_bits) + (M - 1) (m_bits) + M bits of x >> T
//!                                             with L = min(leading_zeros(x), l_max),
//!                                             T = trailing_zeros(x), M = W - L - T; window := (L, M)
//! ```
//!
//! Without [`super::FLAG_WINDOW_REUSE`] the selector bit after `'1'` is omitted and every
//! non-zero XOR opens a new window. Storing `M - 1` (instead of Gorilla's "64 encoded as 0")
//! keeps every field value directly meaningful and validation trivial.

use ace_bitpack::BitReader;
use ace_core::{AceError, AceResult};

use super::sink::BitSink;

/// Shape parameters of one IEEE-754 width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GorillaShape {
    /// Value width in bits (`W`).
    pub(crate) bits: u32,
    /// Lane width in bytes.
    pub(crate) lane_bytes: usize,
    /// Cap on the stored leading-zero count.
    pub(crate) l_max: u32,
    /// Bits used to store the leading-zero count.
    pub(crate) l_bits: u32,
    /// Bits used to store `M - 1`.
    pub(crate) m_bits: u32,
}

/// Binary64 parameters (`L` capped at 31 → 5 bits, `M - 1` in 6 bits).
pub(crate) const GORILLA_F64: GorillaShape = GorillaShape {
    bits: 64,
    lane_bytes: 8,
    l_max: 31,
    l_bits: 5,
    m_bits: 6,
};

/// Binary32 parameters (`L` capped at 15 → 4 bits, `M - 1` in 5 bits).
pub(crate) const GORILLA_F32: GorillaShape = GorillaShape {
    bits: 32,
    lane_bytes: 4,
    l_max: 15,
    l_bits: 4,
    m_bits: 5,
};

/// Window `(leading zeros, meaningful bits)` of the last explicitly stored XOR.
type Window = (u32, u32);

/// Encodes the XORs of `values[1..]` against their predecessors into `sink`.
///
/// `values` yields the raw IEEE bits zero-extended to 64 bits. The first value is not encoded
/// (it lives in the header). With `reuse == false` no window is ever reused.
pub(crate) fn encode_xors<S: BitSink>(
    shape: GorillaShape,
    values: impl Iterator<Item = u64>,
    reuse: bool,
    sink: &mut S,
) {
    let mut previous: Option<u64> = None;
    let mut window: Option<Window> = None;
    for value in values {
        let Some(last) = previous.replace(value) else {
            continue;
        };
        let xor = value ^ last;
        if xor == 0 {
            sink.put(0, 1);
            continue;
        }
        sink.put(1, 1);
        // Leading zeros relative to the value width, not to u64.
        let leading = (xor.leading_zeros() - (64 - shape.bits)).min(shape.l_max);
        let trailing = xor.trailing_zeros();
        if reuse {
            if let Some((window_leading, window_meaningful)) = window {
                let window_trailing = shape.bits - window_leading - window_meaningful;
                if leading >= window_leading && trailing >= window_trailing {
                    sink.put(0, 1);
                    sink.put(xor >> window_trailing, window_meaningful);
                    continue;
                }
            }
            sink.put(1, 1);
        }
        let meaningful = shape.bits - leading - trailing;
        sink.put(leading as u64, shape.l_bits);
        sink.put((meaningful - 1) as u64, shape.m_bits);
        sink.put(xor >> trailing, meaningful);
        window = Some((leading, meaningful));
    }
}

/// Decodes `count` values (the first is `first_bits`) and passes each raw bit pattern to
/// `emit` in order. Fails on any structural error or when stream bits remain unused.
pub(crate) fn decode_values(
    shape: GorillaShape,
    first_bits: u64,
    count: usize,
    reuse: bool,
    reader: &mut BitReader<'_>,
    mut emit: impl FnMut(u64),
) -> AceResult<()> {
    let truncated = || AceError::InvalidTimeSeries("truncated Gorilla stream");
    if count == 0 {
        return Ok(());
    }
    let mut value = first_bits;
    emit(value);
    let mut window: Option<Window> = None;
    for _ in 1..count {
        if !reader.read_bit().ok_or_else(truncated)? {
            emit(value);
            continue;
        }
        let new_window = !reuse || reader.read_bit().ok_or_else(truncated)?;
        let (leading, meaningful) = if new_window {
            let leading = reader.read_bits(shape.l_bits).ok_or_else(truncated)? as u32;
            let meaningful = reader.read_bits(shape.m_bits).ok_or_else(truncated)? as u32 + 1;
            if leading > shape.l_max || leading + meaningful > shape.bits {
                return Err(AceError::InvalidTimeSeries("invalid Gorilla bit window"));
            }
            window = Some((leading, meaningful));
            (leading, meaningful)
        } else {
            window.ok_or(AceError::InvalidTimeSeries(
                "Gorilla window reused before definition",
            ))?
        };
        let shift = shape.bits - leading - meaningful;
        // `meaningful >= 1` keeps `shift < bits <= 64`, so the shift cannot overflow.
        let xor = reader.read_bits(meaningful).ok_or_else(truncated)? << shift;
        value ^= xor;
        emit(value);
    }
    if reader.remaining() != 0 {
        return Err(AceError::InvalidTimeSeries(
            "trailing bits after Gorilla stream",
        ));
    }
    Ok(())
}
