use ace_core::{AceConfig, BlockProfile, CandidateTier, CodecId, DecodingPlan, EntropyCodecId, LzMode, PhysicalCompressionPlan, TransformId};

/// Deterministic planner shortcut for blocks whose statistics strongly imply one inexpensive plan.
pub trait PlannerFastPath: Send + Sync {
    /// Returns a complete candidate when no sampled verification is necessary.
    fn try_plan(&self, profile: &BlockProfile, config: &AceConfig) -> Option<PhysicalCompressionPlan>;
}

/// Default ACE 0.3.1 fast-path classifier with the frozen zero-heavy quality guard.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultPlannerFastPath;

/// Implements [`PlannerFastPath`] for [`DefaultPlannerFastPath`].
impl PlannerFastPath for DefaultPlannerFastPath {
    /// Applies high-confidence deterministic shortcuts before general candidate estimation.
    fn try_plan(&self, p: &BlockProfile, config: &AceConfig) -> Option<PhysicalCompressionPlan> {
        if !config.enable_planner_fast_paths { return None; }
        if p.incompressibility_score > 0.985 && p.entropy_h0 > 7.985 && p.repetition_score < 0.003 {
            return Some(PhysicalCompressionPlan::raw());
        }
        if p.zero_ratio > 0.985 || p.run_score > 0.92 {
            // FAST keeps the cheap RLE shortcut. BALANCED and DENSE must compare entropy-coded
            // alternatives because buildfix6 showed all-zero blocks can be much smaller as
            // RAW+rANS / RLE+entropy than as bare RLE packets.
            if matches!(config.profile, ace_core::CompressionProfile::Fast) {
                return Some(simple_plan(
                    CodecId::Rle,
                    EntropyCodecId::None,
                    None,
                    Vec::new(),
                    "0.3.1 fast path: extreme run density under FAST quality policy",
                ));
            }
            return None;
        }
        if matches!(config.profile, ace_core::CompressionProfile::Fast)
            && p.repetition_score > 0.55 && p.sampled_match_length >= 12.0
        {
            return Some(simple_plan(CodecId::Lz, EntropyCodecId::Huffman, Some(LzMode::Fast), Vec::new(), "0.3 fast path: strong LZ evidence"));
        }
        if matches!(config.profile, ace_core::CompressionProfile::Fast)
            && p.delta_score > 0.75 && p.entropy_h0 > 4.0
        {
            return Some(simple_plan(CodecId::Raw, EntropyCodecId::Huffman, None, vec![TransformId::DeltaByte], "0.3 fast path: strong delta evidence"));
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
        decoding: DecodingPlan { transforms, codec, dictionary: None, entropy },
        lz_mode,
        tier: CandidateTier::Likely,
        cost: Default::default(),
        score: 0,
        reason,
    }
}


#[cfg(test)]
mod buildfix8_tests {
    use super::*;
    use ace_core::CompressionProfile;

    /// Builds an extreme zero-heavy profile that previously bypassed entropy alternatives.
    fn zero_heavy_profile() -> BlockProfile {
        BlockProfile {
            size: 262_144,
            entropy_h0: 0.0,
            entropy_h1: 0.0,
            zero_ratio: 1.0,
            run_score: 1.0,
            delta_score: 0.0,
            repetition_score: 1.0,
            sampled_match_length: 64.0,
            unique_byte_count: 1,
            incompressibility_score: 0.0,
        }
    }

    /// BALANCED must compare entropy alternatives instead of forcing bare RLE.
    #[test]
    fn balanced_zero_heavy_uses_general_planner() {
        let config = AceConfig { profile: CompressionProfile::Balanced, ..AceConfig::default() };
        assert!(DefaultPlannerFastPath.try_plan(&zero_heavy_profile(), &config).is_none());
    }

    /// DENSE must compare entropy alternatives instead of forcing bare RLE.
    #[test]
    fn dense_zero_heavy_uses_general_planner() {
        let config = AceConfig { profile: CompressionProfile::Dense, ..AceConfig::default() };
        assert!(DefaultPlannerFastPath.try_plan(&zero_heavy_profile(), &config).is_none());
    }

    /// FAST preserves the speed-first RLE shortcut.
    #[test]
    fn fast_zero_heavy_keeps_rle_shortcut() {
        let config = AceConfig { profile: CompressionProfile::Fast, ..AceConfig::default() };
        let plan = DefaultPlannerFastPath
            .try_plan(&zero_heavy_profile(), &config)
            .expect("FAST zero-heavy profile should use fast path");
        assert_eq!(plan.decoding.codec, CodecId::Rle);
        assert_eq!(plan.decoding.entropy, EntropyCodecId::None);
    }
}
