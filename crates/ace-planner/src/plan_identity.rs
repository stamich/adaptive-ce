//! Deterministic plan identity: tie-break keys, semantic equality and family grouping.

use ace_core::{LzMode, PhysicalCompressionPlan};

/// Produces a stable tie-break key that does not depend on address, thread scheduling or hash iteration order.
pub fn stable_plan_key(plan: &PhysicalCompressionPlan) -> (Vec<u8>, u8, u8, u8) {
    let transforms = plan
        .decoding
        .transforms
        .iter()
        .map(|t| *t as u8)
        .collect::<Vec<_>>();
    (
        transforms,
        plan.decoding.codec as u8,
        plan.decoding.entropy as u8,
        lz_mode_rank(plan),
    )
}

/// Returns true when two physical plans have identical decoder semantics.
pub fn same_plan_semantics(a: &PhysicalCompressionPlan, b: &PhysicalCompressionPlan) -> bool {
    a.decoding == b.decoding && a.lz_mode == b.lz_mode
}

/// Groups entropy variants of the same physical transform/primary-codec family.
pub(crate) fn semantic_family_key(plan: &PhysicalCompressionPlan) -> (Vec<u8>, u8, u8) {
    let transforms = plan
        .decoding
        .transforms
        .iter()
        .map(|t| *t as u8)
        .collect::<Vec<_>>();
    (transforms, plan.decoding.codec as u8, lz_mode_rank(plan))
}

/// Stable numeric rank of the (non-serialized) LZ search mode.
fn lz_mode_rank(plan: &PhysicalCompressionPlan) -> u8 {
    match plan.lz_mode {
        None => 0,
        Some(LzMode::Fast) => 1,
        Some(LzMode::Balanced) => 2,
    }
}
