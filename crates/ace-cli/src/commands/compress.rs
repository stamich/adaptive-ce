//! `compress` and `compress-stream`.

use std::io::{BufReader, BufWriter};

use ace_core::AceConfig;
use ace_engine::AceEngine;
use ace_stream::{compress_reader_known_size, StreamLimits};
use anyhow::{Context, Result};

use crate::files;

/// Compresses a file in memory and prints the most important telemetry.
pub fn compress(input: &str, output: &str, config: AceConfig) -> Result<()> {
    let data = files::read(input)?;
    let (encoded, stats) = AceEngine::new(config)?.compress_with_stats(&data)?;
    files::write(output, &encoded)?;
    println!(
        "ACE {} compressed {} -> {} bytes ratio={:.3} blocks={} numeric={} rANS={} rANS4x={} Huffman={} fast-path={} sampled={}",
        env!("CARGO_PKG_VERSION"),
        stats.input_bytes,
        stats.output_bytes,
        stats.compression_ratio(),
        stats.block_count,
        stats.numeric_blocks,
        stats.rans_blocks,
        stats.rans4x_blocks,
        stats.huffman_blocks,
        stats.planner_fast_path_blocks,
        stats.planner_sampled_candidates
    );
    if stats.float_route_blocks > 0 || stats.time_series_blocks > 0 {
        println!(
            "  float-route={} float-fast={} float-fast-fallbacks={} ts1={} (gorilla_f64={} gorilla_f32={} run_delta={})",
            stats.float_route_blocks,
            stats.float_fast_blocks,
            stats.float_fast_fallbacks,
            stats.time_series_blocks,
            stats.ts1_gorilla_f64_blocks,
            stats.ts1_gorilla_f32_blocks,
            stats.ts1_run_delta_blocks
        );
    }
    Ok(())
}

/// Compresses a file with bounded source-block memory and a deterministic block index.
///
/// The output is byte-identical to `compress` with the same configuration, including the
/// Format 1.4 header when a block uses TS1 (the header is rewritten in place at the end).
pub fn compress_stream(input: &str, output: &str, config: AceConfig) -> Result<()> {
    let source = files::open(input)?;
    let size = source
        .metadata()
        .with_context(|| format!("stat {input}"))?
        .len();
    let sink = files::create(output)?;
    let stats = compress_reader_known_size(
        BufReader::new(source),
        BufWriter::new(sink),
        size,
        config,
        StreamLimits::default(),
    )?;
    println!(
        "ACE {} streamed {} -> {} bytes blocks={} peak_source_buffer={} format=1.{}",
        env!("CARGO_PKG_VERSION"),
        stats.input_bytes,
        stats.output_bytes,
        stats.blocks,
        stats.peak_source_buffer_bytes,
        if stats.format_1_4 { 4 } else { 3 }
    );
    Ok(())
}
