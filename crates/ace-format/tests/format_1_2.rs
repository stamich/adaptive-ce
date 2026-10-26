//! Format 1.2/1.3/1.4/1.5/1.6 header compatibility.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace_format::{
    checksum, decode_file_header, encode_file_header, FileHeader, FORMAT_MINOR, FORMAT_MINOR_BASE,
};

/// The writer emits the requested minor version (1.3 base, 1.6 maximum) in the unchanged
/// fixed header shape; values above the maximum are capped.
#[test]
fn writer_emits_requested_minor_version() {
    assert_eq!((FORMAT_MINOR_BASE, FORMAT_MINOR), (3, 6));
    for (requested, written) in [(3u8, 3u8), (4, 4), (5, 5), (6, 6), (9, 6)] {
        let header = FileHeader {
            minor_version: requested,
            flags: 0,
            default_block_size: 262_144,
            original_size: 123,
            block_count: 1,
        };
        let bytes = encode_file_header(&header);
        assert_eq!(bytes[5], written);
        assert_eq!(decode_file_header(&bytes).unwrap().minor_version, written);
    }
}

/// Verifies the ACE 0.4 reader still accepts a valid Format 1.2 fixed header.
#[test]
fn reader_accepts_1_2() {
    let header = FileHeader {
        minor_version: 3,
        flags: 0,
        default_block_size: 262_144,
        original_size: 0,
        block_count: 0,
    };
    let mut bytes = encode_file_header(&header);
    bytes[5] = 2;
    let crc = checksum(&bytes[..28]);
    bytes[28..32].copy_from_slice(&crc.to_le_bytes());
    assert_eq!(decode_file_header(&bytes).unwrap().minor_version, 2);
}

/// Verifies the ACE 0.4 reader still accepts a valid Format 1.1 fixed header.
#[test]
fn reader_accepts_1_1() {
    let header = FileHeader {
        minor_version: 3,
        flags: 0,
        default_block_size: 262_144,
        original_size: 0,
        block_count: 0,
    };
    let mut bytes = encode_file_header(&header);
    bytes[5] = 1;
    let crc = checksum(&bytes[..28]);
    bytes[28..32].copy_from_slice(&crc.to_le_bytes());
    assert_eq!(decode_file_header(&bytes).unwrap().minor_version, 1);
}

/// Minimal-version rule: the declared version is the highest version a block needs, never below
/// 1.3 and never above the maximum.
#[test]
fn minimal_minor_version_is_the_highest_block_requirement() {
    use ace_format::minimal_minor_version;
    assert_eq!(minimal_minor_version([]), 3);
    assert_eq!(minimal_minor_version([0, 3, 3, 0]), 3);
    assert_eq!(minimal_minor_version([3, 4, 3]), 4);
    assert_eq!(minimal_minor_version([4, 5, 3]), 5);
    assert_eq!(minimal_minor_version([5, 6, 3]), 6);
    assert_eq!(minimal_minor_version([9]), 6);
}
