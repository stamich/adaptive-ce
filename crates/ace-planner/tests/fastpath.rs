//! Tests of `ace_planner` `fastpath` (formerly the inline unit tests of `src`).

use ace_core::{AceConfig, BlockProfile, CodecId, CompressionProfile, EntropyCodecId};
use ace_planner::*;

/// Builds an extreme zero-heavy profile that previously bypassed entropy alternatives.
fn zero_heavy_profile() -> BlockProfile {
    BlockProfile {
        size: 262_144,
        entropy_h0: 0.0,
        entropy_h1: 0.0,
        zero_ratio: 1.0,
        run_score: 1.0,
        delta_score: 0.0,
        repetition_score: 1.0,
        sampled_match_length: 64.0,
        unique_byte_count: 1,
        incompressibility_score: 0.0,
    }
}

/// BALANCED must compare entropy alternatives instead of forcing bare RLE.
#[test]
fn balanced_zero_heavy_uses_general_planner() {
    let config = AceConfig {
        profile: CompressionProfile::Balanced,
        ..AceConfig::default()
    };
    assert!(DefaultPlannerFastPath
        .try_plan(&zero_heavy_profile(), &config)
        .is_none());
}

/// DENSE must compare entropy alternatives instead of forcing bare RLE.
#[test]
fn dense_zero_heavy_uses_general_planner() {
    let config = AceConfig {
        profile: CompressionProfile::Dense,
        ..AceConfig::default()
    };
    assert!(DefaultPlannerFastPath
        .try_plan(&zero_heavy_profile(), &config)
        .is_none());
}

/// FAST preserves the speed-first RLE shortcut.
#[test]
fn fast_zero_heavy_keeps_rle_shortcut() {
    let config = AceConfig {
        profile: CompressionProfile::Fast,
        ..AceConfig::default()
    };
    let plan = DefaultPlannerFastPath
        .try_plan(&zero_heavy_profile(), &config)
        .expect("FAST zero-heavy profile should use fast path");
    assert_eq!(plan.decoding.codec, CodecId::Rle);
    assert_eq!(plan.decoding.entropy, EntropyCodecId::None);
}
