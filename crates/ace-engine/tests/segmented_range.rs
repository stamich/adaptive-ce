//! Segment-level random access (Format 1.6): a range read of a segmented block fetches its
//! header, its segment table and only the segments the range touches.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::io::Cursor;

use ace_core::{AceConfig, AceError, DecodeLimits, SegmentationConfig};
use ace_engine::{AceEngine, AceIndexedDecoder};
use ace_format::{
    checksum, decode_index, decode_trailer, encode_index, encode_trailer, FileTrailer,
    INDEX_FLAG_SEGMENTED, TRAILER_SIZE,
};
use ace_index::AceIndexReader;

/// 4 KiB windows, 16 KiB .. 128 KiB segments.
fn config(segmented: bool) -> AceConfig {
    AceConfig {
        block_size: 1 << 20,
        threads: 1,
        segmentation: segmented.then_some(SegmentationConfig {
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

/// Text, then counters, then noise: 512 KiB that the encoder segments.
fn mixed() -> Vec<u8> {
    let words = [
        "alpha ", "beta ", "gamma ", "delta ", "log ", "entry ", "value=", "ok\n",
    ];
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

fn open(file: &[u8]) -> AceIndexedDecoder<Cursor<&[u8]>> {
    AceIndexedDecoder::open(Cursor::new(file), DecodeLimits::default()).unwrap()
}

/// Rewrites the index of `file` after `edit` changed its entries (trailer and CRC included).
fn with_index(file: &[u8], edit: impl FnOnce(&mut ace_format::BlockIndex)) -> Vec<u8> {
    let trailer_at = file.len() - TRAILER_SIZE;
    let trailer = decode_trailer(&file[trailer_at..]).unwrap();
    let start = trailer.index_offset as usize;
    let mut index = decode_index(&file[start..trailer_at], &DecodeLimits::default()).unwrap();
    edit(&mut index);
    let bytes = encode_index(&index);
    let mut out = file[..start].to_vec();
    out.extend_from_slice(&bytes);
    out.extend_from_slice(&encode_trailer(FileTrailer {
        index_offset: trailer.index_offset,
        index_size: bytes.len() as u64,
        index_crc32c: checksum(&bytes),
    }));
    out
}

#[test]
fn the_writer_flags_segmented_blocks_in_the_index() {
    let input = mixed();
    let segmented = AceEngine::new(config(true))
        .unwrap()
        .compress(&input)
        .unwrap();
    let plain = AceEngine::new(config(false))
        .unwrap()
        .compress(&input)
        .unwrap();
    let flags = |file: &[u8]| {
        AceIndexReader::open(Cursor::new(file), DecodeLimits::default())
            .unwrap()
            .index
            .entries
            .iter()
            .map(|entry| entry.flags)
            .collect::<Vec<_>>()
    };
    assert_eq!(flags(&segmented), vec![INDEX_FLAG_SEGMENTED]);
    assert_eq!(flags(&plain), vec![0]);
}

#[test]
fn a_small_range_reads_one_segment_and_fewer_bytes_than_the_block() {
    let input = mixed();
    let file = AceEngine::new(config(true))
        .unwrap()
        .compress(&input)
        .unwrap();
    let mut decoder = open(&file);
    let span = decoder
        .range_metrics(0..input.len() as u64)
        .unwrap()
        .physical_bytes_read;

    // 1 KiB inside the first (text) segment: the incompressible tail of the block is not read.
    let (bytes, metrics) = decoder.read_range_with_metrics(1000..2024).unwrap();
    assert_eq!(bytes, &input[1000..2024]);
    assert_eq!(metrics.blocks_touched, 1);
    assert_eq!(metrics.segments_decoded, 1);
    assert!(
        metrics.physical_bytes_read < span / 4,
        "{} vs span {span}",
        metrics.physical_bytes_read
    );

    // 1 KiB inside the noise segment at the end: one segment, still fewer bytes than the block.
    let start = input.len() as u64 - 40_000;
    let (bytes, metrics) = decoder
        .read_range_with_metrics(start..start + 1024)
        .unwrap();
    assert_eq!(bytes, &input[start as usize..start as usize + 1024]);
    assert_eq!(metrics.segments_decoded, 1);
    assert!(metrics.physical_bytes_read < span);

    // A range across the text | counters boundary needs two segments at least.
    let start = 192 * 1024 - 100;
    let (bytes, metrics) = decoder.read_range_with_metrics(start..start + 200).unwrap();
    assert_eq!(bytes, &input[start as usize..start as usize + 200]);
    assert!(metrics.segments_decoded >= 2);
    assert!(metrics.physical_bytes_read <= span);
}

#[test]
fn the_whole_block_costs_exactly_its_span_and_never_more() {
    let input = mixed();
    let file = AceEngine::new(config(true))
        .unwrap()
        .compress(&input)
        .unwrap();
    let mut decoder = open(&file);
    let span = decoder
        .range_metrics(0..input.len() as u64)
        .unwrap()
        .physical_bytes_read;
    let (bytes, metrics) = decoder
        .read_range_with_metrics(0..input.len() as u64)
        .unwrap();
    assert_eq!(bytes, input);
    assert_eq!(metrics.physical_bytes_read, span);
    assert!(metrics.segments_decoded >= 2);
}

#[test]
fn unsegmented_files_keep_their_metrics() {
    let input = mixed();
    let file = AceEngine::new(config(false))
        .unwrap()
        .compress(&input)
        .unwrap();
    let mut decoder = open(&file);
    let span = decoder.range_metrics(0..10).unwrap().physical_bytes_read;
    let (bytes, metrics) = decoder.read_range_with_metrics(0..10).unwrap();
    assert_eq!(bytes, &input[..10]);
    assert_eq!(metrics.physical_bytes_read, span);
    assert_eq!(metrics.segments_decoded, 0);
}

#[test]
fn every_range_matches_the_input() {
    let mut input = mixed();
    input.extend(mixed());
    let file = AceEngine::new(AceConfig {
        block_size: 512 * 1024,
        ..config(true)
    })
    .unwrap()
    .compress(&input)
    .unwrap();
    let mut decoder = open(&file);
    let len = input.len() as u64;
    for step in 0..60u64 {
        let start = (step * 17_393) % len;
        let end = (start + 1 + (step * 31_337) % 300_000).min(len);
        let got = decoder.read_range(start..end).unwrap();
        assert_eq!(got, &input[start as usize..end as usize], "{start}..{end}");
    }
}

#[test]
fn a_flag_on_a_block_that_is_not_segmented_is_rejected() {
    let input = mixed();
    let plain = AceEngine::new(config(false))
        .unwrap()
        .compress(&input)
        .unwrap();
    let forged = with_index(&plain, |index| {
        index.entries[0].flags = INDEX_FLAG_SEGMENTED
    });
    let mut decoder = open(&forged);
    assert!(matches!(
        decoder.read_range(0..100),
        Err(AceError::Malformed(_))
    ));
    // Unknown flag bits are refused when the index is read.
    let unknown = with_index(&plain, |index| index.entries[0].flags = 0x2);
    assert!(AceIndexedDecoder::open(Cursor::new(&unknown), DecodeLimits::default()).is_err());
}

/// A missing flag is only a lost optimisation: the block is read whole and still decodes.
#[test]
fn a_missing_flag_falls_back_to_the_whole_block() {
    let input = mixed();
    let file = AceEngine::new(config(true))
        .unwrap()
        .compress(&input)
        .unwrap();
    let unflagged = with_index(&file, |index| index.entries[0].flags = 0);
    let mut decoder = open(&unflagged);
    let (bytes, metrics) = decoder.read_range_with_metrics(5..50).unwrap();
    assert_eq!(bytes, &input[5..50]);
    assert_eq!(metrics.segments_decoded, 0);
}

/// Damage inside a segment is found when the segment is read; a range that does not touch the
/// damaged segment is unaffected, and a full block decode still fails.
#[test]
fn damage_is_local_to_the_touched_segments() {
    let input = mixed();
    let mut file = AceEngine::new(config(true))
        .unwrap()
        .compress(&input)
        .unwrap();
    let last = file.len() - 600; // inside the last segment, before the index
    file[last] ^= 0xFF;
    let mut decoder = open(&file);
    assert!(
        decoder.read_range(0..1024).is_ok(),
        "first segment is intact"
    );
    assert!(decoder
        .read_range(input.len() as u64 - 100..input.len() as u64)
        .is_err());
    assert!(decoder.decode_block(0).is_err());
}
