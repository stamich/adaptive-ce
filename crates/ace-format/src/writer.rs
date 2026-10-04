use std::io::Write;
use ace_core::AceResult;
use crate::{encode_block_header, encode_file_header, BlockHeader, FileHeader};

/// Streaming ACE block writer used when index construction is managed by the caller.
pub struct AceWriter<W: Write> { inner: W }

impl<W: Write> AceWriter<W> {
    /// Creates a writer over an arbitrary byte sink.
    pub fn new(inner: W) -> Self { Self { inner } }
    /// Writes the global file header.
    pub fn write_file_header(&mut self, header: &FileHeader) -> AceResult<()> { self.inner.write_all(&encode_file_header(header))?; Ok(()) }
    /// Writes one block header, entropy metadata and encoded payload.
    pub fn write_block(&mut self, header: &BlockHeader, metadata: &[u8], payload: &[u8]) -> AceResult<()> {
        self.inner.write_all(&encode_block_header(header))?;
        self.inner.write_all(metadata)?;
        self.inner.write_all(payload)?;
        Ok(())
    }
    /// Returns the wrapped sink after all bytes have been written.
    pub fn into_inner(self) -> W { self.inner }
}
