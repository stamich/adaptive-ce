//! `compress` and `compress-stream`.

use std::io::{BufReader, BufWriter, Write};

use ace::{Config, Engine};
use anyhow::{Context, Result};

use crate::files;

/// Compresses a file in memory and prints the most important statistics.
pub fn compress(input: &str, output: &str, config: Config) -> Result<()> {
    files::ensure_distinct(input, output)?;
    let mut out = std::io::stdout().lock();
    let data = files::read(input)?;
    let (encoded, stats) = Engine::new(config)?.compress_with_stats(&data)?;
    files::write(output, &encoded)?;
    writeln!(
        out,
        "ACE {} compressed {} -> {} bytes ratio={:.3} blocks={} raw={} rle={} lz={} numeric={} ts1={} format=1.{}",
        env!("CARGO_PKG_VERSION"),
        stats.input_bytes,
        stats.output_bytes,
        stats.ratio(),
        stats.blocks,
        stats.raw_blocks,
        stats.rle_blocks,
        stats.lz_blocks,
        stats.numeric_blocks,
        stats.time_series_blocks,
        stats.format_minor
    )?;
    Ok(())
}

/// Compresses a file with bounded source-block memory and a deterministic block index.
///
/// The output is byte-identical to `compress` with the same configuration, including the
/// Format 1.4 / 1.5 header when a block needs it (the header is rewritten in place at the end).
pub fn compress_stream(input: &str, output: &str, config: Config) -> Result<()> {
    files::ensure_distinct(input, output)?;
    let mut out = std::io::stdout().lock();
    let source = files::open(input)?;
    let size = source
        .metadata()
        .with_context(|| format!("stat {input}"))?
        .len();
    let sink = files::create(output)?;
    let stats =
        Engine::new(config)?.compress_stream(BufReader::new(source), BufWriter::new(sink), size)?;
    writeln!(
        out,
        "ACE {} streamed {} -> {} bytes blocks={} format=1.{}",
        env!("CARGO_PKG_VERSION"),
        stats.input_bytes,
        stats.output_bytes,
        stats.blocks,
        stats.format_minor
    )?;
    Ok(())
}
