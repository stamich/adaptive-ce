//! ACE 0.6.1: the DENSE profile never stores a block, and therefore never a file, larger than
//! BALANCED does, on every corpus workload (including zero blocks and runs, where the DENSE
//! search used to settle on a larger plan) and on hostile inputs.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace_core::{AceConfig, CompressionProfile};
use ace_corpus::{FalsePositiveCase, Workload};
use ace_engine::AceEngine;

/// Single-threaded engine for `profile`.
fn engine(profile: CompressionProfile) -> AceEngine {
    AceEngine::new(AceConfig {
        profile,
        threads: 1,
        ..AceConfig::default()
    })
    .unwrap()
}

/// Asserts `dense <= balanced` and the round trip for `data`.
fn check(name: &str, data: &[u8]) {
    let balanced = engine(CompressionProfile::Balanced).compress(data).unwrap();
    let dense_engine = engine(CompressionProfile::Dense);
    let dense = dense_engine.compress(data).unwrap();
    assert!(
        dense.len() <= balanced.len(),
        "{name}: dense {} > balanced {}",
        dense.len(),
        balanced.len()
    );
    assert_eq!(dense_engine.decompress(&dense).unwrap(), data, "{name}");
}

#[test]
fn dense_is_never_larger_on_the_corpus() {
    for workload in Workload::ALL {
        check(workload.name(), &workload.generate(512 * 1024));
    }
}

#[test]
fn dense_is_never_larger_on_the_false_positive_corpus() {
    for case in FalsePositiveCase::ALL {
        check(case.name(), &case.generate(256 * 1024));
    }
}

#[test]
fn dense_is_never_larger_on_tiny_and_odd_inputs() {
    for len in [0usize, 1, 7, 100, 4_095, 70_001] {
        check("zeros", &vec![0u8; len]);
        let ramp: Vec<u8> = (0..len).map(|i| (i * 31 % 251) as u8).collect();
        check("ramp", &ramp);
    }
}
