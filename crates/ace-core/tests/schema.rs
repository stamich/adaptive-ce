//! The record layout of columnar blocks: validation of schemas and the column kind table.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace_core::{AceConfig, AceError, ColumnKind, ColumnSpec, Schema};

fn spec(width: u16, kind: ColumnKind) -> ColumnSpec {
    ColumnSpec { width, kind }
}

#[test]
fn a_valid_schema_reports_its_layout() {
    let schema = Schema::new(vec![
        spec(8, ColumnKind::Unsigned),
        spec(4, ColumnKind::Float),
        spec(3, ColumnKind::Bytes),
    ])
    .unwrap();
    assert_eq!(schema.record_size(), 15);
    assert_eq!(schema.widths(), vec![8, 4, 3]);
    assert_eq!(schema.columns().len(), 3);
    assert_eq!(schema.columns()[1].kind, ColumnKind::Float);
}

#[test]
fn invalid_schemas_are_rejected() {
    let invalid = |columns: Vec<ColumnSpec>| {
        assert!(matches!(
            Schema::new(columns),
            Err(AceError::InvalidConfig(_))
        ));
    };
    invalid(Vec::new());
    invalid(vec![spec(0, ColumnKind::Bytes)]);
    invalid(vec![spec(3, ColumnKind::Unsigned)]);
    invalid(vec![spec(16, ColumnKind::Signed)]);
    invalid(vec![spec(2, ColumnKind::Float)]);
    invalid(vec![spec(1, ColumnKind::Bytes); usize::from(u16::MAX) + 1]);
    // A record larger than u32 cannot be described.
    invalid(vec![spec(u16::MAX, ColumnKind::Bytes); 70_000]);
    // The widest legal schema is accepted.
    assert_eq!(
        Schema::new(vec![spec(u16::MAX, ColumnKind::Bytes); 3])
            .unwrap()
            .record_size(),
        3 * u32::from(u16::MAX)
    );
}

#[test]
fn kind_ids_are_the_wire_values() {
    for (kind, id) in [
        (ColumnKind::Bytes, 0u8),
        (ColumnKind::Unsigned, 1),
        (ColumnKind::Signed, 2),
        (ColumnKind::Float, 3),
    ] {
        assert_eq!(kind.id(), id);
        assert_eq!(ColumnKind::from_id(id).unwrap(), kind);
    }
    for id in 4..=u8::MAX {
        assert!(matches!(
            ColumnKind::from_id(id),
            Err(AceError::Malformed(_))
        ));
    }
}

#[test]
fn widths_per_kind() {
    for width in [1u16, 2, 4, 8] {
        assert!(
            ColumnKind::Unsigned.accepts_width(width) && ColumnKind::Signed.accepts_width(width)
        );
    }
    assert!(!ColumnKind::Unsigned.accepts_width(3) && !ColumnKind::Signed.accepts_width(0));
    assert!(ColumnKind::Float.accepts_width(4) && ColumnKind::Float.accepts_width(8));
    assert!(!ColumnKind::Float.accepts_width(1));
    assert!(ColumnKind::Bytes.accepts_width(1) && ColumnKind::Bytes.accepts_width(u16::MAX));
    assert!(!ColumnKind::Bytes.accepts_width(0));
}

#[test]
fn block_sizes_are_rounded_to_whole_records() {
    let schema = Schema::new(vec![
        spec(8, ColumnKind::Unsigned),
        spec(4, ColumnKind::Bytes),
    ])
    .unwrap();
    assert_eq!(schema.record_size(), 12);
    // Down to a multiple, never below one record, exact multiples unchanged.
    assert_eq!(schema.align_block_size(262_144), 262_140);
    assert_eq!(schema.align_block_size(24), 24);
    assert_eq!(schema.align_block_size(23), 12);
    assert_eq!(schema.align_block_size(1), 12);
    assert_eq!(schema.align_block_size(0), 12);
    assert_eq!(schema.align_block_size(usize::MAX) % 12, 0);
}

#[test]
fn the_configuration_aligns_only_with_a_schema() {
    let plain = AceConfig::default();
    assert_eq!(plain.aligned_block_size(1000), 1000);
    let columnar = AceConfig {
        schema: Some(Schema::new(vec![spec(7, ColumnKind::Bytes)]).unwrap()),
        ..AceConfig::default()
    };
    assert_eq!(columnar.aligned_block_size(1000), 994);
    assert_eq!(columnar.aligned_block_size(3), 7);
}
