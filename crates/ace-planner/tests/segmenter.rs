//! The deterministic segmenter: tiling, size limits, determinism and the shape of the cuts.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace_core::{AceConfig, AceError, CompressionProfile};
use ace_planner::{
    classify_window, segment_boundaries, AlphabetBucket, SegmenterConfig,
};
use proptest::prelude::*;

/// A small configuration so the tests stay fast: 4 KiB windows, 8 KiB .. 64 KiB segments.
fn small() -> SegmenterConfig {
    SegmenterConfig {
        window_size: 4096,
        min_segment_size: 8192,
        max_segment_size: 64 * 1024,
        max_segments: 64,
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

/// English-like text over a small alphabet.
fn text(len: usize) -> Vec<u8> {
    let words = ["alpha ", "beta ", "gamma ", "delta ", "log ", "entry ", "value=", "ok\n"];
    let mut out = Vec::with_capacity(len + 8);
    let mut i = 0usize;
    while out.len() < len {
        out.extend_from_slice(words[(i * 7 + i / 3) % words.len()].as_bytes());
        i += 1;
    }
    out.truncate(len);
    out
}

/// Little-endian u32 counters.
fn counters(len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len + 4);
    let mut value = 0u32;
    while out.len() < len {
        out.extend_from_slice(&value.to_le_bytes());
        value += 1;
    }
    out.truncate(len);
    out
}

/// The ranges must tile `0..len` exactly.
fn assert_tiles(ranges: &[std::ops::Range<usize>], len: usize) {
    assert_eq!(ranges.first().unwrap().start, 0);
    assert_eq!(ranges.last().unwrap().end, len);
    for pair in ranges.windows(2) {
        assert_eq!(pair[0].end, pair[1].start);
    }
    assert!(ranges.iter().all(|range| !range.is_empty()));
}

#[test]
fn invalid_configurations_are_rejected() {
    let ok = small();
    assert!(ok.validate().is_ok());
    for bad in [
        SegmenterConfig { window_size: 0, ..ok },
        SegmenterConfig { min_segment_size: 0, ..ok },
        SegmenterConfig { min_segment_size: 5000, ..ok },
        SegmenterConfig { max_segment_size: 10_000, ..ok },
        SegmenterConfig { max_segment_size: 8192, ..ok },
        SegmenterConfig { max_segments: 1, ..ok },
    ] {
        assert!(matches!(bad.validate(), Err(AceError::InvalidConfig(_))), "{bad:?}");
        assert!(segment_boundaries(&[1, 2, 3], &AceConfig::default(), &bad).is_err());
    }
    assert!(SegmenterConfig::default().validate().is_ok());
}

#[test]
fn short_and_uniform_inputs_are_not_segmented() {
    let config = AceConfig::default();
    assert!(segment_boundaries(&[], &config, &small()).unwrap().is_empty());
    assert_eq!(segment_boundaries(&[7; 100], &config, &small()).unwrap(), vec![0..100]);
    let one_and_a_half = text(6000);
    assert_eq!(segment_boundaries(&one_and_a_half, &config, &small()).unwrap(), vec![0..6000]);
    let uniform = text(40_000);
    assert_eq!(segment_boundaries(&uniform, &config, &small()).unwrap(), vec![0..40_000]);
}

#[test]
fn cuts_follow_the_change_of_character() {
    let config = AceConfig::default();
    let mut input = text(128 * 1024);
    input.extend(noise(128 * 1024, 1));
    input.extend(counters(128 * 1024));
    let ranges = segment_boundaries(&input, &config, &small()).unwrap();
    assert_tiles(&ranges, input.len());
    let cuts: Vec<usize> = ranges.iter().map(|range| range.end).collect();
    assert!(cuts.contains(&(128 * 1024)), "text | noise boundary in {cuts:?}");
    assert!(cuts.contains(&(256 * 1024)), "noise | counters boundary in {cuts:?}");
}

#[test]
fn window_classes_distinguish_the_obvious_cases() {
    let config = AceConfig::default();
    assert_eq!(classify_window(&[0; 4096], &config).alphabet, AlphabetBucket::Constant);
    assert_eq!(classify_window(&text(4096), &config).alphabet, AlphabetBucket::Small);
    assert_eq!(classify_window(&noise(4096, 3), &config).alphabet, AlphabetBucket::Wide);
    assert_ne!(
        classify_window(&text(4096), &config),
        classify_window(&noise(4096, 3), &config)
    );
}

#[test]
fn a_short_blip_is_absorbed_and_long_runs_are_cut_evenly() {
    let config = AceConfig::default();
    // One noisy window inside a long text run is shorter than the minimum: it is merged away.
    let mut input = text(100 * 1024);
    input.extend(noise(4096, 9));
    input.extend(text(100 * 1024));
    let ranges = segment_boundaries(&input, &config, &small()).unwrap();
    assert_tiles(&ranges, input.len());
    assert!(ranges.iter().all(|range| range.len() >= 8192), "{ranges:?}");
    // 1 MiB of one character is cut into equal pieces of at most the maximum.
    let constant = vec![0u8; 1 << 20];
    let ranges = segment_boundaries(&constant, &config, &small()).unwrap();
    assert_tiles(&ranges, constant.len());
    assert_eq!(ranges.len(), 16);
    assert!(ranges.iter().all(|range| range.len() == 64 * 1024));
}

#[test]
fn too_many_segments_fall_back_to_a_uniform_cut() {
    let config = AceConfig::default();
    // 8 KiB runs alternate between text and noise: far more runs than 4 allowed segments.
    let mut input = Vec::new();
    for index in 0..40u64 {
        input.extend(if index % 2 == 0 { text(8192) } else { noise(8192, index) });
    }
    let capped = SegmenterConfig { max_segments: 4, ..small() };
    let ranges = segment_boundaries(&input, &config, &capped).unwrap();
    assert_tiles(&ranges, input.len());
    assert!((2..=4).contains(&ranges.len()), "{ranges:?}");
}

#[test]
fn boundaries_do_not_depend_on_threads_or_profile_order() {
    let mut input = text(96 * 1024);
    input.extend(noise(96 * 1024, 5));
    input.extend(counters(96 * 1024));
    let reference = segment_boundaries(&input, &AceConfig::default(), &small()).unwrap();
    for threads in [1usize, 2, 8] {
        let config = AceConfig { threads, ..AceConfig::default() };
        assert_eq!(segment_boundaries(&input, &config, &small()).unwrap(), reference, "{threads}");
    }
    for profile in [CompressionProfile::Fast, CompressionProfile::Balanced, CompressionProfile::Dense] {
        let config = AceConfig { profile, ..AceConfig::default() };
        let ranges = segment_boundaries(&input, &config, &small()).unwrap();
        assert_tiles(&ranges, input.len());
        assert_eq!(ranges, segment_boundaries(&input, &config, &small()).unwrap(), "{profile:?}");
    }
}

proptest! {
    /// For any mixture of parts: the ranges tile the input, respect the size window and the
    /// cap, and a second run gives the same cuts.
    #[test]
    fn any_mixture_respects_the_limits(
        parts in proptest::collection::vec((0u8..4, 1usize..40_000), 1..12),
    ) {
        let mut input = Vec::new();
        for (index, (kind, len)) in parts.iter().enumerate() {
            input.extend(match kind {
                0 => text(*len),
                1 => noise(*len, index as u64 + 1),
                2 => counters(*len),
                _ => vec![0u8; *len],
            });
        }
        let config = AceConfig::default();
        let segmenter = small();
        let ranges = segment_boundaries(&input, &config, &segmenter).unwrap();
        assert_tiles(&ranges, input.len());
        prop_assert!(ranges.len() <= segmenter.max_segments);
        prop_assert!(ranges.iter().all(|range| range.len() <= segmenter.max_segment_size + segmenter.window_size));
        if input.len() >= segmenter.min_segment_size {
            prop_assert!(
                ranges.iter().all(|range| range.len() >= segmenter.min_segment_size),
                "{ranges:?}"
            );
        }
        for range in &ranges[..ranges.len().saturating_sub(1)] {
            prop_assert_eq!(range.end % segmenter.window_size, 0);
        }
        prop_assert_eq!(&ranges, &segment_boundaries(&input, &config, &segmenter).unwrap());
    }
}
