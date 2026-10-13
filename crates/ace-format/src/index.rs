// Decoder hardening (ACE 0.5.1): arithmetic on values read from a file is checked and every
// narrowing cast is explicit (`docs/DECODER-AUDIT.md`); a proven-safe site carries a local
// `allow` with its proof.
#![warn(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)]

use ace_core::{AceError, AceResult, DecodeLimits};

/// Four-byte serialized block-index section magic.
pub const INDEX_MAGIC: [u8; 4] = *b"AIDX";
/// Four-byte end-of-file trailer magic.
pub const TRAILER_MAGIC: [u8; 4] = *b"ACET";
/// Fixed serialized size of one block-index entry.
pub const INDEX_ENTRY_SIZE: usize = 40;
/// Size of the index section header (magic + little-endian `u64` entry count).
const INDEX_HEADER_SIZE: usize = 12;
/// Fixed serialized trailer size.
pub const TRAILER_SIZE: usize = 28;

/// Random-access metadata for one independent ACE block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockIndexEntry {
    /// Stable block identifier.
    pub block_id: u64,
    /// Logical byte offset in reconstructed data.
    pub original_offset: u64,
    /// Reconstructed block size.
    pub original_size: u32,
    /// Physical byte offset of the block header in the ACE file.
    pub file_offset: u64,
    /// Entire serialized block span including header, descriptors, metadata and payload.
    pub encoded_span: u32,
    /// Reserved per-entry flags.
    pub flags: u32,
}

/// Sorted collection of block index entries used by `AceIndexedReader`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BlockIndex {
    /// Entries sorted by `block_id` and logical offset.
    pub entries: Vec<BlockIndexEntry>,
}

/// Inherent methods of [`BlockIndex`].
impl BlockIndex {
    /// Finds one block by identifier using binary search.
    pub fn by_id(&self, block_id: u64) -> Option<&BlockIndexEntry> {
        self.entries
            .binary_search_by_key(&block_id, |entry| entry.block_id)
            .ok()
            .map(|i| &self.entries[i])
    }

    /// Returns entries intersecting the logical half-open byte range `[start, end)`.
    pub fn intersecting(&self, start: u64, end: u64) -> Vec<&BlockIndexEntry> {
        self.intersecting_indices(start, end)
            .map(|index| &self.entries[index])
            .collect()
    }

    /// Returns the contiguous index positions intersecting `[start, end)` without allocating.
    ///
    /// ACE block logical offsets are validated as contiguous and sorted, so every logical range
    /// maps to one contiguous slice of `entries`. Random-access decoding uses this helper to avoid
    /// allocating a temporary vector of index-entry references for every small read.
    pub fn intersecting_indices(&self, start: u64, end: u64) -> std::ops::Range<usize> {
        if start >= end || self.entries.is_empty() {
            return 0..0;
        }
        let first = self.entries.partition_point(|entry| {
            entry
                .original_offset
                .saturating_add(entry.original_size as u64)
                <= start
        });
        let last = self
            .entries
            .partition_point(|entry| entry.original_offset < end);
        first.min(self.entries.len())..last.min(self.entries.len())
    }
}

/// End-of-file pointer to the serialized block-index section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileTrailer {
    /// Physical byte offset of the block-index section.
    pub index_offset: u64,
    /// Serialized block-index section size.
    pub index_size: u64,
    /// CRC32C of the complete index section.
    pub index_crc32c: u32,
}

/// Serializes a complete block index section including its magic and entry count.
pub fn encode_index(index: &BlockIndex) -> Vec<u8> {
    // Capacity hint only (an in-memory index cannot approach `usize::MAX` bytes).
    let capacity = index
        .entries
        .len()
        .saturating_mul(INDEX_ENTRY_SIZE)
        .saturating_add(INDEX_HEADER_SIZE);
    let mut out = Vec::with_capacity(capacity);
    out.extend_from_slice(&INDEX_MAGIC);
    out.extend_from_slice(&(index.entries.len() as u64).to_le_bytes());
    for entry in &index.entries {
        out.extend_from_slice(&entry.block_id.to_le_bytes());
        out.extend_from_slice(&entry.original_offset.to_le_bytes());
        out.extend_from_slice(&entry.original_size.to_le_bytes());
        out.extend_from_slice(&entry.file_offset.to_le_bytes());
        out.extend_from_slice(&entry.encoded_span.to_le_bytes());
        out.extend_from_slice(&entry.flags.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
    }
    out
}

/// Parses and validates a complete block-index section.
///
/// # Errors
///
/// [`AceError::Malformed`] for a wrong magic, length or entry layout,
/// [`AceError::IndexChecksumMismatch`], and [`AceError::ResourceLimitExceeded`] when the
/// entry count exceeds `limits.max_index_entries`.
pub fn decode_index(bytes: &[u8], limits: &DecodeLimits) -> AceResult<BlockIndex> {
    let (header, body) = bytes
        .split_first_chunk::<INDEX_HEADER_SIZE>()
        .ok_or(AceError::Malformed("invalid block index header"))?;
    let (magic, count) = header.split_at(4);
    if magic != INDEX_MAGIC {
        return Err(AceError::Malformed("invalid block index header"));
    }
    let count = u64::from_le_bytes(
        count
            .try_into()
            .map_err(|_| AceError::Malformed("invalid index count"))?,
    );
    // Checked before any size arithmetic or allocation: a count that does not fit `usize`
    // (32-bit hosts) or exceeds the limit is rejected, never truncated.
    let count = usize::try_from(count)
        .ok()
        .filter(|&count| count <= limits.max_index_entries)
        .ok_or(AceError::ResourceLimitExceeded("block index entry count"))?;
    let expected = count
        .checked_mul(INDEX_ENTRY_SIZE)
        .ok_or(AceError::Malformed("index size overflow"))?;
    let (records, rest) = body.as_chunks::<INDEX_ENTRY_SIZE>();
    if body.len() != expected || !rest.is_empty() {
        return Err(AceError::Malformed("block index size mismatch"));
    }
    let mut entries = Vec::with_capacity(count.min(1_000_000));
    let mut previous_id = None;
    let mut previous_original_end = 0u64;
    for (i, record) in records.iter().enumerate() {
        let block_id = u64::from_le_bytes(field(record, 0)?);
        let original_offset = u64::from_le_bytes(field(record, 8)?);
        let original_size = u32::from_le_bytes(field(record, 16)?);
        let file_offset = u64::from_le_bytes(field(record, 20)?);
        let encoded_span = u32::from_le_bytes(field(record, 28)?);
        let flags = u32::from_le_bytes(field(record, 32)?);
        if previous_id.map(|id| block_id <= id).unwrap_or(false) {
            return Err(AceError::Malformed("block ids are not strictly increasing"));
        }
        if i > 0 && original_offset != previous_original_end {
            return Err(AceError::Malformed(
                "logical block offsets are not contiguous",
            ));
        }
        previous_id = Some(block_id);
        previous_original_end = original_offset
            .checked_add(original_size as u64)
            .ok_or(AceError::Malformed("logical offset overflow"))?;
        entries.push(BlockIndexEntry {
            block_id,
            original_offset,
            original_size,
            file_offset,
            encoded_span,
            flags,
        });
    }
    Ok(BlockIndex { entries })
}

/// The `N`-byte field at `offset` of one serialized index entry.
fn field<const N: usize>(record: &[u8; INDEX_ENTRY_SIZE], offset: usize) -> AceResult<[u8; N]> {
    record
        .get(offset..)
        .and_then(<[u8]>::first_chunk::<N>)
        .copied()
        .ok_or(AceError::Malformed("invalid index entry layout"))
}

/// Serializes an end-of-file trailer and protects its first 24 bytes with CRC32C.
pub fn encode_trailer(trailer: FileTrailer) -> [u8; TRAILER_SIZE] {
    let mut out = [0u8; TRAILER_SIZE];
    out[0..4].copy_from_slice(&TRAILER_MAGIC);
    out[4..12].copy_from_slice(&trailer.index_offset.to_le_bytes());
    out[12..20].copy_from_slice(&trailer.index_size.to_le_bytes());
    out[20..24].copy_from_slice(&trailer.index_crc32c.to_le_bytes());
    let crc = crate::checksum(&out[..24]);
    out[24..28].copy_from_slice(&crc.to_le_bytes());
    out
}

/// Parses and validates an end-of-file trailer.
///
/// # Errors
///
/// [`AceError::Malformed`] for a wrong length or magic and
/// [`AceError::IndexChecksumMismatch`] for a wrong CRC.
pub fn decode_trailer(bytes: &[u8]) -> AceResult<FileTrailer> {
    if bytes.len() != TRAILER_SIZE || bytes[0..4] != TRAILER_MAGIC {
        return Err(AceError::Malformed("invalid ACE trailer"));
    }
    let stored = u32::from_le_bytes(
        bytes[24..28]
            .try_into()
            .map_err(|_| AceError::Malformed("invalid trailer CRC"))?,
    );
    if crate::checksum(&bytes[..24]) != stored {
        return Err(AceError::Malformed("trailer checksum mismatch"));
    }
    Ok(FileTrailer {
        index_offset: u64::from_le_bytes(
            bytes[4..12]
                .try_into()
                .map_err(|_| AceError::Malformed("invalid index offset"))?,
        ),
        index_size: u64::from_le_bytes(
            bytes[12..20]
                .try_into()
                .map_err(|_| AceError::Malformed("invalid index size"))?,
        ),
        index_crc32c: u32::from_le_bytes(
            bytes[20..24]
                .try_into()
                .map_err(|_| AceError::Malformed("invalid index checksum"))?,
        ),
    })
}

#[cfg(test)]
mod buildfix9_tests {
    use super::*;

    /// The allocation-free intersection indices must identify the same contiguous blocks as the public vector API.
    #[test]
    fn intersecting_indices_match_entries() {
        let index = BlockIndex {
            entries: vec![
                BlockIndexEntry {
                    block_id: 0,
                    original_offset: 0,
                    original_size: 100,
                    file_offset: 10,
                    encoded_span: 20,
                    flags: 0,
                },
                BlockIndexEntry {
                    block_id: 1,
                    original_offset: 100,
                    original_size: 100,
                    file_offset: 30,
                    encoded_span: 20,
                    flags: 0,
                },
                BlockIndexEntry {
                    block_id: 2,
                    original_offset: 200,
                    original_size: 100,
                    file_offset: 50,
                    encoded_span: 20,
                    flags: 0,
                },
            ],
        };
        assert_eq!(index.intersecting_indices(90, 210), 0..3);
        let ids = index
            .intersecting(90, 210)
            .into_iter()
            .map(|entry| entry.block_id)
            .collect::<Vec<_>>();
        assert_eq!(ids, vec![0, 1, 2]);
        assert_eq!(index.intersecting_indices(100, 200), 1..2);
    }
}
