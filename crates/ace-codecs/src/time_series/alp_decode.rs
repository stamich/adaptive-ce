//! ALP decoder (TS1 mode 4): validates every field of the stream before using it.

// Decoder hardening: arithmetic on values read from a payload is checked and every narrowing
// cast is explicit (`docs/DECODER-AUDIT.md`).
#![warn(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)]

use ace_bitpack::BitReader;
use ace_core::{AceError, AceResult};

use super::alp::{decode_digits, AlpParams};
use super::gamma::read_gamma;

/// Error for a stream that ends early or encodes an impossible field.
fn malformed() -> AceError {
    AceError::InvalidTimeSeries("truncated or invalid ALP stream")
}

/// Reads one field of `width` bits, mapping truncation to [`malformed`].
fn field(reader: &mut BitReader<'_>, width: u32) -> AceResult<u64> {
    reader.read_bits(width).ok_or_else(malformed)
}

/// Decodes `count` lanes of `lane_bits` bits into `emit`; rejects bad parameters, exceptions out
/// of order or past the end, offsets that leave the integer range, a stream whose length is not
/// exactly the declared sections and exceptions that do not carry a zero offset.
pub(crate) fn decode_alp(
    lane_bits: u32,
    count: usize,
    reader: &mut BitReader<'_>,
    mut emit: impl FnMut(u64),
) -> AceResult<()> {
    if count == 0 {
        return Ok(());
    }
    let e = field(reader, 5)?;
    let f = field(reader, 5)?;
    let params =
        AlpParams::new(e, f).ok_or(AceError::InvalidTimeSeries("ALP exponents out of range"))?;
    let width = u32::try_from(field(reader, 7)?)
        .ok()
        .filter(|&width| width <= 64)
        .ok_or(AceError::InvalidTimeSeries("ALP width out of range"))?;
    let base = field(reader, 64)? as i64;
    let total = u64::try_from(count).map_err(|_| malformed())?;
    let exception_count = read_gamma(reader).ok_or_else(malformed)?;
    if exception_count > total {
        return Err(AceError::InvalidTimeSeries(
            "ALP exceptions exceed value count",
        ));
    }
    // Every exception consumes at least one gamma bit and a lane, so the vector is bounded by
    // the stream length.
    let mut exceptions: Vec<(u64, u64)> = Vec::new();
    let mut next = 0u64;
    for _ in 0..exception_count {
        let gap = read_gamma(reader).ok_or_else(malformed)?;
        let position = next
            .checked_add(gap)
            .filter(|&position| position < total)
            .ok_or(AceError::InvalidTimeSeries("ALP exception out of range"))?;
        let bits = field(reader, lane_bits)?;
        if lane_bits < 64 && bits >> lane_bits != 0 {
            return Err(AceError::InvalidTimeSeries(
                "ALP exception exceeds lane width",
            ));
        }
        exceptions.push((position, bits));
        next = position.checked_add(1).ok_or_else(malformed)?;
    }
    let packed_bits = total.checked_mul(u64::from(width)).ok_or_else(malformed)?;
    if reader.remaining() != packed_bits {
        return Err(AceError::InvalidTimeSeries("ALP stream length mismatch"));
    }
    let mut pending = exceptions.iter().copied().peekable();
    for index in 0..total {
        let offset = field(reader, width)?;
        if let Some((_, bits)) = pending.next_if(|&(position, _)| position == index) {
            if offset != 0 {
                return Err(AceError::InvalidTimeSeries("ALP exception slot not zero"));
            }
            emit(bits);
            continue;
        }
        let d = i128::from(base)
            .checked_add(i128::from(offset))
            .and_then(|d| i64::try_from(d).ok())
            .ok_or(AceError::InvalidTimeSeries("ALP value out of range"))?;
        emit(decode_digits(d, params, lane_bits));
    }
    Ok(())
}
