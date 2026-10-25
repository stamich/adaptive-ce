//! Columnar blocks (Format 1.6, `COL1`): selection by the encoder, decoding in every reader,
//! tails, random schemas, the reader gate and damaged tables.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::io::Cursor;

use ace_core::{
    AceConfig, AceError, CodecId, ColumnKind, ColumnSpec, DecodeLimits, EntropyCodecId, Schema,
    SegmentationConfig,
};
use ace_engine::{AceEngine, AceIndexedDecoder};
use ace_format::{
    checksum, encode_column_payload, serialize_block, split_columns, AceReader, AceWriter,
    BlockHeader, FileHeader, COLUMN_ENTRY_SIZE, COLUMN_HEADER_SIZE, FILE_FLAG_HAS_INDEX,
};
use proptest::prelude::*;

fn spec(width: u16, kind: ColumnKind) -> ColumnSpec {
    ColumnSpec { width, kind }
}

/// `timestamp u64 | price f64 | id u32 | flags [u8; 4]` — 24 bytes per record.
fn trade_schema() -> Schema {
    Schema::new(vec![
        spec(8, ColumnKind::Unsigned),
        spec(8, ColumnKind::Float),
        spec(4, ColumnKind::Unsigned),
        spec(4, ColumnKind::Bytes),
    ])
    .unwrap()
}

/// `records` trade records followed by `tail` extra bytes of an incomplete record.
fn trades(records: usize, tail: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(records * 24 + tail);
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    let mut price = 100.0f64;
    for i in 0..records {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        price += ((seed >> 40) % 7) as f64 * 0.01 - 0.03;
        out.extend_from_slice(&(1_700_000_000_000u64 + i as u64 * 250).to_le_bytes());
        out.extend_from_slice(&((price * 100.0).round() / 100.0).to_le_bytes());
        out.extend_from_slice(&(((seed >> 20) % 50) as u32).to_le_bytes());
        out.extend_from_slice(&[b'A' + (i % 3) as u8, 0, 0, 1]);
    }
    out.extend((0..tail).map(|i| (i * 37) as u8));
    out
}

fn config(schema: Option<Schema>) -> AceConfig {
    AceConfig {
        block_size: 1 << 20,
        threads: 1,
        schema,
        ..AceConfig::default()
    }
}

fn open(file: &[u8]) -> AceIndexedDecoder<Cursor<&[u8]>> {
    AceIndexedDecoder::open(Cursor::new(file), DecodeLimits::default()).unwrap()
}

#[test]
fn a_record_block_is_stored_by_column_and_is_smaller() {
    let input = trades(20_000, 11);
    let rowwise = AceEngine::new(config(None)).unwrap().compress(&input).unwrap();
    let engine = AceEngine::new(config(Some(trade_schema()))).unwrap();
    let (file, stats) = engine.compress_with_stats(&input).unwrap();
    assert!(file.len() < rowwise.len(), "{} !< {}", file.len(), rowwise.len());
    assert_eq!((stats.columnar_blocks, stats.column_streams), (1, 4));

    let mut reader = AceReader::new(Cursor::new(&file), DecodeLimits::default());
    let header = reader.read_file_header().unwrap();
    assert_eq!(header.minor_version, 6, "a columnar block makes the file Format 1.6");
    let (block, metadata, _) = reader.read_block().unwrap();
    assert_eq!(block.codec, CodecId::Columnar);
    assert!(metadata.is_empty() && block.transforms.is_empty() && block.dictionary.is_none());

    // Every reader returns the original bytes, the tail included.
    assert_eq!(engine.decompress(&file).unwrap(), input);
    let mut into = Vec::new();
    engine.decompress_into(&file, &mut into).unwrap();
    assert_eq!(into, input);
    let mut streamed = Vec::new();
    engine.decompress_from(Cursor::new(&file), &mut streamed).unwrap();
    assert_eq!(streamed, input);
    let mut decoder = open(&file);
    assert_eq!(decoder.read_range(0..input.len() as u64).unwrap(), input);
    let last = input.len() as u64;
    assert_eq!(decoder.read_range(last - 30..last).unwrap(), &input[input.len() - 30..]);
}

#[test]
fn without_a_schema_the_bytes_are_those_of_a_rowwise_block() {
    let input = trades(5_000, 0);
    let a = AceEngine::new(config(None)).unwrap().compress(&input).unwrap();
    let mut reader = AceReader::new(Cursor::new(&a), DecodeLimits::default());
    assert!(reader.read_file_header().unwrap().minor_version < 6);
    assert_ne!(reader.read_block().unwrap().0.codec, CodecId::Columnar);
}

#[test]
fn a_block_is_never_larger_than_rowwise() {
    // Random bytes, constant bytes and too little data: the columnar form must not be chosen
    // when it does not pay, and a short input holds no whole record.
    let random = {
        let mut seed = 99u64;
        (0..48_000)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                (seed >> 24) as u8
            })
            .collect::<Vec<u8>>()
    };
    for input in [random, vec![0u8; 48_000], trades(1, 5), trades(0, 20), Vec::new()] {
        let rowwise = AceEngine::new(config(None)).unwrap().compress(&input).unwrap();
        let columnar = AceEngine::new(config(Some(trade_schema()))).unwrap().compress(&input).unwrap();
        assert!(columnar.len() <= rowwise.len());
        assert_eq!(AceEngine::default_engine().decompress(&columnar).unwrap(), input);
    }
}

#[test]
fn segmentation_and_schema_together_keep_the_smallest() {
    let mut input = trades(12_000, 0);
    input.extend(vec![b'x'; 200_000]);
    let segmentation = SegmentationConfig {
        window_size: 4096,
        min_segment_size: 16 * 1024,
        max_segment_size: 128 * 1024,
        max_segments: 64,
    };
    let only = |segmentation: Option<SegmentationConfig>, schema: Option<Schema>| {
        AceEngine::new(AceConfig { segmentation, ..config(schema) })
            .unwrap()
            .compress(&input)
            .unwrap()
            .len()
    };
    let both = only(Some(segmentation), Some(trade_schema()));
    assert!(both <= only(None, None));
    assert!(both <= only(Some(segmentation), None));
    assert!(both <= only(None, Some(trade_schema())));
    let file = AceEngine::new(AceConfig { segmentation: Some(segmentation), ..config(Some(trade_schema())) })
        .unwrap()
        .compress(&input)
        .unwrap();
    assert_eq!(AceEngine::default_engine().decompress(&file).unwrap(), input);
}

#[test]
fn the_output_does_not_depend_on_the_worker_count() {
    let input = trades(60_000, 5);
    let reference = AceEngine::new(AceConfig { block_size: 480 * 1024, ..config(Some(trade_schema())) })
        .unwrap()
        .compress(&input)
        .unwrap();
    for threads in [2usize, 4, 8] {
        let engine = AceEngine::new(AceConfig {
            threads,
            block_size: 480 * 1024,
            ..config(Some(trade_schema()))
        })
        .unwrap();
        assert_eq!(engine.compress(&input).unwrap(), reference, "threads {threads}");
    }
}

// ---- forged files ----------------------------------------------------------------------------

/// A RAW inner block holding `data`.
fn raw_inner(id: u64, data: &[u8]) -> Vec<u8> {
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

/// A file of format `minor` holding `whole` as one columnar block with RAW columns.
fn forged_file(minor: u8, specs: &[ColumnSpec], whole: &[u8], tweak: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
    let widths: Vec<u16> = specs.iter().map(|s| s.width).collect();
    let record: usize = widths.iter().map(|&w| usize::from(w)).sum();
    let (streams, tail) = split_columns(whole, &widths).unwrap();
    let columns: Vec<(ColumnSpec, Vec<u8>)> = specs
        .iter()
        .zip(&streams)
        .enumerate()
        .map(|(i, (s, stream))| (*s, raw_inner(i as u64, stream)))
        .collect();
    let mut payload = encode_column_payload(&columns, (whole.len() / record) as u32, tail).unwrap();
    tweak(&mut payload);
    let header = BlockHeader {
        block_id: 0,
        original_size: whole.len() as u32,
        encoded_size: payload.len() as u32,
        metadata_size: 0,
        codec: CodecId::Columnar,
        entropy: EntropyCodecId::None,
        transforms: Vec::new(),
        dictionary: None,
        flags: 0,
        payload_crc32c: checksum(whole),
    };
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
    writer.write_block(&header, &[], &payload).unwrap();
    writer.finish(true).unwrap().0
}

fn assert_rejected(file: &[u8], what: &str) -> AceError {
    let engine = AceEngine::default_engine();
    let error = engine.decompress(file).expect_err(what);
    assert!(engine.decompress_from(Cursor::new(file), Vec::new()).is_err(), "{what}");
    if let Ok(mut decoder) = AceIndexedDecoder::open(Cursor::new(file), DecodeLimits::default()) {
        assert!(decoder.decode_block(0).is_err(), "{what}");
    }
    error
}

#[test]
fn forged_column_tables_are_rejected_by_every_reader() {
    let specs = [spec(4, ColumnKind::Unsigned), spec(3, ColumnKind::Bytes)];
    let whole: Vec<u8> = (0..7 * 40 + 5).map(|i| (i * 13 % 251) as u8).collect();
    assert_eq!(AceEngine::default_engine().decompress(&forged_file(6, &specs, &whole, |_| {})).unwrap(), whole);
    let entry = |index: usize| COLUMN_HEADER_SIZE + index * COLUMN_ENTRY_SIZE;
    type Tweak = Box<dyn Fn(&mut Vec<u8>)>;
    let cases: Vec<(&str, Tweak)> = vec![
        ("magic", Box::new(|p| p[0] = b'X')),
        ("version", Box::new(|p| p[4] = 1)),
        ("flags", Box::new(|p| p[5] = 1)),
        ("column count 0", Box::new(|p| p[6..8].copy_from_slice(&0u16.to_le_bytes()))),
        ("column count too large", Box::new(|p| p[6..8].copy_from_slice(&u16::MAX.to_le_bytes()))),
        ("record size", Box::new(|p| p[8..12].copy_from_slice(&8u32.to_le_bytes()))),
        ("record count", Box::new(|p| p[12..16].copy_from_slice(&41u32.to_le_bytes()))),
        ("tail length", Box::new(|p| p[16..20].copy_from_slice(&4u32.to_le_bytes()))),
        ("reserved", Box::new(|p| p[20] = 1)),
        ("width", Box::new(move |p| p[entry(0)] = 5)),
        ("kind", Box::new(move |p| p[entry(1) + 2] = 7)),
        ("kind/width mismatch", Box::new(move |p| p[entry(1) + 2] = 1)),
        ("serialized length", Box::new(move |p| p[entry(0) + 4] = p[entry(0) + 4].wrapping_add(1))),
        ("truncated", Box::new(|p| p.truncate(p.len() - 1))),
        ("trailing byte", Box::new(|p| p.push(0))),
        ("table cut short", Box::new(|p| p.truncate(COLUMN_HEADER_SIZE + 4))),
    ];
    for (what, tweak) in &cases {
        assert_rejected(&forged_file(6, &specs, &whole, |p| tweak(p)), what);
    }
    for minor in 0..=5u8 {
        let error = assert_rejected(&forged_file(minor, &specs, &whole, |_| {}), "old minor");
        assert!(matches!(error, AceError::UnsupportedVersion { .. }), "minor {minor}");
    }
}

#[test]
fn a_swapped_column_is_caught_by_the_outer_checksum() {
    // Two columns of equal width: swapping their inner blocks is consistent for the table and
    // every inner checksum, but the reconstructed block differs, so the outer CRC fails.
    let specs = [spec(2, ColumnKind::Bytes), spec(2, ColumnKind::Bytes)];
    let whole: Vec<u8> = (0..4 * 50).map(|i| (i * 7 % 253) as u8).collect();
    let widths = [2u16, 2];
    let (streams, tail) = split_columns(&whole, &widths).unwrap();
    let swapped = [
        (specs[0], raw_inner(0, &streams[1])),
        (specs[1], raw_inner(1, &streams[0])),
    ];
    let payload = encode_column_payload(&swapped, 50, tail).unwrap();
    let header = BlockHeader {
        block_id: 0,
        original_size: whole.len() as u32,
        encoded_size: payload.len() as u32,
        metadata_size: 0,
        codec: CodecId::Columnar,
        entropy: EntropyCodecId::None,
        transforms: Vec::new(),
        dictionary: None,
        flags: 0,
        payload_crc32c: checksum(&whole),
    };
    let mut writer = AceWriter::new(Vec::new());
    writer
        .write_file_header(&FileHeader {
            minor_version: 6,
            flags: FILE_FLAG_HAS_INDEX,
            default_block_size: 1 << 20,
            original_size: whole.len() as u64,
            block_count: 1,
        })
        .unwrap();
    writer.write_block(&header, &[], &payload).unwrap();
    let file = writer.finish(true).unwrap().0;
    assert!(matches!(assert_rejected(&file, "swapped"), AceError::ChecksumMismatch(_)));
}

proptest! {
    /// Any schema and any bytes round-trip through a hand-built columnar block in every reader,
    /// whatever the tail length.
    #[test]
    fn random_schemas_round_trip(
        columns in proptest::collection::vec((1u16..9, 0u8..4), 1..7),
        records in 1usize..60,
        tail in 0usize..8,
        seed in any::<u64>(),
    ) {
        let specs: Vec<ColumnSpec> = columns
            .iter()
            .map(|&(width, kind)| {
                let kind = ColumnKind::from_id(kind).unwrap();
                let width = if kind.accepts_width(width) { width } else { 1 };
                ColumnSpec { width, kind: if kind.accepts_width(width) { kind } else { ColumnKind::Bytes } }
            })
            .collect();
        let record: usize = specs.iter().map(|s| usize::from(s.width)).sum();
        let mut state = seed | 1;
        let tail = tail.min(record - 1);
        let whole: Vec<u8> = (0..records * record + tail)
            .map(|_| { state ^= state << 13; state ^= state >> 7; state ^= state << 17; (state >> 24) as u8 })
            .collect();
        let file = forged_file(6, &specs, &whole, |_| {});
        let engine = AceEngine::default_engine();
        prop_assert_eq!(engine.decompress(&file).unwrap(), whole.clone());
        let mut decoder = open(&file);
        prop_assert_eq!(decoder.read_range(0..whole.len() as u64).unwrap(), whole);
    }
}
