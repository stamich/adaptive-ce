//! Engine-level NUM1 selection, Auto block policy and numeric round-trips.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::io::Cursor;

use ace_core::{AccessHint, AceConfig, BlockSizePolicy, CompressionProfile};
use ace_engine::AceEngine;
use ace_format::AceReader;

/// Builds a deterministic monotonic u32 workload representative of telemetry counters.
fn u32_counter(bytes: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes);
    let mut value = 10_000u32;
    while out.len() < bytes {
        value = value.wrapping_add(3);
        out.extend_from_slice(&value.to_le_bytes());
    }
    out.truncate(bytes);
    out
}

/// Planner V4 must be able to select the numeric codec and preserve exact reconstruction.
#[test]
fn planner_v4_numeric_roundtrip() {
    let input = u32_counter(2 * 1024 * 1024);
    let config = AceConfig {
        profile: CompressionProfile::Balanced,
        threads: 1,
        ..AceConfig::default()
    };
    let engine = AceEngine::new(config).unwrap();
    let (encoded, stats) = engine.compress_with_stats(&input).unwrap();
    assert_eq!(engine.decompress(&encoded).unwrap(), input);
    assert!(
        stats.numeric_blocks > 0,
        "numeric workload should admit/select the Format 1.3 numeric codec"
    );
}

/// Auto block policy uses a larger file-level block for strong numeric sequential input.
#[test]
fn auto_block_policy_selects_large_numeric_block() {
    let input = u32_counter(4 * 1024 * 1024);
    let config = AceConfig {
        block_size_policy: BlockSizePolicy::Auto,
        access_hint: AccessHint::Sequential,
        threads: 1,
        ..AceConfig::default()
    };
    let engine = AceEngine::new(config).unwrap();
    let encoded = engine.compress(&input).unwrap();
    let mut reader = AceReader::new(Cursor::new(encoded), ace_core::DecodeLimits::default());
    let header = reader.read_file_header().unwrap();
    assert!(header.default_block_size >= 512 * 1024);
}

/// Random-access access hint caps automatic block size at 256 KiB.
#[test]
fn auto_block_policy_respects_random_access_cap() {
    let input = u32_counter(4 * 1024 * 1024);
    let config = AceConfig {
        block_size_policy: BlockSizePolicy::Auto,
        access_hint: AccessHint::RandomAccess,
        threads: 1,
        ..AceConfig::default()
    };
    let engine = AceEngine::new(config).unwrap();
    let encoded = engine.compress(&input).unwrap();
    let mut reader = AceReader::new(Cursor::new(encoded), ace_core::DecodeLimits::default());
    let header = reader.read_file_header().unwrap();
    assert_eq!(header.default_block_size, 256 * 1024);
}
