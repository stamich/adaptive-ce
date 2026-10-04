use ace_core::{
    CodecId, CompressionProfile, CostWeights, EntropyCodecId, LzMode,
    PhysicalCompressionPlan, PlanCost,
};

/// Deterministic ACE 0.2.1-buildfix1 cost model.
///
/// The original 0.2.1 score mixed raw byte counts with already-normalized CPU terms,
/// which made the size component dominate even for `CompressionProfile::Fast`.
/// Buildfix1 converts every dimension into a bounded per-input-byte representation
/// before applying profile-specific weights and CPU scales.
#[derive(Debug, Default, Clone, Copy)]
pub struct DeterministicCostModel;

impl DeterministicCostModel {
    /// Computes multidimensional cost from encoded size, explicit metadata and static work coefficients.
    pub fn cost(
        &self,
        plan: &PhysicalCompressionPlan,
        input_bytes: usize,
        encoded_bytes: usize,
        metadata_bytes: usize,
    ) -> PlanCost {
        let n = input_bytes as u64;
        let codec_encode = match (plan.decoding.codec, plan.lz_mode) {
            (CodecId::Raw, _) => 1,
            (CodecId::Rle, _) => 2,
            (CodecId::Lz, Some(LzMode::Balanced)) => 14,
            (CodecId::Lz, _) => 5,
        };
        let codec_decode = match plan.decoding.codec {
            CodecId::Raw => 1,
            CodecId::Rle => 2,
            CodecId::Lz => 3,
        };
        // ACE 0.2 measured scalar rANS as materially slower than Huffman at encode time.
        let entropy_encode = match plan.decoding.entropy {
            EntropyCodecId::None => 0,
            EntropyCodecId::Huffman => 5,
            EntropyCodecId::Rans => 9,
        };
        let entropy_decode = match plan.decoding.entropy {
            EntropyCodecId::None => 0,
            EntropyCodecId::Huffman => 7,
            EntropyCodecId::Rans => 5,
        };
        let transform_units = plan.decoding.transforms.len() as u64;
        PlanCost {
            predicted_size_bytes: encoded_bytes as u64,
            metadata_bytes: metadata_bytes as u64,
            encode_units: n.saturating_mul(codec_encode + entropy_encode + transform_units),
            decode_units: n.saturating_mul(codec_decode + entropy_decode + transform_units),
            memory_bytes: (input_bytes.saturating_mul(3).saturating_add(encoded_bytes)) as u64,
        }
    }

    /// Converts multidimensional cost into a stable sortable scalar using normalized dimensions.
    ///
    /// Size is represented as parts-per-million of the original block size. CPU work is
    /// represented as static work units per input byte. Profile-specific scale factors make
    /// encoder work competitive with size only for FAST, while BALANCED and DENSE remain
    /// progressively more size-oriented. No wall-clock measurement participates in selection.
    pub fn score(
        &self,
        profile: CompressionProfile,
        cost: PlanCost,
        input_bytes: usize,
    ) -> u128 {
        let w = CostWeights::for_profile(profile);
        let divisor = input_bytes.max(1) as u128;

        let size_ppm = (cost.predicted_size_bytes as u128)
            .saturating_mul(1_000_000)
            / divisor;
        let encode_units_per_byte = cost.encode_units as u128 / divisor;
        let decode_units_per_byte = cost.decode_units as u128 / divisor;
        let memory_bytes_per_byte = cost.memory_bytes as u128 / divisor;

        let (encode_scale, decode_scale) = match profile {
            CompressionProfile::Fast => (30_000u128, 5_000u128),
            CompressionProfile::Balanced => (3_000u128, 1_000u128),
            CompressionProfile::Dense => (500u128, 250u128),
        };

        let size = size_ppm.saturating_mul(w.size as u128);
        let encode = encode_units_per_byte
            .saturating_mul(encode_scale)
            .saturating_mul(w.encode_cpu as u128);
        let decode = decode_units_per_byte
            .saturating_mul(decode_scale)
            .saturating_mul(w.decode_cpu as u128);
        let memory = memory_bytes_per_byte
            .saturating_mul(1_000)
            .saturating_mul(w.memory as u128);

        size
            .saturating_add(encode)
            .saturating_add(decode)
            .saturating_add(memory)
    }
}
