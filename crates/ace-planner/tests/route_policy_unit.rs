//! Tests of `ace_planner` `route_policy` (formerly the inline unit tests of `src`).

use ace_core::{
    AceConfig, BlockProfile, CandidateTier, CodecId, CompressionProfile, DecodingPlan,
    EntropyCodecId, LzMode, PhysicalCompressionPlan, PlanCost,
};
use ace_planner::*;

/// Builds a neutral block profile for route-policy unit tests.
fn test_profile() -> BlockProfile {
    BlockProfile {
        size: 256 * 1024,
        entropy_h0: 4.0,
        entropy_h1: 3.0,
        zero_ratio: 0.0,
        run_score: 0.0,
        delta_score: 0.2,
        repetition_score: 0.1,
        sampled_match_length: 8.0,
        unique_byte_count: 128,
        incompressibility_score: 0.2,
    }
}

/// Builds a minimal test plan without invoking candidate generation.
fn test_plan(codec: CodecId, lz_mode: Option<LzMode>) -> PhysicalCompressionPlan {
    PhysicalCompressionPlan {
        decoding: DecodingPlan {
            transforms: Vec::new(),
            codec,
            dictionary: None,
            entropy: EntropyCodecId::None,
        },
        lz_mode,
        tier: CandidateTier::Likely,
        cost: PlanCost::default(),
        score: 0,
        reason: "route-policy test plan",
    }
}

/// Generic route must exclude Numeric from release-gate oracle eligibility.
#[test]
fn generic_marks_numeric_diagnostic_only() {
    let profile = test_profile();
    let config = AceConfig::default();
    assert!(matches!(
        RoutePolicy::candidate_eligibility(
            PlannerRoute::Generic,
            &test_plan(CodecId::Numeric, None),
            &profile,
            &config,
        ),
        CandidateEligibility::DiagnosticOnly(_)
    ));
}

/// NumericGeneral exposes only the intentionally bounded physical families.
#[test]
fn numeric_general_allows_numeric_raw_rle_and_lz() {
    let profile = test_profile();
    let config = AceConfig::default();
    for plan in [
        test_plan(CodecId::Numeric, None),
        test_plan(CodecId::Raw, None),
        test_plan(CodecId::Rle, None),
        test_plan(CodecId::Lz, Some(LzMode::Fast)),
        test_plan(CodecId::Lz, Some(LzMode::Balanced)),
    ] {
        assert!(RoutePolicy::candidate_allowed(
            PlannerRoute::NumericGeneral,
            &plan,
            &profile,
            &config,
        ));
    }
}

/// Strong Numeric dominance disables Hybrid-LZ work for BALANCED.
#[test]
fn numeric_margin_disables_hybrid_lz_when_dominant() {
    let margin = NumericMargin::new(100, 1_000);
    assert_eq!(
        margin.hybrid_lz_policy(CompressionProfile::Balanced),
        RouteHybridLzPolicy::Disabled
    );
}
