//! Random-access stress (ACE 0.4.6 hardening): 10 000 deterministic ranges and concurrent
//! independent readers must always return exactly the source bytes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::io::Cursor;
use std::sync::Arc;
use std::thread;

use ace_core::{AceConfig, DecodeLimits};
use ace_corpus::Workload;
use ace_engine::{AceEngine, AceIndexedDecoder};

/// Block size of the fixture (small, so ranges cross many block boundaries).
const BLOCK: u64 = 64 * 1024;
/// Fixture size: 4 MiB = 64 blocks.
const SIZE: usize = 4 * 1024 * 1024;
/// Number of ranges checked by the sequential stress test.
const RANGES: usize = 10_000;

/// Mixed 4 MiB source and its encoding with 64 KiB blocks.
fn fixture() -> (Vec<u8>, Vec<u8>) {
    let data = Workload::Mixed.generate(SIZE);
    let engine = AceEngine::new(AceConfig {
        block_size: BLOCK as usize,
        ..AceConfig::default()
    })
    .unwrap();
    let encoded = engine.compress(&data).unwrap();
    (data, encoded)
}

/// Deterministic xorshift64 generator for range selection.
struct Rng(u64);

impl Rng {
    /// Next pseudo-random value.
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// The `index`-th range of the stress sequence: cycles through the shapes listed in the
/// hardening plan (1 B, 4 KiB, 64 KiB, block-crossing, multi-block, EOF-touching, empty).
fn range(index: usize, rng: &mut Rng) -> (u64, u64) {
    let size = SIZE as u64;
    let offset = rng.next() % size;
    let (start, length) = match index % 7 {
        0 => (offset, 1),
        1 => (offset, 4096),
        2 => (offset, 65_536),
        3 => {
            let boundary = (offset / BLOCK + 1) * BLOCK;
            (boundary.saturating_sub(100), 200)
        }
        4 => (offset, 3 * BLOCK + 17),
        5 => (size - (offset % 10_000) - 1, 20_000),
        _ => (offset, 0),
    };
    let start = start.min(size);
    (start, (start + length).min(size))
}

/// 10 000 ranges on one reused decoder equal the source slices.
#[test]
fn ten_thousand_ranges_match_source() {
    let (data, encoded) = fixture();
    let mut decoder =
        AceIndexedDecoder::open(Cursor::new(&encoded), DecodeLimits::default()).unwrap();
    let mut rng = Rng(0x0ACE_0406);
    for index in 0..RANGES {
        let (start, end) = range(index, &mut rng);
        let bytes = decoder.read_range(start..end).unwrap();
        assert_eq!(
            bytes,
            &data[start as usize..end as usize],
            "range #{index} {start}..{end}"
        );
    }
}

/// Every block decodes individually to its source slice.
#[test]
fn every_block_decodes_individually() {
    let (data, encoded) = fixture();
    let mut decoder =
        AceIndexedDecoder::open(Cursor::new(&encoded), DecodeLimits::default()).unwrap();
    assert_eq!(decoder.block_count(), SIZE / BLOCK as usize);
    for block in 0..decoder.block_count() as u64 {
        let start = (block * BLOCK) as usize;
        assert_eq!(
            decoder.decode_block(block).unwrap(),
            &data[start..start + BLOCK as usize]
        );
    }
}

/// Ranges beyond the end of the file are rejected without panicking.
#[test]
fn out_of_bounds_ranges_are_rejected() {
    let (_, encoded) = fixture();
    let mut decoder =
        AceIndexedDecoder::open(Cursor::new(&encoded), DecodeLimits::default()).unwrap();
    let size = SIZE as u64;
    assert!(decoder.read_range(size..size + 1).is_err());
    assert!(decoder.read_range(0..u64::MAX).is_err());
    assert!(decoder.decode_block(u64::MAX).is_err());
}

/// Eight threads with independent decoders over one shared file read consistent bytes.
#[test]
fn concurrent_independent_readers() {
    let (data, encoded) = fixture();
    let data = Arc::new(data);
    let encoded = Arc::new(encoded);
    let handles: Vec<_> = (0..8u64)
        .map(|worker| {
            let data = Arc::clone(&data);
            let encoded = Arc::clone(&encoded);
            thread::spawn(move || {
                let mut decoder = AceIndexedDecoder::open(
                    Cursor::new(encoded.as_slice()),
                    DecodeLimits::default(),
                )
                .unwrap();
                let mut rng = Rng(0x5EED + worker);
                for index in 0..500 {
                    let (start, end) = range(index, &mut rng);
                    let bytes = decoder.read_range(start..end).unwrap();
                    assert_eq!(bytes, &data[start as usize..end as usize]);
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
}
