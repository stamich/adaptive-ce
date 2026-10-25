//! The `COL1` payload of a columnar block (Format 1.6): a column table, one inner block per
//! column stream and the tail of an incomplete last record (`docs/FORMAT-1.6.md`).
//!
//! ```text
//! "COL1" · version u8 (0) · flags u8 (0) · columns u16 · record_size u32 · record_count u32
//!        · tail_len u32 · reserved u32 (0)
//!        · columns × (width u16 · kind u8 · reserved u8 (0) · serialized_len u32)
//!        · inner blocks (one per column) · tail bytes
//! ```
//!
//! A *record* is the concatenation of the columns; the stream of column *i* holds byte range
//! `[offset_i, offset_i + width_i)` of every record, record after record. [`split_columns`] and
//! [`join_columns`] are the transpose and its inverse; they are exact inverses for every input,
//! including one whose length is not a multiple of the record size.

// Decoder hardening: arithmetic on values read from a file is checked and every narrowing cast
// is explicit (`docs/DECODER-AUDIT.md`).
#![warn(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)]

use std::ops::Range;

use ace_core::{AceError, AceResult, DecodeLimits};
pub use ace_core::{ColumnKind, ColumnSpec};

use crate::BLOCK_HEADER_SIZE;

/// Magic bytes that open a columnar payload.
pub const COLUMN_MAGIC: [u8; 4] = *b"COL1";

/// Size of the fixed part of the payload.
pub const COLUMN_HEADER_SIZE: usize = 24;

/// Size of one column table entry.
pub const COLUMN_ENTRY_SIZE: usize = 8;

/// A table row: the column plus the size of its serialized inner block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnEntry {
    /// The column.
    pub spec: ColumnSpec,
    /// Bytes of the serialized inner block in the payload (at least a block header).
    pub serialized_len: u32,
}

/// A validated column table with the positions it implies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnTable {
    /// Bytes of one record (sum of the widths).
    pub record_size: u32,
    /// Whole records in the block (non-zero).
    pub record_count: u32,
    /// Bytes of the incomplete last record, `< record_size`.
    pub tail_len: u32,
    /// The rows, in column order.
    pub columns: Vec<ColumnEntry>,
}

/// Inherent methods of [`ColumnTable`].
impl ColumnTable {
    /// Parses and validates the table at the start of `payload`.
    ///
    /// `block_original_size` is the `original_size` of the outer block header. The column count,
    /// the record size and the block size are compared with `limits` before the table is read.
    ///
    /// # Errors
    /// [`AceError::Malformed`] for a wrong magic, version, flags or reserved field, an unknown
    /// kind, a width the kind does not allow, a zero record count, a record size that is not the
    /// sum of the widths, a tail that is not shorter than a record, or sums that do not match
    /// the outer block and the payload length; [`AceError::ResourceLimitExceeded`] for too many
    /// columns or a block or record larger than `limits.max_block_size`.
    pub fn parse(
        payload: &[u8],
        block_original_size: usize,
        limits: &DecodeLimits,
    ) -> AceResult<Self> {
        let (head, rest) = payload
            .split_first_chunk::<COLUMN_HEADER_SIZE>()
            .ok_or(AceError::Malformed("truncated column table"))?;
        if head[..4] != COLUMN_MAGIC {
            return Err(AceError::Malformed("missing COL1 magic"));
        }
        if head[4] != 0 || head[5] != 0 {
            return Err(AceError::Malformed("unsupported COL1 version or flags"));
        }
        if head[20..24] != [0; 4] {
            return Err(AceError::Malformed("non-zero reserved COL1 field"));
        }
        let count = usize::from(u16::from_le_bytes([head[6], head[7]]));
        let record_size = read_u32(&head[8..12])?;
        let record_count = read_u32(&head[12..16])?;
        let tail_len = read_u32(&head[16..20])?;
        if count == 0 {
            return Err(AceError::Malformed("columnar block without columns"));
        }
        if count > limits.max_columns {
            return Err(AceError::ResourceLimitExceeded("column count"));
        }
        if block_original_size > limits.max_block_size {
            return Err(AceError::ResourceLimitExceeded("block size"));
        }
        let record = to_usize(record_size)?;
        if record > limits.max_block_size {
            return Err(AceError::ResourceLimitExceeded("record size"));
        }
        if record == 0 || record_count == 0 {
            return Err(AceError::Malformed("empty record layout"));
        }
        if tail_len >= record_size {
            return Err(AceError::Malformed("tail is not shorter than a record"));
        }
        let records_bytes = to_usize(record_count)?
            .checked_mul(record)
            .and_then(|bytes| bytes.checked_add(to_usize(tail_len).ok()?))
            .ok_or(AceError::Malformed("column block size overflow"))?;
        if records_bytes != block_original_size {
            return Err(AceError::Malformed(
                "record layout does not add up to the block",
            ));
        }
        let table_bytes = count
            .checked_mul(COLUMN_ENTRY_SIZE)
            .ok_or(AceError::Malformed("column table size overflow"))?;
        let (table, data) = rest
            .split_at_checked(table_bytes)
            .ok_or(AceError::Malformed("truncated column table"))?;
        let mut columns = Vec::with_capacity(count);
        let mut width_total = 0usize;
        let mut serialized_total = 0usize;
        for row in table.chunks_exact(COLUMN_ENTRY_SIZE) {
            let width = u16::from_le_bytes([row[0], row[1]]);
            let kind = ColumnKind::from_id(row[2])?;
            if row[3] != 0 {
                return Err(AceError::Malformed("non-zero reserved column field"));
            }
            if !kind.accepts_width(width) {
                return Err(AceError::Malformed("column width does not fit its kind"));
            }
            let serialized_len = read_u32(&row[4..8])?;
            if to_usize(serialized_len)? < BLOCK_HEADER_SIZE {
                return Err(AceError::Malformed("column shorter than a block header"));
            }
            width_total = width_total
                .checked_add(usize::from(width))
                .ok_or(AceError::Malformed("column width overflow"))?;
            serialized_total = serialized_total
                .checked_add(to_usize(serialized_len)?)
                .ok_or(AceError::Malformed("column length overflow"))?;
            columns.push(ColumnEntry {
                spec: ColumnSpec { width, kind },
                serialized_len,
            });
        }
        if width_total != record {
            return Err(AceError::Malformed(
                "column widths do not add up to the record size",
            ));
        }
        let expected = serialized_total
            .checked_add(to_usize(tail_len)?)
            .ok_or(AceError::Malformed("column length overflow"))?;
        if expected != data.len() {
            return Err(AceError::Malformed(
                "column lengths do not add up to the payload",
            ));
        }
        Ok(Self {
            record_size,
            record_count,
            tail_len,
            columns,
        })
    }

    /// Offset of the first inner block in the payload.
    pub fn data_offset(&self) -> usize {
        COLUMN_HEADER_SIZE.saturating_add(self.columns.len().saturating_mul(COLUMN_ENTRY_SIZE))
    }

    /// Reconstructed size of each column stream: `record_count × width`.
    pub fn stream_sizes(&self) -> Vec<u32> {
        self.columns
            .iter()
            .map(|column| {
                self.record_count
                    .saturating_mul(u32::from(column.spec.width))
            })
            .collect()
    }

    /// The bytes of each inner block within the payload, in column order.
    pub fn payload_ranges(&self) -> Vec<Range<usize>> {
        let mut start = self.data_offset();
        self.columns
            .iter()
            .map(|column| {
                let end = start.saturating_add(column.serialized_len as usize);
                let range = start..end;
                start = end;
                range
            })
            .collect()
    }

    /// The tail bytes within the payload (they close the payload).
    pub fn tail_range(&self) -> Range<usize> {
        let end = self
            .payload_ranges()
            .last()
            .map_or(self.data_offset(), |range| range.end);
        end..end.saturating_add(self.tail_len as usize)
    }
}

/// Builds a `COL1` payload.
///
/// `columns` pairs each column with its serialized inner block (see [`crate::serialize_block`]);
/// `tail` holds the bytes of the incomplete last record. The record layout is derived from the
/// columns and `record_count`.
///
/// # Errors
/// [`AceError::InvalidConfig`] for no columns, more than `u16::MAX`, a width the kind does not
/// allow, an inner block shorter than a block header, a zero record count or a tail that is not
/// shorter than a record, and [`AceError::ResourceLimitExceeded`] when a size does not fit in
/// `u32`.
pub fn encode_column_payload(
    columns: &[(ColumnSpec, Vec<u8>)],
    record_count: u32,
    tail: &[u8],
) -> AceResult<Vec<u8>> {
    let count =
        u16::try_from(columns.len()).map_err(|_| AceError::InvalidConfig("too many columns"))?;
    if count == 0 || record_count == 0 {
        return Err(AceError::InvalidConfig(
            "a columnar block needs columns and records",
        ));
    }
    let mut record_size = 0u32;
    for (spec, inner) in columns {
        if !spec.kind.accepts_width(spec.width) || inner.len() < BLOCK_HEADER_SIZE {
            return Err(AceError::InvalidConfig("invalid column"));
        }
        record_size = record_size
            .checked_add(u32::from(spec.width))
            .ok_or(AceError::ResourceLimitExceeded("record size"))?;
    }
    let tail_len =
        u32::try_from(tail.len()).map_err(|_| AceError::ResourceLimitExceeded("column tail"))?;
    if tail_len >= record_size {
        return Err(AceError::InvalidConfig(
            "tail must be shorter than a record",
        ));
    }
    let mut out = Vec::new();
    out.extend_from_slice(&COLUMN_MAGIC);
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&record_size.to_le_bytes());
    out.extend_from_slice(&record_count.to_le_bytes());
    out.extend_from_slice(&tail_len.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    for (spec, inner) in columns {
        let serialized_len = u32::try_from(inner.len())
            .map_err(|_| AceError::ResourceLimitExceeded("column length"))?;
        out.extend_from_slice(&spec.width.to_le_bytes());
        out.push(spec.kind.id());
        out.push(0);
        out.extend_from_slice(&serialized_len.to_le_bytes());
    }
    for (_, inner) in columns {
        out.extend_from_slice(inner);
    }
    out.extend_from_slice(tail);
    Ok(out)
}

/// Splits `data` into one stream per column plus the tail of an incomplete last record.
///
/// With `record_size = Σ widths`, stream *i* holds, record after record, the bytes
/// `[offset_i, offset_i + width_i)` of each of the `data.len() / record_size` whole records; the
/// remaining `data.len() % record_size` bytes are returned as the tail.
///
/// # Errors
/// [`AceError::InvalidConfig`] for an empty schema or a zero width.
pub fn split_columns<'a>(data: &'a [u8], widths: &[u16]) -> AceResult<(Vec<Vec<u8>>, &'a [u8])> {
    let record = record_size_of(widths)?;
    let records = data.len().checked_div(record).unwrap_or(0);
    let whole = records.saturating_mul(record);
    let (body, tail) = data.split_at(whole);
    let mut streams: Vec<Vec<u8>> = widths
        .iter()
        .map(|&width| Vec::with_capacity(records.saturating_mul(usize::from(width))))
        .collect();
    for row in body.chunks_exact(record) {
        let mut offset = 0usize;
        for (stream, &width) in streams.iter_mut().zip(widths) {
            let end = offset.saturating_add(usize::from(width));
            stream.extend_from_slice(&row[offset..end]);
            offset = end;
        }
    }
    Ok((streams, tail))
}

/// The exact inverse of [`split_columns`]: interleaves the streams record by record and appends
/// the tail.
///
/// # Errors
/// [`AceError::InvalidConfig`] for an empty schema or a zero width, and [`AceError::Malformed`]
/// when the number of streams differs from the number of widths or a stream does not hold
/// exactly `record_count × width` bytes for the common `record_count`.
pub fn join_columns<S: AsRef<[u8]>>(
    streams: &[S],
    widths: &[u16],
    tail: &[u8],
) -> AceResult<Vec<u8>> {
    let record = record_size_of(widths)?;
    if streams.len() != widths.len() {
        return Err(AceError::Malformed(
            "stream count differs from column count",
        ));
    }
    let records = streams[0]
        .as_ref()
        .len()
        .checked_div(usize::from(widths[0]))
        .unwrap_or(0);
    for (stream, &width) in streams.iter().zip(widths) {
        if stream.as_ref().len() != records.saturating_mul(usize::from(width)) {
            return Err(AceError::Malformed("column stream has the wrong length"));
        }
    }
    let mut out = Vec::with_capacity(records.saturating_mul(record).saturating_add(tail.len()));
    for index in 0..records {
        for (stream, &width) in streams.iter().zip(widths) {
            let width = usize::from(width);
            let start = index.saturating_mul(width);
            out.extend_from_slice(&stream.as_ref()[start..start.saturating_add(width)]);
        }
    }
    out.extend_from_slice(tail);
    Ok(out)
}

/// `Σ widths`, validated.
fn record_size_of(widths: &[u16]) -> AceResult<usize> {
    if widths.is_empty() || widths.contains(&0) {
        return Err(AceError::InvalidConfig(
            "a schema needs non-zero column widths",
        ));
    }
    Ok(widths.iter().map(|&width| usize::from(width)).sum())
}

/// `u32` → `usize` without a lossy cast.
fn to_usize(value: u32) -> AceResult<usize> {
    usize::try_from(value).map_err(|_| AceError::ResourceLimitExceeded("column size"))
}

/// Little-endian `u32` from exactly four bytes.
fn read_u32(bytes: &[u8]) -> AceResult<u32> {
    Ok(u32::from_le_bytes(bytes.try_into().map_err(|_| {
        AceError::Malformed("truncated column field")
    })?))
}
