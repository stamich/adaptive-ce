use ace_core::{
    AceConfig, BlockProfile, CandidateTier, CompressionProfile, EntropyCodecId, LzMode,
};
use ace_planner::{CompressionPlanner, DefaultCompressionPlanner, EntropySelectionPolicy};

/// Builds one representative structured profile for deterministic planner unit tests.
fn structured_profile() -> BlockProfile {
    BlockProfile {
        size: 262_144,
        entropy_h0: 5.2,
        entropy_h1: 2.1,
        zero_ratio: 0.04,
        run_score: 0.03,
        delta_score: 0.12,
        repetition_score: 0.22,
        sampled_match_length: 12.0,
        unique_byte_count: 96,
        incompressibility_score: 0.28,
    }
}

/// Ensures BALANCED retains mandatory entropy baselines used by oracle diagnostics.
#[test]
fn balanced_mandatory_entropy_baselines_are_present() {
    let profile = structured_profile();
    let cfg = AceConfig::default();
    let candidates = DefaultCompressionPlanner.candidates(&profile, &cfg);
    assert!(candidates.iter().any(|p| {
        p.tier == CandidateTier::Mandatory && p.decoding.entropy == EntropyCodecId::Huffman
    }));
    assert!(candidates.iter().any(|p| {
        p.tier == CandidateTier::Mandatory && p.decoding.entropy == EntropyCodecId::Rans
    }));
}

/// Ensures FAST never evaluates the expensive balanced LZ matcher.
#[test]
fn fast_excludes_balanced_lz() {
    let profile = structured_profile();
    let mut cfg = AceConfig::default();
    cfg.profile = CompressionProfile::Fast;
    let candidates = DefaultCompressionPlanner.candidates(&profile, &cfg);
    assert!(candidates.iter().all(|p| p.lz_mode != Some(LzMode::Balanced)));
}

/// Ensures FAST does not treat scalar rANS as a mandatory baseline.
#[test]
fn fast_does_not_have_mandatory_rans() {
    let profile = structured_profile();
    let mut cfg = AceConfig::default();
    cfg.profile = CompressionProfile::Fast;
    let candidates = DefaultCompressionPlanner.candidates(&profile, &cfg);
    assert!(candidates.iter().all(|p| {
        !(p.tier == CandidateTier::Mandatory && p.decoding.entropy == EntropyCodecId::Rans)
    }));
}

/// Ensures DENSE admits at least as many candidates as FAST for the same evidence.
#[test]
fn dense_search_space_is_not_narrower_than_fast() {
    let profile = structured_profile();
    let mut fast = AceConfig::default();
    fast.profile = CompressionProfile::Fast;
    let mut dense = fast.clone();
    dense.profile = CompressionProfile::Dense;
    assert!(
        DefaultCompressionPlanner.candidates(&profile, &dense).len()
            >= DefaultCompressionPlanner.candidates(&profile, &fast).len()
    );
}

/// Ensures BALANCED covers deep LZ and Delta+LZ oracle families.
#[test]
fn balanced_covers_deeper_lz_oracle_families() {
    let profile = structured_profile();
    let cfg = AceConfig::default();
    let candidates = DefaultCompressionPlanner.candidates(&profile, &cfg);
    assert!(candidates.iter().any(|p| p.lz_mode == Some(LzMode::Balanced)));
    assert!(
        candidates
            .iter()
            .any(|p| !p.decoding.transforms.is_empty() && p.lz_mode.is_some())
    );
}

/// Ensures rANS eligibility remains profile-specific and deterministic.
#[test]
fn rans_policy_is_profile_specific() {
    let fast = EntropySelectionPolicy::for_profile(CompressionProfile::Fast);
    let balanced = EntropySelectionPolicy::for_profile(CompressionProfile::Balanced);
    let dense = EntropySelectionPolicy::for_profile(CompressionProfile::Dense);
    assert!(fast.min_rans_input_size > balanced.min_rans_input_size);
    assert!(balanced.min_rans_input_size > dense.min_rans_input_size);
    assert!(fast.min_rans_gain_fraction > balanced.min_rans_gain_fraction);
}
