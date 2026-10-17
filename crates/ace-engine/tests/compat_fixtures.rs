//! Frozen binary compatibility fixtures (ACE 0.5.1): `.ace` files written by earlier releases
//! must decode byte for byte forever.
//!
//! `examples/compat/<release>/` holds files produced by that release's own encoder (never
//! regenerated) and `examples/compat/MANIFEST.json` their SHA-256, the SHA-256 of the input
//! and the declared format version. The inputs are not stored: [`input`] regenerates them
//! with integer-only generators that must never change (a changed generator fails the input
//! SHA-256 check below, not silently the fixtures).
//!
//! The golden SHA-256 files guard the *writer* (same input → same bytes); these fixtures
//! guard the *reader* (old bytes → same input).
//!
//! `ACE_COMPAT_WRITE_INPUTS=DIR cargo test -p ace-engine --test compat_fixtures -- --ignored`
//! writes the inputs, which is how the fixtures were produced with each old release.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use ace_core::DecodeLimits;
use ace_engine::{AceEngine, AceIndexedDecoder};
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Generator of one fixture input.
type Generator = fn() -> Vec<u8>;

/// Fixture cases: name and input generator. Never change a generator; add a new case.
const CASES: [(&str, Generator); 8] = [
    ("text", text),
    ("u32-counter", u32_counter),
    ("binary-runs", binary_runs),
    ("f64-series", f64_series),
    ("u64-sparse", u64_sparse),
    ("random", random),
    ("u64-outliers", u64_outliers),
    ("f64-decimal", f64_decimal),
];

/// Deterministic xorshift64 generator (never change).
struct XorShift(u64);

/// Inherent methods of [`XorShift`].
impl XorShift {
    /// Next 64-bit value.
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// 64 KiB + 5 bytes of word-salad text (LZ, Huffman and rANS paths).
fn text() -> Vec<u8> {
    const WORDS: [&str; 16] = [
        "adaptive",
        "compression",
        "engine",
        "block",
        "planner",
        "entropy",
        "the",
        "of",
        "and",
        "frozen",
        "fixture",
        "reader",
        "forever",
        "bytes",
        "format",
        "stable",
    ];
    let mut rng = XorShift(0xACE0_0001);
    let mut out = Vec::new();
    while out.len() < 65_541 {
        let r = rng.next();
        out.extend_from_slice(WORDS[(r % 16) as usize].as_bytes());
        out.push(if r >> 60 == 0 { b'\n' } else { b' ' });
    }
    out.truncate(65_541);
    out
}

/// 16 384 little-endian u32 values `7 + 3i` plus a 3-byte tail (NUM1 since 0.4).
fn u32_counter() -> Vec<u8> {
    let mut out: Vec<u8> = (0u32..16_384)
        .flat_map(|i| (7 + 3 * i).to_le_bytes())
        .collect();
    out.extend_from_slice(&[1, 2, 3]);
    out
}

/// 64 KiB of byte runs of random length and value (RLE / delta paths).
fn binary_runs() -> Vec<u8> {
    let mut rng = XorShift(0xACE0_0003);
    let mut out = Vec::new();
    while out.len() < 65_536 {
        let r = rng.next();
        let run = 1 + (r % 40) as usize;
        out.extend(std::iter::repeat_n((r >> 32) as u8, run));
    }
    out.truncate(65_536);
    out
}

/// 8 192 f64 values: a slow ramp with small integer-derived noise (TS1 Gorilla since 0.5).
/// Only `+`, `*` and exact conversions are used, so every platform produces the same bits.
fn f64_series() -> Vec<u8> {
    let mut rng = XorShift(0xACE0_0004);
    (0u32..8_192)
        .flat_map(|i| {
            let noise = (rng.next() % 16) as f64 * 0.0625;
            (1_000.0 + f64::from(i) * 0.25 + noise).to_le_bytes()
        })
        .collect()
}

/// 8 192 little-endian u64 values that change every 200 values (TS1 RunDelta since 0.5).
fn u64_sparse() -> Vec<u8> {
    (0u64..8_192)
        .flat_map(|i| (1_700_000_000 + (i / 200) * 17).to_le_bytes())
        .collect()
}

/// 4 KiB of incompressible bytes (RAW fallback).
fn random() -> Vec<u8> {
    let mut rng = XorShift(0xACE0_0006);
    (0..4096).map(|_| (rng.next() >> 24) as u8).collect()
}

/// 8 192 little-endian u64 values `1 000 000 + 3i` with a `+2^40` jump every 1 000th value
/// (NUM1 Patched FOR since 0.6.1, Format 1.5).
fn u64_outliers() -> Vec<u8> {
    (0u64..8_192)
        .flat_map(|i| {
            let jump = if i % 1_000 == 999 { 1u64 << 40 } else { 0 };
            (1_000_000 + 3 * i + jump).to_le_bytes()
        })
        .collect()
}

/// 8 192 f64 prices `cents / 100` (a random walk) with a NaN, a `-0.0` and an infinity among
/// them (TS1 ALP since 0.6.1, Format 1.5). `i64` arithmetic, one exact division per value.
fn f64_decimal() -> Vec<u8> {
    let mut rng = XorShift(0xACE0_0008);
    let mut cents = 24_990i64;
    (0usize..8_192)
        .flat_map(|i| {
            cents = (cents + (rng.next() % 41) as i64 - 20).max(1);
            let value = match i {
                1_000 => f64::NAN,
                2_000 => -0.0,
                3_000 => f64::INFINITY,
                _ => cents as f64 / 100.0,
            };
            value.to_le_bytes()
        })
        .collect()
}

/// One manifest entry.
#[derive(Debug, Deserialize)]
struct Entry {
    /// Release that wrote the file (`0.2.1`, ...).
    release: String,
    /// Fixture case (a key of [`CASES`]).
    case: String,
    /// Profile used by the writer.
    profile: String,
    /// Declared format minor version (header byte 5).
    format_minor: u8,
    /// SHA-256 of the `.ace` file.
    sha256: String,
    /// SHA-256 of the input it must decode to.
    input_sha256: String,
}

/// Lower-case hex SHA-256 of `bytes`.
fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// `examples/compat` of the workspace.
fn compat_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/compat")
}

/// The manifest, keyed by relative path.
fn manifest() -> BTreeMap<String, Entry> {
    let text = std::fs::read_to_string(compat_dir().join("MANIFEST.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// Regenerated input of `case`.
fn input(case: &str) -> Vec<u8> {
    let (_, generate) = CASES
        .iter()
        .find(|(name, _)| *name == case)
        .unwrap_or_else(|| panic!("unknown compat case {case}"));
    generate()
}

/// The generators still produce the inputs the fixtures were written from.
#[test]
fn generators_are_frozen() {
    let manifest = manifest();
    for (name, generate) in CASES {
        let expected: Vec<&str> = manifest
            .values()
            .filter(|e| e.case == name)
            .map(|e| e.input_sha256.as_str())
            .collect();
        assert!(!expected.is_empty(), "case {name} has no fixture");
        let actual = sha256(&generate());
        assert!(
            expected.iter().all(|e| *e == actual),
            "generator {name} changed"
        );
    }
}

/// Every fixture is unmodified, declares its recorded format version and decodes to its
/// input through the sequential reader and (when indexed) block by block through the
/// indexed reader.
#[test]
fn every_fixture_decodes_to_its_input() {
    let manifest = manifest();
    assert!(manifest.len() >= 5 * CASES.len(), "fixtures missing");
    let engine = AceEngine::default_engine();
    for (path, entry) in &manifest {
        let file = std::fs::read(compat_dir().join(path)).unwrap();
        assert_eq!(sha256(&file), entry.sha256, "{path}: fixture was modified");
        assert_eq!(file[5], entry.format_minor, "{path}: format version");
        let expected = input(&entry.case);
        assert_eq!(sha256(&expected), entry.input_sha256, "{path}: input");
        let decoded = engine
            .decompress(&file)
            .unwrap_or_else(|e| panic!("{path} ({} {}): {e}", entry.release, entry.profile));
        assert!(decoded == expected, "{path}: decoded bytes differ");
        if let Ok(mut indexed) =
            AceIndexedDecoder::open(Cursor::new(&file), DecodeLimits::default())
        {
            let ranged = indexed.read_range(0..expected.len() as u64).unwrap();
            assert!(ranged == expected, "{path}: indexed decode differs");
        }
    }
}

/// Writes the fixture inputs to `$ACE_COMPAT_WRITE_INPUTS` (used once per old release).
#[test]
#[ignore = "fixture generation helper"]
fn write_inputs() {
    let Ok(dir) = std::env::var("ACE_COMPAT_WRITE_INPUTS") else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    for (name, generate) in CASES {
        std::fs::write(Path::new(&dir).join(format!("{name}.bin")), generate()).unwrap();
    }
}

/// Writes the fixtures of the *current* encoder for the cases added in 0.6.1 to
/// `$ACE_COMPAT_WRITE_FIXTURES/<case>-<profiles>.ace` and prints their manifest entries
/// (16 KiB blocks, one thread; profiles with identical output share one file). Used once, for
/// release 0.6.1; the resulting files are then frozen like every other fixture.
#[test]
#[ignore = "fixture generation helper"]
fn write_fixtures_0_6_1() {
    use ace_core::{AceConfig, CompressionProfile};
    let Ok(dir) = std::env::var("ACE_COMPAT_WRITE_FIXTURES") else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    for case in ["u64-outliers", "f64-decimal"] {
        let data = input(case);
        let mut outputs: Vec<(Vec<u8>, Vec<&str>)> = Vec::new();
        for (name, profile) in [
            ("fast", CompressionProfile::Fast),
            ("balanced", CompressionProfile::Balanced),
            ("dense", CompressionProfile::Dense),
        ] {
            let engine = AceEngine::new(AceConfig {
                block_size: 16_384,
                profile,
                threads: 1,
                ..AceConfig::default()
            })
            .unwrap();
            let file = engine.compress(&data).unwrap();
            match outputs.iter_mut().find(|(bytes, _)| *bytes == file) {
                Some((_, names)) => names.push(name),
                None => outputs.push((file, vec![name])),
            }
        }
        for (file, names) in outputs {
            let profile = names.join("+");
            let name = format!("{case}-{profile}.ace");
            std::fs::write(Path::new(&dir).join(&name), &file).unwrap();
            println!(
                "\"0.6.1/{name}\": {{\"case\": \"{case}\", \"format_minor\": {}, \"input_sha256\": \"{}\", \"profile\": \"{profile}\", \"release\": \"0.6.1\", \"sha256\": \"{}\"}}",
                file[5],
                sha256(&data),
                sha256(&file)
            );
        }
    }
}
