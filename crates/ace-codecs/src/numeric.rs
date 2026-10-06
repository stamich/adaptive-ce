use ace_bitpack::{
    delta_i32, delta_i64, delta_of_delta_i32, delta_of_delta_i64, frame_of_reference_u32,
    frame_of_reference_u64, max_bit_width_u32, max_bit_width_u64, pack_u32, pack_u64, undelta_i32,
    undelta_i64, undelta_of_delta_i32, undelta_of_delta_i64, unframe_of_reference_u32,
    unframe_of_reference_u64, unpack_u32, unpack_u64, unzigzag_u32, unzigzag_u64, zigzag_i32,
    zigzag_i64,
};
use ace_core::{AceError, AceResult, NumericWidth};

const MAGIC: [u8; 4] = *b"NUM1";
/// Fixed byte size of the self-describing NUM1 numeric payload header.
pub const NUMERIC_HEADER_SIZE: usize = 40;
const HEADER_SIZE: usize = NUMERIC_HEADER_SIZE;
const WIDTH_U32: u8 = 4;
const WIDTH_U64: u8 = 8;

/// Numeric sub-codec selected inside the self-describing ACE Format 1.3 numeric payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NumericMode {
    /// Stores offsets from the minimum value and bit-packs those offsets.
    FrameOfReference = 1,
    /// Stores ZigZag first-order deltas after the first value.
    Delta = 2,
    /// Stores first value, first delta, then ZigZag delta-of-delta values.
    DeltaOfDelta = 3,
}

impl TryFrom<u8> for NumericMode {
    type Error = AceError;

    /// Parses one serialized numeric mode identifier.
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::FrameOfReference),
            2 => Ok(Self::Delta),
            3 => Ok(Self::DeltaOfDelta),
            _ => Err(AceError::Malformed("unknown numeric mode")),
        }
    }
}

/// Compact estimate for one deterministic numeric encoding choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumericEstimate {
    /// Selected integer width.
    pub width: NumericWidth,
    /// Selected sub-codec mode.
    pub mode: NumericMode,
    /// Bit width used by the packed value stream.
    pub bit_width: u8,
    /// Complete encoded payload size including numeric header and tail bytes.
    pub encoded_bytes: usize,
}

/// Estimates the best deterministic numeric representation of arbitrary source bytes.
pub fn estimate_numeric(input: &[u8]) -> Option<NumericEstimate> {
    let u32_estimate = if input.len() >= 8 {
        Some(estimate_u32_input(input))
    } else {
        None
    };
    let u64_estimate = if input.len() >= 16 {
        Some(estimate_u64_input(input))
    } else {
        None
    };
    [u32_estimate, u64_estimate]
        .into_iter()
        .flatten()
        .min_by_key(|estimate| {
            (
                estimate.encoded_bytes,
                width_rank(estimate.width),
                estimate.mode as u8,
            )
        })
}

/// Encodes bytes with the smallest deterministic u32/u64 FOR/Delta/DoD+BitPack representation.
pub fn numeric_encode(input: &[u8]) -> AceResult<Vec<u8>> {
    let estimate = estimate_numeric(input).ok_or(AceError::Malformed(
        "numeric codec requires at least two integer values",
    ))?;
    match estimate.width {
        NumericWidth::U32 => encode_u32(input, estimate.mode),
        NumericWidth::U64 => encode_u64(input, estimate.mode),
    }
}

/// Encodes a previously validated fixed-step numeric block directly as zero-bit-width DoD.
///
/// The caller must supply evidence produced by ACE's strict full-block NumericFast validator.
/// No numeric mode search, delta allocation, bit-width scan or bit-packing pass is performed.
pub fn numeric_encode_fixed_step(
    input: &[u8],
    width: NumericWidth,
    first_value: u64,
    first_delta: i64,
    value_count: usize,
    tail_bytes: usize,
) -> AceResult<Vec<u8>> {
    let width_bytes = width.bytes();
    let value_bytes = value_count
        .checked_mul(width_bytes)
        .ok_or(AceError::ResourceLimitExceeded("numeric fast value bytes"))?;
    if value_bytes
        .checked_add(tail_bytes)
        .ok_or(AceError::ResourceLimitExceeded("numeric fast total bytes"))?
        != input.len()
    {
        return Err(AceError::Malformed(
            "numeric fast evidence/input length mismatch",
        ));
    }
    if value_count < 2 {
        return Err(AceError::Malformed(
            "numeric fast path requires at least two values",
        ));
    }

    let tail = &input[value_bytes..];
    build_payload(
        width,
        NumericMode::DeltaOfDelta,
        0,
        value_count,
        tail,
        first_value,
        first_delta,
        &[],
    )
}

/// Decodes a self-describing ACE Format 1.3 numeric payload to exactly `expected_size` bytes.
pub fn numeric_decode(input: &[u8], expected_size: usize) -> AceResult<Vec<u8>> {
    if input.len() < HEADER_SIZE {
        return Err(AceError::Malformed("truncated numeric header"));
    }
    if input[0..4] != MAGIC {
        return Err(AceError::Malformed("invalid numeric payload magic"));
    }
    let width = match input[4] {
        WIDTH_U32 => NumericWidth::U32,
        WIDTH_U64 => NumericWidth::U64,
        _ => return Err(AceError::Malformed("unsupported numeric width")),
    };
    let mode = NumericMode::try_from(input[5])?;
    let bit_width = input[6];
    let max_width = match width {
        NumericWidth::U32 => 32,
        NumericWidth::U64 => 64,
    };
    if bit_width > max_width {
        return Err(AceError::Malformed("numeric bit width exceeds value width"));
    }
    let count = u32::from_le_bytes(
        input[8..12]
            .try_into()
            .map_err(|_| AceError::Malformed("invalid numeric count"))?,
    ) as usize;
    let tail_len = u32::from_le_bytes(
        input[12..16]
            .try_into()
            .map_err(|_| AceError::Malformed("invalid numeric tail"))?,
    ) as usize;
    let first = u64::from_le_bytes(
        input[16..24]
            .try_into()
            .map_err(|_| AceError::Malformed("invalid numeric first/base value"))?,
    );
    let first_delta = i64::from_le_bytes(
        input[24..32]
            .try_into()
            .map_err(|_| AceError::Malformed("invalid numeric first delta"))?,
    );
    let packed_len = u32::from_le_bytes(
        input[32..36]
            .try_into()
            .map_err(|_| AceError::Malformed("invalid numeric packed length"))?,
    ) as usize;
    let payload_end = HEADER_SIZE
        .checked_add(packed_len)
        .ok_or(AceError::Malformed("numeric length overflow"))?;
    let tail_end = payload_end
        .checked_add(tail_len)
        .ok_or(AceError::Malformed("numeric length overflow"))?;
    if tail_end != input.len() {
        return Err(AceError::Malformed("numeric payload length mismatch"));
    }
    let value_bytes = count
        .checked_mul(width.bytes())
        .ok_or(AceError::Malformed("numeric output size overflow"))?;
    let reconstructed = value_bytes
        .checked_add(tail_len)
        .ok_or(AceError::Malformed("numeric output size overflow"))?;
    if reconstructed != expected_size {
        return Err(AceError::Malformed("numeric decoded size mismatch"));
    }
    let packed = &input[HEADER_SIZE..payload_end];
    let tail = &input[payload_end..tail_end];
    let mut out = match width {
        NumericWidth::U32 => decode_u32(
            mode,
            first as u32,
            first_delta as i32,
            bit_width,
            count,
            packed,
        )?,
        NumericWidth::U64 => decode_u64(mode, first, first_delta, bit_width, count, packed)?,
    };
    out.extend_from_slice(tail);
    debug_assert_eq!(out.len(), expected_size);
    Ok(out)
}

/// Estimates the best u32 interpretation of one source byte slice.
fn estimate_u32_input(input: &[u8]) -> NumericEstimate {
    let values = input
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("four-byte chunk")))
        .collect::<Vec<_>>();
    estimate_numeric_u32(&values, input.len() % 4)
}

/// Estimates the best u64 interpretation of one source byte slice.
fn estimate_u64_input(input: &[u8]) -> NumericEstimate {
    let values = input
        .chunks_exact(8)
        .map(|chunk| u64::from_le_bytes(chunk.try_into().expect("eight-byte chunk")))
        .collect::<Vec<_>>();
    estimate_numeric_u64(&values, input.len() % 8)
}

/// Estimates the smallest exact numeric representation for a u32 sequence.
pub fn estimate_numeric_u32(values: &[u32], tail_len: usize) -> NumericEstimate {
    let for_est = estimate_for_u32(values, tail_len);
    let delta_est = estimate_delta_u32(values, tail_len);
    let dod_est = estimate_dod_u32(values, tail_len);
    [for_est, delta_est, dod_est]
        .into_iter()
        .min_by_key(|estimate| (estimate.encoded_bytes, estimate.mode as u8))
        .unwrap_or(for_est)
}

/// Estimates the smallest exact numeric representation for a u64 sequence.
pub fn estimate_numeric_u64(values: &[u64], tail_len: usize) -> NumericEstimate {
    let for_est = estimate_for_u64(values, tail_len);
    let delta_est = estimate_delta_u64(values, tail_len);
    let dod_est = estimate_dod_u64(values, tail_len);
    [for_est, delta_est, dod_est]
        .into_iter()
        .min_by_key(|estimate| (estimate.encoded_bytes, estimate.mode as u8))
        .unwrap_or(for_est)
}

/// Encodes one u32 numeric mode.
fn encode_u32(input: &[u8], mode: NumericMode) -> AceResult<Vec<u8>> {
    let values = input
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("four-byte chunk")))
        .collect::<Vec<_>>();
    let tail = &input[values.len() * 4..];
    let (first, first_delta, encoded_values) = match mode {
        NumericMode::FrameOfReference => {
            let (base, offsets) = frame_of_reference_u32(&values);
            (base as u64, 0i64, offsets)
        }
        NumericMode::Delta => {
            let first = values[0];
            (
                first as u64,
                0,
                delta_i32(&values).into_iter().map(zigzag_i32).collect(),
            )
        }
        NumericMode::DeltaOfDelta => {
            let first = values[0];
            let (delta, dod) = delta_of_delta_i32(&values);
            (
                first as u64,
                delta as i64,
                dod.into_iter().map(zigzag_i32).collect(),
            )
        }
    };
    let bit_width = max_bit_width_u32(&encoded_values);
    let packed = pack_u32(&encoded_values, bit_width)?;
    build_payload(
        NumericWidth::U32,
        mode,
        bit_width,
        values.len(),
        tail,
        first,
        first_delta,
        &packed,
    )
}

/// Encodes one u64 numeric mode.
fn encode_u64(input: &[u8], mode: NumericMode) -> AceResult<Vec<u8>> {
    let values = input
        .chunks_exact(8)
        .map(|chunk| u64::from_le_bytes(chunk.try_into().expect("eight-byte chunk")))
        .collect::<Vec<_>>();
    let tail = &input[values.len() * 8..];
    let (first, first_delta, encoded_values) = match mode {
        NumericMode::FrameOfReference => {
            let (base, offsets) = frame_of_reference_u64(&values);
            (base, 0i64, offsets)
        }
        NumericMode::Delta => {
            let first = values[0];
            (
                first,
                0,
                delta_i64(&values).into_iter().map(zigzag_i64).collect(),
            )
        }
        NumericMode::DeltaOfDelta => {
            let first = values[0];
            let (delta, dod) = delta_of_delta_i64(&values);
            (first, delta, dod.into_iter().map(zigzag_i64).collect())
        }
    };
    let bit_width = max_bit_width_u64(&encoded_values);
    let packed = pack_u64(&encoded_values, bit_width)?;
    build_payload(
        NumericWidth::U64,
        mode,
        bit_width,
        values.len(),
        tail,
        first,
        first_delta,
        &packed,
    )
}

/// Serializes the common fixed numeric header followed by packed values and tail bytes.
fn build_payload(
    width: NumericWidth,
    mode: NumericMode,
    bit_width: u8,
    count: usize,
    tail: &[u8],
    first: u64,
    first_delta: i64,
    packed: &[u8],
) -> AceResult<Vec<u8>> {
    if count > u32::MAX as usize
        || tail.len() > u32::MAX as usize
        || packed.len() > u32::MAX as usize
    {
        return Err(AceError::ResourceLimitExceeded("numeric payload metadata"));
    }
    let mut out = vec![0u8; HEADER_SIZE];
    out[0..4].copy_from_slice(&MAGIC);
    out[4] = width.bytes() as u8;
    out[5] = mode as u8;
    out[6] = bit_width;
    out[8..12].copy_from_slice(&(count as u32).to_le_bytes());
    out[12..16].copy_from_slice(&(tail.len() as u32).to_le_bytes());
    out[16..24].copy_from_slice(&first.to_le_bytes());
    out[24..32].copy_from_slice(&first_delta.to_le_bytes());
    out[32..36].copy_from_slice(&(packed.len() as u32).to_le_bytes());
    out.extend_from_slice(packed);
    out.extend_from_slice(tail);
    Ok(out)
}

/// Decodes one u32 numeric stream and serializes reconstructed values to bytes.
fn decode_u32(
    mode: NumericMode,
    first: u32,
    first_delta: i32,
    bit_width: u8,
    count: usize,
    packed: &[u8],
) -> AceResult<Vec<u8>> {
    if bit_width == 0 {
        return decode_zero_width_u32(mode, first, first_delta, count, packed);
    }

    let values = match mode {
        NumericMode::FrameOfReference => {
            unframe_of_reference_u32(first, &unpack_u32(packed, count, bit_width)?)
        }
        NumericMode::Delta => {
            if count == 0 {
                Vec::new()
            } else {
                let deltas = unpack_u32(packed, count.saturating_sub(1), bit_width)?
                    .into_iter()
                    .map(unzigzag_u32)
                    .collect::<Vec<_>>();
                undelta_i32(first, &deltas)
            }
        }
        NumericMode::DeltaOfDelta => {
            if count == 0 {
                Vec::new()
            } else if count == 1 {
                vec![first]
            } else {
                let dod = unpack_u32(packed, count.saturating_sub(2), bit_width)?
                    .into_iter()
                    .map(unzigzag_u32)
                    .collect::<Vec<_>>();
                undelta_of_delta_i32(first, first_delta, &dod)
            }
        }
    };
    if values.len() != count {
        return Err(AceError::Malformed("numeric u32 value count mismatch"));
    }
    let mut out = Vec::with_capacity(count.saturating_mul(4));
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
    Ok(out)
}

/// Reconstructs zero-bit-width u32 numeric streams without a bit reader or temporary value vectors.
fn decode_zero_width_u32(
    mode: NumericMode,
    first: u32,
    first_delta: i32,
    count: usize,
    packed: &[u8],
) -> AceResult<Vec<u8>> {
    if !packed.is_empty() {
        return Err(AceError::Malformed(
            "zero-width numeric u32 stream must have empty payload",
        ));
    }
    let mut out = Vec::with_capacity(count.saturating_mul(4));
    match mode {
        NumericMode::FrameOfReference | NumericMode::Delta => {
            for _ in 0..count {
                out.extend_from_slice(&first.to_le_bytes());
            }
        }
        NumericMode::DeltaOfDelta => {
            let mut value = first;
            for index in 0..count {
                if index > 0 {
                    value = value.wrapping_add(first_delta as u32);
                }
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
    Ok(out)
}

/// Decodes one u64 numeric stream and serializes reconstructed values to bytes.
fn decode_u64(
    mode: NumericMode,
    first: u64,
    first_delta: i64,
    bit_width: u8,
    count: usize,
    packed: &[u8],
) -> AceResult<Vec<u8>> {
    if bit_width == 0 {
        return decode_zero_width_u64(mode, first, first_delta, count, packed);
    }

    let values = match mode {
        NumericMode::FrameOfReference => {
            unframe_of_reference_u64(first, &unpack_u64(packed, count, bit_width)?)
        }
        NumericMode::Delta => {
            if count == 0 {
                Vec::new()
            } else {
                let deltas = unpack_u64(packed, count.saturating_sub(1), bit_width)?
                    .into_iter()
                    .map(unzigzag_u64)
                    .collect::<Vec<_>>();
                undelta_i64(first, &deltas)
            }
        }
        NumericMode::DeltaOfDelta => {
            if count == 0 {
                Vec::new()
            } else if count == 1 {
                vec![first]
            } else {
                let dod = unpack_u64(packed, count.saturating_sub(2), bit_width)?
                    .into_iter()
                    .map(unzigzag_u64)
                    .collect::<Vec<_>>();
                undelta_of_delta_i64(first, first_delta, &dod)
            }
        }
    };
    if values.len() != count {
        return Err(AceError::Malformed("numeric u64 value count mismatch"));
    }
    let mut out = Vec::with_capacity(count.saturating_mul(8));
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
    Ok(out)
}

/// Reconstructs zero-bit-width u64 numeric streams without a bit reader or temporary value vectors.
fn decode_zero_width_u64(
    mode: NumericMode,
    first: u64,
    first_delta: i64,
    count: usize,
    packed: &[u8],
) -> AceResult<Vec<u8>> {
    if !packed.is_empty() {
        return Err(AceError::Malformed(
            "zero-width numeric u64 stream must have empty payload",
        ));
    }
    let mut out = Vec::with_capacity(count.saturating_mul(8));
    match mode {
        NumericMode::FrameOfReference | NumericMode::Delta => {
            for _ in 0..count {
                out.extend_from_slice(&first.to_le_bytes());
            }
        }
        NumericMode::DeltaOfDelta => {
            let mut value = first;
            for index in 0..count {
                if index > 0 {
                    value = value.wrapping_add(first_delta as u64);
                }
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
    Ok(out)
}

/// Estimates FOR+BitPack size for u32 values.
fn estimate_for_u32(values: &[u32], tail_len: usize) -> NumericEstimate {
    let (_, offsets) = frame_of_reference_u32(values);
    estimate_stream(
        NumericWidth::U32,
        NumericMode::FrameOfReference,
        max_bit_width_u32(&offsets),
        offsets.len(),
        tail_len,
    )
}
/// Estimates Delta+BitPack size for u32 values.
fn estimate_delta_u32(values: &[u32], tail_len: usize) -> NumericEstimate {
    let encoded = delta_i32(values)
        .into_iter()
        .map(zigzag_i32)
        .collect::<Vec<_>>();
    estimate_stream(
        NumericWidth::U32,
        NumericMode::Delta,
        max_bit_width_u32(&encoded),
        encoded.len(),
        tail_len,
    )
}
/// Estimates DoD+BitPack size for u32 values.
fn estimate_dod_u32(values: &[u32], tail_len: usize) -> NumericEstimate {
    let (_, dod) = delta_of_delta_i32(values);
    let encoded = dod.into_iter().map(zigzag_i32).collect::<Vec<_>>();
    estimate_stream(
        NumericWidth::U32,
        NumericMode::DeltaOfDelta,
        max_bit_width_u32(&encoded),
        encoded.len(),
        tail_len,
    )
}
/// Estimates FOR+BitPack size for u64 values.
fn estimate_for_u64(values: &[u64], tail_len: usize) -> NumericEstimate {
    let (_, offsets) = frame_of_reference_u64(values);
    estimate_stream(
        NumericWidth::U64,
        NumericMode::FrameOfReference,
        max_bit_width_u64(&offsets),
        offsets.len(),
        tail_len,
    )
}
/// Estimates Delta+BitPack size for u64 values.
fn estimate_delta_u64(values: &[u64], tail_len: usize) -> NumericEstimate {
    let encoded = delta_i64(values)
        .into_iter()
        .map(zigzag_i64)
        .collect::<Vec<_>>();
    estimate_stream(
        NumericWidth::U64,
        NumericMode::Delta,
        max_bit_width_u64(&encoded),
        encoded.len(),
        tail_len,
    )
}
/// Estimates DoD+BitPack size for u64 values.
fn estimate_dod_u64(values: &[u64], tail_len: usize) -> NumericEstimate {
    let (_, dod) = delta_of_delta_i64(values);
    let encoded = dod.into_iter().map(zigzag_i64).collect::<Vec<_>>();
    estimate_stream(
        NumericWidth::U64,
        NumericMode::DeltaOfDelta,
        max_bit_width_u64(&encoded),
        encoded.len(),
        tail_len,
    )
}

/// Converts count/bit-width metadata into a full numeric payload size.
fn estimate_stream(
    width: NumericWidth,
    mode: NumericMode,
    bit_width: u8,
    count: usize,
    tail_len: usize,
) -> NumericEstimate {
    let bits = count.saturating_mul(bit_width as usize);
    NumericEstimate {
        width,
        mode,
        bit_width,
        encoded_bytes: HEADER_SIZE + (bits + 7) / 8 + tail_len,
    }
}

/// Provides a deterministic width tie-break preferring u32 over u64 when encoded sizes match.
fn width_rank(width: NumericWidth) -> u8 {
    match width {
        NumericWidth::U32 => 0,
        NumericWidth::U64 => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies monotonic u32 counters choose a compact numeric representation and round-trip exactly.
    #[test]
    fn u32_counter_roundtrip() {
        let mut input = Vec::new();
        for value in 1000u32..50_000 {
            input.extend_from_slice(&value.to_le_bytes());
        }
        input.extend_from_slice(&[1, 2, 3]);
        let encoded = numeric_encode(&input).unwrap();
        assert!(encoded.len() < input.len() / 2);
        assert_eq!(numeric_decode(&encoded, input.len()).unwrap(), input);
    }

    /// Verifies millisecond-like u64 timestamps use a reversible compact path.
    #[test]
    fn u64_timestamp_roundtrip() {
        let mut input = Vec::new();
        let mut value = 1_780_000_000_000u64;
        for i in 0..20_000u64 {
            value += 1000 + (i % 3);
            input.extend_from_slice(&value.to_le_bytes());
        }
        let encoded = numeric_encode(&input).unwrap();
        assert!(encoded.len() < input.len() / 2);
        assert_eq!(numeric_decode(&encoded, input.len()).unwrap(), input);
    }

    /// Verifies validated fixed-step evidence can bypass the generic numeric mode search.
    #[test]
    fn direct_fixed_step_encoder_roundtrip() {
        let mut input = Vec::new();
        let mut value = 10_000u32;
        let first;
        value = value.wrapping_add(3);
        first = value;
        input.extend_from_slice(&value.to_le_bytes());
        for _ in 1..10_000 {
            value = value.wrapping_add(3);
            input.extend_from_slice(&value.to_le_bytes());
        }

        let encoded =
            numeric_encode_fixed_step(&input, NumericWidth::U32, first as u64, 3, 10_000, 0)
                .unwrap();
        assert_eq!(encoded[5], NumericMode::DeltaOfDelta as u8);
        assert_eq!(encoded[6], 0);
        assert_eq!(numeric_decode(&encoded, input.len()).unwrap(), input);
    }

    /// Verifies the zero-bit-width DoD decoder reconstructs a fixed-step u32 stream directly.
    #[test]
    fn zero_width_u32_dod_roundtrip() {
        let mut input = Vec::new();
        let mut value = 10_000u32;
        for _ in 0..10_000 {
            value = value.wrapping_add(3);
            input.extend_from_slice(&value.to_le_bytes());
        }
        let encoded = numeric_encode(&input).unwrap();
        assert_eq!(
            encoded[6], 0,
            "fixed-step DoD should require zero packed bits"
        );
        assert_eq!(numeric_decode(&encoded, input.len()).unwrap(), input);
    }

    /// Verifies the zero-bit-width DoD decoder reconstructs a fixed-step u64 stream directly.
    #[test]
    fn zero_width_u64_dod_roundtrip() {
        let mut input = Vec::new();
        let mut value = 1_780_000_000_000u64;
        for _ in 0..10_000 {
            value = value.wrapping_add(1000);
            input.extend_from_slice(&value.to_le_bytes());
        }
        let encoded = numeric_encode(&input).unwrap();
        assert_eq!(
            encoded[6], 0,
            "fixed-step DoD should require zero packed bits"
        );
        assert_eq!(numeric_decode(&encoded, input.len()).unwrap(), input);
    }
}
