//! Tests of `ace_format` `writer` (formerly the inline unit tests of `src`).

use ace_core::{CodecId, DecodeLimits, EntropyCodecId};
use ace_format::*;
use ace_format::{decode_index, decode_trailer, AceReader, TRAILER_SIZE};

/// Builds a RAW block header for `payload`.
fn raw_header(block_id: u64, payload: &[u8]) -> BlockHeader {
    BlockHeader {
        block_id,
        original_size: payload.len() as u32,
        encoded_size: payload.len() as u32,
        metadata_size: 0,
        codec: CodecId::Raw,
        entropy: EntropyCodecId::None,
        transforms: Vec::new(),
        dictionary: None,
        flags: 0,
        payload_crc32c: checksum(payload),
    }
}

/// The writer records correct offsets/spans and a trailer pointing at a valid index.
#[test]
fn writer_builds_consistent_index_and_trailer() {
    let blocks: [&[u8]; 3] = [b"alpha", b"beta-beta", b"g"];
    let mut writer = AceWriter::new(Vec::new());
    writer
        .write_file_header(&FileHeader {
            minor_version: 3,
            flags: ace_format::FILE_FLAG_HAS_INDEX,
            default_block_size: 16,
            original_size: 15,
            block_count: 3,
        })
        .unwrap();
    for (id, payload) in blocks.iter().enumerate() {
        writer
            .write_block(&raw_header(id as u64, payload), &[], payload)
            .unwrap();
    }
    let (bytes, total) = writer.finish(true).unwrap();
    assert_eq!(total as usize, bytes.len());

    let trailer = decode_trailer(&bytes[bytes.len() - TRAILER_SIZE..]).unwrap();
    let start = trailer.index_offset as usize;
    let index = decode_index(
        &bytes[start..start + trailer.index_size as usize],
        &DecodeLimits::default(),
    )
    .unwrap();
    assert_eq!(
        index
            .entries
            .iter()
            .map(|e| e.original_offset)
            .collect::<Vec<_>>(),
        vec![0, 5, 14]
    );
    let mut reader = AceReader::new(std::io::Cursor::new(&bytes), DecodeLimits::default());
    reader.read_file_header().unwrap();
    for (entry, payload) in index.entries.iter().zip(blocks) {
        let (header, _, read_payload) = reader.read_block().unwrap();
        assert_eq!(header.block_id, entry.block_id);
        assert_eq!(read_payload, payload);
    }
}

/// Without an index the container ends right after the last block.
#[test]
fn writer_without_index_has_no_trailer() {
    let mut writer = AceWriter::new(Vec::new());
    writer
        .write_block(&raw_header(0, b"xyz"), &[], b"xyz")
        .unwrap();
    let written = writer.bytes_written();
    let (bytes, total) = writer.finish(false).unwrap();
    assert_eq!((bytes.len() as u64, total), (written, written));
}
