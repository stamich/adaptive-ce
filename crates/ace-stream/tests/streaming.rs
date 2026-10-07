use std::io::Cursor;
use ace_core::{AceConfig, DecodeLimits};
use ace_engine::AceEngine;
use ace_stream::{compress_reader_known_size, decompress_stream, StreamLimits};

/// Verifies bounded streaming produces a regular indexed ACE 1.2 stream with exact round-trip.
#[test]
fn streaming_round_trip() {
    let data = b"adaptive-compression-engine\n".repeat(50_000);
    let mut encoded = Vec::new();
    let stats = compress_reader_known_size(
        Cursor::new(&data), &mut encoded, data.len() as u64, AceConfig::default(), StreamLimits::default()
    ).unwrap();
    assert_eq!(stats.input_bytes, data.len() as u64);
    assert!(stats.peak_source_buffer_bytes <= AceConfig::default().block_size);
    assert_eq!(AceEngine::default_engine().decompress(&encoded).unwrap(), data);
    let mut restored = Vec::new();
    decompress_stream(Cursor::new(&encoded), &mut restored, DecodeLimits::default()).unwrap();
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
    compress_reader_known_size(Cursor::new(&data), &mut streamed, data.len() as u64, AceConfig::default(), StreamLimits::default()).unwrap();
    assert_eq!(streamed, AceEngine::default_engine().compress(&data).unwrap());
}

/// A reader that ends early or yields extra bytes is rejected.
#[test]
fn streaming_rejects_size_mismatch() {
    let data = vec![7u8; 10_000];
    let limits = StreamLimits::default();
    assert!(compress_reader_known_size(Cursor::new(&data), Vec::new(), 10_001, AceConfig::default(), limits).is_err());
    assert!(compress_reader_known_size(Cursor::new(&data), Vec::new(), 9_999, AceConfig::default(), limits).is_err());
}
