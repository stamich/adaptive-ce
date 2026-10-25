//! Seekable reader that loads and validates the AIDX block index once.

// Decoder hardening (ACE 0.5.1): arithmetic on values read from a file is checked and every
// narrowing cast is explicit (`docs/DECODER-AUDIT.md`); a proven-safe site carries a local
// `allow` with its proof.
#![warn(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)]

use std::io::{Read, Seek, SeekFrom};

use ace_core::{AceError, AceResult, DecodeLimits};
use ace_format::{
    checksum, decode_file_header, decode_index, decode_trailer, read_block_header,
    read_dictionary_section, read_serialized_block, BlockHeader, BlockIndex, EmbeddedDictionary,
    FileHeader, SerializedBlock, FILE_FLAG_HAS_DICTIONARIES, FILE_FLAG_HAS_INDEX,
    FILE_HEADER_SIZE, INDEX_ENTRY_SIZE, INDEX_HEADER_SIZE, TRAILER_SIZE,
};

/// Seek offset of the trailer from the end of the file.
#[allow(clippy::arithmetic_side_effects, clippy::cast_possible_wrap)] // constant: -28
const TRAILER_OFFSET_FROM_END: i64 = -(TRAILER_SIZE as i64);

/// Largest index section `limits` admit: the header plus `max_index_entries` entries.
fn max_index_bytes(limits: &DecodeLimits) -> usize {
    limits
        .max_index_entries
        .saturating_mul(INDEX_ENTRY_SIZE)
        .saturating_add(INDEX_HEADER_SIZE)
}

/// Seekable parser that loads the ACE block index once and can fetch individual encoded blocks.
pub struct AceIndexReader<R: Read + Seek> {
    /// Underlying seekable source.
    reader: R,
    /// Parsed global file header.
    pub file_header: FileHeader,
    /// Validated sorted block index.
    pub index: BlockIndex,
    /// Dictionaries of the file's dictionary section (Format 1.6); empty for files without one.
    pub dictionaries: Vec<EmbeddedDictionary>,
    /// Limits applied while parsing blocks.
    limits: DecodeLimits,
}

/// Inherent methods of [`AceIndexReader`].
impl<R: Read + Seek> AceIndexReader<R> {
    /// Opens a seekable ACE file, loads its trailer (Format 1.1 and later) and validates the block index.
    ///
    /// # Errors
    ///
    /// Header, trailer and index validation errors and [`AceError::Io`]; a file without a
    /// trailer is [`AceError::Malformed`].
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
        // The section sits right behind the file header; block offsets are absolute, so the
        // index is not affected by it.
        let dictionaries = if file_header.flags & FILE_FLAG_HAS_DICTIONARIES != 0 {
            read_dictionary_section(&mut reader, &limits)?
        } else {
            Vec::new()
        };
        if file_header.block_count > limits.max_block_count() {
            return Err(AceError::ResourceLimitExceeded("block count"));
        }
        let file_len = reader.seek(SeekFrom::End(0))?;
        if file_len < (FILE_HEADER_SIZE + TRAILER_SIZE) as u64 {
            return Err(AceError::Malformed(
                "file is too small for indexed ACE format",
            ));
        }
        reader.seek(SeekFrom::End(TRAILER_OFFSET_FROM_END))?;
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
        // The index allocation is bounded by `max_index_entries` (not only by the file length),
        // so a forged trailer cannot make the reader allocate more than the limits allow.
        let index_size = usize::try_from(trailer.index_size)
            .ok()
            .filter(|&size| size <= max_index_bytes(&limits))
            .ok_or(AceError::ResourceLimitExceeded("index size"))?;
        reader.seek(SeekFrom::Start(trailer.index_offset))?;
        let mut index_bytes = vec![0u8; index_size];
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
            dictionaries,
            limits,
        })
    }

    /// Reads one indexed encoded block without decoding its compression pipeline.
    ///
    /// # Errors
    ///
    /// [`AceError::BlockNotFound`] for an unknown `block_id`; block read errors and
    /// [`AceError::Io`].
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

    /// Reads only the header of an indexed block (fixed part and descriptors) and returns it with
    /// its length in bytes. The payload is not read.
    ///
    /// # Errors
    ///
    /// [`AceError::BlockNotFound`] for an unknown `block_id`, the header and limit errors of
    /// [`ace_format::read_block_header`], [`AceError::Malformed`] when the stored id differs from
    /// the index and [`AceError::Io`].
    pub fn read_block_header_only(&mut self, block_id: u64) -> AceResult<(BlockHeader, usize)> {
        let file_offset = self
            .index
            .by_id(block_id)
            .ok_or(AceError::BlockNotFound(block_id))?
            .file_offset;
        self.reader.seek(SeekFrom::Start(file_offset))?;
        let (header, header_len) =
            read_block_header(&mut self.reader, self.file_header.minor_version, &self.limits)?;
        if header.block_id != block_id {
            return Err(AceError::Malformed(
                "index block id does not match serialized block",
            ));
        }
        Ok((header, header_len))
    }

    /// Reads `len` bytes that start `offset` bytes after the first byte of an indexed block.
    ///
    /// The window must lie inside the block's indexed span, so a forged length cannot make the
    /// reader allocate more than the index allows (the span is itself bounded by the limits when
    /// the block header is read).
    ///
    /// # Errors
    ///
    /// [`AceError::BlockNotFound`] for an unknown `block_id`, [`AceError::Malformed`] for a
    /// window outside the span and [`AceError::Io`].
    pub fn read_within_block(
        &mut self,
        block_id: u64,
        offset: u64,
        len: usize,
    ) -> AceResult<Vec<u8>> {
        let entry = self
            .index
            .by_id(block_id)
            .ok_or(AceError::BlockNotFound(block_id))?;
        let end = offset
            .checked_add(len as u64)
            .ok_or(AceError::Malformed("block window overflow"))?;
        if end > u64::from(entry.encoded_span) {
            return Err(AceError::Malformed("block window lies outside the block"));
        }
        let start = entry
            .file_offset
            .checked_add(offset)
            .ok_or(AceError::Malformed("block window overflow"))?;
        self.reader.seek(SeekFrom::Start(start))?;
        let mut bytes = vec![0u8; len];
        self.reader.read_exact(&mut bytes)?;
        Ok(bytes)
    }

    /// Returns the wrapped seekable reader.
    pub fn into_inner(self) -> R {
        self.reader
    }
}
