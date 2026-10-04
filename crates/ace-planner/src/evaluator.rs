use crate::{DeterministicCostModel, EntropySelectionPolicy};
use ace_codecs::encode_codec;
use ace_core::{
    AceConfig, AceError, AceResult, CodecId, CompressionProfile, EntropyCodecId, LzMode,
    PhysicalCompressionPlan,
};
use ace_entropy::encode_entropy;
use ace_transforms::apply_transform;

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
        let metadata_prefix = if matches!(candidate.decoding.entropy, EntropyCodecId::None) {
            0
        } else {
            4
        };
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
    apply_entropy_selection_policy(&mut evaluated, config.profile);
    Ok(evaluated)
}

/// Executes only the reversible payload pipeline of one plan and returns entropy metadata, payload and primary-stream size.
pub fn encode_plan_payload(
    input: &[u8],
    plan: &PhysicalCompressionPlan,
) -> AceResult<(Vec<u8>, Vec<u8>, usize)> {
    let mut stage = input.to_vec();
    for &transform in &plan.decoding.transforms {
        stage = apply_transform(transform, &stage)?;
    }
    stage = encode_codec(plan.decoding.codec, plan.lz_mode, &stage)?;
    let primary_len = stage.len();
    let (metadata, payload) = encode_entropy(plan.decoding.entropy, &stage)?;
    Ok((metadata, payload, primary_len))
}

/// Produces a stable tie-break key that does not depend on address, thread scheduling or hash iteration order.
pub fn stable_plan_key(plan: &PhysicalCompressionPlan) -> (Vec<u8>, u8, u8, u8) {
    let transforms = plan
        .decoding
        .transforms
        .iter()
        .map(|t| *t as u8)
        .collect::<Vec<_>>();
    let lz = match plan.lz_mode {
        None => 0,
        Some(LzMode::Fast) => 1,
        Some(LzMode::Balanced) => 2,
    };
    (
        transforms,
        plan.decoding.codec as u8,
        plan.decoding.entropy as u8,
        lz,
    )
}

/// Returns true when two physical plans have identical decoder semantics.
pub fn same_plan_semantics(a: &PhysicalCompressionPlan, b: &PhysicalCompressionPlan) -> bool {
    a.decoding == b.decoding && a.lz_mode == b.lz_mode
}

/// Penalizes rANS plans that do not beat an equivalent Huffman pipeline by the profile-specific minimum gain.
fn apply_entropy_selection_policy(
    plans: &mut [PhysicalCompressionPlan],
    profile: CompressionProfile,
) {
    let policy = EntropySelectionPolicy::for_profile(profile);
    let snapshot = plans.to_vec();
    for plan in plans.iter_mut().filter(|p| {
        matches!(
            p.decoding.entropy,
            EntropyCodecId::Rans | EntropyCodecId::Rans4x
        )
    }) {
        let peer = snapshot.iter().find(|other| {
            matches!(other.decoding.entropy, EntropyCodecId::Huffman)
                && other.decoding.transforms == plan.decoding.transforms
                && other.decoding.codec == plan.decoding.codec
                && other.lz_mode == plan.lz_mode
        });
        if let Some(huffman) = peer {
            let h = huffman.cost.predicted_size_bytes.max(1) as f64;
            let r = plan.cost.predicted_size_bytes as f64;
            let gain = ((h - r) / h).max(0.0) as f32;
            if gain < policy.min_rans_gain_fraction {
                plan.score = plan.score.saturating_add(u128::MAX / 4);
            }
        }
    }
}

/// Planner V3.5 telemetry used by engine statistics, `ace explain` and benchmarks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlannerTelemetry {
    /// True when a deterministic fast path selected the plan without candidate sampling.
    pub fast_path_hit: bool,
    /// Number of candidates ranked by the analytical estimator.
    pub estimated_candidates: usize,
    /// Number of candidates entering stage-one sample verification.
    pub sampled_candidates: usize,
    /// Number of candidates entering the larger second-stage verifier.
    pub second_stage_candidates: usize,
    /// Number of LZ candidates refined by bounded production-codec micro-trials.
    pub hybrid_lz_candidates: usize,
    /// Total source bytes processed by bounded LZ micro-trials for this block.
    pub hybrid_lz_sample_bytes: usize,
    /// Largest analytical-vs-micro-trial LZ disagreement observed, in parts per million.
    pub hybrid_lz_max_disagreement_ppm: u64,
    /// Number of candidates that passed the profile-specific quality envelope.
    pub quality_qualified_candidates: usize,
    /// Smallest blended size observed in the final candidate pool.
    pub best_blended_size_bytes: u64,
    /// Inclusive maximum blended size admitted by the active quality envelope.
    pub quality_limit_bytes: u64,
    /// Blended size of the plan finally selected by the scalar cost model.
    pub selected_blended_size_bytes: u64,
    /// One-based size rank of the selected plan in the full post-sampling pool.
    pub selected_size_rank: usize,
    /// One-based scalar-cost rank of the selected plan in the full post-sampling pool.
    pub selected_cost_rank: usize,
    /// Number of full candidate trial encodes performed by the hot-path planner.
    pub full_trial_encodes: usize,
}

/// Result returned by the ACE 0.3-buildfix8 Planner V3.5 hot path.
#[derive(Debug, Clone)]
pub struct PlannerDecision {
    /// Selected physical compression plan.
    pub plan: PhysicalCompressionPlan,
    /// Work performed while reaching the decision.
    pub telemetry: PlannerTelemetry,
    /// Complete analytical ranking before sample verification.
    pub analytical_ranked_plans: Vec<PhysicalCompressionPlan>,
    /// Plans that survived analytical pruning and entered stage-one verification.
    pub top_k_plans: Vec<PhysicalCompressionPlan>,
    /// Complete ranking after stage-one sample verification.
    pub stage_one_ranked_plans: Vec<PhysicalCompressionPlan>,
    /// Plans selected for the larger second-stage verifier.
    pub second_stage_plans: Vec<PhysicalCompressionPlan>,
    /// Complete post-sampling ranking before quality-envelope filtering.
    pub final_ranked_plans: Vec<PhysicalCompressionPlan>,
    /// Plans admitted by the profile-specific quality envelope.
    pub quality_qualified_plans: Vec<PhysicalCompressionPlan>,
}

/// Estimates all candidates, verifies a quality-preserving adaptive pool and selects a plan.
///
/// ACE 0.3-buildfix8 starts from buildfix6, retains zero full trials and the quality envelope, and replaces LZ reset-window verification with a bounded hybrid production-codec micro-trial.
/// Adaptive Top-K and semantic-family anchors define the search pool; sampling refines
/// scores but never removes a stage-one survivor. The final QualityEnvelope first filters by
/// blended compressed size, then the cost model selects the cheapest quality-safe plan.
pub fn evaluate_candidates_v3(
    input: &[u8],
    profile: &ace_core::BlockProfile,
    candidates: &[PhysicalCompressionPlan],
    config: &AceConfig,
) -> AceResult<PlannerDecision> {
    use crate::{DefaultPlannerFastPath, PlannerFastPath};
    use ace_cost::{
        adaptive_top_k, CandidateEstimator, CostModelV3, DefaultCandidateEstimator, SamplePolicy,
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
        });
    }
    if candidates.is_empty() {
        return Err(AceError::Malformed("planner generated no candidates"));
    }

    let estimator = DefaultCandidateEstimator;
    let mut estimated = candidates
        .iter()
        .map(|candidate| estimator.estimate(candidate, profile, config.profile))
        .collect::<Vec<_>>();
    estimated.sort_by(estimated_order);
    let analytical_ranked_plans = estimated
        .iter()
        .map(|candidate| candidate.plan.clone())
        .collect::<Vec<_>>();

    let policy = SamplePolicy::for_profile(config.profile);
    let confidence = estimated.first().map(|c| c.confidence).unwrap_or(0.0);
    let adaptive_k = adaptive_top_k(config.profile, confidence, policy.top_k, estimated.len());
    let stage_one_pool = quality_preserving_pool(&estimated, adaptive_k, config.profile);
    let top_k_plans = stage_one_pool
        .iter()
        .map(|c| c.plan.clone())
        .collect::<Vec<_>>();
    let model = CostModelV3;
    let hybrid = crate::HybridLzEstimator;
    let mut hybrid_lz_candidates = 0usize;
    let mut hybrid_lz_sample_bytes = 0usize;
    let mut hybrid_lz_max_disagreement_ppm = 0u64;

    let mut stage_one = Vec::with_capacity(stage_one_pool.len());
    for candidate in stage_one_pool {
        if matches!(candidate.plan.decoding.codec, CodecId::Lz) {
            let (refined, observation) =
                hybrid.refine(input, candidate, config.profile, 1, &model)?;
            hybrid_lz_candidates = hybrid_lz_candidates.saturating_add(1);
            hybrid_lz_sample_bytes =
                hybrid_lz_sample_bytes.saturating_add(observation.sampled_input_bytes);
            hybrid_lz_max_disagreement_ppm =
                hybrid_lz_max_disagreement_ppm.max(observation.disagreement_ppm);
            stage_one.push(refined);
        } else {
            stage_one.push(verify_candidate_samples(
                input,
                candidate,
                config.profile,
                policy,
                1,
                &model,
            )?);
        }
    }
    apply_entropy_policy_estimates(&mut stage_one, config.profile);
    stage_one.sort_by(estimated_order);
    let stage_one_ranked_plans = stage_one
        .iter()
        .map(|candidate| candidate.plan.clone())
        .collect::<Vec<_>>();

    let stage_two_k =
        adaptive_second_stage_k(&stage_one, config.profile, policy.second_stage_top_k);
    let second_stage_plans = stage_one
        .iter()
        .take(stage_two_k)
        .map(|c| c.plan.clone())
        .collect::<Vec<_>>();

    // Planner V3.5 is ranking-only: stage two may refine a candidate's score, but it does not
    // remove candidates that survived stage one. This preserves Top-K quality while retaining
    // zero full-block trial encodes. Candidates outside the stage-two budget keep their stage-one
    // score and remain eligible for the final deterministic ranking.
    let mut final_pool = Vec::with_capacity(stage_one.len());
    for (idx, candidate) in stage_one.into_iter().enumerate() {
        if idx < stage_two_k {
            if matches!(candidate.plan.decoding.codec, CodecId::Lz) {
                let (refined, observation) =
                    hybrid.refine(input, candidate, config.profile, 2, &model)?;
                hybrid_lz_candidates = hybrid_lz_candidates.saturating_add(1);
                hybrid_lz_sample_bytes =
                    hybrid_lz_sample_bytes.saturating_add(observation.sampled_input_bytes);
                hybrid_lz_max_disagreement_ppm =
                    hybrid_lz_max_disagreement_ppm.max(observation.disagreement_ppm);
                final_pool.push(refined);
            } else {
                final_pool.push(verify_candidate_samples(
                    input,
                    candidate,
                    config.profile,
                    policy,
                    2,
                    &model,
                )?);
            }
        } else {
            final_pool.push(candidate);
        }
    }
    apply_entropy_policy_estimates(&mut final_pool, config.profile);
    final_pool.sort_by(estimated_order);

    let final_ranked_plans = final_pool
        .iter()
        .map(|candidate| candidate.plan.clone())
        .collect::<Vec<_>>();
    let envelope = ace_cost::QualityEnvelope::for_profile(config.profile);
    let best_blended_size_bytes = final_pool
        .iter()
        .map(|candidate| candidate.blended_size_bytes)
        .min()
        .ok_or(AceError::Malformed(
            "sample verifier produced no candidates",
        ))?;
    let quality_limit_bytes = envelope.limit_bytes(best_blended_size_bytes);
    let mut qualified = envelope.qualify(&final_pool);
    if qualified.is_empty() {
        return Err(AceError::Malformed(
            "quality envelope produced no candidates",
        ));
    }
    qualified.sort_by(estimated_order);
    let quality_qualified_plans = qualified
        .iter()
        .map(|candidate| candidate.plan.clone())
        .collect::<Vec<_>>();
    let selected_candidate = qualified.first().cloned().ok_or(AceError::Malformed(
        "quality envelope produced no selected candidate",
    ))?;

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
            estimated_candidates: candidates.len(),
            sampled_candidates: top_k_plans.len(),
            second_stage_candidates: second_stage_plans.len(),
            hybrid_lz_candidates,
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
    })
}

/// Deterministically orders analytical/sample estimates.
fn estimated_order(
    a: &ace_cost::EstimatedCandidate,
    b: &ace_cost::EstimatedCandidate,
) -> std::cmp::Ordering {
    a.score
        .cmp(&b.score)
        .then_with(|| stable_plan_key(&a.plan).cmp(&stable_plan_key(&b.plan)))
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
    let mut pool = estimated
        .iter()
        .take(adaptive_k)
        .cloned()
        .collect::<Vec<_>>();
    if matches!(profile, CompressionProfile::Fast) {
        return pool;
    }

    let cap = match profile {
        CompressionProfile::Fast => adaptive_k.max(2),
        CompressionProfile::Balanced => 8,
        CompressionProfile::Dense => 10,
    }
    .min(estimated.len());

    for candidate in estimated {
        if pool.len() >= cap {
            break;
        }
        if pool
            .iter()
            .any(|p| same_plan_semantics(&p.plan, &candidate.plan))
        {
            continue;
        }
        let family = semantic_family_key(&candidate.plan);
        if pool.iter().any(|p| semantic_family_key(&p.plan) == family) {
            continue;
        }
        pool.push(candidate.clone());
    }

    pool.sort_by(estimated_order);
    pool
}

/// Groups entropy variants of the same physical transform/primary-codec family.
fn semantic_family_key(plan: &PhysicalCompressionPlan) -> (Vec<u8>, u8, u8) {
    let transforms = plan
        .decoding
        .transforms
        .iter()
        .map(|t| *t as u8)
        .collect::<Vec<_>>();
    let lz = match plan.lz_mode {
        None => 0,
        Some(LzMode::Fast) => 1,
        Some(LzMode::Balanced) => 2,
    };
    (transforms, plan.decoding.codec as u8, lz)
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
    let ranges =
        ace_cost::codec_sample_ranges(input.len(), policy, candidate.plan.decoding.codec, stage);
    if ranges.is_empty() {
        return Ok(candidate);
    }

    let analytical_size = candidate.analytical_size_bytes;
    let mut sample_input = 0usize;
    let mut sample_payload = 0usize;
    let mut metadata_once = 0usize;
    for range in ranges {
        let sample = &input[range];
        let (metadata, payload, _) = encode_plan_payload(sample, &candidate.plan)?;
        sample_input = sample_input.saturating_add(sample.len());
        sample_payload = sample_payload.saturating_add(payload.len());
        metadata_once = metadata_once.max(metadata.len());
    }

    if sample_input != 0 {
        let projected_payload = ((sample_payload as u128).saturating_mul(input.len() as u128)
            / sample_input as u128)
            .min(u64::MAX as u128) as u64;
        let sample_projected = projected_payload.saturating_add(metadata_once as u64);
        let (sample_weight, analytical_weight) = verification_blend_weights(
            candidate.plan.decoding.codec,
            candidate.confidence,
            stage,
            profile,
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

/// Returns deterministic sample/analytical blending weights for non-LZ candidates in Planner V3.5.
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
            if high {
                20
            } else if medium {
                25
            } else {
                30
            }
        }
        _ => {
            if high {
                25
            } else if medium {
                40
            } else {
                55
            }
        }
    };
    let stage_adjusted = if stage >= 2 {
        sample.saturating_add(5).min(60)
    } else {
        sample
    };
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
    if candidates.is_empty() {
        return 0;
    }
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

/// Applies the entropy-gain policy to estimated candidates without requiring full-block encoding.
fn apply_entropy_policy_estimates(
    plans: &mut [ace_cost::EstimatedCandidate],
    profile: CompressionProfile,
) {
    let policy = EntropySelectionPolicy::for_profile(profile);
    let snapshot = plans.to_vec();
    for plan in plans.iter_mut().filter(|p| {
        matches!(
            p.plan.decoding.entropy,
            EntropyCodecId::Rans | EntropyCodecId::Rans4x
        )
    }) {
        let peer = snapshot.iter().find(|other| {
            matches!(other.plan.decoding.entropy, EntropyCodecId::Huffman)
                && other.plan.decoding.transforms == plan.plan.decoding.transforms
                && other.plan.decoding.codec == plan.plan.decoding.codec
                && other.plan.lz_mode == plan.plan.lz_mode
        });
        if let Some(huffman) = peer {
            let h = huffman.cost.predicted_size_bytes.max(1) as f64;
            let r = plan.cost.predicted_size_bytes as f64;
            let gain = ((h - r) / h).max(0.0) as f32;
            if gain < policy.min_rans_gain_fraction {
                plan.score = plan.score.saturating_add(u128::MAX / 4);
            }
        }
    }
}

#[cfg(test)]
mod buildfix5_tests {
    use super::*;

    /// Ensures high-confidence LZ verification trusts full-block statistics more than samples.
    #[test]
    fn high_confidence_lz_prefers_analytical_weight() {
        let (sample, analytical) =
            verification_blend_weights(CodecId::Lz, 0.95, 2, CompressionProfile::Dense);
        assert!(analytical > sample);
        assert_eq!(sample + analytical, 100);
    }

    /// Ensures low-confidence non-LZ verification still keeps analytical evidence in the blend.
    #[test]
    fn low_confidence_sampling_never_fully_replaces_estimate() {
        let (sample, analytical) =
            verification_blend_weights(CodecId::Raw, 0.40, 2, CompressionProfile::Balanced);
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
