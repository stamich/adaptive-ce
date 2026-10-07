//! Analytical (no-encode) candidate size and resource estimation.

use ace_core::{
    BlockProfile, CodecId, CompressionProfile, EntropyCodecId, LzMode, PhysicalCompressionPlan,
    PlanCost, TransformId,
};

use crate::CostModelV3;

/// Cheap deterministic estimate of one candidate compression plan.
#[derive(Debug, Clone)]
pub struct EstimatedCandidate {
    /// Candidate plan to which the estimate belongs.
    pub plan: PhysicalCompressionPlan,
    /// Full-block analytical size predicted before any sample encoding.
    pub analytical_size_bytes: u64,
    /// Most recent full-block size projection derived from deterministic sample encoding.
    pub sampled_size_bytes: Option<u64>,
    /// Size estimate used for quality qualification after analytical/sample blending.
    pub blended_size_bytes: u64,
    /// Estimated multidimensional resource cost. Its `predicted_size_bytes` mirrors
    /// `blended_size_bytes` so existing cost-model code remains compatible.
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

/// Default analytical estimator used by ACE 0.3-buildfix6.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultCandidateEstimator;

/// Implements [`CandidateEstimator`] for [`DefaultCandidateEstimator`].
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
            CodecId::Numeric => {
                // Numeric candidate ranking deliberately uses only existing cheap BlockProfile
                // signals. Planner V4 sample verification then executes the real self-describing
                // numeric codec, avoiding the formula-overfitting failure seen in buildfix7.
                let delta = p.delta_score.clamp(0.0, 1.0) as f64;
                let structural = (1.0 - (p.entropy_h0 as f64 / 8.0)).clamp(0.0, 1.0);
                let gain = (delta * 0.75 + structural * 0.25).clamp(0.0, 0.90);
                primary_ratio *= (1.0 - gain).clamp(0.04, 1.02);
                confidence = confidence.max((0.58 + p.delta_score * 0.35).clamp(0.0, 0.96));
            }
            CodecId::TimeSeries => {
                // TS1 is never ranked from the generic BlockProfile: the Float lane and the
                // RunDelta admission use dedicated exact/sample estimators (ace_cost::time_series).
                // A neutral ratio keeps an accidental TS1 candidate from winning generic ranking.
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
            (CodecId::Numeric, _) => 5,
            (CodecId::TimeSeries, _) => 4,
        };
        let codec_decode = match candidate.decoding.codec {
            CodecId::Raw => 1u64,
            CodecId::Rle => 2,
            CodecId::Lz => 3,
            CodecId::Numeric => 2,
            CodecId::TimeSeries => 2,
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
            analytical_size_bytes: predicted_size,
            sampled_size_bytes: None,
            blended_size_bytes: predicted_size,
            cost,
            score,
            confidence,
        }
    }
}
