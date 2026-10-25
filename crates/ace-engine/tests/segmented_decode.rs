//! Decoding of segmented blocks (Format 1.6) in every reader, the reader gate for older
//! formats and the behaviour on damaged input. Encoding is covered by `segmented_encode.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::io::Cursor;

use ace_core::{AceConfig, AceError, DecodeLimits, SegmentationConfig};
use ace_engine::{AceEngine, AceIndexedDecoder};

/// 4 KiB windows, 16 KiB .. 128 KiB segments.
fn config() -> AceConfig {
    AceConfig {
        block_size: 1 << 20,
        threads: 1,
        segmentation: Some(SegmentationConfig {
            window_size: 4096,
            min_segment_size: 16 * 1024,
            max_segment_size: 128 * 1024,
            max_segments: 64,
        }),
        ..AceConfig::default()
    }
}

/// Deterministic pseudo-random bytes (xorshift).
fn noise(len: usize, mut seed: u64) -> Vec<u8> {
    (0..len)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 24) as u8
        })
        .collect()
}

/// Text, then counters, then noise: the encoder segments this block.
fn mixed() -> Vec<u8> {
    let words = ["alpha ", "beta ", "gamma ", "delta ", "log ", "entry ", "value=", "ok\n"];
    let mut input = Vec::new();
    let mut i = 0usize;
    while input.len() < 192 * 1024 {
        input.extend_from_slice(words[(i * 7 + i / 3) % words.len()].as_bytes());
        i += 1;
    }
    input.extend((0u32..48 * 1024).flat_map(u32::to_le_bytes));
    input.extend(noise(128 * 1024, 4));
    input
}

/// A segmented file of [`mixed`] and the input it encodes.
fn segmented_file() -> (Vec<u8>, Vec<u8>) {
    let input = mixed();
    let (file, stats) = AceEngine::new(config()).unwrap().compress_with_stats(&input).unwrap();
    assert_eq!(stats.segmented_blocks, 1, "the fixture must be segmented");
    (file, input)
}

/// `bytes` with the header's minor version patched and its CRC recomputed.
fn with_minor(bytes: &[u8], minor: u8) -> Vec<u8> {
    let mut patched = bytes.to_vec();
    patched[5] = minor;
    let crc = ace_format::checksum(&patched[..28]);
    patched[28..32].copy_from_slice(&crc.to_le_bytes());
    patched
}

#[test]
fn every_reader_returns_the_original_bytes() {
    let (file, input) = segmented_file();
    let engine = AceEngine::default_engine();
    assert_eq!(engine.decompress(&file).unwrap(), input);

    let mut into = Vec::new();
    assert_eq!(engine.decompress_into(&file, &mut into).unwrap(), input.len());
    assert_eq!(into, input);

    let mut streamed = Vec::new();
    engine.decompress_from(Cursor::new(&file), &mut streamed).unwrap();
    assert_eq!(streamed, input);

    let mut indexed = AceIndexedDecoder::open(Cursor::new(&file), DecodeLimits::default()).unwrap();
    assert_eq!(indexed.read_range(0..input.len() as u64).unwrap(), input);
    // Ranges across the segment boundaries (text | counters | noise) and inside one segment.
    for range in [(0u64, 10u64), (190 * 1024, 200 * 1024), (100_000, 500_000), (input.len() as u64 - 5, input.len() as u64)] {
        let got = indexed.read_range(range.0..range.1).unwrap();
        assert_eq!(got, &input[range.0 as usize..range.1 as usize], "{range:?}");
    }
}

#[test]
fn several_blocks_mix_segmented_and_plain_blocks() {
    // A plain block (a single character) between two segmented ones.
    let mut input = mixed();
    input.extend(vec![0u8; 1 << 20]);
    input.extend(mixed());
    let engine = AceEngine::new(config()).unwrap();
    let (file, stats) = engine.compress_with_stats(&input).unwrap();
    assert!(stats.segmented_blocks >= 2);
    assert!(stats.segmented_blocks < 4, "the constant block must stay unsegmented");
    assert_eq!(engine.decompress(&file).unwrap(), input);
}

#[test]
fn files_older_than_1_6_reject_a_segmented_block() {
    let (file, _) = segmented_file();
    for minor in 0..=5u8 {
        let old = with_minor(&file, minor);
        let engine = AceEngine::default_engine();
        assert!(matches!(engine.decompress(&old), Err(AceError::UnsupportedVersion { .. })), "{minor}");
        let mut out = Vec::new();
        assert!(matches!(engine.decompress_from(Cursor::new(&old), &mut out), Err(AceError::UnsupportedVersion { .. })));
        assert!(AceIndexedDecoder::open(Cursor::new(&old), DecodeLimits::default())
            .and_then(|mut decoder| decoder.read_range(0..100))
            .is_err());
    }
}

/// A flipped byte anywhere in the segmented block either fails or still yields the original
/// bytes (a flip in an unused bit); it never produces different output and never panics.
#[test]
fn damage_is_detected_at_every_position() {
    let (file, input) = segmented_file();
    let engine = AceEngine::default_engine();
    // The header, the table and the first inner headers byte by byte, then a stride.
    let positions = (0..600).chain((600..file.len()).step_by(997));
    for position in positions {
        let mut damaged = file.clone();
        damaged[position] ^= 0x5A;
        if let Ok(output) = engine.decompress(&damaged) {
            assert_eq!(output, input, "flip at {position} changed the output silently");
        }
    }
    for len in (0..file.len()).step_by(1013) {
        assert!(engine.decompress(&file[..len]).is_err(), "prefix {len}");
    }
}

#[test]
fn the_segment_limit_is_enforced_by_the_reader() {
    let (file, _) = segmented_file();
    let tight = AceEngine::default_engine()
        .with_decode_limits(DecodeLimits { max_segments: 2, ..DecodeLimits::default() });
    assert!(matches!(
        tight.decompress(&file),
        Err(AceError::ResourceLimitExceeded("segment count"))
    ));
}
