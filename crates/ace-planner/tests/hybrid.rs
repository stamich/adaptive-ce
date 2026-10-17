//! Tests of `ace_planner` `hybrid` (formerly the inline unit tests of `src`).

use ace_core::CompressionProfile;
use ace_planner::*;

/// DENSE spends more micro-trial bytes than BALANCED, while both remain bounded.
#[test]
fn dense_budget_is_larger_but_bounded() {
    let balanced = HybridLzPolicy::for_profile(CompressionProfile::Balanced, 1);
    let dense = HybridLzPolicy::for_profile(CompressionProfile::Dense, 1);
    assert!(
        dense.window_bytes * dense.window_count >= balanced.window_bytes * balanced.window_count
    );
    assert!(dense.window_bytes * dense.window_count <= 64 * 1024);
}

/// Deterministic range generation must never depend on process state or randomness.
#[test]
fn ranges_are_deterministic() {
    let a = ace_cost::stratified_ranges(262_144, 8 * 1024, 3);
    let b = ace_cost::stratified_ranges(262_144, 8 * 1024, 3);
    assert_eq!(a, b);
    assert_eq!(a.len(), 3);
}
