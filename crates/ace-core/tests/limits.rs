//! Tests of `ace_core` `limits` (formerly the inline unit tests of `src`).

use ace_core::*;

/// The block-count limit follows the index-entry limit.
#[test]
fn block_count_limit_follows_index_entries() {
    let limits = DecodeLimits {
        max_index_entries: 7,
        ..DecodeLimits::default()
    };
    assert_eq!(limits.max_block_count(), 7);
    assert_eq!(DecodeLimits::default().max_block_count(), 16_000_000);
}
