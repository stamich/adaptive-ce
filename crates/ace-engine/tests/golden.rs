//! Golden-file check: the semantic-freeze guard of ACE 0.4.6.
//!
//! For every `ace-corpus` workload and every profile, the SHA-256 of the input, of the
//! compressed container and of the decoded bytes must equal `examples/golden/0.4.6/GOLDEN.json`.
//! A mismatch means a change altered planner decisions or encoded bytes (a *semantic* change).
//!
//! * `ACE_GOLDEN_UPDATE=1 cargo test -p ace-engine --test golden` rewrites the file (only for
//!   intentional, documented semantic changes).
//! * `ACE_SIMD=scalar cargo test -p ace-engine --test golden` proves that the portable code
//!   paths produce the same bytes as the accelerated ones.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::path::PathBuf;

use ace_core::{AceConfig, CompressionProfile};
use ace_corpus::Workload;
use ace_engine::AceEngine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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
}

/// Lower-case hex SHA-256.
fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Location of the golden file in the repository.
fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/golden/0.4.6/GOLDEN.json")
}

/// Computes the golden document from the current code.
fn compute() -> GoldenFile {
    let entries = Workload::CORPUS_V3
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
        milestone: "0.4.6".into(),
        bytes_per_workload: WORKLOAD_BYTES,
        entries,
    }
}

/// Every hash matches the committed golden file (or rewrites it in update mode).
#[test]
fn golden_hashes_match() {
    let actual = compute();
    for entry in &actual.entries {
        assert_eq!(
            entry.decoded_sha256, entry.input_sha256,
            "{} does not round-trip",
            entry.workload
        );
    }
    let path = golden_path();
    if std::env::var("ACE_GOLDEN_UPDATE").is_ok_and(|value| value == "1") {
        std::fs::write(&path, serde_json::to_string_pretty(&actual).unwrap() + "\n").unwrap();
        return;
    }
    let expected: GoldenFile =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    for (want, got) in expected.entries.iter().zip(&actual.entries) {
        assert_eq!(want, got, "golden mismatch for workload {}", want.workload);
    }
    assert_eq!(expected, actual);
}
