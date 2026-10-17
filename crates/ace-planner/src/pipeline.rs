//! Execution of a plan's reversible encode pipeline (transforms → primary codec → entropy).

use ace_codecs::encode_codec;
use ace_core::{AceResult, PhysicalCompressionPlan};
use ace_entropy::encode_entropy;
use ace_transforms::apply_transform;

/// Executes only the reversible payload pipeline of one plan and returns entropy metadata, payload and primary-stream size.
///
/// # Errors
///
/// Propagates transform, codec and entropy encoding errors.
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
