//! Planner V3.6/V4 hot-path evaluator: analytical estimates, adaptive Top-K, sample
//! verification, quality envelope and final cost selection — without full trial encodes.

use ace_core::{AceConfig, AceError, AceResult, CodecId, CompressionProfile, LzMode, PhysicalCompressionPlan};

use crate::plan_identity::semantic_family_key;
use crate::{encode_plan_payload, same_plan_semantics, stable_plan_key, EntropySelectionPolicy, PlannerDecision, PlannerTelemetry};

/// ACE 0.4 Planner V4 compatibility entry point.
///
/// Callers that already own a `PlanningContext` should use
/// `evaluate_candidates_v4_with_route` to avoid a duplicate route-classification pass.
pub fn evaluate_candidates_v4(
    input: &[u8],
    profile: &ace_core::BlockProfile,
    candidates: &[PhysicalCompressionPlan],
    config: &AceConfig,
) -> AceResult<PlannerDecision> {
    let route = crate::RoutePolicy::classify(input, config);
    evaluate_candidates_v4_with_route(input, profile, candidates, config, &route)
}

/// Evaluates Planner V4.3 candidates using an already-computed route decision.
///
/// NumericFast returns directly from reusable validation evidence. NumericGeneral performs one
/// exact Numeric estimate; Generic never performs a full Numeric estimate.
pub fn evaluate_candidates_v4_with_route(
    input: &[u8],
    profile: &ace_core::BlockProfile,
    candidates: &[PhysicalCompressionPlan],
    config: &AceConfig,
    route: &crate::RouteDecision,
) -> AceResult<PlannerDecision> {
    if let Some(decision) = crate::numeric_fast_decision_from_route(input, route) {
        return Ok(decision);
    }

    // Safety net (0.4.5): even when the caller built `candidates` without the route-aware
    // generator, a NumericGeneral block must be able to choose Numeric.
    let injected;
    let candidates = if crate::needs_numeric_candidate(candidates, route.route, config) {
        let mut with_numeric = candidates.to_vec();
        crate::ensure_numeric_candidate(&mut with_numeric, route.route, config);
        injected = with_numeric;
        &injected[..]
    } else {
        candidates
    };

    let numeric_estimate = if matches!(route.route, crate::PlannerRoute::NumericGeneral) {
        match (config.profile, route.prefilter.width_hint) {
            // FAST trusts the prefilter's lane width and scans the block once instead of three
            // times (u16/u32/u64). BALANCED/DENSE keep the exhaustive exact search.
            (ace_core::CompressionProfile::Fast, Some(width)) => {
                ace_codecs::estimate_numeric_for_width(input, width)
            }
            _ => ace_codecs::estimate_numeric(input),
        }
    } else {
        None
    };
    let mut decision = evaluate_candidates_internal(
        input,
        profile,
        candidates,
        config,
        numeric_estimate.map(|estimate| estimate.encoded_bytes as u64),
        Some(route.route),
    )?;
    // Hand the exact estimate to the encoder so it does not repeat the width/mode search.
    decision.numeric_estimate = numeric_estimate;
    Ok(decision)
}

/// Estimates all candidates, verifies a quality-preserving adaptive pool and selects a plan.
///
/// ACE 0.4 Planner V4 preserves the hardened Planner V3.6 behavior: profile/data-class budgets and confidence bands avoid unnecessary Hybrid LZ work without changing decoder semantics.
/// Adaptive Top-K and semantic-family anchors define the search pool; sampling refines
/// scores but never removes a stage-one survivor. The final QualityEnvelope first filters by
/// blended compressed size, then the cost model selects the cheapest quality-safe plan.
pub fn evaluate_candidates_v3(
    input: &[u8],
    profile: &ace_core::BlockProfile,
    candidates: &[PhysicalCompressionPlan],
    config: &AceConfig,
) -> AceResult<PlannerDecision> {
    evaluate_candidates_internal(input, profile, candidates, config, None, None)
}

/// Shared hardened V3.6/V4 evaluator.
///
/// `numeric_exact_size` is `None` for the frozen V3.6 path. Planner V4 supplies the deterministic
/// exact-size estimate produced by `ace_codecs::estimate_numeric`, allowing Numeric to compete on
/// its actual bit-packed size without performing a full trial encode.
fn evaluate_candidates_internal(
    input: &[u8],
    profile: &ace_core::BlockProfile,
    candidates: &[PhysicalCompressionPlan],
    config: &AceConfig,
    numeric_exact_size: Option<u64>,
    route: Option<crate::PlannerRoute>,
) -> AceResult<PlannerDecision> {
    use ace_cost::{adaptive_top_k, CandidateEstimator, CostModelV3, DefaultCandidateEstimator, SamplePolicy};
    use crate::{
        DefaultPlannerFastPath, EstimateConfidence, PlannerDataClass, PlannerFastPath, PlanningBudget,
    };

    if let Some(plan) = DefaultPlannerFastPath.try_plan(profile, config) {
        return Ok(PlannerDecision {
            plan: plan.clone(),
            telemetry: PlannerTelemetry {
                fast_path_hit: true,
                estimated_candidates: 0,
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
                best_blended_size_bytes: 0,
                quality_limit_bytes: 0,
                selected_blended_size_bytes: 0,
                selected_size_rank: 1,
                selected_cost_rank: 1,
                full_trial_encodes: 0,
            },
            analytical_ranked_plans: vec![plan.clone()],
            top_k_plans: Vec::new(),
            stage_one_ranked_plans: vec![plan.clone()],
            second_stage_plans: Vec::new(),
            final_ranked_plans: vec![plan.clone()],
            quality_qualified_plans: vec![plan.clone()],
            numeric_estimate: None,
        });
    }
    if candidates.is_empty() { return Err(AceError::Malformed("planner generated no candidates")); }

    let estimator = DefaultCandidateEstimator;
    let model = CostModelV3;
    let mut estimated = candidates
        .iter()
        .filter(|candidate| {
            if let Some(route) = route {
                crate::RoutePolicy::candidate_allowed(route, candidate, profile, config)
            } else {
                // Frozen V3.6 path never receives the ACE 0.4 Numeric candidate.
                !matches!(candidate.decoding.codec, CodecId::Numeric) || numeric_exact_size.is_some()
            }
        })
        .map(|candidate| estimator.estimate(candidate, profile, config.profile))
        .collect::<Vec<_>>();

    // Planner V4 numeric specialization has an exact deterministic size estimator. Override only
    // the Numeric candidate; every generic candidate retains the frozen V3.6 analytical model.
    if let Some(exact_size) = numeric_exact_size {
        for candidate in estimated.iter_mut().filter(|candidate| {
            matches!(candidate.plan.decoding.codec, CodecId::Numeric)
        }) {
            candidate.analytical_size_bytes = exact_size;
            candidate.sampled_size_bytes = None;
            candidate.blended_size_bytes = exact_size;
            candidate.cost.predicted_size_bytes = exact_size;
            candidate.cost.metadata_bytes = ace_codecs::NUMERIC_HEADER_SIZE as u64;
            candidate.confidence = 0.99;
            candidate.score = model.score(config.profile, candidate.cost, input.len());
        }
    }

    estimated.sort_by(estimated_order);

    let mut _numeric_margin = None;
    let mut hybrid_policy = crate::RouteHybridLzPolicy::Full;
    let mut route_budget = route
        .map(|r| crate::RoutePolicy::budget(r, config))
        .unwrap_or_else(|| crate::RouteBudget::for_route(crate::PlannerRoute::Generic, config.profile));

    if matches!(route, Some(crate::PlannerRoute::NumericGeneral)) {
        if let Some(numeric_size) = numeric_exact_size {
            let best_generic = estimated
                .iter()
                .filter(|candidate| !matches!(candidate.plan.decoding.codec, CodecId::Numeric))
                .map(|candidate| candidate.blended_size_bytes)
                .min()
                .unwrap_or(u64::MAX);
            if best_generic != u64::MAX {
                let margin = crate::NumericMargin::new(numeric_size, best_generic);
                hybrid_policy = margin.hybrid_lz_policy(config.profile);
                _numeric_margin = Some(margin);
            }
        }
        estimated = reduce_numeric_general_candidates(
            estimated,
            route_budget.max_generated_candidates,
        );
        if matches!(hybrid_policy, crate::RouteHybridLzPolicy::Disabled) {
            route_budget.max_sampled_candidates = route_budget.max_sampled_candidates.min(1);
        }
    }

    let analytical_ranked_plans = estimated.iter().map(|candidate| candidate.plan.clone()).collect::<Vec<_>>();

    let policy = SamplePolicy::for_profile(config.profile);
    let confidence = estimated.first().map(|c| c.confidence).unwrap_or(0.0);
    let adaptive_k = adaptive_top_k(config.profile, confidence, policy.top_k, estimated.len())
        .min(route_budget.max_sampled_candidates.max(1));
    let mut stage_one_pool = quality_preserving_pool(&estimated, adaptive_k, config.profile);
    if matches!(route, Some(crate::PlannerRoute::NumericGeneral)) {
        stage_one_pool = cap_numeric_general_verification_pool(
            stage_one_pool,
            route_budget.max_sampled_candidates,
        );
    }
    let top_k_plans = stage_one_pool.iter().map(|c| c.plan.clone()).collect::<Vec<_>>();
    let hybrid = crate::HybridLzEstimator;
    let data_class = PlannerDataClass::classify(profile);
    let planning_budget = PlanningBudget::for_block(config.profile, data_class);
    let mut hybrid_lz_candidates = 0usize;
    let mut hybrid_lz_stage1_candidates = 0usize;
    let mut hybrid_lz_stage2_candidates = 0usize;
    let mut hybrid_lz_skipped_candidates = 0usize;
    let mut hybrid_lz_high_confidence_skips = 0usize;
    let mut hybrid_lz_sample_bytes = 0usize;
    let mut hybrid_lz_max_disagreement_ppm = 0u64;

    let mut stage_one = Vec::with_capacity(stage_one_pool.len());
    let mut stage1_lz_used = 0usize;
    for candidate in stage_one_pool {
        if matches!(candidate.plan.decoding.codec, CodecId::Lz) {
            let stage1_limit = match hybrid_policy {
                crate::RouteHybridLzPolicy::Disabled => 0,
                crate::RouteHybridLzPolicy::OneStage => 1,
                crate::RouteHybridLzPolicy::Full => planning_budget.hybrid_stage1_candidates,
            };
            if stage1_lz_used < stage1_limit {
                let (refined, observation) = hybrid.refine(input, candidate, config.profile, 1, &model)?;
                stage1_lz_used = stage1_lz_used.saturating_add(1);
                hybrid_lz_candidates = hybrid_lz_candidates.saturating_add(1);
                hybrid_lz_stage1_candidates = hybrid_lz_stage1_candidates.saturating_add(1);
                hybrid_lz_sample_bytes = hybrid_lz_sample_bytes.saturating_add(observation.sampled_input_bytes);
                hybrid_lz_max_disagreement_ppm = hybrid_lz_max_disagreement_ppm.max(observation.disagreement_ppm);
                stage_one.push(refined);
            } else {
                hybrid_lz_skipped_candidates = hybrid_lz_skipped_candidates.saturating_add(1);
                stage_one.push(candidate);
            }
        } else if matches!(candidate.plan.decoding.codec, CodecId::Numeric)
            && numeric_exact_size.is_some()
        {
            // Exact Numeric estimates are already full-block deterministic; re-encoding samples
            // only adds planner cost and cannot improve the size projection.
            stage_one.push(candidate);
        } else {
            stage_one.push(verify_candidate_samples(
                input, candidate, config.profile, policy, 1, &model,
            )?);
        }
    }
    EntropySelectionPolicy::for_profile(config.profile).penalize_weak_rans(&mut stage_one);
    stage_one.sort_by(estimated_order);
    let stage_one_ranked_plans = stage_one.iter().map(|candidate| candidate.plan.clone()).collect::<Vec<_>>();

    let stage_two_k = if route_budget.allow_second_stage
        && !matches!(hybrid_policy, crate::RouteHybridLzPolicy::Disabled | crate::RouteHybridLzPolicy::OneStage)
    {
        adaptive_second_stage_k(&stage_one, config.profile, policy.second_stage_top_k)
    } else {
        0
    };
    let second_stage_plans = stage_one.iter().take(stage_two_k).map(|c| c.plan.clone()).collect::<Vec<_>>();

    // Planner V3.6 remains ranking-only: stage two may refine a candidate's score, but it does not
    // remove candidates that survived stage one. This preserves Top-K quality while retaining
    // zero full-block trial encodes. Candidates outside the stage-two budget keep their stage-one
    // score and remain eligible for the final deterministic ranking.
    let mut final_pool = Vec::with_capacity(stage_one.len());
    let mut stage2_lz_used = 0usize;
    for (idx, candidate) in stage_one.into_iter().enumerate() {
        if idx < stage_two_k {
            if matches!(candidate.plan.decoding.codec, CodecId::Lz) {
                let confidence = EstimateConfidence::from_candidate(&candidate);
                if !confidence.needs_second_stage() {
                    hybrid_lz_high_confidence_skips = hybrid_lz_high_confidence_skips.saturating_add(1);
                    final_pool.push(candidate);
                } else if stage2_lz_used < planning_budget.hybrid_stage2_candidates {
                    let (refined, observation) = hybrid.refine(input, candidate, config.profile, 2, &model)?;
                    stage2_lz_used = stage2_lz_used.saturating_add(1);
                    hybrid_lz_candidates = hybrid_lz_candidates.saturating_add(1);
                    hybrid_lz_stage2_candidates = hybrid_lz_stage2_candidates.saturating_add(1);
                    hybrid_lz_sample_bytes = hybrid_lz_sample_bytes.saturating_add(observation.sampled_input_bytes);
                    hybrid_lz_max_disagreement_ppm = hybrid_lz_max_disagreement_ppm.max(observation.disagreement_ppm);
                    final_pool.push(refined);
                } else {
                    hybrid_lz_skipped_candidates = hybrid_lz_skipped_candidates.saturating_add(1);
                    final_pool.push(candidate);
                }
            } else {
                final_pool.push(verify_candidate_samples(
                    input, candidate, config.profile, policy, 2, &model,
                )?);
            }
        } else {
            final_pool.push(candidate);
        }
    }
    EntropySelectionPolicy::for_profile(config.profile).penalize_weak_rans(&mut final_pool);
    final_pool.sort_by(estimated_order);

    let final_ranked_plans = final_pool.iter().map(|candidate| candidate.plan.clone()).collect::<Vec<_>>();
    let envelope = ace_cost::QualityEnvelope::for_profile(config.profile);
    let best_blended_size_bytes = final_pool
        .iter()
        .map(|candidate| candidate.blended_size_bytes)
        .min()
        .ok_or(AceError::Malformed("sample verifier produced no candidates"))?;
    let quality_limit_bytes = envelope.limit_bytes(best_blended_size_bytes);
    let mut qualified = envelope.qualify(&final_pool);
    if qualified.is_empty() {
        return Err(AceError::Malformed("quality envelope produced no candidates"));
    }
    qualified.sort_by(estimated_order);
    let quality_qualified_plans = qualified.iter().map(|candidate| candidate.plan.clone()).collect::<Vec<_>>();
    let selected_candidate = qualified
        .first()
        .cloned()
        .ok_or(AceError::Malformed("quality envelope produced no selected candidate"))?;

    let selected_cost_rank = final_pool
        .iter()
        .position(|candidate| same_plan_semantics(&candidate.plan, &selected_candidate.plan))
        .map(|idx| idx + 1)
        .unwrap_or(1);
    let mut size_ranked = final_pool.clone();
    size_ranked.sort_by(|a, b| {
        a.blended_size_bytes
            .cmp(&b.blended_size_bytes)
            .then_with(|| stable_plan_key(&a.plan).cmp(&stable_plan_key(&b.plan)))
    });
    let selected_size_rank = size_ranked
        .iter()
        .position(|candidate| same_plan_semantics(&candidate.plan, &selected_candidate.plan))
        .map(|idx| idx + 1)
        .unwrap_or(1);

    let selected_blended_size_bytes = selected_candidate.blended_size_bytes;
    let mut selected = selected_candidate.plan;
    selected.cost = selected_candidate.cost;
    selected.score = selected_candidate.score;

    Ok(PlannerDecision {
        plan: selected,
        telemetry: PlannerTelemetry {
            fast_path_hit: false,
            estimated_candidates: analytical_ranked_plans.len(),
            sampled_candidates: top_k_plans.len(),
            second_stage_candidates: second_stage_plans.len(),
            hybrid_lz_candidates,
            hybrid_lz_stage1_candidates,
            hybrid_lz_stage2_candidates,
            hybrid_lz_skipped_candidates,
            hybrid_lz_high_confidence_skips,
            hybrid_lz_sample_bytes,
            hybrid_lz_max_disagreement_ppm,
            quality_qualified_candidates: quality_qualified_plans.len(),
            best_blended_size_bytes,
            quality_limit_bytes,
            selected_blended_size_bytes,
            selected_size_rank,
            selected_cost_rank,
            full_trial_encodes: 0,
        },
        analytical_ranked_plans,
        top_k_plans,
        stage_one_ranked_plans,
        second_stage_plans,
        final_ranked_plans,
        quality_qualified_plans,
        numeric_estimate: None,
    })
}

/// Reduces NumericGeneral analytical candidates to a bounded semantic family set.
///
/// Numeric is retained first when present. The remaining slots are filled in analytical order
/// while admitting at most one RAW, one RLE, one LZ-Fast and one LZ-Balanced candidate. This
/// prevents entropy variants from consuming the entire NumericGeneral budget.
fn reduce_numeric_general_candidates(
    estimated: Vec<ace_cost::EstimatedCandidate>,
    max_candidates: usize,
) -> Vec<ace_cost::EstimatedCandidate> {
    let mut reduced = Vec::with_capacity(max_candidates.min(estimated.len()));
    let mut have_numeric = false;
    let mut have_raw = false;
    let mut have_rle = false;
    let mut have_lz_fast = false;
    let mut have_lz_balanced = false;

    for candidate in estimated {
        if reduced.len() >= max_candidates {
            break;
        }

        let admit = match candidate.plan.decoding.codec {
            CodecId::Numeric if !have_numeric => {
                have_numeric = true;
                true
            }
            CodecId::Raw if !have_raw => {
                have_raw = true;
                true
            }
            CodecId::Rle if !have_rle => {
                have_rle = true;
                true
            }
            CodecId::Lz if matches!(candidate.plan.lz_mode, Some(LzMode::Fast)) && !have_lz_fast => {
                have_lz_fast = true;
                true
            }
            CodecId::Lz
                if matches!(candidate.plan.lz_mode, Some(LzMode::Balanced))
                    && !have_lz_balanced =>
            {
                have_lz_balanced = true;
                true
            }
            _ => false,
        };
        if admit {
            reduced.push(candidate);
        }
    }

    reduced.sort_by(estimated_order);
    reduced
}

/// Caps NumericGeneral verification after quality anchors have been added.
///
/// The exact Numeric candidate is retained when present; the remaining slots are filled in stable
/// estimated order. This makes the route's sample budget a real invariant rather than only an
/// adaptive-Top-K hint.
fn cap_numeric_general_verification_pool(
    pool: Vec<ace_cost::EstimatedCandidate>,
    max_candidates: usize,
) -> Vec<ace_cost::EstimatedCandidate> {
    if pool.len() <= max_candidates {
        return pool;
    }

    let mut capped = Vec::with_capacity(max_candidates);
    if let Some(numeric) = pool
        .iter()
        .find(|candidate| matches!(candidate.plan.decoding.codec, CodecId::Numeric))
        .cloned()
    {
        capped.push(numeric);
    }

    for candidate in pool {
        if capped.len() >= max_candidates {
            break;
        }
        if capped.iter().any(|existing| same_plan_semantics(&existing.plan, &candidate.plan)) {
            continue;
        }
        capped.push(candidate);
    }
    capped.sort_by(estimated_order);
    capped
}

/// Deterministically orders analytical/sample estimates.
fn estimated_order(a: &ace_cost::EstimatedCandidate, b: &ace_cost::EstimatedCandidate) -> std::cmp::Ordering {
    a.score.cmp(&b.score).then_with(|| stable_plan_key(&a.plan).cmp(&stable_plan_key(&b.plan)))
}

/// Builds stage-one verification input from Top-K plus semantic-family quality anchors.
///
/// The quality anchors prevent a bad scalar estimate from eliminating every member of an
/// otherwise promising codec/transform family before sampling. FAST remains narrow; BALANCED
/// and DENSE add one best analytical representative per semantic family.
fn quality_preserving_pool(
    estimated: &[ace_cost::EstimatedCandidate],
    adaptive_k: usize,
    profile: CompressionProfile,
) -> Vec<ace_cost::EstimatedCandidate> {
    let mut pool = estimated.iter().take(adaptive_k).cloned().collect::<Vec<_>>();
    if matches!(profile, CompressionProfile::Fast) { return pool; }

    let cap = match profile {
        CompressionProfile::Fast => adaptive_k.max(2),
        CompressionProfile::Balanced => 8,
        CompressionProfile::Dense => 10,
    }.min(estimated.len());

    for candidate in estimated {
        if pool.len() >= cap { break; }
        if pool.iter().any(|p| same_plan_semantics(&p.plan, &candidate.plan)) { continue; }
        let family = semantic_family_key(&candidate.plan);
        if pool.iter().any(|p| semantic_family_key(&p.plan) == family) { continue; }
        pool.push(candidate.clone());
    }

    pool.sort_by(estimated_order);
    pool
}

/// Applies non-LZ sample verification while retaining analytical, sampled and blended size diagnostics.
///
/// LZ candidates are handled by [`crate::HybridLzEstimator`] so reset-window pessimism cannot
/// dominate the buildfix6 full-block analytical model.
fn verify_candidate_samples(
    input: &[u8],
    mut candidate: ace_cost::EstimatedCandidate,
    profile: CompressionProfile,
    policy: ace_cost::SamplePolicy,
    stage: u8,
    model: &ace_cost::CostModelV3,
) -> AceResult<ace_cost::EstimatedCandidate> {
    let ranges = ace_cost::codec_sample_ranges(input.len(), policy, candidate.plan.decoding.codec, stage);
    if ranges.is_empty() { return Ok(candidate); }

    let analytical_size = candidate.analytical_size_bytes;
    let mut sample_input = 0usize;
    let mut sample_payload = 0usize;
    let mut metadata_once = 0usize;
    for range in ranges {
        let sample = &input[range];
        let (metadata, payload, _) = encode_plan_payload(sample, &candidate.plan)?;
        sample_input = sample_input.saturating_add(sample.len());
        if matches!(candidate.plan.decoding.codec, CodecId::Numeric) {
            // Every independently encoded sample contains its own fixed NUM1 header. Project only
            // the packed body to full-block size and charge the header exactly once.
            sample_payload = sample_payload.saturating_add(payload.len().saturating_sub(ace_codecs::NUMERIC_HEADER_SIZE));
            metadata_once = metadata_once.max(ace_codecs::NUMERIC_HEADER_SIZE);
        } else {
            sample_payload = sample_payload.saturating_add(payload.len());
            metadata_once = metadata_once.max(metadata.len());
        }
    }

    if sample_input != 0 {
        let projected_payload = ((sample_payload as u128)
            .saturating_mul(input.len() as u128)
            / sample_input as u128)
            .min(u64::MAX as u128) as u64;
        let sample_projected = projected_payload.saturating_add(metadata_once as u64);
        let (sample_weight, analytical_weight) = verification_blend_weights(
            candidate.plan.decoding.codec, candidate.confidence, stage, profile,
        );
        let blended = ((sample_projected as u128)
            .saturating_mul(sample_weight as u128)
            .saturating_add((analytical_size as u128).saturating_mul(analytical_weight as u128))
            / 100u128)
            .min(u64::MAX as u128) as u64;
        candidate.sampled_size_bytes = Some(sample_projected);
        candidate.blended_size_bytes = blended;
        candidate.cost.predicted_size_bytes = blended;
        candidate.cost.metadata_bytes = metadata_once as u64;
        candidate.score = model.score(profile, candidate.cost, input.len());
    }
    Ok(candidate)
}

/// Returns deterministic sample/analytical blending weights for non-LZ candidates in Planner V3.6.
///
/// High-confidence analytical estimates retain most of the authority. LZ always receives a
/// stronger analytical weight because short windows cannot faithfully reproduce long-range
/// match opportunities. DENSE is intentionally conservative: sampling refines ranking rather
/// than replacing full-block statistics.
fn verification_blend_weights(
    codec: CodecId,
    confidence: f32,
    stage: u8,
    profile: CompressionProfile,
) -> (u64, u64) {
    let high = confidence >= 0.90;
    let medium = confidence >= 0.75;
    let sample: u64 = match codec {
        CodecId::Lz => {
            if high { 20 } else if medium { 25 } else { 30 }
        }
        _ => {
            if high { 25 } else if medium { 40 } else { 55 }
        }
    };
    let stage_adjusted = if stage >= 2 { sample.saturating_add(5).min(60) } else { sample };
    let profile_adjusted = match profile {
        CompressionProfile::Dense => stage_adjusted.saturating_sub(5),
        CompressionProfile::Balanced => stage_adjusted,
        CompressionProfile::Fast => stage_adjusted.saturating_add(5).min(65),
    };
    (profile_adjusted, 100 - profile_adjusted)
}

/// Chooses the stage-two width and widens it when the leading estimates remain ambiguous.
fn adaptive_second_stage_k(
    candidates: &[ace_cost::EstimatedCandidate],
    profile: CompressionProfile,
    nominal: usize,
) -> usize {
    if candidates.is_empty() { return 0; }
    let mut k = nominal.max(1).min(candidates.len());
    if candidates.len() >= 2 {
        let best = candidates[0].cost.predicted_size_bytes.max(1) as f64;
        let second = candidates[1].cost.predicted_size_bytes as f64;
        let margin = ((second - best).max(0.0) / best) as f32;
        if margin < 0.03 || candidates[0].confidence < 0.80 {
            k = (k + 1).min(candidates.len());
        }
    }
    if matches!(profile, CompressionProfile::Dense) {
        k = k.max(3).min(candidates.len());
    }
    k
}

#[cfg(test)]
mod buildfix5_tests {
    use super::*;

    /// Ensures high-confidence LZ verification trusts full-block statistics more than samples.
    #[test]
    fn high_confidence_lz_prefers_analytical_weight() {
        let (sample, analytical) = verification_blend_weights(
            CodecId::Lz, 0.95, 2, CompressionProfile::Dense,
        );
        assert!(analytical > sample);
        assert_eq!(sample + analytical, 100);
    }

    /// Ensures low-confidence non-LZ verification still keeps analytical evidence in the blend.
    #[test]
    fn low_confidence_sampling_never_fully_replaces_estimate() {
        let (sample, analytical) = verification_blend_weights(
            CodecId::Raw, 0.40, 2, CompressionProfile::Balanced,
        );
        assert!(sample < 100);
        assert!(analytical > 0);
        assert_eq!(sample + analytical, 100);
    }

    /// Verifies all supported profile, codec, confidence, and stage combinations produce valid percentages.
    #[test]
    fn verification_blend_weights_are_valid_percentages() {
        for profile in [
            CompressionProfile::Fast,
            CompressionProfile::Balanced,
            CompressionProfile::Dense,
        ] {
            for codec in [CodecId::Raw, CodecId::Rle, CodecId::Lz] {
                for confidence in [0.40_f32, 0.80_f32, 0.95_f32] {
                    for stage in [1_u8, 2_u8] {
                        let (sample, analytical) =
                            verification_blend_weights(codec, confidence, stage, profile);
                        assert!(sample <= 100);
                        assert!(analytical <= 100);
                        assert_eq!(sample + analytical, 100);
                    }
                }
            }
        }
    }
}
