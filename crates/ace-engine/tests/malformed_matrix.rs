//! Malformed-input matrix (ACE 0.4.6 hardening, TS1 / Format 1.4 fixture since 0.5.0).
//!
//! Every hostile mutation of a valid Format 1.3 or 1.4 file must be rejected with an error — never a
//! panic, never silently wrong output, never an allocation driven by an untrusted size field.
//! The table covers truncation, magic, version, flags, size/count fields (with a re-sealed
//! header CRC so the size checks themselves are exercised), index/trailer damage and an
//! exhaustive single-byte flip sweep.
//!
//! The sequential decoder stops after the declared, CRC-checked blocks, so damage confined to
//! the index/trailer (or bytes appended after it) is detected by the indexed reader only; every
//! mutation must therefore be rejected by *at least one* reader and misdecoded by *none*.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::io::Cursor;

use ace_core::{AceConfig, DecodeLimits};
use ace_corpus::Workload;
use ace_engine::{AceEngine, AceIndexedDecoder, PREALLOCATION_CAP_BYTES};
use ace_format::{checksum, FILE_HEADER_SIZE};

/// Small block size so the fixture has several blocks and a real index.
const BLOCK_SIZE: usize = 16 * 1024;

/// Source bytes and their valid ACE encoding.
fn fixture() -> (Vec<u8>, Vec<u8>) {
    let data = Workload::Mixed.generate(96 * 1024);
    let engine = AceEngine::new(AceConfig {
        block_size: BLOCK_SIZE,
        threads: 1,
        ..AceConfig::default()
    })
    .unwrap();
    let encoded = engine.compress(&data).unwrap();
    (data, encoded)
}

/// Format 1.4 fixture: Gorilla, RunDelta-on-floats and RunDelta-on-integers TS1 blocks.
fn ts1_fixture() -> (Vec<u8>, Vec<u8>) {
    let mut data = Workload::F64Noisy.generate(48 * 1024);
    data.extend(Workload::F64Step.generate(32 * 1024));
    data.extend(Workload::IntSparseChange.generate(48 * 1024));
    let engine = AceEngine::new(AceConfig {
        block_size: BLOCK_SIZE,
        threads: 1,
        profile: ace_core::CompressionProfile::Fast,
        ..AceConfig::default()
    })
    .unwrap();
    let (encoded, stats) = engine.compress_with_stats(&data).unwrap();
    assert!(
        stats.ts1_gorilla_f64_blocks > 0,
        "fixture lacks Gorilla blocks"
    );
    assert!(
        stats.ts1_run_delta_blocks > 0,
        "fixture lacks RunDelta blocks"
    );
    assert_eq!(encoded[5], 4, "fixture must be Format 1.4");
    (data, encoded)
}

/// Recomputes the file-header CRC after a deliberate header mutation.
fn reseal_header(file: &mut [u8]) {
    let crc = checksum(&file[..FILE_HEADER_SIZE - 4]);
    file[FILE_HEADER_SIZE - 4..FILE_HEADER_SIZE].copy_from_slice(&crc.to_le_bytes());
}

/// A named mutation of a valid file.
struct Mutation {
    /// Case name reported on failure.
    name: &'static str,
    /// Transformation applied to a copy of the valid file.
    apply: fn(&mut Vec<u8>),
}

/// The deterministic mutation table.
fn mutations() -> Vec<Mutation> {
    vec![
        Mutation {
            name: "empty",
            apply: |f| f.clear(),
        },
        Mutation {
            name: "header-only",
            apply: |f| f.truncate(FILE_HEADER_SIZE),
        },
        Mutation {
            name: "truncated-header",
            apply: |f| f.truncate(FILE_HEADER_SIZE - 1),
        },
        Mutation {
            name: "truncated-half",
            apply: |f| {
                let n = f.len() / 2;
                f.truncate(n)
            },
        },
        Mutation {
            name: "truncated-last-byte",
            apply: |f| {
                f.pop();
            },
        },
        Mutation {
            name: "bad-magic",
            apply: |f| f[0] ^= 0xFF,
        },
        Mutation {
            name: "future-major",
            apply: |f| {
                f[4] = 2;
                reseal_header(f)
            },
        },
        Mutation {
            name: "future-minor",
            apply: |f| {
                f[5] = 99;
                reseal_header(f)
            },
        },
        Mutation {
            name: "unknown-flags",
            apply: |f| {
                f[7] |= 0x80;
                reseal_header(f)
            },
        },
        Mutation {
            name: "header-crc",
            apply: |f| f[FILE_HEADER_SIZE - 1] ^= 0x01,
        },
        Mutation {
            name: "original-size-max",
            apply: |f| {
                f[12..20].copy_from_slice(&u64::MAX.to_le_bytes());
                reseal_header(f)
            },
        },
        Mutation {
            name: "original-size-plus-one",
            apply: |f| {
                let size = u64::from_le_bytes(f[12..20].try_into().unwrap()) + 1;
                f[12..20].copy_from_slice(&size.to_le_bytes());
                reseal_header(f)
            },
        },
        Mutation {
            name: "block-count-max",
            apply: |f| {
                f[20..28].copy_from_slice(&u64::MAX.to_le_bytes());
                reseal_header(f)
            },
        },
        Mutation {
            name: "block-count-zero",
            apply: |f| {
                f[20..28].fill(0);
                reseal_header(f)
            },
        },
        Mutation {
            name: "first-block-header",
            apply: |f| f[FILE_HEADER_SIZE + 2] ^= 0x5A,
        },
        Mutation {
            name: "first-payload-byte",
            apply: |f| f[FILE_HEADER_SIZE + 64] ^= 0x01,
        },
        Mutation {
            name: "trailer-last-byte",
            apply: |f| {
                let n = f.len() - 1;
                f[n] ^= 0x01
            },
        },
        Mutation {
            name: "index-region",
            apply: |f| {
                let n = f.len() - 24;
                f[n] ^= 0x40
            },
        },
        Mutation {
            name: "appended-garbage",
            apply: |f| f.extend_from_slice(&[0xAB; 37]),
        },
    ]
}

/// Sequential decoding of `file` must fail or reproduce `data` exactly.
fn assert_sequential_safe(name: &str, data: &[u8], file: &[u8]) -> bool {
    match AceEngine::default_engine().decompress(file) {
        Ok(decoded) => {
            assert_eq!(
                decoded, data,
                "{name}: decoder accepted a mutation and returned wrong bytes"
            );
            false
        }
        Err(_) => true,
    }
}

/// Indexed opening + a full range read of `file` must fail or reproduce `data` exactly;
/// returns `true` when the file was rejected.
fn assert_indexed_safe(name: &str, data: &[u8], file: &[u8]) -> bool {
    let Ok(mut decoder) = AceIndexedDecoder::open(Cursor::new(file), DecodeLimits::default())
    else {
        return true;
    };
    match decoder.read_range(0..data.len() as u64) {
        Ok(range) => {
            assert_eq!(range, data, "{name}: indexed reader returned wrong bytes");
            false
        }
        Err(_) => true,
    }
}

/// Runs both readers; at least one must reject and neither may misdecode.
fn assert_rejected(name: &str, data: &[u8], file: &[u8]) {
    let sequential = assert_sequential_safe(name, data, file);
    let indexed = assert_indexed_safe(name, data, file);
    assert!(sequential || indexed, "{name}: accepted by both readers");
}

/// Every table mutation is rejected by at least one reader and misdecoded by none.
#[test]
fn mutation_table_is_rejected() {
    let (data, valid) = fixture();
    for mutation in mutations() {
        let mut file = valid.clone();
        (mutation.apply)(&mut file);
        assert_rejected(mutation.name, &data, &file);
    }
}

/// Every truncation point is rejected (never a panic or a short successful decode).
#[test]
fn every_truncation_is_rejected() {
    let (data, valid) = fixture();
    for length in (0..valid.len()).step_by(7) {
        assert_rejected(
            &format!("truncation to {length} bytes"),
            &data,
            &valid[..length],
        );
    }
}

/// Flipping any single byte of the file never produces wrong output or a panic.
#[test]
fn single_byte_flip_sweep_never_misdecodes() {
    let (data, valid) = fixture();
    for position in 0..valid.len() {
        let mut file = valid.clone();
        file[position] ^= 0x01 << (position % 8);
        assert_sequential_safe("flip", &data, &file);
        if position % 5 == 0 {
            assert_indexed_safe("flip", &data, &file);
        }
    }
}

/// Every truncation of the Format 1.4 / TS1 fixture is rejected.
#[test]
fn ts1_truncations_are_rejected() {
    let (data, valid) = ts1_fixture();
    for length in (0..valid.len()).step_by(3) {
        assert_rejected(
            &format!("TS1 truncation to {length}"),
            &data,
            &valid[..length],
        );
    }
}

/// Flipping any single byte of the TS1 fixture never produces wrong output or a panic.
#[test]
fn ts1_single_byte_flip_sweep_never_misdecodes() {
    let (data, valid) = ts1_fixture();
    for position in 0..valid.len() {
        let mut file = valid.clone();
        file[position] ^= 0x01 << (position % 8);
        assert_sequential_safe("TS1 flip", &data, &file);
    }
}

/// The TS1 fixture with its header downgraded to Format 1.3 is rejected (TS1 needs 1.4).
#[test]
fn ts1_in_format_1_3_is_rejected() {
    let (data, mut file) = ts1_fixture();
    file[5] = 3;
    reseal_header(&mut file);
    assert_rejected("TS1 in 1.3", &data, &file);
}

/// A forged `original_size` must not drive a huge up-front allocation in `decompress_into`.
#[test]
fn forged_original_size_is_not_preallocated() {
    let (_, valid) = fixture();
    let mut file = valid;
    file[12..20].copy_from_slice(&(u64::MAX / 2).to_le_bytes());
    reseal_header(&mut file);
    let mut out = Vec::new();
    assert!(AceEngine::default_engine()
        .decompress_into(&file, &mut out)
        .is_err());
    assert!(out.capacity() as u64 <= PREALLOCATION_CAP_BYTES);

    let strict = DecodeLimits {
        max_output_size: 1024,
        ..DecodeLimits::default()
    };
    let mut small = Vec::new();
    let engine = AceEngine::default_engine().with_decode_limits(strict);
    assert!(engine.decompress_into(&file, &mut small).is_err());
    assert!(
        small.capacity() <= 1024,
        "reservation must respect max_output_size"
    );
}

/// Tight decoder limits reject a valid file instead of exceeding them.
#[test]
fn resource_limits_are_enforced() {
    let (data, valid) = fixture();
    let cases = [
        DecodeLimits {
            max_output_size: data.len() as u64 - 1,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            max_block_size: BLOCK_SIZE - 1,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            max_encoded_block_size: 16,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            max_index_entries: 1,
            ..DecodeLimits::default()
        },
    ];
    for (index, limits) in cases.into_iter().enumerate() {
        let sequential = AceEngine::default_engine()
            .with_decode_limits(limits.clone())
            .decompress(&valid);
        let indexed = AceIndexedDecoder::open(Cursor::new(&valid), limits)
            .and_then(|mut decoder| decoder.read_range(0..data.len() as u64));
        assert!(
            sequential.is_err() || indexed.is_err(),
            "limit case {index} accepted by both decoders"
        );
    }
}

/// `default_block_size` in the file header is informational (every block header carries its
/// own size), so a forged value must not change what is decoded.
#[test]
fn informational_block_size_field_does_not_affect_decoding() {
    let (data, valid) = fixture();
    for forged in [0u32, 1, u32::MAX] {
        let mut file = valid.clone();
        file[8..12].copy_from_slice(&forged.to_le_bytes());
        reseal_header(&mut file);
        assert!(!assert_sequential_safe("block-size", &data, &file));
        assert!(!assert_indexed_safe("block-size", &data, &file));
    }
}
