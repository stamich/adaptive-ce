//! The record corpus: determinism, record sizes and layouts.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use ace_corpus::{FieldKind, RecordWorkload};

#[test]
fn layouts_add_up_to_the_record_size() {
    let sizes: Vec<usize> = RecordWorkload::ALL
        .iter()
        .map(|w| w.record_size())
        .collect();
    assert_eq!(sizes, vec![24, 16, 32]);
    for workload in RecordWorkload::ALL {
        assert!(!workload.layout().is_empty());
        for &(width, kind) in workload.layout() {
            assert!(width > 0);
            match kind {
                FieldKind::Unsigned | FieldKind::Signed => assert!(matches!(width, 1 | 2 | 4 | 8)),
                FieldKind::Float => assert!(matches!(width, 4 | 8)),
                FieldKind::Bytes => {}
            }
        }
    }
}

#[test]
fn generation_is_deterministic_and_whole_records() {
    for workload in RecordWorkload::ALL {
        let record = workload.record_size();
        let a = workload.generate_records(1000);
        assert_eq!(a.len(), 1000 * record);
        assert_eq!(a, workload.generate_records(1000));
        // A prefix of a longer table is the shorter table.
        assert_eq!(&workload.generate_records(2000)[..a.len()], &a[..]);
        // `generate` rounds down to whole records.
        assert_eq!(workload.generate(1000 * record + record - 1), a);
        assert!(workload.generate(record - 1).is_empty());
    }
}

#[test]
fn names_are_distinct_and_stable() {
    let names: Vec<&str> = RecordWorkload::ALL.iter().map(|w| w.name()).collect();
    assert_eq!(
        names,
        vec!["records-trades", "records-telemetry", "records-events"]
    );
}

/// The first bytes are fixed (guards against accidental generator changes).
#[test]
fn first_records_are_fixed() {
    let trades = RecordWorkload::Trades.generate_records(2);
    let first_time = u64::from_le_bytes(trades[..8].try_into().unwrap());
    assert!(
        (1_700_000_000_000..1_700_000_000_016).contains(&first_time),
        "{first_time}"
    );
    assert_eq!(
        &trades[20..23],
        &[b'A', 0, 0],
        "flags start with the record's cycle letter"
    );
    let telemetry = RecordWorkload::Telemetry.generate_records(3);
    assert_eq!(&telemetry[..2], &0u16.to_le_bytes());
    assert_eq!(&telemetry[16..18], &1u16.to_le_bytes());
    assert_eq!(&telemetry[32..34], &2u16.to_le_bytes());
}

#[test]
fn workloads_are_found_by_name() {
    for workload in RecordWorkload::ALL {
        assert_eq!(RecordWorkload::from_name(workload.name()), Some(workload));
    }
    assert_eq!(RecordWorkload::from_name("trades"), None);
}
