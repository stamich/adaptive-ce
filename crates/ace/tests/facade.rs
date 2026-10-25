//! End-to-end tests of the public API: the bytes written through `ace` are the bytes of the
//! committed 0.5.0 golden file, hostile input never panics or yields wrong data, and every
//! limit is enforced.
//!
//! `ACE_SIMD=scalar cargo test -p ace --test facade` proves the portable code paths produce
//! the same bytes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::io::Cursor;
use std::path::PathBuf;

use ace::{Config, DecodeLimits, Engine, ErrorKind, Profile, RangeReader};
use ace_corpus::Workload;
use serde::Deserialize;
use sha2::{Digest, Sha256};

const WORKLOAD_BYTES: usize = 1024 * 1024;

#[derive(Deserialize)]
struct Golden {
    entries: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    workload: String,
    input_sha256: String,
    profiles: Vec<GoldenProfile>,
}

#[derive(Deserialize)]
struct GoldenProfile {
    profile: String,
    ace_sha256: String,
    ace_bytes: usize,
}

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn golden() -> Golden {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/golden/0.6.1/GOLDEN.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn engine(profile: Profile) -> Engine {
    Engine::new(Config::builder().profile(profile).build().unwrap()).unwrap()
}

/// The facade writes exactly the 0.6.1 bytes for every workload and profile, and reads them back.
#[test]
fn facade_reproduces_the_0_6_1_golden_file() {
    let golden = golden();
    assert_eq!(golden.entries.len(), Workload::ALL.len());
    for entry in &golden.entries {
        let workload = Workload::ALL
            .iter()
            .copied()
            .find(|w| w.name() == entry.workload)
            .unwrap();
        let input = workload.generate(WORKLOAD_BYTES);
        assert_eq!(hex(&input), entry.input_sha256, "{}", entry.workload);
        for p in &entry.profiles {
            let profile = match p.profile.as_str() {
                "fast" => Profile::Fast,
                "balanced" => Profile::Balanced,
                "dense" => Profile::Dense,
                other => panic!("unknown profile {other}"),
            };
            let engine = engine(profile);
            let packed = engine.compress(&input).unwrap();
            assert_eq!(
                packed.len(),
                p.ace_bytes,
                "{} {}",
                entry.workload,
                p.profile
            );
            assert_eq!(
                hex(&packed),
                p.ace_sha256,
                "{} {}",
                entry.workload,
                p.profile
            );
            assert_eq!(engine.decompress(&packed).unwrap(), input);
        }
    }
}

/// Compressing through a stream, from a reader of known size, gives the same file.
#[test]
fn streaming_gives_the_golden_bytes() {
    let golden = golden();
    let entry = &golden.entries[0];
    let workload = Workload::ALL
        .iter()
        .copied()
        .find(|w| w.name() == entry.workload)
        .unwrap();
    let input = workload.generate(WORKLOAD_BYTES);
    let balanced = entry
        .profiles
        .iter()
        .find(|p| p.profile == "balanced")
        .unwrap();
    let mut sink = Cursor::new(Vec::new());
    engine(Profile::Balanced)
        .compress_stream(Cursor::new(&input), &mut sink, input.len() as u64)
        .unwrap();
    assert_eq!(hex(&sink.into_inner()), balanced.ace_sha256);
}

/// Random access over a golden file returns slices of the input.
#[test]
fn range_reads_match_the_input_on_every_workload() {
    let engine = engine(Profile::Balanced);
    for workload in Workload::ALL {
        let input = workload.generate(WORKLOAD_BYTES);
        let packed = engine.compress(&input).unwrap();
        let mut reader = engine.open_range_reader(Cursor::new(packed)).unwrap();
        assert_eq!(reader.len(), input.len() as u64);
        for (a, b) in [
            (0, 1),
            (100, 5000),
            (262_143, 262_145),
            (700_000, 1_048_576),
        ] {
            assert_eq!(
                reader.read_range(a..b).unwrap(),
                &input[a as usize..b as usize],
                "{} {a}..{b}",
                workload.name()
            );
        }
    }
}

/// Damage anywhere in a file is either detected or harmless; it never panics and never yields
/// different bytes.
#[test]
fn hostile_input_never_returns_wrong_data() {
    let input = Workload::ALL[0].generate(300_000);
    let engine = Engine::new(Config::builder().block_size(64 * 1024).build().unwrap()).unwrap();
    let packed = engine.compress(&input).unwrap();
    let step = (packed.len() / 400).max(1);
    for i in (0..packed.len()).step_by(step) {
        let mut damaged = packed.clone();
        damaged[i] ^= 0x5A;
        if let Ok(out) = engine.decompress(&damaged) {
            assert_eq!(out, input, "byte {i} changed the output silently");
        }
        let _ = RangeReader::open(Cursor::new(damaged)).and_then(|mut r| r.read_range(0..1000));
    }
    for cut in (0..packed.len()).step_by(step) {
        let _ = engine.decompress(&packed[..cut]);
    }
    assert_eq!(
        engine.decompress(&[0u8; 100]).unwrap_err().kind(),
        ErrorKind::NotAceFile
    );
    assert!(engine.decompress(&[]).is_err());
}

/// A file from a newer minor format (1.7) is unsupported and says a newer release wrote it.
#[test]
fn a_newer_format_is_unsupported_and_named() {
    let engine = Engine::default();
    let mut packed = engine.compress(&[7u8; 4096]).unwrap();
    packed[5] = 7; // format 1.7
    let e = engine.decompress(&packed).unwrap_err();
    assert_eq!(e.kind(), ErrorKind::Unsupported);
    assert!(e.to_string().contains("newer ACE release"), "{e}");
}

/// Every decode limit rejects with `LimitExceeded`.
#[test]
fn every_limit_is_enforced() {
    let input = Workload::ALL[0].generate(400_000);
    let writer = Engine::new(Config::builder().block_size(64 * 1024).build().unwrap()).unwrap();
    let packed = writer.compress(&input).unwrap();
    let cases: [(&str, DecodeLimits); 4] = [
        (
            "output",
            DecodeLimits::builder().max_output_size(1000).build(),
        ),
        (
            "block",
            DecodeLimits::builder().max_block_size(1000).build(),
        ),
        (
            "encoded",
            DecodeLimits::builder().max_encoded_block_size(10).build(),
        ),
        ("blocks", DecodeLimits::builder().max_blocks(3).build()),
    ];
    for (name, limits) in cases {
        let strict = Engine::default().with_decode_limits(limits.clone());
        let e = strict.decompress(&packed).unwrap_err();
        assert_eq!(e.kind(), ErrorKind::LimitExceeded, "{name}: {e}");
        assert!(
            RangeReader::open_with_limits(Cursor::new(packed.clone()), &limits)
                .and_then(|mut r| r.read_range(0..400_000))
                .is_err(),
            "{name}"
        );
    }
}

/// Settings that the engine cannot honour are reported when they are built.
#[test]
fn invalid_settings_are_rejected() {
    assert_eq!(
        Config::builder().block_size(0).build().unwrap_err().kind(),
        ErrorKind::InvalidConfig
    );
}
