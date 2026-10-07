//! Reusable per-worker buffers.

/// Reusable per-worker buffers reserved for ACE 0.3 codec and transform work.
///
/// The current milestone exposes this structure as the stable allocation-reuse boundary.  Codec
/// implementations can progressively adopt it without changing file format or planner semantics.
#[derive(Debug)]
pub struct WorkerScratch {
    /// Reusable transform output buffer.
    pub transform_buffer: Vec<u8>,
    /// Reusable primary codec buffer.
    pub codec_buffer: Vec<u8>,
    /// Reusable entropy output buffer.
    pub entropy_buffer: Vec<u8>,
    /// Reusable byte histogram storage.
    pub histogram: [u32; 256],
}

/// Provides the documented default values of [`WorkerScratch`].
impl Default for WorkerScratch {
    /// Creates an empty scratch workspace with a zero-initialized histogram.
    ///
    /// This implementation is explicit instead of derived so ACE remains compatible with Rust
    /// toolchains where `Default` is not implemented generically for arrays larger than 32 items.
    fn default() -> Self {
        Self {
            transform_buffer: Vec::new(),
            codec_buffer: Vec::new(),
            entropy_buffer: Vec::new(),
            histogram: [0; 256],
        }
    }
}

/// Inherent methods of [`WorkerScratch`].
impl WorkerScratch {
    /// Creates buffers with capacity sized for a typical independent ACE block.
    pub fn for_block_size(block_size: usize) -> Self {
        Self {
            transform_buffer: Vec::with_capacity(block_size),
            codec_buffer: Vec::with_capacity(block_size),
            entropy_buffer: Vec::with_capacity(block_size),
            histogram: [0; 256],
        }
    }

    /// Clears logical contents while retaining allocations for the next block.
    pub fn reset(&mut self) {
        self.transform_buffer.clear();
        self.codec_buffer.clear();
        self.entropy_buffer.clear();
        self.histogram = [0; 256];
    }

    /// Returns total currently reserved byte capacity of the reusable vector buffers.
    pub fn reserved_bytes(&self) -> usize {
        self.transform_buffer
            .capacity()
            .saturating_add(self.codec_buffer.capacity())
            .saturating_add(self.entropy_buffer.capacity())
            .saturating_add(std::mem::size_of_val(&self.histogram))
    }
}

#[cfg(test)]
mod tests {
    use super::WorkerScratch;

    /// Verifies the manual default implementation zeroes histogram storage and starts with empty buffers.
    #[test]
    fn worker_scratch_default_is_empty_and_zeroed() {
        let scratch = WorkerScratch::default();
        assert!(scratch.transform_buffer.is_empty());
        assert!(scratch.codec_buffer.is_empty());
        assert!(scratch.entropy_buffer.is_empty());
        assert!(scratch.histogram.iter().all(|&value| value == 0));
    }
}
