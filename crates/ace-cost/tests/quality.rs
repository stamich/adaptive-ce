//! Tests of `ace_cost` `quality` (formerly the inline unit tests of `src`).

use ace_core::CompressionProfile;
use ace_cost::*;

/// Verifies profile envelopes become progressively stricter from FAST to DENSE.
#[test]
fn profiles_have_monotonic_quality_slack() {
    let fast = QualityEnvelope::for_profile(CompressionProfile::Fast).slack_ppm;
    let balanced = QualityEnvelope::for_profile(CompressionProfile::Balanced).slack_ppm;
    let dense = QualityEnvelope::for_profile(CompressionProfile::Dense).slack_ppm;
    assert!(fast > balanced);
    assert!(balanced > dense);
}

/// Verifies the quality limit uses inclusive deterministic integer arithmetic.
#[test]
fn inclusive_limit_is_deterministic() {
    let envelope = QualityEnvelope { slack_ppm: 10_000 };
    assert_eq!(envelope.limit_bytes(10_000), 10_100);
    assert!(envelope.qualifies(10_000, 10_100));
    assert!(!envelope.qualifies(10_000, 10_101));
}

/// Builds a small deterministic candidate for envelope filtering tests.
fn candidate(size: u64, score: u128) -> EstimatedCandidate {
    let mut plan = ace_core::PhysicalCompressionPlan::raw();
    plan.cost.predicted_size_bytes = size;
    plan.score = score;
    EstimatedCandidate {
        plan,
        analytical_size_bytes: size,
        sampled_size_bytes: None,
        blended_size_bytes: size,
        cost: ace_core::PlanCost {
            predicted_size_bytes: size,
            metadata_bytes: 0,
            encode_units: 1,
            decode_units: 1,
            memory_bytes: 1,
        },
        score,
        confidence: 1.0,
    }
}

/// Verifies a very cheap CPU candidate cannot enter a strict DENSE pool when its size is too large.
#[test]
fn dense_envelope_rejects_cpu_cheap_but_oversized_candidate() {
    let envelope = QualityEnvelope::for_profile(CompressionProfile::Dense);
    let candidates = vec![
        candidate(100_000, 10_000),
        candidate(100_200, 20_000),
        candidate(101_000, 1),
    ];
    let qualified = envelope.qualify(&candidates);
    assert_eq!(qualified.len(), 2);
    assert!(qualified
        .iter()
        .all(|candidate| candidate.blended_size_bytes <= 100_300));
}

/// Verifies BALANCED admits a modest size trade-off while still excluding a large ratio loss.
#[test]
fn balanced_envelope_allows_small_but_not_large_quality_tradeoff() {
    let envelope = QualityEnvelope::for_profile(CompressionProfile::Balanced);
    let candidates = vec![
        candidate(100_000, 30),
        candidate(101_000, 10),
        candidate(103_000, 1),
    ];
    let qualified = envelope.qualify(&candidates);
    assert_eq!(qualified.len(), 2);
    assert!(qualified
        .iter()
        .any(|candidate| candidate.blended_size_bytes == 101_000));
    assert!(!qualified
        .iter()
        .any(|candidate| candidate.blended_size_bytes == 103_000));
}
