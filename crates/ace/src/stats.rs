//! What a compression run did.

use std::collections::BTreeMap;

use ace_core::CompressionStats;

/// Summary of one compression run.
///
/// Only the counters that are stable across releases are exposed; the planner's internal
/// timings and counters are not part of the public API. New fields may be added in a minor
/// release.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct Stats {
    /// Bytes read.
    pub input_bytes: u64,
    /// Bytes of the ACE file written.
    pub output_bytes: u64,
    /// Number of blocks.
    pub blocks: u64,
    /// Blocks stored as is.
    pub raw_blocks: u64,
    /// Blocks coded as run-length.
    pub rle_blocks: u64,
    /// Blocks coded with the LZ codec.
    pub lz_blocks: u64,
    /// Blocks coded with the integer codecs (FOR, delta, delta-of-delta).
    pub numeric_blocks: u64,
    /// Blocks coded with the floating-point and time-series codec.
    pub time_series_blocks: u64,
    /// How many blocks use each pipeline, keyed by its label (for example `"lz+huffman"`).
    pub plan_distribution: BTreeMap<String, u64>,
}

impl Stats {
    /// `input_bytes / output_bytes`; `1.0` when nothing was written.
    #[must_use]
    pub fn ratio(&self) -> f64 {
        if self.output_bytes == 0 {
            1.0
        } else {
            self.input_bytes as f64 / self.output_bytes as f64
        }
    }

    pub(crate) fn from_core(stats: &CompressionStats) -> Self {
        Self {
            input_bytes: stats.input_bytes,
            output_bytes: stats.output_bytes,
            blocks: stats.block_count,
            raw_blocks: stats.raw_blocks,
            rle_blocks: stats.rle_blocks,
            lz_blocks: stats.lz_blocks,
            numeric_blocks: stats.numeric_blocks,
            time_series_blocks: stats.time_series_blocks,
            plan_distribution: stats.plan_distribution.clone(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

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
            ..CompressionStats::default()
        };
        core.record_plan("lz+huffman");
        let s = Stats::from_core(&core);
        assert_eq!((s.input_bytes, s.output_bytes, s.blocks), (1000, 250, 3));
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
}
