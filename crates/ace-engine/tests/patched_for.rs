//! Patched FOR (NUM1 mode 4) through the engine: the planner selects it for blocks with rare
//! outliers, the file is declared Format 1.5, and every reader returns the input.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::io::Cursor;

use ace_codecs::{numeric_inspect, NumericMode};
use ace_core::{AceConfig, CompressionProfile, DecodeLimits};
use ace_corpus::Workload;
use ace_engine::{AceEngine, AceIndexedDecoder};

const BYTES: usize = 1 << 20;

fn engine(profile: CompressionProfile) -> AceEngine {
    AceEngine::new(AceConfig {
        profile,
        ..AceConfig::default()
    })
    .unwrap()
}

/// The outlier workloads shrink by a large factor and are declared Format 1.5.
#[test]
fn outlier_workloads_use_patched_for() {
    // (workload, smallest ratio expected). 0.6.0 stored these at 1.8x, 19x, 6.4x and 2.7x.
    for (workload, minimum_ratio) in [
        (Workload::MonotonicOutliers, 300.0),
        (Workload::GaugeSawtooth, 500.0),
        (Workload::DeltaVariable, 9.0),
        (Workload::IntCounterReset, 20.0),
    ] {
        let input = workload.generate(BYTES);
        for profile in [CompressionProfile::Balanced, CompressionProfile::Dense] {
            let engine = engine(profile);
            let packed = engine.compress(&input).unwrap();
            assert_eq!(packed[5], 5, "{} {profile:?}", workload.name());
            let ratio = input.len() as f64 / packed.len() as f64;
            assert!(
                ratio >= minimum_ratio,
                "{} {profile:?}: ratio {ratio:.1}",
                workload.name()
            );
            assert_eq!(engine.decompress(&packed).unwrap(), input);
        }
    }
}

/// Random access works on a patched file, and every block is a mode 4 payload.
#[test]
fn patched_blocks_are_randomly_accessible() {
    let input = Workload::MonotonicOutliers.generate(BYTES);
    let packed = engine(CompressionProfile::Balanced)
        .compress(&input)
        .unwrap();
    let mut reader =
        AceIndexedDecoder::open(Cursor::new(&packed), DecodeLimits::default()).unwrap();
    assert_eq!(
        reader.read_range(300_000..300_100).unwrap(),
        &input[300_000..300_100]
    );
    assert_eq!(reader.read_range(0..BYTES as u64).unwrap(), input);

    // Block 0 starts after the 32-byte file header and the 32-byte block header; its payload
    // length is the "encoded size" field (offset 12) of that header.
    let encoded = u32::from_le_bytes(packed[44..48].try_into().unwrap()) as usize;
    let info = numeric_inspect(&packed[64..64 + encoded]).unwrap();
    assert_eq!(info.mode, NumericMode::Patched);
}

/// Blocks without outliers are not touched: the file stays Format 1.3 and keeps its bytes.
#[test]
fn clean_numeric_data_stays_format_1_3() {
    for workload in [Workload::U64FixedStep, Workload::U32Counter] {
        let input = workload.generate(BYTES);
        let packed = engine(CompressionProfile::Balanced)
            .compress(&input)
            .unwrap();
        assert_eq!(packed[5], 3, "{}", workload.name());
    }
}

/// A dominant exact Numeric estimate keeps the single verification slot in FAST: before the
/// planner ordered it first, a generic candidate with a guessed size could take it and the
/// block was stored at a fraction of the achievable ratio.
#[test]
fn dominant_numeric_wins_the_fast_profile() {
    let input = Workload::IntCounterReset.generate(BYTES);
    let packed = engine(CompressionProfile::Fast).compress(&input).unwrap();
    assert_eq!(packed[5], 5);
    assert!(packed.len() * 20 < input.len(), "{} bytes", packed.len());
}
