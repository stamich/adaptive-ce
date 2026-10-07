//! Bounded streaming round-trips and equals in-memory compression.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace_core::{AceConfig, DecodeLimits};
use ace_engine::AceEngine;
use ace_stream::{compress_reader_known_size, decompress_stream, StreamLimits};
use std::io::Cursor;

/// Verifies bounded streaming produces a regular indexed ACE stream with exact round-trip.
#[test]
fn streaming_round_trip() {
    let data = b"adaptive-compression-engine\n".repeat(50_000);
    let mut encoded = Vec::new();
    let stats = compress_reader_known_size(
        Cursor::new(&data),
        Cursor::new(&mut encoded),
        data.len() as u64,
        AceConfig::default(),
        StreamLimits::default(),
    )
    .unwrap();
    assert_eq!(stats.input_bytes, data.len() as u64);
    assert!(stats.peak_source_buffer_bytes <= AceConfig::default().block_size);
    assert_eq!(
        AceEngine::default_engine().decompress(&encoded).unwrap(),
        data
    );
    let mut restored = Vec::new();
    decompress_stream(
        Cursor::new(&encoded),
        &mut restored,
        DecodeLimits::default(),
    )
    .unwrap();
    assert_eq!(restored, data);
}

/// Streaming and in-memory compression share one container writer and one planner, so their
/// outputs must be byte-identical (0.4.5-buildfix2 DRY refactor guard).
#[test]
fn streaming_output_equals_in_memory_output() {
    let mut data = Vec::new();
    for i in 0..300_000u32 {
        data.extend_from_slice(&(1_000 + 3 * i + i % 5).to_le_bytes());
    }
    data.extend(b"tail".repeat(10_000));
    let mut streamed = Vec::new();
    compress_reader_known_size(
        Cursor::new(&data),
        Cursor::new(&mut streamed),
        data.len() as u64,
        AceConfig::default(),
        StreamLimits::default(),
    )
    .unwrap();
    assert_eq!(
        streamed,
        AceEngine::default_engine().compress(&data).unwrap()
    );
}

/// A TS1 block late in the stream makes the encoder rewrite the header as Format 1.4; the
/// result still equals in-memory compression, and a stream without TS1 stays Format 1.3.
#[test]
fn streaming_rewrites_header_for_late_ts1_block() {
    let mut data = b"adaptive-compression-engine\n".repeat(40_000);
    data.extend((0..65_536).flat_map(|i| (20.5 + f64::from(i / 1_000)).to_le_bytes()));
    let mut streamed = Vec::new();
    let stats = compress_reader_known_size(
        Cursor::new(&data),
        Cursor::new(&mut streamed),
        data.len() as u64,
        AceConfig::default(),
        StreamLimits::default(),
    )
    .unwrap();
    assert!(stats.format_1_4);
    assert_eq!(streamed[5], 4);
    let engine = AceEngine::default_engine();
    assert_eq!(streamed, engine.compress(&data).unwrap());
    assert_eq!(engine.decompress(&streamed).unwrap(), data);

    let text = b"adaptive-compression-engine\n".repeat(40_000);
    let mut plain = Vec::new();
    let stats = compress_reader_known_size(
        Cursor::new(&text),
        Cursor::new(&mut plain),
        text.len() as u64,
        AceConfig::default(),
        StreamLimits::default(),
    )
    .unwrap();
    assert!(!stats.format_1_4);
    assert_eq!(plain[5], 3);
}

/// A reader that ends early or yields extra bytes is rejected.
#[test]
fn streaming_rejects_size_mismatch() {
    let data = vec![7u8; 10_000];
    let limits = StreamLimits::default();
    assert!(compress_reader_known_size(
        Cursor::new(&data),
        Cursor::new(Vec::new()),
        10_001,
        AceConfig::default(),
        limits
    )
    .is_err());
    assert!(compress_reader_known_size(
        Cursor::new(&data),
        Cursor::new(Vec::new()),
        9_999,
        AceConfig::default(),
        limits
    )
    .is_err());
}
