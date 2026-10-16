//! `decompress`, `verify`, `decode-block` and `read-range`.

use std::io::{Cursor, Write};

use ace::{DecodeLimits, Engine, RangeReader};
use ace_format::{AceReader, FILE_FLAG_HAS_INDEX};
use anyhow::{Context, Result};

use crate::files;

/// Decompresses an ACE file and writes the reconstructed bytes.
pub fn decompress(input: &str, output: &str) -> Result<()> {
    files::ensure_distinct(input, output)?;
    let decoded = Engine::default().decompress(&files::read(input)?)?;
    files::write(output, &decoded)
}

/// Verifies full reconstruction, every block CRC32C and (if present) the block index.
pub fn verify(input: &str) -> Result<()> {
    let mut out = std::io::stdout().lock();
    let data = files::read(input)?;
    let decoded = Engine::default().decompress(&data)?;
    let header =
        AceReader::new(Cursor::new(&data), ace_core::DecodeLimits::default()).read_file_header()?;
    if header.flags & FILE_FLAG_HAS_INDEX != 0 {
        let indexed = RangeReader::open(Cursor::new(&data))?;
        writeln!(
            out,
            "validated block index entries={}",
            indexed.block_count()
        )?;
    }
    writeln!(out, "verified {} reconstructed bytes", decoded.len())?;
    Ok(())
}

/// Decodes one indexed block and writes its reconstructed bytes.
pub fn decode_block(input: &str, block_id: u64, output: &str) -> Result<()> {
    files::ensure_distinct(input, output)?;
    let mut out = std::io::stdout().lock();
    let mut reader = RangeReader::open_with_limits(files::open(input)?, &DecodeLimits::default())?;
    let block = reader.read_block(block_id)?;
    files::write(output, &block)?;
    writeln!(out, "decoded block {block_id}: {} bytes", block.len())?;
    Ok(())
}

/// Decodes only the blocks intersecting `[offset, offset + length)` and writes that range.
pub fn read_range(input: &str, offset: u64, length: u64, output: &str) -> Result<()> {
    files::ensure_distinct(input, output)?;
    let mut out = std::io::stdout().lock();
    let end = offset.checked_add(length).context("range overflow")?;
    let mut reader = RangeReader::open_with_limits(files::open(input)?, &DecodeLimits::default())?;
    let (bytes, stats) = reader.read_range_with_stats(offset..end)?;
    files::write(output, &bytes)?;
    writeln!(out,
        "decoded logical range [{offset}, {end}): {} bytes physical={} blocks={} physical/logical={:.3}",
        bytes.len(),
        stats.physical_bytes,
        stats.blocks,
        stats.read_amplification()
    )?;
    Ok(())
}
