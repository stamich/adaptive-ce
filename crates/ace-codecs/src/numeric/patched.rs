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

/// Values per chunk of the outlier pre-test.
const SPREAD_CHUNK: usize = 16;
/// A source needs chunks whose bit widths differ by at least this much before Patched FOR can
/// repay an exception (a position costs `bits(len - 1)` bits plus the high part).
const MIN_SPREAD_BITS: u32 = 8;

/// Cheap, vectorisable pre-test for the Patched estimate: `true` when the block might contain
/// outliers, i.e. in some source (FOR offsets, ZigZag deltas or delta-of-deltas) the widest
/// 16-value chunk needs at least [`MIN_SPREAD_BITS`] more bits than the narrowest. Blocks with
/// evenly wide values (counters, timestamps, noise) skip the histogram pass. Blocks of fewer
/// than four chunks are always tried. Only [`estimate_numeric`](crate::estimate_numeric) uses it;
/// [`estimate_patched`] and the encoder are unaffected.
pub(crate) fn patched_worth_trying(input: &[u8], width: NumericWidth) -> bool {
    dispatch_lane!(width, spread_pretest(input))
}

/// Width-monomorphized body of [`patched_worth_trying`].
// `chunks_exact` mirrors the other NUM1 kernels (the width is a trait constant, not a literal).
// `unknown_lints`: clippy < 1.98 lacks the lint.
#[allow(unknown_lints, clippy::chunks_exact_to_as_chunks)]
fn spread_pretest<T: Lane>(input: &[u8]) -> bool {
    let count = input.len() / T::BYTES;
    if count < 4 * SPREAD_CHUNK {
        return true;
    }
    let Some(minimum) = input.chunks_exact(T::BYTES).map(T::read_le).min() else {
        return true;
    };
    let (mut narrowest, mut widest) = ([u32::MAX; 3], [0u32; 3]);
    let mut all = input.chunks_exact(T::BYTES).map(T::read_le);
    let (Some(first), Some(second)) = (all.next(), all.next()) else {
        return true;
    };
    let mut previous = second;
    let mut previous_delta = second.wrapping_sub(first);
    let mut lanes = all;
    let mut chunk_index = 0usize;
    loop {
        let mut or = [0u64; 3];
        let mut taken = 0usize;
        for value in lanes.by_ref().take(SPREAD_CHUNK) {
            let delta = value.wrapping_sub(previous);
            let dod = delta.wrapping_sub(previous_delta);
            or[0] |= value.wrapping_sub(minimum).to_u64();
            or[1] |= T::zigzag(delta.to_signed()).to_u64();
            or[2] |= T::zigzag(dod.to_signed()).to_u64();
            previous_delta = delta;
            previous = value;
            taken += 1;
        }
        if taken < SPREAD_CHUNK {
            break;
        }
        for source in 0..3 {
            let bits = u32::from(bits_required_u64(or[source]));
            narrowest[source] = narrowest[source].min(bits);
            widest[source] = widest[source].max(bits);
        }
        chunk_index += 1;
    }
    chunk_index < 4
        || (0..3).any(|source| widest[source].saturating_sub(narrowest[source]) >= MIN_SPREAD_BITS)
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
    let plan = dispatch_lane!(width, plan_patched(input))?;
    Some(NumericEstimate {
        width,
        mode: NumericMode::Patched,
        bit_width: plan.bit_width,
        encoded_bytes: plan.bytes,
    })
}

/// Statistics pass and search over `(source, b)`; `None` below two values.
// `chunks_exact` mirrors the other NUM1 kernels (the width is a trait constant, not a literal).
// `unknown_lints`: clippy < 1.98 lacks the lint.
#[allow(unknown_lints, clippy::chunks_exact_to_as_chunks)]
fn plan_patched<T: Lane>(input: &[u8]) -> Option<PatchPlan> {
    let count = input.len() / T::BYTES;
    if count < 2 {
        return None;
    }
    let tail = input.len() % T::BYTES;
    let mut lanes = input.chunks_exact(T::BYTES).map(T::read_le);
    let first = lanes.next()?;
    let minimum = input.chunks_exact(T::BYTES).map(T::read_le).min()?;

    // hist[source][bits] = number of stream values of that source needing `bits` bits.
    let mut hist = [[0u32; 65]; 3];
    hist[0][usize::from(bits_required_u64(first.wrapping_sub(minimum).to_u64()))] += 1;
    let (mut previous, mut previous_delta) = (first, None::<T>);
    for value in lanes {
        hist[0][usize::from(bits_required_u64(value.wrapping_sub(minimum).to_u64()))] += 1;
        let delta = value.wrapping_sub(previous);
        hist[1][usize::from(bits_required_u64(T::zigzag(delta.to_signed()).to_u64()))] += 1;
        if let Some(last) = previous_delta {
            let dod = delta.wrapping_sub(last);
            hist[2][usize::from(bits_required_u64(T::zigzag(dod.to_signed()).to_u64()))] += 1;
        }
        previous_delta = Some(delta);
        previous = value;
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

/// Encodes `input` as the best Patched FOR payload for lane width `width`.
///
/// # Errors
///
/// [`ace_core::AceError::Malformed`] below two complete values; bit-packing and serialization
/// errors are propagated (internal invariants).
pub(super) fn encode_patched<T: Lane>(input: &[u8]) -> AceResult<Vec<u8>> {
    let plan = plan_patched::<T>(input).ok_or(ace_core::AceError::Malformed(
        "numeric codec requires at least two integer values",
    ))?;
    let values = read_lanes::<T>(input);
    let tail = &input[values.len() * T::BYTES..];
    let (first, first_delta, stream) = base_stream(&values, plan.source);

    let mask = if plan.bit_width >= 64 {
        u64::MAX
    } else {
        (1u64 << plan.bit_width) - 1
    };
    let mut lows = Vec::with_capacity(stream.len());
    let (mut positions, mut highs) = (Vec::<u64>::new(), Vec::<T>::new());
    for (index, value) in stream.iter().enumerate() {
        let value = value.to_u64();
        lows.push(T::from_u64_truncating(value & mask));
        let high = if plan.bit_width >= 64 {
            0
        } else {
            value >> plan.bit_width
        };
        if high != 0 {
            positions.push(index as u64);
            highs.push(T::from_u64_truncating(high));
        }
    }
    debug_assert_eq!(positions.len(), plan.exceptions);

    let packed = pack(&lows, plan.bit_width)?;
    let mut section = Vec::new();
    if !positions.is_empty() {
        let position_bits = NumericPatch::position_bits(stream.len());
        section.push(plan.high_bits);
        section.extend_from_slice(&pack(&positions, position_bits)?);
        section.extend_from_slice(&pack(&highs, plan.high_bits)?);
    }
    let info = NumericPayloadInfo {
        width: width_of::<T>(),
        mode: NumericMode::Patched,
        bit_width: plan.bit_width,
        value_count: values.len(),
        tail_bytes: tail.len(),
        packed_bytes: packed.len(),
        first_value: first.to_u64(),
        first_delta,
        patch: Some(NumericPatch {
            source: plan.source,
            exception_count: positions.len(),
            high_bits: if positions.is_empty() {
                0
            } else {
                plan.high_bits
            },
            section_bytes: section.len(),
        }),
    };
    let payload = numeric_serialize_patched_payload(&info, &packed, &section, tail)?;
    debug_assert_eq!(payload.len(), plan.bytes);
    Ok(payload)
}
