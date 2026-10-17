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
    /// Minor version of the file format written (`1.<minor>`): 3, or 4 when a block uses the
    /// floating-point and time-series codec.
    pub format_minor: u8,
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
            format_minor: if stats.time_series_blocks > 0 { 4 } else { 3 },
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
