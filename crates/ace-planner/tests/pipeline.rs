//! Tests of `ace_planner` `pipeline` (formerly the inline unit tests of `src`).

use ace_core::{CodecId, EntropyCodecId, PhysicalCompressionPlan, TransformId};
use ace_planner::*;

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
