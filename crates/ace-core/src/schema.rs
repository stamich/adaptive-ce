//! The record layout of a columnar block (Format 1.6, `COL1`).
//!
//! ACE never guesses a layout: a [`Schema`] is supplied by the caller (`AceConfig::schema`) and
//! says how many bytes each field of a fixed-size record takes and what it holds. The engine
//! then stores the fields column by column, which lets every column use the codec that suits it
//! (`docs/FORMAT-1.6.md`).

use crate::{AceError, AceResult};

/// What a column holds. It does not change decoding — every stored column describes itself — it
/// lets the encoder choose a suitable codec and lets tools interpret a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnKind {
    /// Opaque bytes of any non-zero width (identifiers, text, flags).
    Bytes,
    /// Unsigned little-endian integers of width 1, 2, 4 or 8 (counters, ids, timestamps).
    Unsigned,
    /// Signed little-endian integers of width 1, 2, 4 or 8 (deltas, offsets).
    Signed,
    /// IEEE-754 little-endian floats of width 4 or 8 (measurements, prices).
    Float,
}

/// Inherent methods of [`ColumnKind`].
impl ColumnKind {
    /// The byte stored in the `COL1` column table: 0 bytes, 1 unsigned, 2 signed, 3 float.
    pub const fn id(self) -> u8 {
        match self {
            Self::Bytes => 0,
            Self::Unsigned => 1,
            Self::Signed => 2,
            Self::Float => 3,
        }
    }

    /// Parses the table byte.
    ///
    /// # Errors
    /// [`AceError::Malformed`] for an unknown kind.
    pub const fn from_id(id: u8) -> AceResult<Self> {
        match id {
            0 => Ok(Self::Bytes),
            1 => Ok(Self::Unsigned),
            2 => Ok(Self::Signed),
            3 => Ok(Self::Float),
            _ => Err(AceError::Malformed("unknown column kind")),
        }
    }

    /// Whether `width` (bytes) is allowed for this kind.
    pub const fn accepts_width(self, width: u16) -> bool {
        match self {
            Self::Bytes => width != 0,
            Self::Unsigned | Self::Signed => matches!(width, 1 | 2 | 4 | 8),
            Self::Float => matches!(width, 4 | 8),
        }
    }
}

/// One column of a schema: its width in bytes and what it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnSpec {
    /// Bytes per record taken by this column (non-zero, valid for `kind`).
    pub width: u16,
    /// What the column holds.
    pub kind: ColumnKind,
}

/// A validated list of columns: the layout of one fixed-size record.
///
/// A record is the concatenation of the columns, so its size is the sum of the widths. Column
/// *i* is the byte range `[offset_i, offset_i + width_i)` of every record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schema {
    /// The columns, in record order.
    columns: Vec<ColumnSpec>,
    /// Sum of the widths.
    record_size: u32,
}

/// Inherent methods of [`Schema`].
impl Schema {
    /// Validates `columns` and builds the schema.
    ///
    /// # Errors
    /// [`AceError::InvalidConfig`] for no columns, more than `u16::MAX` columns, a width the
    /// kind does not allow, or a record size that does not fit in `u32`.
    pub fn new(columns: Vec<ColumnSpec>) -> AceResult<Self> {
        if columns.is_empty() {
            return Err(AceError::InvalidConfig("a schema needs at least one column"));
        }
        if columns.len() > usize::from(u16::MAX) {
            return Err(AceError::InvalidConfig("a schema has too many columns"));
        }
        let mut record_size = 0u32;
        for column in &columns {
            if !column.kind.accepts_width(column.width) {
                return Err(AceError::InvalidConfig("column width does not fit its kind"));
            }
            record_size = record_size
                .checked_add(u32::from(column.width))
                .ok_or(AceError::InvalidConfig("record size does not fit u32"))?;
        }
        Ok(Self {
            columns,
            record_size,
        })
    }

    /// The columns, in record order.
    pub fn columns(&self) -> &[ColumnSpec] {
        &self.columns
    }

    /// Bytes of one record: the sum of the widths.
    pub fn record_size(&self) -> u32 {
        self.record_size
    }

    /// Rounds `block_size` down to a whole number of records, and up to one record when it is
    /// smaller, so that blocks are cut at record boundaries and only the last block of a file
    /// can end with an incomplete record (`docs/FORMAT-1.6.md`).
    pub fn align_block_size(&self, block_size: usize) -> usize {
        let record = self.record_size as usize;
        match block_size.checked_div(record) {
            Some(records) if records > 0 => records.saturating_mul(record),
            _ => record,
        }
    }

    /// The column widths, in record order.
    pub fn widths(&self) -> Vec<u16> {
        self.columns.iter().map(|column| column.width).collect()
    }
}
