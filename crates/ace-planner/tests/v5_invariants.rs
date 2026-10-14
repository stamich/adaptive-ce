//! Planner V5 invariants as properties (ACE 0.5.1), checked on every Corpus V3 / V4 workload,
//! every profile, with the Float lane on and off, and on generated inputs.
//!
//! The decision is reproduced exactly as the engine's block encoder makes it: NumericFast
//! evidence, then FloatFast, then the generic V4.3 pipeline followed by the TS1 policy.
//!
//! 1. `full_trial_encodes == 0` on every route (the hot path never encodes a whole candidate).
//! 2. A TS1 plan ⇔ a `TimeSeriesSelection`; the plan is plain (no transform, no entropy stage)
//!    and either FloatFast kept its payload within `input / 16` or the dominance rule
//!    `ts1_beats_generic` held against the generic decision.
//! 3. NumericFast carries no Float evidence; a Float route always carries it; with the Float
//!    lane disabled there is no Float route and no TS1 plan.
//! 4. Every other selected plan is one of the candidates generated for the block's route.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace_analysis::{AnalysisLevel, DefaultBlockAnalyzer};
use ace_core::{AceConfig, CodecId, CompressionProfile, EntropyCodecId};
use ace_corpus::Workload;
use ace_planner::{
    apply_time_series_policy, evaluate_candidates_v4_with_route, float_fast_decision,
    generic_decision_bytes, numeric_fast_decision_from_route, stable_plan_key, ts1_beats_generic,
    DefaultCompressionPlanner, FloatFastOutcome, PlannerDecision, PlannerRoute, PlanningContext,
};
use proptest::prelude::*;

/// Block size of the corpus checks (one production-sized block per workload).
const BLOCK: usize = 256 * 1024;

/// Checks every invariant for one block and configuration.
fn check_block(input: &[u8], config: &AceConfig, label: &str) {
    let context = PlanningContext::classify(input, config);
    let route = &context.route;

    // 3. Route / evidence consistency.
    if route.route == PlannerRoute::NumericFast {
        assert!(
            route.float_evidence.is_none(),
            "{label}: NumericFast with Float evidence"
        );
    }
    if route.route.is_float() {
        assert!(
            route.float_evidence.is_some(),
            "{label}: Float route without evidence"
        );
    }
    if !config.enable_float_specialization {
        assert!(
            !route.route.is_float(),
            "{label}: Float route with the lane disabled"
        );
    }

    // Reproduce the engine's decision (block_encoder::plan_block).
    if let Some(decision) = numeric_fast_decision_from_route(input, route) {
        assert_eq!(route.route, PlannerRoute::NumericFast, "{label}");
        assert!(
            decision.plan.is_plain_numeric(),
            "{label}: NumericFast plan"
        );
        check_common(input, &decision, label);
        return;
    }
    let float_fast = float_fast_decision(input, route).unwrap();
    if let FloatFastOutcome::Selected(decision) = float_fast {
        assert_eq!(route.route, PlannerRoute::FloatFast, "{label}");
        assert!(decision.telemetry.float_fast_hit, "{label}");
        let selection = decision.time_series.as_ref().expect("FloatFast selection");
        let payload = selection.payload.as_ref().expect("FloatFast payload");
        assert!(
            payload.len() * 16 <= input.len(),
            "{label}: FloatFast above input / 16"
        );
        check_common(input, &decision, label);
        return;
    }
    let profile =
        DefaultBlockAnalyzer.analyze_with_level(input, AnalysisLevel::for_profile(config.profile));
    let candidates =
        DefaultCompressionPlanner.candidates_for_route(&profile, config, route.candidate_route());
    let generic =
        evaluate_candidates_v4_with_route(input, &profile, &candidates, config, route).unwrap();
    let generic_bytes = generic_decision_bytes(&generic);
    let decision = apply_time_series_policy(input, generic, route, config).unwrap();
    check_common(input, &decision, label);

    match &decision.time_series {
        Some(selection) => {
            assert!(
                config.enable_float_specialization,
                "{label}: TS1 with the lane off"
            );
            assert!(
                ts1_beats_generic(&selection.estimate, generic_bytes, input.len()),
                "{label}: TS1 selected without dominating the generic plan"
            );
        }
        None => {
            // 4. Selected ∈ eligible candidates (a fast path may return RAW for trivial data).
            let key = stable_plan_key(&decision.plan);
            let generated = candidates.iter().any(|c| stable_plan_key(c) == key);
            assert!(
                generated || decision.telemetry.fast_path_hit,
                "{label}: selected plan {:?} was not a candidate",
                decision.plan.decoding
            );
        }
    }
}

/// Invariants 1 and 2, shared by every route.
fn check_common(input: &[u8], decision: &PlannerDecision, label: &str) {
    assert_eq!(
        decision.telemetry.full_trial_encodes, 0,
        "{label}: full trial encode"
    );
    let is_ts1 = decision.plan.decoding.codec == CodecId::TimeSeries;
    assert_eq!(
        is_ts1,
        decision.time_series.is_some(),
        "{label}: TS1 plan ⇔ selection"
    );
    if is_ts1 {
        assert!(
            decision.plan.decoding.transforms.is_empty(),
            "{label}: TS1 with transform"
        );
        assert_eq!(
            decision.plan.decoding.entropy,
            EntropyCodecId::None,
            "{label}"
        );
        assert!(decision.plan.decoding.dictionary.is_none(), "{label}");
        assert!(!input.is_empty(), "{label}");
    }
}

/// Configuration of the matrix.
fn config(profile: CompressionProfile, float: bool) -> AceConfig {
    AceConfig {
        profile,
        threads: 1,
        enable_float_specialization: float,
        ..AceConfig::default()
    }
}

/// Every Corpus V3 / V4 workload × profile × Float lane on / off.
#[test]
fn corpus_blocks_satisfy_v5_invariants() {
    for workload in Workload::ALL {
        let input = workload.generate(BLOCK);
        for profile in [
            CompressionProfile::Fast,
            CompressionProfile::Balanced,
            CompressionProfile::Dense,
        ] {
            for float in [true, false] {
                check_block(
                    &input,
                    &config(profile, float),
                    &format!("{workload} {profile:?} float={float}"),
                );
            }
        }
    }
}

/// Lane-structured generated input: f64 or u64 lanes with small random steps, optional runs
/// and a short tail — the shapes the Float and run lanes look for.
fn lane_input() -> impl Strategy<Value = Vec<u8>> {
    (
        any::<bool>(),
        1usize..4096,
        0u64..64,
        0u32..32,
        any::<u64>(),
        0usize..8,
    )
        .prop_map(|(float, values, max_step, run_every, seed, tail)| {
            let mut state = seed | 1;
            let mut value = 0u64;
            let mut out = Vec::with_capacity(values * 8 + tail);
            for i in 0..values {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let hold = run_every > 0 && !(i as u32).is_multiple_of(run_every);
                if !hold {
                    value = value.wrapping_add(state % (max_step + 1));
                }
                let bytes = if float {
                    (1000.0 + value as f64 * 0.125).to_le_bytes()
                } else {
                    value.to_le_bytes()
                };
                out.extend_from_slice(&bytes);
            }
            out.extend(std::iter::repeat_n(0xA5, tail));
            out
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// Generated lane data under every profile, Float lane on.
    #[test]
    fn generated_blocks_satisfy_v5_invariants(input in lane_input(), profile in 0usize..3) {
        let profile = [
            CompressionProfile::Fast,
            CompressionProfile::Balanced,
            CompressionProfile::Dense,
        ][profile];
        check_block(&input, &config(profile, true), "generated");
    }
}
