//! Tests of `ace_format` `index` (formerly the inline unit tests of `src`).

use ace_format::*;

/// The allocation-free intersection indices must identify the same contiguous blocks as the public vector API.
#[test]
fn intersecting_indices_match_entries() {
    let index = BlockIndex {
        entries: vec![
            BlockIndexEntry {
                block_id: 0,
                original_offset: 0,
                original_size: 100,
                file_offset: 10,
                encoded_span: 20,
                flags: 0,
            },
            BlockIndexEntry {
                block_id: 1,
                original_offset: 100,
                original_size: 100,
                file_offset: 30,
                encoded_span: 20,
                flags: 0,
            },
            BlockIndexEntry {
                block_id: 2,
                original_offset: 200,
                original_size: 100,
                file_offset: 50,
                encoded_span: 20,
                flags: 0,
            },
        ],
    };
    assert_eq!(index.intersecting_indices(90, 210), 0..3);
    let ids = index
        .intersecting(90, 210)
        .into_iter()
        .map(|entry| entry.block_id)
        .collect::<Vec<_>>();
    assert_eq!(ids, vec![0, 1, 2]);
    assert_eq!(index.intersecting_indices(100, 200), 1..2);
}
