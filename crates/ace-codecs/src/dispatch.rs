//! `CodecId` → codec dispatch (the only place that knows every primary codec).

use ace_core::{AceResult, CodecId, LzMode};

use crate::{
    lz_decode, lz_encode, numeric_decode, numeric_encode, raw_decode, raw_encode, rle_decode,
    rle_encode, ts1_decode, ts1_encode,
};

/// Encodes a transformed byte stream with the requested primary codec.
///
/// `lz_mode` is only consulted for [`CodecId::Lz`] and defaults to [`LzMode::Fast`];
/// [`CodecId::TimeSeries`] picks the smallest TS1 layout.
///
/// # Errors
///
/// Propagates the codec's error; the Numeric and TS1 encoders reject input that has no
/// valid layout (for example fewer than two values).
pub fn encode_codec(id: CodecId, lz_mode: Option<LzMode>, input: &[u8]) -> AceResult<Vec<u8>> {
    match id {
        CodecId::Raw => Ok(raw_encode(input)),
        CodecId::Rle => Ok(rle_encode(input)),
        CodecId::Lz => Ok(lz_encode(input, lz_mode.unwrap_or(LzMode::Fast))),
        CodecId::Numeric => numeric_encode(input),
        CodecId::TimeSeries => ts1_encode(input),
    }
}

/// Decodes a primary codec stream to exactly `expected_size` bytes.
///
/// # Errors
///
/// Propagates the codec's validation error ([`ace_core::AceError::Malformed`],
/// [`ace_core::AceError::InvalidLzReference`], [`ace_core::AceError::InvalidTimeSeries`]) when the payload is
/// corrupt or does not decode to exactly `expected_size` bytes.
pub fn decode_codec(id: CodecId, input: &[u8], expected_size: usize) -> AceResult<Vec<u8>> {
    match id {
        CodecId::Raw => raw_decode(input, expected_size),
        CodecId::Rle => rle_decode(input, expected_size),
        CodecId::Lz => lz_decode(input, expected_size),
        CodecId::Numeric => numeric_decode(input, expected_size),
        CodecId::TimeSeries => ts1_decode(input, expected_size),
    }
}
