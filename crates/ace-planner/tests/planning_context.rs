//! Tests of `ace_planner` `planning_context` (formerly the inline unit tests of `src`).

use ace_core::AceConfig;
use ace_planner::*;

/// PlanningContext preserves the evidence returned by the one route-classification pass.
#[test]
fn fixed_counter_context_contains_fast_evidence() {
    let mut input = Vec::new();
    let mut value = 1000u32;
    for _ in 0..4096 {
        value += 3;
        input.extend_from_slice(&value.to_le_bytes());
    }
    let context = PlanningContext::classify(&input, &AceConfig::default());
    assert!(context.is_numeric_fast());
    assert!(context.route.numeric_fast_evidence.is_some());
}
