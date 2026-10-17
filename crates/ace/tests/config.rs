//! Tests of `ace` `config` (formerly the inline unit tests of `src`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use ace::internal::config_core;
use ace::*;
use ace_core::{AccessHint as CoreAccess, AceConfig, BlockSizePolicy as CorePolicy};

#[test]
fn defaults_equal_the_engine_defaults() {
    let c = Config::default();
    let d = AceConfig::default();
    assert_eq!(c.block_size(), d.block_size);
    assert_eq!(c.profile(), Profile::Balanced);
    assert_eq!(c.threads(), d.threads);
    assert!(c.write_index());
    let b = Config::builder().build().unwrap();
    assert_eq!(config_core(&b).memory_budget_bytes, d.memory_budget_bytes);
    assert_eq!(config_core(&b).block_size_policy, d.block_size_policy);
    assert_eq!(config_core(&b).access_hint, d.access_hint);
}

#[test]
fn setters_reach_the_engine_config() {
    let c = Config::builder()
        .block_size(64 * 1024)
        .profile(Profile::Dense)
        .threads(2)
        .write_index(false)
        .block_size_policy(BlockSizePolicy::Auto)
        .access_hint(AccessHint::RandomAccess)
        .numeric_specialization(false)
        .float_specialization(false)
        .build()
        .unwrap();
    assert_eq!(c.block_size(), 64 * 1024);
    assert_eq!(c.profile(), Profile::Dense);
    assert_eq!(c.threads(), 2);
    assert!(!c.write_index());
    assert_eq!(config_core(&c).block_size_policy, CorePolicy::Auto);
    assert_eq!(config_core(&c).access_hint, CoreAccess::RandomAccess);
    assert!(!config_core(&c).enable_numeric_specialization);
    assert!(!config_core(&c).enable_float_specialization);
}

#[test]
fn invalid_values_are_rejected() {
    let e = Config::builder().block_size(0).build().unwrap_err();
    assert_eq!(e.kind(), ErrorKind::InvalidConfig);
    let e = Config::builder()
        .block_size(usize::MAX)
        .build()
        .unwrap_err();
    assert_eq!(e.kind(), ErrorKind::InvalidConfig);
}
