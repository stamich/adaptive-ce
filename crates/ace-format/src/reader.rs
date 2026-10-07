//! Sequential container reader.

use std::io::Read;

use ace_core::{AceError, AceResult, DecodeLimits};

use crate::{
    decode_file_header, read_serialized_block, FileHeader, SerializedBlock, FILE_HEADER_SIZE,
};

/// Streaming ACE reader that validates structural limits before allocating block buffers.
pub struct AceReader<R: Read> {
    /// Underlying byte source.
    inner: R,
    /// Resource limits applied to every header.
    limits: DecodeLimits,
    /// Minor version of the file header (selects the block-descriptor layout).
    minor_version: u8,
}

/// Inherent methods of [`AceReader`].
impl<R: Read> AceReader<R> {
    /// Creates a reader with explicit decoder resource limits.
    pub fn new(inner: R, limits: DecodeLimits) -> Self {
        Self {
            inner,
            limits,
            minor_version: 0,
        }
    }

    /// Reads and validates the fixed file header and records its minor format version.
    pub fn read_file_header(&mut self) -> AceResult<FileHeader> {
        let mut bytes = [0u8; FILE_HEADER_SIZE];
        self.inner.read_exact(&mut bytes)?;
        let header = decode_file_header(&bytes)?;
        if header.original_size > self.limits.max_output_size {
            return Err(AceError::ResourceLimitExceeded("file output size"));
        }
        self.minor_version = header.minor_version;
        Ok(header)
    }

    /// Reads one block and returns header, entropy metadata and encoded payload.
    pub fn read_block(&mut self) -> AceResult<SerializedBlock> {
        read_serialized_block(&mut self.inner, self.minor_version, &self.limits)
    }

    /// Returns the wrapped reader after parsing is complete.
    pub fn into_inner(self) -> R {
        self.inner
    }
}
