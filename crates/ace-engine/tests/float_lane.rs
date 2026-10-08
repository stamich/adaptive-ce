//! Planner V5 Float lane through the engine: routes, TS1 selection, Format 1.4 version rule,
//! no change on Corpus V3, explain agreeing with compress.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace_core::{AceConfig, CodecId, CompressionProfile};
use ace_corpus::Workload;
use ace_engine::AceEngine;
use ace_planner::PlannerRoute;

/// Bytes per workload (4 blocks of 256 KiB).
const BYTES: usize = 1024 * 1024;
/// Every profile.
const PROFILES: [CompressionProfile; 3] = [
    CompressionProfile::Fast,
    CompressionProfile::Balanced,
    CompressionProfile::Dense,
];

/// Single-threaded engine for `profile`, with or without the Float lane.
fn engine(profile: CompressionProfile, float: bool) -> AceEngine {
    AceEngine::new(AceConfig {
        profile,
        threads: 1,
        enable_float_specialization: float,
        ..AceConfig::default()
    })
    .unwrap()
}

/// Minor format version declared in the file header (byte 5 of the container).
fn minor_version(encoded: &[u8]) -> u8 {
    encoded[5]
}

/// A constant f64 series is encoded by FloatFast with RunDelta, in Format 1.4.
#[test]
fn constant_f64_uses_float_fast() {
    let data = Workload::F64Constant.generate(BYTES);
    for profile in PROFILES {
        let engine = engine(profile, true);
        let (encoded, stats) = engine.compress_with_stats(&data).unwrap();
        assert_eq!(engine.decompress(&encoded).unwrap(), data);
        assert_eq!(stats.float_fast_blocks, stats.block_count, "{profile:?}");
        assert_eq!(stats.ts1_run_delta_blocks, stats.block_count);
        assert_eq!(stats.float_fast_fallbacks, 0);
        assert_eq!(stats.planner_full_trial_encodes, 0);
        assert_eq!(minor_version(&encoded), 4);
        assert!(
            stats.compression_ratio() > 1000.0,
            "{}",
            stats.compression_ratio()
        );
    }
}

/// Disabling the Float lane gives the 0.4.6 pipeline: no TS1, Format 1.3.
#[test]
fn disabled_float_lane_writes_format_1_3() {
    let data = Workload::F64Constant.generate(BYTES);
    let (encoded, stats) = engine(CompressionProfile::Balanced, false)
        .compress_with_stats(&data)
        .unwrap();
    assert_eq!(stats.time_series_blocks, 0);
    assert_eq!(stats.float_route_blocks, 0);
    assert_eq!(stats.time_series_estimates, 0);
    assert_eq!(minor_version(&encoded), 3);
}

/// The Float lane never changes a Corpus V3 byte (the 0.4.x corpus stays Format 1.3).
#[test]
fn corpus_v3_is_untouched() {
    for workload in Workload::CORPUS_V3 {
        let data = workload.generate(BYTES);
        for profile in PROFILES {
            let with = engine(profile, true).compress(&data).unwrap();
            let without = engine(profile, false).compress(&data).unwrap();
            assert!(with == without, "{} {profile:?}", workload.name());
            assert_eq!(minor_version(&with), 3);
        }
    }
}

/// On Corpus V4 the Float lane never makes output larger and FloatFast never falls back.
#[test]
fn corpus_v4_never_regresses() {
    for workload in Workload::CORPUS_V4 {
        let data = workload.generate(BYTES);
        for profile in PROFILES {
            let float = engine(profile, true);
            let (with, stats) = float.compress_with_stats(&data).unwrap();
            let without = engine(profile, false).compress(&data).unwrap();
            assert_eq!(float.decompress(&with).unwrap(), data);
            assert!(
                with.len() <= without.len(),
                "{} {profile:?}: {} > {}",
                workload.name(),
                with.len(),
                without.len()
            );
            assert_eq!(stats.float_fast_fallbacks, 0, "{}", workload.name());
            let expected_minor = if stats.time_series_blocks > 0 { 4 } else { 3 };
            assert_eq!(minor_version(&with), expected_minor);
        }
    }
}

/// `explain` reports the same route and codec per block as `compress` stores.
#[test]
fn explain_matches_compress() {
    for workload in [
        Workload::F64Step,
        Workload::F64Smooth,
        Workload::IntSparseChange,
        Workload::Mixed,
    ] {
        let data = workload.generate(BYTES);
        let engine = engine(CompressionProfile::Balanced, true);
        let explained = engine.explain(&data).unwrap();
        let (_, stats) = engine.compress_with_stats(&data).unwrap();
        let ts1_explained = explained
            .iter()
            .filter(|block| block.selected.decoding.codec == CodecId::TimeSeries)
            .count() as u64;
        assert_eq!(
            ts1_explained,
            stats.time_series_blocks,
            "{}",
            workload.name()
        );
        let float_routes = explained
            .iter()
            .filter(|block| block.route.route.is_float())
            .count() as u64;
        assert_eq!(float_routes, stats.float_route_blocks);
        for block in &explained {
            assert_eq!(
                block.time_series.is_some(),
                block.selected.decoding.codec == CodecId::TimeSeries
            );
        }
    }
    let explained = engine(CompressionProfile::Fast, true)
        .explain(&Workload::F64Constant.generate(BYTES))
        .unwrap();
    assert!(explained.iter().all(
        |block| block.route.route == PlannerRoute::FloatFast && block.telemetry.float_fast_hit
    ));
}
