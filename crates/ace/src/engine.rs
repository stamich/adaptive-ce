//! Compression and decompression.

use std::io::{Read, Seek, Write};

use ace_engine::AceEngine;
use ace_stream::{compress_reader_known_size, StreamLimits};

use crate::{Config, DecodeLimits, Error, Result, Stats};

/// Compresses and decompresses ACE files.
///
/// An `Engine` holds a validated [`Config`] and the [`DecodeLimits`] applied when reading. It is
/// cheap to create, can be shared between threads and reused for any number of calls.
///
/// # Determinism
///
/// The same input and configuration always produce the same bytes, for every thread count and
/// CPU. Decompression returns exactly the original bytes or an [`Error`]; it never returns
/// altered data.
///
/// ```
/// let engine = ace::Engine::default();
/// let data = b"row,1,2,3\nrow,2,3,4\nrow,3,4,5\n".repeat(1000);
/// let packed = engine.compress(&data)?;
/// assert!(packed.len() < data.len());
/// assert_eq!(engine.decompress(&packed)?, data);
/// # Ok::<(), ace::Error>(())
/// ```
#[derive(Debug)]
pub struct Engine {
    config: Config,
    limits: DecodeLimits,
    inner: AceEngine,
}

impl Default for Engine {
    fn default() -> Self {
        Self {
            config: Config::default(),
            limits: DecodeLimits::default(),
            inner: AceEngine::default_engine(),
        }
    }
}

impl Engine {
    /// An engine with the given settings and the default decode limits.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::InvalidConfig`](crate::ErrorKind::InvalidConfig) when the settings are
    /// rejected by the engine (a [`Config`] built with its builder already passed this check).
    pub fn new(config: Config) -> Result<Self> {
        let inner = AceEngine::new(config.core().clone()).map_err(Error::from_core)?;
        Ok(Self {
            config,
            limits: DecodeLimits::default(),
            inner,
        })
    }

    /// Replaces the limits applied while decoding.
    #[must_use]
    pub fn with_decode_limits(mut self, limits: DecodeLimits) -> Self {
        self.inner = self.inner.with_decode_limits(limits.core().clone());
        self.limits = limits;
        self
    }

    #[cfg(feature = "explain")]
    pub(crate) fn internal(&self) -> &AceEngine {
        &self.inner
    }

    /// The settings this engine compresses with.
    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// The limits this engine decodes under.
    #[must_use]
    pub fn decode_limits(&self) -> &DecodeLimits {
        &self.limits
    }

    /// Compresses `input` into an ACE file in memory.
    ///
    /// # Errors
    ///
    /// Any [`Error`] raised while encoding; none for valid settings and ordinary input.
    pub fn compress(&self, input: &[u8]) -> Result<Vec<u8>> {
        self.inner.compress(input).map_err(Error::from_core)
    }

    /// Like [`compress`](Self::compress), also returning what the run did.
    ///
    /// # Errors
    ///
    /// As [`compress`](Self::compress).
    pub fn compress_with_stats(&self, input: &[u8]) -> Result<(Vec<u8>, Stats)> {
        let (bytes, stats) = self
            .inner
            .compress_with_stats(input)
            .map_err(Error::from_core)?;
        Ok((bytes, Stats::from_core(&stats)))
    }

    /// Compresses `input` and writes the ACE file to `writer`.
    ///
    /// # Errors
    ///
    /// As [`compress`](Self::compress), and [`ErrorKind::Io`](crate::ErrorKind::Io) when
    /// writing fails.
    pub fn compress_to<W: Write>(&self, input: &[u8], writer: W) -> Result<Stats> {
        let stats = self
            .inner
            .compress_to(input, writer)
            .map_err(Error::from_core)?;
        Ok(Stats::from_core(&stats))
    }

    /// Compresses exactly `size` bytes read from `reader` into `writer` using bounded memory:
    /// at most one block per worker is held, never the whole input. `writer` must be seekable
    /// because the header is completed at the end. The bytes equal [`compress`](Self::compress)
    /// of the same data.
    ///
    /// The configuration must use [`BlockSizePolicy::Fixed`](crate::BlockSizePolicy::Fixed):
    /// an automatic block size needs to see the whole input first.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::InvalidConfig`](crate::ErrorKind::InvalidConfig) for an automatic block
    /// size, [`ErrorKind::Corrupt`](crate::ErrorKind::Corrupt) when `reader` yields fewer or
    /// more than `size` bytes, [`ErrorKind::Io`](crate::ErrorKind::Io) for read or write
    /// failures.
    pub fn compress_stream<R: Read, W: Write + Seek>(
        &self,
        reader: R,
        writer: W,
        size: u64,
    ) -> Result<Stats> {
        let stats = compress_reader_known_size(
            reader,
            writer,
            size,
            self.config.core().clone(),
            StreamLimits::default(),
        )
        .map_err(Error::from_core)?;
        Ok(Stats {
            input_bytes: stats.input_bytes,
            output_bytes: stats.output_bytes,
            format_minor: if stats.format_1_4 { 4 } else { 3 },
            blocks: stats.blocks,
            ..Stats::default()
        })
    }

    /// Decompresses an ACE file held in memory.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::NotAceFile`](crate::ErrorKind::NotAceFile),
    /// [`ErrorKind::Unsupported`](crate::ErrorKind::Unsupported) (a newer format names the ACE
    /// release that reads it), [`ErrorKind::Corrupt`](crate::ErrorKind::Corrupt),
    /// [`ErrorKind::ChecksumMismatch`](crate::ErrorKind::ChecksumMismatch) or
    /// [`ErrorKind::LimitExceeded`](crate::ErrorKind::LimitExceeded).
    pub fn decompress(&self, input: &[u8]) -> Result<Vec<u8>> {
        self.inner.decompress(input).map_err(Error::from_core)
    }

    /// Decompresses into `out`, reusing its capacity, and returns the number of bytes. `out`
    /// is cleared first; after an error its contents are unspecified.
    ///
    /// # Errors
    ///
    /// As [`decompress`](Self::decompress).
    pub fn decompress_into(&self, input: &[u8], out: &mut Vec<u8>) -> Result<usize> {
        self.inner
            .decompress_into(input, out)
            .map_err(Error::from_core)
    }

    /// Decompresses a file read sequentially from `reader` and writes the original bytes to
    /// `writer`, holding one block at a time.
    ///
    /// # Errors
    ///
    /// As [`decompress`](Self::decompress), and [`ErrorKind::Io`](crate::ErrorKind::Io) for
    /// read or write failures.
    pub fn decompress_from<R: Read, W: Write>(&self, reader: R, writer: W) -> Result<()> {
        self.inner
            .decompress_from(reader, writer)
            .map_err(Error::from_core)
    }
}
