//! Reading one serialized block (header, descriptors, metadata, payload) under decode limits.
//!
//! Both the sequential [`crate::AceReader`] and the seekable index reader (`ace-index`) use
//! this function, so limit checks and parsing exist exactly once.

use std::io::Read;

use ace_core::{AceError, AceResult, DecodeLimits};

use crate::{block_descriptor_size, decode_block_header, BlockHeader, BLOCK_HEADER_SIZE};

/// One serialized block: parsed header, entropy metadata and encoded payload.
pub type SerializedBlock = (BlockHeader, Vec<u8>, Vec<u8>);

/// Byte offset of the transform-count field inside the fixed block header.
const TRANSFORM_COUNT_OFFSET: usize = 22;

/// Reads one block from `reader` positioned at its first header byte.
///
/// Every size declared by the header is checked against `limits` before the corresponding
/// buffer is allocated, so hostile headers cannot trigger large allocations.
pub fn read_serialized_block<R: Read>(
    reader: &mut R,
    minor_version: u8,
    limits: &DecodeLimits,
) -> AceResult<SerializedBlock> {
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
    let mut metadata = vec![0u8; header.metadata_size as usize];
    reader.read_exact(&mut metadata)?;
    let mut payload = vec![0u8; header.encoded_size as usize];
    reader.read_exact(&mut payload)?;
    Ok((header, metadata, payload))
}
