//! RunDelta: integers that rarely change, stored as (run of zero deltas, delta) pairs.
//!
//! ```text
//! header.first_bits = v[0]
//! for every change:  gamma(run) gamma(zigzag(delta))     run = unchanged values before it
//! terminator:        gamma(run) gamma(0)                  run = unchanged values at the end
//! ```
//!
//! Deltas are taken modulo `2^lane_bits` and sign-extended before ZigZag, so every lane value
//! sequence is representable. A non-zero delta has `zigzag(delta) >= 1`, which makes
//! `zigzag = 0` an unambiguous terminator. Unlike NUM1 Delta there is no global bit width: a
//! rare large jump costs only its own bits.

use ace_bitpack::{unzigzag_u64, zigzag_i64, BitReader};
use ace_core::{AceError, AceResult};

use super::gamma::{put_gamma, read_gamma};
use super::sink::BitSink;

/// Mask of the low `lane_bits` bits.
fn lane_mask(lane_bits: u32) -> u64 {
    if lane_bits >= 64 {
        u64::MAX
    } else {
        (1u64 << lane_bits) - 1
    }
}

/// `current - previous` modulo `2^lane_bits`, sign-extended to `i64`.
fn signed_delta(previous: u64, current: u64, lane_bits: u32) -> i64 {
    let shift = 64 - lane_bits;
    ((current.wrapping_sub(previous) << shift) as i64) >> shift
}

/// Encodes `values[1..]` (the first value lives in the header) into `sink`.
pub(crate) fn encode_runs<S: BitSink>(
    values: impl Iterator<Item = u64>,
    lane_bits: u32,
    sink: &mut S,
) {
    let mut previous: Option<u64> = None;
    let mut run = 0u64;
    for value in values {
        let Some(last) = previous.replace(value) else {
            continue;
        };
        if value == last {
            run += 1;
            continue;
        }
        put_gamma(sink, run);
        put_gamma(sink, zigzag_i64(signed_delta(last, value, lane_bits)));
        run = 0;
    }
    if previous.is_some() {
        put_gamma(sink, run);
        put_gamma(sink, 0);
    }
}

/// Decodes `count` values (the first is `first_bits`) into `emit`; rejects runs or changes
/// beyond `count`, deltas wider than the lane, a missing terminator and trailing bits.
pub(crate) fn decode_runs(
    first_bits: u64,
    count: usize,
    lane_bits: u32,
    reader: &mut BitReader<'_>,
    mut emit: impl FnMut(u64),
) -> AceResult<()> {
    let malformed = || AceError::InvalidTimeSeries("truncated or invalid RunDelta stream");
    if count == 0 {
        return Ok(());
    }
    let mask = lane_mask(lane_bits);
    let mut value = first_bits;
    emit(value);
    let mut emitted = 1usize;
    loop {
        let run = read_gamma(reader).ok_or_else(malformed)?;
        let remaining = (count - emitted) as u64;
        if run > remaining {
            return Err(AceError::InvalidTimeSeries(
                "RunDelta run exceeds value count",
            ));
        }
        for _ in 0..run {
            emit(value);
        }
        emitted += run as usize;
        let zigzag = read_gamma(reader).ok_or_else(malformed)?;
        if zigzag == 0 {
            break;
        }
        if lane_bits < 64 && zigzag >> lane_bits != 0 {
            return Err(AceError::InvalidTimeSeries(
                "RunDelta delta exceeds lane width",
            ));
        }
        if emitted == count {
            return Err(AceError::InvalidTimeSeries(
                "RunDelta change beyond value count",
            ));
        }
        value = value.wrapping_add(unzigzag_u64(zigzag) as u64) & mask;
        emit(value);
        emitted += 1;
    }
    if emitted != count {
        return Err(AceError::InvalidTimeSeries("RunDelta value count mismatch"));
    }
    if reader.remaining() != 0 {
        return Err(AceError::InvalidTimeSeries(
            "trailing bits after RunDelta stream",
        ));
    }
    Ok(())
}
