//! Run-lane analysis (ACE 0.5.0): integer lanes whose values rarely change (TS1 RunDelta).
//!
//! RunDelta stores one `(run, delta)` pair per change, so its size is driven by the change
//! rate of whole lane values. The prefilter samples the same windows as the float prefilter
//! ([`crate::lane_sample_windows`]) and keeps every lane width whose sampled values repeat
//! their predecessor at least [`MIN_EQUAL_RATIO`] of the time, or (ACE 0.6.1) at least
//! [`MIN_EQUAL_RATIO_SMALL_DELTA`] of the time while [`MIN_SMALL_DELTA_RATIO`] of the
//! transitions move by at most [`SMALL_DELTA_MAX`]: a slowly drifting integer or a value with
//! geometric step sizes, where the per-change gamma code beats a global bit width although a
//! fifth to a half of the samples change. The planner then counts the exact RunDelta size of
//! each kept width (`ace_cost::estimate_run_delta`) and the usual dominance rule decides.
//!
//! Lanes made of one repeated byte ("splats" such as `0x0000…` or `0x1111…`) are byte runs,
//! not numbers: RLE already encodes them optimally and the 0.4 planner handles them, so a
//! width whose sampled values are mostly splats ([`MAX_SPLAT_RATIO`]) is not admitted. This
//! keeps zero padding and run-heavy byte data in Format 1.3.

use ace_core::read_lane;

use crate::{lane_sample_windows, LANE_WINDOW_VALUES};

/// Lane widths (bytes) considered for RunDelta, widest first.
pub const RUN_LANE_BYTES: [u8; 3] = [8, 4, 2];
/// Minimum fraction of sampled values equal to their predecessor.
pub const MIN_EQUAL_RATIO: f32 = 0.8;
/// Minimum fraction of equal transitions when the changes are small (ACE 0.6.1).
pub const MIN_EQUAL_RATIO_SMALL_DELTA: f32 = 0.5;
/// Largest `|delta|` (in the lane ring) that counts as a small change.
pub const SMALL_DELTA_MAX: u64 = 255;
/// Minimum fraction of sampled transitions with `|delta| <= SMALL_DELTA_MAX` (equal ones count).
pub const MIN_SMALL_DELTA_RATIO: f32 = 0.95;
/// Maximum fraction of sampled values that are a single repeated byte.
pub const MAX_SPLAT_RATIO: f32 = 0.5;

/// Sampled change statistics of a block read as lanes of one width.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunProfile {
    /// Lane width in bytes (2, 4 or 8).
    pub lane_bytes: u8,
    /// Number of complete lane values in the block.
    pub value_count: usize,
    /// Fraction of sampled transitions where the value equals its predecessor.
    pub equal_ratio: f32,
    /// Fraction of sampled transitions that move by at most [`SMALL_DELTA_MAX`] (including the
    /// equal ones), measured in the lane ring: a wrap from `65535` to `0` is a delta of `+1`.
    pub small_delta_ratio: f32,
    /// Fraction of sampled values whose bytes are all equal (byte runs, zero padding).
    pub splat_ratio: f32,
}

/// Inherent methods of [`RunProfile`].
impl RunProfile {
    /// True when RunDelta should be considered: values mostly repeat (or repeat often and move
    /// by little when they change) and are not byte runs.
    pub fn admitted(&self) -> bool {
        let repeats = self.equal_ratio >= MIN_EQUAL_RATIO
            || (self.equal_ratio >= MIN_EQUAL_RATIO_SMALL_DELTA
                && self.small_delta_ratio >= MIN_SMALL_DELTA_RATIO);
        repeats && self.splat_ratio <= MAX_SPLAT_RATIO
    }
}

/// Result of [`run_prefilter`]: the admitted profile of every width in [`RUN_LANE_BYTES`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RunPrefilter {
    /// Admitted profiles, indexed like [`RUN_LANE_BYTES`].
    pub lanes: [Option<RunProfile>; 3],
}

/// Inherent methods of [`RunPrefilter`].
impl RunPrefilter {
    /// Admitted profiles, widest lane first.
    pub fn admitted(&self) -> impl Iterator<Item = RunProfile> + '_ {
        self.lanes.iter().flatten().copied()
    }

    /// True when at least one width was admitted.
    pub fn any(&self) -> bool {
        self.lanes.iter().any(Option::is_some)
    }
}

/// Computes the [`RunProfile`] of `input` read as `lane_bytes` lanes (`None` if too short or
/// the width is unsupported).
pub fn run_profile(input: &[u8], lane_bytes: u8) -> Option<RunProfile> {
    match lane_bytes {
        2 => run_profile_lane::<2>(input),
        4 => run_profile_lane::<4>(input),
        8 => run_profile_lane::<8>(input),
        _ => None,
    }
}

/// Width-monomorphized body of [`run_profile`] (`B` = bytes per lane).
fn run_profile_lane<const B: usize>(input: &[u8]) -> Option<RunProfile> {
    let value_count = input.len() / B;
    if value_count < LANE_WINDOW_VALUES {
        return None;
    }
    let mut transitions = 0u32;
    let mut equal = 0u32;
    let mut small = 0u32;
    let mut values = 0u32;
    let mut splats = 0u32;
    for window in lane_sample_windows(value_count) {
        let mut previous: Option<u64> = None;
        for index in window {
            let bytes = &input[index * B..index * B + B];
            let value = read_lane::<B>(bytes);
            values += 1;
            splats += u32::from(bytes.iter().all(|&b| b == bytes[0]));
            if let Some(previous) = previous {
                transitions += 1;
                equal += u32::from(previous == value);
                small += u32::from(ring_distance::<B>(previous, value) <= SMALL_DELTA_MAX);
            }
            previous = Some(value);
        }
    }
    Some(RunProfile {
        lane_bytes: B as u8,
        value_count,
        equal_ratio: if transitions == 0 {
            0.0
        } else {
            equal as f32 / transitions as f32
        },
        small_delta_ratio: if transitions == 0 {
            0.0
        } else {
            small as f32 / transitions as f32
        },
        splat_ratio: splats as f32 / values.max(1) as f32,
    })
}

/// `|current - previous|` in the ring of `B`-byte lanes (the shorter way round).
fn ring_distance<const B: usize>(previous: u64, current: u64) -> u64 {
    let forward = current.wrapping_sub(previous);
    let backward = previous.wrapping_sub(current);
    let (forward, backward) = if B >= 8 {
        (forward, backward)
    } else {
        let mask = (1u64 << (8 * B)) - 1;
        (forward & mask, backward & mask)
    };
    forward.min(backward)
}

/// Keeps every lane width whose profile is [`RunProfile::admitted`].
///
/// # Determinism
///
/// A pure function of `input` (integer counts, ratios evaluated in a fixed order); widths
/// are reported in the fixed order of `RUN_LANE_BYTES`.
pub fn run_prefilter(input: &[u8]) -> RunPrefilter {
    let mut prefilter = RunPrefilter::default();
    for (slot, lane_bytes) in prefilter.lanes.iter_mut().zip(RUN_LANE_BYTES) {
        *slot = run_profile(input, lane_bytes).filter(RunProfile::admitted);
    }
    prefilter
}
