//! Segmented blocks under a matrix of conditions: the encoder is deterministic across worker
//! counts, profiles and SIMD backends, and every way of forging a `SEG1` block is rejected by
//! every reader without a panic. Happy-path behaviour is in `segmented_encode.rs` and
//! `segmented_decode.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::io::Cursor;

use ace_core::{
    AceConfig, AceError, CodecId, CompressionProfile, DecodeLimits, EntropyCodecId,
    SegmentationConfig, TransformId,
};
use ace_engine::{AceEngine, AceIndexedDecoder};
use ace_format::{
    checksum, encode_segment_payload, serialize_block, AceWriter, BlockHeader, FileHeader,
    FILE_FLAG_HAS_INDEX, SEGMENT_ENTRY_SIZE, SEGMENT_HEADER_SIZE,
};

/// 4 KiB windows, 16 KiB .. 128 KiB segments.
fn segmentation() -> SegmentationConfig {
    SegmentationConfig {
        window_size: 4096,
        min_segment_size: 16 * 1024,
        max_segment_size: 128 * 1024,
        max_segments: 64,
    }
}

/// Deterministic pseudo-random bytes (xorshift).
fn noise(len: usize, mut seed: u64) -> Vec<u8> {
    (0..len)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 24) as u8
        })
        .collect()
}

/// Text, then counters, then noise: 512 KiB that the encoder segments.
fn mixed() -> Vec<u8> {
    let words = ["alpha ", "beta ", "gamma ", "delta ", "log ", "entry ", "value=", "ok\n"];
    let mut input = Vec::new();
    let mut i = 0usize;
    while input.len() < 192 * 1024 {
        input.extend_from_slice(words[(i * 7 + i / 3) % words.len()].as_bytes());
        i += 1;
    }
    input.extend((0u32..48 * 1024).flat_map(u32::to_le_bytes));
    input.extend(noise(128 * 1024, 4));
    input
}

#[test]
fn output_is_identical_for_every_worker_count_and_profile() {
    let mut input = mixed();
    input.extend(mixed());
    input.extend(mixed());
    for profile in [CompressionProfile::Fast, CompressionProfile::Balanced, CompressionProfile::Dense] {
        let mut reference: Option<Vec<u8>> = None;
        for threads in [1usize, 2, 3, 8] {
            let config = AceConfig {
                profile,
                threads,
                block_size: 512 * 1024,
                segmentation: Some(segmentation()),
                ..AceConfig::default()
            };
            let (file, stats) = AceEngine::new(config).unwrap().compress_with_stats(&input).unwrap();
            assert!(stats.segmented_blocks >= 1, "{profile:?}: nothing was segmented");
            match &reference {
                Some(expected) => assert_eq!(expected, &file, "{profile:?} threads={threads}"),
                None => reference = Some(file),
            }
        }
        assert_eq!(AceEngine::default_engine().decompress(&reference.unwrap()).unwrap(), input);
    }
}

/// The segmented file of the fixture is a fixed byte string. CI runs this test with the default
/// SIMD backend and with `ACE_SIMD=scalar` (and on aarch64 with NEON), so equality with one
/// constant proves that the boundaries, the plans and the bytes do not depend on the backend.
#[test]
fn segmented_output_is_fixed_across_backends() {
    let config = AceConfig {
        threads: 1,
        block_size: 1 << 20,
        segmentation: Some(segmentation()),
        ..AceConfig::default()
    };
    let file = AceEngine::new(config).unwrap().compress(&mixed()).unwrap();
    assert_eq!(
        (file.len(), checksum(&file)),
        EXPECTED_FIXTURE,
        "the segmented encoding changed; if that is intended, update EXPECTED_FIXTURE and the golden notes"
    );
}

/// `(length, CRC-32C)` of the segmented fixture file.
const EXPECTED_FIXTURE: (usize, u32) = (133_252, 4_098_501_873);

// ---- forged blocks ---------------------------------------------------------------------------

/// A RAW inner block holding `data`.
fn inner(id: u64, data: &[u8]) -> Vec<u8> {
    serialize_block(&raw_header(id, data), &[], data)
}

fn raw_header(id: u64, data: &[u8]) -> BlockHeader {
    BlockHeader {
        block_id: id,
        original_size: data.len() as u32,
        encoded_size: data.len() as u32,
        metadata_size: 0,
        codec: CodecId::Raw,
        entropy: EntropyCodecId::None,
        transforms: Vec::new(),
        dictionary: None,
        flags: 0,
        payload_crc32c: checksum(data),
    }
}

/// A valid two-segment payload over `a` and `b` and the whole block's bytes.
fn good_payload(a: &[u8], b: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let payload = encode_segment_payload(&[
        (a.len() as u32, inner(0, a)),
        (b.len() as u32, inner(1, b)),
    ])
    .unwrap();
    (payload, [a, b].concat())
}

/// A file of format `minor` with one block whose header comes from `outer`.
fn file_with(minor: u8, outer: impl FnOnce(&mut BlockHeader), metadata: &[u8], payload: &[u8], whole: &[u8]) -> Vec<u8> {
    let mut header = BlockHeader {
        block_id: 0,
        original_size: whole.len() as u32,
        encoded_size: payload.len() as u32,
        metadata_size: metadata.len() as u32,
        codec: CodecId::Segmented,
        entropy: EntropyCodecId::None,
        transforms: Vec::new(),
        dictionary: None,
        flags: 0,
        payload_crc32c: checksum(whole),
    };
    outer(&mut header);
    let mut writer = AceWriter::new(Vec::new());
    writer
        .write_file_header(&FileHeader {
            minor_version: minor,
            flags: FILE_FLAG_HAS_INDEX,
            default_block_size: 1 << 20,
            original_size: whole.len() as u64,
            block_count: 1,
        })
        .unwrap();
    writer.write_block(&header, metadata, payload).unwrap();
    writer.finish(true).unwrap().0
}

/// Every reader must fail on `file`, and none may panic.
fn assert_rejected(file: &[u8], what: &str) -> AceError {
    let engine = AceEngine::default_engine();
    let error = engine.decompress(file).expect_err(what);
    let mut sink = Vec::new();
    assert!(engine.decompress_into(file, &mut sink).is_err(), "{what}: decompress_into");
    assert!(engine.decompress_from(Cursor::new(file), Vec::new()).is_err(), "{what}: decompress_from");
    if let Ok(mut decoder) = AceIndexedDecoder::open(Cursor::new(file), DecodeLimits::default()) {
        assert!(decoder.decode_block(0).is_err(), "{what}: decode_block");
    }
    error
}

#[test]
fn the_forgery_builder_produces_a_valid_file() {
    let (payload, whole) = good_payload(&[1u8; 64], &[2u8; 100]);
    let file = file_with(6, |_| {}, &[], &payload, &whole);
    assert_eq!(AceEngine::default_engine().decompress(&file).unwrap(), whole);
}

#[test]
fn forged_tables_are_rejected_by_every_reader() {
    let (a, b) = (vec![1u8; 64], vec![2u8; 100]);
    let (good, whole) = good_payload(&a, &b);
    let entry = |index: usize| SEGMENT_HEADER_SIZE + index * SEGMENT_ENTRY_SIZE;
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("wrong magic", { let mut p = good.clone(); p[0] = b'X'; p }),
        ("reserved word", { let mut p = good.clone(); p[8] = 1; p }),
        ("count 1", { let mut p = good.clone(); p[4..8].copy_from_slice(&1u32.to_le_bytes()); p }),
        ("count 0", { let mut p = good.clone(); p[4..8].copy_from_slice(&0u32.to_le_bytes()); p }),
        ("count over the limit", { let mut p = good.clone(); p[4..8].copy_from_slice(&u32::MAX.to_le_bytes()); p }),
        ("count beyond the payload", { let mut p = good.clone(); p[4..8].copy_from_slice(&9u32.to_le_bytes()); p }),
        ("zero-size entry", { let mut p = good.clone(); p[entry(0)..entry(0) + 4].copy_from_slice(&0u32.to_le_bytes()); p }),
        ("sizes do not add up", { let mut p = good.clone(); p[entry(0)..entry(0) + 4].copy_from_slice(&65u32.to_le_bytes()); p }),
        ("lengths do not add up", { let mut p = good.clone(); p[entry(1) + 4..entry(1) + 8].copy_from_slice(&1u32.to_le_bytes()); p }),
        ("entry shorter than a header", { let mut p = good.clone(); p[entry(0) + 4..entry(0) + 8].copy_from_slice(&31u32.to_le_bytes()); p }),
        ("trailing byte", { let mut p = good.clone(); p.push(0); p }),
        ("truncated payload", good[..good.len() - 1].to_vec()),
        ("table cut short", good[..SEGMENT_HEADER_SIZE + 3].to_vec()),
        ("empty payload", Vec::new()),
    ];
    for (what, payload) in &cases {
        assert_rejected(&file_with(6, |_| {}, &[], payload, &whole), what);
    }
}

#[test]
fn forged_inner_blocks_are_rejected_by_every_reader() {
    let (a, b) = (vec![1u8; 64], vec![2u8; 100]);
    let whole = [a.clone(), b.clone()].concat();
    let table = |parts: &[(u32, Vec<u8>)]| encode_segment_payload(parts).unwrap();

    // Wrong position (id), wrong declared size, damaged inner CRC, damaged inner payload.
    let wrong_id = table(&[(64, inner(5, &a)), (100, inner(1, &b))]);
    let wrong_size = table(&[(64, inner(0, &a[..60])), (100, inner(1, &b))]);
    let mut bad_crc = inner(1, &b);
    bad_crc[BLOCK_CRC_BYTE] ^= 1;
    let bad_inner_crc = table(&[(64, inner(0, &a)), (100, bad_crc)]);
    let mut bad_payload = inner(1, &b);
    let last = bad_payload.len() - 1;
    bad_payload[last] ^= 1;
    let bad_inner_payload = table(&[(64, inner(0, &a)), (100, bad_payload)]);
    // A container inside a container.
    let (nested_payload, nested_whole) = good_payload(&a, &b);
    let nested_block = serialize_block(
        &BlockHeader {
            block_id: 0,
            original_size: nested_whole.len() as u32,
            encoded_size: nested_payload.len() as u32,
            metadata_size: 0,
            codec: CodecId::Segmented,
            entropy: EntropyCodecId::None,
            transforms: Vec::new(),
            dictionary: None,
            flags: 0,
            payload_crc32c: checksum(&nested_whole),
        },
        &[],
        &nested_payload,
    );
    let nested = table(&[(nested_whole.len() as u32, nested_block), (100, inner(1, &b))]);
    let nested_total = [nested_whole.clone(), b.clone()].concat();

    for (what, payload, total) in [
        ("wrong inner id", &wrong_id, &whole),
        ("wrong inner size", &wrong_size, &whole),
        ("inner header CRC", &bad_inner_crc, &whole),
        ("inner payload", &bad_inner_payload, &whole),
        ("nested container", &nested, &nested_total),
    ] {
        assert_rejected(&file_with(6, |_| {}, &[], payload, total), what);
    }
}

#[test]
fn forged_outer_blocks_are_rejected_by_every_reader() {
    let (payload, whole) = good_payload(&[1u8; 64], &[2u8; 100]);
    let wrong_crc = file_with(6, |h| h.payload_crc32c ^= 1, &[], &payload, &whole);
    assert!(matches!(assert_rejected(&wrong_crc, "outer CRC"), AceError::ChecksumMismatch(_)));
    let wrong_size = file_with(6, |h| h.original_size += 1, &[], &payload, &whole);
    assert_rejected(&wrong_size, "outer size");
    let entropy = file_with(6, |h| h.entropy = EntropyCodecId::Huffman, &[], &payload, &whole);
    assert_rejected(&entropy, "outer entropy stage");
    let transform = file_with(6, |h| h.transforms = vec![TransformId::DeltaByte], &[], &payload, &whole);
    assert_rejected(&transform, "outer transform");
    let metadata = file_with(6, |_| {}, &[0u8; 4], &payload, &whole);
    assert_rejected(&metadata, "outer metadata");
    // Older formats refuse codec 5 outright.
    for minor in 0..=5u8 {
        let old = file_with(minor, |_| {}, &[], &payload, &whole);
        assert!(
            matches!(assert_rejected(&old, "old minor"), AceError::UnsupportedVersion { .. }),
            "minor {minor}"
        );
    }
}

#[test]
fn tight_limits_stop_a_valid_segmented_file() {
    let (payload, whole) = good_payload(&[1u8; 64], &[2u8; 100]);
    let file = file_with(6, |_| {}, &[], &payload, &whole);
    let engine = AceEngine::default_engine();
    for (limits, label) in [
        (DecodeLimits { max_segments: 1, ..DecodeLimits::default() }, "segment count"),
        (DecodeLimits { max_block_size: 99, ..DecodeLimits::default() }, "block output size"),
    ] {
        let tight = engine.clone().with_decode_limits(limits);
        assert!(
            matches!(tight.decompress(&file), Err(AceError::ResourceLimitExceeded(what)) if what.contains(label.split(' ').next().unwrap())),
            "{label}"
        );
    }
}

/// Offset of the block-header CRC inside a serialized block (last four bytes of the fixed part).
const BLOCK_CRC_BYTE: usize = 28;
