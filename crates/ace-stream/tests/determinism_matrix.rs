//! Determinism matrix (ACE 0.4.6, extended with the Float lane in 0.5.0 and with 12 / 16 / 32
//! workers in 0.5.1): workload × profile × worker count × API path must all produce
//! byte-identical containers.
//!
//! The SIMD axis is covered by running the golden test with `ACE_SIMD=scalar` (the backend
//! choice is process-wide), and the process axis by `ace-release0.6.0.sh`, which hashes the
//! output of repeated CLI invocations.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: a panic is a failing test

use std::io::Cursor;

use ace_core::{AceConfig, BlockSizePolicy, CompressionProfile};
use ace_corpus::Workload;
use ace_engine::AceEngine;
use ace_stream::{compress_reader_known_size, StreamLimits};

/// Input size per workload (eight 256 KiB blocks).
const BYTES: usize = 2 * 1024 * 1024;

/// Workloads spanning the generic, structured, both numeric and both Float routes plus
/// RunDelta on integers.
const WORKLOADS: [Workload; 7] = [
    Workload::Mixed,
    Workload::StructuredJson,
    Workload::U32Counter,
    Workload::U64TimestampsNs,
    Workload::F64Step,
    Workload::F64Noisy,
    Workload::IntSparseChange,
];

/// Profiles of the matrix.
const PROFILES: [CompressionProfile; 3] = [
    CompressionProfile::Fast,
    CompressionProfile::Balanced,
    CompressionProfile::Dense,
];

/// Worker counts of the matrix (`0` = all logical CPUs).
const THREADS: [usize; 5] = [1, 2, 4, 8, 0];

/// Engine with `profile` and `threads`.
fn config(profile: CompressionProfile, threads: usize) -> AceConfig {
    AceConfig {
        profile,
        threads,
        ..AceConfig::default()
    }
}

/// Every worker count, `compress_to` and the streaming encoder produce the 1-thread bytes.
#[test]
fn byte_identical_across_threads_and_api_paths() {
    for workload in WORKLOADS {
        let data = workload.generate(BYTES);
        for profile in PROFILES {
            let reference = AceEngine::new(config(profile, 1))
                .unwrap()
                .compress(&data)
                .unwrap();
            for threads in THREADS {
                let engine = AceEngine::new(config(profile, threads)).unwrap();
                assert_eq!(
                    engine.compress(&data).unwrap(),
                    reference,
                    "{workload} {profile:?} {threads}T"
                );
                let mut sink = Vec::new();
                engine.compress_to(&data, &mut sink).unwrap();
                assert_eq!(sink, reference, "{workload} {profile:?} compress_to");
            }
            let mut streamed = Vec::new();
            compress_reader_known_size(
                Cursor::new(&data),
                Cursor::new(&mut streamed),
                data.len() as u64,
                config(profile, 1),
                StreamLimits::default(),
            )
            .unwrap();
            assert_eq!(streamed, reference, "{workload} {profile:?} streaming");
        }
    }
}

/// Worker counts above 8 (rayon splits work differently there; 12 / 16 / 32 match the
/// reference Ryzen 9 5950X), on 64 KiB blocks so that 32 blocks are really in flight.
const HIGH_THREADS: [usize; 3] = [12, 16, 32];

/// Many small blocks and in-flight batch sizes from 1 to all, at 12 / 16 / 32 workers; the
/// streaming encoder is run with the 32-worker configuration on the mixed workload.
#[test]
fn byte_identical_at_high_worker_counts() {
    let small_blocks = |profile, threads, in_flight| AceConfig {
        block_size: 64 * 1024,
        block_size_policy: BlockSizePolicy::Fixed,
        max_in_flight_blocks: in_flight,
        ..config(profile, threads)
    };
    for workload in WORKLOADS {
        let data = workload.generate(BYTES);
        for profile in PROFILES {
            let reference = AceEngine::new(small_blocks(profile, 1, 64))
                .unwrap()
                .compress(&data)
                .unwrap();
            for threads in HIGH_THREADS {
                for in_flight in [1, 7, 64] {
                    let engine = AceEngine::new(small_blocks(profile, threads, in_flight)).unwrap();
                    assert_eq!(
                        engine.compress(&data).unwrap(),
                        reference,
                        "{workload} {profile:?} {threads}T in-flight {in_flight}"
                    );
                }
            }
            if workload == Workload::Mixed {
                let mut streamed = Vec::new();
                compress_reader_known_size(
                    Cursor::new(&data),
                    Cursor::new(&mut streamed),
                    data.len() as u64,
                    small_blocks(profile, 32, 64),
                    StreamLimits::default(),
                )
                .unwrap();
                assert_eq!(streamed, reference, "{workload} {profile:?} streaming 32T");
            }
        }
    }
}

/// Repeated compression in one process is stable (no hidden mutable state).
#[test]
fn repeated_compression_is_stable() {
    let data = Workload::Mixed.generate(BYTES);
    let engine = AceEngine::new(config(CompressionProfile::Balanced, 0)).unwrap();
    let first = engine.compress(&data).unwrap();
    for _ in 0..5 {
        assert_eq!(engine.compress(&data).unwrap(), first);
    }
}

/// `decompress_into` with a reused buffer equals `decompress` for every profile.
#[test]
fn decompress_into_reuses_buffer_and_matches_decompress() {
    let mut buffer = Vec::new();
    for workload in WORKLOADS {
        let data = workload.generate(BYTES / 2);
        for profile in PROFILES {
            let engine = AceEngine::new(config(profile, 1)).unwrap();
            let encoded = engine.compress(&data).unwrap();
            let written = engine.decompress_into(&encoded, &mut buffer).unwrap();
            assert_eq!(written, data.len());
            assert_eq!(buffer, data);
            assert_eq!(engine.decompress(&encoded).unwrap(), data);
        }
    }
}
