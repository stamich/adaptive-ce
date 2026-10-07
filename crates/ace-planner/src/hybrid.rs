//! Bounded Hybrid LZ estimator frozen for the ACE 0.3.1 hardened release.
//!
//! Buildfix8 deliberately starts from the buildfix6 analytical model. LZ candidates that already
//! survived analytical Top-K are refined with a few deterministic real codec micro-trials. This
//! avoids the expensive full-block match analysis introduced experimentally in buildfix7 while
//! giving the planner direct evidence from the production LZ encoder.

use ace_core::{AceResult, CodecId, CompressionProfile};
use ace_cost::{CostModelV3, EstimatedCandidate};

use crate::encode_plan_payload;

/// Profile- and stage-specific bounded micro-trial budget for one LZ candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HybridLzPolicy {
    /// Width of one deterministic micro-trial window.
    pub window_bytes: usize,
    /// Number of windows sampled across the block.
    pub window_count: usize,
    /// Percentage weight assigned to a pessimistic reset-window sample.
    pub pessimistic_sample_weight: u64,
    /// Percentage weight assigned when the micro-trial is more optimistic than analytics.
    pub optimistic_sample_weight: u64,
}

/// Inherent methods of [`HybridLzPolicy`].
impl HybridLzPolicy {
    /// Returns the bounded micro-trial budget for a profile and verifier stage.
    pub fn for_profile(profile: CompressionProfile, stage: u8) -> Self {
        match (profile, stage >= 2) {
            (CompressionProfile::Fast, false) => Self {
                window_bytes: 8 * 1024,
                window_count: 1,
                pessimistic_sample_weight: 20,
                optimistic_sample_weight: 45,
            },
            (CompressionProfile::Fast, true) => Self {
                window_bytes: 16 * 1024,
                window_count: 1,
                pessimistic_sample_weight: 25,
                optimistic_sample_weight: 50,
            },
            (CompressionProfile::Balanced, false) => Self {
                window_bytes: 8 * 1024,
                window_count: 3,
                pessimistic_sample_weight: 30,
                optimistic_sample_weight: 65,
            },
            (CompressionProfile::Balanced, true) => Self {
                window_bytes: 16 * 1024,
                window_count: 2,
                pessimistic_sample_weight: 35,
                optimistic_sample_weight: 70,
            },
            (CompressionProfile::Dense, false) => Self {
                window_bytes: 12 * 1024,
                window_count: 3,
                pessimistic_sample_weight: 20,
                optimistic_sample_weight: 75,
            },
            (CompressionProfile::Dense, true) => Self {
                window_bytes: 24 * 1024,
                window_count: 2,
                pessimistic_sample_weight: 25,
                optimistic_sample_weight: 80,
            },
        }
    }
}

/// Diagnostic result emitted by one bounded LZ refinement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HybridLzObservation {
    /// Full-block size projection from the real codec micro-trials.
    pub sample_projected_size_bytes: u64,
    /// Final analytical/micro-trial hybrid size used by subsequent ranking.
    pub hybrid_size_bytes: u64,
    /// Total source bytes processed by the bounded micro-trials.
    pub sampled_input_bytes: usize,
    /// Absolute analytical-vs-micro-trial disagreement in parts per million.
    pub disagreement_ppm: u64,
}

/// Bounded production-codec LZ estimator.
#[derive(Debug, Default, Clone, Copy)]
pub struct HybridLzEstimator;

/// Inherent methods of [`HybridLzEstimator`].
impl HybridLzEstimator {
    /// Refines one candidate if it uses LZ; non-LZ candidates are returned unchanged.
    ///
    /// Every micro-trial is encoded independently, so no artificial matches are introduced
    /// between disjoint windows. When reset-window sampling is worse than the analytical model,
    /// the hybrid deliberately trusts the buildfix6 full-block analytical estimate more strongly.
    pub fn refine(
        &self,
        input: &[u8],
        mut candidate: EstimatedCandidate,
        profile: CompressionProfile,
        stage: u8,
        model: &CostModelV3,
    ) -> AceResult<(EstimatedCandidate, HybridLzObservation)> {
        if !matches!(candidate.plan.decoding.codec, CodecId::Lz) || input.is_empty() {
            return Ok((candidate, HybridLzObservation::default()));
        }

        let policy = HybridLzPolicy::for_profile(profile, stage);
        let ranges =
            ace_cost::stratified_ranges(input.len(), policy.window_bytes, policy.window_count);
        if ranges.is_empty() {
            return Ok((candidate, HybridLzObservation::default()));
        }

        let mut sampled_input = 0usize;
        let mut payload_bytes = 0usize;
        let mut metadata_once = 0usize;
        for range in ranges {
            let sample = &input[range];
            let (metadata, payload, _) = encode_plan_payload(sample, &candidate.plan)?;
            sampled_input = sampled_input.saturating_add(sample.len());
            payload_bytes = payload_bytes.saturating_add(payload.len());
            metadata_once = metadata_once.max(metadata.len());
        }

        if sampled_input == 0 {
            return Ok((candidate, HybridLzObservation::default()));
        }

        let projected_payload = ((payload_bytes as u128).saturating_mul(input.len() as u128)
            / sampled_input as u128)
            .min(u64::MAX as u128) as u64;
        let sample_projected = projected_payload.saturating_add(metadata_once as u64);
        let analytical = candidate.analytical_size_bytes.max(1);

        let sample_weight = if sample_projected > analytical {
            policy.pessimistic_sample_weight
        } else {
            policy.optimistic_sample_weight
        };
        let analytical_weight = 100u64.saturating_sub(sample_weight);
        let hybrid = ((sample_projected as u128)
            .saturating_mul(sample_weight as u128)
            .saturating_add((analytical as u128).saturating_mul(analytical_weight as u128))
            / 100u128)
            .min(u64::MAX as u128) as u64;

        let delta = sample_projected.abs_diff(analytical) as u128;
        let disagreement_ppm = delta
            .saturating_mul(1_000_000)
            .checked_div(analytical as u128)
            .unwrap_or(0)
            .min(u64::MAX as u128) as u64;

        candidate.sampled_size_bytes = Some(sample_projected);
        candidate.blended_size_bytes = hybrid;
        candidate.cost.predicted_size_bytes = hybrid;
        candidate.cost.metadata_bytes = metadata_once as u64;
        candidate.score = model.score(profile, candidate.cost, input.len());

        Ok((
            candidate,
            HybridLzObservation {
                sample_projected_size_bytes: sample_projected,
                hybrid_size_bytes: hybrid,
                sampled_input_bytes: sampled_input,
                disagreement_ppm,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// DENSE spends more micro-trial bytes than BALANCED, while both remain bounded.
    #[test]
    fn dense_budget_is_larger_but_bounded() {
        let balanced = HybridLzPolicy::for_profile(CompressionProfile::Balanced, 1);
        let dense = HybridLzPolicy::for_profile(CompressionProfile::Dense, 1);
        assert!(
            dense.window_bytes * dense.window_count
                >= balanced.window_bytes * balanced.window_count
        );
        assert!(dense.window_bytes * dense.window_count <= 64 * 1024);
    }

    /// Deterministic range generation must never depend on process state or randomness.
    #[test]
    fn ranges_are_deterministic() {
        let a = ace_cost::stratified_ranges(262_144, 8 * 1024, 3);
        let b = ace_cost::stratified_ranges(262_144, 8 * 1024, 3);
        assert_eq!(a, b);
        assert_eq!(a.len(), 3);
    }
}
