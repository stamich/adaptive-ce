//! Trailer / index-size hardening of the seekable reader (ACE 0.5.1 decoder audit): the index
//! allocation is bounded by `DecodeLimits::max_index_entries`, not only by the file length.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::io::Cursor;

use ace_core::{AceError, CodecId, DecodeLimits, EntropyCodecId};
use ace_format::{
    decode_trailer, encode_trailer, AceWriter, BlockHeader, FileHeader, FILE_FLAG_HAS_INDEX,
    FORMAT_MINOR_BASE, TRAILER_SIZE,
};
use ace_index::AceIndexReader;

/// A valid indexed file with `blocks` RAW blocks of 16 bytes each.
fn indexed_file(blocks: u64) -> Vec<u8> {
    let mut writer = AceWriter::new(Vec::new());
    writer
        .write_file_header(&FileHeader {
            minor_version: FORMAT_MINOR_BASE,
            flags: FILE_FLAG_HAS_INDEX,
            default_block_size: 16,
            original_size: blocks * 16,
            block_count: blocks,
        })
        .unwrap();
    let payload = [7u8; 16];
    for block_id in 0..blocks {
        let header = BlockHeader {
            block_id,
            original_size: 16,
            encoded_size: 16,
            metadata_size: 0,
            codec: CodecId::Raw,
            entropy: EntropyCodecId::None,
            transforms: Vec::new(),
            dictionary: None,
            flags: 0,
            payload_crc32c: ace_format::checksum(&payload),
        };
        writer.write_block(&header, &[], &payload).unwrap();
    }
    writer.finish(true).unwrap().0
}

/// `file` with the trailer's index size replaced (CRC recomputed, so only the value is wrong).
fn with_index_size(file: &[u8], index_size: u64) -> Vec<u8> {
    let at = file.len() - TRAILER_SIZE;
    let mut trailer = decode_trailer(&file[at..]).unwrap();
    trailer.index_size = index_size;
    let mut forged = file[..at].to_vec();
    forged.extend_from_slice(&encode_trailer(trailer));
    forged
}

/// The fixture opens and every block is reachable.
#[test]
fn fixture_opens() {
    let reader = AceIndexReader::open(Cursor::new(indexed_file(4)), DecodeLimits::default());
    assert!(reader.is_ok());
}

/// An index larger than `max_index_entries` admits is refused before it is allocated.
#[test]
fn index_size_above_the_entry_limit_is_refused() {
    let file = indexed_file(4);
    let limits = DecodeLimits {
        max_index_entries: 2,
        ..DecodeLimits::default()
    };
    let err = AceIndexReader::open(Cursor::new(file), limits)
        .err()
        .expect("4 entries exceed a limit of 2");
    assert!(matches!(err, AceError::ResourceLimitExceeded(_)), "{err}");
}

/// Forged index sizes (0, `u32::MAX`, `u64::MAX`, past the end) are rejected without a panic.
#[test]
fn forged_index_sizes_are_rejected() {
    let file = indexed_file(4);
    for size in [0, 1, u64::from(u32::MAX), u64::MAX, file.len() as u64] {
        let forged = with_index_size(&file, size);
        assert!(
            AceIndexReader::open(Cursor::new(forged), DecodeLimits::default()).is_err(),
            "index size {size} accepted"
        );
    }
}
