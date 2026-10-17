//! Resource limits applied while decoding.

use ace_core::DecodeLimits as CoreLimits;

/// Upper bounds a decoder enforces on everything a file declares.
///
/// A limit only ever rejects input (with [`ErrorKind::LimitExceeded`](crate::ErrorKind::LimitExceeded)),
/// before anything of the offending size is allocated; it never changes the decoded bytes. The
/// defaults are conservative and suit files up to 64 GiB. Lower them when decoding untrusted
/// input on a small machine.
///
/// ```
/// let limits = ace::DecodeLimits::builder()
///     .max_output_size(1 << 30)
///     .max_blocks(100_000)
///     .build();
/// assert_eq!(limits.max_output_size(), 1 << 30);
/// ```
#[derive(Debug, Clone, Default)]
pub struct DecodeLimits {
    inner: CoreLimits,
}

impl DecodeLimits {
    /// Starts a builder initialised with the defaults.
    #[must_use]
    pub fn builder() -> DecodeLimitsBuilder {
        DecodeLimitsBuilder {
            inner: CoreLimits::default(),
        }
    }

    /// Maximum total decoded output, in bytes.
    #[must_use]
    pub fn max_output_size(&self) -> u64 {
        self.inner.max_output_size
    }

    /// Maximum decoded size of one block, in bytes.
    #[must_use]
    pub fn max_block_size(&self) -> usize {
        self.inner.max_block_size
    }

    /// Maximum encoded size (metadata plus payload) of one block, in bytes.
    #[must_use]
    pub fn max_encoded_block_size(&self) -> usize {
        self.inner.max_encoded_block_size
    }

    /// Maximum number of transforms of one block.
    #[must_use]
    pub fn max_transforms(&self) -> usize {
        self.inner.max_transforms
    }

    /// Maximum embedded dictionary size, in bytes.
    #[must_use]
    pub fn max_dictionary_size(&self) -> usize {
        self.inner.max_dictionary_size
    }

    /// Maximum number of blocks (and of block-index entries) in a file.
    #[must_use]
    pub fn max_blocks(&self) -> usize {
        self.inner.max_index_entries
    }

    pub(crate) fn core(&self) -> &CoreLimits {
        &self.inner
    }
}

/// Builds [`DecodeLimits`]. Each setter replaces one default.
#[derive(Debug, Clone)]
pub struct DecodeLimitsBuilder {
    inner: CoreLimits,
}

impl DecodeLimitsBuilder {
    /// Maximum total decoded output (default 64 GiB).
    #[must_use]
    pub fn max_output_size(mut self, bytes: u64) -> Self {
        self.inner.max_output_size = bytes;
        self
    }

    /// Maximum decoded size of one block (default 4 MiB).
    #[must_use]
    pub fn max_block_size(mut self, bytes: usize) -> Self {
        self.inner.max_block_size = bytes;
        self
    }

    /// Maximum encoded size of one block (default 8 MiB).
    #[must_use]
    pub fn max_encoded_block_size(mut self, bytes: usize) -> Self {
        self.inner.max_encoded_block_size = bytes;
        self
    }

    /// Maximum number of transforms of one block (default 8).
    #[must_use]
    pub fn max_transforms(mut self, count: usize) -> Self {
        self.inner.max_transforms = count;
        self
    }

    /// Maximum embedded dictionary size (default 16 MiB).
    #[must_use]
    pub fn max_dictionary_size(mut self, bytes: usize) -> Self {
        self.inner.max_dictionary_size = bytes;
        self
    }

    /// Maximum number of blocks in a file (default 16 000 000).
    #[must_use]
    pub fn max_blocks(mut self, count: usize) -> Self {
        self.inner.max_index_entries = count;
        self
    }

    /// Finishes the limits.
    #[must_use]
    pub fn build(self) -> DecodeLimits {
        DecodeLimits { inner: self.inner }
    }
}
