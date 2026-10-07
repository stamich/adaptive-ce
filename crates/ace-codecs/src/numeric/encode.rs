//! NUM1 encoders: automatic selection, explicit `(width, mode)` and the fixed-step fast path.

use ace_bitpack::{
    delta, delta_of_delta, frame_of_reference, max_bit_width, pack, read_lanes, Lane,
};
use ace_core::{AceError, AceResult, NumericWidth};

use super::header::serialize_payload;
use super::lane_dispatch::{dispatch_lane, width_of};
use crate::{estimate_numeric, NumericMode, NumericPayloadInfo};

/// Error returned when fewer than two complete values exist for the requested width.
const TOO_SHORT: AceError =
    AceError::Malformed("numeric codec requires at least two integer values");

/// Encodes bytes with the smallest deterministic FOR/Delta/DoD+BitPack representation.
pub fn numeric_encode(input: &[u8]) -> AceResult<Vec<u8>> {
    let estimate = estimate_numeric(input).ok_or(TOO_SHORT)?;
    numeric_encode_with(input, estimate.width, estimate.mode)
}

/// Encodes bytes with an explicitly chosen lane width and mode.
///
/// Used by the engine to reuse the planner's estimate, by the ablation benchmark and by tests
/// that must exercise every `(width, mode)` pair.
pub fn numeric_encode_with(
    input: &[u8],
    width: NumericWidth,
    mode: NumericMode,
) -> AceResult<Vec<u8>> {
    if input.len() < width.bytes() * 2 {
        return Err(TOO_SHORT);
    }
    dispatch_lane!(width, encode_lane(input, mode))
}

/// Encodes a validated fixed-step block directly as zero-bit-width DoD (header + tail only).
///
/// The caller supplies evidence from ACE's strict full-block NumericFast validator; no mode
/// search, delta allocation or bit packing happens. Output is byte-identical to
/// `numeric_encode_with(input, width, DeltaOfDelta)` for such input.
pub fn numeric_encode_fixed_step(
    input: &[u8],
    width: NumericWidth,
    first_value: u64,
    first_delta: i64,
    value_count: usize,
    tail_bytes: usize,
) -> AceResult<Vec<u8>> {
    let value_bytes = value_count
        .checked_mul(width.bytes())
        .ok_or(AceError::ResourceLimitExceeded("numeric fast value bytes"))?;
    let total = value_bytes
        .checked_add(tail_bytes)
        .ok_or(AceError::ResourceLimitExceeded("numeric fast total bytes"))?;
    if total != input.len() {
        return Err(AceError::Malformed(
            "numeric fast evidence/input length mismatch",
        ));
    }
    if value_count < 2 {
        return Err(AceError::Malformed(
            "numeric fast path requires at least two values",
        ));
    }
    let info = NumericPayloadInfo {
        width,
        mode: NumericMode::DeltaOfDelta,
        bit_width: 0,
        value_count,
        tail_bytes,
        packed_bytes: 0,
        first_value,
        first_delta,
    };
    serialize_payload(&info, &[], &input[value_bytes..])
}

/// Width-generic encoder body: transform, ZigZag, bit-pack, serialize.
fn encode_lane<T: Lane>(input: &[u8], mode: NumericMode) -> AceResult<Vec<u8>> {
    let values = read_lanes::<T>(input);
    let tail = &input[values.len() * T::BYTES..];
    let (first, first_delta, stream): (T, i64, Vec<T>) = match mode {
        NumericMode::FrameOfReference => {
            let (base, offsets) = frame_of_reference(&values);
            (base, 0, offsets)
        }
        NumericMode::Delta => (
            values[0],
            0,
            delta(&values).into_iter().map(T::zigzag).collect(),
        ),
        NumericMode::DeltaOfDelta => {
            let (first_delta, dod) = delta_of_delta(&values);
            (
                values[0],
                T::signed_to_i64(first_delta),
                dod.into_iter().map(T::zigzag).collect(),
            )
        }
    };
    let bit_width = max_bit_width(&stream);
    let packed = pack(&stream, bit_width)?;
    let info = NumericPayloadInfo {
        width: width_of::<T>(),
        mode,
        bit_width,
        value_count: values.len(),
        tail_bytes: tail.len(),
        packed_bytes: packed.len(),
        first_value: first.to_u64(),
        first_delta,
    };
    serialize_payload(&info, &packed, tail)
}
