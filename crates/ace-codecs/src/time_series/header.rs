//! The 28-byte TS1 wire header: parsing with full validation, and serialization.
//!
//! ```text
//! offset size field
//! 0      4    magic "TS1\0"
//! 4      1    version (= 1)
//! 5      1    mode (TimeSeriesMode)
//! 6      1    lane_bytes (8 / 4 for Gorilla; 2 / 4 / 8 for RunDelta)
//! 7      1    flags (bit 0: Gorilla window reuse; all other bits must be zero)
//! 8      4    value_count (complete lanes)
//! 12     1    tail_len (= original_len % lane_bytes)
//! 13     3    reserved (zero)
//! 16     8    first_bits (raw bits of the first value, zero-extended)
//! 24     4    stream_bits (bit length of the stream that follows)
//! 28     n    stream (ceil(stream_bits / 8) bytes, LSB-first)
//! 28+n   t    tail (tail_len verbatim bytes)
//! ```

use ace_core::{AceError, AceResult};

use super::TimeSeriesMode;

/// Four-byte magic that starts every TS1 payload.
const MAGIC: [u8; 4] = *b"TS1\0";
/// Only defined TS1 header version.
const VERSION: u8 = 1;
/// Fixed byte size of the TS1 header.
pub const TIME_SERIES_HEADER_SIZE: usize = 28;
/// Flag bit: Gorilla encoder may reuse the previous leading/meaningful-bit window.
pub const FLAG_WINDOW_REUSE: u8 = 0x01;

/// Header-level description of one TS1 payload (everything except stream and tail bytes).
///
/// Produced by [`ts1_inspect`] without decoding any value; also the single input of the
/// serializer, so writer and reader share one definition of the fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeSeriesPayloadInfo {
    /// Encoding mode.
    pub mode: TimeSeriesMode,
    /// Bytes per lane.
    pub lane_bytes: u8,
    /// Flag bits (see [`FLAG_WINDOW_REUSE`]).
    pub flags: u8,
    /// Number of complete lanes.
    pub value_count: usize,
    /// Number of verbatim trailing bytes.
    pub tail_len: usize,
    /// Raw bits of the first value (zero when `value_count == 0`).
    pub first_bits: u64,
    /// Bit length of the encoded stream.
    pub stream_bits: u64,
}

/// Inherent methods of [`TimeSeriesPayloadInfo`].
impl TimeSeriesPayloadInfo {
    /// Number of bytes the payload decodes to (`value_count * lane_bytes + tail_len`).
    pub fn decoded_len(&self) -> AceResult<usize> {
        self.value_count
            .checked_mul(self.lane_bytes as usize)
            .and_then(|len| len.checked_add(self.tail_len))
            .ok_or(AceError::InvalidTimeSeries("decoded length overflow"))
    }

    /// Bytes occupied by the encoded stream.
    pub fn stream_bytes(&self) -> usize {
        self.stream_bits.div_ceil(8) as usize
    }

    /// Average stored stream bits per value (header and tail excluded).
    pub fn bits_per_value(&self) -> f64 {
        if self.value_count == 0 {
            0.0
        } else {
            self.stream_bits as f64 / self.value_count as f64
        }
    }
}

/// Parses and fully validates the TS1 header of `payload` without decoding any value.
///
/// Every structural check of the decoder that does not need the expected output size happens
/// here, so `ace inspect` reports exactly what the decoder would accept.
pub fn ts1_inspect(payload: &[u8]) -> AceResult<TimeSeriesPayloadInfo> {
    let header = payload
        .get(..TIME_SERIES_HEADER_SIZE)
        .ok_or(AceError::InvalidTimeSeries("truncated TS1 header"))?;
    if header[0..4] != MAGIC {
        return Err(AceError::InvalidTimeSeries("invalid TS1 magic"));
    }
    if header[4] != VERSION {
        return Err(AceError::InvalidTimeSeries("unsupported TS1 version"));
    }
    let mode = TimeSeriesMode::try_from(header[5])?;
    let lane_bytes = header[6];
    if !mode.accepts_lane_bytes(lane_bytes) {
        return Err(AceError::InvalidTimeSeries(
            "lane width not valid for TS1 mode",
        ));
    }
    let flags = header[7];
    let allowed_flags = match mode {
        TimeSeriesMode::GorillaF64 | TimeSeriesMode::GorillaF32 => FLAG_WINDOW_REUSE,
        TimeSeriesMode::RunDelta => 0,
    };
    if flags & !allowed_flags != 0 {
        return Err(AceError::InvalidTimeSeries("unknown TS1 flags"));
    }
    if header[13..16] != [0, 0, 0] {
        return Err(AceError::InvalidTimeSeries(
            "reserved TS1 bytes are not zero",
        ));
    }
    let info = TimeSeriesPayloadInfo {
        mode,
        lane_bytes,
        flags,
        value_count: ace_core::read_lane::<4>(&header[8..]) as usize,
        tail_len: header[12] as usize,
        first_bits: ace_core::read_lane::<8>(&header[16..]),
        stream_bits: ace_core::read_lane::<4>(&header[24..]),
    };
    if info.tail_len >= lane_bytes as usize {
        return Err(AceError::InvalidTimeSeries("TS1 tail longer than one lane"));
    }
    let lane_bits = lane_bytes as u32 * 8;
    if lane_bits < 64 && info.first_bits >> lane_bits != 0 {
        return Err(AceError::InvalidTimeSeries(
            "TS1 first value exceeds lane width",
        ));
    }
    if info.value_count == 0 && (info.stream_bits != 0 || info.first_bits != 0) {
        return Err(AceError::InvalidTimeSeries("empty TS1 series carries data"));
    }
    let total = TIME_SERIES_HEADER_SIZE
        .checked_add(info.stream_bytes())
        .and_then(|len| len.checked_add(info.tail_len))
        .ok_or(AceError::InvalidTimeSeries("TS1 length overflow"))?;
    if total != payload.len() {
        return Err(AceError::InvalidTimeSeries("TS1 payload length mismatch"));
    }
    Ok(info)
}

/// Serializes `info` followed by the stream and the tail bytes.
///
/// `info.tail_len` is taken from `tail`, so the header cannot disagree with the body; the
/// stream length must match `info.stream_bits`.
pub(crate) fn serialize_payload(
    info: &TimeSeriesPayloadInfo,
    stream: &[u8],
    tail: &[u8],
) -> AceResult<Vec<u8>> {
    let value_count = u32::try_from(info.value_count)
        .map_err(|_| AceError::ResourceLimitExceeded("TS1 value count"))?;
    let stream_bits = u32::try_from(info.stream_bits)
        .map_err(|_| AceError::ResourceLimitExceeded("TS1 stream length"))?;
    let tail_len =
        u8::try_from(tail.len()).map_err(|_| AceError::InvalidTimeSeries("TS1 tail too long"))?;
    if stream.len() != info.stream_bytes() {
        return Err(AceError::InvalidTimeSeries("TS1 stream length mismatch"));
    }
    let mut out = Vec::with_capacity(TIME_SERIES_HEADER_SIZE + stream.len() + tail.len());
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&[VERSION, info.mode as u8, info.lane_bytes, info.flags]);
    out.extend_from_slice(&value_count.to_le_bytes());
    out.extend_from_slice(&[tail_len, 0, 0, 0]);
    out.extend_from_slice(&info.first_bits.to_le_bytes());
    out.extend_from_slice(&stream_bits.to_le_bytes());
    debug_assert_eq!(out.len(), TIME_SERIES_HEADER_SIZE);
    out.extend_from_slice(stream);
    out.extend_from_slice(tail);
    Ok(out)
}
