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

/// The Format 1.6 limits have conservative defaults and are independent of each other.
#[test]
fn storage_limits_have_documented_defaults() {
    let limits = DecodeLimits::default();
    assert_eq!(limits.max_segments, 1024);
    assert_eq!(limits.max_columns, 256);
    assert_eq!(limits.max_dictionaries, 16);
    assert_eq!(limits.max_dictionary_size, 16 * 1024 * 1024);
    let tight = DecodeLimits {
        max_segments: 2,
        ..DecodeLimits::default()
    };
    assert_eq!((tight.max_segments, tight.max_columns), (2, 256));
}

/// The format version constants agree: 1.6 is the highest readable minor and the first with
/// container blocks and the dictionary section.
#[test]
fn format_constants_agree() {
    assert_eq!(FORMAT_MAJOR, 1);
    assert_eq!(FORMAT_MINOR, 6);
    assert_eq!(FORMAT_MINOR_STORAGE, 6);
}
