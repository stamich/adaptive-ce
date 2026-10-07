//! ACE 0.4.5 numeric-coverage regression tests (engine level).
//!
//! Each test pins one defect found by the 0.4.5 A/B benchmark: u64 nanosecond timestamps being
//! rejected by the prefilter, FAST never considering Numeric, and the new u16 lane.

use ace_core::{AccessHint, AceConfig, BlockSizePolicy, CodecId, CompressionProfile};
use ace_engine::AceEngine;

/// Deterministic xorshift generator so tests need no RNG dependency.
fn next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

/// Monotonic u64 nanosecond clock with up to ~1 ms of jitter (deltas need ~20 bits).
fn u64_ns_timestamps(bytes: usize) -> Vec<u8> {
    let (mut state, mut t) = (0x9E37_79B9_7F4A_7C15u64, 1_700_000_000_000_000_000u64);
    let mut out = Vec::with_capacity(bytes);
    while out.len() < bytes {
        t += 1_000_000 + next(&mut state) % 1_000_000;
        out.extend_from_slice(&t.to_le_bytes());
    }
    out.truncate(bytes);
    out
}

/// Slowly varying u16 sensor samples that wrap the lane once.
fn u16_series(bytes: usize) -> Vec<u8> {
    let (mut state, mut v) = (7u64, 65_000u16);
    let mut out = Vec::with_capacity(bytes);
    while out.len() < bytes {
        v = v.wrapping_add((next(&mut state) % 7) as u16);
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.truncate(bytes);
    out
}

/// Builds a single-threaded engine for the given profile.
fn engine(profile: CompressionProfile) -> AceEngine {
    let config = AceConfig { profile, threads: 1, ..AceConfig::default() };
    AceEngine::new(config).unwrap()
}

/// Nanosecond timestamps must compress well under BALANCED and FAST (previously 1.64x / 1.28x).
#[test]
fn ns_timestamps_use_numeric_in_fast_and_balanced() {
    let input = u64_ns_timestamps(1 << 20);
    for profile in [CompressionProfile::Fast, CompressionProfile::Balanced] {
        let e = engine(profile);
        let (encoded, stats) = e.compress_with_stats(&input).unwrap();
        assert_eq!(e.decompress(&encoded).unwrap(), input);
        assert!(stats.numeric_blocks > 0, "{profile:?} must select Numeric");
        assert!(encoded.len() * 3 < input.len(), "{profile:?} ratio too low: {}", encoded.len());
    }
}

/// `explain` must list a Numeric candidate and select it for FAST on timestamp data.
#[test]
fn fast_explain_selects_numeric_for_timestamps() {
    let input = u64_ns_timestamps(256 * 1024);
    let blocks = engine(CompressionProfile::Fast).explain(&input).unwrap();
    assert!(!blocks.is_empty());
    for block in blocks {
        assert!(block.candidates.iter().any(|c| matches!(c.decoding.codec, CodecId::Numeric)));
        assert!(matches!(block.selected.decoding.codec, CodecId::Numeric));
    }
}

/// The u16 lane must round-trip and compress, including the wrap across 65535 -> 0.
#[test]
fn u16_lane_roundtrips_and_compresses() {
    let input = u16_series(1 << 20);
    let e = engine(CompressionProfile::Balanced);
    let (encoded, stats) = e.compress_with_stats(&input).unwrap();
    assert_eq!(e.decompress(&encoded).unwrap(), input);
    assert!(stats.numeric_blocks > 0);
    assert!(encoded.len() * 2 < input.len());
}

/// Lengths that are not a multiple of the lane width must preserve the tail bytes.
#[test]
fn tail_bytes_survive_for_every_lane_width() {
    for tail in 1..8usize {
        let mut input = u64_ns_timestamps(64 * 1024);
        input.extend((0..tail).map(|i| 0xA0 + i as u8));
        let e = engine(CompressionProfile::Balanced);
        assert_eq!(e.decompress(&e.compress(&input).unwrap()).unwrap(), input, "tail {tail}");
    }
}

/// Output bytes must be identical for any worker-thread count.
#[test]
fn output_is_deterministic_across_thread_counts() {
    let input = u64_ns_timestamps(2 << 20);
    let mut outputs = Vec::new();
    for threads in [1usize, 2, 4] {
        let config = AceConfig { threads, ..AceConfig::default() };
        outputs.push(AceEngine::new(config).unwrap().compress(&input).unwrap());
    }
    assert!(outputs.windows(2).all(|w| w[0] == w[1]));
}

/// Auto block sizing must still honour the RandomAccess cap on numeric input.
#[test]
fn auto_block_policy_respects_random_access_cap() {
    let input = u64_ns_timestamps(4 << 20);
    let config = AceConfig { block_size_policy: BlockSizePolicy::Auto, access_hint: AccessHint::RandomAccess, threads: 1, ..AceConfig::default() };
    let e = AceEngine::new(config).unwrap();
    assert!(e.explain(&input).unwrap().len() >= 16, "random-access blocks must stay small");
    assert_eq!(e.decompress(&e.compress(&input).unwrap()).unwrap(), input);
}

/// Reusing the planner's `NumericEstimate` must produce exactly the bytes `numeric_encode` would
/// (BALANCED/DENSE run the same exhaustive width/mode search as `numeric_encode`).
#[test]
fn reused_numeric_estimate_matches_numeric_encode() {
    use std::io::Cursor;
    let input = u64_ns_timestamps(512 * 1024);
    for profile in [CompressionProfile::Balanced, CompressionProfile::Dense] {
        let encoded = engine(profile).compress(&input).unwrap();
        let mut reader = ace_format::AceReader::new(Cursor::new(&encoded), Default::default());
        let file = reader.read_file_header().unwrap();
        let mut offset = 0usize;
        for _ in 0..file.block_count {
            let (header, _, payload) = reader.read_block().unwrap();
            let block = &input[offset..offset + header.original_size as usize];
            offset += header.original_size as usize;
            if matches!(header.codec, CodecId::Numeric) {
                assert_eq!(payload, ace_codecs::numeric_encode(block).unwrap(), "{profile:?}");
            }
        }
    }
}
