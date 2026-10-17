//! Tests of `ace` `engine` (formerly the inline unit tests of `src`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::Cursor;

use ace_core::AceConfig;
use ace_engine::AceEngine;

use ace::*;
use ace::{ErrorKind, Profile};

fn sample() -> Vec<u8> {
    let mut v = Vec::new();
    for i in 0..40_000u32 {
        v.extend_from_slice(format!("2026-10-16,sensor-{},{}\n", i % 17, i * 3).as_bytes());
    }
    v
}

fn small_blocks() -> Engine {
    Engine::new(Config::builder().block_size(64 * 1024).build().unwrap()).unwrap()
}

#[test]
fn round_trip_equals_the_internal_engine() {
    let data = sample();
    let engine = Engine::default();
    let packed = engine.compress(&data).unwrap();
    let internal = AceEngine::new(AceConfig::default()).unwrap();
    assert_eq!(packed, internal.compress(&data).unwrap());
    assert_eq!(engine.decompress(&packed).unwrap(), data);
}

#[test]
fn profiles_round_trip() {
    let data = sample();
    for profile in [Profile::Fast, Profile::Balanced, Profile::Dense] {
        let engine = Engine::new(Config::builder().profile(profile).build().unwrap()).unwrap();
        let packed = engine.compress(&data).unwrap();
        assert_eq!(engine.decompress(&packed).unwrap(), data, "{profile:?}");
    }
}

#[test]
fn stats_describe_the_run() {
    let data = sample();
    let (packed, stats) = small_blocks().compress_with_stats(&data).unwrap();
    assert_eq!(stats.input_bytes, data.len() as u64);
    assert_eq!(stats.output_bytes, packed.len() as u64);
    assert_eq!(stats.format_minor, packed[5]);
    assert!(stats.blocks > 1);
    assert!(stats.ratio() > 1.0);
    let total: u64 = stats.plan_distribution.values().sum();
    assert_eq!(total, stats.blocks);
}

#[test]
fn compress_to_writes_the_same_bytes() {
    let data = sample();
    let engine = small_blocks();
    let mut sink = Vec::new();
    let stats = engine.compress_to(&data, &mut sink).unwrap();
    assert_eq!(sink, engine.compress(&data).unwrap());
    assert_eq!(stats.output_bytes, sink.len() as u64);
}

#[test]
fn streaming_matches_in_memory_compression() {
    let data = sample();
    let engine = small_blocks();
    let mut sink = Cursor::new(Vec::new());
    let stats = engine
        .compress_stream(Cursor::new(&data), &mut sink, data.len() as u64)
        .unwrap();
    let streamed = sink.into_inner();
    assert_eq!(streamed, engine.compress(&data).unwrap());
    assert_eq!(stats.input_bytes, data.len() as u64);
    assert_eq!(stats.format_minor, streamed[5]);
    let mut back = Vec::new();
    engine
        .decompress_from(Cursor::new(&streamed), &mut back)
        .unwrap();
    assert_eq!(back, data);
}

#[test]
fn streaming_rejects_a_wrong_size() {
    let data = sample();
    let engine = small_blocks();
    let mut sink = Cursor::new(Vec::new());
    let short = engine.compress_stream(Cursor::new(&data[..100]), &mut sink, 200);
    assert!(short.is_err());
}

#[test]
fn decompress_into_reuses_the_buffer() {
    let data = sample();
    let engine = Engine::default();
    let packed = engine.compress(&data).unwrap();
    let mut out = vec![9u8; 5];
    assert_eq!(
        engine.decompress_into(&packed, &mut out).unwrap(),
        data.len()
    );
    assert_eq!(out, data);
}

#[test]
fn empty_input_round_trips() {
    let engine = Engine::default();
    let packed = engine.compress(&[]).unwrap();
    assert!(engine.decompress(&packed).unwrap().is_empty());
}

#[test]
fn foreign_and_damaged_input_is_classified() {
    let engine = Engine::default();
    assert_eq!(
        engine.decompress(&[b'x'; 64]).unwrap_err().kind(),
        ErrorKind::NotAceFile
    );
    let mut packed = engine.compress(&sample()).unwrap();
    let mid = packed.len() / 2;
    packed[mid] ^= 0xFF;
    assert!(engine.decompress(&packed).is_err());
}

#[test]
fn truncated_input_is_corrupt_not_io() {
    let engine = Engine::default();
    let packed = engine.compress(&sample()).unwrap();
    for cut in [4, 20, packed.len() / 2] {
        let kind = engine.decompress(&packed[..cut]).unwrap_err().kind();
        assert!(
            matches!(kind, ErrorKind::Corrupt | ErrorKind::NotAceFile),
            "cut {cut}: {kind:?}"
        );
    }
}

#[test]
fn decode_limits_reject_before_allocating() {
    let data = sample();
    let packed = Engine::default().compress(&data).unwrap();
    let limited =
        Engine::default().with_decode_limits(DecodeLimits::builder().max_output_size(1000).build());
    assert_eq!(
        limited.decompress(&packed).unwrap_err().kind(),
        ErrorKind::LimitExceeded
    );
    assert_eq!(limited.decode_limits().max_output_size(), 1000);
}
