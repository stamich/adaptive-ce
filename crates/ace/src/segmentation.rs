//! Adaptive segmentation settings.

use ace_core::SegmentationConfig;

/// How a block may be split into segments that are compressed on their own.
///
/// A block that mixes kinds of data (a log with an embedded blob, a table next to text) is
/// cut where the kind changes, and each segment gets the codec that suits it. A segmented block
/// is stored only when it is smaller than the unsegmented one by a margin, so enabling
/// segmentation costs time on uniform data but never space. Range reads then decode only the
/// segments they touch. Files with a segmented block are Format 1.6 and need ACE 0.8.0 or later.
///
/// The defaults (segments of 64 KiB to 1 MiB) suit the default 256 KiB block and larger blocks;
/// segmentation does nothing for blocks smaller than twice
/// [`min_segment_size`](Self::min_segment_size).
///
/// ```
/// use ace::{Config, Segmentation};
///
/// let config = Config::builder()
///     .block_size(4 << 20)
///     .segmentation(Segmentation::default().min_segment_size(128 * 1024))
///     .build()?;
/// assert_eq!(config.segmentation().unwrap().min_segment_size_bytes(), 128 * 1024);
/// # Ok::<(), ace::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segmentation {
    /// The validated core settings.
    inner: SegmentationConfig,
}

impl Default for Segmentation {
    /// 16 KiB windows, segments of 64 KiB to 1 MiB, at most 1 024 segments per block.
    fn default() -> Self {
        Self {
            inner: SegmentationConfig::default(),
        }
    }
}

impl Segmentation {
    /// Granularity of the classification, in bytes (default 16 KiB).
    #[must_use]
    pub fn window_size(mut self, bytes: usize) -> Self {
        self.inner.window_size = bytes;
        self
    }

    /// Shortest segment, in bytes (default 64 KiB). Shorter runs are merged into a neighbour.
    #[must_use]
    pub fn min_segment_size(mut self, bytes: usize) -> Self {
        self.inner.min_segment_size = bytes;
        self
    }

    /// Longest segment, in bytes (default 1 MiB). Longer runs are split evenly.
    #[must_use]
    pub fn max_segment_size(mut self, bytes: usize) -> Self {
        self.inner.max_segment_size = bytes;
        self
    }

    /// Most segments in one block (default 1 024, the default read limit).
    #[must_use]
    pub fn max_segments(mut self, count: usize) -> Self {
        self.inner.max_segments = count;
        self
    }

    /// The window size, in bytes.
    #[must_use]
    pub fn window_size_bytes(&self) -> usize {
        self.inner.window_size
    }

    /// The shortest segment, in bytes.
    #[must_use]
    pub fn min_segment_size_bytes(&self) -> usize {
        self.inner.min_segment_size
    }

    /// The longest segment, in bytes.
    #[must_use]
    pub fn max_segment_size_bytes(&self) -> usize {
        self.inner.max_segment_size
    }

    /// The most segments in one block.
    #[must_use]
    pub fn max_segments_per_block(&self) -> usize {
        self.inner.max_segments
    }

    pub(crate) fn core(&self) -> SegmentationConfig {
        self.inner
    }

    pub(crate) fn from_core(inner: SegmentationConfig) -> Self {
        Self { inner }
    }
}
