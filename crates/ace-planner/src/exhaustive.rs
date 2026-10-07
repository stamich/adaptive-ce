//! Exhaustive (oracle/explain) evaluation: every candidate is fully encoded and costed.
//!
//! Used by `ace explain`, the policy oracle and tests. The production hot path is the
//! sampled Planner V4 evaluator in `evaluator.rs`.

use ace_core::{AceConfig, AceError, AceResult, PhysicalCompressionPlan};

use crate::{encode_plan_payload, stable_plan_key, DeterministicCostModel, EntropySelectionPolicy};

/// Deterministically executes candidate pipelines and chooses the lowest cost without wall-clock timing.
pub fn evaluate_candidates(
    input: &[u8],
    candidates: &[PhysicalCompressionPlan],
    config: &AceConfig,
) -> AceResult<PhysicalCompressionPlan> {
    evaluate_all_candidates(input, candidates, config)?
        .into_iter()
        .min_by(|a, b| {
            a.score
                .cmp(&b.score)
                .then_with(|| stable_plan_key(a).cmp(&stable_plan_key(b)))
        })
        .ok_or(AceError::Malformed("planner generated no candidates"))
}

/// Deterministically evaluates every candidate and returns populated cost/score fields in stable input order.
pub fn evaluate_all_candidates(
    input: &[u8],
    candidates: &[PhysicalCompressionPlan],
    config: &AceConfig,
) -> AceResult<Vec<PhysicalCompressionPlan>> {
    let model = DeterministicCostModel;
    let mut evaluated = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let (metadata, payload, _) = encode_plan_payload(input, candidate)?;
        let metadata_prefix = candidate.decoding.entropy.metadata_prefix_bytes();
        let encoded_bytes = metadata
            .len()
            .saturating_add(payload.len())
            .saturating_add(metadata_prefix);
        let mut plan = candidate.clone();
        plan.cost = model.cost(
            &plan,
            input.len(),
            encoded_bytes,
            metadata.len().saturating_add(metadata_prefix),
        );
        plan.score = model.score(config.profile, plan.cost, input.len());
        evaluated.push(plan);
    }
    EntropySelectionPolicy::for_profile(config.profile).penalize_weak_rans(&mut evaluated);
    Ok(evaluated)
}
