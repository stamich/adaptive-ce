//! NumericGeneral route budgets and the zero-full-trial invariant.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace_analysis::{BlockAnalyzer, DefaultBlockAnalyzer};
use ace_core::{AceConfig, CodecId, CompressionProfile};
use ace_planner::{
    evaluate_candidates_v4, CandidateEligibility, CompressionPlanner, DefaultCompressionPlanner,
    PlannerRoute, RoutePolicy,
};

/// Builds a deterministic variable-delta u64 timestamp block representative of NumericGeneral.
fn variable_u64_timestamps(bytes: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes);
    let mut value = 1_780_000_000_000u64;
    let mut i = 0u64;
    while out.len() < bytes {
        value = value.wrapping_add(1000 + (i % 3));
        out.extend_from_slice(&value.to_le_bytes());
        i += 1;
    }
    out.truncate(bytes);
    out
}

/// NumericGeneral must obey the reduced candidate/sample budget and preserve zero full trials.
#[test]
fn numeric_general_uses_bounded_v4_2_budget() {
    let input = variable_u64_timestamps(256 * 1024);
    let config = AceConfig {
        profile: CompressionProfile::Balanced,
        threads: 1,
        ..AceConfig::default()
    };

    let route = RoutePolicy::classify(&input, &config);
    assert_eq!(route.route, PlannerRoute::NumericGeneral);

    let analyzer = DefaultBlockAnalyzer;
    let profile = analyzer.analyze(&input);
    let planner = DefaultCompressionPlanner;
    let candidates = planner.candidates(&profile, &config);
    let decision = evaluate_candidates_v4(&input, &profile, &candidates, &config).unwrap();

    assert!(decision.telemetry.estimated_candidates <= 5);
    assert!(decision.telemetry.sampled_candidates <= 2);
    assert_eq!(decision.telemetry.second_stage_candidates, 0);
    assert_eq!(decision.telemetry.full_trial_encodes, 0);
}

/// Generic zero-heavy routing keeps Numeric visible only to diagnostic/global-oracle analysis.
#[test]
fn zeros_mark_numeric_diagnostic_only() {
    let input = vec![0u8; 256 * 1024];
    let config = AceConfig::default();
    let route = RoutePolicy::classify(&input, &config);
    assert_eq!(route.route, PlannerRoute::Generic);

    let analyzer = DefaultBlockAnalyzer;
    let profile = analyzer.analyze(&input);
    let planner = DefaultCompressionPlanner;
    let candidates = planner.candidates(&profile, &config);
    let numeric = candidates
        .iter()
        .find(|plan| matches!(plan.decoding.codec, CodecId::Numeric))
        .expect("BALANCED generator should expose Numeric for global diagnostics");

    assert!(matches!(
        RoutePolicy::candidate_eligibility(route.route, numeric, &profile, &config),
        CandidateEligibility::DiagnosticOnly(_)
    ));
}
