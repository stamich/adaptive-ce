//! Plans and encodes one independent block.

use std::time::{Duration, Instant};

use ace_analysis::{AnalysisLevel, DefaultBlockAnalyzer};
use ace_codecs::{
    numeric_encode_estimated, numeric_encode_fixed_step, ts1_encode_with, TimeSeriesLayout,
};
use ace_core::{
    AceConfig, AceError, AceResult, BlockProfile, CodecId, ColumnKind, ColumnSpec,
    CompressionProfile, EntropyCodecId, PhysicalCompressionPlan, Schema, SegmentationConfig,
};
use ace_format::{
    checksum, encode_column_payload, encode_segment_payload, serialize_block, split_columns,
    BlockHeader, BLOCK_HEADER_SIZE,
};
use ace_planner::{
    apply_time_series_policy, encode_plan_payload, evaluate_candidates_v4_with_route,
    float_fast_decision, numeric_fast_decision_from_route, same_plan_semantics, segment_boundaries,
    DefaultCompressionPlanner, FloatFastOutcome, PlannerDecision, PlannerRoute, PlannerTelemetry,
    PlanningContext, TimeSeriesSelection,
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
    /// Plans of the inner blocks when this block is segmented (empty otherwise), in segment
    /// order. The segment count of the block is the length of this list.
    pub inner_plans: Vec<PhysicalCompressionPlan>,
}

/// Bytes a block occupies besides its fixed-size header: payload, metadata and transform list.
fn framed_len(block: &EncodedBlock) -> usize {
    block.payload.len() + block.metadata.len() + block.plan.decoding.transforms.len()
}

/// Block analysis computed at most once per block and shared by the plans that need an
/// equivalent one (DENSE plans a block as DENSE and as BALANCED).
#[derive(Default)]
struct SharedAnalysis {
    /// The level the stored profile was computed at, and the profile.
    computed: Option<(AnalysisLevel, BlockProfile)>,
}

/// Inherent methods of [`SharedAnalysis`].
impl SharedAnalysis {
    /// Analyses `input` at `level`, or returns the stored profile when it is equivalent.
    /// The `bool` is true when the analysis ran now (its time belongs to the caller).
    fn get(&mut self, input: &[u8], level: AnalysisLevel) -> (&BlockProfile, bool) {
        let fresh = !self
            .computed
            .as_ref()
            .is_some_and(|(stored, _)| stored.same_analysis_as(level));
        if fresh {
            self.computed = None;
        }
        let (_, profile) = self
            .computed
            .get_or_insert_with(|| (level, DefaultBlockAnalyzer.analyze_with_level(input, level)));
        (profile, fresh)
    }
}

/// One block planned with one profile, ready to be encoded.
struct PlannedBlock {
    /// Planner V5 route decision.
    context: PlanningContext,
    /// Selected plan with its estimates.
    decision: PlannerDecision,
    /// Time spent so far.
    timings: BlockTimings,
}

/// Encodes block `id`; with a segmentation or a schema in the configuration it also tries the
/// segmented and the columnar form and keeps the smallest.
///
/// The unsegmented row-wise block is always encoded first, so the result is never larger than
/// it: a container replaces it only when it saves at least `max(min_gain_bytes, 1 %)` of the
/// framed block (`docs/FORMAT-1.6.md`, writer rules). Among containers that pass, the smaller
/// wins and a tie goes to the segmented block. Without either option this is exactly the
/// row-wise encoder, so the output keeps the bytes of 0.7.0.
///
/// # Determinism
///
/// The segment boundaries are a pure function of the input and the configuration
/// ([`segment_boundaries`]), the column streams a pure function of the input and the schema,
/// and every part is encoded by the same deterministic encoder, so the stored block does not
/// depend on the thread count or the host.
pub(crate) fn encode_block(config: &AceConfig, id: u64, input: &[u8]) -> AceResult<EncodedBlock> {
    let plain = encode_block_plain(config, id, input)?;
    let mut best: Option<EncodedBlock> = None;
    if let Some(segmentation) = &config.segmentation {
        best = try_segmented(config, segmentation, id, input, &plain)?;
    }
    if let Some(schema) = &config.schema {
        if let Some(columnar) = try_columnar(config, schema, id, input, &plain)? {
            let smaller = best
                .as_ref()
                .is_none_or(|segmented| columnar.payload.len() < segmented.payload.len());
            if smaller {
                best = Some(columnar);
            }
        }
    }
    Ok(best.unwrap_or(plain))
}

/// True when a container of `container_payload` bytes beats `plain` by the writer margin.
fn beats_by_margin(config: &AceConfig, plain: &EncodedBlock, container_payload: usize) -> bool {
    let plain_framed = BLOCK_HEADER_SIZE + framed_len(plain);
    let container_framed = BLOCK_HEADER_SIZE + container_payload;
    let margin = config.min_gain_bytes.max(plain_framed / 100);
    plain_framed.saturating_sub(container_framed) >= margin
}

/// The inner blocks of a container, serialized, with the plans and timings of their encoding.
struct InnerBlocks {
    /// `(reconstructed size, serialized inner block)` in order.
    serialized: Vec<(u32, Vec<u8>)>,
    /// The plan each inner block was stored with.
    plans: Vec<PhysicalCompressionPlan>,
    /// Timings of the plain block plus every inner block.
    timings: BlockTimings,
}

/// Encodes `parts` as inner blocks whose ids are their positions (`docs/FORMAT-1.6.md`); every
/// part is planned with the configuration it comes with.
fn encode_inner_blocks<'a>(
    parts: impl IntoIterator<Item = (&'a [u8], &'a AceConfig)>,
    plain_timings: BlockTimings,
) -> AceResult<InnerBlocks> {
    let mut inner_blocks = InnerBlocks {
        serialized: Vec::new(),
        plans: Vec::new(),
        timings: plain_timings,
    };
    for (index, (part, config)) in parts.into_iter().enumerate() {
        let inner = encode_block_plain(config, index as u64, part)?;
        let timings = &mut inner_blocks.timings;
        timings.route_classify += inner.timings.route_classify;
        timings.generic_analysis += inner.timings.generic_analysis;
        timings.planning += inner.timings.planning;
        timings.encoding += inner.timings.encoding;
        let original_size = u32::try_from(part.len())
            .map_err(|_| AceError::ResourceLimitExceeded("inner block size"))?;
        inner_blocks.serialized.push((
            original_size,
            serialize_block(&inner.header, &inner.metadata, &inner.payload),
        ));
        inner_blocks.plans.push(inner.plan);
    }
    Ok(inner_blocks)
}

/// Wraps a container payload into the block that replaces `plain`.
fn container_block(
    id: u64,
    input: &[u8],
    codec: CodecId,
    plan: PhysicalCompressionPlan,
    payload: Vec<u8>,
    inner: InnerBlocks,
    plain: &EncodedBlock,
) -> AceResult<EncodedBlock> {
    let header = BlockHeader {
        block_id: id,
        original_size: u32::try_from(input.len())
            .map_err(|_| AceError::ResourceLimitExceeded("block size"))?,
        encoded_size: u32::try_from(payload.len())
            .map_err(|_| AceError::ResourceLimitExceeded("container payload"))?,
        metadata_size: 0,
        codec,
        entropy: EntropyCodecId::None,
        transforms: Vec::new(),
        dictionary: None,
        flags: 0,
        // The outer checksum covers the whole reconstructed block, so a forged table cannot
        // produce a block that passes (docs/FORMAT-1.6.md, inner blocks).
        payload_crc32c: checksum(input),
    };
    Ok(EncodedBlock {
        header,
        metadata: Vec::new(),
        payload,
        plan,
        timings: inner.timings,
        planner_telemetry: plain.planner_telemetry,
        route: plain.route,
        time_series_layout: None,
        inner_plans: inner.plans,
    })
}

/// The segmented form of block `id`, or `None` when it does not beat `plain` by the margin.
fn try_segmented(
    config: &AceConfig,
    segmentation: &SegmentationConfig,
    id: u64,
    input: &[u8],
    plain: &EncodedBlock,
) -> AceResult<Option<EncodedBlock>> {
    if input.len() < segmentation.min_segment_size.saturating_mul(2) {
        return Ok(None);
    }
    let ranges = segment_boundaries(input, config, segmentation)?;
    if ranges.len() < 2 {
        return Ok(None);
    }
    let inner = encode_inner_blocks(
        ranges.iter().map(|range| (&input[range.clone()], config)),
        plain.timings,
    )?;
    let payload = encode_segment_payload(&inner.serialized)?;
    if !beats_by_margin(config, plain, payload.len()) {
        return Ok(None);
    }
    container_block(
        id,
        input,
        CodecId::Segmented,
        PhysicalCompressionPlan::segmented(),
        payload,
        inner,
        plain,
    )
    .map(Some)
}

/// The configuration a column stream is planned with: the block configuration narrowed by what
/// the schema says the column holds.
///
/// * [`ColumnKind::Bytes`]: no numeric and no float specialization — the stream is searched with
///   the generic candidates only (RAW, RLE, LZ, entropy), which is faster and cannot mistake
///   identifiers or text for numbers.
/// * [`ColumnKind::Unsigned`] / [`ColumnKind::Signed`]: the float lane is off, because the bit
///   patterns of integers must not be tried as floats; NUM1 and the integer TS1 modes stay on.
/// * [`ColumnKind::Float`]: the configuration is unchanged (the float lane and NUM1 compete).
///
/// A specialization the block configuration already disabled stays disabled. The result is a
/// pure function of the configuration and the kind, so the choice is deterministic.
fn column_config(config: &AceConfig, kind: ColumnKind) -> AceConfig {
    let mut narrowed = config.clone();
    match kind {
        ColumnKind::Bytes => {
            narrowed.enable_numeric_specialization = false;
            narrowed.enable_float_specialization = false;
        }
        ColumnKind::Unsigned | ColumnKind::Signed => narrowed.enable_float_specialization = false,
        ColumnKind::Float => {}
    }
    narrowed
}

/// The columnar form of block `id` under `schema`, or `None` when the block holds no whole
/// record or the columnar form does not beat `plain` by the margin.
///
/// Every column stream is planned and encoded on its own, exactly like a block, so a counter
/// column gets NUM1, a text column LZ and a noisy column RAW.
fn try_columnar(
    config: &AceConfig,
    schema: &Schema,
    id: u64,
    input: &[u8],
    plain: &EncodedBlock,
) -> AceResult<Option<EncodedBlock>> {
    let record_size = schema.record_size() as usize;
    if input.len() < record_size {
        return Ok(None);
    }
    let (streams, tail) = split_columns(input, &schema.widths())?;
    let record_count = u32::try_from(input.len() / record_size)
        .map_err(|_| AceError::ResourceLimitExceeded("record count"))?;
    let column_configs: Vec<AceConfig> = schema
        .columns()
        .iter()
        .map(|column| column_config(config, column.kind))
        .collect();
    let inner = encode_inner_blocks(
        streams.iter().map(Vec::as_slice).zip(column_configs.iter()),
        plain.timings,
    )?;
    let columns: Vec<(ColumnSpec, Vec<u8>)> = schema
        .columns()
        .iter()
        .copied()
        .zip(inner.serialized.iter().map(|(_, bytes)| bytes.clone()))
        .collect();
    let payload = encode_column_payload(&columns, record_count, tail)?;
    if !beats_by_margin(config, plain, payload.len()) {
        return Ok(None);
    }
    container_block(
        id,
        input,
        CodecId::Columnar,
        PhysicalCompressionPlan::columnar(),
        payload,
        inner,
        plain,
    )
    .map(Some)
}

/// Encodes block `id`; the DENSE profile never stores a block larger than BALANCED would (0.6.1).
///
/// DENSE plans with a wider and slower search, but its estimates are not exact, so on some data
/// (zero blocks, long runs, decimal floats) the search settles on a plan that is a little larger
/// than the one BALANCED picks. The block is therefore also planned as BALANCED and the smaller
/// result is stored; ties keep the DENSE block.
///
/// Both plans share one block analysis (the two levels analyse identically), and when they
/// select the same encoding the block is encoded once, so the second plan costs little more than
/// its candidate search.
///
/// # Determinism
///
/// Both encodings are pure functions of `(config, input)` and the comparison is on exact stored
/// sizes, so the choice is the same on every platform and thread count.
fn encode_block_plain(config: &AceConfig, id: u64, input: &[u8]) -> AceResult<EncodedBlock> {
    let mut analysis = SharedAnalysis::default();
    let dense_plan = plan_for_profile(config, input, &mut analysis)?;
    if config.profile != CompressionProfile::Dense {
        return encode_planned(config, id, input, dense_plan);
    }
    let balanced_config = AceConfig {
        profile: CompressionProfile::Balanced,
        ..config.clone()
    };
    let balanced_plan = plan_for_profile(&balanced_config, input, &mut analysis)?;
    if same_encoding(&dense_plan, &balanced_plan) {
        // Same bytes by construction: encode once and account for both searches.
        let planning = balanced_plan.timings;
        let mut winner = encode_planned(config, id, input, dense_plan)?;
        winner.timings.route_classify += planning.route_classify;
        winner.timings.generic_analysis += planning.generic_analysis;
        winner.timings.planning += planning.planning;
        return Ok(winner);
    }
    let dense = encode_planned(config, id, input, dense_plan)?;
    let balanced = encode_planned(&balanced_config, id, input, balanced_plan)?;
    let balanced_wins = framed_len(&balanced) < framed_len(&dense);
    let (mut winner, loser) = if balanced_wins {
        (balanced, dense)
    } else {
        (dense, balanced)
    };
    winner.timings.route_classify += loser.timings.route_classify;
    winner.timings.generic_analysis += loser.timings.generic_analysis;
    winner.timings.planning += loser.timings.planning;
    winner.timings.encoding += loser.timings.encoding;
    Ok(winner)
}

/// True when encoding both planned blocks is guaranteed to produce the same bytes: the same plan
/// semantics, the same NUM1 estimate or TS1 selection, and the same fast-path evidence.
fn same_encoding(a: &PlannedBlock, b: &PlannedBlock) -> bool {
    same_plan_semantics(&a.decision.plan, &b.decision.plan)
        && a.decision.numeric_estimate == b.decision.numeric_estimate
        && a.decision.time_series == b.decision.time_series
        && a.decision.telemetry.fast_path_hit == b.decision.telemetry.fast_path_hit
        && a.context.route.numeric_fast_evidence == b.context.route.numeric_fast_evidence
}

/// Plans block `input` with `config.profile` using one authoritative Planner V5 routing decision.
fn plan_for_profile(
    config: &AceConfig,
    input: &[u8],
    analysis: &mut SharedAnalysis,
) -> AceResult<PlannedBlock> {
    // Route classification and strict NumericFast validation happen exactly once.
    let context = PlanningContext::classify(input, config);
    let mut timings = BlockTimings {
        route_classify: context.route_classify_time,
        ..BlockTimings::default()
    };
    let decision = plan_block(config, input, &context, analysis, &mut timings)?;
    Ok(PlannedBlock {
        context,
        decision,
        timings,
    })
}

/// Encodes a planned block (payload, entropy metadata, RAW fallback) and builds its header.
fn encode_planned(
    config: &AceConfig,
    id: u64,
    input: &[u8],
    planned: PlannedBlock,
) -> AceResult<EncodedBlock> {
    let PlannedBlock {
        context,
        mut decision,
        mut timings,
    } = planned;
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
        inner_plans: Vec::new(),
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
    analysis: &mut SharedAnalysis,
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
    let (profile, analysed_now) = analysis.get(input, AnalysisLevel::for_profile(config.profile));
    if analysed_now {
        timings.generic_analysis = analysis_started.elapsed();
    }

    let planning_started = Instant::now();
    let candidates = DefaultCompressionPlanner.candidates_for_route(
        profile,
        config,
        context.route.candidate_route(),
    );
    let decision =
        evaluate_candidates_v4_with_route(input, profile, &candidates, config, &context.route)?;
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
        Some(numeric_encode_estimated(input, &estimate)?)
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
