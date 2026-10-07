//! `release-performance`: exactly the cases the 0.4.6 performance gates are evaluated on.
//!
//! Every row carries a stable `case_id` (`<area>.<variant>`) used by
//! `tools/ace-check_regressions0.5.0.py` and by the interleaved A/B script.
//! Float lane cases (ACE 0.5.0) also measure the same engine with the lane disabled in the
//! same run, so their gates are ratios within one process, not cross-run comparisons.
//! Decompression is measured into a reused, pre-allocated buffer
//! ([`AceEngine::decompress_into`]) so the gate does not measure page faults of a fresh
//! 16 MiB vector; the allocating variant is kept as the diagnostic `decompression_alloc`.

use crate::prelude::*;

/// Size of every release-performance input.
const INPUT_BYTES: usize = 16 * 1024 * 1024;

/// One encode/decode release case.
struct CodecCase {
    /// Stable gate identifier.
    case_id: &'static str,
    /// Corpus workload name (see `ace-corpus`).
    workload: &'static str,
    /// Compression profile used by the engine.
    profile: CompressionProfile,
    /// Also measure compression with the Float lane disabled (Float lane cases).
    compare_disabled: bool,
}

/// Encode/decode cases, in report order.
const CODEC_CASES: [CodecCase; 9] = [
    CodecCase {
        case_id: "compression.fast",
        workload: "mixed",
        profile: CompressionProfile::Fast,
        compare_disabled: false,
    },
    CodecCase {
        case_id: "compression.balanced",
        workload: "mixed",
        profile: CompressionProfile::Balanced,
        compare_disabled: false,
    },
    CodecCase {
        case_id: "compression.dense",
        workload: "mixed",
        profile: CompressionProfile::Dense,
        compare_disabled: false,
    },
    CodecCase {
        case_id: "numeric_fast.u32_counter",
        workload: "u32-counter",
        profile: CompressionProfile::Balanced,
        compare_disabled: false,
    },
    CodecCase {
        case_id: "numeric_general.u64_timestamps",
        workload: "u64-timestamps",
        profile: CompressionProfile::Balanced,
        compare_disabled: false,
    },
    CodecCase {
        case_id: "numeric_general.delta_variable",
        workload: "delta-variable",
        profile: CompressionProfile::Balanced,
        compare_disabled: false,
    },
    CodecCase {
        case_id: "float_fast.f64_step",
        workload: "f64-step",
        profile: CompressionProfile::Balanced,
        compare_disabled: true,
    },
    CodecCase {
        case_id: "float_general.f64_noisy",
        workload: "f64-noisy",
        profile: CompressionProfile::Fast,
        compare_disabled: true,
    },
    CodecCase {
        case_id: "run_delta.int_sparse_change",
        workload: "int-sparse-change",
        profile: CompressionProfile::Balanced,
        compare_disabled: true,
    },
];

/// Warm range reads on an already-open decoder: `(case_id, offset, length)`.
const RANGE_CASES: [(&str, u64, u64); 2] = [
    ("random_access.warm_64k", 3_000_000, 65_536),
    ("random_access.range_4k", 5_000_000, 4_096),
];

/// Runs every release-performance case.
pub(crate) fn release_performance_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    for case in &CODEC_CASES {
        rows.push(codec_case_row(case)?);
    }
    let mixed = mixed_data(INPUT_BYTES / (1024 * 1024));
    let encoded = AceEngine::default_engine().compress(&mixed)?;
    rows.push(full_decompress_row(&mixed, &encoded)?);
    for (case_id, offset, length) in RANGE_CASES {
        rows.push(range_row(case_id, &mixed, &encoded, offset, length)?);
    }
    Ok(rows)
}

/// Single-threaded engine for `profile`.
fn engine_for(profile: CompressionProfile) -> Result<AceEngine, Box<dyn std::error::Error>> {
    Ok(AceEngine::new(AceConfig {
        profile,
        threads: 1,
        ..AceConfig::default()
    })?)
}

/// Measures compression, pre-allocated decompression and allocating decompression.
fn codec_case_row(case: &CodecCase) -> Result<Value, Box<dyn std::error::Error>> {
    let data = workload_bytes(case.workload, INPUT_BYTES);
    let engine = engine_for(case.profile)?;
    let (compression, encoded) = measure(|| Ok(engine.compress(&data)?))?;
    let (decompression, restored) = measure_prealloc_decode(&engine, &encoded, data.len())?;
    let (decompression_alloc, restored_alloc) = measure(|| Ok(engine.decompress(&encoded)?))?;
    assert_eq!(restored, data, "{}: roundtrip mismatch", case.case_id);
    assert_eq!(restored_alloc, data, "{}: roundtrip mismatch", case.case_id);

    let mut row = JsonObjectBuilder::new();
    row.field("case_id", case.case_id)
        .field("workload_id", format!("{}_16m", case.workload))
        .field("profile", format!("{:?}", case.profile))
        .field("input_bytes", data.len())
        .field("compressed_bytes", encoded.len())
        .field(
            "compression_ratio",
            data.len() as f64 / encoded.len().max(1) as f64,
        )
        .value("compression", timing_json(&compression, data.len()))
        .value("decompression", timing_json(&decompression, data.len()))
        .value(
            "decompression_alloc",
            timing_json(&decompression_alloc, data.len()),
        );
    if case.compare_disabled {
        let disabled = AceEngine::new(AceConfig {
            profile: case.profile,
            threads: 1,
            enable_float_specialization: false,
            ..AceConfig::default()
        })?;
        let (compression_disabled, plain) = measure(|| Ok(disabled.compress(&data)?))?;
        row.field("disabled_compressed_bytes", plain.len())
            .field(
                "encode_speedup_vs_disabled",
                compression_disabled.median_ns / compression.median_ns.max(1.0),
            )
            .field(
                "ratio_gain_vs_disabled",
                plain.len() as f64 / encoded.len().max(1) as f64,
            )
            .value(
                "compression_disabled",
                timing_json(&compression_disabled, data.len()),
            );
    }
    Ok(row.build())
}

/// Measures [`AceEngine::decompress_into`] with one reused output buffer and returns the
/// final buffer contents for verification.
fn measure_prealloc_decode(
    engine: &AceEngine,
    encoded: &[u8],
    capacity: usize,
) -> Result<(SampleStats, Vec<u8>), Box<dyn std::error::Error>> {
    let mut buffer = Vec::with_capacity(capacity);
    let (stats, _) = measure(|| Ok(engine.decompress_into(encoded, &mut buffer)?))?;
    Ok((stats, buffer))
}

/// Full pre-allocated decompression of the default-profile mixed file.
fn full_decompress_row(data: &[u8], encoded: &[u8]) -> Result<Value, Box<dyn std::error::Error>> {
    let engine = AceEngine::default_engine();
    let (timing, restored) = measure_prealloc_decode(&engine, encoded, data.len())?;
    assert_eq!(restored, data, "full_decompress: roundtrip mismatch");
    let mut row = JsonObjectBuilder::new();
    row.field("case_id", "full_decompress.mixed")
        .field("workload_id", "mixed_16m")
        .field("input_bytes", data.len())
        .field("compressed_bytes", encoded.len())
        .value("timing", timing_json(&timing, data.len()));
    Ok(row.build())
}

/// Warm range read on an already-open indexed decoder.
fn range_row(
    case_id: &str,
    data: &[u8],
    encoded: &[u8],
    offset: u64,
    length: u64,
) -> Result<Value, Box<dyn std::error::Error>> {
    let end = offset + length;
    let mut decoder =
        AceIndexedDecoder::open(Cursor::new(encoded), ace_core::DecodeLimits::default())?;
    let (timing, range) = measure(|| Ok(decoder.read_range(offset..end)?))?;
    assert_eq!(
        range,
        data[offset as usize..end as usize],
        "{case_id}: range mismatch"
    );
    let mut row = JsonObjectBuilder::new();
    row.field("case_id", case_id)
        .field("workload_id", "mixed_16m")
        .field("offset", offset)
        .field("logical_bytes_requested", length)
        .field("bytes_returned", range.len())
        .value("timing", timing_json(&timing, range.len()));
    Ok(row.build())
}
