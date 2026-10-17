//! `CodecId` → codec dispatch (the only place that knows every primary codec).

use ace_core::{AceResult, CodecId, LzMode};

use crate::{
    lz_decode, lz_encode, numeric_decode, numeric_encode, raw_decode, raw_encode, rle_decode,
    rle_encode, ts1_decode, ts1_encode,
};

/// First NUM1 / TS1 payload mode that needs Format 1.5 (modes 1–3 exist since 1.3 / 1.4).
const FIRST_FORMAT_1_5_MODE: u8 = 4;

/// Byte offset of the mode field in the NUM1 and TS1 payload headers (after the 4-byte magic,
/// the lane / version byte).
const PAYLOAD_MODE_OFFSET: usize = 5;

/// Lowest minor format version whose readers can decode `primary`, the stream `codec` produced.
///
/// RAW, RLE and LZ streams are readable by every version (`0`); a NUM1 stream needs 1.3 and a
/// TS1 stream 1.4, or 1.5 when its header names a mode introduced by Format 1.5. Only the
/// magic and the mode byte are looked at: a payload that is malformed in any other way is
/// reported by the decoder, and a payload whose magic does not match is attributed to the
/// codec's base version.
#[must_use]
pub fn payload_minor_version(codec: CodecId, primary: &[u8]) -> u8 {
    let (base, magic): (u8, &[u8]) = match codec {
        CodecId::Raw | CodecId::Rle | CodecId::Lz => return 0,
        CodecId::Numeric => (3, b"NUM1"),
        CodecId::TimeSeries => (4, b"TS1\0"),
    };
    let has_new_mode = primary.starts_with(magic)
        && primary
            .get(PAYLOAD_MODE_OFFSET)
            .is_some_and(|&mode| mode >= FIRST_FORMAT_1_5_MODE);
    if has_new_mode {
        5
    } else {
        base
    }
}

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
