//! Entropy-coder selection policy (when rANS is worth its cost over Huffman).

use ace_core::{CompressionProfile, EntropyCodecId, PhysicalCompressionPlan};
use ace_cost::EstimatedCandidate;

/// Deterministic policy controlling when scalar rANS is worth considering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntropySelectionPolicy {
    /// Minimum primary-stream size before rANS may be considered.
    pub min_rans_input_size: usize,
    /// Minimum fractional size advantage over Huffman a rANS plan must show.
    pub min_rans_gain_fraction: f32,
}

/// Anything ranked by the planner that carries a plan, a predicted size and a score.
///
/// Implemented for fully evaluated plans and for analytical estimates so the rANS gain rule
/// below is written once for both evaluators.
pub trait RankedPlan {
    /// The physical plan being ranked.
    fn plan(&self) -> &PhysicalCompressionPlan;
    /// Predicted encoded size in bytes.
    fn predicted_size(&self) -> u64;
    /// Mutable access to the ranking score (lower is better).
    fn score_mut(&mut self) -> &mut u128;
}

/// [`RankedPlan`] implementation for [`PhysicalCompressionPlan`].
impl RankedPlan for PhysicalCompressionPlan {
    /// See [`RankedPlan::plan`].
    fn plan(&self) -> &PhysicalCompressionPlan {
        self
    }
    /// See [`RankedPlan::predicted_size`].
    fn predicted_size(&self) -> u64 {
        self.cost.predicted_size_bytes
    }
    /// See [`RankedPlan::score_mut`].
    fn score_mut(&mut self) -> &mut u128 {
        &mut self.score
    }
}

/// [`RankedPlan`] implementation for [`EstimatedCandidate`].
impl RankedPlan for EstimatedCandidate {
    /// See [`RankedPlan::plan`].
    fn plan(&self) -> &PhysicalCompressionPlan {
        &self.plan
    }
    /// See [`RankedPlan::predicted_size`].
    fn predicted_size(&self) -> u64 {
        self.cost.predicted_size_bytes
    }
    /// See [`RankedPlan::score_mut`].
    fn score_mut(&mut self) -> &mut u128 {
        &mut self.score
    }
}

/// Inherent methods of [`EntropySelectionPolicy`].
impl EntropySelectionPolicy {
    /// Returns the calibrated policy for one public compression profile.
    pub fn for_profile(profile: CompressionProfile) -> Self {
        match profile {
            CompressionProfile::Fast => Self {
                min_rans_input_size: 16 * 1024,
                min_rans_gain_fraction: 0.05,
            },
            CompressionProfile::Balanced => Self {
                min_rans_input_size: 4 * 1024,
                min_rans_gain_fraction: 0.02,
            },
            CompressionProfile::Dense => Self {
                min_rans_input_size: 1024,
                min_rans_gain_fraction: 0.0,
            },
        }
    }

    /// Returns true when the input is large enough to amortize scalar rANS metadata/setup costs.
    pub fn input_allows_rans(&self, input_size: usize) -> bool {
        input_size >= self.min_rans_input_size
    }

    /// Pushes rANS / rANS4x plans to the end of the ranking when they do not beat the
    /// equivalent Huffman plan (same transforms, codec and LZ mode) by the minimum gain.
    pub fn penalize_weak_rans<T: RankedPlan>(&self, items: &mut [T]) {
        let penalized: Vec<usize> = (0..items.len())
            .filter(|&index| self.is_weak_rans(items, index))
            .collect();
        for index in penalized {
            let score = items[index].score_mut();
            *score = score.saturating_add(u128::MAX / 4);
        }
    }

    /// True when `items[index]` is a rANS plan whose gain over its Huffman peer is too small.
    fn is_weak_rans<T: RankedPlan>(&self, items: &[T], index: usize) -> bool {
        let candidate = items[index].plan();
        if !matches!(
            candidate.decoding.entropy,
            EntropyCodecId::Rans | EntropyCodecId::Rans4x
        ) {
            return false;
        }
        let Some(huffman) = items.iter().find(|other| {
            let peer = other.plan();
            matches!(peer.decoding.entropy, EntropyCodecId::Huffman)
                && peer.decoding.transforms == candidate.decoding.transforms
                && peer.decoding.codec == candidate.decoding.codec
                && peer.lz_mode == candidate.lz_mode
        }) else {
            return false;
        };
        let huffman_size = huffman.predicted_size().max(1) as f64;
        let rans_size = items[index].predicted_size() as f64;
        let gain = ((huffman_size - rans_size) / huffman_size).max(0.0) as f32;
        gain < self.min_rans_gain_fraction
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ace_core::CodecId;

    /// Builds a RLE plan with the given entropy coder and predicted size.
    fn plan(entropy: EntropyCodecId, size: u64) -> PhysicalCompressionPlan {
        let mut plan = PhysicalCompressionPlan::raw();
        plan.decoding.codec = CodecId::Rle;
        plan.decoding.entropy = entropy;
        plan.cost.predicted_size_bytes = size;
        plan.score = size as u128;
        plan
    }

    /// rANS is penalized only when it does not beat its Huffman peer by the profile minimum.
    #[test]
    fn weak_rans_is_penalized_strong_rans_is_kept() {
        let policy = EntropySelectionPolicy::for_profile(CompressionProfile::Fast); // 5 % minimum
        let mut plans = vec![
            plan(EntropyCodecId::Huffman, 1000),
            plan(EntropyCodecId::Rans, 990),
            plan(EntropyCodecId::Rans4x, 900),
        ];
        policy.penalize_weak_rans(&mut plans);
        assert_eq!(plans[0].score, 1000);
        assert!(
            plans[1].score > u128::MAX / 8,
            "1 % gain is below the FAST minimum"
        );
        assert_eq!(plans[2].score, 900, "10 % gain is kept");

        let mut alone = vec![plan(EntropyCodecId::Rans, 990)];
        policy.penalize_weak_rans(&mut alone);
        assert_eq!(alone[0].score, 990, "no Huffman peer → no penalty");
    }
}
