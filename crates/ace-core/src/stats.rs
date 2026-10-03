use std::collections::BTreeMap;
use std::time::Duration;

/// Aggregate telemetry produced by one compression operation.
#[derive(Debug, Clone, Default)]
pub struct CompressionStats {
    /// Number of source bytes.
    pub input_bytes: u64,
    /// Number of ACE bytes written.
    pub output_bytes: u64,
    /// Number of processed blocks.
    pub block_count: u64,
    /// Number of blocks stored as RAW.
    pub raw_blocks: u64,
    /// Number of blocks using RLE.
    pub rle_blocks: u64,
    /// Number of blocks using LZ.
    pub lz_blocks: u64,
    /// Number of blocks using byte-delta transform.
    pub delta_blocks: u64,
    /// Number of blocks finalized by Huffman.
    pub huffman_blocks: u64,
    /// Number of blocks finalized by rANS.
    pub rans_blocks: u64,
    /// Stable physical-plan label to selected-block count mapping.
    pub plan_distribution: BTreeMap<String, u64>,
    /// Time spent collecting block statistics, summed across workers.
    pub analysis_time: Duration,
    /// Time spent generating/evaluating candidates, summed across workers.
    pub planning_time: Duration,
    /// Time spent encoding selected plans, summed across workers.
    pub encoding_time: Duration,
    /// Time spent assembling block headers/payloads into deterministic file order.
    pub serialization_time: Duration,
    /// Time spent building and serializing the final block index/trailer.
    pub index_time: Duration,
}

impl CompressionStats {
    /// Returns `input_bytes / output_bytes`, or `1.0` when no output bytes were produced.
    pub fn compression_ratio(&self) -> f64 {
        if self.output_bytes == 0 { 1.0 } else { self.input_bytes as f64 / self.output_bytes as f64 }
    }

    /// Increments the stable counter associated with one selected physical plan.
    pub fn record_plan(&mut self, label: impl Into<String>) {
        let entry = self.plan_distribution.entry(label.into()).or_insert(0);
        *entry = entry.saturating_add(1);
    }
}
