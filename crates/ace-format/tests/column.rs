//! The `COL1` payload of a columnar block: table validation, the transpose and its inverse.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace_core::{AceError, CodecId, DecodeLimits, EntropyCodecId};
use ace_format::{
    checksum, encode_column_payload, join_columns, serialize_block, split_columns, BlockHeader,
    ColumnKind, ColumnSpec, ColumnTable, COLUMN_ENTRY_SIZE, COLUMN_HEADER_SIZE, COLUMN_MAGIC,
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

fn spec(width: u16, kind: ColumnKind) -> ColumnSpec {
    ColumnSpec { width, kind }
}

/// A payload for `data` with the schema `specs`; returns it with the outer block size.
fn payload_for(data: &[u8], specs: &[ColumnSpec]) -> (Vec<u8>, usize) {
    let widths: Vec<u16> = specs.iter().map(|s| s.width).collect();
    let (streams, tail) = split_columns(data, &widths).unwrap();
    let record: usize = widths.iter().map(|&w| usize::from(w)).sum();
    let columns: Vec<(ColumnSpec, Vec<u8>)> = specs
        .iter()
        .zip(&streams)
        .enumerate()
        .map(|(index, (s, stream))| (*s, raw_block(index as u64, stream)))
        .collect();
    let payload = encode_column_payload(&columns, (data.len() / record) as u32, tail).unwrap();
    (payload, data.len())
}

fn schema() -> Vec<ColumnSpec> {
    vec![spec(8, ColumnKind::Unsigned), spec(4, ColumnKind::Float), spec(3, ColumnKind::Bytes)]
}

#[test]
fn split_and_join_are_inverse_with_a_tail() {
    let widths = [8u16, 4, 3];
    let data: Vec<u8> = (0..(15 * 7 + 5)).map(|i| (i * 31 % 251) as u8).collect();
    let (streams, tail) = split_columns(&data, &widths).unwrap();
    assert_eq!(tail.len(), 5);
    assert_eq!(streams.iter().map(Vec::len).collect::<Vec<_>>(), vec![56, 28, 21]);
    // Column 1 of record 2 is bytes 8..12 of the second record.
    assert_eq!(&streams[1][4..8], &data[15 + 8..15 + 12]);
    assert_eq!(join_columns(&streams, &widths, tail).unwrap(), data);
}

#[test]
fn split_handles_short_and_empty_input() {
    let (streams, tail) = split_columns(&[1, 2, 3], &[4, 4]).unwrap();
    assert!(streams.iter().all(Vec::is_empty));
    assert_eq!(tail, &[1, 2, 3]);
    assert_eq!(join_columns(&streams, &[4, 4], tail).unwrap(), vec![1, 2, 3]);
    assert!(split_columns(&[], &[1]).unwrap().1.is_empty());
    assert!(split_columns(&[1], &[]).is_err());
    assert!(split_columns(&[1], &[2, 0]).is_err());
}

#[test]
fn join_rejects_inconsistent_streams() {
    let widths = [2u16, 2];
    assert!(join_columns(&[vec![0u8; 4]], &widths, &[]).is_err(), "stream count");
    assert!(join_columns(&[vec![0u8; 4], vec![0u8; 6]], &widths, &[]).is_err(), "lengths");
    assert!(join_columns(&[vec![0u8; 5], vec![0u8; 5]], &widths, &[]).is_err(), "not a multiple");
}

#[test]
fn table_round_trips_and_gives_positions() {
    let data: Vec<u8> = (0..(15 * 40 + 6)).map(|i| (i % 253) as u8).collect();
    let (payload, size) = payload_for(&data, &schema());
    let table = ColumnTable::parse(&payload, size, &DecodeLimits::default()).unwrap();
    assert_eq!((table.record_size, table.record_count, table.tail_len), (15, 40, 6));
    assert_eq!(table.columns.len(), 3);
    assert_eq!(table.stream_sizes(), vec![320, 160, 120]);
    assert_eq!(table.data_offset(), COLUMN_HEADER_SIZE + 3 * COLUMN_ENTRY_SIZE);
    let ranges = table.payload_ranges();
    assert_eq!(ranges[0].start, table.data_offset());
    assert_eq!(table.tail_range().end, payload.len());
    assert_eq!(ranges[2].end, table.tail_range().start);
    assert_eq!(&payload[table.tail_range()], &data[data.len() - 6..]);
}

#[test]
fn table_is_validated_against_the_block() {
    let data = vec![7u8; 15 * 10 + 2];
    let (payload, size) = payload_for(&data, &schema());
    let limits = DecodeLimits::default();
    let parse = |bytes: &[u8], size: usize| ColumnTable::parse(bytes, size, &limits);
    assert!(parse(&payload, size).is_ok());
    assert!(matches!(parse(&payload, size + 1), Err(AceError::Malformed(_))));
    let flip = |offset: usize, value: u8| {
        let mut bad = payload.clone();
        bad[offset] = value;
        bad
    };
    // magic, version, flags, reserved
    for (offset, value) in [(0, b'X'), (4, 1), (5, 1), (20, 1)] {
        assert!(matches!(parse(&flip(offset, value), size), Err(AceError::Malformed(_))), "{offset}");
    }
    // zero columns, record size not the sum of the widths, zero records, tail >= record
    let mut bad = payload.clone();
    bad[6..8].copy_from_slice(&0u16.to_le_bytes());
    assert!(matches!(parse(&bad, size), Err(AceError::Malformed(_))));
    let mut bad = payload.clone();
    bad[8..12].copy_from_slice(&16u32.to_le_bytes());
    assert!(parse(&bad, size).is_err());
    let mut bad = payload.clone();
    bad[12..16].copy_from_slice(&0u32.to_le_bytes());
    assert!(matches!(parse(&bad, size), Err(AceError::Malformed(_))));
    let mut bad = payload.clone();
    bad[16..20].copy_from_slice(&15u32.to_le_bytes());
    assert!(matches!(parse(&bad, size), Err(AceError::Malformed(_))));
    // entry: zero width, unknown kind, width not allowed for the kind, reserved byte, short length
    let entry = |index: usize| COLUMN_HEADER_SIZE + index * COLUMN_ENTRY_SIZE;
    let mut bad = payload.clone();
    bad[entry(0)..entry(0) + 2].copy_from_slice(&0u16.to_le_bytes());
    assert!(matches!(parse(&bad, size), Err(AceError::Malformed(_))));
    assert!(matches!(parse(&flip(entry(0) + 2, 9), size), Err(AceError::Malformed(_))));
    // column 2 is 3 bytes wide: kind "unsigned" does not allow that width
    assert!(matches!(parse(&flip(entry(2) + 2, 1), size), Err(AceError::Malformed(_))));
    // a reserved byte and a too-short serialized length
    assert!(matches!(parse(&flip(entry(0) + 3, 1), size), Err(AceError::Malformed(_))));
    let mut bad = payload.clone();
    bad[entry(0) + 4..entry(0) + 8].copy_from_slice(&5u32.to_le_bytes());
    assert!(matches!(parse(&bad, size), Err(AceError::Malformed(_))));
}

#[test]
fn width_must_fit_the_kind() {
    assert!(ColumnKind::Unsigned.accepts_width(8) && !ColumnKind::Unsigned.accepts_width(3));
    assert!(ColumnKind::Signed.accepts_width(1) && !ColumnKind::Signed.accepts_width(16));
    assert!(ColumnKind::Float.accepts_width(4) && !ColumnKind::Float.accepts_width(2));
    assert!(ColumnKind::Bytes.accepts_width(3) && !ColumnKind::Bytes.accepts_width(0));
    for id in 0..4u8 {
        assert_eq!(ColumnKind::from_id(id).unwrap().id(), id);
    }
    assert!(ColumnKind::from_id(4).is_err());
    let bad = [(spec(3, ColumnKind::Unsigned), raw_block(0, b"abc")), (spec(1, ColumnKind::Bytes), raw_block(1, b"a"))];
    assert!(encode_column_payload(&bad, 1, &[]).is_err());
}

#[test]
fn truncation_and_trailing_bytes_are_errors() {
    let data = vec![3u8; 15 * 6 + 1];
    let (payload, size) = payload_for(&data, &schema());
    let limits = DecodeLimits::default();
    for len in 0..COLUMN_HEADER_SIZE + 3 * COLUMN_ENTRY_SIZE {
        assert!(ColumnTable::parse(&payload[..len], size, &limits).is_err(), "prefix {len}");
    }
    assert!(ColumnTable::parse(&payload[..payload.len() - 1], size, &limits).is_err());
    let mut longer = payload.clone();
    longer.push(0);
    assert!(ColumnTable::parse(&longer, size, &limits).is_err());
}

#[test]
fn limits_apply_before_any_allocation() {
    let data = vec![1u8; 15 * 4];
    let (payload, size) = payload_for(&data, &schema());
    let few = DecodeLimits { max_columns: 2, ..DecodeLimits::default() };
    assert!(matches!(
        ColumnTable::parse(&payload, size, &few),
        Err(AceError::ResourceLimitExceeded("column count"))
    ));
    // A forged column count of 65 535 with no table behind it is rejected by the limit.
    let mut forged = COLUMN_MAGIC.to_vec();
    forged.extend_from_slice(&[0, 0]);
    forged.extend_from_slice(&u16::MAX.to_le_bytes());
    forged.extend_from_slice(&[0; 16]);
    assert!(matches!(
        ColumnTable::parse(&forged, 1, &DecodeLimits::default()),
        Err(AceError::ResourceLimitExceeded("column count"))
    ));
    let small = DecodeLimits { max_block_size: 30, ..DecodeLimits::default() };
    assert!(matches!(
        ColumnTable::parse(&payload, size, &small),
        Err(AceError::ResourceLimitExceeded("block size"))
    ));
    // record_count × record_size overflow is Malformed, not a wrap-around.
    let mut huge = payload.clone();
    huge[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(ColumnTable::parse(&huge, usize::MAX, &DecodeLimits { max_block_size: usize::MAX, ..DecodeLimits::default() }).is_err());
}

proptest! {
    /// Any schema and any input round-trip through the transpose and the payload table.
    #[test]
    fn random_layouts_round_trip(
        widths in proptest::collection::vec(1u16..9, 1..8),
        data in proptest::collection::vec(any::<u8>(), 0..600),
    ) {
        let (streams, tail) = split_columns(&data, &widths).unwrap();
        prop_assert_eq!(join_columns(&streams, &widths, tail).unwrap(), data.clone());
        let record: usize = widths.iter().map(|&w| usize::from(w)).sum();
        prop_assert!(tail.len() < record);
        if data.len() >= record {
            let specs: Vec<ColumnSpec> = widths.iter().map(|&w| spec(w, ColumnKind::Bytes)).collect();
            let (payload, size) = payload_for(&data, &specs);
            let table = ColumnTable::parse(&payload, size, &DecodeLimits::default()).unwrap();
            prop_assert_eq!(table.tail_range().end, payload.len());
            prop_assert_eq!(table.stream_sizes().iter().map(|&n| n as usize).sum::<usize>() + tail.len(), size);
        }
    }
}
