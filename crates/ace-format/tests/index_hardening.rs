//! Block index (AIDX) and trailer (ACET) validation against malformed input.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace_core::DecodeLimits;
use ace_format::{
    decode_index, decode_trailer, encode_index, encode_trailer, BlockIndex, BlockIndexEntry,
    FileTrailer,
};

/// Builds a small valid index fixture used by malformed-index tests.
fn valid_index() -> BlockIndex {
    BlockIndex {
        entries: vec![
            BlockIndexEntry {
                block_id: 0,
                original_offset: 0,
                original_size: 64,
                file_offset: 32,
                encoded_span: 48,
                flags: 0,
            },
            BlockIndexEntry {
                block_id: 1,
                original_offset: 64,
                original_size: 64,
                file_offset: 80,
                encoded_span: 48,
                flags: 0,
            },
        ],
    }
}

/// A valid serialized index must survive encode/decode exactly.
#[test]
fn index_roundtrip_is_exact() {
    let index = valid_index();
    let bytes = encode_index(&index);
    assert_eq!(
        decode_index(&bytes, &DecodeLimits::default()).unwrap(),
        index
    );
}

/// Index entry count must be checked before attacker-controlled allocation.
#[test]
fn index_entry_limit_is_enforced() {
    let index = valid_index();
    let bytes = encode_index(&index);
    let limits = DecodeLimits {
        max_index_entries: 1,
        ..DecodeLimits::default()
    };
    assert!(decode_index(&bytes, &limits).is_err());
}

/// Logical index offsets must be contiguous.
#[test]
fn non_contiguous_logical_offsets_are_rejected() {
    let mut index = valid_index();
    index.entries[1].original_offset = 65;
    let bytes = encode_index(&index);
    assert!(decode_index(&bytes, &DecodeLimits::default()).is_err());
}

/// Block identifiers must increase strictly.
#[test]
fn duplicate_block_ids_are_rejected() {
    let mut index = valid_index();
    index.entries[1].block_id = 0;
    let bytes = encode_index(&index);
    assert!(decode_index(&bytes, &DecodeLimits::default()).is_err());
}

/// Trailer checksum corruption must be rejected deterministically.
#[test]
fn trailer_crc_corruption_is_rejected() {
    let trailer = FileTrailer {
        index_offset: 123,
        index_size: 456,
        index_crc32c: 789,
    };
    let mut bytes = encode_trailer(trailer);
    bytes[8] ^= 0x5a;
    assert!(decode_trailer(&bytes).is_err());
}

/// A valid trailer must round-trip without changing pointer metadata.
#[test]
fn trailer_roundtrip_is_exact() {
    let trailer = FileTrailer {
        index_offset: 123,
        index_size: 456,
        index_crc32c: 789,
    };
    let bytes = encode_trailer(trailer);
    assert_eq!(decode_trailer(&bytes).unwrap(), trailer);
}
