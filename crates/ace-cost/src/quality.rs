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
