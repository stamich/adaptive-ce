use ace_core::{
    AceConfig, BlockProfile, CodecId, CompressionProfile, LzMode, PhysicalCompressionPlan,
};

use crate::{classify_float_lane, classify_planner_route, PlannerRoute, RouteDecision};

/// Eligibility assigned by Planner V4.3 to a physical candidate under a concrete route.
///
/// `DiagnosticOnly` candidates remain visible to offline global-oracle analysis but are not
/// allowed to influence production route-aware ranking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateEligibility {
    /// Candidate participates in production route-aware planning.
    Allowed,
    /// Candidate is excluded from production but may be evaluated by diagnostic global oracles.
    DiagnosticOnly(RouteRejectionReason),
    /// Candidate is not meaningful for this route and should not be evaluated.
    Rejected(RouteRejectionReason),
}

/// Stable reason explaining why a candidate is outside the production route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteRejectionReason {
    /// Numeric is intentionally excluded from the Generic route.
    NumericOutsideNumericRoute,
    /// Non-Numeric candidates are diagnostic-only after NumericFast validation.
    NonNumericOutsideFastRoute,
    /// Transform/codec family is outside the bounded NumericGeneral search space.
    OutsideNumericGeneralBudget,
}

/// Hybrid-LZ work permitted by one route budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteHybridLzPolicy {
    /// Do not perform Hybrid-LZ microtrials.
    Disabled,
    /// Permit only one stage-one Hybrid-LZ refinement.
    OneStage,
    /// Preserve the hardened generic two-stage Hybrid-LZ policy.
    Full,
}

/// Work budget associated with one Planner V4.3 route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteBudget {
    /// Maximum number of candidates entering analytical route-aware evaluation.
    pub max_generated_candidates: usize,
    /// Maximum number of candidates entering sample verification.
    pub max_sampled_candidates: usize,
    /// Default Hybrid-LZ policy before Numeric dominance is known.
    pub hybrid_lz_policy: RouteHybridLzPolicy,
    /// Whether the generic second-stage verifier is allowed.
    pub allow_second_stage: bool,
}

/// Inherent methods of [`RouteBudget`].
impl RouteBudget {
    /// Returns the deterministic default budget for a route/profile pair.
    pub fn for_route(route: PlannerRoute, profile: CompressionProfile) -> Self {
        match route {
            PlannerRoute::NumericFast | PlannerRoute::FloatFast => Self {
                max_generated_candidates: 1,
                max_sampled_candidates: 0,
                hybrid_lz_policy: RouteHybridLzPolicy::Disabled,
                allow_second_stage: false,
            },
            PlannerRoute::NumericGeneral => Self {
                max_generated_candidates: 5,
                max_sampled_candidates: 2,
                hybrid_lz_policy: RouteHybridLzPolicy::OneStage,
                allow_second_stage: false,
            },
            // FloatGeneral runs its base route's pipeline; this budget applies when the base is
            // Generic (the evaluator always receives `RouteDecision::candidate_route`).
            PlannerRoute::Generic | PlannerRoute::FloatGeneral => Self {
                max_generated_candidates: match profile {
                    CompressionProfile::Fast => 8,
                    CompressionProfile::Balanced => 16,
                    CompressionProfile::Dense => 20,
                },
                max_sampled_candidates: match profile {
                    CompressionProfile::Fast => 3,
                    CompressionProfile::Balanced => 8,
                    CompressionProfile::Dense => 10,
                },
                hybrid_lz_policy: RouteHybridLzPolicy::Full,
                allow_second_stage: true,
            },
        }
    }
}

/// Relative size margin describing Numeric dominance over the best generic analytical estimate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NumericMargin {
    /// Exact Numeric size estimate in bytes.
    pub numeric_bytes: u64,
    /// Best generic analytical size estimate in bytes.
    pub best_generic_bytes: u64,
    /// Signed `generic - numeric` byte advantage.
    pub margin_bytes: i64,
    /// `numeric / generic`; smaller means stronger Numeric dominance.
    pub ratio: f64,
}

/// Inherent methods of [`NumericMargin`].
impl NumericMargin {
    /// Builds a stable dominance descriptor from exact Numeric and generic sizes.
    pub fn new(numeric_bytes: u64, best_generic_bytes: u64) -> Self {
        let ratio = if best_generic_bytes == 0 {
            1.0
        } else {
            numeric_bytes as f64 / best_generic_bytes as f64
        };
        Self {
            numeric_bytes,
            best_generic_bytes,
            margin_bytes: best_generic_bytes as i64 - numeric_bytes as i64,
            ratio,
        }
    }

    /// Chooses Hybrid-LZ work according to profile-specific Numeric dominance.
    pub fn hybrid_lz_policy(self, profile: CompressionProfile) -> RouteHybridLzPolicy {
        let disabled_threshold = match profile {
            CompressionProfile::Fast => 0.70,
            CompressionProfile::Balanced => 0.80,
            CompressionProfile::Dense => 0.90,
        };
        if self.ratio <= disabled_threshold {
            RouteHybridLzPolicy::Disabled
        } else if self.ratio <= 0.97 {
            RouteHybridLzPolicy::OneStage
        } else {
            RouteHybridLzPolicy::Full
        }
    }
}

/// Single source of truth for production route classification and candidate eligibility.
#[derive(Debug, Default, Clone, Copy)]
pub struct RoutePolicy;

/// Inherent methods of [`RoutePolicy`].
impl RoutePolicy {
    /// Classifies one input block: Planner V4.3 routing, then the Planner V5 Float lane.
    ///
    /// Order (cheap → expensive): NumericFast validation, float prefilter (FloatFast /
    /// FloatGeneral), the V4.3 NumericGeneral / Generic verdict. Floats are checked before
    /// NumericGeneral because f64 bit patterns often look like monotonic u64 values.
    pub fn classify(input: &[u8], config: &AceConfig) -> RouteDecision {
        classify_float_lane(input, config, classify_planner_route(input, config))
    }

    /// Returns whether a candidate may participate in the selected production route.
    ///
    /// The policy deliberately keeps RAW/RLE and at most one variant of each LZ mode in
    /// `NumericGeneral`. Entropy variants outside that compact lane remain diagnostic-only.
    pub fn candidate_eligibility(
        route: PlannerRoute,
        plan: &PhysicalCompressionPlan,
        _profile: &BlockProfile,
        _config: &AceConfig,
    ) -> CandidateEligibility {
        match route {
            PlannerRoute::Generic | PlannerRoute::FloatGeneral | PlannerRoute::FloatFast => {
                if matches!(plan.decoding.codec, CodecId::Numeric) {
                    CandidateEligibility::DiagnosticOnly(
                        RouteRejectionReason::NumericOutsideNumericRoute,
                    )
                } else {
                    CandidateEligibility::Allowed
                }
            }
            PlannerRoute::NumericFast => {
                if matches!(plan.decoding.codec, CodecId::Numeric) {
                    CandidateEligibility::Allowed
                } else {
                    CandidateEligibility::DiagnosticOnly(
                        RouteRejectionReason::NonNumericOutsideFastRoute,
                    )
                }
            }
            PlannerRoute::NumericGeneral => match plan.decoding.codec {
                // TS1 RunDelta competes on integer lanes (Planner V5).
                CodecId::Numeric | CodecId::Raw | CodecId::Rle | CodecId::TimeSeries => {
                    CandidateEligibility::Allowed
                }
                CodecId::Lz if matches!(plan.lz_mode, Some(LzMode::Fast | LzMode::Balanced)) => {
                    CandidateEligibility::Allowed
                }
                _ => CandidateEligibility::DiagnosticOnly(
                    RouteRejectionReason::OutsideNumericGeneralBudget,
                ),
            },
        }
    }

    /// Convenience predicate used by production filtering and route-aware oracle code.
    pub fn candidate_allowed(
        route: PlannerRoute,
        plan: &PhysicalCompressionPlan,
        profile: &BlockProfile,
        config: &AceConfig,
    ) -> bool {
        matches!(
            Self::candidate_eligibility(route, plan, profile, config),
            CandidateEligibility::Allowed
        )
    }

    /// Returns the route's initial work budget before Numeric dominance is measured.
    pub fn budget(route: PlannerRoute, config: &AceConfig) -> RouteBudget {
        RouteBudget::for_route(route, config.profile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ace_core::{CandidateTier, DecodingPlan, EntropyCodecId, PlanCost};

    /// Builds a neutral block profile for route-policy unit tests.
    fn test_profile() -> BlockProfile {
        BlockProfile {
            size: 256 * 1024,
            entropy_h0: 4.0,
            entropy_h1: 3.0,
            zero_ratio: 0.0,
            run_score: 0.0,
            delta_score: 0.2,
            repetition_score: 0.1,
            sampled_match_length: 8.0,
            unique_byte_count: 128,
            incompressibility_score: 0.2,
        }
    }

    /// Builds a minimal test plan without invoking candidate generation.
    fn test_plan(codec: CodecId, lz_mode: Option<LzMode>) -> PhysicalCompressionPlan {
        PhysicalCompressionPlan {
            decoding: DecodingPlan {
                transforms: Vec::new(),
                codec,
                dictionary: None,
                entropy: EntropyCodecId::None,
            },
            lz_mode,
            tier: CandidateTier::Likely,
            cost: PlanCost::default(),
            score: 0,
            reason: "route-policy test plan",
        }
    }

    /// Generic route must exclude Numeric from release-gate oracle eligibility.
    #[test]
    fn generic_marks_numeric_diagnostic_only() {
        let profile = test_profile();
        let config = AceConfig::default();
        assert!(matches!(
            RoutePolicy::candidate_eligibility(
                PlannerRoute::Generic,
                &test_plan(CodecId::Numeric, None),
                &profile,
                &config,
            ),
            CandidateEligibility::DiagnosticOnly(_)
        ));
    }

    /// NumericGeneral exposes only the intentionally bounded physical families.
    #[test]
    fn numeric_general_allows_numeric_raw_rle_and_lz() {
        let profile = test_profile();
        let config = AceConfig::default();
        for plan in [
            test_plan(CodecId::Numeric, None),
            test_plan(CodecId::Raw, None),
            test_plan(CodecId::Rle, None),
            test_plan(CodecId::Lz, Some(LzMode::Fast)),
            test_plan(CodecId::Lz, Some(LzMode::Balanced)),
        ] {
            assert!(RoutePolicy::candidate_allowed(
                PlannerRoute::NumericGeneral,
                &plan,
                &profile,
                &config,
            ));
        }
    }

    /// Strong Numeric dominance disables Hybrid-LZ work for BALANCED.
    #[test]
    fn numeric_margin_disables_hybrid_lz_when_dominant() {
        let margin = NumericMargin::new(100, 1_000);
        assert_eq!(
            margin.hybrid_lz_policy(CompressionProfile::Balanced),
            RouteHybridLzPolicy::Disabled
        );
    }
}
