//! Profile-aware quality envelope for ACE Planner V3.3.
//!
//! The envelope separates two concerns that were previously conflated by the scalar cost model:
//! first preserve compression quality, then optimize CPU/memory cost among quality-qualified plans.

use crate::EstimatedCandidate;
use ace_core::CompressionProfile;

/// Deterministic size-quality constraint applied before the final scalar cost-model selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QualityEnvelope {
    /// Maximum tolerated size slack over the best blended-size candidate, in parts per million.
    pub slack_ppm: u64,
}

/// Inherent methods of [`QualityEnvelope`].
impl QualityEnvelope {
    /// Returns the default quality envelope for a public compression profile.
    ///
    /// FAST keeps a wide envelope because throughput is the primary objective. BALANCED allows
    /// only a small quality loss, while DENSE stays very close to the smallest predicted output.
    pub fn for_profile(profile: CompressionProfile) -> Self {
        let slack_ppm = match profile {
            CompressionProfile::Fast => 250_000,    // +25.0%
            CompressionProfile::Balanced => 15_000, // +1.5%
            CompressionProfile::Dense => 3_000,     // +0.3%
        };
        Self { slack_ppm }
    }

    /// Computes the inclusive maximum blended size admitted by this envelope.
    pub fn limit_bytes(&self, best_size_bytes: u64) -> u64 {
        let extra = (best_size_bytes as u128)
            .saturating_mul(self.slack_ppm as u128)
            .saturating_add(999_999)
            / 1_000_000u128;
        (best_size_bytes as u128)
            .saturating_add(extra)
            .min(u64::MAX as u128) as u64
    }

    /// Returns true when `candidate_size_bytes` is inside the envelope around `best_size_bytes`.
    pub fn qualifies(&self, best_size_bytes: u64, candidate_size_bytes: u64) -> bool {
        candidate_size_bytes <= self.limit_bytes(best_size_bytes)
    }

    /// Returns all candidates whose blended size stays within the profile quality envelope.
    ///
    /// The returned order is intentionally unchanged. The caller may then apply the scalar
    /// cost-model order to choose the cheapest CPU/memory plan among quality-safe alternatives.
    pub fn qualify(&self, candidates: &[EstimatedCandidate]) -> Vec<EstimatedCandidate> {
        let Some(best) = candidates.iter().map(|c| c.blended_size_bytes).min() else {
            return Vec::new();
        };
        let limit = self.limit_bytes(best);
        candidates
            .iter()
            .filter(|candidate| candidate.blended_size_bytes <= limit)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies profile envelopes become progressively stricter from FAST to DENSE.
    #[test]
    fn profiles_have_monotonic_quality_slack() {
        let fast = QualityEnvelope::for_profile(CompressionProfile::Fast).slack_ppm;
        let balanced = QualityEnvelope::for_profile(CompressionProfile::Balanced).slack_ppm;
        let dense = QualityEnvelope::for_profile(CompressionProfile::Dense).slack_ppm;
        assert!(fast > balanced);
        assert!(balanced > dense);
    }

    /// Verifies the quality limit uses inclusive deterministic integer arithmetic.
    #[test]
    fn inclusive_limit_is_deterministic() {
        let envelope = QualityEnvelope { slack_ppm: 10_000 };
        assert_eq!(envelope.limit_bytes(10_000), 10_100);
        assert!(envelope.qualifies(10_000, 10_100));
        assert!(!envelope.qualifies(10_000, 10_101));
    }

    /// Builds a small deterministic candidate for envelope filtering tests.
    fn candidate(size: u64, score: u128) -> EstimatedCandidate {
        let mut plan = ace_core::PhysicalCompressionPlan::raw();
        plan.cost.predicted_size_bytes = size;
        plan.score = score;
        EstimatedCandidate {
            plan,
            analytical_size_bytes: size,
            sampled_size_bytes: None,
            blended_size_bytes: size,
            cost: ace_core::PlanCost {
                predicted_size_bytes: size,
                metadata_bytes: 0,
                encode_units: 1,
                decode_units: 1,
                memory_bytes: 1,
            },
            score,
            confidence: 1.0,
        }
    }

    /// Verifies a very cheap CPU candidate cannot enter a strict DENSE pool when its size is too large.
    #[test]
    fn dense_envelope_rejects_cpu_cheap_but_oversized_candidate() {
        let envelope = QualityEnvelope::for_profile(CompressionProfile::Dense);
        let candidates = vec![
            candidate(100_000, 10_000),
            candidate(100_200, 20_000),
            candidate(101_000, 1),
        ];
        let qualified = envelope.qualify(&candidates);
        assert_eq!(qualified.len(), 2);
        assert!(qualified
            .iter()
            .all(|candidate| candidate.blended_size_bytes <= 100_300));
    }

    /// Verifies BALANCED admits a modest size trade-off while still excluding a large ratio loss.
    #[test]
    fn balanced_envelope_allows_small_but_not_large_quality_tradeoff() {
        let envelope = QualityEnvelope::for_profile(CompressionProfile::Balanced);
        let candidates = vec![
            candidate(100_000, 30),
            candidate(101_000, 10),
            candidate(103_000, 1),
        ];
        let qualified = envelope.qualify(&candidates);
        assert_eq!(qualified.len(), 2);
        assert!(qualified
            .iter()
            .any(|candidate| candidate.blended_size_bytes == 101_000));
        assert!(!qualified
            .iter()
            .any(|candidate| candidate.blended_size_bytes == 103_000));
    }
}
