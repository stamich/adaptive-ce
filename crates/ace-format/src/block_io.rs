//! Reading one serialized block (header, descriptors, metadata, payload) under decode limits.
//!
//! Both the sequential [`crate::AceReader`] and the seekable index reader (`ace-index`) use
//! this function, so limit checks and parsing exist exactly once.

// Decoder hardening (ACE 0.5.1): arithmetic on values read from a file is checked and every
// narrowing cast is explicit (`docs/DECODER-AUDIT.md`).
#![warn(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)]

use std::io::{Cursor, Read};

use ace_core::{AceError, AceResult, DecodeLimits};

use crate::{
    block_descriptor_size, decode_block_header, encode_block_header, BlockHeader, BLOCK_HEADER_SIZE,
};

/// One serialized block: parsed header, entropy metadata and encoded payload.
pub type SerializedBlock = (BlockHeader, Vec<u8>, Vec<u8>);

/// Byte offset of the transform-count field inside the fixed block header.
const TRANSFORM_COUNT_OFFSET: usize = 22;

/// Reads one block from `reader` positioned at its first header byte.
///
/// Every size declared by the header is checked against `limits` before the corresponding
/// buffer is allocated, so hostile headers cannot trigger large allocations.
///
/// # Errors
///
/// [`AceError::ResourceLimitExceeded`] when a declared size exceeds `limits`,
/// [`AceError::Malformed`] / unsupported-identifier errors from the header, and
/// [`AceError::Io`] (including an unexpected end of file).
pub fn read_serialized_block<R: Read>(
    reader: &mut R,
    minor_version: u8,
    limits: &DecodeLimits,
) -> AceResult<SerializedBlock> {
    let (header, _header_len) = read_block_header(reader, minor_version, limits)?;
    let mut metadata = vec![0u8; header.metadata_size as usize];
    reader.read_exact(&mut metadata)?;
    let mut payload = vec![0u8; header.encoded_size as usize];
    reader.read_exact(&mut payload)?;
    Ok((header, metadata, payload))
}

/// Reads only the header of one block (fixed part and descriptors) from `reader` positioned at
/// its first byte, and returns it with the number of bytes consumed.
///
/// The declared sizes are checked against `limits` exactly as [`read_serialized_block`] checks
/// them, before anything is allocated for the metadata or the payload. A range read of a
/// segmented block uses this to learn the payload size without reading the payload.
///
/// # Errors
///
/// The conditions of [`read_serialized_block`] up to the end of the header.
pub fn read_block_header<R: Read>(
    reader: &mut R,
    minor_version: u8,
    limits: &DecodeLimits,
) -> AceResult<(BlockHeader, usize)> {
    let mut fixed = [0u8; BLOCK_HEADER_SIZE];
    reader.read_exact(&mut fixed)?;
    if fixed[TRANSFORM_COUNT_OFFSET] as usize > limits.max_transforms {
        return Err(AceError::ResourceLimitExceeded("transform count"));
    }
    let mut descriptors = vec![0u8; block_descriptor_size(&fixed, minor_version)];
    reader.read_exact(&mut descriptors)?;
    let header = decode_block_header(&fixed, &descriptors, minor_version)?;
    if header.original_size as usize > limits.max_block_size {
        return Err(AceError::ResourceLimitExceeded("block output size"));
    }
    let total = (header.metadata_size as usize)
        .checked_add(header.encoded_size as usize)
        .ok_or(AceError::Malformed("encoded block size overflow"))?;
    if total > limits.max_encoded_block_size {
        return Err(AceError::ResourceLimitExceeded("encoded block size"));
    }
    let header_len = BLOCK_HEADER_SIZE.saturating_add(descriptors.len());
    Ok((header, header_len))
}

/// Serializes one block exactly as [`crate::AceWriter::write_block`] writes it to a file: the
/// fixed header with its descriptors and CRC-32C, the entropy metadata, the payload. Segmented
/// and columnar blocks (Format 1.6) store their inner blocks in this form.
pub fn serialize_block(header: &BlockHeader, metadata: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut bytes = encode_block_header(header);
    bytes.extend_from_slice(metadata);
    bytes.extend_from_slice(payload);
    bytes
}

/// Parses one inner block of a segmented or columnar block (Format 1.6) from `bytes`.
///
/// `bytes` must contain exactly one serialized block. The block must carry `block_id`
/// (its position in the outer block) and `original_size` bytes of output, and must not be a
/// container itself: nesting is one level deep.
///
/// # Errors
/// The conditions of [`read_serialized_block`], and [`AceError::Malformed`] for trailing bytes, a
/// wrong id or size, or an inner block of a container codec.
pub fn parse_inner_block(
    bytes: &[u8],
    block_id: u64,
    original_size: u32,
    minor_version: u8,
    limits: &DecodeLimits,
) -> AceResult<SerializedBlock> {
    let mut cursor = Cursor::new(bytes);
    let block = read_serialized_block(&mut cursor, minor_version, limits)?;
    if usize::try_from(cursor.position()).ok() != Some(bytes.len()) {
        return Err(AceError::Malformed("inner block length mismatch"));
    }
    if block.0.block_id != block_id {
        return Err(AceError::Malformed("inner block id mismatch"));
    }
    if block.0.original_size != original_size {
        return Err(AceError::Malformed("inner block size mismatch"));
    }
    if block.0.codec.is_container() {
        return Err(AceError::Malformed("nested container block"));
    }
    Ok(block)
}
