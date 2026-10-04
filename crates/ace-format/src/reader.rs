use std::io::Read;
use ace_core::{AceError, AceResult, DecodeLimits};
use crate::{
    block_descriptor_size, decode_block_header, decode_file_header, BlockHeader, FileHeader,
    BLOCK_HEADER_SIZE, FILE_HEADER_SIZE,
};

/// Streaming ACE reader that validates structural limits before allocating block buffers.
pub struct AceReader<R: Read> {
    inner: R,
    limits: DecodeLimits,
    minor_version: u8,
}

impl<R: Read> AceReader<R> {
    /// Creates a reader with explicit decoder resource limits.
    pub fn new(inner: R, limits: DecodeLimits) -> Self { Self { inner, limits, minor_version: 0 } }

    /// Reads and validates the fixed file header and records its minor format version.
    pub fn read_file_header(&mut self) -> AceResult<FileHeader> {
        let mut bytes = [0u8; FILE_HEADER_SIZE];
        self.inner.read_exact(&mut bytes)?;
        let header = decode_file_header(&bytes)?;
        if header.original_size > self.limits.max_output_size { return Err(AceError::ResourceLimitExceeded("file output size")); }
        self.minor_version = header.minor_version;
        Ok(header)
    }

    /// Reads one block and returns header, entropy metadata and encoded payload.
    pub fn read_block(&mut self) -> AceResult<(BlockHeader, Vec<u8>, Vec<u8>)> {
        let mut fixed = [0u8; BLOCK_HEADER_SIZE];
        self.inner.read_exact(&mut fixed)?;
        let descriptor_size = block_descriptor_size(&fixed, self.minor_version);
        if fixed[22] as usize > self.limits.max_transforms { return Err(AceError::ResourceLimitExceeded("transform count")); }
        let mut descriptors = vec![0u8; descriptor_size];
        self.inner.read_exact(&mut descriptors)?;
        let header = decode_block_header(&fixed, &descriptors, self.minor_version)?;
        if header.original_size as usize > self.limits.max_block_size { return Err(AceError::ResourceLimitExceeded("block output size")); }
        let total = (header.metadata_size as usize).checked_add(header.encoded_size as usize).ok_or(AceError::Malformed("encoded block size overflow"))?;
        if total > self.limits.max_encoded_block_size { return Err(AceError::ResourceLimitExceeded("encoded block size")); }
        let mut metadata = vec![0u8; header.metadata_size as usize];
        self.inner.read_exact(&mut metadata)?;
        let mut payload = vec![0u8; header.encoded_size as usize];
        self.inner.read_exact(&mut payload)?;
        Ok((header, metadata, payload))
    }

    /// Returns the wrapped reader after parsing is complete.
    pub fn into_inner(self) -> R { self.inner }
}
