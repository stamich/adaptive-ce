//! Plans and encodes one independent block.

use std::time::{Duration, Instant};

use ace_analysis::{AnalysisLevel, DefaultBlockAnalyzer};
use ace_codecs::{
    numeric_encode_fixed_step, numeric_encode_with, ts1_encode_with, TimeSeriesLayout,
};
use ace_core::{AceConfig, AceResult, CodecId, PhysicalCompressionPlan};
use ace_format::{checksum, BlockHeader, BLOCK_HEADER_SIZE};
use ace_planner::{
    apply_time_series_policy, encode_plan_payload, evaluate_candidates_v4_with_route,
    float_fast_decision, numeric_fast_decision_from_route, DefaultCompressionPlanner,
    FloatFastOutcome, PlannerDecision, PlannerRoute, PlannerTelemetry, PlanningContext,
    TimeSeriesSelection,
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
    /// Route the block was classified to (Planner V5).
    pub route: PlannerRoute,
    /// TS1 layout of the stored block (`None` unless the stored codec is TS1).
    pub time_series_layout: Option<TimeSeriesLayout>,
}

/// Encodes block `id` with one authoritative Planner V5 routing decision.
pub(crate) fn encode_block(config: &AceConfig, id: u64, input: &[u8]) -> AceResult<EncodedBlock> {
    // Route classification and strict NumericFast validation happen exactly once.
    let context = PlanningContext::classify(input, config);
    let mut timings = BlockTimings {
        route_classify: context.route_classify_time,
        ..BlockTimings::default()
    };
    let mut decision = plan_block(config, input, &context, &mut timings)?;
    // The TS1 selection (with a FloatFast payload) is moved into the encoder, not copied.
    let time_series = decision.time_series.take();
    let time_series_layout = time_series.as_ref().map(|s| s.estimate.layout);

    let encoding_started = Instant::now();
    let (entropy_metadata, payload, primary_len) =
        encode_selected_plan(input, &decision, time_series, &context)?;
    let metadata = wrap_entropy_metadata(
        decision.plan.decoding.entropy,
        primary_len,
        entropy_metadata,
    )?;
    let (plan, metadata, payload) =
        apply_raw_fallback(config, input, decision.plan, metadata, payload);
    timings.encoding = encoding_started.elapsed();
    let time_series_layout =
        time_series_layout.filter(|_| plan.decoding.codec == CodecId::TimeSeries);

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
        route: context.route.route,
        time_series_layout,
    })
}

/// Selects the block plan (Planner V5).
///
/// 1. NumericFast: direct NUM1 decision from the route evidence.
/// 2. FloatFast: one TS1 encode; a payload above the fallback bound re-plans the block as
///    FloatGeneral (counted in `float_fast_fallback`).
/// 3. Otherwise: generic analysis + the V4.3 pipeline of the base route, then the TS1
///    candidate (Float evidence or RunDelta) competes via `apply_time_series_policy`.
fn plan_block(
    config: &AceConfig,
    input: &[u8],
    context: &PlanningContext,
    timings: &mut BlockTimings,
) -> AceResult<PlannerDecision> {
    if let Some(decision) = numeric_fast_decision_from_route(input, &context.route) {
        return Ok(decision);
    }
    let planning_started = Instant::now();
    let float_fast_fallback = match float_fast_decision(input, &context.route)? {
        FloatFastOutcome::Selected(decision) => {
            timings.planning = planning_started.elapsed();
            return Ok(*decision);
        }
        FloatFastOutcome::Fallback => true,
        FloatFastOutcome::NotFloatFast => false,
    };
    let float_fast_time = planning_started.elapsed();

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
    let mut decision = apply_time_series_policy(input, decision, &context.route, config)?;
    decision.telemetry.float_fast_fallback = float_fast_fallback;
    timings.planning = float_fast_time + planning_started.elapsed();
    Ok(decision)
}

/// Produces `(entropy metadata, payload, primary length)` for the selected plan.
///
/// Plain NUM1 plans reuse planner evidence instead of re-running the width/mode search:
/// NumericFast evidence serializes a header-only payload; a NumericGeneral estimate fixes
/// `(width, mode)`. TS1 plans store the FloatFast payload or encode the selected layout once.
/// Every other plan runs the generic reversible pipeline.
fn encode_selected_plan(
    input: &[u8],
    decision: &PlannerDecision,
    time_series: Option<TimeSeriesSelection>,
    context: &PlanningContext,
) -> AceResult<(Vec<u8>, Vec<u8>, usize)> {
    if let Some(selection) = time_series {
        let payload = match selection.payload {
            Some(payload) => payload,
            None => ts1_encode_with(input, selection.estimate.layout)?,
        };
        let primary_len = payload.len();
        return Ok((Vec::new(), payload, primary_len));
    }
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
