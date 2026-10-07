//! ACE 0.4.5 planner-level regression tests for Numeric candidate admission.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace_analysis::{AnalysisLevel, DefaultBlockAnalyzer};
use ace_core::{AceConfig, CodecId, CompressionProfile};
use ace_planner::{DefaultCompressionPlanner, PlannerRoute, PlanningContext};

/// Monotonic u64 timestamps with the given base step and jitter span.
fn timestamps(step: u64, jitter: u64) -> Vec<u8> {
    let (mut s, mut t) = (12345u64, 1_700_000_000_000u64);
    let mut out = Vec::new();
    for _ in 0..32_768 {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        t += step + s % jitter;
        out.extend_from_slice(&t.to_le_bytes());
    }
    out
}

/// Returns the candidate codecs the route-aware generator produces for `input`.
fn codecs(input: &[u8], profile: CompressionProfile) -> (PlannerRoute, Vec<CodecId>) {
    let config = AceConfig {
        profile,
        ..AceConfig::default()
    };
    let ctx = PlanningContext::classify(input, &config);
    let p = DefaultBlockAnalyzer.analyze_with_level(input, AnalysisLevel::for_profile(profile));
    let cands = DefaultCompressionPlanner.candidates_for_route(&p, &config, ctx.route.route);
    (
        ctx.route.route,
        cands.iter().map(|c| c.decoding.codec).collect(),
    )
}

/// FAST's Huffman baseline (Raw codec + entropy stage) must not suppress Numeric injection.
#[test]
fn fast_numeric_general_route_gets_numeric_candidate() {
    let (route, cs) = codecs(&timestamps(1_000_000, 1_000_000), CompressionProfile::Fast);
    assert_eq!(route, PlannerRoute::NumericGeneral);
    assert!(cs.contains(&CodecId::Numeric));
}

/// BALANCED and DENSE must also see Numeric for the same jittered timestamps.
#[test]
fn balanced_and_dense_get_numeric_candidate() {
    for p in [CompressionProfile::Balanced, CompressionProfile::Dense] {
        assert!(codecs(&timestamps(1_000_000, 1_000_000), p)
            .1
            .contains(&CodecId::Numeric));
    }
}

/// Non-numeric (pseudo-random) data takes the Generic route, so no route-driven injection happens.
#[test]
fn random_data_is_not_injected() {
    let mut s = 99u64;
    let data: Vec<u8> = (0..262_144)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s as u8
        })
        .collect();
    let (route, cs) = codecs(&data, CompressionProfile::Fast);
    assert_eq!(route, PlannerRoute::Generic);
    assert!(!cs.contains(&CodecId::Numeric));
}
