//! Bounded-memory streaming adapters for ACE 0.3.
//!
//! Format 1.2 retains a fixed file header containing original size and block count, therefore
//! the non-seekable encoder accepts the expected source size up front.  This keeps the wire
//! format deterministic while allowing the payload to be processed one independent block at a
//! time without holding the complete input in memory.

use ace_core::{AceConfig, AceError, AceResult, DecodeLimits};
use ace_engine::AceEngine;
use ace_format::{
    checksum, encode_block_header, encode_file_header, encode_index, encode_trailer, AceReader,
    BlockIndex, BlockIndexEntry, FileHeader, FileTrailer, FILE_FLAG_HAS_INDEX, FILE_HEADER_SIZE,
};
use std::io::{Cursor, Read, Write};

/// Resource limits applied to a bounded streaming operation.
#[derive(Debug, Clone, Copy)]
pub struct StreamLimits {
    /// Maximum declared input size accepted by the encoder.
    pub max_input_bytes: u64,
    /// Maximum number of independent blocks accepted by one stream.
    pub max_blocks: u64,
    /// Maximum bytes retained for one input block plus local encoded representation.
    pub max_in_flight_bytes: usize,
}

impl Default for StreamLimits {
    /// Returns conservative limits suitable for normal CLI and benchmark use.
    fn default() -> Self {
        Self {
            max_input_bytes: 1 << 40,
            max_blocks: 16_777_216,
            max_in_flight_bytes: 64 * 1024 * 1024,
        }
    }
}

/// Aggregate telemetry produced by the bounded streaming encoder.
#[derive(Debug, Clone, Copy, Default)]
pub struct StreamingStats {
    /// Source bytes consumed from the reader.
    pub input_bytes: u64,
    /// ACE bytes written to the sink.
    pub output_bytes: u64,
    /// Number of blocks processed.
    pub blocks: u64,
    /// Maximum source block bytes resident at one time.
    pub peak_source_buffer_bytes: usize,
}

/// Compresses a reader into one ACE 1.2 stream using bounded block memory.
///
/// `original_size` is required because ACE's deterministic fixed header is written before the
/// first block.  The function verifies that the reader yields exactly that many bytes.
pub fn compress_reader_known_size<R: Read, W: Write>(
    mut reader: R,
    mut writer: W,
    original_size: u64,
    config: AceConfig,
    limits: StreamLimits,
) -> AceResult<StreamingStats> {
    if original_size > limits.max_input_bytes {
        return Err(AceError::ResourceLimitExceeded("stream input size"));
    }
    if config.block_size == 0 || config.block_size > u32::MAX as usize {
        return Err(AceError::InvalidConfig(
            "stream block size must fit u32 and be non-zero",
        ));
    }
    let block_count = if original_size == 0 {
        0
    } else {
        original_size
            .checked_add(config.block_size as u64 - 1)
            .ok_or(AceError::Malformed(
                "stream block-count calculation overflow",
            ))?
            / config.block_size as u64
    };
    if block_count > limits.max_blocks {
        return Err(AceError::ResourceLimitExceeded("stream block count"));
    }
    if config.block_size.saturating_mul(3) > limits.max_in_flight_bytes {
        return Err(AceError::ResourceLimitExceeded(
            "stream in-flight memory budget",
        ));
    }

    let header = FileHeader {
        minor_version: 2,
        flags: FILE_FLAG_HAS_INDEX,
        default_block_size: config.block_size as u32,
        original_size,
        block_count,
    };
    writer.write_all(&encode_file_header(&header))?;
    let mut bytes_written = FILE_HEADER_SIZE as u64;
    let mut index = BlockIndex::default();
    let mut original_offset = 0u64;
    let mut stats = StreamingStats::default();

    let mut inner_config = config.clone();
    inner_config.write_index = false;
    inner_config.threads = 1;
    let engine = AceEngine::new(inner_config)?;
    let mut buffer = vec![0u8; config.block_size];

    for block_id in 0..block_count {
        let remaining = (original_size - stats.input_bytes).min(config.block_size as u64) as usize;
        let mut filled = 0usize;
        while filled < remaining {
            let n = reader.read(&mut buffer[filled..remaining])?;
            if n == 0 {
                return Err(AceError::Malformed(
                    "stream ended before declared original size",
                ));
            }
            filled += n;
        }
        let source = &buffer[..remaining];
        stats.peak_source_buffer_bytes = stats.peak_source_buffer_bytes.max(source.len());

        // Reuse the regular engine to keep planner/codec semantics identical; only one block is
        // resident and the temporary one-block container is discarded before reading the next.
        let encoded = engine.compress(source)?;
        let mut parser = AceReader::new(Cursor::new(encoded), DecodeLimits::default());
        let one = parser.read_file_header()?;
        if one.block_count != 1 {
            return Err(AceError::Malformed(
                "internal streaming block container has unexpected block count",
            ));
        }
        let (mut block_header, metadata, payload) = parser.read_block()?;
        block_header.block_id = block_id;

        let file_offset = bytes_written;
        let header_bytes = encode_block_header(&block_header);
        writer.write_all(&header_bytes)?;
        writer.write_all(&metadata)?;
        writer.write_all(&payload)?;
        let span = header_bytes
            .len()
            .saturating_add(metadata.len())
            .saturating_add(payload.len());
        let span_u32 = u32::try_from(span)
            .map_err(|_| AceError::ResourceLimitExceeded("stream encoded block span"))?;
        index.entries.push(BlockIndexEntry {
            block_id,
            original_offset,
            original_size: remaining as u32,
            file_offset,
            encoded_span: span_u32,
            flags: 0,
        });
        bytes_written = bytes_written
            .checked_add(span as u64)
            .ok_or(AceError::Malformed("stream output size overflow"))?;
        original_offset += remaining as u64;
        stats.input_bytes += remaining as u64;
        stats.blocks += 1;
    }

    let mut extra = [0u8; 1];
    if reader.read(&mut extra)? != 0 {
        return Err(AceError::Malformed(
            "stream contains bytes beyond declared original size",
        ));
    }
    let index_bytes = encode_index(&index);
    let index_offset = bytes_written;
    writer.write_all(&index_bytes)?;
    bytes_written += index_bytes.len() as u64;
    let trailer = FileTrailer {
        index_offset,
        index_size: index_bytes.len() as u64,
        index_crc32c: checksum(&index_bytes),
    };
    let trailer_bytes = encode_trailer(trailer);
    writer.write_all(&trailer_bytes)?;
    bytes_written += trailer_bytes.len() as u64;
    writer.flush()?;
    stats.output_bytes = bytes_written;
    Ok(stats)
}

/// Sequentially decompresses an ACE 1.0/1.1/1.2 stream into a writer with explicit limits.
pub fn decompress_stream<R: Read, W: Write>(
    reader: R,
    writer: W,
    limits: DecodeLimits,
) -> AceResult<()> {
    AceEngine::default_engine()
        .with_decode_limits(limits)
        .decompress_from(reader, writer)
}
