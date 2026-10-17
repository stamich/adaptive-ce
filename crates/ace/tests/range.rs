//! Tests of `ace` `range` (formerly the inline unit tests of `src`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::Cursor;

use ace::*;
use ace::{Config, ErrorKind};

fn file() -> (Vec<u8>, Vec<u8>, Engine) {
    let data: Vec<u8> = (0..120_000u32)
        .flat_map(|i| (i / 3).to_le_bytes())
        .collect();
    let engine = Engine::new(Config::builder().block_size(32 * 1024).build().unwrap()).unwrap();
    let packed = engine.compress(&data).unwrap();
    (data, packed, engine)
}

#[test]
fn a_range_equals_the_slice_of_the_input() {
    let (data, packed, engine) = file();
    let mut r = engine.open_range_reader(Cursor::new(packed)).unwrap();
    assert_eq!(r.len(), data.len() as u64);
    assert!(r.block_count() > 3);
    for range in [
        0..0,
        0..1,
        5..40_000,
        32_767..32_769,
        100_000..data.len() as u64,
    ] {
        let want = &data[range.start as usize..range.end as usize];
        assert_eq!(r.read_range(range.clone()).unwrap(), want, "{range:?}");
    }
}

#[test]
fn stats_count_only_the_blocks_touched() {
    let (_, packed, engine) = file();
    let mut r = engine
        .open_range_reader(Cursor::new(packed.clone()))
        .unwrap();
    let (bytes, stats) = r.read_range_with_stats(10..20).unwrap();
    assert_eq!(bytes.len(), 10);
    assert_eq!((stats.logical_bytes, stats.blocks), (10, 1));
    assert!(stats.physical_bytes > 0 && stats.physical_bytes < packed.len() as u64);
    assert!(stats.read_amplification() > 1.0);
    assert_eq!(RangeStats::default().read_amplification(), 0.0);
}

#[test]
fn bad_ranges_and_blocks_are_rejected() {
    let (data, packed, engine) = file();
    let mut r = engine.open_range_reader(Cursor::new(packed)).unwrap();
    let end = data.len() as u64;
    assert_eq!(
        r.read_range(0..end + 1).unwrap_err().kind(),
        ErrorKind::InvalidConfig
    );
    #[allow(clippy::reversed_empty_ranges)]
    let reversed = 10..5;
    assert_eq!(
        r.read_range(reversed).unwrap_err().kind(),
        ErrorKind::InvalidConfig
    );
    let e = r.read_block(10_000).unwrap_err();
    assert_eq!(
        (e.kind(), e.block_id()),
        (ErrorKind::BlockNotFound, Some(10_000))
    );
    assert_eq!(r.read_block(0).unwrap(), data[..32 * 1024]);
}

#[test]
fn foreign_input_and_missing_index_fail_to_open() {
    let e = RangeReader::open(Cursor::new(vec![b'x'; 64])).unwrap_err();
    assert_eq!(e.kind(), ErrorKind::NotAceFile);
    let engine = Engine::new(Config::builder().write_index(false).build().unwrap()).unwrap();
    let packed = engine.compress(&[1u8; 5000]).unwrap();
    assert!(RangeReader::open(Cursor::new(packed)).is_err());
}
