//! `inspect`: container and per-block metadata without decoding payloads.

use std::io::Cursor;

use ace_codecs::{numeric_inspect, ts1_inspect};
use ace_core::{CodecId, DecodeLimits, EntropyCodecId};
use ace_format::{AceReader, FILE_FLAG_HAS_INDEX};
use anyhow::Result;

use crate::files;

/// Prints the file header, optionally every block, and numeric (NUM1) / TS1 telemetry.
///
/// For every `Numeric` block the NUM1 header is parsed with `numeric_inspect` and the lane
/// width, sub-mode, packed bit width and bits-per-value are shown; every `TimeSeries` block
/// shows its TS1 mode, lane width, value count and bits per value (`ts1_inspect`). Summary
/// lines report how many blocks and bytes each specialized codec stored even without
/// `--blocks`.
pub fn inspect(input: &str, show_blocks: bool) -> Result<()> {
    let data = files::read(input)?;
    let mut reader = AceReader::new(Cursor::new(&data), DecodeLimits::default());
    let file = reader.read_file_header()?;
    println!(
        "ACE format 1.{} flags=0x{:04x} original={} block_size={} blocks={} indexed={}",
        file.minor_version,
        file.flags,
        file.original_size,
        file.default_block_size,
        file.block_count,
        file.flags & FILE_FLAG_HAS_INDEX != 0
    );
    let (mut numeric_blocks, mut numeric_original, mut numeric_payload) = (0u64, 0u64, 0u64);
    let (mut ts1_blocks, mut ts1_original, mut ts1_payload) = (0u64, 0u64, 0u64);
    for _ in 0..file.block_count {
        let (header, metadata, payload) = reader.read_block()?;
        let numeric = if matches!(header.codec, CodecId::Numeric)
            && matches!(header.entropy, EntropyCodecId::None)
        {
            numeric_inspect(&payload).ok()
        } else {
            None
        };
        let ts1 = if matches!(header.codec, CodecId::TimeSeries) {
            ts1_inspect(&payload).ok()
        } else {
            None
        };
        if ts1.is_some() {
            ts1_blocks += 1;
            ts1_original += header.original_size as u64;
            ts1_payload += payload.len() as u64;
        }
        if numeric.is_some() {
            numeric_blocks += 1;
            numeric_original += header.original_size as u64;
            numeric_payload += payload.len() as u64;
        }
        if show_blocks {
            println!("block={} original={} codec={:?} entropy={:?} transforms={:?} metadata={} payload={}", header.block_id, header.original_size, header.codec, header.entropy, header.transforms, metadata.len(), payload.len());
            if let Some(info) = numeric {
                println!(
                    "  numeric width={:?} mode={} bit_width={} values={} tail={} bits_per_value={:.2}",
                    info.width, info.mode.label(), info.bit_width, info.value_count, info.tail_bytes, info.bits_per_value()
                );
            }
            if let Some(info) = ts1 {
                println!(
                    "  ts1 mode={} lane={} values={} tail={} window_reuse={} bits_per_value={:.2}",
                    info.mode.label(),
                    info.lane_bytes,
                    info.value_count,
                    info.tail_len,
                    info.flags & ace_codecs::FLAG_WINDOW_REUSE != 0,
                    info.bits_per_value()
                );
            }
        }
    }
    if numeric_blocks > 0 {
        println!(
            "numeric blocks={} original={} payload={} ratio={:.2}",
            numeric_blocks,
            numeric_original,
            numeric_payload,
            numeric_original as f64 / numeric_payload.max(1) as f64
        );
    }
    if ts1_blocks > 0 {
        println!(
            "ts1 blocks={} original={} payload={} ratio={:.2}",
            ts1_blocks,
            ts1_original,
            ts1_payload,
            ts1_original as f64 / ts1_payload.max(1) as f64
        );
    }
    Ok(())
}
