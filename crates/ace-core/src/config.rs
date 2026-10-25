use crate::{AccessHint, AceError, AceResult, BlockSizePolicy};

/// Planner profile that changes the relative importance of compressed size and runtime cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionProfile {
    /// Minimizes analysis and encoding CPU cost.
    Fast,
    /// Balances encoded size and CPU cost.
    Balanced,
    /// Prefers encoded size over encoder CPU cost.
    Dense,
}

/// Window and segment sizes of adaptive segmentation (Format 1.6, `SEG1`).
///
/// Segmentation is opt-in ([`AceConfig::segmentation`]): when set, the encoder cuts a block where
/// its character changes, plans every segment on its own and stores the segmented block only if
/// it is smaller than the unsegmented one by a margin (`docs/FORMAT-1.6.md`). All sizes are in
/// bytes; the minimum and the maximum must be multiples of the window, because a segment is a
/// whole number of windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentationConfig {
    /// Granularity of the classification (default 16 KiB).
    pub window_size: usize,
    /// Smallest segment produced, unless the block itself is shorter (default 64 KiB).
    pub min_segment_size: usize,
    /// Largest segment produced, unless `max_segments` forces larger ones (default 1 MiB).
    pub max_segment_size: usize,
    /// Most segments in one block (default 1 024, the `DecodeLimits` default). A reader with a
    /// smaller `DecodeLimits::max_segments` rejects blocks that exceed its own limit.
    pub max_segments: usize,
}

/// Defaults sized for the 256 KiB default block: sixteen windows, segments of 64 KiB and more.
impl Default for SegmentationConfig {
    fn default() -> Self {
        Self {
            window_size: 16 * 1024,
            min_segment_size: 64 * 1024,
            max_segment_size: 1024 * 1024,
            max_segments: 1024,
        }
    }
}

/// Inherent methods of [`SegmentationConfig`].
impl SegmentationConfig {
    /// Checks the relations the segmenter relies on.
    ///
    /// # Errors
    /// [`AceError::InvalidConfig`] when `window_size` is zero, the minimum or the maximum is not
    /// a non-zero multiple of the window, the maximum is smaller than twice the minimum (a
    /// segment cut in two must stay above the minimum), or fewer than two segments are allowed.
    pub fn validate(&self) -> AceResult<()> {
        let multiple =
            |size: usize| self.window_size != 0 && size != 0 && size.is_multiple_of(self.window_size);
        if !multiple(self.min_segment_size) || !multiple(self.max_segment_size) {
            return Err(AceError::InvalidConfig(
                "segment sizes must be non-zero multiples of the window size",
            ));
        }
        if self.max_segment_size / 2 < self.min_segment_size {
            return Err(AceError::InvalidConfig(
                "max segment size must be at least twice the min segment size",
            ));
        }
        if self.max_segments < 2 {
            return Err(AceError::InvalidConfig("at least two segments must be allowed"));
        }
        Ok(())
    }
}

/// Runtime configuration of the ACE encoder.
#[derive(Debug, Clone)]
pub struct AceConfig {
    /// Target size of each independent input block.
    pub block_size: usize,
    /// Cost profile used by the deterministic planner.
    pub profile: CompressionProfile,
    /// Minimum number of bytes a non-RAW plan must save after framing overhead.
    pub min_gain_bytes: usize,
    /// Enables conservative early selection of RAW for likely incompressible blocks.
    pub enable_early_raw: bool,
    /// Number of worker threads used for parallel block processing. Zero means host logical CPU count.
    pub threads: usize,
    /// Maximum number of encoded blocks allowed in the in-flight scheduling window.
    pub max_in_flight_blocks: usize,
    /// Soft memory budget used to cap in-flight block work.
    pub memory_budget_bytes: usize,
    /// Emits a serialized block index and trailer when true.
    pub write_index: bool,
    /// Enables deterministic planner fast paths inherited from ACE 0.3.
    pub enable_planner_fast_paths: bool,
    /// Controls whether the configured block size is fixed or chosen once per file.
    pub block_size_policy: BlockSizePolicy,
    /// Expected access pattern consulted by the automatic block-size advisor.
    pub access_hint: AccessHint,
    /// Enables ACE 0.4 schema-free numeric candidate generation.
    pub enable_numeric_specialization: bool,
    /// Enables the ACE 0.5 Float lane and TS1 candidates (Gorilla, RunDelta).
    ///
    /// `false` reproduces the 0.4.6 planner decisions and Format 1.3 output exactly.
    pub enable_float_specialization: bool,
    /// Adaptive segmentation (ACE 0.8, Format 1.6); `None` (the default) never segments, so the
    /// output keeps the bytes of 0.7.0.
    pub segmentation: Option<SegmentationConfig>,
}

/// Provides the documented default values of [`AceConfig`].
impl Default for AceConfig {
    /// Returns the recommended ACE 0.4 defaults.
    fn default() -> Self {
        Self {
            block_size: 256 * 1024,
            profile: CompressionProfile::Balanced,
            min_gain_bytes: 32,
            enable_early_raw: true,
            threads: 0,
            max_in_flight_blocks: 32,
            memory_budget_bytes: 256 * 1024 * 1024,
            write_index: true,
            enable_planner_fast_paths: true,
            block_size_policy: BlockSizePolicy::Fixed,
            access_hint: AccessHint::Balanced,
            enable_numeric_specialization: true,
            enable_float_specialization: true,
            segmentation: None,
        }
    }
}
