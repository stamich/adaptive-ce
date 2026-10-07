//! Planner hot path: zero full trials and deterministic decisions.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace_analysis::{BlockAnalyzer, DefaultBlockAnalyzer};
use ace_core::{AceConfig, CompressionProfile};
use ace_planner::{evaluate_candidates_v3, CompressionPlanner, DefaultCompressionPlanner};

/// Verifies that Planner V3.1 preserves the zero-full-trial hot-path invariant.
#[test]
fn hot_path_has_zero_full_trials() {
    let data = b"status=ACTIVE region=eu service=ace\n".repeat(10_000);
    let config = AceConfig {
        profile: CompressionProfile::Balanced,
        ..AceConfig::default()
    };
    let profile = DefaultBlockAnalyzer.analyze(&data);
    let candidates = DefaultCompressionPlanner.candidates(&profile, &config);
    let decision = evaluate_candidates_v3(&data, &profile, &candidates, &config).unwrap();
    assert_eq!(decision.telemetry.full_trial_encodes, 0);
    assert!(decision.telemetry.sampled_candidates <= 8);
    assert!(decision.telemetry.second_stage_candidates <= decision.telemetry.sampled_candidates);
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
    assert_eq!(a.telemetry, b.telemetry);
}

/// Verifies that DENSE keeps a quality-preserving verification pool for structured data.
#[test]
fn dense_keeps_multiple_quality_candidates() {
    let data = b"{\"status\":\"ACTIVE\",\"service\":\"ace\",\"region\":\"eu\"}\n".repeat(6_000);
    let config = AceConfig {
        profile: CompressionProfile::Dense,
        ..AceConfig::default()
    };
    let profile = DefaultBlockAnalyzer.analyze(&data);
    let candidates = DefaultCompressionPlanner.candidates(&profile, &config);
    let decision = evaluate_candidates_v3(&data, &profile, &candidates, &config).unwrap();
    if !decision.telemetry.fast_path_hit {
        assert!(decision.top_k_plans.len() >= 4);
        assert!(decision.second_stage_plans.len() >= 3);
    }
}
