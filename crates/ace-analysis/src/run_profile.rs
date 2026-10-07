//! Run-lane analysis (ACE 0.5.0): integer lanes whose values rarely change (TS1 RunDelta).
//!
//! RunDelta stores one `(run, delta)` pair per change, so its size is driven by the change
//! rate of whole lane values. The prefilter samples the same windows as the float prefilter
//! ([`crate::lane_sample_windows`]) and keeps every lane width whose sampled values repeat
//! their predecessor at least [`MIN_EQUAL_RATIO`] of the time. The planner then counts the
//! exact RunDelta size of each kept width (`ace_cost::estimate_run_delta`).

use ace_core::read_lane;

use crate::{lane_sample_windows, LANE_WINDOW_VALUES};

/// Lane widths (bytes) considered for RunDelta, widest first.
pub const RUN_LANE_BYTES: [u8; 3] = [8, 4, 2];
/// Minimum fraction of sampled values equal to their predecessor.
pub const MIN_EQUAL_RATIO: f32 = 0.8;

/// Sampled change statistics of a block read as lanes of one width.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunProfile {
    /// Lane width in bytes (2, 4 or 8).
    pub lane_bytes: u8,
    /// Number of complete lane values in the block.
    pub value_count: usize,
    /// Fraction of sampled transitions where the value equals its predecessor.
    pub equal_ratio: f32,
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
    for window in lane_sample_windows(value_count) {
        let mut previous: Option<u64> = None;
        for index in window {
            let value = read_lane::<B>(&input[index * B..index * B + B]);
            if let Some(previous) = previous {
                transitions += 1;
                equal += u32::from(previous == value);
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
    })
}

/// Keeps every lane width whose sampled equal ratio reaches [`MIN_EQUAL_RATIO`].
pub fn run_prefilter(input: &[u8]) -> RunPrefilter {
    let mut prefilter = RunPrefilter::default();
    for (slot, lane_bytes) in prefilter.lanes.iter_mut().zip(RUN_LANE_BYTES) {
        *slot = run_profile(input, lane_bytes).filter(|p| p.equal_ratio >= MIN_EQUAL_RATIO);
    }
    prefilter
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A u32 series changing every 200 values is admitted at every width that sees it.
    #[test]
    fn sparse_changes_are_admitted() {
        let data: Vec<u8> = (0..65_536u32)
            .flat_map(|i| (1_000 + i / 200 * 7).to_le_bytes())
            .collect();
        let prefilter = run_prefilter(&data);
        let widths: Vec<u8> = prefilter.admitted().map(|p| p.lane_bytes).collect();
        assert_eq!(widths, vec![8, 4]);
        assert!(prefilter.lanes[1].expect("u32").equal_ratio > 0.98);
    }

    /// A counter changes every value and is rejected.
    #[test]
    fn counters_are_rejected() {
        let data: Vec<u8> = (0..65_536u32).flat_map(u32::to_le_bytes).collect();
        assert!(!run_prefilter(&data).any());
    }

    /// Short inputs and unsupported widths produce no profile.
    #[test]
    fn short_or_unsupported() {
        assert_eq!(run_profile(&[0u8; 100], 8), None);
        assert_eq!(run_profile(&[0u8; 4096], 3), None);
        assert!(run_profile(&[0u8; 4096], 2).is_some());
    }
}
