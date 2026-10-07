//! NUM1 decoder.

use ace_bitpack::{
    undelta_iter, undelta_of_delta_iter, unframe_iter, unpack_iter, write_lanes_from_iter, Lane,
};
use ace_core::{AceError, AceResult};

use super::lane_dispatch::dispatch_lane;
use crate::{numeric_inspect, NumericMode, NumericPayloadInfo, NUMERIC_HEADER_SIZE};

/// Decodes a NUM1 payload to exactly `expected_size` bytes.
///
/// The declared value/tail sizes must add up to `expected_size`; this bounds every allocation
/// by a caller-controlled limit (`DecodeLimits::max_block_size`) before any value is decoded.
pub fn numeric_decode(input: &[u8], expected_size: usize) -> AceResult<Vec<u8>> {
    let info = numeric_inspect(input)?;
    if info.decoded_len()? != expected_size {
        return Err(AceError::Malformed("numeric decoded size mismatch"));
    }
    let packed = &input[NUMERIC_HEADER_SIZE..NUMERIC_HEADER_SIZE + info.packed_bytes];
    let tail = &input[NUMERIC_HEADER_SIZE + info.packed_bytes..];

    let mut out = if info.bit_width == 0 {
        if !packed.is_empty() {
            return Err(AceError::Malformed(
                "zero-width numeric stream must have empty payload",
            ));
        }
        dispatch_lane!(info.width, decode_zero_width(&info))
    } else {
        dispatch_lane!(info.width, decode_lane(&info, packed))?
    };
    out.extend_from_slice(tail);
    debug_assert_eq!(out.len(), expected_size);
    Ok(out)
}

/// Width-generic decoder body for a non-empty packed stream.
///
/// Unpacking, ZigZag decoding, the inverse transform and lane serialization are fused into one
/// iterator pipeline, so no intermediate vector is allocated.
fn decode_lane<T: Lane>(info: &NumericPayloadInfo, packed: &[u8]) -> AceResult<Vec<u8>> {
    let count = info.value_count;
    let stream = unpack_iter::<T>(packed, info.mode.stream_len(count), info.bit_width)?;
    let first = T::from_u64_truncating(info.first_value);
    let (bytes, written) = match info.mode {
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

/// Reconstructs a zero-bit-width stream (constant or fixed-step sequence) for any lane.
///
/// With `bit_width == 0` every stored value is zero, so the sequence is implied by the header:
/// FOR/Delta give a constant `first`; DoD gives `first + i * first_delta` in the lane ring. The
/// output is written in place without a bit reader or temporary vector (memory speed).
fn decode_zero_width<T: Lane>(info: &NumericPayloadInfo) -> Vec<u8> {
    let step = match info.mode {
        NumericMode::DeltaOfDelta => T::from_u64_truncating(info.first_delta as u64),
        NumericMode::FrameOfReference | NumericMode::Delta => T::default(),
    };
    let mut out = vec![0u8; info.value_count.saturating_mul(T::BYTES)];
    let mut value = T::from_u64_truncating(info.first_value);
    for chunk in out.chunks_exact_mut(T::BYTES) {
        value.write_le(chunk);
        value = value.wrapping_add(step);
    }
    out
}
