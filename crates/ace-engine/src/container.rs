//! Ordered container assembly and statistics aggregation for the in-memory encoder.

use std::time::Instant;

use ace_core::{AceResult, CompressionStats};
use ace_format::{
    minimal_minor_version, AceWriter, FileHeader, FILE_FLAG_HAS_INDEX, FILE_HEADER_SIZE,
};

use crate::block_encoder::EncodedBlock;

/// Serializes blocks (already in id order) into a container and fills `stats`.
///
/// The declared format version is the minimal one the blocks need: 1.4 only when a block uses
/// the TS1 codec, otherwise 1.3 (see [`minimal_minor_version`]).
pub(crate) fn assemble_container(
    blocks: Vec<EncodedBlock>,
    input_len: usize,
    block_size: usize,
    write_index: bool,
    stats: &mut CompressionStats,
) -> AceResult<Vec<u8>> {
    let file_header = FileHeader {
        minor_version: minimal_minor_version(blocks.iter().map(|block| block.header.codec)),
        flags: if write_index { FILE_FLAG_HAS_INDEX } else { 0 },
        default_block_size: block_size as u32,
        original_size: input_len as u64,
        block_count: blocks.len() as u64,
    };
    let mut writer = AceWriter::new(Vec::with_capacity(
        input_len.saturating_add(FILE_HEADER_SIZE),
    ));
    writer.write_file_header(&file_header)?;

    let serialization_started = Instant::now();
    for block in &blocks {
        writer.write_block(&block.header, &block.metadata, &block.payload)?;
        record_block(stats, block);
    }
    stats.serialization_time = serialization_started.elapsed();

    let index_started = Instant::now();
    let (output, total) = writer.finish(write_index)?;
    if write_index {
        stats.index_time = index_started.elapsed();
    }
    stats.output_bytes = total;
    Ok(output)
}

/// Adds one block's timings, plan and planner counters to the aggregate statistics.
fn record_block(stats: &mut CompressionStats, block: &EncodedBlock) {
    let timings = block.timings;
    stats.route_classify_time += timings.route_classify;
    stats.generic_analysis_time += timings.generic_analysis;
    stats.analysis_time += timings.route_classify + timings.generic_analysis;
    stats.planning_time += timings.planning;
    stats.encoding_time += timings.encoding;
    stats.record_selected_plan(&block.plan);
    let telemetry = &block.planner_telemetry;
    stats.planner_fast_path_blocks += telemetry.fast_path_hit as u64;
    stats.planner_estimated_candidates += telemetry.estimated_candidates as u64;
    stats.planner_sampled_candidates += telemetry.sampled_candidates as u64;
    stats.planner_full_trial_encodes += telemetry.full_trial_encodes as u64;
}
