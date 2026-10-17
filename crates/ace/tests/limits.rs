//! Tests of `ace` `limits` (formerly the inline unit tests of `src`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use ace::internal::limits_core;
use ace::*;
use ace_core::DecodeLimits as CoreLimits;

#[test]
fn defaults_equal_the_internal_defaults() {
    let d = CoreLimits::default();
    let l = DecodeLimits::default();
    assert_eq!(l.max_output_size(), d.max_output_size);
    assert_eq!(l.max_block_size(), d.max_block_size);
    assert_eq!(l.max_encoded_block_size(), d.max_encoded_block_size);
    assert_eq!(l.max_transforms(), d.max_transforms);
    assert_eq!(l.max_dictionary_size(), d.max_dictionary_size);
    assert_eq!(l.max_blocks(), d.max_index_entries);
}

#[test]
fn every_limit_round_trips() {
    let l = DecodeLimits::builder()
        .max_output_size(11)
        .max_block_size(22)
        .max_encoded_block_size(33)
        .max_transforms(4)
        .max_dictionary_size(55)
        .max_blocks(66)
        .build();
    assert_eq!(l.max_output_size(), 11);
    assert_eq!(l.max_block_size(), 22);
    assert_eq!(l.max_encoded_block_size(), 33);
    assert_eq!(l.max_transforms(), 4);
    assert_eq!(l.max_dictionary_size(), 55);
    assert_eq!(l.max_blocks(), 66);
    assert_eq!(limits_core(&l).max_block_count(), 66);
}
