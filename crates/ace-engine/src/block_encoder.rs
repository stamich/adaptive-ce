//! Plans and encodes one independent block.

use std::time::{Duration, Instant};

use ace_analysis::{AnalysisLevel, DefaultBlockAnalyzer};
use ace_codecs::{numeric_encode_fixed_step, numeric_encode_with};
use ace_core::{AceConfig, AceResult, CodecId, PhysicalCompressionPlan};
use ace_format::{checksum, BlockHeader, BLOCK_HEADER_SIZE};
use ace_planner::{
    encode_plan_payload, evaluate_candidates_v4_with_route, numeric_fast_decision_from_route,
    DefaultCompressionPlanner, PlannerDecision, PlannerTelemetry, PlanningContext,
};

use crate::block_pipeline::wrap_entropy_metadata;

/// Per-block wall-clock breakdown (summed across workers in `CompressionStats`).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct BlockTimings {
    /// Route prefilter / classification / NumericFast validation.
    pub route_classify: Duration,
    /// Generic block analysis after routing.
    pub generic_analysis: Duration,
    /// Candidate generation and evaluation.
    pub planning: Duration,
    /// Encoding of the selected plan (including RAW fallback).
    pub encoding: Duration,
}

/// Fully encoded independent block produced by a worker before ordered container assembly.
#[derive(Debug)]
pub(crate) struct EncodedBlock {
    /// Serialized block header (plan, sizes, checksum).
    pub header: BlockHeader,
    /// Stored metadata (entropy model with primary-length prefix when required).
    pub metadata: Vec<u8>,
    /// Encoded payload.
    pub payload: Vec<u8>,
    /// Plan finally stored (may be the RAW fallback).
    pub plan: PhysicalCompressionPlan,
    /// Time breakdown.
    pub timings: BlockTimings,
    /// Planner work counters.
    pub planner_telemetry: PlannerTelemetry,
}

/// Encodes block `id` with one authoritative Planner V4.3 routing decision.
pub(crate) fn encode_block(config: &AceConfig, id: u64, input: &[u8]) -> AceResult<EncodedBlock> {
    // Route classification and strict NumericFast validation happen exactly once.
    let context = PlanningContext::classify(input, config);
    let mut timings = BlockTimings {
        route_classify: context.route_classify_time,
        ..BlockTimings::default()
    };
    let decision = plan_block(config, input, &context, &mut timings)?;

    let encoding_started = Instant::now();
    let (entropy_metadata, payload, primary_len) =
        encode_selected_plan(input, &decision, &context)?;
    let metadata = wrap_entropy_metadata(
        decision.plan.decoding.entropy,
        primary_len,
        entropy_metadata,
    )?;
    let (plan, metadata, payload) =
        apply_raw_fallback(config, input, decision.plan, metadata, payload);
    timings.encoding = encoding_started.elapsed();

    let header = BlockHeader {
        block_id: id,
        original_size: input.len() as u32,
        encoded_size: payload.len() as u32,
        metadata_size: metadata.len() as u32,
        codec: plan.decoding.codec,
        entropy: plan.decoding.entropy,
        transforms: plan.decoding.transforms.clone(),
        dictionary: plan.decoding.dictionary,
        flags: 0,
        payload_crc32c: checksum(input),
    };
    Ok(EncodedBlock {
        header,
        metadata,
        payload,
        plan,
        timings,
        planner_telemetry: decision.telemetry,
    })
}

/// Selects the block plan: the NumericFast shortcut or generic analysis + candidate evaluation.
fn plan_block(
    config: &AceConfig,
    input: &[u8],
    context: &PlanningContext,
    timings: &mut BlockTimings,
) -> AceResult<PlannerDecision> {
    if let Some(decision) = numeric_fast_decision_from_route(input, &context.route) {
        return Ok(decision);
    }
    let analysis_started = Instant::now();
    let profile =
        DefaultBlockAnalyzer.analyze_with_level(input, AnalysisLevel::for_profile(config.profile));
    timings.generic_analysis = analysis_started.elapsed();

    let planning_started = Instant::now();
    let candidates = DefaultCompressionPlanner.candidates_for_route(
        &profile,
        config,
        context.route.candidate_route(),
    );
    let decision =
        evaluate_candidates_v4_with_route(input, &profile, &candidates, config, &context.route)?;
    timings.planning = planning_started.elapsed();
    Ok(decision)
}

/// Produces `(entropy metadata, payload, primary length)` for the selected plan.
///
/// Plain NUM1 plans reuse planner evidence instead of re-running the width/mode search:
/// NumericFast evidence serializes a header-only payload; a NumericGeneral estimate fixes
/// `(width, mode)`. Every other plan runs the generic reversible pipeline.
fn encode_selected_plan(
    input: &[u8],
    decision: &PlannerDecision,
    context: &PlanningContext,
) -> AceResult<(Vec<u8>, Vec<u8>, usize)> {
    let plan = &decision.plan;
    let numeric_payload = if !plan.is_plain_numeric() {
        None
    } else if let (true, Some(evidence)) = (
        decision.telemetry.fast_path_hit,
        context.route.numeric_fast_evidence,
    ) {
        Some(numeric_encode_fixed_step(
            input,
            evidence.width,
            evidence.first_value,
            evidence.first_delta,
            evidence.value_count,
            evidence.tail_bytes,
        )?)
    } else if let Some(estimate) = decision.numeric_estimate {
        Some(numeric_encode_with(input, estimate.width, estimate.mode)?)
    } else {
        None
    };
    match numeric_payload {
        Some(payload) => {
            let primary_len = payload.len();
            Ok((Vec::new(), payload, primary_len))
        }
        None => encode_plan_payload(input, plan),
    }
}

/// Replaces a plan by RAW when it does not save at least `min_gain_bytes` after framing.
fn apply_raw_fallback(
    config: &AceConfig,
    input: &[u8],
    plan: PhysicalCompressionPlan,
    metadata: Vec<u8>,
    payload: Vec<u8>,
) -> (PhysicalCompressionPlan, Vec<u8>, Vec<u8>) {
    let is_raw = plan.decoding.codec == CodecId::Raw
        && plan.decoding.transforms.is_empty()
        && metadata.is_empty();
    let framed =
        payload.len() + BLOCK_HEADER_SIZE + plan.decoding.transforms.len() + metadata.len();
    if !is_raw && framed.saturating_add(config.min_gain_bytes) >= input.len() + BLOCK_HEADER_SIZE {
        return (PhysicalCompressionPlan::raw(), Vec::new(), input.to_vec());
    }
    (plan, metadata, payload)
}
