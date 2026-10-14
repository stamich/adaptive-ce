//! `allocation-profile` (ACE 0.5.1): where the allocations of one block come from.
//!
//! The 0.5.0 baseline showed `int-sparse-change` BALANCED allocating about 37 bytes per input
//! byte (peak memory was normal: this is churn). This family attributes the churn to the
//! stages of the block encoder by replaying them exactly as `block_encoder::plan_block` runs
//! them — route classification, NumericFast, FloatFast, generic analysis, candidate
//! generation, the V4.3 evaluator, the TS1 policy and the final payload encode — with the
//! counting allocator around each stage. Measurement only: 0.5.1 changes no stage order
//! (an earlier TS1 decision is a 0.6 item, since it changes the order of work).

use ace_codecs::{numeric_encode_fixed_step, numeric_encode_with, ts1_encode_with};
use ace_planner::{
    apply_time_series_policy, float_fast_decision, numeric_fast_decision_from_route,
    FloatFastOutcome, PlannerDecision,
};

use super::memory::ALLOCATION_CASES;
use crate::alloc::{count_allocations, AllocationReport};
use crate::prelude::*;

/// Blocks replayed per case (the first `BLOCKS` blocks of the workload).
const BLOCKS: usize = 16;

/// Stage names in pipeline order.
const STAGES: [&str; 8] = [
    "classify",
    "numeric_fast",
    "float_fast",
    "analysis",
    "candidates",
    "evaluate",
    "ts1_policy",
    "encode",
];

/// Allocation totals of one stage over all replayed blocks.
#[derive(Debug, Default, Clone, Copy)]
struct StageTotals {
    /// Blocks that ran the stage.
    blocks: usize,
    /// Allocation calls.
    allocations: u64,
    /// Bytes requested.
    allocated_bytes: u64,
    /// Largest per-block peak of live bytes.
    max_peak_live_bytes: u64,
}

/// Inherent methods of [`StageTotals`].
impl StageTotals {
    /// Adds one block's report.
    fn add(&mut self, report: AllocationReport) {
        self.blocks += 1;
        self.allocations += report.allocations;
        self.allocated_bytes += report.allocated_bytes;
        self.max_peak_live_bytes = self.max_peak_live_bytes.max(report.peak_live_bytes);
    }
}

/// Runs the family: one row per `memory` allocation case.
pub(crate) fn allocation_profile_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    ALLOCATION_CASES
        .iter()
        .map(|&(path, workload, profile)| profile_case(path, workload, profile))
        .collect()
}

/// Replays the first [`BLOCKS`] blocks of `workload` stage by stage.
fn profile_case(
    path: &str,
    workload: &str,
    profile: CompressionProfile,
) -> Result<Value, Box<dyn std::error::Error>> {
    let config = AceConfig {
        profile,
        threads: 1,
        ..AceConfig::default()
    };
    let data = workload_bytes(workload, BLOCKS * config.block_size);
    let mut totals = [StageTotals::default(); STAGES.len()];
    let mut routes = BTreeMap::<String, usize>::new();
    for block in data.chunks(config.block_size) {
        let route = replay_block(block, &config, &mut totals)?;
        *routes.entry(route).or_default() += 1;
    }
    let (_, whole) = count_allocations(|| AceEngine::new(config.clone())?.compress(&data));
    let staged: u64 = totals.iter().map(|stage| stage.allocated_bytes).sum();

    let mut stages = JsonObjectBuilder::new();
    for (name, stage) in STAGES.iter().zip(totals) {
        let mut row = JsonObjectBuilder::new();
        row.field("blocks", stage.blocks)
            .field("allocations", stage.allocations)
            .field("allocated_bytes", stage.allocated_bytes)
            .field(
                "allocated_bytes_per_input_byte",
                stage.allocated_bytes as f64 / data.len() as f64,
            )
            .field(
                "share_percent",
                stage.allocated_bytes as f64 * 100.0 / staged.max(1) as f64,
            )
            .field("max_peak_live_bytes", stage.max_peak_live_bytes);
        stages.value(name, row.build());
    }
    let mut row = JsonObjectBuilder::new();
    row.field(
        "workload_id",
        format!("{workload}_{BLOCKS}x{}k", config.block_size / 1024),
    )
    .field("path", format!("allocation_profile_{path}"))
    .field("profile", format!("{profile:?}"))
    .field("input_bytes", data.len())
    .field("blocks", data.len().div_ceil(config.block_size))
    .field("routes", routes)
    .value("stages", stages.build())
    .field("staged_allocated_bytes", staged)
    .field(
        "staged_bytes_per_input_byte",
        staged as f64 / data.len() as f64,
    )
    .field("engine_compress", whole)
    .field(
        "engine_bytes_per_input_byte",
        whole.allocated_bytes as f64 / data.len() as f64,
    );
    Ok(row.build())
}

/// Replays `plan_block` + the payload encode for one block; returns the route name.
fn replay_block(
    block: &[u8],
    config: &AceConfig,
    totals: &mut [StageTotals; STAGES.len()],
) -> Result<String, Box<dyn std::error::Error>> {
    let (context, report) = count_allocations(|| PlanningContext::classify(block, config));
    totals[0].add(report);
    let route = context.route.route.label().to_string();

    let (fast, report) =
        count_allocations(|| numeric_fast_decision_from_route(block, &context.route));
    totals[1].add(report);
    let decision = match fast {
        Some(decision) => decision,
        None => {
            let (float_fast, report) =
                count_allocations(|| float_fast_decision(block, &context.route));
            totals[2].add(report);
            match float_fast? {
                FloatFastOutcome::Selected(decision) => *decision,
                _ => generic_decision(block, config, &context, totals)?,
            }
        }
    };
    let (encoded, report) = count_allocations(|| encode_payload(block, &decision, &context));
    totals[7].add(report);
    encoded?;
    Ok(route)
}

/// Generic analysis, candidates, V4.3 evaluator and TS1 policy (stages 3 – 6).
fn generic_decision(
    block: &[u8],
    config: &AceConfig,
    context: &PlanningContext,
    totals: &mut [StageTotals; STAGES.len()],
) -> Result<PlannerDecision, Box<dyn std::error::Error>> {
    let (profile, report) = count_allocations(|| {
        DefaultBlockAnalyzer.analyze_with_level(block, AnalysisLevel::for_profile(config.profile))
    });
    totals[3].add(report);
    let (candidates, report) = count_allocations(|| {
        DefaultCompressionPlanner.candidates_for_route(
            &profile,
            config,
            context.route.candidate_route(),
        )
    });
    totals[4].add(report);
    let (generic, report) = count_allocations(|| {
        evaluate_candidates_v4_with_route(block, &profile, &candidates, config, &context.route)
    });
    totals[5].add(report);
    let (decision, report) =
        count_allocations(|| apply_time_series_policy(block, generic?, &context.route, config));
    totals[6].add(report);
    Ok(decision?)
}

/// The engine's payload encode for `decision` (`block_encoder::encode_selected_plan`): the
/// FloatFast payload or one TS1 encode, NumericFast evidence or the reused NUM1 estimate, or
/// the generic pipeline.
fn encode_payload(
    block: &[u8],
    decision: &PlannerDecision,
    context: &PlanningContext,
) -> AceResult<usize> {
    if let Some(selection) = &decision.time_series {
        return match &selection.payload {
            Some(payload) => Ok(payload.len()),
            None => ts1_encode_with(block, selection.estimate.layout).map(|p| p.len()),
        };
    }
    if decision.plan.is_plain_numeric() {
        if let (true, Some(evidence)) = (
            decision.telemetry.fast_path_hit,
            context.route.numeric_fast_evidence,
        ) {
            return numeric_encode_fixed_step(
                block,
                evidence.width,
                evidence.first_value,
                evidence.first_delta,
                evidence.value_count,
                evidence.tail_bytes,
            )
            .map(|p| p.len());
        }
        if let Some(estimate) = decision.numeric_estimate {
            return numeric_encode_with(block, estimate.width, estimate.mode).map(|p| p.len());
        }
    }
    encode_plan_payload(block, &decision.plan).map(|(_, payload, _)| payload.len())
}
