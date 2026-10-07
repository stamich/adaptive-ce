//! Exact, allocation-free size estimation of NUM1 encodings.

use ace_bitpack::{bits_required_u64, Lane};
use ace_core::NumericWidth;

use super::lane_dispatch::{dispatch_lane, width_of};
use crate::{NumericMode, NUMERIC_HEADER_SIZE};

/// Size estimate of one `(width, mode)` NUM1 encoding.
///
/// The estimate is *exact*: `encoded_bytes` equals the length of the payload that
/// [`crate::numeric_encode_with`] produces, because bit packing has no per-value overhead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumericEstimate {
    /// Selected integer width.
    pub width: NumericWidth,
    /// Selected sub-codec mode.
    pub mode: NumericMode,
    /// Bit width used by the packed value stream.
    pub bit_width: u8,
    /// Complete encoded payload size including header and tail bytes.
    pub encoded_bytes: usize,
}

/// Estimates the best NUM1 representation over every lane width and mode.
///
/// Ties are broken by smaller payload, then width rank (u32, u64, u16 — preserving pre-0.4.5
/// decisions), then mode id.
pub fn estimate_numeric(input: &[u8]) -> Option<NumericEstimate> {
    [NumericWidth::U32, NumericWidth::U64, NumericWidth::U16]
        .into_iter()
        .filter_map(|width| estimate_numeric_for_width(input, width))
        .min_by_key(|estimate| {
            (
                estimate.encoded_bytes,
                width_rank(estimate.width),
                estimate.mode as u8,
            )
        })
}

/// Estimates the best mode for one caller-chosen lane width (`None` below two values).
///
/// Planner FAST passes the prefilter's width hint to avoid scanning the block three times.
pub fn estimate_numeric_for_width(input: &[u8], width: NumericWidth) -> Option<NumericEstimate> {
    estimate_every_mode(input, width)?
        .into_iter()
        .min_by_key(|estimate| (estimate.encoded_bytes, estimate.mode as u8))
}

/// Exact estimates of all three modes for one width, in [`NumericMode::ALL`] order.
pub fn estimate_every_mode(input: &[u8], width: NumericWidth) -> Option<[NumericEstimate; 3]> {
    if input.len() < width.bytes() * 2 {
        return None;
    }
    let stats = dispatch_lane!(width, lane_stats(input));
    Some(NumericMode::ALL.map(|mode| stats.estimate(mode, input.len())))
}

/// Single-pass statistics from which the exact size of every [`NumericMode`] follows.
struct LaneStats {
    /// Lane width the statistics were computed for.
    width: NumericWidth,
    /// Number of complete values.
    count: usize,
    /// Bits needed by the largest `v - min(v)` offset (FOR).
    for_bits: u8,
    /// Bits needed by the largest ZigZag delta (Delta).
    delta_bits: u8,
    /// Bits needed by the largest ZigZag delta-of-delta (DoD).
    dod_bits: u8,
}

/// Inherent methods of [`LaneStats`].
impl LaneStats {
    /// Exact payload size for `mode`, given the full input length.
    fn estimate(&self, mode: NumericMode, input_len: usize) -> NumericEstimate {
        let bit_width = match mode {
            NumericMode::FrameOfReference => self.for_bits,
            NumericMode::Delta => self.delta_bits,
            NumericMode::DeltaOfDelta => self.dod_bits,
        };
        let packed_bits = mode
            .stream_len(self.count)
            .saturating_mul(bit_width as usize);
        NumericEstimate {
            width: self.width,
            mode,
            bit_width,
            encoded_bytes: NUMERIC_HEADER_SIZE
                + packed_bits.div_ceil(8)
                + input_len % self.width.bytes(),
        }
    }
}

/// Computes [`LaneStats`] in one pass without allocating.
///
/// Deltas live in the lane ring, so a wrapping 16-bit counter has delta `+1` at the wrap. The
/// bit widths are accumulated with OR (same highest bit as a running maximum, fewer branches).
fn lane_stats<T: Lane>(input: &[u8]) -> LaneStats {
    let mut lanes = input.chunks_exact(T::BYTES).map(T::read_le);
    let Some(first) = lanes.next() else {
        return LaneStats {
            width: width_of::<T>(),
            count: 0,
            for_bits: 0,
            delta_bits: 0,
            dod_bits: 0,
        };
    };
    let (mut min, mut max) = (first, first);
    let (mut delta_or, mut dod_or) = (0u64, 0u64);
    let (mut previous, mut previous_delta) = (first, None::<T>);
    let mut count = 1usize;
    for value in lanes {
        min = min.min(value);
        max = max.max(value);
        let delta = value.wrapping_sub(previous);
        delta_or |= T::zigzag(delta.to_signed()).to_u64();
        if let Some(last) = previous_delta {
            dod_or |= T::zigzag(delta.wrapping_sub(last).to_signed()).to_u64();
        }
        previous_delta = Some(delta);
        previous = value;
        count += 1;
    }
    LaneStats {
        width: width_of::<T>(),
        count,
        for_bits: bits_required_u64(max.wrapping_sub(min).to_u64()),
        delta_bits: bits_required_u64(delta_or),
        dod_bits: bits_required_u64(dod_or),
    }
}

/// Deterministic width tie-break: u32 over u64 over u16 when encoded sizes match.
fn width_rank(width: NumericWidth) -> u8 {
    match width {
        NumericWidth::U32 => 0,
        NumericWidth::U64 => 1,
        NumericWidth::U16 => 2,
    }
}
