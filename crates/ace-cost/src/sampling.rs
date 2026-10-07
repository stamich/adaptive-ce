//! Deterministic sample-window selection and adaptive Top-K widths for the verifier.

use std::ops::Range;

use ace_core::{CodecId, CompressionProfile};

/// Deterministic sampling policy used by the two-stage candidate verifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SamplePolicy {
    /// Base number of bytes in one stage-one verification sample.
    pub sample_bytes: usize,
    /// Maximum number of deterministic stage-one samples taken from one block.
    pub sample_count: usize,
    /// Nominal maximum number of candidates entering stage one before confidence widening.
    pub top_k: usize,
    /// Larger sample width used by the second verification stage.
    pub second_stage_bytes: usize,
    /// Number of stage-two sample windows.
    pub second_stage_count: usize,
    /// Number of candidates retained for stage two.
    pub second_stage_top_k: usize,
}

/// Inherent methods of [`SamplePolicy`].
impl SamplePolicy {
    /// Returns the default quality/speed verification budget for a public compression profile.
    pub fn for_profile(profile: CompressionProfile) -> Self {
        match profile {
            CompressionProfile::Fast => Self {
                sample_bytes: 4 * 1024,
                sample_count: 1,
                top_k: 2,
                second_stage_bytes: 8 * 1024,
                second_stage_count: 1,
                second_stage_top_k: 1,
            },
            CompressionProfile::Balanced => Self {
                sample_bytes: 8 * 1024,
                sample_count: 3,
                top_k: 4,
                second_stage_bytes: 32 * 1024,
                second_stage_count: 3,
                second_stage_top_k: 2,
            },
            CompressionProfile::Dense => Self {
                sample_bytes: 16 * 1024,
                sample_count: 4,
                top_k: 6,
                second_stage_bytes: 64 * 1024,
                second_stage_count: 3,
                second_stage_top_k: 3,
            },
        }
    }
}

/// Returns an adaptive stage-one Top-K width from the best estimate confidence.
///
/// Lower confidence widens the search. DENSE retains a larger quality floor even when
/// the estimator appears confident because the profile explicitly values compression ratio.
pub fn adaptive_top_k(
    profile: CompressionProfile,
    confidence: f32,
    nominal: usize,
    total: usize,
) -> usize {
    if total == 0 {
        return 0;
    }
    let k = match profile {
        CompressionProfile::Fast => {
            if confidence >= 0.95 {
                1
            } else {
                nominal.max(2)
            }
        }
        CompressionProfile::Balanced => {
            if confidence >= 0.95 {
                2
            } else if confidence >= 0.80 {
                3
            } else {
                nominal.max(4)
            }
        }
        CompressionProfile::Dense => {
            if confidence >= 0.95 {
                4
            } else if confidence >= 0.80 {
                5
            } else {
                nominal.max(6)
            }
        }
    };
    k.max(1).min(total)
}

/// Returns stable, non-random sample ranges for a block.
///
/// This helper is retained for non-codec-specific callers. Planner V3.1 uses
/// [`codec_sample_ranges`] so LZ can receive larger stratified windows.
pub fn deterministic_sample_ranges(len: usize, policy: SamplePolicy) -> Vec<Range<usize>> {
    stratified_ranges(len, policy.sample_bytes, policy.sample_count)
}

/// Returns deterministic sample ranges specialized for the candidate codec and verifier stage.
///
/// LZ receives wider windows than RAW/RLE because long-distance matches cannot be represented
/// faithfully by tiny samples. Each range is encoded independently; callers must never
/// concatenate disjoint samples because doing so would create artificial LZ matches.
pub fn codec_sample_ranges(
    len: usize,
    policy: SamplePolicy,
    codec: CodecId,
    stage: u8,
) -> Vec<Range<usize>> {
    let (mut width, mut count) = if stage <= 1 {
        (policy.sample_bytes, policy.sample_count)
    } else {
        (policy.second_stage_bytes, policy.second_stage_count)
    };
    if matches!(codec, CodecId::Lz) {
        width = width.max(if stage <= 1 { 32 * 1024 } else { 64 * 1024 });
        count = count.max(if stage <= 1 { 4 } else { 3 });
    }
    stratified_ranges(len, width, count)
}

/// Builds deterministic evenly distributed windows without overlapping the end of the block.
///
/// Shared by the stage-one/two sample verifier and the Hybrid-LZ micro-trials (the single
/// implementation of ACE's sampling geometry). A single window is centred; duplicates are
/// removed when windows would collapse onto the same start.
#[allow(clippy::single_range_in_vec_init)] // a one-window sample set is a legitimate result
pub fn stratified_ranges(
    len: usize,
    requested_width: usize,
    requested_count: usize,
) -> Vec<Range<usize>> {
    if len == 0 || requested_count == 0 {
        return Vec::new();
    }
    let width = requested_width.min(len).max(1);
    if width == len {
        return vec![0..len];
    }
    let max_start = len - width;
    let count = requested_count.max(1);
    if count == 1 {
        return vec![max_start / 2..max_start / 2 + width];
    }

    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let start = ((max_start as u128) * (i as u128) / ((count - 1) as u128)) as usize;
        let range = start..start + width;
        if out
            .last()
            .map(|r: &Range<usize>| r.start != range.start)
            .unwrap_or(true)
        {
            out.push(range);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ensures generic sampling never depends on randomness or process state.
    #[test]
    fn sample_ranges_are_stable() {
        let p = SamplePolicy {
            sample_bytes: 16,
            sample_count: 3,
            top_k: 2,
            second_stage_bytes: 32,
            second_stage_count: 2,
            second_stage_top_k: 1,
        };
        assert_eq!(
            deterministic_sample_ranges(100, p),
            vec![0..16, 42..58, 84..100]
        );
    }

    /// Ensures LZ verification gets larger deterministic windows than RAW verification.
    #[test]
    fn lz_sampling_uses_wider_strata() {
        let p = SamplePolicy::for_profile(CompressionProfile::Balanced);
        let raw = codec_sample_ranges(262_144, p, CodecId::Raw, 1);
        let lz = codec_sample_ranges(262_144, p, CodecId::Lz, 1);
        assert!(lz[0].len() >= 32 * 1024);
        assert!(lz.len() >= raw.len());
        assert_eq!(lz, codec_sample_ranges(262_144, p, CodecId::Lz, 1));
    }

    /// Verifies DENSE never collapses to a one-candidate search solely on estimator confidence.
    #[test]
    fn dense_has_quality_floor() {
        assert_eq!(adaptive_top_k(CompressionProfile::Dense, 0.99, 6, 20), 4);
    }
}
