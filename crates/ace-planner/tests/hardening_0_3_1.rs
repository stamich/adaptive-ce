use ace_analysis::{BlockAnalyzer, DefaultBlockAnalyzer};
use ace_core::{AceConfig, CompressionProfile};
use ace_planner::{
    evaluate_candidates_v3, CompressionPlanner, DefaultCompressionPlanner, PlannerDataClass,
    PlanningBudget,
};

/// Runs Planner V3.6 for one block and returns the decision telemetry.
fn plan_block(data: &[u8], profile: CompressionProfile) -> ace_planner::PlannerDecision {
    let config = AceConfig { profile, ..AceConfig::default() };
    let block_profile = DefaultBlockAnalyzer.analyze(data);
    let candidates = DefaultCompressionPlanner.candidates(&block_profile, &config);
    evaluate_candidates_v3(data, &block_profile, &candidates, &config).unwrap()
}

/// Zero-heavy data must not spend Hybrid-LZ micro-trial budget.
#[test]
fn zero_heavy_blocks_skip_hybrid_lz() {
    let data = vec![0u8; 262_144];
    let decision = plan_block(&data, CompressionProfile::Dense);
    assert_eq!(decision.telemetry.hybrid_lz_candidates, 0);
    assert_eq!(decision.telemetry.hybrid_lz_sample_bytes, 0);
}

/// Incompressible input has a zero Hybrid-LZ work budget.
#[test]
fn incompressible_budget_is_zero() {
    let budget = PlanningBudget::for_block(
        CompressionProfile::Balanced,
        PlannerDataClass::Incompressible,
    );
    assert_eq!(budget.hybrid_stage1_candidates, 0);
    assert_eq!(budget.hybrid_stage2_candidates, 0);
}

/// Planner V3.6 must never perform a full trial encode on the production decision path.
#[test]
fn production_planner_has_zero_full_trials() {
    let data = b"{\"service\":\"ace\",\"status\":\"ACTIVE\",\"value\":123456}\n".repeat(4_000);
    for profile in [CompressionProfile::Fast, CompressionProfile::Balanced, CompressionProfile::Dense] {
        let decision = plan_block(&data, profile);
        assert_eq!(decision.telemetry.full_trial_encodes, 0);
    }
}

/// Repeated planning of identical bytes must preserve plan semantics and telemetry.
#[test]
fn planner_v3_6_is_fully_deterministic() {
    let data = (0u8..=255).cycle().take(262_144).collect::<Vec<_>>();
    let a = plan_block(&data, CompressionProfile::Balanced);
    let b = plan_block(&data, CompressionProfile::Balanced);
    assert_eq!(a.plan.decoding, b.plan.decoding);
    assert_eq!(a.plan.lz_mode, b.plan.lz_mode);
    assert_eq!(a.telemetry, b.telemetry);
}
