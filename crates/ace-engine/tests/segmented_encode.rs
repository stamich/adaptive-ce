//! The segmented block encoder (Format 1.6): when it is used, what it writes, and that it never
//! makes a block larger. Decoding is covered by `segmented_decode.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::io::Cursor;

use ace_core::{AceConfig, AceError, CodecId, DecodeLimits, SegmentationConfig};
use ace_engine::AceEngine;
use ace_format::{parse_inner_block, AceReader, SegmentTable};

/// 4 KiB windows, 16 KiB .. 128 KiB segments: small enough for fast tests.
fn segmentation() -> SegmentationConfig {
    SegmentationConfig {
        window_size: 4096,
        min_segment_size: 16 * 1024,
        max_segment_size: 128 * 1024,
        max_segments: 64,
    }
}

fn config(segmented: bool) -> AceConfig {
    AceConfig {
        block_size: 1 << 20,
        threads: 1,
        segmentation: segmented.then(segmentation),
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

/// Text that LZ and Huffman compress well.
fn text(len: usize) -> Vec<u8> {
    let words = [
        "alpha ", "beta ", "gamma ", "delta ", "log ", "entry ", "value=", "ok\n",
    ];
    let mut out = Vec::with_capacity(len + 8);
    let mut i = 0usize;
    while out.len() < len {
        out.extend_from_slice(words[(i * 7 + i / 3) % words.len()].as_bytes());
        i += 1;
    }
    out.truncate(len);
    out
}

/// A block whose halves need different plans: text, then fixed-step u32 counters, then noise.
fn mixed() -> Vec<u8> {
    let mut input = text(192 * 1024);
    input.extend((0u32..48 * 1024).flat_map(u32::to_le_bytes));
    input.extend(noise(128 * 1024, 4));
    input
}

/// The codec of every top-level block of `file` with its serialized form.
fn blocks_of(file: &[u8]) -> (u8, Vec<(ace_format::BlockHeader, Vec<u8>)>) {
    let mut reader = AceReader::new(Cursor::new(file), DecodeLimits::default());
    let header = reader.read_file_header().unwrap();
    let blocks = (0..header.block_count)
        .map(|_| {
            let (block_header, _metadata, payload) = reader.read_block().unwrap();
            (block_header, payload)
        })
        .collect();
    (header.minor_version, blocks)
}

#[test]
fn default_configuration_never_segments_and_keeps_the_old_bytes() {
    let input = mixed();
    let plain = AceEngine::new(config(false))
        .unwrap()
        .compress(&input)
        .unwrap();
    let (minor, blocks) = blocks_of(&plain);
    assert!(minor < 6, "no container codec, so not Format 1.6");
    assert!(blocks
        .iter()
        .all(|(header, _)| header.codec != CodecId::Segmented));
}

#[test]
fn a_mixed_block_is_stored_as_segments_and_is_smaller() {
    let input = mixed();
    let plain = AceEngine::new(config(false))
        .unwrap()
        .compress(&input)
        .unwrap();
    let engine = AceEngine::new(config(true)).unwrap();
    let (file, stats) = engine.compress_with_stats(&input).unwrap();
    assert!(
        file.len() < plain.len(),
        "{} !< {}",
        file.len(),
        plain.len()
    );

    let (minor, blocks) = blocks_of(&file);
    assert_eq!(minor, 6, "a segmented block makes the file Format 1.6");
    assert_eq!(blocks.len(), 1);
    let (header, payload) = &blocks[0];
    assert_eq!(header.codec, CodecId::Segmented);
    assert_eq!(header.metadata_size, 0);
    assert!(header.transforms.is_empty() && header.dictionary.is_none());
    assert_eq!(header.original_size as usize, input.len());

    // The payload is a valid SEG1 table and every inner block is valid and carries its position.
    let limits = DecodeLimits::default();
    let table = SegmentTable::parse(payload, input.len(), &limits).unwrap();
    assert!(table.entries.len() >= 2);
    for (index, (range, logical)) in table
        .payload_ranges()
        .into_iter()
        .zip(table.logical_ranges())
        .enumerate()
    {
        let inner = parse_inner_block(
            &payload[range],
            index as u64,
            logical.len() as u32,
            6,
            &limits,
        )
        .unwrap();
        assert_ne!(inner.0.codec, CodecId::Segmented);
        assert_ne!(inner.0.codec, CodecId::Columnar);
    }

    assert_eq!(stats.segmented_blocks, 1);
    assert_eq!(stats.segments as usize, table.entries.len());
    // The codec counters describe the stored bytes: one count per inner block, none for the outer.
    let inner_blocks = stats.raw_blocks
        + stats.rle_blocks
        + stats.lz_blocks
        + stats.numeric_blocks
        + stats.time_series_blocks;
    assert_eq!(inner_blocks, stats.segments);
}

#[test]
fn a_block_of_one_character_is_never_larger_than_unsegmented() {
    for input in [
        text(600 * 1024),
        vec![0u8; 600 * 1024],
        noise(300 * 1024, 7),
        (0u32..90_000).flat_map(u32::to_le_bytes).collect(),
    ] {
        let plain = AceEngine::new(config(false))
            .unwrap()
            .compress(&input)
            .unwrap();
        let segmented = AceEngine::new(config(true))
            .unwrap()
            .compress(&input)
            .unwrap();
        assert!(
            segmented.len() <= plain.len(),
            "{} > {}",
            segmented.len(),
            plain.len()
        );
    }
}

#[test]
fn short_blocks_are_left_alone() {
    // Below twice the minimum segment size there is nothing to cut.
    let input = mixed()[..24 * 1024].to_vec();
    let plain = AceEngine::new(config(false))
        .unwrap()
        .compress(&input)
        .unwrap();
    let segmented = AceEngine::new(config(true))
        .unwrap()
        .compress(&input)
        .unwrap();
    assert_eq!(plain, segmented);
}

#[test]
fn the_output_does_not_depend_on_the_thread_count() {
    let mut input = mixed();
    input.extend(mixed());
    let reference = AceEngine::new(config(true))
        .unwrap()
        .compress(&input)
        .unwrap();
    for threads in [2usize, 4] {
        let engine = AceEngine::new(AceConfig {
            threads,
            block_size: 512 * 1024,
            ..config(true)
        })
        .unwrap();
        let single_thread = AceEngine::new(AceConfig {
            threads: 1,
            block_size: 512 * 1024,
            ..config(true)
        })
        .unwrap();
        assert_eq!(
            engine.compress(&input).unwrap(),
            single_thread.compress(&input).unwrap()
        );
    }
    assert_eq!(
        reference,
        AceEngine::new(config(true))
            .unwrap()
            .compress(&input)
            .unwrap()
    );
}

#[test]
fn invalid_segmentation_is_rejected_when_the_engine_is_built() {
    let bad = AceConfig {
        segmentation: Some(SegmentationConfig {
            window_size: 0,
            ..segmentation()
        }),
        ..AceConfig::default()
    };
    assert!(matches!(
        AceEngine::new(bad),
        Err(AceError::InvalidConfig(_))
    ));
}
