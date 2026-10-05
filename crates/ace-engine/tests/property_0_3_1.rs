use std::io::Cursor;

use ace_core::{AceConfig, CompressionProfile, DecodeLimits};
use ace_engine::{AceEngine, AceIndexedDecoder};
use proptest::prelude::*;

/// Builds a deterministic engine configuration suitable for property tests.
fn config_for(profile: CompressionProfile, block_size: usize) -> AceConfig {
    let mut config = AceConfig::default();
    config.profile = profile;
    config.block_size = block_size;
    config.threads = 1;
    config
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// Arbitrary byte vectors must survive a complete encode/decode round-trip.
    #[test]
    fn arbitrary_roundtrip(
        bytes in proptest::collection::vec(any::<u8>(), 0..131_072usize),
        profile_index in 0usize..3usize,
    ) {
        let profile = match profile_index {
            0 => CompressionProfile::Fast,
            1 => CompressionProfile::Balanced,
            _ => CompressionProfile::Dense,
        };
        let engine = AceEngine::new(config_for(profile, 64 * 1024)).unwrap();
        let encoded = engine.compress(&bytes).unwrap();
        let decoded = engine.decompress(&encoded).unwrap();
        prop_assert_eq!(decoded, bytes);
    }

    /// Indexed range reads must equal the corresponding source slice for arbitrary ranges.
    #[test]
    fn arbitrary_indexed_range_matches_source(
        bytes in proptest::collection::vec(any::<u8>(), 1..262_144usize),
        a in 0usize..262_144usize,
        b in 0usize..262_144usize,
    ) {
        let engine = AceEngine::new(config_for(CompressionProfile::Balanced, 64 * 1024)).unwrap();
        let encoded = engine.compress(&bytes).unwrap();
        let mut decoder = AceIndexedDecoder::open(Cursor::new(encoded), DecodeLimits::default()).unwrap();
        let lo = a.min(b).min(bytes.len());
        let hi = a.max(b).min(bytes.len());
        let range = decoder.read_range(lo as u64..hi as u64).unwrap();
        prop_assert_eq!(range, bytes[lo..hi].to_vec());
    }
}

/// The same logical input must serialize identically across representative worker counts.
#[test]
fn deterministic_worker_matrix_0_3_1() {
    let data = b"ACE 0.3.1 deterministic matrix / planner-v3.6\n".repeat(40_000);
    for profile in [
        CompressionProfile::Fast,
        CompressionProfile::Balanced,
        CompressionProfile::Dense,
    ] {
        for block_size in [64 * 1024usize, 256 * 1024usize, 1024 * 1024usize] {
            let mut reference = None;
            for threads in [1usize, 2, 4] {
                let mut config = config_for(profile, block_size);
                config.threads = threads;
                let encoded = AceEngine::new(config).unwrap().compress(&data).unwrap();
                if let Some(expected) = &reference {
                    assert_eq!(expected, &encoded);
                } else {
                    reference = Some(encoded);
                }
            }
        }
    }
}
