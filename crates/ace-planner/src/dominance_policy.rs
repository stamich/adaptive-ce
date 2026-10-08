use ace_core::{
    AccessHint, AceConfig, BlockProfile, CodecId, CompressionProfile, PhysicalCompressionPlan,
};

use crate::{CandidateEligibility, PlannerRoute, RoutePolicy};

/// Product-level preference applied after route eligibility but before policy-oracle size ranking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CandidatePreference {
    /// Candidate is explicitly preferred for the detected data/access pattern.
    Preferred,
    /// Candidate is production-eligible with no additional preference.
    Neutral,
    /// Candidate remains eligible but should lose to a near-size preferred/neutral alternative.
    Penalized,
    /// Candidate is visible only to the global diagnostic oracle.
    DiagnosticOnly,
    /// Candidate is irrelevant to the current route.
    Rejected,
}

/// Explainable reason for one product-level preference decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DominanceReason {
    /// No special codec family dominates the workload.
    NoDominance,
    /// RLE is preferred for zero/run-heavy blocks because decode is exceptionally cheap.
    RunLengthDominance,
    /// RAW is preferred for incompressible data to avoid useless codec work.
    IncompressibleRawDominance,
    /// Numeric is preferred for an admitted numeric route.
    NumericDominance,
    /// Random-access policy penalizes expensive decode families.
    RandomAccessPreference,
}

/// Maximum size premium accepted when a cheaper-decode candidate is product-preferred.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DominanceEnvelope {
    /// Maximum extra bytes accepted relative to the smallest route-eligible candidate.
    pub max_absolute_size_loss: u64,
    /// Maximum fractional size loss accepted for ordinary-size route candidates.
    pub max_relative_size_loss: f64,
}

/// Inherent methods of [`DominanceEnvelope`].
impl DominanceEnvelope {
    /// Returns the profile/access-specific preference envelope.
    pub fn for_config(config: &AceConfig) -> Self {
        let (mut absolute, mut relative): (u64, f64) = match config.profile {
            CompressionProfile::Fast => (4096_u64, 0.02_f64),
            CompressionProfile::Balanced => (2048_u64, 0.01_f64),
            CompressionProfile::Dense => (512_u64, 0.005_f64),
        };
        if matches!(config.access_hint, AccessHint::RandomAccess) {
            absolute = absolute.saturating_mul(2);
            relative *= 2.0;
        }
        Self {
            max_absolute_size_loss: absolute,
            max_relative_size_loss: relative,
        }
    }

    /// Returns whether `candidate_bytes` is close enough to `best_bytes` for preference to win.
    pub fn admits(self, best_bytes: u64, candidate_bytes: u64) -> bool {
        if candidate_bytes <= best_bytes {
            return true;
        }
        let loss = candidate_bytes.saturating_sub(best_bytes);
        let relative = if best_bytes == 0 {
            f64::INFINITY
        } else {
            loss as f64 / best_bytes as f64
        };
        loss <= self.max_absolute_size_loss || relative <= self.max_relative_size_loss
    }
}

/// Product-level dominance policy shared by policy-oracle benchmarks and explain tooling.
#[derive(Debug, Default, Clone, Copy)]
pub struct DominancePolicy;

/// Inherent methods of [`DominancePolicy`].
impl DominancePolicy {
    /// Assigns a preference class and reason to one physical plan.
    pub fn preference(
        route: PlannerRoute,
        plan: &PhysicalCompressionPlan,
        profile: &BlockProfile,
        config: &AceConfig,
    ) -> (CandidatePreference, DominanceReason) {
        match RoutePolicy::candidate_eligibility(route, plan, profile, config) {
            CandidateEligibility::DiagnosticOnly(_) => {
                return (
                    CandidatePreference::DiagnosticOnly,
                    DominanceReason::NoDominance,
                )
            }
            CandidateEligibility::Rejected(_) => {
                return (CandidatePreference::Rejected, DominanceReason::NoDominance)
            }
            CandidateEligibility::Allowed => {}
        }

        if matches!(
            route,
            PlannerRoute::NumericFast | PlannerRoute::NumericGeneral
        ) && matches!(plan.decoding.codec, CodecId::Numeric)
        {
            return (
                CandidatePreference::Preferred,
                DominanceReason::NumericDominance,
            );
        }

        let run_heavy = profile.zero_ratio >= 0.95 || profile.run_score >= 0.85;
        if run_heavy {
            return match plan.decoding.codec {
                CodecId::Rle => (
                    CandidatePreference::Preferred,
                    DominanceReason::RunLengthDominance,
                ),
                CodecId::Raw => (
                    CandidatePreference::Neutral,
                    DominanceReason::RunLengthDominance,
                ),
                _ => (
                    CandidatePreference::Penalized,
                    DominanceReason::RunLengthDominance,
                ),
            };
        }

        if profile.incompressibility_score >= 0.90 {
            return if matches!(plan.decoding.codec, CodecId::Raw) {
                (
                    CandidatePreference::Preferred,
                    DominanceReason::IncompressibleRawDominance,
                )
            } else {
                (
                    CandidatePreference::Penalized,
                    DominanceReason::IncompressibleRawDominance,
                )
            };
        }

        if matches!(config.access_hint, AccessHint::RandomAccess) {
            return match plan.decoding.codec {
                CodecId::Raw | CodecId::Rle => (
                    CandidatePreference::Preferred,
                    DominanceReason::RandomAccessPreference,
                ),
                CodecId::Numeric | CodecId::TimeSeries | CodecId::Lz => (
                    CandidatePreference::Penalized,
                    DominanceReason::RandomAccessPreference,
                ),
            };
        }

        (CandidatePreference::Neutral, DominanceReason::NoDominance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ace_core::{CandidateTier, DecodingPlan, EntropyCodecId, PlanCost};

    /// Builds a minimal plan used to exercise preference semantics.
    fn plan(codec: CodecId) -> PhysicalCompressionPlan {
        PhysicalCompressionPlan {
            decoding: DecodingPlan {
                transforms: Vec::new(),
                codec,
                dictionary: None,
                entropy: EntropyCodecId::None,
            },
            lz_mode: None,
            tier: CandidateTier::Likely,
            cost: PlanCost::default(),
            score: 0,
            reason: "dominance-policy test",
        }
    }

    /// Builds a block profile focused on zero/run-heavy behavior.
    fn run_profile() -> BlockProfile {
        BlockProfile {
            size: 256 * 1024,
            entropy_h0: 0.1,
            entropy_h1: 0.1,
            zero_ratio: 1.0,
            run_score: 1.0,
            delta_score: 1.0,
            repetition_score: 1.0,
            sampled_match_length: 64.0,
            unique_byte_count: 1,
            incompressibility_score: 0.0,
        }
    }

    /// RLE is product-preferred for zero/run-heavy generic blocks.
    #[test]
    fn run_heavy_prefers_rle() {
        let config = AceConfig::default();
        let profile = run_profile();
        let (preference, reason) = DominancePolicy::preference(
            PlannerRoute::Generic,
            &plan(CodecId::Rle),
            &profile,
            &config,
        );
        assert_eq!(preference, CandidatePreference::Preferred);
        assert_eq!(reason, DominanceReason::RunLengthDominance);
    }

    /// BALANCED permits the expected small absolute policy premium.
    #[test]
    fn balanced_envelope_accepts_small_absolute_loss() {
        let config = AceConfig::default();
        let envelope = DominanceEnvelope::for_config(&config);
        assert!(envelope.admits(520, 772));
    }
}
