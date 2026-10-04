//! Deterministic candidate-size and resource estimators used by the ACE 0.3 planner.
//!
//! ACE 0.3-buildfix4 keeps the cheap analytical estimator introduced in 0.3, but
//! adds profile-aware adaptive Top-K sizing and codec-specific deterministic sample
//! windows.  LZ candidates are verified with larger stratified windows because tiny
//! samples systematically under-represent long-range matchability.

use ace_core::{
    BlockProfile, CodecId, CompressionProfile, CostWeights, EntropyCodecId, LzMode,
    PhysicalCompressionPlan, PlanCost, TransformId,
};
use std::ops::Range;

/// Cheap deterministic estimate of one candidate compression plan.
#[derive(Debug, Clone)]
pub struct EstimatedCandidate {
    /// Candidate plan to which the estimate belongs.
    pub plan: PhysicalCompressionPlan,
    /// Estimated multidimensional resource cost.
    pub cost: PlanCost,
    /// Scalar cost used only for deterministic ranking; lower is better.
    pub score: u128,
    /// Confidence in `0.0..=1.0` based on how strongly block statistics support the estimate.
    pub confidence: f32,
}

/// Produces inexpensive estimates without encoding a complete block.
pub trait CandidateEstimator: Send + Sync {
    /// Estimates one candidate using a previously computed statistical block profile.
    fn estimate(
        &self,
        candidate: &PhysicalCompressionPlan,
        profile: &BlockProfile,
        compression_profile: CompressionProfile,
    ) -> EstimatedCandidate;
}

/// Default analytical estimator used by ACE 0.3-buildfix4.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultCandidateEstimator;

impl CandidateEstimator for DefaultCandidateEstimator {
    /// Predicts encoded size and resource cost from block statistics without executing codecs.
    fn estimate(
        &self,
        candidate: &PhysicalCompressionPlan,
        p: &BlockProfile,
        compression_profile: CompressionProfile,
    ) -> EstimatedCandidate {
        let input = p.size.max(1) as f64;
        let mut primary_ratio = 1.0f64;
        let mut confidence = 0.78f32;

        if candidate
            .decoding
            .transforms
            .iter()
            .any(|t| matches!(t, TransformId::DeltaByte))
        {
            let gain = (p.delta_score as f64).clamp(0.0, 0.85);
            primary_ratio *= 1.0 - gain * 0.55;
            confidence = confidence.max((0.60 + p.delta_score * 0.35).clamp(0.0, 0.98));
        }

        match candidate.decoding.codec {
            CodecId::Raw => {}
            CodecId::Rle => {
                let structural = p.run_score.max(p.zero_ratio).clamp(0.0, 1.0) as f64;
                primary_ratio *= (1.0 - structural * 0.90).clamp(0.015, 1.05);
                confidence =
                    confidence.max((0.62 + p.run_score.max(p.zero_ratio) * 0.36).clamp(0.0, 0.99));
            }
            CodecId::Lz => {
                let repetition = p.repetition_score.clamp(0.0, 1.0) as f64;
                let match_signal = (p.sampled_match_length / 48.0).clamp(0.0, 1.0) as f64;
                let signal = repetition.max(match_signal);
                let depth_bonus = match candidate.lz_mode {
                    Some(LzMode::Balanced) => 0.72,
                    _ => 0.62,
                };
                primary_ratio *= (1.0 - signal * depth_bonus).clamp(0.05, 1.04);
                // LZ confidence deliberately remains lower than pure entropy/RLE confidence:
                // small analyzer sketches cannot fully represent long-range matches.
                confidence = confidence.max((0.54 + signal as f32 * 0.34).clamp(0.0, 0.94));
            }
        }

        let entropy_factor = match candidate.decoding.entropy {
            EntropyCodecId::None => 1.0,
            EntropyCodecId::Huffman => {
                let h = (p.entropy_h0 as f64 / 8.0).clamp(0.02, 1.0);
                (0.04 + h * 0.96).clamp(0.04, 1.03)
            }
            EntropyCodecId::Rans | EntropyCodecId::Rans4x => {
                let h = (p.entropy_h0 as f64 / 8.0).clamp(0.02, 1.0);
                (0.025 + h * 0.94).clamp(0.03, 1.02)
            }
        };
        let metadata = match candidate.decoding.entropy {
            EntropyCodecId::None => 0u64,
            EntropyCodecId::Huffman => 260u64,
            EntropyCodecId::Rans => 516u64,
            EntropyCodecId::Rans4x => 2064u64,
        };
        let predicted_size =
            (input * primary_ratio * entropy_factor).round().max(1.0) as u64 + metadata;

        let codec_encode = match (candidate.decoding.codec, candidate.lz_mode) {
            (CodecId::Raw, _) => 1u64,
            (CodecId::Rle, _) => 2,
            (CodecId::Lz, Some(LzMode::Balanced)) => 9,
            (CodecId::Lz, _) => 4,
        };
        let codec_decode = match candidate.decoding.codec {
            CodecId::Raw => 1u64,
            CodecId::Rle => 2,
            CodecId::Lz => 3,
        };
        let entropy_encode = match candidate.decoding.entropy {
            EntropyCodecId::None => 0u64,
            EntropyCodecId::Huffman => 4,
            EntropyCodecId::Rans => 9,
            EntropyCodecId::Rans4x => 5,
        };
        let entropy_decode = match candidate.decoding.entropy {
            EntropyCodecId::None => 0u64,
            EntropyCodecId::Huffman => 6,
            EntropyCodecId::Rans => 5,
            EntropyCodecId::Rans4x => 3,
        };
        let transform = candidate.decoding.transforms.len() as u64;
        let n = p.size as u64;
        let cost = PlanCost {
            predicted_size_bytes: predicted_size,
            metadata_bytes: metadata,
            encode_units: n.saturating_mul(codec_encode + entropy_encode + transform),
            decode_units: n.saturating_mul(codec_decode + entropy_decode + transform),
            memory_bytes: n.saturating_mul(if matches!(candidate.decoding.codec, CodecId::Lz) {
                3
            } else {
                2
            }),
        };
        let score = CostModelV3.score(compression_profile, cost, p.size);
        EstimatedCandidate {
            plan: candidate.clone(),
            cost,
            score,
            confidence,
        }
    }
}

/// Deterministic ACE 0.3 multidimensional cost model.
#[derive(Debug, Default, Clone, Copy)]
pub struct CostModelV3;

impl CostModelV3 {
    /// Converts normalized size, CPU and memory estimates into a stable sortable scalar.
    pub fn score(&self, profile: CompressionProfile, cost: PlanCost, input_bytes: usize) -> u128 {
        let weights = CostWeights::for_profile(profile);
        let divisor = input_bytes.max(1) as u128;
        let size_ppm = (cost.predicted_size_bytes as u128).saturating_mul(1_000_000) / divisor;
        let encode_per_byte = cost.encode_units as u128 / divisor;
        let decode_per_byte = cost.decode_units as u128 / divisor;
        let memory_per_byte = cost.memory_bytes as u128 / divisor;
        let (encode_scale, decode_scale) = match profile {
            CompressionProfile::Fast => (34_000u128, 6_000u128),
            CompressionProfile::Balanced => (3_500u128, 1_300u128),
            CompressionProfile::Dense => (550u128, 300u128),
        };
        size_ppm
            .saturating_mul(weights.size as u128)
            .saturating_add(
                encode_per_byte
                    .saturating_mul(encode_scale)
                    .saturating_mul(weights.encode_cpu as u128),
            )
            .saturating_add(
                decode_per_byte
                    .saturating_mul(decode_scale)
                    .saturating_mul(weights.decode_cpu as u128),
            )
            .saturating_add(
                memory_per_byte
                    .saturating_mul(1_000)
                    .saturating_mul(weights.memory as u128),
            )
    }
}

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
fn stratified_ranges(
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
