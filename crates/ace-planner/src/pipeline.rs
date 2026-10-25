//! Execution of a plan's reversible encode pipeline (transforms → primary codec → entropy).

use std::borrow::Cow;

use ace_codecs::encode_codec;
use ace_core::{AceResult, EntropyCodecId, PhysicalCompressionPlan};
use ace_entropy::encode_entropy;
use ace_transforms::apply_transform;

/// Executes only the reversible payload pipeline of one plan and returns entropy metadata, payload and primary-stream size.
///
/// The input is borrowed until a transform produces a new buffer, and without an entropy stage
/// the primary codec's output becomes the payload as it is, so a plan with neither copies
/// nothing besides what its codec writes.
///
/// # Errors
///
/// Propagates transform, codec and entropy encoding errors.
pub fn encode_plan_payload(
    input: &[u8],
    plan: &PhysicalCompressionPlan,
) -> AceResult<(Vec<u8>, Vec<u8>, usize)> {
    let mut stage: Cow<'_, [u8]> = Cow::Borrowed(input);
    for &transform in &plan.decoding.transforms {
        stage = Cow::Owned(apply_transform(transform, &stage)?);
    }
    let primary = encode_codec(plan.decoding.codec, plan.lz_mode, &stage)?;
    let primary_len = primary.len();
    if plan.decoding.entropy == EntropyCodecId::None {
        return Ok((Vec::new(), primary, primary_len));
    }
    let (metadata, payload) = encode_entropy(plan.decoding.entropy, &primary)?;
    Ok((metadata, payload, primary_len))
}
