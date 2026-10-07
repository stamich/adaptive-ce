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
    pub fn write_file_header(&mut self, header: &FileHeader) -> AceResult<()> {
        self.write_all(&encode_file_header(header))
    }

    /// Writes one block (header, entropy metadata, payload) and records its index entry.
    pub fn write_block(
        &mut self,
        header: &BlockHeader,
        metadata: &[u8],
        payload: &[u8],
    ) -> AceResult<()> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode_index, decode_trailer, AceReader, TRAILER_SIZE};
    use ace_core::{CodecId, DecodeLimits, EntropyCodecId};

    /// Builds a RAW block header for `payload`.
    fn raw_header(block_id: u64, payload: &[u8]) -> BlockHeader {
        BlockHeader {
            block_id,
            original_size: payload.len() as u32,
            encoded_size: payload.len() as u32,
            metadata_size: 0,
            codec: CodecId::Raw,
            entropy: EntropyCodecId::None,
            transforms: Vec::new(),
            dictionary: None,
            flags: 0,
            payload_crc32c: checksum(payload),
        }
    }

    /// The writer records correct offsets/spans and a trailer pointing at a valid index.
    #[test]
    fn writer_builds_consistent_index_and_trailer() {
        let blocks: [&[u8]; 3] = [b"alpha", b"beta-beta", b"g"];
        let mut writer = AceWriter::new(Vec::new());
        writer
            .write_file_header(&FileHeader {
                minor_version: 3,
                flags: crate::FILE_FLAG_HAS_INDEX,
                default_block_size: 16,
                original_size: 15,
                block_count: 3,
            })
            .unwrap();
        for (id, payload) in blocks.iter().enumerate() {
            writer
                .write_block(&raw_header(id as u64, payload), &[], payload)
                .unwrap();
        }
        let (bytes, total) = writer.finish(true).unwrap();
        assert_eq!(total as usize, bytes.len());

        let trailer = decode_trailer(&bytes[bytes.len() - TRAILER_SIZE..]).unwrap();
        let start = trailer.index_offset as usize;
        let index = decode_index(
            &bytes[start..start + trailer.index_size as usize],
            &DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(
            index
                .entries
                .iter()
                .map(|e| e.original_offset)
                .collect::<Vec<_>>(),
            vec![0, 5, 14]
        );
        let mut reader = AceReader::new(std::io::Cursor::new(&bytes), DecodeLimits::default());
        reader.read_file_header().unwrap();
        for (entry, payload) in index.entries.iter().zip(blocks) {
            let (header, _, read_payload) = reader.read_block().unwrap();
            assert_eq!(header.block_id, entry.block_id);
            assert_eq!(read_payload, payload);
        }
    }

    /// Without an index the container ends right after the last block.
    #[test]
    fn writer_without_index_has_no_trailer() {
        let mut writer = AceWriter::new(Vec::new());
        writer
            .write_block(&raw_header(0, b"xyz"), &[], b"xyz")
            .unwrap();
        let written = writer.bytes_written();
        let (bytes, total) = writer.finish(false).unwrap();
        assert_eq!((bytes.len() as u64, total), (written, written));
    }
}
