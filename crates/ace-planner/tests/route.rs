//! Tests of `ace_planner` `route` (formerly the inline unit tests of `src`).

use ace_core::{AceConfig, CodecId};
use ace_planner::*;

/// Strong monotonic counters use NumericFast and avoid candidate sampling.
#[test]
fn counter_routes_to_numeric_fast() {
    let mut bytes = Vec::new();
    let mut value = 1_000u32;
    for _ in 0..65_536 {
        value += 3;
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    let config = AceConfig::default();
    let route = classify_planner_route(&bytes, &config);
    assert_eq!(route.route, PlannerRoute::NumericFast);
    let decision = try_numeric_fast_path(&bytes, &config).expect("numeric fast path");
    assert_eq!(decision.plan.decoding.codec, CodecId::Numeric);
    assert_eq!(decision.telemetry.hybrid_lz_candidates, 0);
    assert_eq!(decision.telemetry.sampled_candidates, 0);
    assert_eq!(decision.telemetry.full_trial_encodes, 0);
}

/// Zero-heavy input stays on the generic path so RLE remains available for cheap decode.
#[test]
fn zeros_do_not_use_numeric_fast() {
    let bytes = vec![0u8; 256 * 1024];
    let route = classify_planner_route(&bytes, &AceConfig::default());
    assert_ne!(route.route, PlannerRoute::NumericFast);
}
