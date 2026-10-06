use ace_core::{
    AceConfig, BlockProfile, CandidateTier, CodecId, DecodingPlan, EntropyCodecId,
    PhysicalCompressionPlan, PlanCost,
};
use ace_planner::{
    CandidatePreference, DominancePolicy, PlannerRoute, PolicyOracle, PolicyOracleCandidate,
};

/// Builds a minimal plan used by policy-oracle integration tests.
fn plan(codec: CodecId, entropy: EntropyCodecId) -> PhysicalCompressionPlan {
    PhysicalCompressionPlan {
        decoding: DecodingPlan {
            transforms: Vec::new(),
            codec,
            dictionary: None,
            entropy,
        },
        lz_mode: None,
        tier: CandidateTier::Likely,
        cost: PlanCost::default(),
        score: 0,
        reason: "buildfix4 policy-oracle test",
    }
}

/// Builds a zero-heavy profile matching the production dominance use case.
fn zero_profile() -> BlockProfile {
    BlockProfile {
        size: 256 * 1024,
        entropy_h0: 0.0,
        entropy_h1: 0.0,
        zero_ratio: 1.0,
        run_score: 1.0,
        delta_score: 1.0,
        repetition_score: 1.0,
        sampled_match_length: 128.0,
        unique_byte_count: 1,
        incompressibility_score: 0.0,
    }
}

/// Policy oracle must prefer RLE over a slightly smaller RAW route candidate for zeros.
#[test]
fn zero_policy_oracle_prefers_rle_within_envelope() {
    let config = AceConfig::default();
    let profile = zero_profile();
    let raw = plan(CodecId::Raw, EntropyCodecId::Rans);
    let rle = plan(CodecId::Rle, EntropyCodecId::Huffman);
    let candidates = [
        PolicyOracleCandidate {
            plan: &raw,
            encoded_bytes: 520,
        },
        PolicyOracleCandidate {
            plan: &rle,
            encoded_bytes: 772,
        },
    ];

    let decision = PolicyOracle::choose(PlannerRoute::Generic, &candidates, &profile, &config)
        .expect("policy oracle decision");

    assert!(matches!(
        decision.candidate.plan.decoding.codec,
        CodecId::Rle
    ));
    assert_eq!(decision.preference, CandidatePreference::Preferred);
    assert_eq!(decision.accepted_size_loss_bytes, 252);
}

/// Dominance policy still exposes the preferred class directly to explain/diagnostic callers.
#[test]
fn zero_profile_marks_rle_preferred() {
    let config = AceConfig::default();
    let profile = zero_profile();
    let rle = plan(CodecId::Rle, EntropyCodecId::None);
    let (preference, _) =
        DominancePolicy::preference(PlannerRoute::Generic, &rle, &profile, &config);
    assert_eq!(preference, CandidatePreference::Preferred);
}
