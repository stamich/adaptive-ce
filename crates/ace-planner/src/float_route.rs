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
use ace_cost::{estimate_gorilla, estimate_run_delta, TimeSeriesEstimate};

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
/// its values mostly repeat; the smaller one wins (ties: Gorilla). `None` on estimator errors.
fn float_evidence(input: &[u8], profile: FloatProfile) -> Option<FloatEvidence> {
    let gorilla = estimate_gorilla(input, profile.width).ok()?;
    let lane_bytes = profile.width.lane_bytes() as u8;
    let repeats = run_profile(input, lane_bytes).is_some_and(|p| p.equal_ratio >= MIN_EQUAL_RATIO);
    let run = if repeats {
        estimate_run_delta(input, lane_bytes).ok()
    } else {
        None
    };
    let estimates = 1 + usize::from(run.is_some());
    let estimate = match run {
        Some(run) if run.estimated_bytes < gorilla.estimated_bytes => run,
        _ => gorilla,
    };
    Some(FloatEvidence {
        profile,
        estimate,
        estimates,
    })
}

/// FloatFast: encodes the evidence layout once and keeps it if it meets the fallback bound.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RoutePolicy;
    use ace_codecs::TimeSeriesLayout;

    /// Little-endian f64 bytes of `values`.
    fn f64_bytes(values: impl IntoIterator<Item = f64>) -> Vec<u8> {
        values.into_iter().flat_map(f64::to_le_bytes).collect()
    }

    /// Smooth f64 values (small random increments, as in Corpus V4).
    fn smooth(count: usize) -> Vec<u8> {
        let mut value = 20.0f64;
        let mut state = 0x1234_5678u32;
        f64_bytes((0..count).map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            value += f64::from(state % 17) * 0.001 - 0.008;
            value
        }))
    }

    /// The dominance rule needs both the relative and the absolute win.
    #[test]
    fn dominance_rule() {
        let exact = |bytes| TimeSeriesEstimate {
            layout: TimeSeriesLayout::run_delta(4),
            estimated_bytes: bytes,
            exact: true,
            confidence: 1.0,
        };
        let sampled = |bytes| TimeSeriesEstimate {
            layout: TimeSeriesLayout::GORILLA_F64,
            exact: false,
            ..exact(bytes)
        };
        assert!(ts1_beats_generic(&exact(100), 100_000, 262_144));
        assert!(!ts1_beats_generic(&exact(95_000), 100_000, 262_144)); // only 5 % better
        assert!(!ts1_beats_generic(&exact(100), 8_000, 262_144)); // saves < input / 32
        assert!(ts1_beats_generic(&exact(0), 8_192, 262_144)); // saves exactly input / 32
        assert!(ts1_beats_generic(&exact(85_000), 100_000, 262_144));
        assert!(!ts1_beats_generic(&sampled(85_000), 100_000, 262_144)); // sampled: ≤ 75 %
        assert!(ts1_beats_generic(&sampled(75_000), 100_000, 262_144));
    }

    /// A constant f64 block is FloatFast with RunDelta and the payload is kept.
    #[test]
    fn constant_f64_is_float_fast_run_delta() {
        let input = f64_bytes(std::iter::repeat_n(21.5, 32_768));
        let route = RoutePolicy::classify(&input, &AceConfig::default());
        assert_eq!(route.route, PlannerRoute::FloatFast);
        assert_eq!(route.reason, RouteReason::DominantFloat);
        let evidence = route.float_evidence.expect("evidence");
        assert_eq!(evidence.estimate.layout, TimeSeriesLayout::run_delta(8));
        let FloatFastOutcome::Selected(decision) = float_fast_decision(&input, &route).unwrap()
        else {
            panic!("expected FloatFast selection");
        };
        assert_eq!(decision.plan.decoding.codec, CodecId::TimeSeries);
        assert!(decision.telemetry.float_fast_hit);
        let selection = decision.time_series.expect("selection");
        assert!(selection.payload.is_some());
        assert!(selection.estimate.exact);
    }

    /// A smooth f64 block is FloatGeneral; its base route is kept for the generic pipeline.
    #[test]
    fn smooth_f64_is_float_general() {
        let input = smooth(32_768);
        let route = RoutePolicy::classify(&input, &AceConfig::default());
        assert_eq!(route.route, PlannerRoute::FloatGeneral);
        assert!(!route.base_route.is_float());
        assert_eq!(route.candidate_route(), route.base_route);
        assert!(matches!(
            float_fast_decision(&input, &route).unwrap(),
            FloatFastOutcome::NotFloatFast
        ));
    }

    /// Disabling float specialization restores the V4.3 route decision exactly.
    #[test]
    fn disabled_specialization_is_v4() {
        let input = f64_bytes(std::iter::repeat_n(21.5, 32_768));
        let config = AceConfig {
            enable_float_specialization: false,
            ..AceConfig::default()
        };
        let route = RoutePolicy::classify(&input, &config);
        assert_eq!(route, crate::classify_planner_route(&input, &config));
        assert!(!route.run_prefilter.any());
    }

    /// Integer blocks with sparse changes get the run prefilter, not a Float route.
    #[test]
    fn sparse_integers_get_run_prefilter() {
        let input: Vec<u8> = (0..65_536u32)
            .flat_map(|i| (1_000 + i / 200 * 7).to_le_bytes())
            .collect();
        let route = RoutePolicy::classify(&input, &AceConfig::default());
        assert!(!route.route.is_float());
        assert!(route.run_prefilter.any());
    }
}
