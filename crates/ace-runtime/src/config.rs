//! Validated runtime settings derived from `AceConfig`.

use ace_core::{AceConfig, AceError, AceResult};

/// Resolved runtime settings used by the ACE engine after validating user configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeConfig {
    /// Number of Rayon worker threads.
    pub threads: usize,
    /// Maximum number of blocks allowed in a scheduling batch.
    pub max_in_flight_blocks: usize,
    /// Soft memory budget used to derive the effective scheduling batch.
    pub memory_budget_bytes: usize,
}

/// Inherent methods of [`RuntimeConfig`].
impl RuntimeConfig {
    /// Resolves zero/automatic values and validates memory/in-flight limits against block size.
    pub fn from_ace(config: &AceConfig) -> AceResult<Self> {
        if config.block_size == 0 {
            return Err(AceError::InvalidConfig("block size must be non-zero"));
        }
        let threads = if config.threads == 0 {
            num_cpus::get().max(1)
        } else {
            config.threads.max(1)
        };
        let block_working_set = config.block_size.saturating_mul(4).max(1);
        let memory_limited = (config.memory_budget_bytes / block_working_set).max(1);
        let max_in_flight_blocks = config.max_in_flight_blocks.max(1).min(memory_limited);
        Ok(Self {
            threads,
            max_in_flight_blocks,
            memory_budget_bytes: config.memory_budget_bytes,
        })
    }

    /// Builds an isolated Rayon thread pool so ACE does not mutate the process-global pool.
    pub fn build_pool(&self) -> AceResult<rayon::ThreadPool> {
        rayon::ThreadPoolBuilder::new()
            .num_threads(self.threads)
            .build()
            .map_err(|_| AceError::InvalidConfig("failed to create Rayon thread pool"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Zero threads resolve to the host CPU count; memory budget caps in-flight blocks.
    #[test]
    fn resolves_automatic_values_and_memory_cap() {
        let mut config = AceConfig {
            threads: 0,
            block_size: 1 << 20,
            memory_budget_bytes: 8 << 20,
            max_in_flight_blocks: 64,
            ..AceConfig::default()
        };
        let runtime = RuntimeConfig::from_ace(&config).unwrap();
        assert!(runtime.threads >= 1);
        assert_eq!(
            runtime.max_in_flight_blocks, 2,
            "8 MiB / (4 x 1 MiB working set)"
        );
        config.block_size = 0;
        assert!(RuntimeConfig::from_ace(&config).is_err());
    }
}
