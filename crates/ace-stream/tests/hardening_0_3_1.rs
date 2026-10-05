use std::io::Cursor;

use ace_core::AceConfig;
use ace_stream::{compress_reader_known_size, StreamLimits};

/// Declared input size larger than the configured stream limit must be rejected before reading.
#[test]
fn input_limit_is_enforced_before_streaming() {
    let data = vec![1u8; 32];
    let limits = StreamLimits {
        max_input_bytes: 16,
        ..StreamLimits::default()
    };
    let mut encoded = Vec::new();
    assert!(compress_reader_known_size(
        Cursor::new(data),
        &mut encoded,
        32,
        AceConfig::default(),
        limits,
    )
    .is_err());
}

/// Premature EOF relative to the declared deterministic source size must be rejected.
#[test]
fn short_reader_is_rejected() {
    let data = vec![7u8; 32];
    let mut encoded = Vec::new();
    assert!(compress_reader_known_size(
        Cursor::new(data),
        &mut encoded,
        64,
        AceConfig::default(),
        StreamLimits::default(),
    )
    .is_err());
}

/// Extra source bytes beyond the declared deterministic source size must be rejected.
#[test]
fn reader_longer_than_declared_size_is_rejected() {
    let data = vec![7u8; 64];
    let mut encoded = Vec::new();
    assert!(compress_reader_known_size(
        Cursor::new(data),
        &mut encoded,
        32,
        AceConfig::default(),
        StreamLimits::default(),
    )
    .is_err());
}
