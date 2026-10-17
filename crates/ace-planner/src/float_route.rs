//! Planner V5 (ACE 0.5.0): the Float lane and the TS1 candidate.
//!
//! ```text
//! RoutePolicy::classify
//!   V4.3 classification ─ NumericFast? ─────────────────────────────▶ NumericFast (unchanged)
//!   float_prefilter admits?  ── TS1 estimate ≤ input / 32 ──────────▶ FloatFast
//!                            └─ otherwise ──────────────────────────▶ FloatGeneral
//!   otherwise: V4.3 route (NumericGeneral / Generic) + run prefilter (RunDelta widths)
//!
//! FloatFast:    encode TS1 once; > input / 16 → fall back to FloatGeneral (telemetry)
//! FloatGeneral, NumericGeneral, Generic:
//!               V4.3 pipeline of the base route → generic decision
//!               apply_time_series_policy: TS1 replaces it only if it is clearly better
//! ```
//!
//! The TS1 dominance rule ([`ts1_beats_generic`]) needs both a relative win (≤ 90 % of the
//! generic size for exact RunDelta sizes, ≤ 75 % for sampled Gorilla estimates) and an absolute one (saves ≥ 1/32 of the input). The absolute floor keeps
//! blocks that the 0.4 pipeline already compresses below 3 % of their size in Format 1.3:
//! a further gain of a few hundred bytes does not justify a file that 0.4.x cannot read.
//! Thresholds were calibrated on the `float-ablation` family
//! (`docs/FLOAT-CALIBRATION-0.5.0.md`).

use ace_analysis::{float_prefilter, run_prefilter, run_profile, FloatProfile, MIN_EQUAL_RATIO};
use ace_codecs::{ts1_encode_with, TIME_SERIES_HEADER_SIZE};
use ace_core::{
    AceConfig, AceResult, CandidateTier, CodecId, DecodingPlan, EntropyCodecId,
    PhysicalCompressionPlan, PlanCost,
};
use ace_cost::{
    alp_plausible, estimate_alp, estimate_gorilla, estimate_run_delta, TimeSeriesEstimate,
};

use crate::{
    PlannerDecision, PlannerRoute, PlannerTelemetry, RouteDecision, RouteReason,
    TimeSeriesSelection,
};

/// FloatFast admission: the TS1 estimate is at most `input / FLOAT_FAST_DIVISOR`.
pub const FLOAT_FAST_DIVISOR: u64 = 32;
/// FloatFast fallback: an encoded payload above `input / FLOAT_FAST_FALLBACK_DIVISOR` is
/// discarded and the block is re-planned as FloatGeneral.
pub const FLOAT_FAST_FALLBACK_DIVISOR: u64 = 16;
/// An exact TS1 size (RunDelta) must be at most this fraction of the generic size.
pub const TS1_MAX_FRACTION_OF_GENERIC: f64 = 0.9;
/// A sampled TS1 estimate (Gorilla) must be at most this fraction of the generic size.
///
/// Stricter than the exact rule: the Gorilla sample and the generic blended estimate can err
/// in opposite directions (calibration: `f32-smooth` BALANCED lost 2.5 % at 0.9).
pub const TS1_MAX_FRACTION_SAMPLED: f64 = 0.75;
/// TS1 must save at least `input / TS1_MIN_SAVING_DIVISOR` bytes over the generic plan.
pub const TS1_MIN_SAVING_DIVISOR: u64 = 32;

/// Float lane evidence collected once by the route classifier.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloatEvidence {
    /// Admitted float profile (width and sampled statistics).
    pub profile: FloatProfile,
    /// Smallest TS1 estimate at the float width: Gorilla (sampled) or RunDelta (exact).
    pub estimate: TimeSeriesEstimate,
    /// Number of TS1 estimates computed (1 or 2).
    pub estimates: usize,
}

/// Outcome of [`float_fast_decision`].
#[derive(Debug, Clone)]
pub enum FloatFastOutcome {
    /// The block is not on the FloatFast route.
    NotFloatFast,
    /// TS1 was encoded once and selected; the payload travels in the decision.
    Selected(Box<PlannerDecision>),
    /// The encoded payload missed the fallback bound; plan the block as FloatGeneral.
    Fallback,
}

/// Adds the Planner V5 Float lane to a V4.3 route decision.
///
/// Disabled specialization and NumericFast blocks are returned unchanged. An admitted float
/// block gets a Float route whose `base_route` keeps the V4.3 verdict; any other block gets
/// the run prefilter so RunDelta can compete after the generic decision.
pub fn classify_float_lane(input: &[u8], config: &AceConfig, v4: RouteDecision) -> RouteDecision {
    if !config.enable_float_specialization || v4.route == PlannerRoute::NumericFast {
        return v4;
    }
    if let Some(evidence) = float_prefilter(input)
        .admitted
        .and_then(|profile| float_evidence(input, profile))
    {
        let dominant = evidence
            .estimate
            .estimated_bytes
            .saturating_mul(FLOAT_FAST_DIVISOR)
            <= input.len() as u64;
        let (route, reason) = if dominant {
            (PlannerRoute::FloatFast, RouteReason::DominantFloat)
        } else {
            (PlannerRoute::FloatGeneral, RouteReason::FloatCandidate)
        };
        return RouteDecision {
            route,
            reason,
            base_route: v4.route,
            float_evidence: Some(evidence),
            ..v4
        };
    }
    RouteDecision {
        run_prefilter: run_prefilter(input),
        ..v4
    }
}

/// Gorilla estimate at the admitted width, plus the exact RunDelta size of the same lane when
/// its values mostly repeat and (0.6.1) the exact ALP size unless the block is already
/// FloatFast-dominant; the smallest
/// wins (ties: Gorilla, then RunDelta). `None` on estimator errors.
fn float_evidence(input: &[u8], profile: FloatProfile) -> Option<FloatEvidence> {
    let gorilla = estimate_gorilla(input, profile.width).ok()?;
    let lane_bytes = profile.width.lane_bytes() as u8;
    let repeats = run_profile(input, lane_bytes).is_some_and(|p| p.equal_ratio >= MIN_EQUAL_RATIO);
    let run = if repeats {
        estimate_run_delta(input, lane_bytes).ok()
    } else {
        None
    };
    let mut estimate = gorilla;
    if let Some(run) = run.filter(|run| run.estimated_bytes < gorilla.estimated_bytes) {
        estimate = run;
    }
    // ALP is counted only when the block is not already dominated by Gorilla / RunDelta: a
    // constant or stepped series needs no third opinion, noisy full-precision floats fail the
    // 64-value decimal test, and the exact count is a full pass.
    let dominant =
        estimate.estimated_bytes.saturating_mul(FLOAT_FAST_DIVISOR) <= input.len() as u64;
    let alp = if dominant || !alp_plausible(input, profile.width) {
        None
    } else {
        estimate_alp(input, profile.width).ok()
    };
    if let Some(alp) = alp.filter(|alp| alp.estimated_bytes < estimate.estimated_bytes) {
        estimate = alp;
    }
    let estimates = 1 + usize::from(run.is_some()) + usize::from(alp.is_some());
    Some(FloatEvidence {
        profile,
        estimate,
        estimates,
    })
}

/// FloatFast: encodes the evidence layout once and keeps it if it meets the fallback bound.
///
/// # Errors
///
/// Propagates the TS1 encoder's error for the evidence layout.
///
/// # Invariants
///
/// `Selected` ⇒ the payload is the evidence layout encoded once (no other candidate is
/// encoded) and `payload.len() * 16 <= input.len()`; otherwise `Fallback` or `NotFloatFast`,
/// and the caller continues with the generic route unchanged.
///
/// # Determinism
///
/// A pure function of `input` and `route`.
pub fn float_fast_decision(input: &[u8], route: &RouteDecision) -> AceResult<FloatFastOutcome> {
    let (PlannerRoute::FloatFast, Some(evidence)) = (route.route, route.float_evidence) else {
        return Ok(FloatFastOutcome::NotFloatFast);
    };
    let payload = ts1_encode_with(input, evidence.estimate.layout)?;
    if (payload.len() as u64).saturating_mul(FLOAT_FAST_FALLBACK_DIVISOR) > input.len() as u64 {
        return Ok(FloatFastOutcome::Fallback);
    }
    let estimate = TimeSeriesEstimate {
        estimated_bytes: payload.len() as u64,
        exact: true,
        confidence: 1.0,
        ..evidence.estimate
    };
    let telemetry = PlannerTelemetry {
        fast_path_hit: true,
        float_fast_hit: true,
        estimated_candidates: 1,
        quality_qualified_candidates: 1,
        selected_size_rank: 1,
        selected_cost_rank: 1,
        best_blended_size_bytes: estimate.estimated_bytes,
        quality_limit_bytes: estimate.estimated_bytes,
        selected_blended_size_bytes: estimate.estimated_bytes,
        time_series_estimates: evidence.estimates,
        ..PlannerTelemetry::default()
    };
    Ok(FloatFastOutcome::Selected(Box::new(time_series_decision(
        input,
        estimate,
        Some(payload),
        telemetry,
        "Planner V5 FloatFast: dominant TS1 estimate, encoded once",
    ))))
}

/// The TS1 dominance rule: relative (≤ 90 % of generic when exact, ≤ 75 % when sampled) and
/// absolute (≥ input / 32 saved).
pub fn ts1_beats_generic(
    estimate: &TimeSeriesEstimate,
    generic_bytes: u64,
    input_len: usize,
) -> bool {
    let fraction = if estimate.exact {
        TS1_MAX_FRACTION_OF_GENERIC
    } else {
        TS1_MAX_FRACTION_SAMPLED
    };
    let ts1_bytes = estimate.estimated_bytes;
    let relative = (ts1_bytes as f64) <= fraction * generic_bytes as f64;
    let saving = generic_bytes.saturating_sub(ts1_bytes);
    relative && saving.saturating_mul(TS1_MIN_SAVING_DIVISOR) >= input_len as u64
}

/// Size the generic decision is expected to produce (blended sample size when available).
pub fn generic_decision_bytes(decision: &PlannerDecision) -> u64 {
    if decision.telemetry.selected_blended_size_bytes > 0 {
        decision.telemetry.selected_blended_size_bytes
    } else {
        decision.plan.cost.predicted_size_bytes
    }
}

/// Lets the TS1 candidate compete with a finished generic decision.
///
/// Skipped when specialization is off, when a planner fast path decided the block (trivial
/// data), and when the generic plan already needs at most `input / 32` bytes — no TS1 plan
/// could pass the absolute-saving rule then, so RunDelta is not even counted.
///
/// # Errors
///
/// Propagates TS1 RunDelta estimation errors.
///
/// # Invariants
///
/// A TS1 decision is returned only when [`ts1_beats_generic`] holds (≤ 90 % of the generic
/// size for an exact estimate, ≤ 75 % for a sampled one, and at least `input / 32` bytes saved);
/// otherwise the generic plan is returned unchanged (only the telemetry gains the TS1
/// estimates). No full trial encode is performed.
///
/// # Determinism
///
/// A pure function of its arguments; the RunDelta candidate is the smallest exact estimate,
/// and on equal sizes the first admitted width (prefilter order) wins.
pub fn apply_time_series_policy(
    input: &[u8],
    decision: PlannerDecision,
    route: &RouteDecision,
    config: &AceConfig,
) -> AceResult<PlannerDecision> {
    if !config.enable_float_specialization || decision.telemetry.fast_path_hit {
        return Ok(decision);
    }
    let generic_bytes = generic_decision_bytes(&decision);
    if generic_bytes.saturating_mul(TS1_MIN_SAVING_DIVISOR) <= input.len() as u64 {
        return Ok(decision);
    }
    let (candidate, estimates) = match route.float_evidence {
        Some(evidence) => (Some(evidence.estimate), evidence.estimates),
        None => best_run_delta(input, route)?,
    };
    let mut telemetry = decision.telemetry;
    telemetry.time_series_estimates = estimates;
    match candidate {
        Some(estimate) if ts1_beats_generic(&estimate, generic_bytes, input.len()) => {
            Ok(time_series_decision(
                input,
                estimate,
                None,
                telemetry,
                "Planner V5 TS1 dominance over the generic decision",
            ))
        }
        _ => Ok(PlannerDecision {
            telemetry,
            ..decision
        }),
    }
}

/// Smallest exact RunDelta estimate over the widths admitted by the run prefilter.
fn best_run_delta(
    input: &[u8],
    route: &RouteDecision,
) -> AceResult<(Option<TimeSeriesEstimate>, usize)> {
    let mut best: Option<TimeSeriesEstimate> = None;
    let mut count = 0;
    for profile in route.run_prefilter.admitted() {
        let estimate = estimate_run_delta(input, profile.lane_bytes)?;
        count += 1;
        if best.is_none_or(|b| estimate.estimated_bytes < b.estimated_bytes) {
            best = Some(estimate);
        }
    }
    Ok((best, count))
}

/// The physical plan of a TS1 block: no transforms, no entropy stage.
pub fn time_series_plan(
    input: &[u8],
    estimate: &TimeSeriesEstimate,
    reason: &'static str,
) -> PhysicalCompressionPlan {
    let values = (input.len() / estimate.layout.lane_bytes.max(1) as usize) as u64;
    PhysicalCompressionPlan {
        decoding: DecodingPlan {
            transforms: Vec::new(),
            codec: CodecId::TimeSeries,
            dictionary: None,
            entropy: EntropyCodecId::None,
        },
        lz_mode: None,
        tier: CandidateTier::Likely,
        cost: PlanCost {
            predicted_size_bytes: estimate.estimated_bytes,
            metadata_bytes: TIME_SERIES_HEADER_SIZE as u64,
            encode_units: values,
            decode_units: values,
            memory_bytes: input.len() as u64,
        },
        score: u128::from(estimate.estimated_bytes),
        reason,
    }
}

/// Builds a TS1 [`PlannerDecision`] carrying the selection (and FloatFast payload).
fn time_series_decision(
    input: &[u8],
    estimate: TimeSeriesEstimate,
    payload: Option<Vec<u8>>,
    telemetry: PlannerTelemetry,
    reason: &'static str,
) -> PlannerDecision {
    let plan = time_series_plan(input, &estimate, reason);
    PlannerDecision {
        plan: plan.clone(),
        telemetry,
        analytical_ranked_plans: vec![plan.clone()],
        top_k_plans: vec![plan.clone()],
        stage_one_ranked_plans: vec![plan.clone()],
        second_stage_plans: Vec::new(),
        final_ranked_plans: vec![plan.clone()],
        quality_qualified_plans: vec![plan],
        numeric_estimate: None,
        time_series: Some(TimeSeriesSelection { estimate, payload }),
    }
}
