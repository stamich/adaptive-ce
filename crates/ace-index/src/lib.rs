//! Seekable block-index loading and indexed raw-block access for ACE formats 1.2/1.3.

use ace_core::{AceError, AceResult, DecodeLimits};
use ace_format::{
    block_descriptor_size, checksum, decode_block_header, decode_file_header, decode_index,
    decode_trailer, BlockHeader, BlockIndex, FileHeader, BLOCK_HEADER_SIZE, FILE_FLAG_HAS_INDEX,
    FILE_HEADER_SIZE, TRAILER_SIZE,
};
use std::io::{Read, Seek, SeekFrom};

/// Seekable parser that loads the ACE block index once and can fetch individual encoded blocks.
pub struct AceIndexReader<R: Read + Seek> {
    reader: R,
    /// Parsed global file header.
    pub file_header: FileHeader,
    /// Validated sorted block index.
    pub index: BlockIndex,
    limits: DecodeLimits,
}

impl<R: Read + Seek> AceIndexReader<R> {
    /// Opens a seekable ACE file, loads its format-1.1/1.2 trailer and validates the block index.
    pub fn open(mut reader: R, limits: DecodeLimits) -> AceResult<Self> {
        reader.seek(SeekFrom::Start(0))?;
        let mut header_bytes = [0u8; FILE_HEADER_SIZE];
        reader.read_exact(&mut header_bytes)?;
        let file_header = decode_file_header(&header_bytes)?;
        if file_header.flags & FILE_FLAG_HAS_INDEX == 0 {
            return Err(AceError::Malformed(
                "ACE file does not contain a block index",
            ));
        }
        if file_header.original_size > limits.max_output_size {
            return Err(AceError::ResourceLimitExceeded("file output size"));
        }
        let file_len = reader.seek(SeekFrom::End(0))?;
        if file_len < (FILE_HEADER_SIZE + TRAILER_SIZE) as u64 {
            return Err(AceError::Malformed(
                "file is too small for indexed ACE format",
            ));
        }
        reader.seek(SeekFrom::End(-(TRAILER_SIZE as i64)))?;
        let mut trailer_bytes = [0u8; TRAILER_SIZE];
        reader.read_exact(&mut trailer_bytes)?;
        let trailer = decode_trailer(&trailer_bytes)?;
        let index_end = trailer
            .index_offset
            .checked_add(trailer.index_size)
            .ok_or(AceError::Malformed("index offset overflow"))?;
        if index_end > file_len.saturating_sub(TRAILER_SIZE as u64) {
            return Err(AceError::Malformed("block index points outside file"));
        }
        if trailer.index_size > usize::MAX as u64 {
            return Err(AceError::ResourceLimitExceeded("index size"));
        }
        reader.seek(SeekFrom::Start(trailer.index_offset))?;
        let mut index_bytes = vec![0u8; trailer.index_size as usize];
        reader.read_exact(&mut index_bytes)?;
        if checksum(&index_bytes) != trailer.index_crc32c {
            return Err(AceError::IndexChecksumMismatch);
        }
        let index = decode_index(&index_bytes, &limits)?;
        if index.entries.len() as u64 != file_header.block_count {
            return Err(AceError::Malformed(
                "block count differs from index entry count",
            ));
        }
        for entry in &index.entries {
            let end = entry
                .file_offset
                .checked_add(entry.encoded_span as u64)
                .ok_or(AceError::Malformed("indexed block span overflow"))?;
            if entry.file_offset < FILE_HEADER_SIZE as u64 || end > trailer.index_offset {
                return Err(AceError::Malformed(
                    "indexed block points outside block region",
                ));
            }
        }
        Ok(Self {
            reader,
            file_header,
            index,
            limits,
        })
    }

    /// Reads one indexed encoded block without decoding its compression pipeline.
    pub fn read_encoded_block(
        &mut self,
        block_id: u64,
    ) -> AceResult<(BlockHeader, Vec<u8>, Vec<u8>)> {
        let entry = self
            .index
            .by_id(block_id)
            .ok_or(AceError::BlockNotFound(block_id))?
            .clone();
        self.reader.seek(SeekFrom::Start(entry.file_offset))?;
        let mut fixed = [0u8; BLOCK_HEADER_SIZE];
        self.reader.read_exact(&mut fixed)?;
        let descriptor_size = block_descriptor_size(&fixed, self.file_header.minor_version);
        if fixed[22] as usize > self.limits.max_transforms {
            return Err(AceError::ResourceLimitExceeded("transform count"));
        }
        let mut descriptors = vec![0u8; descriptor_size];
        self.reader.read_exact(&mut descriptors)?;
        let header = decode_block_header(&fixed, &descriptors, self.file_header.minor_version)?;
        if header.block_id != block_id {
            return Err(AceError::Malformed(
                "index block id does not match serialized block",
            ));
        }
        let total = (header.metadata_size as usize)
            .checked_add(header.encoded_size as usize)
            .ok_or(AceError::Malformed("encoded block size overflow"))?;
        if total > self.limits.max_encoded_block_size {
            return Err(AceError::ResourceLimitExceeded("encoded block size"));
        }
        let mut metadata = vec![0u8; header.metadata_size as usize];
        self.reader.read_exact(&mut metadata)?;
        let mut payload = vec![0u8; header.encoded_size as usize];
        self.reader.read_exact(&mut payload)?;
        Ok((header, metadata, payload))
    }

    /// Returns the wrapped seekable reader.
    pub fn into_inner(self) -> R {
        self.reader
    }
}
