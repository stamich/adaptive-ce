//! Schemas in the facade: validation, the builder and an end-to-end columnar round trip.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace::{Column, ColumnKind, Config, Engine, ErrorKind, Schema};

/// `time u64 | value f64 | id u32 | flags [u8; 4]` records.
fn records(count: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for i in 0..count {
        out.extend_from_slice(&(1_700_000_000_000u64 + i as u64 * 100).to_le_bytes());
        out.extend_from_slice(&(20.0f64 + (i % 50) as f64 * 0.25).to_le_bytes());
        out.extend_from_slice(&((i % 40) as u32).to_le_bytes());
        out.extend_from_slice(&[1, 0, 0, (i % 2) as u8]);
    }
    out
}

fn schema() -> Schema {
    Schema::new([Column::unsigned(8), Column::float(8), Column::unsigned(4), Column::bytes(4)]).unwrap()
}

#[test]
fn columns_report_their_width_and_kind() {
    let columns = [
        (Column::bytes(3), 3, ColumnKind::Bytes),
        (Column::unsigned(2), 2, ColumnKind::Unsigned),
        (Column::signed(8), 8, ColumnKind::Signed),
        (Column::float(4), 4, ColumnKind::Float),
    ];
    for (column, width, kind) in columns {
        assert_eq!((column.width(), column.kind()), (width, kind));
    }
    let schema = schema();
    assert_eq!(schema.record_size(), 24);
    assert_eq!(schema.columns().len(), 4);
    assert_eq!(schema.columns()[1], Column::float(8));
}

#[test]
fn invalid_schemas_are_invalid_configuration() {
    for columns in [
        vec![],
        vec![Column::bytes(0)],
        vec![Column::unsigned(3)],
        vec![Column::signed(16)],
        vec![Column::float(2)],
    ] {
        assert_eq!(Schema::new(columns).unwrap_err().kind(), ErrorKind::InvalidConfig);
    }
}

#[test]
fn the_builder_keeps_the_schema_and_aligns_the_block_size() {
    let config = Config::builder().schema(schema()).block_size(100_000).build().unwrap();
    assert_eq!(config.schema(), Some(schema()));
    assert_eq!(config.block_size(), 99_984); // 4166 records of 24 bytes
    assert_eq!(Config::default().schema(), None);
    assert_eq!(Config::default().block_size(), 256 * 1024);
}

#[test]
fn a_schema_with_too_many_columns_is_refused_by_build() {
    let wide = Schema::new((0..257).map(|_| Column::bytes(1))).unwrap();
    assert_eq!(
        Config::builder().schema(wide).build().unwrap_err().kind(),
        ErrorKind::InvalidConfig
    );
}

#[test]
fn a_table_round_trips_and_is_stored_by_column() {
    let input = records(30_000);
    let row_wise = Engine::new(Config::default()).unwrap().compress(&input).unwrap();
    let engine = Engine::new(Config::builder().schema(schema()).build().unwrap()).unwrap();
    let (file, stats) = engine.compress_with_stats(&input).unwrap();
    assert!(file.len() < row_wise.len(), "{} !< {}", file.len(), row_wise.len());
    assert_eq!(engine.decompress(&file).unwrap(), input);
    assert!(stats.format_minor >= 6, "columnar blocks make the file Format 1.6");
    // Files without a schema are unchanged: still readable by the default engine.
    assert_eq!(Engine::new(Config::default()).unwrap().decompress(&file).unwrap(), input);
}
