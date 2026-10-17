//! Tests of `ace_planner` `dominance_policy` (formerly the inline unit tests of `src`).

use ace_core::{
    AceConfig, BlockProfile, CandidateTier, CodecId, DecodingPlan, EntropyCodecId,
    PhysicalCompressionPlan, PlanCost,
};
use ace_planner::*;

/// Builds a minimal plan used to exercise preference semantics.
fn plan(codec: CodecId) -> PhysicalCompressionPlan {
    PhysicalCompressionPlan {
        decoding: DecodingPlan {
            transforms: Vec::new(),
            codec,
            dictionary: None,
            entropy: EntropyCodecId::None,
        },
        lz_mode: None,
        tier: CandidateTier::Likely,
        cost: PlanCost::default(),
        score: 0,
        reason: "dominance-policy test",
    }
}

/// Builds a block profile focused on zero/run-heavy behavior.
fn run_profile() -> BlockProfile {
    BlockProfile {
        size: 256 * 1024,
        entropy_h0: 0.1,
        entropy_h1: 0.1,
        zero_ratio: 1.0,
        run_score: 1.0,
        delta_score: 1.0,
        repetition_score: 1.0,
        sampled_match_length: 64.0,
        unique_byte_count: 1,
        incompressibility_score: 0.0,
    }
}

/// RLE is product-preferred for zero/run-heavy generic blocks.
#[test]
fn run_heavy_prefers_rle() {
    let config = AceConfig::default();
    let profile = run_profile();
    let (preference, reason) = DominancePolicy::preference(
        PlannerRoute::Generic,
        &plan(CodecId::Rle),
        &profile,
        &config,
    );
    assert_eq!(preference, CandidatePreference::Preferred);
    assert_eq!(reason, DominanceReason::RunLengthDominance);
}

/// BALANCED permits the expected small absolute policy premium.
#[test]
fn balanced_envelope_accepts_small_absolute_loss() {
    let config = AceConfig::default();
    let envelope = DominanceEnvelope::for_config(&config);
    assert!(envelope.admits(520, 772));
}
