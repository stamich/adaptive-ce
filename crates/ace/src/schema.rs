//! The record layout of columnar blocks.

use ace_core::{ColumnKind as CoreKind, ColumnSpec, Schema as CoreSchema};

use crate::{Error, Result};

/// What a column holds. The kind steers the codec ACE tries for the column; it never changes
/// how the file is decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ColumnKind {
    /// Opaque bytes of any width: identifiers, text, flags. Only the generic codecs are tried.
    Bytes,
    /// Unsigned little-endian integers of width 1, 2, 4 or 8: counters, ids, timestamps.
    Unsigned,
    /// Signed little-endian integers of width 1, 2, 4 or 8: deltas, offsets.
    Signed,
    /// IEEE-754 little-endian floats of width 4 or 8: measurements, prices.
    Float,
}

/// One field of a fixed-size record: how many bytes it takes and what it holds.
///
/// ```
/// use ace::{Column, ColumnKind};
///
/// let timestamp = Column::unsigned(8);
/// assert_eq!((timestamp.width(), timestamp.kind()), (8, ColumnKind::Unsigned));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Column {
    /// Bytes per record.
    width: u16,
    /// What the column holds.
    kind: ColumnKind,
}

impl Column {
    /// A column of opaque bytes, `width` bytes per record (non-zero).
    #[must_use]
    pub fn bytes(width: u16) -> Self {
        Self {
            width,
            kind: ColumnKind::Bytes,
        }
    }

    /// A column of unsigned little-endian integers; `width` must be 1, 2, 4 or 8.
    #[must_use]
    pub fn unsigned(width: u16) -> Self {
        Self {
            width,
            kind: ColumnKind::Unsigned,
        }
    }

    /// A column of signed little-endian integers; `width` must be 1, 2, 4 or 8.
    #[must_use]
    pub fn signed(width: u16) -> Self {
        Self {
            width,
            kind: ColumnKind::Signed,
        }
    }

    /// A column of IEEE-754 little-endian floats; `width` must be 4 or 8.
    #[must_use]
    pub fn float(width: u16) -> Self {
        Self {
            width,
            kind: ColumnKind::Float,
        }
    }

    /// Bytes per record.
    #[must_use]
    pub fn width(&self) -> u16 {
        self.width
    }

    /// What the column holds.
    #[must_use]
    pub fn kind(&self) -> ColumnKind {
        self.kind
    }
}

/// The layout of one record: a validated list of [`Column`]s. A record is the concatenation of
/// its columns, so a file that is a table of fixed-size records is described by listing the
/// fields. ACE then stores the file column by column, which usually compresses much better
/// than row by row. ACE never guesses a schema; without one, files are stored row-wise.
///
/// Blocks are cut at whole records (the configured block size is rounded to a multiple of the
/// record size), and a columnar block is stored only when it is smaller than the row-wise
/// block, so a wrong schema costs time but never space. Files with a columnar block are
/// Format 1.6 and need ACE 0.8.0 or later to read.
///
/// ```
/// use ace::{Column, Config, Schema};
///
/// // timestamp u64 | price f64 | id u32 | flags [u8; 4]
/// let schema = Schema::new([
///     Column::unsigned(8),
///     Column::float(8),
///     Column::unsigned(4),
///     Column::bytes(4),
/// ])?;
/// assert_eq!(schema.record_size(), 24);
/// let config = Config::builder().schema(schema).build()?;
/// assert_eq!(config.block_size() % 24, 0, "blocks hold whole records");
/// # Ok::<(), ace::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schema {
    inner: CoreSchema,
}

impl Schema {
    /// Validates the columns and builds the schema.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::InvalidConfig`](crate::ErrorKind::InvalidConfig) for no columns, a width the
    /// kind does not allow (integers 1, 2, 4 or 8 bytes, floats 4 or 8, bytes non-zero), or a
    /// record that does not fit in 32 bits.
    pub fn new(columns: impl IntoIterator<Item = Column>) -> Result<Self> {
        let specs = columns
            .into_iter()
            .map(|column| ColumnSpec {
                width: column.width,
                kind: match column.kind {
                    ColumnKind::Bytes => CoreKind::Bytes,
                    ColumnKind::Unsigned => CoreKind::Unsigned,
                    ColumnKind::Signed => CoreKind::Signed,
                    ColumnKind::Float => CoreKind::Float,
                },
            })
            .collect();
        CoreSchema::new(specs)
            .map(|inner| Self { inner })
            .map_err(Error::from_core)
    }

    /// The columns, in record order.
    #[must_use]
    pub fn columns(&self) -> Vec<Column> {
        self.inner
            .columns()
            .iter()
            .map(|spec| Column {
                width: spec.width,
                kind: match spec.kind {
                    CoreKind::Bytes => ColumnKind::Bytes,
                    CoreKind::Unsigned => ColumnKind::Unsigned,
                    CoreKind::Signed => ColumnKind::Signed,
                    CoreKind::Float => ColumnKind::Float,
                },
            })
            .collect()
    }

    /// Bytes of one record: the sum of the column widths.
    #[must_use]
    pub fn record_size(&self) -> u32 {
        self.inner.record_size()
    }

    pub(crate) fn from_core(inner: CoreSchema) -> Self {
        Self { inner }
    }

    pub(crate) fn core(&self) -> &CoreSchema {
        &self.inner
    }
}
