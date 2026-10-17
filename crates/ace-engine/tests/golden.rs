//! Golden-file checks: the semantic guards of ACE 0.6.x.
//!
//! For every `ace-corpus` workload and every profile, the SHA-256 of the input, of the
//! compressed container and of the decoded bytes must equal the committed golden file.
//! A mismatch means a change altered planner decisions or encoded bytes (a *semantic* change).
//!
//! * `examples/golden/0.6.1/GOLDEN.json` — the reference of ACE 0.6.1: Corpus V3 + V4
//!   (31 workloads) with the container's format version (1.3 to 1.5).
//! * `examples/golden/0.5.0/GOLDEN.json` — frozen (also the reference of 0.5.1 and 0.6.0).
//!   0.6.1 changes bytes on purpose for the workloads in [`CHANGED_SINCE_0_5_0`]; every other
//!   workload must still reproduce these hashes, and the changed ones may only get smaller.
//! * `examples/golden/0.4.6/GOLDEN.json` — frozen: the Corpus V3 workloads outside
//!   [`CHANGED_SINCE_0_5_0`] must still produce the 0.4.6 bytes.
//! * `ACE_GOLDEN_UPDATE=1 cargo test -p ace-engine --test golden` rewrites the 0.6.1 file
//!   (only for intentional, documented semantic changes; the frozen files are never written).
//! * `ACE_SIMD=scalar cargo test -p ace-engine --test golden` proves that the portable code
//!   paths produce the same bytes as the accelerated ones.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::path::PathBuf;

use ace_core::{AceConfig, CompressionProfile};
use ace_corpus::Workload;
use ace_engine::AceEngine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Workloads whose bytes ACE 0.6.1 changes on purpose, with the feature that changes them.
///
/// The frozen 0.5.0 and 0.4.6 files cannot be regenerated, so this list is the only place where
/// a mismatch with them is accepted. Each workload listed must really differ (the list cannot
/// go stale) and must not grow.
const CHANGED_SINCE_0_5_0: &[(&str, &str)] = &[
    (
        "gauge-sawtooth",
        "Patched FOR: the wrap of the sawtooth is one exception (Format 1.5)",
    ),
    (
        "monotonic-outliers",
        "Patched FOR: the periodic jump is an exception (Format 1.5)",
    ),
    (
        "delta-variable",
        "Patched FOR: narrower slots plus exceptions (Format 1.5)",
    ),
    (
        "int-counter-reset",
        "Patched FOR: the reset is an exception (Format 1.5)",
    ),
    (
        "f64-financial-price",
        "ALP: prices with two decimals become integers (Format 1.5)",
    ),
];

/// Workloads added after 0.5.0 (Corpus V5): they are not in the frozen files and are guarded by
/// the 0.6.1 file only.
const NEW_SINCE_0_5_0: &[&str] = &[
    "int-slow-drift",
    "int-geometric-delta",
    "f64-decimal-price",
    "f32-decimal-sensor",
];

/// Bytes generated per workload (four 256 KiB blocks).
const WORKLOAD_BYTES: usize = 1024 * 1024;

/// Profiles covered by the golden file.
const PROFILES: [(&str, CompressionProfile); 3] = [
    ("fast", CompressionProfile::Fast),
    ("balanced", CompressionProfile::Balanced),
    ("dense", CompressionProfile::Dense),
];

/// Root document of `GOLDEN.json`.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct GoldenFile {
    /// Document schema identifier.
    schema: String,
    /// Milestone that produced the hashes.
    milestone: String,
    /// Input size of every workload.
    bytes_per_workload: usize,
    /// One entry per workload.
    entries: Vec<GoldenEntry>,
}

/// Hashes of one workload.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct GoldenEntry {
    /// `ace-corpus` workload name.
    workload: String,
    /// SHA-256 of the generated input (guards the generator itself).
    input_sha256: String,
    /// Per-profile container hashes, in [`PROFILES`] order.
    profiles: Vec<GoldenProfile>,
    /// SHA-256 of the decoded bytes (must equal `input_sha256`).
    decoded_sha256: String,
}

/// Container hash for one profile.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct GoldenProfile {
    /// Profile name.
    profile: String,
    /// SHA-256 of the `.ace` container.
    ace_sha256: String,
    /// Container size in bytes.
    ace_bytes: usize,
    /// Declared format minor version (0.5.0 files only; absent in the 0.4.6 file).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    format_minor: Option<u8>,
}

/// Lower-case hex SHA-256.
fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Location of the golden file of `milestone` in the repository.
fn golden_path(milestone: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../examples/golden/{milestone}/GOLDEN.json"))
}

/// Computes a golden document for `workloads` from the current code.
///
/// `with_format` records the container's format minor version (the 0.5.0 schema).
fn compute(milestone: &str, workloads: &[Workload], with_format: bool) -> GoldenFile {
    let entries = workloads
        .iter()
        .map(|&workload| {
            let input = workload.generate(WORKLOAD_BYTES);
            let mut decoded_sha256 = String::new();
            let profiles = PROFILES
                .iter()
                .map(|&(name, profile)| {
                    let engine = AceEngine::new(AceConfig {
                        profile,
                        ..AceConfig::default()
                    })
                    .unwrap();
                    let encoded = engine.compress(&input).unwrap();
                    decoded_sha256 = sha256_hex(&engine.decompress(&encoded).unwrap());
                    GoldenProfile {
                        profile: name.into(),
                        ace_sha256: sha256_hex(&encoded),
                        ace_bytes: encoded.len(),
                        format_minor: with_format.then(|| encoded[5]),
                    }
                })
                .collect();
            GoldenEntry {
                workload: workload.name().into(),
                input_sha256: sha256_hex(&input),
                profiles,
                decoded_sha256,
            }
        })
        .collect();
    GoldenFile {
        schema: "ace-golden-1".into(),
        milestone: milestone.into(),
        bytes_per_workload: WORKLOAD_BYTES,
        entries,
    }
}

/// Asserts round-trips and compares `actual` with the committed file at `path`.
fn assert_matches(actual: &GoldenFile, path: &PathBuf) {
    for entry in &actual.entries {
        assert_eq!(
            entry.decoded_sha256, entry.input_sha256,
            "{} does not round-trip",
            entry.workload
        );
    }
    let expected: GoldenFile =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    for (want, got) in expected.entries.iter().zip(&actual.entries) {
        assert_eq!(want, got, "golden mismatch for workload {}", want.workload);
    }
    assert_eq!(&expected, actual);
}

/// Every 0.6.1 hash matches the committed golden file (or rewrites it in update mode).
#[test]
fn golden_hashes_match() {
    let actual = compute("0.6.1", &Workload::ALL, true);
    let path = golden_path("0.6.1");
    if std::env::var("ACE_GOLDEN_UPDATE").is_ok_and(|value| value == "1") {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, serde_json::to_string_pretty(&actual).unwrap() + "\n").unwrap();
        return;
    }
    assert_matches(&actual, &path);
}

/// Compares `actual` with a frozen file: unchanged workloads must match it exactly; workloads of
/// [`CHANGED_SINCE_0_5_0`] must differ, round-trip and not be larger in any profile.
fn assert_frozen(actual: &GoldenFile, path: &PathBuf) {
    let expected: GoldenFile =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    for got in &actual.entries {
        assert_eq!(got.decoded_sha256, got.input_sha256, "{}", got.workload);
        let Some(want) = expected
            .entries
            .iter()
            .find(|entry| entry.workload == got.workload)
        else {
            assert!(
                NEW_SINCE_0_5_0.contains(&got.workload.as_str()),
                "{} is missing from {}",
                got.workload,
                path.display()
            );
            continue;
        };
        assert_eq!(want.input_sha256, got.input_sha256, "{}", got.workload);
        if CHANGED_SINCE_0_5_0
            .iter()
            .any(|(name, _)| *name == got.workload)
        {
            assert_ne!(
                want, got,
                "{} is listed as changed but is not",
                got.workload
            );
            for (old, new) in want.profiles.iter().zip(&got.profiles) {
                assert!(
                    new.ace_bytes <= old.ace_bytes,
                    "{} {} grew: {} -> {}",
                    got.workload,
                    new.profile,
                    old.ace_bytes,
                    new.ace_bytes
                );
            }
        } else {
            let mut got = got.profiles.iter().map(|p| p.ace_sha256.clone());
            for old in &want.profiles {
                assert_eq!(
                    got.next().as_ref(),
                    Some(&old.ace_sha256),
                    "golden mismatch for workload {} {}",
                    want.workload,
                    old.profile
                );
            }
        }
    }
}

/// Workloads 0.6.1 does not touch still equal the frozen 0.5.0 file, byte for byte.
#[test]
fn unchanged_workloads_match_0_5_0_golden() {
    assert_frozen(
        &compute("0.6.1", &Workload::ALL, true),
        &golden_path("0.5.0"),
    );
}

/// Corpus V3 workloads 0.6.1 does not touch still equal the frozen 0.4.6 file.
#[test]
fn corpus_v3_matches_0_4_6_golden() {
    assert_frozen(
        &compute("0.6.1", &Workload::CORPUS_V3, false),
        &golden_path("0.4.6"),
    );
}
