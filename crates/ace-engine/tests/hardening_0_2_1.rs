use ace_core::{AceConfig, CompressionProfile, DecodeLimits};
use ace_engine::{AceEngine, AceIndexedDecoder};
use std::io::Cursor;

/// Verifies bit-identical output for the full worker-count matrix used by ACE 0.2.1 benchmarks.
#[test]
fn deterministic_thread_matrix() {
    let data = b"ACE deterministic worker matrix / GraphNet / AdaptiveDB\n".repeat(80_000);
    for profile in [
        CompressionProfile::Fast,
        CompressionProfile::Balanced,
        CompressionProfile::Dense,
    ] {
        let mut reference = None;
        for threads in [1usize, 2, 4, 6, 8, 12] {
            let mut config = AceConfig::default();
            config.profile = profile;
            config.threads = threads;
            let encoded = AceEngine::new(config).unwrap().compress(&data).unwrap();
            if let Some(ref expected) = reference {
                assert_eq!(expected, &encoded);
            } else {
                reference = Some(encoded);
            }
        }
    }
}

/// Verifies that random-access diagnostics report touched blocks and non-zero physical work.
#[test]
fn random_access_metrics_are_consistent() {
    let data = (0u8..=255).cycle().take(2_000_000).collect::<Vec<_>>();
    let encoded = AceEngine::default_engine().compress(&data).unwrap();
    let indexed = AceIndexedDecoder::open(Cursor::new(encoded), DecodeLimits::default()).unwrap();
    let metrics = indexed.range_metrics(250_000..400_000).unwrap();
    assert_eq!(metrics.logical_bytes_requested, 150_000);
    assert!(metrics.blocks_touched >= 2);
    assert_eq!(metrics.blocks_touched, metrics.blocks_decoded);
    assert!(metrics.physical_bytes_read > 0);
}
