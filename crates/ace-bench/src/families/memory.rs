//! `memory`: scratch reuse, allocation counts and peak RSS of the main encode/decode paths.
//!
//! Allocation figures come from the harness' counting global allocator (`alloc.rs`); each
//! operation runs once, outside any timing loop.

use crate::alloc::{count_allocations, peak_rss_bytes};
use crate::prelude::*;

/// Input size of every allocation case.
const INPUT_BYTES: usize = 16 * 1024 * 1024;

/// `(path, workload, profile)` of every allocation case.
const ALLOCATION_CASES: [(&str, &str, CompressionProfile); 8] = [
    ("fast", "mixed", CompressionProfile::Fast),
    ("balanced", "mixed", CompressionProfile::Balanced),
    ("dense", "mixed", CompressionProfile::Dense),
    ("numeric_fast", "u32-counter", CompressionProfile::Balanced),
    (
        "numeric_general",
        "u64-timestamps",
        CompressionProfile::Balanced,
    ),
    ("float_fast", "f64-step", CompressionProfile::Balanced),
    ("ts1_gorilla", "f64-noisy", CompressionProfile::Fast),
    (
        "ts1_run_delta",
        "int-sparse-change",
        CompressionProfile::Balanced,
    ),
];

/// Runs the memory family: one scratch row plus one allocation row per case.
pub(crate) fn memory_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let mut rows = vec![scratch_row()?];
    for (path, workload, profile) in ALLOCATION_CASES {
        rows.push(allocation_row(path, workload, profile)?);
    }
    Ok(rows)
}

/// Counts allocations of one compress, one allocating decompress and one pre-allocated
/// decompress of `workload` with `profile` (single-threaded).
fn allocation_row(
    path: &str,
    workload: &str,
    profile: CompressionProfile,
) -> Result<Value, Box<dyn std::error::Error>> {
    let data = workload_bytes(workload, INPUT_BYTES);
    let engine = AceEngine::new(AceConfig {
        profile,
        threads: 1,
        ..AceConfig::default()
    })?;
    let (encoded, compress) = count_allocations(|| engine.compress(&data));
    let encoded = encoded?;
    let (decoded, decompress) = count_allocations(|| engine.decompress(&encoded));
    assert_eq!(decoded?, data, "{path}: roundtrip mismatch");
    let mut buffer = Vec::with_capacity(data.len());
    let (written, decompress_into) =
        count_allocations(|| engine.decompress_into(&encoded, &mut buffer));
    assert_eq!(
        written?,
        data.len(),
        "{path}: decompress_into length mismatch"
    );

    let mut row = JsonObjectBuilder::new();
    row.field("workload_id", format!("{workload}_16m"))
        .field("path", format!("allocations_{path}"))
        .field("input_bytes", data.len())
        .field("compressed_bytes", encoded.len())
        .field("compress", compress)
        .field("decompress", decompress)
        .field("decompress_into", decompress_into)
        .field(
            "compress_allocated_bytes_per_input_byte",
            compress.allocated_bytes as f64 / data.len() as f64,
        )
        .field("process_peak_rss_bytes", peak_rss_bytes());
    Ok(row.build())
}

/// Reusable scratch capacity and planner trial-encode elimination (since ACE 0.3).
fn scratch_row() -> Result<Value, Box<dyn std::error::Error>> {
    let data = mixed_data(16);
    let cfg = AceConfig {
        threads: 1,
        ..AceConfig::default()
    };
    let engine = AceEngine::new(cfg.clone())?;
    let (_, telemetry) = engine.compress_with_stats(&data)?;
    let mut scratch = ace_runtime::WorkerScratch::for_block_size(cfg.block_size);
    let reserved_before = scratch.reserved_bytes();
    scratch
        .transform_buffer
        .extend_from_slice(&data[..cfg.block_size.min(data.len())]);
    scratch.reset();
    let reserved_after_reset = scratch.reserved_bytes();
    let mut row = JsonObjectBuilder::new();
    row.field("workload_id", "mixed_16m")
        .field("path", "worker_scratch_and_planner")
        .field("block_size_bytes", cfg.block_size)
        .field("scratch_reserved_bytes", reserved_before)
        .field("scratch_reserved_after_reset_bytes", reserved_after_reset)
        .field(
            "scratch_capacity_preserved",
            reserved_before == reserved_after_reset,
        )
        .field(
            "planner_fast_path_blocks",
            telemetry.planner_fast_path_blocks,
        )
        .field(
            "planner_estimated_candidates",
            telemetry.planner_estimated_candidates,
        )
        .field(
            "planner_sampled_candidates",
            telemetry.planner_sampled_candidates,
        )
        .field(
            "planner_full_trial_encodes",
            telemetry.planner_full_trial_encodes,
        );
    Ok(row.build())
}
