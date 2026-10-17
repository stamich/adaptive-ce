//! Sequential container writer that also builds the block index and trailer.
//!
//! The engine (in-memory) and the streaming encoder (bounded memory) both serialize containers
//! through this type, so offsets, span limits and the AIDX/ACET tail exist exactly once.

use std::io::Write;

use ace_core::{AceError, AceResult};

use crate::{
    checksum, encode_block_header, encode_file_header, encode_index, encode_trailer, BlockHeader,
    BlockIndex, BlockIndexEntry, FileHeader, FileTrailer,
};

/// Streaming ACE writer: file header, blocks, then (optionally) block index and trailer.
pub struct AceWriter<W: Write> {
    /// Underlying byte sink.
    inner: W,
    /// Bytes written so far (= file offset of the next byte).
    bytes_written: u64,
    /// Logical offset of the next block's first reconstructed byte.
    original_offset: u64,
    /// Index entries collected for every written block.
    index: BlockIndex,
}

/// Inherent methods of [`AceWriter`].
impl<W: Write> AceWriter<W> {
    /// Creates a writer over an arbitrary byte sink.
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            bytes_written: 0,
            original_offset: 0,
            index: BlockIndex::default(),
        }
    }

    /// Writes the global file header.
    ///
    /// # Errors
    ///
    /// [`AceError::Io`] when the sink fails.
    pub fn write_file_header(&mut self, header: &FileHeader) -> AceResult<()> {
        self.write_all(&encode_file_header(header))
    }

    /// Writes one block (header, entropy metadata, payload) and records its index entry.
    ///
    /// # Errors
    ///
    /// [`AceError::Malformed`] for more than 255 transforms (the header's count byte),
    /// [`AceError::ResourceLimitExceeded`] when the serialized block span does not fit in
    /// `u32`, [`AceError::Malformed`] when the logical output offset overflows, and
    /// [`AceError::Io`] when the sink fails.
    pub fn write_block(
        &mut self,
        header: &BlockHeader,
        metadata: &[u8],
        payload: &[u8],
    ) -> AceResult<()> {
        if header.transforms.len() > usize::from(u8::MAX) {
            return Err(AceError::Malformed("more than 255 transforms in one block"));
        }
        let file_offset = self.bytes_written;
        self.write_all(&encode_block_header(header))?;
        self.write_all(metadata)?;
        self.write_all(payload)?;
        let span = u32::try_from(self.bytes_written - file_offset)
            .map_err(|_| AceError::ResourceLimitExceeded("serialized block span"))?;
        self.index.entries.push(BlockIndexEntry {
            block_id: header.block_id,
            original_offset: self.original_offset,
            original_size: header.original_size,
            file_offset,
            encoded_span: span,
            flags: 0,
        });
        self.original_offset = self
            .original_offset
            .checked_add(header.original_size as u64)
            .ok_or(AceError::Malformed("logical output offset overflow"))?;
        Ok(())
    }

    /// Number of bytes written so far.
    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    /// Appends the block index and trailer when `write_index` is set, flushes and returns the
    /// sink together with the total container size.
    ///
    /// # Errors
    ///
    /// [`AceError::Io`] when the sink fails.
    pub fn finish(mut self, write_index: bool) -> AceResult<(W, u64)> {
        if write_index {
            let index_bytes = encode_index(&self.index);
            let trailer = FileTrailer {
                index_offset: self.bytes_written,
                index_size: index_bytes.len() as u64,
                index_crc32c: checksum(&index_bytes),
            };
            self.write_all(&index_bytes)?;
            self.write_all(&encode_trailer(trailer))?;
        }
        self.inner.flush()?;
        Ok((self.inner, self.bytes_written))
    }

    /// Returns the wrapped sink without writing an index.
    pub fn into_inner(self) -> W {
        self.inner
    }

    /// Writes bytes and advances the file offset.
    fn write_all(&mut self, bytes: &[u8]) -> AceResult<()> {
        self.inner.write_all(bytes)?;
        self.bytes_written += bytes.len() as u64;
        Ok(())
    }
}
