use ace_analysis::{BlockAnalyzer, DefaultBlockAnalyzer};
use ace_core::{AceConfig, CompressionProfile};
use ace_planner::{evaluate_candidates_v3, CompressionPlanner, DefaultCompressionPlanner};

/// Verifies that the hot planner performs no full-block candidate trial encodes.
#[test]
fn hot_path_has_zero_full_trials() {
    let data = b"status=ACTIVE region=eu service=ace\n".repeat(10_000);
    let mut config = AceConfig::default();
    config.profile = CompressionProfile::Balanced;
    let profile = DefaultBlockAnalyzer.analyze(&data);
    let candidates = DefaultCompressionPlanner.candidates(&profile, &config);
    let decision = evaluate_candidates_v3(&data, &profile, &candidates, &config).unwrap();
    assert_eq!(decision.telemetry.full_trial_encodes, 0);
    assert!(decision.telemetry.sampled_candidates <= 2);
}

/// Verifies that repeated planning of identical input yields identical physical semantics.
#[test]
fn planner_is_deterministic() {
    let data = (0u8..=255).cycle().take(200_000).collect::<Vec<_>>();
    let config = AceConfig::default();
    let profile = DefaultBlockAnalyzer.analyze(&data);
    let candidates = DefaultCompressionPlanner.candidates(&profile, &config);
    let a = evaluate_candidates_v3(&data, &profile, &candidates, &config).unwrap();
    let b = evaluate_candidates_v3(&data, &profile, &candidates, &config).unwrap();
    assert_eq!(a.plan.decoding, b.plan.decoding);
    assert_eq!(a.plan.lz_mode, b.plan.lz_mode);
}
