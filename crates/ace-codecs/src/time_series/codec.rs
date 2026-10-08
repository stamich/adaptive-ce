//! Public TS1 entry points: encode with an explicit layout, exact encoded size, decode.

use ace_bitpack::{BitReader, BitWriter};
use ace_core::{read_lane, AceError, AceResult};

use super::gorilla::{self, GorillaShape, GORILLA_F32, GORILLA_F64};
use super::header::{serialize_payload, ts1_inspect, TIME_SERIES_HEADER_SIZE};
use super::run_delta;
use super::sink::{BitCounter, BitSink};
use super::{TimeSeriesMode, TimeSeriesPayloadInfo, FLAG_WINDOW_REUSE};

/// Mode plus lane width: everything an encoder needs to know about the wire layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimeSeriesLayout {
    /// Encoding mode.
    pub mode: TimeSeriesMode,
    /// Bytes per lane (must be accepted by `mode`).
    pub lane_bytes: u8,
}

/// Inherent methods of [`TimeSeriesLayout`].
impl TimeSeriesLayout {
    /// Gorilla over IEEE-754 binary64.
    pub const GORILLA_F64: Self = Self {
        mode: TimeSeriesMode::GorillaF64,
        lane_bytes: 8,
    };
    /// Gorilla over IEEE-754 binary32.
    pub const GORILLA_F32: Self = Self {
        mode: TimeSeriesMode::GorillaF32,
        lane_bytes: 4,
    };

    /// RunDelta over lanes of `lane_bytes` (2, 4 or 8) bytes.
    pub const fn run_delta(lane_bytes: u8) -> Self {
        Self {
            mode: TimeSeriesMode::RunDelta,
            lane_bytes,
        }
    }

    /// Every valid layout, in wire order.
    pub const ALL: [Self; 5] = [
        Self::GORILLA_F64,
        Self::GORILLA_F32,
        Self::run_delta(2),
        Self::run_delta(4),
        Self::run_delta(8),
    ];

    /// Stable label (`gorilla_f64`, `run_delta_u32`, …) for telemetry and `ace inspect`.
    pub fn label(self) -> String {
        match self.mode {
            TimeSeriesMode::RunDelta => format!("run_delta_u{}", self.lane_bytes as u32 * 8),
            mode => mode.label().to_string(),
        }
    }
}

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

/// Runs the layout's stream encoder over the lanes of `input` into `sink`.
fn encode_stream<S: BitSink>(input: &[u8], layout: TimeSeriesLayout, sink: &mut S) {
    let lane_bits = layout.lane_bytes as u32 * 8;
    match (layout.mode, layout.lane_bytes) {
        (TimeSeriesMode::GorillaF64, _) => {
            gorilla::encode_xors(GORILLA_F64, lanes::<8>(input), true, sink)
        }
        (TimeSeriesMode::GorillaF32, _) => {
            gorilla::encode_xors(GORILLA_F32, lanes::<4>(input), true, sink)
        }
        (TimeSeriesMode::RunDelta, 2) => run_delta::encode_runs(lanes::<2>(input), lane_bits, sink),
        (TimeSeriesMode::RunDelta, 4) => run_delta::encode_runs(lanes::<4>(input), lane_bits, sink),
        (TimeSeriesMode::RunDelta, _) => run_delta::encode_runs(lanes::<8>(input), lane_bits, sink),
    }
}

/// Validates the layout and builds the header fields that depend only on `input`.
fn payload_info(
    input: &[u8],
    layout: TimeSeriesLayout,
    stream_bits: u64,
) -> AceResult<TimeSeriesPayloadInfo> {
    if !layout.mode.accepts_lane_bytes(layout.lane_bytes) {
        return Err(AceError::InvalidTimeSeries(
            "lane width not valid for TS1 mode",
        ));
    }
    let lane_bytes = layout.lane_bytes as usize;
    let value_count = input.len() / lane_bytes;
    let first_bits = match (value_count > 0, lane_bytes) {
        (false, _) => 0,
        (true, 2) => read_lane::<2>(input),
        (true, 4) => read_lane::<4>(input),
        (true, _) => read_lane::<8>(input),
    };
    Ok(TimeSeriesPayloadInfo {
        mode: layout.mode,
        lane_bytes: layout.lane_bytes,
        flags: if gorilla_shape(layout.mode).is_some() {
            FLAG_WINDOW_REUSE
        } else {
            0
        },
        value_count,
        tail_len: input.len() % lane_bytes,
        first_bits,
        stream_bits,
    })
}

/// Encodes `input` as a TS1 payload with `layout`.
pub fn ts1_encode_with(input: &[u8], layout: TimeSeriesLayout) -> AceResult<Vec<u8>> {
    let mut writer = BitWriter::with_capacity(input.len() * 8 + 64);
    encode_stream(input, layout, &mut writer);
    let (stream, stream_bits) = writer.finish();
    let info = payload_info(input, layout, stream_bits)?;
    let tail = &input[input.len() - info.tail_len..];
    serialize_payload(&info, &stream, tail)
}

/// Exact byte length of `ts1_encode_with(input, layout)`, computed without writing a stream.
///
/// Runs the same encoder against a bit counter, so it can never disagree with the encoder.
pub fn ts1_encoded_len(input: &[u8], layout: TimeSeriesLayout) -> AceResult<usize> {
    let mut counter = BitCounter::default();
    encode_stream(input, layout, &mut counter);
    let info = payload_info(input, layout, counter.bits)?;
    Ok(TIME_SERIES_HEADER_SIZE + info.stream_bytes() + info.tail_len)
}

/// Exact number of stream bits `ts1_encode_with(input, layout)` would write (header and tail
/// excluded); used by the planner's sample estimator, which needs sub-byte precision.
pub fn ts1_stream_bits(input: &[u8], layout: TimeSeriesLayout) -> AceResult<u64> {
    let mut counter = BitCounter::default();
    encode_stream(input, layout, &mut counter);
    payload_info(input, layout, counter.bits).map(|info| info.stream_bits)
}

/// Encodes `input` with the smallest of the given layouts (ties: earlier layout wins).
pub fn ts1_encode_best(input: &[u8], layouts: &[TimeSeriesLayout]) -> AceResult<Vec<u8>> {
    let mut best: Option<(usize, TimeSeriesLayout)> = None;
    for &layout in layouts {
        let len = ts1_encoded_len(input, layout)?;
        if best.is_none_or(|(best_len, _)| len < best_len) {
            best = Some((len, layout));
        }
    }
    let (_, layout) = best.ok_or(AceError::InvalidTimeSeries("no TS1 layout requested"))?;
    ts1_encode_with(input, layout)
}

/// Encodes `input` with the smallest TS1 layout (used by the generic codec dispatcher).
pub fn ts1_encode(input: &[u8]) -> AceResult<Vec<u8>> {
    ts1_encode_best(input, &TimeSeriesLayout::ALL)
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
        None => run_delta::decode_runs(
            info.first_bits,
            info.value_count,
            lane_bytes as u32 * 8,
            &mut reader,
            &mut push,
        )?,
    }
    out.extend_from_slice(&payload[stream_end..]);
    Ok(out)
}
