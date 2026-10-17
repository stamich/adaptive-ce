//! Patched FOR (NUM1 mode 4, Format 1.5): planning, size estimation and encoding.
//!
//! A patched payload stores the stream of a base mode (FOR offsets, ZigZag deltas or ZigZag
//! delta-of-deltas) at a bit width `b` that fits most values. A value of the stream is split
//! into `low = x & (2^b - 1)`, which stays in the packed slot, and `high = x >> b`; the few
//! values with `high != 0` are *exceptions* and list their position and high part in a side
//! section. One rare outlier therefore costs `position + high bits` instead of widening every
//! slot: the `monotonic-outliers` workload (step 1 with a jump every 1024 values) stores 2-bit
//! deltas plus one 17-bit exception per 1024 values instead of 17-bit deltas.
//!
//! The size is exact and allocation-free to compute: a histogram of the bit length of every
//! stream value gives, for each candidate `b`, the number of exceptions and the high-part width,
//! and the positions are bit-packed at `bits(stream length - 1)` bits (the section layout is in
//! the module documentation of the header).

use ace_bitpack::{bits_required_u64, pack, read_lanes, Lane};
use ace_core::{AceResult, NumericWidth};

use super::encode::base_stream;
use super::header::numeric_serialize_patched_payload;
use super::lane_dispatch::{dispatch_lane, width_of};
use crate::{NumericEstimate, NumericMode, NumericPatch, NumericPayloadInfo, NUMERIC_HEADER_SIZE};

/// Sample windows of the Patched FOR pre-test.
const SAMPLE_WINDOWS: usize = 64;
/// Stream values per sample window (two more values precede each window so that the first
/// delta and delta-of-delta need no state from the previous one).
const WINDOW_VALUES: usize = 64;
/// A predicted gain below `base / SAMPLE_MARGIN_DIVISOR` is not worth the statistics pass.
const SAMPLE_MARGIN_DIVISOR: usize = 64;

/// Cheap pre-test for the Patched estimate: `false` when a sample of the block predicts that no
/// Patched FOR payload of this width would be noticeably smaller than `base_bytes` (the best
/// base-mode size), so the full statistics pass is skipped.
///
/// The sample is 64 windows of 64 consecutive values spread evenly over the block, about 6 % of
/// the values of a 64 Ki-value block; it yields the same bit-length histogram as
/// [`estimate_patched`] does for the whole block. The exception count of every packed width is
/// scaled to the block (at least one while a width is below the widest value), the widest value
/// of each source is the exact one (`widest`), and the block is tried unless the best predicted
/// size is within `1/64` of `base_bytes` or larger. Blocks too short for 64 windows are always
/// tried. The pre-test only decides whether to look; the estimate and the encoder stay exact, and
/// the result is a pure function of the block.
pub(crate) fn patched_worth_trying(
    input: &[u8],
    width: NumericWidth,
    minimum: u64,
    base_bytes: usize,
    widest: [u8; 3],
) -> bool {
    dispatch_lane!(width, sample_pretest(input, minimum, base_bytes, widest))
}

/// Width-monomorphized body of [`patched_worth_trying`].
fn sample_pretest<T: Lane>(input: &[u8], minimum: u64, base_bytes: usize, widest: [u8; 3]) -> bool {
    let count = input.len() / T::BYTES;
    let span = SAMPLE_WINDOWS * (WINDOW_VALUES + 2);
    if count < 2 * span {
        return true;
    }
    let minimum = T::from_u64_truncating(minimum);
    // sample[source][bits] = sampled stream values of that source needing `bits` bits.
    let mut sample = [[0usize; 65]; 3];
    let last_start = count - (WINDOW_VALUES + 2);
    for window in 0..SAMPLE_WINDOWS {
        let start = last_start * window / (SAMPLE_WINDOWS - 1);
        let bytes = &input[start * T::BYTES..(start + WINDOW_VALUES + 2) * T::BYTES];
        let (current, behind) = (&bytes[2 * T::BYTES..], &bytes[T::BYTES..]);
        for ((now, one), two) in current
            .chunks_exact(T::BYTES)
            .zip(behind.chunks_exact(T::BYTES))
            .zip(bytes.chunks_exact(T::BYTES))
        {
            let (value, one, two) = (T::read_le(now), T::read_le(one), T::read_le(two));
            let delta = value.wrapping_sub(one);
            let dod = delta.wrapping_sub(one.wrapping_sub(two));
            let bits = |x: u64| usize::from(bits_required_u64(x));
            sample[0][bits(value.wrapping_sub(minimum).to_u64())] += 1;
            sample[1][bits(T::zigzag(delta.to_signed()).to_u64())] += 1;
            sample[2][bits(T::zigzag(dod.to_signed()).to_u64())] += 1;
        }
    }
    let sampled = SAMPLE_WINDOWS * WINDOW_VALUES;
    let tail = input.len() % T::BYTES;
    let threshold = base_bytes - base_bytes / SAMPLE_MARGIN_DIVISOR;
    NumericMode::PATCH_SOURCES
        .into_iter()
        .zip(sample)
        .zip(widest)
        .any(|((source, histogram), widest)| {
            let widest = usize::from(widest);
            let stream_len = source.stream_len(count);
            let position_bits = usize::from(NumericPatch::position_bits(stream_len));
            let mut over = sampled;
            (0..=widest).any(|bit_width| {
                over -= histogram[bit_width];
                let exceptions = if bit_width >= widest {
                    0
                } else {
                    (over * stream_len).div_ceil(sampled).max(1)
                };
                let section = if exceptions == 0 {
                    0
                } else {
                    1 + (exceptions * position_bits).div_ceil(8)
                        + (exceptions * (widest - bit_width)).div_ceil(8)
                };
                NUMERIC_HEADER_SIZE + (stream_len * bit_width).div_ceil(8) + section + tail
                    < threshold
            })
        })
}

/// The Patched FOR configuration chosen for one block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PatchPlan {
    /// Base mode whose stream is patched.
    source: NumericMode,
    /// Packed width `b` of the low part of every value.
    bit_width: u8,
    /// Bits of each exception high part (`0` without exceptions).
    high_bits: u8,
    /// Number of exceptions.
    exceptions: usize,
    /// Exact size of the whole payload.
    bytes: usize,
}

/// Exact size of the best Patched FOR payload of `input` for one lane width.
///
/// The candidate set is every base mode (FOR, Delta, DoD) times every packed width `b` from 0 to
/// the width of the largest stream value; ties are broken by (size, base mode id, `b`), never by
/// evaluation order. A block without outliers gets `b` = the full width and no exceptions, whose
/// size equals the base mode's, so Patched wins over a base mode only when it is strictly
/// smaller. `None` below two values.
pub fn estimate_patched(input: &[u8], width: NumericWidth) -> Option<NumericEstimate> {
    patched_estimate_with_minimum(input, width, dispatch_lane!(width, smallest_value(input))?)
}

/// [`estimate_patched`] for a caller that already knows the smallest value of the block.
pub(crate) fn patched_estimate_with_minimum(
    input: &[u8],
    width: NumericWidth,
    minimum: u64,
) -> Option<NumericEstimate> {
    let plan = dispatch_lane!(width, plan_patched(input, minimum))?;
    Some(NumericEstimate {
        width,
        mode: NumericMode::Patched,
        bit_width: plan.bit_width,
        encoded_bytes: plan.bytes,
        patch_source: Some(plan.source),
    })
}

/// Smallest lane value of `input` (`None` below two values).
// `chunks_exact` mirrors the other NUM1 kernels (the width is a trait constant, not a literal).
// `unknown_lints`: clippy < 1.98 lacks the lint.
#[allow(unknown_lints, clippy::chunks_exact_to_as_chunks)]
fn smallest_value<T: Lane>(input: &[u8]) -> Option<u64> {
    if input.len() < 2 * T::BYTES {
        return None;
    }
    input
        .chunks_exact(T::BYTES)
        .map(T::read_le)
        .min()
        .map(Lane::to_u64)
}

/// Statistics pass and search over `(source, b)`; `None` below two values.
///
/// `minimum` is the smallest value of the block (the frame of reference).
// `chunks_exact` mirrors the other NUM1 kernels (the width is a trait constant, not a literal).
// `unknown_lints`: clippy < 1.98 lacks the lint.
#[allow(unknown_lints, clippy::chunks_exact_to_as_chunks)]
fn plan_patched<T: Lane>(input: &[u8], minimum: u64) -> Option<PatchPlan> {
    let count = input.len() / T::BYTES;
    if count < 2 {
        return None;
    }
    let tail = input.len() % T::BYTES;
    let minimum = T::from_u64_truncating(minimum);
    let (first, second) = (T::read_le(input), T::read_le(&input[T::BYTES..]));

    // hist[source][bits] = number of stream values of that source needing `bits` bits. The
    // arrays hold 128 entries so that masking the index removes every bounds check.
    let mut hist = [[0u32; 128]; 3];
    let bin = |value: u64| usize::from(bits_required_u64(value)) & 127;
    let first_delta = second.wrapping_sub(first);
    hist[0][bin(first.wrapping_sub(minimum).to_u64())] += 1;
    hist[0][bin(second.wrapping_sub(minimum).to_u64())] += 1;
    hist[1][bin(T::zigzag(first_delta.to_signed()).to_u64())] += 1;
    // Value `i >= 2` needs the two values behind it; three aligned views of the same bytes.
    let (current, behind) = (&input[2 * T::BYTES..], &input[T::BYTES..]);
    for ((now, one), two) in current
        .chunks_exact(T::BYTES)
        .zip(behind.chunks_exact(T::BYTES))
        .zip(input.chunks_exact(T::BYTES))
    {
        let (value, one, two) = (T::read_le(now), T::read_le(one), T::read_le(two));
        let delta = value.wrapping_sub(one);
        let dod = delta.wrapping_sub(one.wrapping_sub(two));
        hist[0][bin(value.wrapping_sub(minimum).to_u64())] += 1;
        hist[1][bin(T::zigzag(delta.to_signed()).to_u64())] += 1;
        hist[2][bin(T::zigzag(dod.to_signed()).to_u64())] += 1;
    }

    let mut best: Option<PatchPlan> = None;
    for (source, histogram) in NumericMode::PATCH_SOURCES.into_iter().zip(hist) {
        let stream_len = source.stream_len(count);
        if stream_len == 0 {
            continue;
        }
        let widest = histogram.iter().rposition(|&n| n > 0).unwrap_or(0);
        let position_bits = usize::from(NumericPatch::position_bits(stream_len));
        let mut exceptions = stream_len;
        for (bit_width, &at_width) in histogram.iter().enumerate().take(widest + 1) {
            exceptions -= at_width as usize;
            let high_bits = widest - bit_width;
            let section = if exceptions == 0 {
                0
            } else {
                1 + (exceptions * position_bits).div_ceil(8) + (exceptions * high_bits).div_ceil(8)
            };
            let bytes = NUMERIC_HEADER_SIZE + (stream_len * bit_width).div_ceil(8) + section + tail;
            if best.is_none_or(|current| bytes < current.bytes) {
                best = Some(PatchPlan {
                    source,
                    bit_width: bit_width as u8,
                    high_bits: if exceptions == 0 { 0 } else { high_bits as u8 },
                    exceptions,
                    bytes,
                });
            }
        }
    }
    best
}

/// Encodes `input` as a Patched FOR payload for lane type `T`.
///
/// Without `hint` the best `(source, packed width)` is searched like [`estimate_patched`] does.
/// With `hint` (the source and packed width an estimate of this same block chose) the statistics
/// pass is skipped: the exceptions are whatever does not fit the hinted width, so the payload is
/// valid for any hint and is the estimated one when the hint came from this block.
///
/// # Errors
///
/// [`ace_core::AceError::Malformed`] below two complete values; bit-packing and serialization
/// errors are propagated (internal invariants).
pub(super) fn encode_patched<T: Lane>(
    input: &[u8],
    hint: Option<(NumericMode, u8)>,
) -> AceResult<Vec<u8>> {
    let too_short =
        || ace_core::AceError::Malformed("numeric codec requires at least two integer values");
    let (source, bit_width, planned) = match hint {
        Some((source, bit_width)) if input.len() >= 2 * T::BYTES => (source, bit_width, None),
        _ => {
            let minimum = smallest_value::<T>(input).ok_or_else(too_short)?;
            let plan = plan_patched::<T>(input, minimum).ok_or_else(too_short)?;
            (plan.source, plan.bit_width, Some(plan))
        }
    };
    let values = read_lanes::<T>(input);
    let tail = &input[values.len() * T::BYTES..];
    let (first, first_delta, stream) = base_stream(&values, source);

    let mask = if bit_width >= 64 {
        u64::MAX
    } else {
        (1u64 << bit_width) - 1
    };
    let mut lows = Vec::with_capacity(stream.len());
    let (mut positions, mut highs) = (Vec::<u64>::new(), Vec::<T>::new());
    for (index, value) in stream.iter().enumerate() {
        let value = value.to_u64();
        lows.push(T::from_u64_truncating(value & mask));
        let high = if bit_width >= 64 {
            0
        } else {
            value >> bit_width
        };
        if high != 0 {
            positions.push(index as u64);
            highs.push(T::from_u64_truncating(high));
        }
    }
    // The widest exception fixes the width of every high part.
    let high_bits = highs
        .iter()
        .map(|high| bits_required_u64(high.to_u64()))
        .max()
        .unwrap_or(0);
    if let Some(plan) = planned {
        debug_assert_eq!(positions.len(), plan.exceptions);
        debug_assert_eq!(high_bits, plan.high_bits);
    }

    let packed = pack(&lows, bit_width)?;
    let mut section = Vec::new();
    if !positions.is_empty() {
        let position_bits = NumericPatch::position_bits(stream.len());
        section.push(high_bits);
        section.extend_from_slice(&pack(&positions, position_bits)?);
        section.extend_from_slice(&pack(&highs, high_bits)?);
    }
    let info = NumericPayloadInfo {
        width: width_of::<T>(),
        mode: NumericMode::Patched,
        bit_width,
        value_count: values.len(),
        tail_bytes: tail.len(),
        packed_bytes: packed.len(),
        first_value: first.to_u64(),
        first_delta,
        patch: Some(NumericPatch {
            source,
            exception_count: positions.len(),
            high_bits,
            section_bytes: section.len(),
        }),
    };
    let payload = numeric_serialize_patched_payload(&info, &packed, &section, tail)?;
    if let Some(plan) = planned {
        debug_assert_eq!(payload.len(), plan.bytes);
    }
    Ok(payload)
}
