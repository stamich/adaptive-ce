//! Tests of `ace_planner` `evaluator` (formerly the inline unit tests of `src`).

use ace_core::{CodecId, CompressionProfile};
use ace_planner::*;

/// Ensures high-confidence LZ verification trusts full-block statistics more than samples.
#[test]
fn high_confidence_lz_prefers_analytical_weight() {
    let (sample, analytical) =
        verification_blend_weights(CodecId::Lz, 0.95, 2, CompressionProfile::Dense);
    assert!(analytical > sample);
    assert_eq!(sample + analytical, 100);
}

/// Ensures low-confidence non-LZ verification still keeps analytical evidence in the blend.
#[test]
fn low_confidence_sampling_never_fully_replaces_estimate() {
    let (sample, analytical) =
        verification_blend_weights(CodecId::Raw, 0.40, 2, CompressionProfile::Balanced);
    assert!(sample < 100);
    assert!(analytical > 0);
    assert_eq!(sample + analytical, 100);
}

/// Verifies all supported profile, codec, confidence, and stage combinations produce valid percentages.
#[test]
fn verification_blend_weights_are_valid_percentages() {
    for profile in [
        CompressionProfile::Fast,
        CompressionProfile::Balanced,
        CompressionProfile::Dense,
    ] {
        for codec in [CodecId::Raw, CodecId::Rle, CodecId::Lz] {
            for confidence in [0.40_f32, 0.80_f32, 0.95_f32] {
                for stage in [1_u8, 2_u8] {
                    let (sample, analytical) =
                        verification_blend_weights(codec, confidence, stage, profile);
                    assert!(sample <= 100);
                    assert!(analytical <= 100);
                    assert_eq!(sample + analytical, 100);
                }
            }
        }
    }
}
