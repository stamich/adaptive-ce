//! Segmentation in the facade: validation, the builder and an end-to-end segmented file.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::io::Cursor;

use ace::{Config, Engine, ErrorKind, RangeReader, Segmentation};

/// Text, then counters, then noise: a block the segmenter cuts.
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
    let mut state = 4u64;
    input.extend((0..128 * 1024).map(|_| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 24) as u8
    }));
    input
}

fn segmentation() -> Segmentation {
    Segmentation::default()
        .window_size(4096)
        .min_segment_size(16 * 1024)
        .max_segment_size(128 * 1024)
        .max_segments(64)
}

#[test]
fn the_defaults_and_getters() {
    let defaults = Segmentation::default();
    assert_eq!(defaults.window_size_bytes(), 16 * 1024);
    assert_eq!(defaults.min_segment_size_bytes(), 64 * 1024);
    assert_eq!(defaults.max_segment_size_bytes(), 1 << 20);
    assert_eq!(defaults.max_segments_per_block(), 1024);
    assert_eq!(Config::default().segmentation(), None);
    let config = Config::builder()
        .segmentation(segmentation())
        .build()
        .unwrap();
    assert_eq!(config.segmentation(), Some(segmentation()));
}

#[test]
fn invalid_settings_are_invalid_configuration() {
    for bad in [
        Segmentation::default().window_size(0),
        Segmentation::default().min_segment_size(1000),
        Segmentation::default().max_segment_size(64 * 1024),
        Segmentation::default().max_segments(1),
    ] {
        let error = Config::builder().segmentation(bad).build().unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidConfig);
    }
}

#[test]
fn a_mixed_block_is_segmented_and_read_back() {
    let input = mixed();
    let plain = Engine::new(Config::builder().block_size(1 << 20).build().unwrap()).unwrap();
    let engine = Engine::new(
        Config::builder()
            .block_size(1 << 20)
            .segmentation(segmentation())
            .build()
            .unwrap(),
    )
    .unwrap();
    let (file, stats) = engine.compress_with_stats(&input).unwrap();
    assert_eq!(stats.segmented_blocks, 1);
    assert_eq!(stats.format_minor, 6);
    assert!(file.len() < plain.compress(&input).unwrap().len());
    assert_eq!(Engine::default().decompress(&file).unwrap(), input);

    // A range inside the counters decodes one segment, not the whole block.
    let mut reader = RangeReader::open(Cursor::new(&file)).unwrap();
    let offset = 192 * 1024 + 40_000;
    let (bytes, range_stats) = reader.read_range_with_stats(offset..offset + 100).unwrap();
    assert_eq!(bytes, &input[offset as usize..offset as usize + 100]);
    assert!(range_stats.physical_bytes < file.len() as u64 / 2);
}

#[test]
fn uniform_data_is_not_segmented() {
    let text = b"the same line again and again\n".repeat(20_000);
    let engine = Engine::new(
        Config::builder()
            .block_size(1 << 20)
            .segmentation(segmentation())
            .build()
            .unwrap(),
    )
    .unwrap();
    let (file, stats) = engine.compress_with_stats(&text).unwrap();
    assert_eq!(stats.segmented_blocks, 0);
    let plain = Engine::new(Config::builder().block_size(1 << 20).build().unwrap()).unwrap();
    assert_eq!(file, plain.compress(&text).unwrap());
}
