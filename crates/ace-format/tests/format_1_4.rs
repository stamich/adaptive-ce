//! Format 1.4: the TS1 codec id is accepted only in 1.4 files; 1.0 – 1.4 headers are read.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace_core::{AceError, CodecId, EntropyCodecId};
use ace_format::{
    decode_block_header, decode_file_header, encode_block_header, encode_file_header, BlockHeader,
    FileHeader, BLOCK_HEADER_SIZE, FORMAT_MINOR,
};

/// Serialized block header using `codec`.
fn block_bytes(codec: CodecId) -> Vec<u8> {
    encode_block_header(&BlockHeader {
        block_id: 3,
        original_size: 65_536,
        encoded_size: 1024,
        metadata_size: 0,
        codec,
        entropy: EntropyCodecId::None,
        transforms: Vec::new(),
        dictionary: None,
        flags: 0,
        payload_crc32c: 9,
    })
}

/// A TS1 block parses in a 1.4 file and is rejected by every older minor version.
#[test]
fn time_series_codec_requires_format_1_4() {
    let bytes = block_bytes(CodecId::TimeSeries);
    let (fixed, descriptors) = bytes.split_at(BLOCK_HEADER_SIZE);
    assert_eq!(
        decode_block_header(fixed, descriptors, 4).unwrap().codec,
        CodecId::TimeSeries
    );
    for minor in 0..4u8 {
        assert!(
            matches!(
                decode_block_header(fixed, descriptors, minor),
                Err(AceError::UnsupportedVersion { minor: m, .. }) if m == minor
            ),
            "minor {minor}"
        );
    }
}

/// Codecs of older formats stay valid inside 1.4 files.
#[test]
fn older_codecs_are_valid_in_format_1_4() {
    for codec in [CodecId::Raw, CodecId::Rle, CodecId::Lz, CodecId::Numeric] {
        let bytes = block_bytes(codec);
        let (fixed, descriptors) = bytes.split_at(BLOCK_HEADER_SIZE);
        assert_eq!(
            decode_block_header(fixed, descriptors, 4).unwrap().codec,
            codec
        );
    }
}

/// Every file-header minor version 0 – 4 is readable; 5 is rejected as unsupported.
#[test]
fn file_headers_1_0_to_1_4_are_readable() {
    for minor in 0..=FORMAT_MINOR {
        let bytes = encode_file_header(&FileHeader {
            minor_version: minor,
            flags: 0,
            default_block_size: 262_144,
            original_size: 0,
            block_count: 0,
        });
        assert_eq!(decode_file_header(&bytes).unwrap().minor_version, minor);
    }
    let mut future = encode_file_header(&FileHeader {
        minor_version: FORMAT_MINOR,
        flags: 0,
        default_block_size: 1,
        original_size: 0,
        block_count: 0,
    });
    future[5] = FORMAT_MINOR + 1;
    let crc = ace_format::checksum(&future[..28]);
    future[28..32].copy_from_slice(&crc.to_le_bytes());
    assert!(matches!(
        decode_file_header(&future),
        Err(AceError::UnsupportedVersion { minor: 5, .. })
    ));
}
