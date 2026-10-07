use ace_analysis::{numeric_prefilter, strong_numeric_evidence, NumericFastEvidence, NumericPrefilter};
use ace_core::{
    AceConfig, CandidateTier, CodecId, CompressionProfile, DecodingPlan, EntropyCodecId,
    PhysicalCompressionPlan, PlanCost,
};

use crate::{PlannerDecision, PlannerTelemetry};

/// Planner V4.3 execution route chosen before expensive candidate evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannerRoute {
    /// Uses the frozen generic V3.6-style analysis and bounded candidate pipeline.
    Generic,
    /// Uses the exact Numeric estimator together with a reduced generic search space.
    NumericGeneral,
    /// Selects Numeric directly after cheap sampling plus full-block structural validation.
    NumericFast,
}

/// Explainable reason for selecting one Planner V4 route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteReason {
    /// Numeric specialization is disabled in configuration.
    NumericDisabled,
    /// Bounded prefilter found insufficient numeric structure.
    PrefilterRejected,
    /// Numeric structure is plausible but not strong enough for direct selection.
    NumericCandidate,
    /// Fixed-step monotonic structure was validated across the complete block.
    StrongFixedStepNumeric,
}

/// Route-classifier output used by tests, explain and route benchmarks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RouteDecision {
    /// Selected planner route.
    pub route: PlannerRoute,
    /// Deterministic reason for the route.
    pub reason: RouteReason,
    /// Cheap prefilter statistics supporting the decision.
    pub prefilter: NumericPrefilter,
    /// Reusable full-block evidence present only for a validated NumericFast route.
    pub numeric_fast_evidence: Option<NumericFastEvidence>,
}

/// Classifies one block without running the generic block analyzer or numeric encoder.
pub fn classify_planner_route(input: &[u8], config: &AceConfig) -> RouteDecision {
    if !config.enable_numeric_specialization {
        return RouteDecision {
            route: PlannerRoute::Generic,
            reason: RouteReason::NumericDisabled,
            prefilter: numeric_prefilter(&[]),
            numeric_fast_evidence: None,
        };
    }

    let prefilter = numeric_prefilter(input);
    if !prefilter.likely_numeric {
        return RouteDecision {
            route: PlannerRoute::Generic,
            reason: RouteReason::PrefilterRejected,
            prefilter,
            numeric_fast_evidence: None,
        };
    }

    let fast_threshold = match config.profile {
        CompressionProfile::Fast => 0.995,
        CompressionProfile::Balanced => 0.98,
        CompressionProfile::Dense => 0.97,
    };

    let numeric_fast_evidence = if prefilter.strong_numeric
        && prefilter.confidence >= fast_threshold
    {
        prefilter
            .width_hint
            .and_then(|width| strong_numeric_evidence(input, width))
    } else {
        None
    };

    if numeric_fast_evidence.is_some() {
        RouteDecision {
            route: PlannerRoute::NumericFast,
            reason: RouteReason::StrongFixedStepNumeric,
            prefilter,
            numeric_fast_evidence,
        }
    } else {
        RouteDecision {
            route: PlannerRoute::NumericGeneral,
            reason: RouteReason::NumericCandidate,
            prefilter,
            numeric_fast_evidence: None,
        }
    }
}

/// Returns a direct Numeric decision from an already-computed route decision.
///
/// This function never classifies or validates the block again. Buildfix4 therefore guarantees
/// one route-classification/strong-validation pass per production block.
pub fn numeric_fast_decision_from_route(
    input: &[u8],
    route: &RouteDecision,
) -> Option<PlannerDecision> {
    if !matches!(route.route, PlannerRoute::NumericFast) {
        return None;
    }

    let evidence = route.numeric_fast_evidence?;
    let predicted_size =
        ace_codecs::NUMERIC_HEADER_SIZE.saturating_add(evidence.tail_bytes) as u64;

    let plan = PhysicalCompressionPlan {
        decoding: DecodingPlan {
            transforms: Vec::new(),
            codec: CodecId::Numeric,
            dictionary: None,
            entropy: EntropyCodecId::None,
        },
        lz_mode: None,
        tier: CandidateTier::Likely,
        cost: PlanCost {
            predicted_size_bytes: predicted_size,
            metadata_bytes: ace_codecs::NUMERIC_HEADER_SIZE as u64,
            encode_units: evidence.value_count as u64,
            decode_units: evidence.value_count as u64,
            memory_bytes: input.len() as u64,
        },
        score: predicted_size as u128,
        reason: "Planner V4.3 validated fixed-step Numeric fast path",
    };

    let telemetry = PlannerTelemetry {
        fast_path_hit: true,
        estimated_candidates: 1,
        sampled_candidates: 0,
        second_stage_candidates: 0,
        hybrid_lz_candidates: 0,
        hybrid_lz_stage1_candidates: 0,
        hybrid_lz_stage2_candidates: 0,
        hybrid_lz_skipped_candidates: 0,
        hybrid_lz_high_confidence_skips: 0,
        hybrid_lz_sample_bytes: 0,
        hybrid_lz_max_disagreement_ppm: 0,
        quality_qualified_candidates: 1,
        best_blended_size_bytes: predicted_size,
        quality_limit_bytes: predicted_size,
        selected_blended_size_bytes: predicted_size,
        selected_size_rank: 1,
        selected_cost_rank: 1,
        full_trial_encodes: 0,
    };

    Some(PlannerDecision {
        plan: plan.clone(),
        telemetry,
        analytical_ranked_plans: vec![plan.clone()],
        top_k_plans: vec![plan.clone()],
        stage_one_ranked_plans: vec![plan.clone()],
        second_stage_plans: Vec::new(),
        final_ranked_plans: vec![plan.clone()],
        quality_qualified_plans: vec![plan],
        numeric_estimate: None,
    })
}

/// Compatibility wrapper that classifies once and then builds the NumericFast decision.
///
/// Production engine code should prefer `numeric_fast_decision_from_route` with its existing
/// `PlanningContext`; this wrapper remains useful to focused unit tests and callers outside engine.
pub fn try_numeric_fast_path(input: &[u8], config: &AceConfig) -> Option<PlannerDecision> {
    let route = classify_planner_route(input, config);
    numeric_fast_decision_from_route(input, &route)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
