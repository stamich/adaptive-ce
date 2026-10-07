//! End-to-end round-trips and repeat determinism on heterogeneous data.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace_core::{AceConfig, DecodeLimits};
use ace_engine::{AceEngine, AceIndexedDecoder};
use std::io::Cursor;

/// Ensures heterogeneous input survives a complete ACE 0.3 encode/decode cycle.
#[test]
fn heterogeneous_roundtrip() {
    let mut data = vec![0u8; 200_000];
    data.extend((0u8..=255).cycle().take(300_000));
    for _ in 0..5000 {
        data.extend_from_slice(b"graphnet/ace/adaptive-db repeated record\n");
    }
    let engine = AceEngine::default_engine();
    let encoded = engine.compress(&data).unwrap();
    let restored = engine.decompress(&encoded).unwrap();
    assert_eq!(restored, data);
}

/// Ensures repeated compression is bit-for-bit deterministic.
#[test]
fn deterministic_output() {
    let data = b"deterministic deterministic deterministic".repeat(5000);
    let engine = AceEngine::default_engine();
    assert_eq!(
        engine.compress(&data).unwrap(),
        engine.compress(&data).unwrap()
    );
}

/// Ensures worker count does not change the serialized physical representation.
#[test]
fn parallel_output_matches_single_thread() {
    let data = b"parallel deterministic ACE block\n".repeat(100_000);
    let one = AceConfig {
        threads: 1,
        ..AceConfig::default()
    };
    let mut many = one.clone();
    many.threads = 8;
    assert_eq!(
        AceEngine::new(one).unwrap().compress(&data).unwrap(),
        AceEngine::new(many).unwrap().compress(&data).unwrap()
    );
}

/// Ensures indexed block and range reads reconstruct the same logical bytes as the source.
#[test]
fn indexed_random_access_roundtrip() {
    let data = (0u8..=255).cycle().take(2_000_000).collect::<Vec<_>>();
    let encoded = AceEngine::default_engine().compress(&data).unwrap();
    let mut indexed =
        AceIndexedDecoder::open(Cursor::new(encoded), DecodeLimits::default()).unwrap();
    let block = indexed.decode_block(3).unwrap();
    assert_eq!(block, &data[3 * 262_144..4 * 262_144]);
    let range = indexed.read_range(700_000..900_000).unwrap();
    assert_eq!(range, &data[700_000..900_000]);
}
