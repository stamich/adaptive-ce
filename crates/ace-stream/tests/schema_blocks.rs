//! Streaming compression cuts blocks at whole records exactly like `AceEngine::compress`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::io::Cursor;

use ace_core::{AceConfig, ColumnKind, ColumnSpec, Schema};
use ace_engine::AceEngine;
use ace_stream::{compress_reader_known_size, StreamLimits};

/// `records` records of `id u32 | value u64 | tag [u8; 3]` (15 bytes) plus a 4-byte tail.
fn table(records: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for i in 0..records {
        out.extend_from_slice(&(i as u32).to_le_bytes());
        out.extend_from_slice(&(1_000_000u64 + (i as u64 % 97) * 3).to_le_bytes());
        out.extend_from_slice(&[b'a' + (i % 4) as u8, 0, 7]);
    }
    out.extend_from_slice(&[9, 9, 9, 9]);
    out
}

fn schema() -> Schema {
    Schema::new(vec![
        ColumnSpec { width: 4, kind: ColumnKind::Unsigned },
        ColumnSpec { width: 8, kind: ColumnKind::Unsigned },
        ColumnSpec { width: 3, kind: ColumnKind::Bytes },
    ])
    .unwrap()
}

#[test]
fn streaming_and_in_memory_output_are_identical() {
    let input = table(50_000);
    for block_size in [1_000usize, 65_536, 1 << 20] {
        let config = AceConfig {
            block_size,
            threads: 1,
            schema: Some(schema()),
            ..AceConfig::default()
        };
        let in_memory = AceEngine::new(config.clone()).unwrap().compress(&input).unwrap();
        let mut streamed = Cursor::new(Vec::new());
        compress_reader_known_size(
            Cursor::new(&input),
            &mut streamed,
            input.len() as u64,
            config,
            StreamLimits::default(),
        )
        .unwrap();
        assert_eq!(streamed.into_inner(), in_memory, "block size {block_size}");
        assert_eq!(AceEngine::default_engine().decompress(&in_memory).unwrap(), input);
    }
}
