//! Patched FOR (NUM1 mode 4, Format 1.5) decoder.

// Decoder hardening (ACE 0.6.1): arithmetic on values read from a payload is checked and every
// narrowing cast is explicit (`docs/DECODER-AUDIT.md`).
#![warn(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)]

use ace_bitpack::{
    packed_len, undelta_iter, undelta_of_delta_iter, unframe_iter, unpack_iter,
    write_lanes_from_iter, Lane,
};
use ace_core::{AceError, AceResult};

use crate::{NumericMode, NumericPatch, NumericPayloadInfo};

/// Decodes a validated Patched FOR payload body (`packed` low parts and `section` exceptions).
///
/// `numeric_inspect` has proved the section length, the exception count and the width byte;
/// this function additionally proves that the positions are strictly increasing and inside the
/// stream and that every high part is non-zero (an exception that changes nothing is not a
/// valid encoding), before any value is produced.
pub(super) fn decode_patched_lane<T: Lane>(
    info: &NumericPayloadInfo,
    patch: &NumericPatch,
    packed: &[u8],
    section: &[u8],
) -> AceResult<Vec<u8>> {
    let stream_len = patch.source.stream_len(info.value_count);
    let low = unpack_iter::<T>(packed, stream_len, info.bit_width)?;
    if patch.exception_count == 0 {
        return reconstruct(info, patch.source, low);
    }

    let position_bits = NumericPatch::position_bits(stream_len);
    let (position_bytes, high_bytes) = section
        .get(1..)
        .and_then(|body| {
            body.split_at_checked(packed_len(patch.exception_count, position_bits).ok()?)
        })
        .ok_or(AceError::Malformed("numeric exception section mismatch"))?;
    let positions = || unpack_iter::<u64>(position_bytes, patch.exception_count, position_bits);
    let highs = || unpack_iter::<T>(high_bytes, patch.exception_count, patch.high_bits);

    let mut previous: Option<u64> = None;
    for position in positions()? {
        let inside = usize::try_from(position).is_ok_and(|index| index < stream_len);
        if !inside || previous.is_some_and(|last| position <= last) {
            return Err(AceError::Malformed(
                "numeric exception positions out of order",
            ));
        }
        previous = Some(position);
    }
    if highs()?.any(|high| high == T::default()) {
        return Err(AceError::Malformed("numeric exception without high bits"));
    }

    let mut positions = positions()?;
    let mut highs = highs()?;
    let patched = PatchedStream {
        stream: low,
        pending: positions.next().zip(highs.next()),
        positions,
        highs,
        index: 0,
        shift: u32::from(info.bit_width),
    };
    reconstruct(info, patch.source, patched)
}

/// The low-part stream with the exception high parts or-ed in at their positions.
struct PatchedStream<T, S, P, H> {
    /// Packed low parts, one per stream value.
    stream: S,
    /// Remaining exception positions (strictly increasing).
    positions: P,
    /// Remaining exception high parts.
    highs: H,
    /// The next exception: `(position, high part)`.
    pending: Option<(u64, T)>,
    /// Index of the value `next` yields.
    index: u64,
    /// Packed width: how far a high part is shifted left.
    shift: u32,
}

/// Iterator implementation of [`PatchedStream`].
impl<T, S, P, H> Iterator for PatchedStream<T, S, P, H>
where
    T: Lane,
    S: Iterator<Item = T>,
    P: Iterator<Item = u64>,
    H: Iterator<Item = T>,
{
    type Item = T;

    fn next(&mut self) -> Option<T> {
        let low = self.stream.next()?;
        let value = match self.pending {
            Some((position, high)) if position == self.index => {
                self.pending = self.positions.next().zip(self.highs.next());
                // `numeric_inspect` proved `shift + high bits <= lane bits`, so nothing is lost.
                let high = high.to_u64().checked_shl(self.shift).unwrap_or(0);
                T::from_u64_truncating(low.to_u64() | high)
            }
            _ => low,
        };
        self.index = self.index.saturating_add(1);
        Some(value)
    }
}

/// Inverse transform of the source mode over the (patched) stream, written as lanes.
fn reconstruct<T: Lane>(
    info: &NumericPayloadInfo,
    source: NumericMode,
    stream: impl Iterator<Item = T>,
) -> AceResult<Vec<u8>> {
    let count = info.value_count;
    let first = T::from_u64_truncating(info.first_value);
    let (bytes, written) = match source {
        NumericMode::FrameOfReference => write_lanes_from_iter(count, unframe_iter(first, stream)),
        NumericMode::Delta if count > 0 => {
            write_lanes_from_iter(count, undelta_iter(first, stream.map(T::unzigzag)))
        }
        NumericMode::DeltaOfDelta if count > 1 => {
            let first_delta = T::from_u64_truncating(info.first_delta as u64).to_signed();
            write_lanes_from_iter(
                count,
                undelta_of_delta_iter(first, first_delta, stream.map(T::unzigzag)),
            )
        }
        NumericMode::DeltaOfDelta if count == 1 => write_lanes_from_iter(1, std::iter::once(first)),
        _ => (Vec::new(), 0),
    };
    if written != count {
        return Err(AceError::Malformed("numeric value count mismatch"));
    }
    Ok(bytes)
}
