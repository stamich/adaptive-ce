//! Compression settings.

use ace_core::{
    AccessHint as CoreAccess, AceConfig, BlockSizePolicy as CorePolicy, CompressionProfile,
};
use ace_engine::AceEngine;

use crate::{Error, Result};

/// How much encoder effort to spend to make the output smaller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Profile {
    /// Least analysis and encoding CPU cost.
    Fast,
    /// Balances size and CPU cost. The default.
    #[default]
    Balanced,
    /// Prefers the smallest output over encoder CPU cost.
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

    /// Target size of each independent block, in bytes.
    #[must_use]
    pub fn block_size(&self) -> usize {
        self.inner.block_size
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

    /// Enables the integer and float specialisations (default `true`). `false` makes the
    /// planner use only the general-purpose pipelines.
    #[must_use]
    pub fn numeric_specialization(mut self, enabled: bool) -> Self {
        self.inner.enable_numeric_specialization = enabled;
        self.inner.enable_float_specialization = enabled;
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::ErrorKind;

    #[test]
    fn defaults_equal_the_engine_defaults() {
        let c = Config::default();
        let d = AceConfig::default();
        assert_eq!(c.block_size(), d.block_size);
        assert_eq!(c.profile(), Profile::Balanced);
        assert_eq!(c.threads(), d.threads);
        assert!(c.write_index());
        let b = Config::builder().build().unwrap();
        assert_eq!(b.core().memory_budget_bytes, d.memory_budget_bytes);
        assert_eq!(b.core().block_size_policy, d.block_size_policy);
        assert_eq!(b.core().access_hint, d.access_hint);
    }

    #[test]
    fn setters_reach_the_engine_config() {
        let c = Config::builder()
            .block_size(64 * 1024)
            .profile(Profile::Dense)
            .threads(2)
            .write_index(false)
            .block_size_policy(BlockSizePolicy::Auto)
            .access_hint(AccessHint::RandomAccess)
            .numeric_specialization(false)
            .build()
            .unwrap();
        assert_eq!(c.block_size(), 64 * 1024);
        assert_eq!(c.profile(), Profile::Dense);
        assert_eq!(c.threads(), 2);
        assert!(!c.write_index());
        assert_eq!(c.core().block_size_policy, CorePolicy::Auto);
        assert_eq!(c.core().access_hint, CoreAccess::RandomAccess);
        assert!(!c.core().enable_numeric_specialization);
        assert!(!c.core().enable_float_specialization);
    }

    #[test]
    fn invalid_values_are_rejected() {
        let e = Config::builder().block_size(0).build().unwrap_err();
        assert_eq!(e.kind(), ErrorKind::InvalidConfig);
        let e = Config::builder()
            .block_size(usize::MAX)
            .build()
            .unwrap_err();
        assert_eq!(e.kind(), ErrorKind::InvalidConfig);
    }
}
