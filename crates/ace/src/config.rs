//! Compression settings.

use ace_core::{
    AccessHint as CoreAccess, AceConfig, BlockSizePolicy as CorePolicy, CompressionProfile,
    DictionaryScope,
};
use ace_engine::AceEngine;

use crate::{Dictionary, Error, Result, Schema, Segmentation};

/// How much encoder effort to spend to make the output smaller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Profile {
    /// Least analysis and encoding CPU cost.
    Fast,
    /// Balances size and CPU cost. The default.
    #[default]
    Balanced,
    /// Spends about a third more encoding time to search more candidates. It saves up to
    /// about 3 % on floating-point and low-cardinality data but is not guaranteed to be smaller
    /// than [`Profile::Balanced`] (see `docs/BENCHMARKS.md`).
    Dense,
}

/// Whether the block size is used as given or chosen once per file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum BlockSizePolicy {
    /// Use [`ConfigBuilder::block_size`] exactly. The default.
    #[default]
    Fixed,
    /// Choose one deterministic block size for the whole file, guided by [`AccessHint`].
    Auto,
}

/// The expected way the file will be read; consulted by [`BlockSizePolicy::Auto`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum AccessHint {
    /// Long sequential scans: favour throughput.
    Sequential,
    /// A mix of scans and range reads. The default.
    #[default]
    Balanced,
    /// Point and range reads: cap the block size to protect latency.
    RandomAccess,
}

/// Validated compression settings. Create with [`Config::builder`] or [`Config::default`].
#[derive(Debug, Clone, Default)]
pub struct Config {
    inner: AceConfig,
}

impl Config {
    /// Starts a builder initialised with the defaults.
    #[must_use]
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder {
            inner: AceConfig::default(),
        }
    }

    /// Target size of each independent block, in bytes. With a [`Schema`] this is rounded to a
    /// whole number of records.
    #[must_use]
    pub fn block_size(&self) -> usize {
        self.inner.aligned_block_size(self.inner.block_size)
    }

    /// The effort profile.
    #[must_use]
    pub fn profile(&self) -> Profile {
        match self.inner.profile {
            CompressionProfile::Fast => Profile::Fast,
            CompressionProfile::Balanced => Profile::Balanced,
            CompressionProfile::Dense => Profile::Dense,
        }
    }

    /// Worker threads; zero means one per logical CPU.
    #[must_use]
    pub fn threads(&self) -> usize {
        self.inner.threads
    }

    /// The record layout of columnar blocks, if one was set.
    #[must_use]
    pub fn schema(&self) -> Option<Schema> {
        self.inner.schema.clone().map(Schema::from_core)
    }

    /// The segmentation settings, if segmentation is enabled.
    #[must_use]
    pub fn segmentation(&self) -> Option<Segmentation> {
        self.inner.segmentation.map(Segmentation::from_core)
    }

    /// The preset dictionary, if one was set.
    #[must_use]
    pub fn dictionary(&self) -> Option<Dictionary> {
        self.inner.dictionary.as_ref().map(Dictionary::from_core)
    }

    /// `true` when the dictionary is stored in the file, `false` when only its id is stored
    /// (and readers need [`Engine::with_dictionaries`](crate::Engine::with_dictionaries)). Also
    /// `false` when there is no dictionary.
    #[must_use]
    pub fn dictionary_is_embedded(&self) -> bool {
        self.inner
            .dictionary
            .as_ref()
            .is_some_and(|dictionary| dictionary.scope == DictionaryScope::File)
    }

    /// Whether a block index is written (needed for random access).
    #[must_use]
    pub fn write_index(&self) -> bool {
        self.inner.write_index
    }

    pub(crate) fn core(&self) -> &AceConfig {
        &self.inner
    }
}

/// Builds a [`Config`]. Every setter is optional; [`build`](Self::build) validates the result.
#[derive(Debug, Clone)]
pub struct ConfigBuilder {
    inner: AceConfig,
}

impl ConfigBuilder {
    /// Target size of each independent block (default 256 KiB). Must be non-zero and fit `u32`.
    #[must_use]
    pub fn block_size(mut self, bytes: usize) -> Self {
        self.inner.block_size = bytes;
        self
    }

    /// Encoder effort (default [`Profile::Balanced`]).
    #[must_use]
    pub fn profile(mut self, profile: Profile) -> Self {
        self.inner.profile = match profile {
            Profile::Fast => CompressionProfile::Fast,
            Profile::Dense => CompressionProfile::Dense,
            Profile::Balanced => CompressionProfile::Balanced,
        };
        self
    }

    /// Worker threads; zero (the default) means one per logical CPU. Output is identical
    /// for every thread count.
    #[must_use]
    pub fn threads(mut self, threads: usize) -> Self {
        self.inner.threads = threads;
        self
    }

    /// Soft cap on the memory used by blocks in flight (default 256 MiB).
    #[must_use]
    pub fn memory_budget(mut self, bytes: usize) -> Self {
        self.inner.memory_budget_bytes = bytes;
        self
    }

    /// Writes the block index and trailer (default `true`). Without it the file can only be
    /// decoded sequentially.
    #[must_use]
    pub fn write_index(mut self, write: bool) -> Self {
        self.inner.write_index = write;
        self
    }

    /// Fixed or automatic block size (default [`BlockSizePolicy::Fixed`]).
    #[must_use]
    pub fn block_size_policy(mut self, policy: BlockSizePolicy) -> Self {
        self.inner.block_size_policy = match policy {
            BlockSizePolicy::Fixed => CorePolicy::Fixed,
            BlockSizePolicy::Auto => CorePolicy::Auto,
        };
        self
    }

    /// Expected access pattern (default [`AccessHint::Balanced`]).
    #[must_use]
    pub fn access_hint(mut self, hint: AccessHint) -> Self {
        self.inner.access_hint = match hint {
            AccessHint::Sequential => CoreAccess::Sequential,
            AccessHint::Balanced => CoreAccess::Balanced,
            AccessHint::RandomAccess => CoreAccess::RandomAccess,
        };
        self
    }

    /// Enables the integer pipelines (FOR, delta, delta-of-delta; default `true`).
    #[must_use]
    pub fn numeric_specialization(mut self, enabled: bool) -> Self {
        self.inner.enable_numeric_specialization = enabled;
        self
    }

    /// Enables the floating-point and time-series pipelines (default `true`). `false`
    /// reproduces the planner decisions of ACE 0.4.6 and always writes format 1.3.
    #[must_use]
    pub fn float_specialization(mut self, enabled: bool) -> Self {
        self.inner.enable_float_specialization = enabled;
        self
    }

    /// Stores the file as columns of fixed-size records (default: row-wise). The block size is
    /// rounded to a whole number of records, and a columnar block is written only when it is
    /// smaller than the row-wise block. See [`Schema`].
    #[must_use]
    pub fn schema(mut self, schema: Schema) -> Self {
        self.inner.schema = Some(schema.core().clone());
        self
    }

    /// Splits blocks that mix kinds of data into segments that get their own codecs (default:
    /// no segmentation). A segmented block is stored only when it is smaller than the
    /// unsegmented one by a margin. See [`Segmentation`].
    #[must_use]
    pub fn segmentation(mut self, segmentation: Segmentation) -> Self {
        self.inner.segmentation = Some(segmentation.core());
        self
    }

    /// Uses `dictionary` and **embeds it in the file** when that pays (default: no
    /// dictionary). The file is then self-contained; the dictionary is stored once, only if the
    /// blocks save more than it costs. Use a small dictionary (1 – 4 KiB) for small files, or
    /// [`external_dictionary`](Self::external_dictionary).
    #[must_use]
    pub fn dictionary(mut self, dictionary: Dictionary) -> Self {
        self.inner.dictionary = Some(dictionary.to_core(DictionaryScope::File));
        self
    }

    /// Uses `dictionary` but stores **only its id** in the file. Whoever reads the file needs
    /// the same dictionary ([`Engine::with_dictionaries`](crate::Engine::with_dictionaries)),
    /// otherwise decoding fails with
    /// [`ErrorKind::Unsupported`](crate::ErrorKind::Unsupported). This is the right choice when
    /// many small files share one dictionary.
    #[must_use]
    pub fn external_dictionary(mut self, dictionary: Dictionary) -> Self {
        self.inner.dictionary = Some(dictionary.to_core(DictionaryScope::External));
        self
    }

    /// Validates the settings.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::InvalidConfig`](crate::ErrorKind::InvalidConfig) when a value is out of
    /// range, for example a zero block size.
    pub fn build(self) -> Result<Config> {
        AceEngine::new(self.inner.clone()).map_err(Error::from_core)?;
        Ok(Config { inner: self.inner })
    }
}
