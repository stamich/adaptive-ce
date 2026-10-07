//! Public TS1 entry points: encode with an explicit mode, exact encoded size, decode.

use ace_bitpack::{BitReader, BitWriter};
use ace_core::{read_lane, AceError, AceResult};

use super::gorilla::{self, GorillaShape, GORILLA_F32, GORILLA_F64};
use super::header::{serialize_payload, ts1_inspect, TIME_SERIES_HEADER_SIZE};
use super::sink::{BitCounter, BitSink};
use super::{TimeSeriesMode, TimeSeriesPayloadInfo, FLAG_WINDOW_REUSE};

/// Little-endian lanes of `B` bytes of `input` (the incomplete tail is not included).
fn lanes<const B: usize>(input: &[u8]) -> impl Iterator<Item = u64> + '_ {
    input.chunks_exact(B).map(read_lane::<B>)
}

/// Gorilla shape of a Gorilla mode.
fn gorilla_shape(mode: TimeSeriesMode) -> Option<GorillaShape> {
    match mode {
        TimeSeriesMode::GorillaF64 => Some(GORILLA_F64),
        TimeSeriesMode::GorillaF32 => Some(GORILLA_F32),
        TimeSeriesMode::RunDelta => None,
    }
}

/// Runs the mode's stream encoder over the lanes of `input` into `sink`.
fn encode_stream<S: BitSink>(input: &[u8], mode: TimeSeriesMode, sink: &mut S) {
    match mode {
        TimeSeriesMode::GorillaF64 => {
            gorilla::encode_xors(GORILLA_F64, lanes::<8>(input), true, sink)
        }
        TimeSeriesMode::GorillaF32 => {
            gorilla::encode_xors(GORILLA_F32, lanes::<4>(input), true, sink)
        }
        TimeSeriesMode::RunDelta => {}
    }
}

/// Lane width a mode uses on `input`.
fn lane_bytes_of(mode: TimeSeriesMode) -> usize {
    gorilla_shape(mode).map_or(8, |shape| shape.lane_bytes)
}

/// Header fields that depend only on `input` and the mode.
fn payload_info(input: &[u8], mode: TimeSeriesMode, stream_bits: u64) -> TimeSeriesPayloadInfo {
    let lane_bytes = lane_bytes_of(mode);
    let value_count = input.len() / lane_bytes;
    let first_bits = match lane_bytes {
        4 if value_count > 0 => read_lane::<4>(input),
        8 if value_count > 0 => read_lane::<8>(input),
        _ => 0,
    };
    TimeSeriesPayloadInfo {
        mode,
        lane_bytes: lane_bytes as u8,
        flags: if gorilla_shape(mode).is_some() {
            FLAG_WINDOW_REUSE
        } else {
            0
        },
        value_count,
        tail_len: input.len() % lane_bytes,
        first_bits,
        stream_bits,
    }
}

/// Encodes `input` as a TS1 payload with `mode`.
pub fn ts1_encode_with(input: &[u8], mode: TimeSeriesMode) -> AceResult<Vec<u8>> {
    let mut writer = BitWriter::with_capacity(input.len() * 8 + 64);
    encode_stream(input, mode, &mut writer);
    let (stream, stream_bits) = writer.finish();
    let info = payload_info(input, mode, stream_bits);
    let tail = &input[input.len() - info.tail_len..];
    serialize_payload(&info, &stream, tail)
}

/// Exact byte length of `ts1_encode_with(input, mode)`, computed without writing the stream.
///
/// Runs the same encoder against a bit counter, so it can never disagree with the encoder.
pub fn ts1_encoded_len(input: &[u8], mode: TimeSeriesMode) -> usize {
    let mut counter = BitCounter::default();
    encode_stream(input, mode, &mut counter);
    let info = payload_info(input, mode, counter.bits);
    TIME_SERIES_HEADER_SIZE + info.stream_bytes() + info.tail_len
}

/// Decodes a TS1 payload to exactly `expected_size` bytes.
///
/// The header is validated (including `decoded_len == expected_size`) before the output buffer
/// is allocated; the stream is read through a bounds-checked [`BitReader`].
pub fn ts1_decode(payload: &[u8], expected_size: usize) -> AceResult<Vec<u8>> {
    let info = ts1_inspect(payload)?;
    if info.decoded_len()? != expected_size {
        return Err(AceError::InvalidTimeSeries("TS1 decoded size mismatch"));
    }
    let stream_end = TIME_SERIES_HEADER_SIZE + info.stream_bytes();
    let mut reader = BitReader::new(
        &payload[TIME_SERIES_HEADER_SIZE..stream_end],
        info.stream_bits,
    )?;
    let mut out = Vec::with_capacity(expected_size);
    let lane_bytes = info.lane_bytes as usize;
    let mut push = |bits: u64| out.extend_from_slice(&bits.to_le_bytes()[..lane_bytes]);
    match gorilla_shape(info.mode) {
        Some(shape) => gorilla::decode_values(
            shape,
            info.first_bits,
            info.value_count,
            info.flags & FLAG_WINDOW_REUSE != 0,
            &mut reader,
            &mut push,
        )?,
        None => return Err(AceError::InvalidTimeSeries("unknown TS1 mode")),
    }
    out.extend_from_slice(&payload[stream_end..]);
    Ok(out)
}
