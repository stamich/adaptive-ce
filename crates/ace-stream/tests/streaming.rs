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
