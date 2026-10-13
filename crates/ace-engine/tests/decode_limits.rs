//! Every `DecodeLimits` field is enforced by the sequential and by the indexed reader
//! (ACE 0.5.1 resource-limit contract, see the table on `DecodeLimits`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::io::Cursor;

use ace_core::{AceConfig, AceError, BlockSizePolicy, DecodeLimits};
use ace_engine::{AceEngine, AceIndexedDecoder};

/// Block size of the fixture (eight blocks of 4 KiB).
const BLOCK: usize = 4096;

/// Eight blocks of compressible text.
fn fixture() -> (Vec<u8>, Vec<u8>) {
    let input: Vec<u8> = b"limit fixture: the quick brown fox. "
        .iter()
        .copied()
        .cycle()
        .take(8 * BLOCK)
        .collect();
    let config = AceConfig {
        block_size: BLOCK,
        block_size_policy: BlockSizePolicy::Fixed,
        ..AceConfig::default()
    };
    let encoded = AceEngine::new(config).unwrap().compress(&input).unwrap();
    (input, encoded)
}

/// Decodes `encoded` sequentially (in memory and from a reader) and through the indexed
/// reader; returns the three outcomes.
fn decode_all(encoded: &[u8], limits: &DecodeLimits) -> [Result<(), AceError>; 3] {
    let engine = AceEngine::default_engine().with_decode_limits(limits.clone());
    let in_memory = engine.decompress(encoded).map(drop);
    let streamed = engine.decompress_from(Cursor::new(encoded), Vec::new());
    let indexed = AceIndexedDecoder::open(Cursor::new(encoded), limits.clone())
        .and_then(|mut decoder| decoder.read_range(0..8 * BLOCK as u64).map(drop));
    [in_memory, streamed, indexed]
}

/// Asserts that every reader rejects `encoded` under `limits` with a resource-limit error.
fn assert_all_refuse(encoded: &[u8], limits: DecodeLimits, what: &str) {
    for (reader, outcome) in ["in-memory", "streamed", "indexed"]
        .iter()
        .zip(decode_all(encoded, &limits))
    {
        match outcome {
            Err(AceError::ResourceLimitExceeded(_)) => {}
            other => panic!("{what}: {reader} reader returned {other:?}"),
        }
    }
}

/// The fixture decodes under the defaults and under limits that are exactly large enough.
#[test]
fn exact_limits_accept_the_fixture() {
    let (_, encoded) = fixture();
    let exact = DecodeLimits {
        max_output_size: 8 * BLOCK as u64,
        max_block_size: BLOCK,
        max_index_entries: 8,
        ..DecodeLimits::default()
    };
    for limits in [DecodeLimits::default(), exact] {
        for outcome in decode_all(&encoded, &limits) {
            outcome.unwrap();
        }
    }
}

/// Block count above `max_index_entries` (= `max_block_count`).
#[test]
fn block_count_limit() {
    let (_, encoded) = fixture();
    let limits = DecodeLimits {
        max_index_entries: 7,
        ..DecodeLimits::default()
    };
    assert_eq!(limits.max_block_count(), 7);
    assert_all_refuse(&encoded, limits, "max_index_entries");
}

/// Output size, block size and encoded block size one below the fixture's needs.
#[test]
fn size_limits() {
    let (_, encoded) = fixture();
    let cases = [
        (
            "max_output_size",
            DecodeLimits {
                max_output_size: 8 * BLOCK as u64 - 1,
                ..DecodeLimits::default()
            },
        ),
        (
            "max_block_size",
            DecodeLimits {
                max_block_size: BLOCK - 1,
                ..DecodeLimits::default()
            },
        ),
        (
            "max_encoded_block_size",
            DecodeLimits {
                max_encoded_block_size: 1,
                ..DecodeLimits::default()
            },
        ),
    ];
    for (what, limits) in cases {
        assert_all_refuse(&encoded, limits, what);
    }
}
