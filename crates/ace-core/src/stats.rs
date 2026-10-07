use std::collections::BTreeMap;
use std::time::Duration;

use crate::{CodecId, EntropyCodecId, PhysicalCompressionPlan, TransformId};

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
    /// Number of blocks using ACE 0.4 numeric FOR/Delta/DoD+BitPack codec.
    pub numeric_blocks: u64,
    /// Blocks encoded with the TS1 time-series codec (Format 1.4).
    pub time_series_blocks: u64,
    /// Number of blocks using byte-delta transform.
    pub delta_blocks: u64,
    /// Number of blocks finalized by Huffman.
    pub huffman_blocks: u64,
    /// Number of blocks finalized by scalar rANS.
    pub rans_blocks: u64,
    /// Number of blocks finalized by four-lane rANS.
    pub rans4x_blocks: u64,
    /// Number of blocks resolved by a planner fast path without sampled verification.
    pub planner_fast_path_blocks: u64,
    /// Number of candidates analytically estimated by Planner V4.
    pub planner_estimated_candidates: u64,
    /// Number of candidates actually sample-encoded by Planner V4.
    pub planner_sampled_candidates: u64,
    /// Number of complete candidate trial encodes performed by the hot-path planner.
    pub planner_full_trial_encodes: u64,
    /// Stable physical-plan label to selected-block count mapping.
    pub plan_distribution: BTreeMap<String, u64>,
    /// Time spent in route prefilter/classification/full NumericFast validation.
    pub route_classify_time: Duration,
    /// Time spent collecting generic block statistics after route classification.
    pub generic_analysis_time: Duration,
    /// Time spent collecting block statistics, including route classification.
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

/// Inherent methods of [`CompressionStats`].
impl CompressionStats {
    /// Returns `input_bytes / output_bytes`, or `1.0` when no output bytes were produced.
    pub fn compression_ratio(&self) -> f64 {
        if self.output_bytes == 0 {
            1.0
        } else {
            self.input_bytes as f64 / self.output_bytes as f64
        }
    }

    /// Counts one selected block plan: codec, entropy coder, delta transform and plan label.
    pub fn record_selected_plan(&mut self, plan: &PhysicalCompressionPlan) {
        match plan.decoding.codec {
            CodecId::Raw => self.raw_blocks += 1,
            CodecId::Rle => self.rle_blocks += 1,
            CodecId::Lz => self.lz_blocks += 1,
            CodecId::Numeric => self.numeric_blocks += 1,
            CodecId::TimeSeries => self.time_series_blocks += 1,
        }
        match plan.decoding.entropy {
            EntropyCodecId::Huffman => self.huffman_blocks += 1,
            EntropyCodecId::Rans => self.rans_blocks += 1,
            EntropyCodecId::Rans4x => self.rans4x_blocks += 1,
            EntropyCodecId::None => {}
        }
        if plan.decoding.transforms.contains(&TransformId::DeltaByte) {
            self.delta_blocks += 1;
        }
        self.record_plan(plan.label());
    }

    /// Increments the stable counter associated with one selected physical plan.
    pub fn record_plan(&mut self, label: impl Into<String>) {
        let entry = self.plan_distribution.entry(label.into()).or_insert(0);
        *entry = entry.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Recording a plan increments exactly the matching counters.
    #[test]
    fn record_selected_plan_counts_codec_entropy_and_delta() {
        let mut stats = CompressionStats::default();
        let mut plan = PhysicalCompressionPlan::raw();
        plan.decoding.codec = CodecId::Rle;
        plan.decoding.entropy = EntropyCodecId::Huffman;
        plan.decoding.transforms = vec![TransformId::DeltaByte];
        stats.record_selected_plan(&plan);
        stats.record_selected_plan(&PhysicalCompressionPlan::raw());
        assert_eq!(
            (
                stats.rle_blocks,
                stats.raw_blocks,
                stats.huffman_blocks,
                stats.delta_blocks
            ),
            (1, 1, 1, 1)
        );
        assert_eq!(stats.plan_distribution.get("delta+rle+huffman"), Some(&1));
        assert_eq!(stats.plan_distribution.get("raw+none"), Some(&1));
    }
}
