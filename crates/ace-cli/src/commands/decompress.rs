//! `decompress`, `verify`, `decode-block` and `read-range`.

use std::io::Cursor;

use ace_core::DecodeLimits;
use ace_engine::{AceEngine, AceIndexedDecoder};
use ace_format::{AceReader, FILE_FLAG_HAS_INDEX};
use anyhow::{Context, Result};

use crate::files;

/// Decompresses an ACE file and writes the reconstructed bytes.
pub fn decompress(input: &str, output: &str) -> Result<()> {
    let decoded = AceEngine::default_engine().decompress(&files::read(input)?)?;
    files::write(output, &decoded)
}

/// Verifies full reconstruction, every block CRC32C and (if present) the block index.
pub fn verify(input: &str) -> Result<()> {
    let data = files::read(input)?;
    let decoded = AceEngine::default_engine().decompress(&data)?;
    let header = AceReader::new(Cursor::new(&data), DecodeLimits::default()).read_file_header()?;
    if header.flags & FILE_FLAG_HAS_INDEX != 0 {
        let indexed = AceIndexedDecoder::open(Cursor::new(&data), DecodeLimits::default())?;
        println!("validated block index entries={}", indexed.block_count());
    }
    println!("verified {} reconstructed bytes", decoded.len());
    Ok(())
}

/// Decodes one indexed block and writes its reconstructed bytes.
pub fn decode_block(input: &str, block_id: u64, output: &str) -> Result<()> {
    let mut decoder = AceIndexedDecoder::open(files::open(input)?, DecodeLimits::default())?;
    let block = decoder.decode_block(block_id)?;
    files::write(output, &block)?;
    println!("decoded block {block_id}: {} bytes", block.len());
    Ok(())
}

/// Decodes only the blocks intersecting `[offset, offset + length)` and writes that range.
pub fn read_range(input: &str, offset: u64, length: u64, output: &str) -> Result<()> {
    let end = offset.checked_add(length).context("range overflow")?;
    let mut decoder = AceIndexedDecoder::open(files::open(input)?, DecodeLimits::default())?;
    let (bytes, metrics) = decoder.read_range_with_metrics(offset..end)?;
    files::write(output, &bytes)?;
    println!(
        "decoded logical range [{offset}, {end}): {} bytes physical={} blocks={} physical/logical={:.3}",
        bytes.len(),
        metrics.physical_bytes_read,
        metrics.blocks_touched,
        metrics.physical_to_logical_ratio()
    );
    Ok(())
}
