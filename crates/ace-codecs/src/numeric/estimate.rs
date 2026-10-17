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

/// The best base mode must need at least this many bits per value before Patched FOR is
/// considered: below it an exception list cannot repay its section byte and positions, and the
/// extra statistics pass is skipped on the most compressible blocks.
const PATCHED_MIN_BASE_BITS: u8 = 4;

/// `estimate_numeric` evaluates Patched FOR only at widths whose base mode is at most
/// `1 + 1/PATCHED_WIDTH_SLACK` times the best base size (here 25 %).
const PATCHED_WIDTH_SLACK: usize = 4;

/// Estimates the best NUM1 representation over every lane width and mode.
///
/// Ties are broken by smaller payload, then width rank (u32, u64, u16 — preserving pre-0.4.5
/// decisions), then mode id. Patched FOR (Format 1.5) is a candidate too, and wins only when it
/// is strictly smaller than the best base mode, so a block without outliers is encoded exactly
/// as before.
///
/// # Determinism
///
/// Exact sizes from integer arithmetic; ties are broken by the fixed key (size, width rank,
/// mode id), never by evaluation order.
pub fn estimate_numeric(input: &[u8]) -> Option<NumericEstimate> {
    let widths = [NumericWidth::U32, NumericWidth::U64, NumericWidth::U16];
    let bases: Vec<NumericEstimate> = widths
        .into_iter()
        .filter_map(|width| best_base_estimate(input, width))
        .collect();
    let best_base_bytes = bases.iter().map(|base| base.encoded_bytes).min()?;
    // Patched FOR only competes at lane widths whose base mode is within a quarter of the best
    // one: a width that is far worse as a base is not where the outliers are, and the extra
    // statistics pass is the cost of the feature on every block that needs 4 bits or more.
    let patched_limit = best_base_bytes.saturating_add(best_base_bytes / PATCHED_WIDTH_SLACK);
    bases
        .into_iter()
        .map(|base| {
            if base.encoded_bytes <= patched_limit {
                with_patched(input, base)
            } else {
                base
            }
        })
        .min_by_key(|estimate| {
            (
                estimate.encoded_bytes,
                width_rank(estimate.width),
                estimate.mode as u8,
            )
        })
}

/// Smallest of the three base modes for one width (`None` below two values).
fn best_base_estimate(input: &[u8], width: NumericWidth) -> Option<NumericEstimate> {
    estimate_every_mode(input, width)?
        .into_iter()
        .min_by_key(|estimate| (estimate.encoded_bytes, estimate.mode as u8))
}

/// `base`, or the Patched FOR estimate of the same width when the base needs at least
/// [`PATCHED_MIN_BASE_BITS`] bits per value and Patched is strictly smaller.
fn with_patched(input: &[u8], base: NumericEstimate) -> NumericEstimate {
    if base.bit_width >= PATCHED_MIN_BASE_BITS && crate::patched_worth_trying(input, base.width) {
        if let Some(patched) = crate::estimate_patched(input, base.width) {
            if patched.encoded_bytes < base.encoded_bytes {
                return patched;
            }
        }
    }
    base
}

/// Estimates the best mode for one caller-chosen lane width (`None` below two values).
///
/// Planner FAST passes the prefilter's width hint to avoid scanning the block three times.
///
/// The result is the smallest of the three base modes or, when the best base mode needs at
/// least `PATCHED_MIN_BASE_BITS` (4) bits per value and Patched FOR is strictly smaller, the Patched
/// estimate ([`crate::estimate_patched`]).
pub fn estimate_numeric_for_width(input: &[u8], width: NumericWidth) -> Option<NumericEstimate> {
    best_base_estimate(input, width).map(|base| with_patched(input, base))
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
            NumericMode::FrameOfReference | NumericMode::Patched => self.for_bits,
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
// `chunks_exact` is kept on purpose: this hot kernel stays codegen-identical in the 0.5.1
// hardening release (hot loops showed layout-sensitive timings in the 0.5.0 A/B); `as_chunks`
// lands in 0.6 with an interleaved A/B run. `unknown_lints`: clippy < 1.98 lacks the lint.
#[allow(unknown_lints, clippy::chunks_exact_to_as_chunks)]
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
