use ace_analysis::{BlockAnalyzer, DefaultBlockAnalyzer};
use ace_core::{AceConfig, CodecId, CompressionProfile};
use ace_planner::{evaluate_candidates_v4, CompressionPlanner, DefaultCompressionPlanner};

/// Builds one deterministic monotonic little-endian u32 counter block.
fn numeric_counter_block(bytes: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes);
    let mut value = 10_000u32;
    while out.len() < bytes {
        value = value.wrapping_add(3);
        out.extend_from_slice(&value.to_le_bytes());
    }
    out.truncate(bytes);
    out
}

/// Planner V4 must retain and select Numeric when its exact bit-packed estimate dominates.
///
/// This protects against the 0.4 bug where byte-level `BlockProfile` heuristics ranked Numeric
/// below the stage-one pool before the real numeric codec could be sampled.
#[test]
fn exact_numeric_estimate_keeps_numeric_in_v4_search() {
    let input = numeric_counter_block(256 * 1024);
    let analyzer = DefaultBlockAnalyzer;
    let profile = analyzer.analyze(&input);
    let mut config = AceConfig::default();
    config.profile = CompressionProfile::Balanced;
    config.threads = 1;

    let planner = DefaultCompressionPlanner;
    let candidates = planner.candidates(&profile, &config);
    assert!(
        candidates
            .iter()
            .any(|candidate| matches!(candidate.decoding.codec, CodecId::Numeric)),
        "Planner V4 candidate generation must admit Numeric"
    );

    let decision = evaluate_candidates_v4(&input, &profile, &candidates, &config).unwrap();
    assert!(
        decision
            .top_k_plans
            .iter()
            .any(|plan| matches!(plan.decoding.codec, CodecId::Numeric)),
        "exact Numeric estimate must keep Numeric inside stage-one verification"
    );
    assert_eq!(decision.plan.decoding.codec, CodecId::Numeric);
    assert_eq!(decision.telemetry.full_trial_encodes, 0);
}
