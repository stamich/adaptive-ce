//! Tests of `ace_planner` `policy` (formerly the inline unit tests of `src`).

use ace_core::{CodecId, CompressionProfile, EntropyCodecId, PhysicalCompressionPlan};
use ace_planner::*;

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
