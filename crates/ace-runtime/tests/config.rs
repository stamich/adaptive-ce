//! Tests of `ace_runtime` `config` (formerly the inline unit tests of `src`).

use ace_core::AceConfig;
use ace_runtime::*;

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
