use crate::{DeterministicCostModel, EntropySelectionPolicy};
use ace_codecs::encode_codec;
use ace_core::{AceConfig, AceError, AceResult, EntropyCodecId, PhysicalCompressionPlan};
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
fn stable_plan_key(plan: &PhysicalCompressionPlan) -> (Vec<u8>, u8, u8, u8) {
    let transforms = plan
        .decoding
        .transforms
        .iter()
        .map(|t| *t as u8)
        .collect::<Vec<_>>();
    let lz = match plan.lz_mode {
        None => 0,
        Some(ace_core::LzMode::Fast) => 1,
        Some(ace_core::LzMode::Balanced) => 2,
    };
    (
        transforms,
        plan.decoding.codec as u8,
        plan.decoding.entropy as u8,
        lz,
    )
}

/// Penalizes scalar-rANS plans that do not beat an equivalent Huffman pipeline by the profile-specific minimum gain.
fn apply_entropy_selection_policy(
    plans: &mut [PhysicalCompressionPlan],
    profile: ace_core::CompressionProfile,
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

/// Planner V3 telemetry used by engine statistics, `ace explain` and benchmarks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlannerTelemetry {
    /// True when a deterministic fast path selected the plan without candidate sampling.
    pub fast_path_hit: bool,
    /// Number of candidates ranked by the analytical estimator.
    pub estimated_candidates: usize,
    /// Number of top-K candidates encoded only on deterministic samples.
    pub sampled_candidates: usize,
    /// Number of full candidate trial encodes performed by the hot-path planner.
    pub full_trial_encodes: usize,
}

/// Result returned by the ACE 0.3 planner hot path.
#[derive(Debug, Clone)]
pub struct PlannerDecision {
    /// Selected physical compression plan.
    pub plan: PhysicalCompressionPlan,
    /// Work performed while reaching the decision.
    pub telemetry: PlannerTelemetry,
}

/// Estimates all candidates, verifies only a tiny deterministic top-K sample and selects a plan.
///
/// Unlike ACE 0.2.x this function never trial-encodes a complete block.  The final full-block
/// encode is performed exactly once by the engine after the decision is returned.
pub fn evaluate_candidates_v3(
    input: &[u8],
    profile: &ace_core::BlockProfile,
    candidates: &[PhysicalCompressionPlan],
    config: &AceConfig,
) -> AceResult<PlannerDecision> {
    use crate::{DefaultPlannerFastPath, PlannerFastPath};
    use ace_cost::{
        deterministic_sample_ranges, CandidateEstimator, CostModelV3, DefaultCandidateEstimator,
        SamplePolicy,
    };

    if let Some(plan) = DefaultPlannerFastPath.try_plan(profile, config) {
        return Ok(PlannerDecision {
            plan,
            telemetry: PlannerTelemetry {
                fast_path_hit: true,
                estimated_candidates: 0,
                sampled_candidates: 0,
                full_trial_encodes: 0,
            },
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
    estimated.sort_by(|a, b| {
        a.score
            .cmp(&b.score)
            .then_with(|| stable_plan_key(&a.plan).cmp(&stable_plan_key(&b.plan)))
    });

    let policy = SamplePolicy::for_profile(config.profile);
    let confidence = estimated.first().map(|c| c.confidence).unwrap_or(0.0);
    let adaptive_k = if confidence >= 0.96 {
        1
    } else if confidence >= 0.82 {
        policy.top_k.min(2)
    } else {
        policy.top_k
    };
    let top_k = adaptive_k.max(1).min(estimated.len());
    let ranges = deterministic_sample_ranges(input.len(), policy);
    let model = CostModelV3;

    let mut verified = Vec::with_capacity(top_k);
    for mut candidate in estimated.into_iter().take(top_k) {
        let mut sample_input = 0usize;
        let mut sample_encoded = 0usize;
        let mut sample_metadata = 0usize;
        for range in &ranges {
            let sample = &input[range.clone()];
            let (metadata, payload, _) = encode_plan_payload(sample, &candidate.plan)?;
            sample_input = sample_input.saturating_add(sample.len());
            sample_metadata = sample_metadata.saturating_add(metadata.len());
            sample_encoded = sample_encoded
                .saturating_add(metadata.len())
                .saturating_add(payload.len());
        }
        if sample_input != 0 {
            let projected = ((sample_encoded as u128).saturating_mul(input.len() as u128)
                / sample_input as u128)
                .min(u64::MAX as u128) as u64;
            candidate.cost.predicted_size_bytes = projected;
            candidate.cost.metadata_bytes = ((sample_metadata as u128)
                .saturating_mul(input.len() as u128)
                / sample_input as u128)
                .min(u64::MAX as u128) as u64;
            candidate.score = model.score(config.profile, candidate.cost, input.len());
        }
        let mut plan = candidate.plan;
        plan.cost = candidate.cost;
        plan.score = candidate.score;
        verified.push(plan);
    }
    apply_entropy_selection_policy(&mut verified, config.profile);
    let selected = verified
        .into_iter()
        .min_by(|a, b| {
            a.score
                .cmp(&b.score)
                .then_with(|| stable_plan_key(a).cmp(&stable_plan_key(b)))
        })
        .ok_or(AceError::Malformed(
            "sample verifier produced no candidates",
        ))?;

    Ok(PlannerDecision {
        plan: selected,
        telemetry: PlannerTelemetry {
            fast_path_hit: false,
            estimated_candidates: candidates.len(),
            sampled_candidates: top_k,
            full_trial_encodes: 0,
        },
    })
}
