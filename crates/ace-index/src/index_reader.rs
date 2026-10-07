//! Seekable reader that loads and validates the AIDX block index once.

use std::io::{Read, Seek, SeekFrom};

use ace_core::{AceError, AceResult, DecodeLimits};
use ace_format::{
    checksum, decode_file_header, decode_index, decode_trailer, read_serialized_block, BlockIndex,
    FileHeader, SerializedBlock, FILE_FLAG_HAS_INDEX, FILE_HEADER_SIZE, TRAILER_SIZE,
};

/// Seekable parser that loads the ACE block index once and can fetch individual encoded blocks.
pub struct AceIndexReader<R: Read + Seek> {
    /// Underlying seekable source.
    reader: R,
    /// Parsed global file header.
    pub file_header: FileHeader,
    /// Validated sorted block index.
    pub index: BlockIndex,
    /// Limits applied while parsing blocks.
    limits: DecodeLimits,
}

/// Inherent methods of [`AceIndexReader`].
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
    pub fn read_encoded_block(&mut self, block_id: u64) -> AceResult<SerializedBlock> {
        let file_offset = self
            .index
            .by_id(block_id)
            .ok_or(AceError::BlockNotFound(block_id))?
            .file_offset;
        self.reader.seek(SeekFrom::Start(file_offset))?;
        let block = read_serialized_block(
            &mut self.reader,
            self.file_header.minor_version,
            &self.limits,
        )?;
        if block.0.block_id != block_id {
            return Err(AceError::Malformed(
                "index block id does not match serialized block",
            ));
        }
        Ok(block)
    }

    /// Returns the wrapped seekable reader.
    pub fn into_inner(self) -> R {
        self.reader
    }
}
