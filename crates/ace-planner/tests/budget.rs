//! Tests of `ace_planner` `budget` (formerly the inline unit tests of `src`).

use ace_core::CompressionProfile;
use ace_planner::*;

/// Numeric data retains more LZ work than structured data in BALANCED.
#[test]
fn numeric_keeps_wide_lz_budget() {
    let numeric =
        PlanningBudget::for_block(CompressionProfile::Balanced, PlannerDataClass::Numeric);
    let structured =
        PlanningBudget::for_block(CompressionProfile::Balanced, PlannerDataClass::Structured);
    assert!(numeric.hybrid_stage1_candidates >= structured.hybrid_stage1_candidates);
    assert!(numeric.hybrid_stage2_candidates >= structured.hybrid_stage2_candidates);
}

/// Zero-heavy blocks must not spend micro-trial budget on LZ alternatives.
#[test]
fn zero_heavy_skips_hybrid_lz() {
    let budget = PlanningBudget::for_block(CompressionProfile::Dense, PlannerDataClass::ZeroHeavy);
    assert_eq!(budget.hybrid_stage1_candidates, 0);
    assert_eq!(budget.hybrid_stage2_candidates, 0);
}
