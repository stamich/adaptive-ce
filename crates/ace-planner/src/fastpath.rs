use ace_core::{
    AceConfig, BlockProfile, CandidateTier, CodecId, DecodingPlan, EntropyCodecId, LzMode,
    PhysicalCompressionPlan, TransformId,
};

/// Deterministic planner shortcut for blocks whose statistics strongly imply one inexpensive plan.
pub trait PlannerFastPath: Send + Sync {
    /// Returns a complete candidate when no sampled verification is necessary.
    fn try_plan(
        &self,
        profile: &BlockProfile,
        config: &AceConfig,
    ) -> Option<PhysicalCompressionPlan>;
}

/// Default ACE 0.3 fast-path classifier.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultPlannerFastPath;

impl PlannerFastPath for DefaultPlannerFastPath {
    /// Applies high-confidence deterministic shortcuts before general candidate estimation.
    fn try_plan(&self, p: &BlockProfile, config: &AceConfig) -> Option<PhysicalCompressionPlan> {
        if !config.enable_planner_fast_paths {
            return None;
        }
        if p.incompressibility_score > 0.985 && p.entropy_h0 > 7.985 && p.repetition_score < 0.003 {
            return Some(PhysicalCompressionPlan::raw());
        }
        if p.zero_ratio > 0.985 || p.run_score > 0.92 {
            return Some(simple_plan(
                CodecId::Rle,
                EntropyCodecId::None,
                None,
                Vec::new(),
                "0.3 fast path: extreme run density",
            ));
        }
        if matches!(config.profile, ace_core::CompressionProfile::Fast)
            && p.repetition_score > 0.55
            && p.sampled_match_length >= 12.0
        {
            return Some(simple_plan(
                CodecId::Lz,
                EntropyCodecId::Huffman,
                Some(LzMode::Fast),
                Vec::new(),
                "0.3 fast path: strong LZ evidence",
            ));
        }
        if matches!(config.profile, ace_core::CompressionProfile::Fast)
            && p.delta_score > 0.75
            && p.entropy_h0 > 4.0
        {
            return Some(simple_plan(
                CodecId::Raw,
                EntropyCodecId::Huffman,
                None,
                vec![TransformId::DeltaByte],
                "0.3 fast path: strong delta evidence",
            ));
        }
        None
    }
}

/// Builds a planner-owned fast-path plan with stable explanation metadata.
fn simple_plan(
    codec: CodecId,
    entropy: EntropyCodecId,
    lz_mode: Option<LzMode>,
    transforms: Vec<TransformId>,
    reason: &'static str,
) -> PhysicalCompressionPlan {
    PhysicalCompressionPlan {
        decoding: DecodingPlan {
            transforms,
            codec,
            dictionary: None,
            entropy,
        },
        lz_mode,
        tier: CandidateTier::Likely,
        cost: Default::default(),
        score: 0,
        reason,
    }
}
