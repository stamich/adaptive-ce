//! Tests of `ace_core` `stats` (formerly the inline unit tests of `src`).

use ace_core::*;

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
