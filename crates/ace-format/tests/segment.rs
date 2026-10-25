//! The `SEG1` payload of a segmented block: framing, validation against the outer block, and the
//! positions the table implies.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace_core::{AceError, CodecId, DecodeLimits, EntropyCodecId};
use ace_format::{
    checksum, encode_segment_payload, parse_inner_block, read_block_header, serialize_block,
    BlockHeader, SegmentTable, BLOCK_HEADER_SIZE, SEGMENT_ENTRY_SIZE, SEGMENT_HEADER_SIZE,
    SEGMENT_MAGIC,
};
use proptest::prelude::*;

/// A RAW inner block with id `id` holding `data`.
fn raw_block(id: u64, data: &[u8]) -> Vec<u8> {
    serialize_block(
        &BlockHeader {
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
        },
        &[],
        data,
    )
}

/// Alias of [`payload_of`] with a clearer name for the prefix tests.
fn payload_for_parts(parts: &[Vec<u8>]) -> (Vec<u8>, usize) {
    payload_of(parts)
}

/// A payload of segments with the given contents.
fn payload_of(parts: &[Vec<u8>]) -> (Vec<u8>, usize) {
    let segments: Vec<(u32, Vec<u8>)> = parts
        .iter()
        .enumerate()
        .map(|(index, data)| (data.len() as u32, raw_block(index as u64, data)))
        .collect();
    let total = parts.iter().map(Vec::len).sum();
    (encode_segment_payload(&segments).unwrap(), total)
}

#[test]
fn table_round_trips_and_gives_positions() {
    let parts = vec![vec![1u8; 100], vec![2u8; 7], vec![3u8; 50_000]];
    let (payload, total) = payload_of(&parts);
    let table = SegmentTable::parse(&payload, total, &DecodeLimits::default()).unwrap();
    assert_eq!(table.entries.len(), 3);
    assert_eq!(table.data_offset(), SEGMENT_HEADER_SIZE + 3 * SEGMENT_ENTRY_SIZE);
    assert_eq!(table.logical_ranges(), vec![0..100, 100..107, 107..50_107]);
    let ranges = table.payload_ranges();
    assert_eq!(ranges[0].start, table.data_offset());
    assert_eq!(ranges.last().unwrap().end, payload.len());
    for (index, range) in ranges.iter().enumerate() {
        let block = parse_inner_block(
            &payload[range.clone()],
            index as u64,
            parts[index].len() as u32,
            6,
            &DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(block.2, parts[index]);
    }
}

#[test]
fn writer_rejects_degenerate_input() {
    let one = vec![(4u32, raw_block(0, b"abcd"))];
    assert!(encode_segment_payload(&one).is_err(), "a single segment");
    assert!(encode_segment_payload(&[]).is_err());
    let empty = vec![(0u32, raw_block(0, b"a")), (4, raw_block(1, b"abcd"))];
    assert!(encode_segment_payload(&empty).is_err(), "empty segment");
    let short = vec![(1u32, vec![0u8; BLOCK_HEADER_SIZE - 1]), (4, raw_block(1, b"abcd"))];
    assert!(encode_segment_payload(&short).is_err(), "inner block shorter than a header");
}

/// Every way to make the table disagree with the outer block or the payload is `Malformed`, and
/// the limits are enforced before the table is read.
#[test]
fn table_is_validated_against_the_block() {
    let (payload, total) = payload_of(&[vec![9u8; 64], vec![8u8; 64]]);
    let limits = DecodeLimits::default();
    let parse = |bytes: &[u8], size: usize| SegmentTable::parse(bytes, size, &limits);
    assert!(parse(&payload, total).is_ok());
    // outer size mismatch (either direction)
    assert!(matches!(parse(&payload, total + 1), Err(AceError::Malformed(_))));
    assert!(matches!(parse(&payload, total - 1), Err(AceError::Malformed(_))));
    // magic, reserved word
    let mut bad = payload.clone();
    bad[0] ^= 1;
    assert!(matches!(parse(&bad, total), Err(AceError::Malformed(_))));
    let mut bad = payload.clone();
    bad[8] = 1;
    assert!(matches!(parse(&bad, total), Err(AceError::Malformed(_))));
    // count: one, zero, too many for the payload
    for count in [0u32, 1] {
        let mut bad = payload.clone();
        bad[4..8].copy_from_slice(&count.to_le_bytes());
        assert!(matches!(parse(&bad, total), Err(AceError::Malformed(_))), "count {count}");
    }
    let mut bad = payload.clone();
    bad[4..8].copy_from_slice(&3u32.to_le_bytes());
    assert!(matches!(parse(&bad, total), Err(AceError::Malformed(_))));
    // entry: zero original size, serialized length shorter than a header, sum mismatch
    let entry = |index: usize| SEGMENT_HEADER_SIZE + index * SEGMENT_ENTRY_SIZE;
    let mut bad = payload.clone();
    bad[entry(0)..entry(0) + 4].copy_from_slice(&0u32.to_le_bytes());
    assert!(matches!(parse(&bad, total), Err(AceError::Malformed(_))));
    let mut bad = payload.clone();
    bad[entry(0) + 4..entry(0) + 8].copy_from_slice(&5u32.to_le_bytes());
    assert!(matches!(parse(&bad, total), Err(AceError::Malformed(_))));
    let mut bad = payload.clone();
    bad.push(0);
    assert!(matches!(parse(&bad, total), Err(AceError::Malformed(_))), "trailing byte");
    // truncation at every length
    for len in 0..SEGMENT_HEADER_SIZE + 2 * SEGMENT_ENTRY_SIZE {
        assert!(parse(&payload[..len], total).is_err(), "prefix {len}");
    }
}

#[test]
fn limits_apply_before_any_allocation() {
    let (payload, total) = payload_of(&[vec![1u8; 10], vec![2u8; 10], vec![3u8; 10]]);
    let tight = DecodeLimits {
        max_segments: 2,
        ..DecodeLimits::default()
    };
    assert!(matches!(
        SegmentTable::parse(&payload, total, &tight),
        Err(AceError::ResourceLimitExceeded("segment count"))
    ));
    // A forged count of u32::MAX must be rejected by the limit, not by an allocation.
    let mut forged = SEGMENT_MAGIC.to_vec();
    forged.extend_from_slice(&u32::MAX.to_le_bytes());
    forged.extend_from_slice(&[0; 4]);
    assert!(matches!(
        SegmentTable::parse(&forged, 1, &DecodeLimits::default()),
        Err(AceError::ResourceLimitExceeded("segment count"))
    ));
    let small_blocks = DecodeLimits {
        max_block_size: 9,
        ..DecodeLimits::default()
    };
    assert!(matches!(
        SegmentTable::parse(&payload, total, &small_blocks),
        Err(AceError::ResourceLimitExceeded("segment size"))
    ));
}

#[test]
fn inner_blocks_are_checked() {
    let limits = DecodeLimits::default();
    let block = raw_block(3, b"abcdef");
    assert!(parse_inner_block(&block, 3, 6, 6, &limits).is_ok());
    assert!(matches!(parse_inner_block(&block, 4, 6, 6, &limits), Err(AceError::Malformed(_))));
    assert!(matches!(parse_inner_block(&block, 3, 7, 6, &limits), Err(AceError::Malformed(_))));
    let mut trailing = block.clone();
    trailing.push(0);
    assert!(matches!(parse_inner_block(&trailing, 3, 6, 6, &limits), Err(AceError::Malformed(_))));
    // An inner container block is refused (nesting is one level deep).
    let nested = serialize_block(
        &BlockHeader {
            block_id: 0,
            original_size: 6,
            encoded_size: 0,
            metadata_size: 0,
            codec: CodecId::Segmented,
            entropy: EntropyCodecId::None,
            transforms: Vec::new(),
            dictionary: None,
            flags: 0,
            payload_crc32c: 0,
        },
        &[],
        &[],
    );
    assert!(matches!(parse_inner_block(&nested, 0, 6, 6, &limits), Err(AceError::Malformed(_))));
}

/// A range reader holds only the head and the table: it learns the count from 12 bytes, fetches
/// the table, and validates it against the outer payload length without the segments.
#[test]
fn the_table_parses_from_a_prefix() {
    let parts = vec![vec![1u8; 300], vec![2u8; 700], vec![3u8; 90]];
    let (payload, total) = payload_for_parts(&parts);
    let limits = DecodeLimits::default();
    let count = SegmentTable::count_from_head(&payload[..SEGMENT_HEADER_SIZE], &limits).unwrap();
    assert_eq!(count, 3);
    let table_len = SegmentTable::table_len(count).unwrap();
    assert_eq!(table_len, SEGMENT_HEADER_SIZE + 3 * SEGMENT_ENTRY_SIZE);
    let from_prefix =
        SegmentTable::parse_prefix(&payload[..table_len], payload.len(), total, &limits).unwrap();
    assert_eq!(from_prefix, SegmentTable::parse(&payload, total, &limits).unwrap());
    // A wrong payload length is caught from the prefix alone; so is a prefix cut inside the table.
    assert!(SegmentTable::parse_prefix(&payload[..table_len], payload.len() + 1, total, &limits).is_err());
    assert!(SegmentTable::parse_prefix(&payload[..table_len], table_len - 1, total, &limits).is_err());
    assert!(SegmentTable::parse_prefix(&payload[..table_len - 1], payload.len(), total, &limits).is_err());
    // The head alone enforces the magic, the count range and the limit.
    assert!(SegmentTable::count_from_head(&payload[..SEGMENT_HEADER_SIZE - 1], &limits).is_err());
    let few = DecodeLimits { max_segments: 2, ..DecodeLimits::default() };
    assert!(matches!(
        SegmentTable::count_from_head(&payload[..SEGMENT_HEADER_SIZE], &few),
        Err(AceError::ResourceLimitExceeded("segment count"))
    ));
    assert!(SegmentTable::table_len(usize::MAX).is_err());
}

/// The header-only read consumes the fixed header and its descriptors and stops there.
#[test]
fn the_block_header_can_be_read_without_the_payload() {
    let block = raw_block(7, b"payload bytes");
    let mut reader = std::io::Cursor::new(&block);
    let (header, header_len) = read_block_header(&mut reader, 6, &DecodeLimits::default()).unwrap();
    assert_eq!((header.block_id, header.original_size), (7, 13));
    assert_eq!(header_len, BLOCK_HEADER_SIZE);
    assert_eq!(reader.position() as usize, header_len);
    let tiny = DecodeLimits { max_block_size: 4, ..DecodeLimits::default() };
    assert!(matches!(
        read_block_header(&mut std::io::Cursor::new(&block), 6, &tiny),
        Err(AceError::ResourceLimitExceeded("block output size"))
    ));
    assert!(read_block_header(&mut std::io::Cursor::new(&block[..10]), 6, &DecodeLimits::default()).is_err());
}

proptest! {
    /// Any list of at least two non-empty segments round-trips through the table, and the
    /// ranges tile the block and the payload exactly.
    #[test]
    fn random_tables_tile_the_block(sizes in proptest::collection::vec(1usize..2000, 2..40)) {
        let parts: Vec<Vec<u8>> = sizes.iter().enumerate().map(|(i, &n)| vec![i as u8; n]).collect();
        let (payload, total) = payload_of(&parts);
        let table = SegmentTable::parse(&payload, total, &DecodeLimits::default()).unwrap();
        let logical = table.logical_ranges();
        prop_assert_eq!(logical.first().unwrap().start, 0);
        prop_assert_eq!(logical.last().unwrap().end, total);
        for pair in logical.windows(2) { prop_assert_eq!(pair[0].end, pair[1].start); }
        let physical = table.payload_ranges();
        prop_assert_eq!(physical.first().unwrap().start, table.data_offset());
        prop_assert_eq!(physical.last().unwrap().end, payload.len());
        for pair in physical.windows(2) { prop_assert_eq!(pair[0].end, pair[1].start); }
    }
}
