//! The `SEG1` payload of a segmented block (Format 1.6): a table of segments followed by the
//! segments, each an inner block (`docs/FORMAT-1.6.md`).
//!
//! ```text
//! "SEG1" · count u32 · reserved u32 (0) · count × (original_size u32 · serialized_len u32) · inner blocks
//! ```
//!
//! The table alone says where every segment starts in the reconstructed block (prefix sums of
//! `original_size`) and in the payload (prefix sums of `serialized_len`), so a range read can
//! decode only the segments it touches. Parsing validates the table against the outer block
//! before any inner block is read or any buffer is sized from it.

// Decoder hardening: arithmetic on values read from a file is checked and every narrowing cast
// is explicit (`docs/DECODER-AUDIT.md`).
#![warn(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)]

use std::ops::Range;

use ace_core::{AceError, AceResult, DecodeLimits};

use crate::BLOCK_HEADER_SIZE;

/// Magic bytes that open a segmented payload.
pub const SEGMENT_MAGIC: [u8; 4] = *b"SEG1";

/// Size of the fixed part of the payload: magic, count, reserved word.
pub const SEGMENT_HEADER_SIZE: usize = 12;

/// Size of one table entry: `original_size` and `serialized_len`.
pub const SEGMENT_ENTRY_SIZE: usize = 8;

/// One row of the segment table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentEntry {
    /// Bytes of the reconstructed block that this segment produces (non-zero).
    pub original_size: u32,
    /// Bytes of the serialized inner block in the payload (at least a block header).
    pub serialized_len: u32,
}

/// A validated segment table with the positions it implies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentTable {
    /// The rows, in segment order.
    pub entries: Vec<SegmentEntry>,
}

impl SegmentTable {
    /// Parses and validates the table at the start of `payload`.
    ///
    /// `block_original_size` is the `original_size` of the outer block header. The count is
    /// compared with `limits.max_segments` before the table is read, and the sizes are summed
    /// with checked arithmetic.
    ///
    /// # Errors
    /// [`AceError::Malformed`] for a wrong magic, a non-zero reserved word, fewer than two
    /// segments, a zero or too small entry, or sums that do not match the outer block and the
    /// payload length; [`AceError::ResourceLimitExceeded`] for too many segments or a segment
    /// larger than `limits.max_block_size`.
    pub fn parse(
        payload: &[u8],
        block_original_size: usize,
        limits: &DecodeLimits,
    ) -> AceResult<Self> {
        Self::parse_prefix(payload, payload.len(), block_original_size, limits)
    }

    /// Validates the 12 fixed bytes at the start of a payload and returns the segment count.
    ///
    /// A range reader calls this on the first [`SEGMENT_HEADER_SIZE`] bytes, learns how many
    /// table bytes to fetch ([`Self::table_len`]) and then calls [`Self::parse_prefix`].
    ///
    /// # Errors
    /// [`AceError::Malformed`] for a short head, a wrong magic, a non-zero reserved word or fewer
    /// than two segments; [`AceError::ResourceLimitExceeded`] above `limits.max_segments`.
    pub fn count_from_head(head: &[u8], limits: &DecodeLimits) -> AceResult<usize> {
        let head = head
            .first_chunk::<SEGMENT_HEADER_SIZE>()
            .ok_or(AceError::Malformed("truncated segment table"))?;
        if head[..4] != SEGMENT_MAGIC {
            return Err(AceError::Malformed("missing SEG1 magic"));
        }
        if head[8..12] != [0; 4] {
            return Err(AceError::Malformed("non-zero reserved SEG1 field"));
        }
        let count = usize::try_from(read_u32(&head[4..8])?)
            .map_err(|_| AceError::ResourceLimitExceeded("segment count"))?;
        if count < 2 {
            return Err(AceError::Malformed("fewer than two segments"));
        }
        if count > limits.max_segments {
            return Err(AceError::ResourceLimitExceeded("segment count"));
        }
        Ok(count)
    }

    /// Bytes of the fixed head plus the table for `count` segments (the offset of the first
    /// inner block in the payload).
    ///
    /// # Errors
    /// [`AceError::Malformed`] on overflow.
    pub fn table_len(count: usize) -> AceResult<usize> {
        count
            .checked_mul(SEGMENT_ENTRY_SIZE)
            .and_then(|table| table.checked_add(SEGMENT_HEADER_SIZE))
            .ok_or(AceError::Malformed("segment table size overflow"))
    }

    /// Like [`Self::parse`], for a reader that holds only the head and the table.
    ///
    /// `prefix` starts at the first payload byte and covers at least the table; `payload_len` is
    /// the length of the whole payload (the outer header's `encoded_size`), against which the
    /// serialized lengths are summed.
    ///
    /// # Errors
    /// The conditions of [`Self::parse`].
    pub fn parse_prefix(
        prefix: &[u8],
        payload_len: usize,
        block_original_size: usize,
        limits: &DecodeLimits,
    ) -> AceResult<Self> {
        let count = Self::count_from_head(prefix, limits)?;
        let table_end = Self::table_len(count)?;
        let table = prefix
            .get(SEGMENT_HEADER_SIZE..table_end)
            .ok_or(AceError::Malformed("truncated segment table"))?;
        let data_len = payload_len
            .checked_sub(table_end)
            .ok_or(AceError::Malformed("truncated segment table"))?;
        let mut entries = Vec::with_capacity(count);
        let mut original_total = 0usize;
        let mut serialized_total = 0usize;
        for row in table.chunks_exact(SEGMENT_ENTRY_SIZE) {
            let original_size = read_u32(&row[..4])?;
            let serialized_len = read_u32(&row[4..8])?;
            let original = usize::try_from(original_size)
                .map_err(|_| AceError::ResourceLimitExceeded("segment size"))?;
            let serialized = usize::try_from(serialized_len)
                .map_err(|_| AceError::ResourceLimitExceeded("segment size"))?;
            if original == 0 {
                return Err(AceError::Malformed("empty segment"));
            }
            if original > limits.max_block_size {
                return Err(AceError::ResourceLimitExceeded("segment size"));
            }
            if serialized < BLOCK_HEADER_SIZE {
                return Err(AceError::Malformed("segment shorter than a block header"));
            }
            original_total = original_total
                .checked_add(original)
                .ok_or(AceError::Malformed("segment size overflow"))?;
            serialized_total = serialized_total
                .checked_add(serialized)
                .ok_or(AceError::Malformed("segment length overflow"))?;
            entries.push(SegmentEntry {
                original_size,
                serialized_len,
            });
        }
        if original_total != block_original_size {
            return Err(AceError::Malformed("segment sizes do not add up to the block"));
        }
        if serialized_total != data_len {
            return Err(AceError::Malformed("segment lengths do not add up to the payload"));
        }
        Ok(Self { entries })
    }

    /// Offset of the first inner block in the payload.
    pub fn data_offset(&self) -> usize {
        SEGMENT_HEADER_SIZE
            .saturating_add(self.entries.len().saturating_mul(SEGMENT_ENTRY_SIZE))
    }

    /// The part of the reconstructed block produced by each segment, in order.
    pub fn logical_ranges(&self) -> Vec<Range<usize>> {
        let mut start = 0usize;
        self.entries
            .iter()
            .map(|entry| {
                let end = start.saturating_add(entry.original_size as usize);
                let range = start..end;
                start = end;
                range
            })
            .collect()
    }

    /// The bytes of each inner block within the payload, in order.
    pub fn payload_ranges(&self) -> Vec<Range<usize>> {
        let mut start = self.data_offset();
        self.entries
            .iter()
            .map(|entry| {
                let end = start.saturating_add(entry.serialized_len as usize);
                let range = start..end;
                start = end;
                range
            })
            .collect()
    }
}

/// Builds a `SEG1` payload from serialized inner blocks.
///
/// `segments` pairs the reconstructed size of each segment with its serialized inner block (see
/// [`crate::serialize_block`]).
///
/// # Errors
/// [`AceError::InvalidConfig`] for fewer than two segments, an empty segment or an inner block
/// shorter than a block header, and [`AceError::ResourceLimitExceeded`] when a length does not
/// fit in `u32`.
pub fn encode_segment_payload(segments: &[(u32, Vec<u8>)]) -> AceResult<Vec<u8>> {
    if segments.len() < 2 {
        return Err(AceError::InvalidConfig("a segmented block needs two segments"));
    }
    let count = u32::try_from(segments.len())
        .map_err(|_| AceError::ResourceLimitExceeded("segment count"))?;
    let mut out = Vec::new();
    out.extend_from_slice(&SEGMENT_MAGIC);
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    for (original_size, inner) in segments {
        if *original_size == 0 || inner.len() < BLOCK_HEADER_SIZE {
            return Err(AceError::InvalidConfig("invalid segment"));
        }
        let serialized_len = u32::try_from(inner.len())
            .map_err(|_| AceError::ResourceLimitExceeded("segment length"))?;
        out.extend_from_slice(&original_size.to_le_bytes());
        out.extend_from_slice(&serialized_len.to_le_bytes());
    }
    for (_, inner) in segments {
        out.extend_from_slice(inner);
    }
    Ok(out)
}

/// Little-endian `u32` from exactly four bytes.
fn read_u32(bytes: &[u8]) -> AceResult<u32> {
    Ok(u32::from_le_bytes(
        bytes
            .try_into()
            .map_err(|_| AceError::Malformed("truncated segment field"))?,
    ))
}
