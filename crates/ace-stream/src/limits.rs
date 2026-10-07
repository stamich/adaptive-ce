//! Streaming resource limits and telemetry.

/// Resource limits applied to a bounded streaming operation.
#[derive(Debug, Clone, Copy)]
pub struct StreamLimits {
    /// Maximum declared input size accepted by the encoder.
    pub max_input_bytes: u64,
    /// Maximum number of independent blocks accepted by one stream.
    pub max_blocks: u64,
    /// Maximum bytes retained for one input block plus its local encoded representation.
    pub max_in_flight_bytes: usize,
}

/// Documented default values of [`StreamLimits`].
impl Default for StreamLimits {
    /// Conservative limits suitable for normal CLI and benchmark use.
    fn default() -> Self {
        Self {
            max_input_bytes: 1 << 40,
            max_blocks: 16_777_216,
            max_in_flight_bytes: 64 * 1024 * 1024,
        }
    }
}

/// Aggregate telemetry produced by the bounded streaming encoder.
#[derive(Debug, Clone, Copy, Default)]
pub struct StreamingStats {
    /// Source bytes consumed from the reader.
    pub input_bytes: u64,
    /// ACE bytes written to the sink.
    pub output_bytes: u64,
    /// Number of blocks processed.
    pub blocks: u64,
    /// Maximum source block bytes resident at one time.
    pub peak_source_buffer_bytes: usize,
}
