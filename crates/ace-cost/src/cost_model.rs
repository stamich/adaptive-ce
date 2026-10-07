//! Scalar cost model turning a multidimensional [`PlanCost`] into a sortable score.

use ace_core::{CompressionProfile, CostWeights, PlanCost};

/// Deterministic ACE 0.3 multidimensional cost model.
#[derive(Debug, Default, Clone, Copy)]
pub struct CostModelV3;

/// Inherent methods of [`CostModelV3`].
impl CostModelV3 {
    /// Converts normalized size, CPU and memory estimates into a stable sortable scalar.
    pub fn score(&self, profile: CompressionProfile, cost: PlanCost, input_bytes: usize) -> u128 {
        let weights = CostWeights::for_profile(profile);
        let divisor = input_bytes.max(1) as u128;
        let size_ppm = (cost.predicted_size_bytes as u128).saturating_mul(1_000_000) / divisor;
        let encode_per_byte = cost.encode_units as u128 / divisor;
        let decode_per_byte = cost.decode_units as u128 / divisor;
        let memory_per_byte = cost.memory_bytes as u128 / divisor;
        let (encode_scale, decode_scale) = match profile {
            CompressionProfile::Fast => (34_000u128, 6_000u128),
            CompressionProfile::Balanced => (3_500u128, 1_300u128),
            CompressionProfile::Dense => (550u128, 300u128),
        };
        size_ppm
            .saturating_mul(weights.size as u128)
            .saturating_add(
                encode_per_byte
                    .saturating_mul(encode_scale)
                    .saturating_mul(weights.encode_cpu as u128),
            )
            .saturating_add(
                decode_per_byte
                    .saturating_mul(decode_scale)
                    .saturating_mul(weights.decode_cpu as u128),
            )
            .saturating_add(
                memory_per_byte
                    .saturating_mul(1_000)
                    .saturating_mul(weights.memory as u128),
            )
    }
}
