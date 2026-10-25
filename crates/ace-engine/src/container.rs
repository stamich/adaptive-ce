//! Ordered container assembly and statistics aggregation for the in-memory encoder.

use std::time::Instant;

use ace_codecs::TimeSeriesMode;
use ace_core::{
    AceResult, CodecId, CompressionStats, DictionaryScope, EncoderDictionary, FORMAT_MINOR_STORAGE,
};
use ace_format::{
    minimal_minor_version, AceWriter, EmbeddedDictionary, FileHeader, FILE_FLAG_HAS_DICTIONARIES,
    FILE_FLAG_HAS_INDEX, FILE_HEADER_SIZE,
};

use crate::block_encoder::{uses_dictionary, EncodedBlock};
use crate::block_pipeline::block_format_minor;

/// Serializes blocks (already in id order) into a container and fills `stats`.
///
/// The declared format version is the minimal one the blocks need: 1.4 when a block uses the
/// TS1 codec, 1.5 when a payload uses a Format 1.5 mode, otherwise 1.3 (see
/// [`minimal_minor_version`] and [`block_format_minor`]).
///
/// A block that refers to `dictionary` (Format 1.6) makes the file 1.6, and with scope `File`
/// the dictionary is embedded in a section right behind the file header. The section is written
/// only when a block uses it, so a file that happens not to use the dictionary is exactly the
/// file the same input would give without one.
pub(crate) fn assemble_container(
    blocks: Vec<EncodedBlock>,
    input_len: usize,
    block_size: usize,
    write_index: bool,
    dictionary: Option<&EncoderDictionary>,
    stats: &mut CompressionStats,
) -> AceResult<Vec<u8>> {
    let dictionary_used = dictionary.filter(|_| blocks.iter().any(uses_dictionary));
    let embedded = dictionary_used.filter(|dictionary| dictionary.scope == DictionaryScope::File);
    let mut minor_version =
        minimal_minor_version(blocks.iter().map(|block| {
            block_format_minor(block.header.codec, block.header.entropy, &block.payload)
        }));
    if dictionary_used.is_some() {
        minor_version = minor_version.max(FORMAT_MINOR_STORAGE);
    }
    stats.format_minor = minor_version;
    let mut flags = if write_index { FILE_FLAG_HAS_INDEX } else { 0 };
    if embedded.is_some() {
        flags |= FILE_FLAG_HAS_DICTIONARIES;
    }
    let file_header = FileHeader {
        minor_version,
        flags,
        default_block_size: block_size as u32,
        original_size: input_len as u64,
        block_count: blocks.len() as u64,
    };
    let mut writer = AceWriter::new(Vec::with_capacity(
        input_len.saturating_add(FILE_HEADER_SIZE),
    ));
    writer.write_file_header(&file_header)?;
    if let Some(dictionary) = embedded {
        writer.write_dictionary_section(&[EmbeddedDictionary {
            id: dictionary.id,
            bytes: dictionary.bytes.to_vec(),
        }])?;
    }

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
    // A container block also counts the plans of its inner blocks, so the codec and entropy
    // counters keep describing the bytes actually stored.
    let inner_blocks = block.inner_plans.len() as u64;
    match block.header.codec {
        CodecId::Segmented => stats.segments += inner_blocks,
        CodecId::Columnar => stats.column_streams += inner_blocks,
        _ => {}
    }
    for plan in &block.inner_plans {
        stats.record_selected_plan(plan);
    }
    let telemetry = &block.planner_telemetry;
    stats.planner_fast_path_blocks += telemetry.fast_path_hit as u64;
    stats.planner_estimated_candidates += telemetry.estimated_candidates as u64;
    stats.planner_sampled_candidates += telemetry.sampled_candidates as u64;
    stats.planner_full_trial_encodes += telemetry.full_trial_encodes as u64;
    stats.time_series_estimates += telemetry.time_series_estimates as u64;
    stats.float_route_blocks += block.route.is_float() as u64;
    stats.float_fast_blocks += telemetry.float_fast_hit as u64;
    stats.float_fast_fallbacks += telemetry.float_fast_fallback as u64;
    match block.time_series_layout.map(|layout| layout.mode) {
        Some(TimeSeriesMode::GorillaF64) => stats.ts1_gorilla_f64_blocks += 1,
        Some(TimeSeriesMode::GorillaF32) => stats.ts1_gorilla_f32_blocks += 1,
        Some(TimeSeriesMode::RunDelta) => stats.ts1_run_delta_blocks += 1,
        Some(TimeSeriesMode::Alp) => stats.ts1_alp_blocks += 1,
        None => {}
    }
}
