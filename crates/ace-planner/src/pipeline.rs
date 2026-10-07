//! Execution of a plan's reversible encode pipeline (transforms → primary codec → entropy).

use ace_codecs::encode_codec;
use ace_core::{AceResult, PhysicalCompressionPlan};
use ace_entropy::encode_entropy;
use ace_transforms::apply_transform;

/// Executes only the reversible payload pipeline of one plan and returns entropy metadata, payload and primary-stream size.
pub fn encode_plan_payload(
    input: &[u8],
    plan: &PhysicalCompressionPlan,
) -> AceResult<(Vec<u8>, Vec<u8>, usize)> {
    let mut stage = input.to_vec();
    for &transform in &plan.decoding.transforms {
        stage = apply_transform(transform, &stage)?;
    }
    stage = encode_codec(plan.decoding.codec, plan.lz_mode, &stage)?;
    let primary_len = stage.len();
    let (metadata, payload) = encode_entropy(plan.decoding.entropy, &stage)?;
    Ok((metadata, payload, primary_len))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ace_core::{CodecId, EntropyCodecId, TransformId};

    /// The pipeline output decodes back to the input through the inverse stages.
    #[test]
    fn pipeline_roundtrip() {
        let input: Vec<u8> = (0..10_000u32).map(|i| (i / 7) as u8).collect();
        let mut plan = PhysicalCompressionPlan::raw();
        plan.decoding.transforms = vec![TransformId::DeltaByte];
        plan.decoding.codec = CodecId::Rle;
        plan.decoding.entropy = EntropyCodecId::Huffman;
        let (metadata, payload, primary_len) = encode_plan_payload(&input, &plan).unwrap();
        let primary =
            ace_entropy::decode_entropy(EntropyCodecId::Huffman, &metadata, &payload, primary_len)
                .unwrap();
        let transformed = ace_codecs::decode_codec(CodecId::Rle, &primary, input.len()).unwrap();
        assert_eq!(
            ace_transforms::invert_transform(TransformId::DeltaByte, &transformed).unwrap(),
            input
        );
    }
}
