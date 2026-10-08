//! Planner output types: [`PlannerDecision`] and its work counters [`PlannerTelemetry`].

use ace_core::PhysicalCompressionPlan;
use ace_cost::TimeSeriesEstimate;

/// Planner V4 telemetry used by engine statistics, `ace explain` and benchmarks.
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
    /// Number of LZ candidates refined during stage one.
    pub hybrid_lz_stage1_candidates: usize,
    /// Number of LZ candidates refined during stage two.
    pub hybrid_lz_stage2_candidates: usize,
    /// Number of LZ candidates skipped because the profile/data-class budget did not justify work.
    pub hybrid_lz_skipped_candidates: usize,
    /// Number of stage-two LZ trials skipped because stage-one analytical/sample agreement was high.
    pub hybrid_lz_high_confidence_skips: usize,
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
    /// Number of TS1 size estimates computed for this block (Planner V5).
    pub time_series_estimates: usize,
    /// True when the FloatFast route selected TS1 without generic analysis (Planner V5).
    pub float_fast_hit: bool,
    /// True when a FloatFast encode missed its bound and the block was re-planned (Planner V5).
    ///
    /// A release gate requires zero fallbacks on Corpus V4.
    pub float_fast_fallback: bool,
}

/// Result returned by the Planner V4 / V5 hot path.
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
    /// Exact NUM1 estimate computed while planning a NumericGeneral block (ACE 0.4.5).
    ///
    /// When the selected plan is plain `Numeric` (no transforms, no entropy stage), the engine
    /// encodes with exactly this `(width, mode)` via `numeric_encode_with` instead of calling
    /// `numeric_encode`, which would repeat the full three-width estimate. BALANCED/DENSE run
    /// the exhaustive search, so the bytes equal `numeric_encode`; FAST estimates only the
    /// prefilter's lane width, which may pick a different (still self-describing) NUM1 layout.
    pub numeric_estimate: Option<ace_codecs::NumericEstimate>,
    /// TS1 selection (Planner V5): layout, estimate and, for FloatFast, the payload already
    /// encoded while validating the route — the engine stores it instead of encoding again.
    pub time_series: Option<TimeSeriesSelection>,
}

/// TS1 plan chosen by Planner V5.
#[derive(Debug, Clone, PartialEq)]
pub struct TimeSeriesSelection {
    /// Estimate that won the comparison (its `layout` is the layout to encode).
    pub estimate: TimeSeriesEstimate,
    /// Payload encoded by the FloatFast route, if any.
    pub payload: Option<Vec<u8>>,
}
