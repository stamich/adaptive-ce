//! Tests of `ace_planner` `confidence` (formerly the inline unit tests of `src`).

use ace_core::{PhysicalCompressionPlan, PlanCost};
use ace_cost::EstimatedCandidate;
use ace_planner::*;

/// Builds a compact candidate fixture with a selected analytical/sample pair.
fn candidate(analytical: u64, sampled: Option<u64>) -> EstimatedCandidate {
    EstimatedCandidate {
        plan: PhysicalCompressionPlan::raw(),
        analytical_size_bytes: analytical,
        sampled_size_bytes: sampled,
        blended_size_bytes: analytical,
        cost: PlanCost::default(),
        score: 0,
        confidence: 0.8,
    }
}

/// Confidence bands must follow the documented five/fifteen-percent thresholds.
#[test]
fn confidence_bands_are_deterministic() {
    assert_eq!(
        EstimateConfidence::from_candidate(&candidate(1000, Some(1030))),
        EstimateConfidence::High
    );
    assert_eq!(
        EstimateConfidence::from_candidate(&candidate(1000, Some(1100))),
        EstimateConfidence::Medium
    );
    assert_eq!(
        EstimateConfidence::from_candidate(&candidate(1000, Some(1300))),
        EstimateConfidence::Low
    );
    assert_eq!(
        EstimateConfidence::from_candidate(&candidate(1000, None)),
        EstimateConfidence::Low
    );
}
