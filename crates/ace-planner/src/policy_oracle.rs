use ace_core::{AceConfig, BlockProfile, PhysicalCompressionPlan};

use crate::{
    CandidatePreference, DominanceEnvelope, DominancePolicy, DominanceReason, PlannerRoute,
};

/// One fully measured candidate supplied to the offline policy oracle.
#[derive(Debug, Clone, Copy)]
pub struct PolicyOracleCandidate<'a> {
    /// Physical candidate plan.
    pub plan: &'a PhysicalCompressionPlan,
    /// Real encoded byte size measured by benchmark/test oracle code.
    pub encoded_bytes: u64,
}

/// Policy-oracle result retaining the selected preference and explanation.
#[derive(Debug, Clone, Copy)]
pub struct PolicyOracleDecision<'a> {
    /// Candidate selected by product policy.
    pub candidate: PolicyOracleCandidate<'a>,
    /// Preference class responsible for the final policy tier.
    pub preference: CandidatePreference,
    /// Explainable dominance reason.
    pub reason: DominanceReason,
    /// Extra bytes accepted relative to the smallest route-eligible candidate.
    pub accepted_size_loss_bytes: u64,
}

/// Offline policy oracle that mirrors production route/dominance semantics.
///
/// The oracle is intentionally not called by the production hot path because its benchmark/test
/// input candidates contain real full-encode sizes. Production continues to use estimators and
/// bounded verification; this type exists to evaluate whether that decision matches product policy.
#[derive(Debug, Default, Clone, Copy)]
pub struct PolicyOracle;

/// Inherent methods of [`PolicyOracle`].
impl PolicyOracle {
    /// Chooses the best candidate under route eligibility, dominance preference and size envelope.
    pub fn choose<'a>(
        route: PlannerRoute,
        candidates: &'a [PolicyOracleCandidate<'a>],
        profile: &BlockProfile,
        config: &AceConfig,
    ) -> Option<PolicyOracleDecision<'a>> {
        let mut eligible = candidates
            .iter()
            .filter_map(|candidate| {
                let (preference, reason) =
                    DominancePolicy::preference(route, candidate.plan, profile, config);
                if matches!(
                    preference,
                    CandidatePreference::DiagnosticOnly | CandidatePreference::Rejected
                ) {
                    None
                } else {
                    Some((*candidate, preference, reason))
                }
            })
            .collect::<Vec<_>>();

        if eligible.is_empty() {
            return None;
        }

        eligible.sort_by_key(|(candidate, _, _)| candidate.encoded_bytes);
        let best_bytes = eligible[0].0.encoded_bytes;
        let envelope = DominanceEnvelope::for_config(config);

        for preference in [
            CandidatePreference::Preferred,
            CandidatePreference::Neutral,
            CandidatePreference::Penalized,
        ] {
            if let Some((candidate, actual_preference, reason)) = eligible
                .iter()
                .filter(|(_, candidate_preference, _)| *candidate_preference == preference)
                .filter(|(candidate, _, _)| envelope.admits(best_bytes, candidate.encoded_bytes))
                .min_by_key(|(candidate, _, _)| candidate.encoded_bytes)
                .copied()
            {
                return Some(PolicyOracleDecision {
                    candidate,
                    preference: actual_preference,
                    reason,
                    accepted_size_loss_bytes: candidate.encoded_bytes.saturating_sub(best_bytes),
                });
            }
        }

        let (candidate, preference, reason) = eligible[0];
        Some(PolicyOracleDecision {
            candidate,
            preference,
            reason,
            accepted_size_loss_bytes: 0,
        })
    }
}
