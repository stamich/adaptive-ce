//! Random access to a compressed file.

use std::io::{Read, Seek};
use std::ops::Range;

use ace_engine::AceIndexedDecoder;

use crate::{DecodeLimits, Engine, Error, Result};

/// The work one range read did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct RangeStats {
    /// Bytes of the original file that were asked for.
    pub logical_bytes: u64,
    /// Bytes of the compressed file that had to be read for them.
    pub physical_bytes: u64,
    /// Independent blocks that were decoded.
    pub blocks: usize,
}

impl RangeStats {
    /// Compressed bytes read per original byte returned; `0.0` for an empty range.
    #[must_use]
    pub fn read_amplification(&self) -> f64 {
        if self.logical_bytes == 0 {
            0.0
        } else {
            self.physical_bytes as f64 / self.logical_bytes as f64
        }
    }
}

/// Reads byte ranges of an indexed ACE file without decompressing the rest of it.
///
/// Only the blocks that overlap the requested range are read and decoded, so the cost of a read
/// depends on the range and the block size, not on the file size. The file needs its block index,
/// which [`Config`](crate::Config) writes by default.
///
/// ```
/// let data: Vec<u8> = (0..200_000u32).flat_map(|i| i.to_le_bytes()).collect();
/// let engine = ace::Engine::new(ace::Config::builder().block_size(64 * 1024).build()?)?;
/// let packed = engine.compress(&data)?;
///
/// let mut reader = engine.open_range_reader(std::io::Cursor::new(packed))?;
/// assert_eq!(reader.len(), data.len() as u64);
/// assert_eq!(reader.read_range(1000..1100)?, data[1000..1100]);
/// # Ok::<(), ace::Error>(())
/// ```
pub struct RangeReader<R: Read + Seek> {
    inner: AceIndexedDecoder<R>,
}

impl<R: Read + Seek> std::fmt::Debug for RangeReader<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RangeReader")
            .field("len", &self.len())
            .field("block_count", &self.block_count())
            .finish()
    }
}

impl<R: Read + Seek> RangeReader<R> {
    /// Opens `reader` with the default [`DecodeLimits`]; the header, trailer and block index are
    /// validated.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::NotAceFile`](crate::ErrorKind::NotAceFile),
    /// [`ErrorKind::Corrupt`](crate::ErrorKind::Corrupt) (including a file without an index),
    /// [`ErrorKind::ChecksumMismatch`](crate::ErrorKind::ChecksumMismatch),
    /// [`ErrorKind::Unsupported`](crate::ErrorKind::Unsupported) or
    /// [`ErrorKind::LimitExceeded`](crate::ErrorKind::LimitExceeded).
    pub fn open(reader: R) -> Result<Self> {
        Self::open_with_limits(reader, &DecodeLimits::default())
    }

    /// Opens `reader` under the given limits.
    ///
    /// # Errors
    ///
    /// As [`open`](Self::open).
    pub fn open_with_limits(reader: R, limits: &DecodeLimits) -> Result<Self> {
        let inner =
            AceIndexedDecoder::open(reader, limits.core().clone()).map_err(Error::from_core)?;
        Ok(Self { inner })
    }

    /// Size of the original file in bytes.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.inner.original_size()
    }

    /// `true` when the original file is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Number of independent blocks.
    #[must_use]
    pub fn block_count(&self) -> usize {
        self.inner.block_count()
    }

    /// Returns the original bytes in `range`.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::InvalidConfig`](crate::ErrorKind::InvalidConfig) when the range is
    /// reversed or ends beyond [`len`](Self::len); the decoding errors of
    /// [`Engine::decompress`] for a damaged block.
    pub fn read_range(&mut self, range: Range<u64>) -> Result<Vec<u8>> {
        self.read_range_with_stats(range).map(|(bytes, _)| bytes)
    }

    /// Like [`read_range`](Self::read_range), also reporting the work done.
    ///
    /// # Errors
    ///
    /// As [`read_range`](Self::read_range).
    pub fn read_range_with_stats(&mut self, range: Range<u64>) -> Result<(Vec<u8>, RangeStats)> {
        if range.start > range.end || range.end > self.len() {
            return Err(Error::new(
                crate::ErrorKind::InvalidConfig,
                format!(
                    "range {}..{} lies outside the file of {} bytes",
                    range.start,
                    range.end,
                    self.len()
                ),
            ));
        }
        let (bytes, metrics) = self
            .inner
            .read_range_with_metrics(range)
            .map_err(Error::from_core)?;
        Ok((
            bytes,
            RangeStats {
                logical_bytes: metrics.logical_bytes_requested,
                physical_bytes: metrics.physical_bytes_read,
                blocks: metrics.blocks_decoded,
            },
        ))
    }

    /// Decodes one whole block by number.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::BlockNotFound`](crate::ErrorKind::BlockNotFound) when `block_id` is not in
    /// the file.
    pub fn read_block(&mut self, block_id: u64) -> Result<Vec<u8>> {
        self.inner.decode_block(block_id).map_err(Error::from_core)
    }
}

impl Engine {
    /// Opens a random-access reader over `reader` under this engine's decode limits.
    ///
    /// # Errors
    ///
    /// As [`RangeReader::open`].
    pub fn open_range_reader<R: Read + Seek>(&self, reader: R) -> Result<RangeReader<R>> {
        RangeReader::open_with_limits(reader, self.decode_limits())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::{Config, ErrorKind};

    fn file() -> (Vec<u8>, Vec<u8>, Engine) {
        let data: Vec<u8> = (0..120_000u32)
            .flat_map(|i| (i / 3).to_le_bytes())
            .collect();
        let engine = Engine::new(Config::builder().block_size(32 * 1024).build().unwrap()).unwrap();
        let packed = engine.compress(&data).unwrap();
        (data, packed, engine)
    }

    #[test]
    fn a_range_equals_the_slice_of_the_input() {
        let (data, packed, engine) = file();
        let mut r = engine.open_range_reader(Cursor::new(packed)).unwrap();
        assert_eq!(r.len(), data.len() as u64);
        assert!(r.block_count() > 3);
        for range in [
            0..0,
            0..1,
            5..40_000,
            32_767..32_769,
            100_000..data.len() as u64,
        ] {
            let want = &data[range.start as usize..range.end as usize];
            assert_eq!(r.read_range(range.clone()).unwrap(), want, "{range:?}");
        }
    }

    #[test]
    fn stats_count_only_the_blocks_touched() {
        let (_, packed, engine) = file();
        let mut r = engine
            .open_range_reader(Cursor::new(packed.clone()))
            .unwrap();
        let (bytes, stats) = r.read_range_with_stats(10..20).unwrap();
        assert_eq!(bytes.len(), 10);
        assert_eq!((stats.logical_bytes, stats.blocks), (10, 1));
        assert!(stats.physical_bytes > 0 && stats.physical_bytes < packed.len() as u64);
        assert!(stats.read_amplification() > 1.0);
        assert_eq!(RangeStats::default().read_amplification(), 0.0);
    }

    #[test]
    fn bad_ranges_and_blocks_are_rejected() {
        let (data, packed, engine) = file();
        let mut r = engine.open_range_reader(Cursor::new(packed)).unwrap();
        let end = data.len() as u64;
        assert_eq!(
            r.read_range(0..end + 1).unwrap_err().kind(),
            ErrorKind::InvalidConfig
        );
        #[allow(clippy::reversed_empty_ranges)]
        let reversed = 10..5;
        assert_eq!(
            r.read_range(reversed).unwrap_err().kind(),
            ErrorKind::InvalidConfig
        );
        let e = r.read_block(10_000).unwrap_err();
        assert_eq!(
            (e.kind(), e.block_id()),
            (ErrorKind::BlockNotFound, Some(10_000))
        );
        assert_eq!(r.read_block(0).unwrap(), data[..32 * 1024]);
    }

    #[test]
    fn foreign_input_and_missing_index_fail_to_open() {
        let e = RangeReader::open(Cursor::new(vec![b'x'; 64])).unwrap_err();
        assert_eq!(e.kind(), ErrorKind::NotAceFile);
        let engine = Engine::new(Config::builder().write_index(false).build().unwrap()).unwrap();
        let packed = engine.compress(&[1u8; 5000]).unwrap();
        assert!(RangeReader::open(Cursor::new(packed)).is_err());
    }
}
