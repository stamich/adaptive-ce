use ace_core::{
    CandidateTier, CodecId, CompressionProfile, DecodingPlan, EntropyCodecId, PhysicalCompressionPlan,
};
use ace_planner::DeterministicCostModel;

/// Creates one synthetic RAW+entropy plan for cost-model ordering tests.
fn synthetic_plan(entropy: EntropyCodecId) -> PhysicalCompressionPlan {
    PhysicalCompressionPlan {
        decoding: DecodingPlan {
            transforms: Vec::new(),
            codec: CodecId::Raw,
            dictionary: None,
            entropy,
        },
        lz_mode: None,
        tier: CandidateTier::Likely,
        cost: Default::default(),
        score: 0,
        reason: "synthetic cost test",
    }
}

/// Verifies that FAST can prefer a cheaper CPU plan even when the encoded output is larger.
#[test]
fn fast_cpu_cost_can_outweigh_size_gain() {
    let model = DeterministicCostModel;
    let input = 262_144usize;
    let raw = synthetic_plan(EntropyCodecId::None);
    let dense = synthetic_plan(EntropyCodecId::Rans);
    let raw_cost = model.cost(&raw, input, input, 0);
    let dense_cost = model.cost(&dense, input, input / 3, 512);
    assert!(
        model.score(CompressionProfile::Fast, raw_cost, input)
            < model.score(CompressionProfile::Fast, dense_cost, input)
    );
}

/// Verifies that DENSE still values a substantial size reduction above encoder work.
#[test]
fn dense_prefers_large_size_gain() {
    let model = DeterministicCostModel;
    let input = 262_144usize;
    let raw = synthetic_plan(EntropyCodecId::None);
    let compact = synthetic_plan(EntropyCodecId::Rans);
    let raw_cost = model.cost(&raw, input, input, 0);
    let compact_cost = model.cost(&compact, input, input / 3, 512);
    assert!(
        model.score(CompressionProfile::Dense, compact_cost, input)
            < model.score(CompressionProfile::Dense, raw_cost, input)
    );
}
