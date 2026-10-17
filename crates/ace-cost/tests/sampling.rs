//! Tests of `ace_cost` `sampling` (formerly the inline unit tests of `src`).

use ace_core::{CodecId, CompressionProfile};
use ace_cost::*;

/// Ensures generic sampling never depends on randomness or process state.
#[test]
fn sample_ranges_are_stable() {
    let p = SamplePolicy {
        sample_bytes: 16,
        sample_count: 3,
        top_k: 2,
        second_stage_bytes: 32,
        second_stage_count: 2,
        second_stage_top_k: 1,
    };
    assert_eq!(
        deterministic_sample_ranges(100, p),
        vec![0..16, 42..58, 84..100]
    );
}

/// Ensures LZ verification gets larger deterministic windows than RAW verification.
#[test]
fn lz_sampling_uses_wider_strata() {
    let p = SamplePolicy::for_profile(CompressionProfile::Balanced);
    let raw = codec_sample_ranges(262_144, p, CodecId::Raw, 1);
    let lz = codec_sample_ranges(262_144, p, CodecId::Lz, 1);
    assert!(lz[0].len() >= 32 * 1024);
    assert!(lz.len() >= raw.len());
    assert_eq!(lz, codec_sample_ranges(262_144, p, CodecId::Lz, 1));
}

/// Verifies DENSE never collapses to a one-candidate search solely on estimator confidence.
#[test]
fn dense_has_quality_floor() {
    assert_eq!(adaptive_top_k(CompressionProfile::Dense, 0.99, 6, 20), 4);
}
