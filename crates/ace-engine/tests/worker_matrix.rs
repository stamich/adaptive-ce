//! Bit-identical output across worker counts and random-access diagnostics.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace_core::{AceConfig, CompressionProfile, DecodeLimits};
use ace_engine::{AceEngine, AceIndexedDecoder};
use std::io::Cursor;

/// Verifies bit-identical output for the full worker-count matrix used by ACE 0.2.1 benchmarks.
#[test]
fn deterministic_thread_matrix() {
    let data = b"ACE deterministic worker matrix / GraphNet / AdaptiveDB\n".repeat(80_000);
    for profile in [
        CompressionProfile::Fast,
        CompressionProfile::Balanced,
        CompressionProfile::Dense,
    ] {
        let mut reference = None;
        for threads in [1usize, 2, 4, 6, 8, 12] {
            let config = AceConfig {
                profile,
                threads,
                ..AceConfig::default()
            };
            let encoded = AceEngine::new(config).unwrap().compress(&data).unwrap();
            if let Some(ref expected) = reference {
                assert_eq!(expected, &encoded);
            } else {
                reference = Some(encoded);
            }
        }
    }
}

/// The Format 1.5 modes (Patched FOR, ALP, small-delta RunDelta) and the DENSE-versus-BALANCED
/// rule produce the same bytes for every worker count.
#[test]
fn deterministic_thread_matrix_for_format_1_5() {
    for workload in [
        ace_corpus::Workload::MonotonicOutliers,
        ace_corpus::Workload::IntCounterReset,
        ace_corpus::Workload::F64DecimalPrice,
        ace_corpus::Workload::IntGeometricDelta,
    ] {
        let data = workload.generate(768 * 1024);
        for profile in [
            CompressionProfile::Fast,
            CompressionProfile::Balanced,
            CompressionProfile::Dense,
        ] {
            let mut reference: Option<Vec<u8>> = None;
            for threads in [1usize, 2, 4, 8] {
                let config = AceConfig {
                    profile,
                    threads,
                    block_size: 64 * 1024,
                    ..AceConfig::default()
                };
                let encoded = AceEngine::new(config).unwrap().compress(&data).unwrap();
                match &reference {
                    Some(expected) => assert_eq!(
                        expected,
                        &encoded,
                        "{} {profile:?} threads {threads}",
                        workload.name()
                    ),
                    None => reference = Some(encoded),
                }
            }
        }
    }
}

/// Verifies that random-access diagnostics report touched blocks and non-zero physical work.
#[test]
fn random_access_metrics_are_consistent() {
    let data = (0u8..=255).cycle().take(2_000_000).collect::<Vec<_>>();
    let encoded = AceEngine::default_engine().compress(&data).unwrap();
    let indexed = AceIndexedDecoder::open(Cursor::new(encoded), DecodeLimits::default()).unwrap();
    let metrics = indexed.range_metrics(250_000..400_000).unwrap();
    assert_eq!(metrics.logical_bytes_requested, 150_000);
    assert!(metrics.blocks_touched >= 2);
    assert_eq!(metrics.blocks_touched, metrics.blocks_decoded);
    assert!(metrics.physical_bytes_read > 0);
}
