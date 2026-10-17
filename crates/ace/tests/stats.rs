//! Tests of `ace` `stats` (formerly the inline unit tests of `src`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use ace::internal::stats_from_core;
use ace::*;
use ace_core::CompressionStats;

#[test]
fn counters_and_ratio_follow_the_internal_stats() {
    let mut core = CompressionStats {
        input_bytes: 1000,
        output_bytes: 250,
        block_count: 3,
        raw_blocks: 1,
        lz_blocks: 2,
        numeric_blocks: 4,
        time_series_blocks: 5,
        rle_blocks: 6,
        format_minor: 4,
        ..CompressionStats::default()
    };
    core.record_plan("lz+huffman");
    let s = stats_from_core(&core);
    assert_eq!((s.input_bytes, s.output_bytes, s.blocks), (1000, 250, 3));
    assert_eq!(s.format_minor, 4);
    assert_eq!(
        (
            s.raw_blocks,
            s.rle_blocks,
            s.lz_blocks,
            s.numeric_blocks,
            s.time_series_blocks
        ),
        (1, 6, 2, 4, 5)
    );
    assert_eq!(s.plan_distribution.get("lz+huffman"), Some(&1));
    assert!((s.ratio() - 4.0).abs() < 1e-12);
}

#[test]
fn ratio_of_an_empty_run_is_one() {
    assert!((Stats::default().ratio() - 1.0).abs() < 1e-12);
}
