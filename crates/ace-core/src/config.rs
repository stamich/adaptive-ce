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
}

impl Default for AceConfig {
    /// Returns the recommended ACE 0.2.1-buildfix1 defaults.
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
        }
    }
}
