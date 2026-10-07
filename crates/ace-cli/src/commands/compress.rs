//! `compress` and `compress-stream`.

use std::io::{BufReader, BufWriter};

use ace_core::{AccessHint, AceConfig, BlockSizePolicy, CompressionProfile};
use ace_engine::AceEngine;
use ace_stream::{compress_reader_known_size, StreamLimits};
use anyhow::{Context, Result};

use crate::files;

/// Compresses a file in memory and prints the most important telemetry.
pub fn compress(
    input: &str,
    output: &str,
    threads: usize,
    profile: CompressionProfile,
    block_policy: BlockSizePolicy,
    access_hint: AccessHint,
) -> Result<()> {
    let data = files::read(input)?;
    let config = AceConfig {
        threads,
        profile,
        block_size_policy: block_policy,
        access_hint,
        ..AceConfig::default()
    };
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
    Ok(())
}

/// Compresses a file with bounded source-block memory and a deterministic Format 1.3 index.
pub fn compress_stream(input: &str, output: &str, profile: CompressionProfile) -> Result<()> {
    let source = files::open(input)?;
    let size = source
        .metadata()
        .with_context(|| format!("stat {input}"))?
        .len();
    let sink = files::create(output)?;
    let config = AceConfig {
        profile,
        ..AceConfig::default()
    };
    let stats = compress_reader_known_size(
        BufReader::new(source),
        BufWriter::new(sink),
        size,
        config,
        StreamLimits::default(),
    )?;
    println!(
        "ACE {} streamed {} -> {} bytes blocks={} peak_source_buffer={}",
        env!("CARGO_PKG_VERSION"),
        stats.input_bytes,
        stats.output_bytes,
        stats.blocks,
        stats.peak_source_buffer_bytes
    );
    Ok(())
}
