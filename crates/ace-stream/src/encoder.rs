//! Bounded-memory streaming compression.

use std::io::{Cursor, Read, Seek, SeekFrom, Write};

use ace_core::{AceConfig, AceError, AceResult, BlockSizePolicy, CodecId, DecodeLimits};
use ace_engine::AceEngine;
use ace_format::{
    encode_file_header, minimal_minor_version, AceReader, AceWriter, FileHeader, SerializedBlock,
    FILE_FLAG_HAS_INDEX, FORMAT_MINOR_BASE,
};

use crate::{StreamLimits, StreamingStats};

/// Compresses a reader into one indexed ACE stream using bounded block memory.
///
/// `original_size` is required because the fixed file header precedes the first block. The
/// reader must yield exactly that many bytes. Output is byte-identical to
/// `AceEngine::compress` with the same configuration and a single worker thread.
///
/// The sink must be seekable: the header is written first as Format 1.3 and rewritten in place
/// as Format 1.4 only if a block selected the TS1 codec (the minimal-version rule), which keeps
/// streaming and in-memory output identical without buffering the whole stream.
pub fn compress_reader_known_size<R: Read, W: Write + Seek>(
    mut reader: R,
    writer: W,
    original_size: u64,
    config: AceConfig,
    limits: StreamLimits,
) -> AceResult<StreamingStats> {
    let block_count = validate_stream(original_size, &config, &limits)?;
    let mut file_header = FileHeader {
        minor_version: FORMAT_MINOR_BASE,
        flags: FILE_FLAG_HAS_INDEX,
        default_block_size: config.block_size as u32,
        original_size,
        block_count,
    };
    let mut writer = writer;
    // Header position in the sink (non-zero when appending to an existing stream).
    let start = writer.stream_position()?;
    let mut ace = AceWriter::new(writer);
    ace.write_file_header(&file_header)?;
    let mut codecs_used = Vec::new();

    let engine = single_block_engine(&config)?;
    let mut buffer = vec![0u8; config.block_size];
    let mut stats = StreamingStats::default();
    for block_id in 0..block_count {
        let remaining = (original_size - stats.input_bytes).min(config.block_size as u64) as usize;
        let source = &mut buffer[..remaining];
        read_exactly(&mut reader, source)?;
        let (mut header, metadata, payload) = encode_single_block(&engine, source)?;
        header.block_id = block_id;
        if !codecs_used.contains(&header.codec) {
            codecs_used.push(header.codec);
        }
        ace.write_block(&header, &metadata, &payload)?;
        stats.peak_source_buffer_bytes = stats.peak_source_buffer_bytes.max(remaining);
        stats.input_bytes += remaining as u64;
        stats.blocks += 1;
    }
    if reader.read(&mut [0u8; 1])? != 0 {
        return Err(AceError::Malformed(
            "stream contains bytes beyond declared original size",
        ));
    }
    let (mut sink, output_bytes) = ace.finish(true)?;
    let minor_version = minimal_minor_version(codecs_used.iter().copied());
    if minor_version != file_header.minor_version {
        // Rewrite only the fixed 32-byte header; its CRC covers the version byte.
        file_header.minor_version = minor_version;
        let end = sink.stream_position()?;
        sink.seek(SeekFrom::Start(start))?;
        sink.write_all(&encode_file_header(&file_header))?;
        sink.seek(SeekFrom::Start(end))?;
        sink.flush()?;
    }
    stats.format_1_4 = codecs_used.contains(&CodecId::TimeSeries);
    stats.output_bytes = output_bytes;
    Ok(stats)
}

/// Validates sizes and configuration; returns the number of blocks the stream will contain.
fn validate_stream(
    original_size: u64,
    config: &AceConfig,
    limits: &StreamLimits,
) -> AceResult<u64> {
    if original_size > limits.max_input_bytes {
        return Err(AceError::ResourceLimitExceeded("stream input size"));
    }
    if matches!(config.block_size_policy, BlockSizePolicy::Auto) {
        return Err(AceError::InvalidConfig("streaming requires fixed block size; auto policy requires a seekable/presampled source"));
    }
    if config.block_size == 0 || config.block_size > u32::MAX as usize {
        return Err(AceError::InvalidConfig(
            "stream block size must fit u32 and be non-zero",
        ));
    }
    let block_count = original_size.div_ceil(config.block_size as u64);
    if block_count > limits.max_blocks {
        return Err(AceError::ResourceLimitExceeded("stream block count"));
    }
    if config.block_size.saturating_mul(3) > limits.max_in_flight_bytes {
        return Err(AceError::ResourceLimitExceeded(
            "stream in-flight memory budget",
        ));
    }
    Ok(block_count)
}

/// Engine used per block: same planner/codec semantics, single thread, no index.
fn single_block_engine(config: &AceConfig) -> AceResult<AceEngine> {
    let mut inner = config.clone();
    inner.write_index = false;
    inner.threads = 1;
    AceEngine::new(inner)
}

/// Encodes one block with the regular engine and extracts its serialized parts.
///
/// Reusing the engine keeps planner/codec behaviour identical to in-memory compression; the
/// temporary one-block container is discarded immediately, so only one block is resident.
fn encode_single_block(engine: &AceEngine, source: &[u8]) -> AceResult<SerializedBlock> {
    let encoded = engine.compress(source)?;
    let mut parser = AceReader::new(Cursor::new(encoded), DecodeLimits::default());
    if parser.read_file_header()?.block_count != 1 {
        return Err(AceError::Malformed(
            "internal streaming block container has unexpected block count",
        ));
    }
    parser.read_block()
}

/// Fills `buffer` completely or fails if the reader ends early.
fn read_exactly<R: Read>(reader: &mut R, buffer: &mut [u8]) -> AceResult<()> {
    let mut filled = 0usize;
    while filled < buffer.len() {
        let read = reader.read(&mut buffer[filled..])?;
        if read == 0 {
            return Err(AceError::Malformed(
                "stream ended before declared original size",
            ));
        }
        filled += read;
    }
    Ok(())
}
