//! `format-1-5` (ACE 0.6.1): the Phase B changes against their targets.
//!
//! * `format_1_5` rows — ratio, format version and routes for every Phase B workload and
//!   profile, next to the same data with the Float lane disabled (the 0.6.0 behaviour for
//!   floats and sparse integers).
//! * `format_1_5_codec` rows — codec-level speed of ALP (TS1 mode 4) and Patched FOR (NUM1
//!   mode 4), block by block like the engine.
//! * `summary` — DENSE is never larger than BALANCED on any corpus workload.
//!
//! The gates live in `tools/ace-check_regressions0.6.1.py` (section `format15`).

use ace_codecs::{numeric_decode, numeric_encode_with, NumericMode, TimeSeriesLayout};
use ace_core::NumericWidth;
use ace_corpus::Workload;

use super::float::{best_ts1_timing, profile_name};
use crate::prelude::*;

/// Input size of the ratio rows.
const BYTES: usize = 4 * 1024 * 1024;
/// Input size of the DENSE-versus-BALANCED sweep over the whole corpus.
const SWEEP_BYTES: usize = 1024 * 1024;
/// Engine block size, the codec timing mirrors it.
const BLOCK: usize = 256 * 1024;

/// Workloads whose bytes Phase B targets.
const WORKLOADS: [Workload; 9] = [
    Workload::IntCounterReset,
    Workload::MonotonicOutliers,
    Workload::GaugeSawtooth,
    Workload::DeltaVariable,
    Workload::IntGeometricDelta,
    Workload::IntSlowDrift,
    Workload::F64FinancialPrice,
    Workload::F64DecimalPrice,
    Workload::F32DecimalSensor,
];

/// Profiles measured.
const PROFILES: [CompressionProfile; 3] = [
    CompressionProfile::Fast,
    CompressionProfile::Balanced,
    CompressionProfile::Dense,
];

/// Single-threaded configuration with the Float lane on or off.
fn config(profile: CompressionProfile, float: bool) -> AceConfig {
    AceConfig {
        threads: 1,
        profile,
        enable_float_specialization: float,
        ..AceConfig::default()
    }
}

/// Codec-level timing of NUM1 Patched FOR over the blocks of `data` (u32 lanes).
fn patched_timing(data: &[u8]) -> Result<Value, Box<dyn std::error::Error>> {
    let (encode, payloads) = measure(|| {
        let mut payloads = Vec::with_capacity(data.len() / BLOCK + 1);
        for block in data.chunks(BLOCK) {
            payloads.push(numeric_encode_with(
                block,
                NumericWidth::U32,
                NumericMode::Patched,
            )?);
        }
        Ok(payloads)
    })?;
    let (decode, restored) = measure(|| {
        let mut out = Vec::with_capacity(data.len());
        for (payload, block) in payloads.iter().zip(data.chunks(BLOCK)) {
            out.extend_from_slice(&numeric_decode(payload, block.len())?);
        }
        Ok(out)
    })?;
    assert_eq!(restored, data, "Patched FOR roundtrip");
    let bytes: usize = payloads.iter().map(Vec::len).sum();
    let mut row = JsonObjectBuilder::new();
    row.field("layout", "num1_patched_u32")
        .field("compressed_bytes", bytes)
        .value("encode", timing_json(&encode, data.len()))
        .value("decode", timing_json(&decode, data.len()));
    Ok(row.build())
}

/// `format-1-5`: ratios, codec speed and the DENSE guarantee.
pub(crate) fn format15_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    for workload in WORKLOADS {
        let data = generate(workload, BYTES);
        for profile in PROFILES {
            let engine = AceEngine::new(config(profile, true))?;
            let (encoded, stats) = engine.compress_with_stats(&data)?;
            assert_eq!(engine.decompress(&encoded)?, data, "roundtrip");
            let disabled = AceEngine::new(config(profile, false))?.compress(&data)?;
            let mut row = JsonObjectBuilder::new();
            row.field("workload_id", workload.name())
                .field("path", "format_1_5")
                .field("profile", profile_name(profile))
                .field("input_bytes", data.len())
                .field("compressed_bytes", encoded.len())
                .field(
                    "compression_ratio",
                    data.len() as f64 / encoded.len().max(1) as f64,
                )
                .field("disabled_compressed_bytes", disabled.len())
                .field(
                    "gain_vs_float_lane_disabled",
                    disabled.len() as f64 / encoded.len().max(1) as f64,
                )
                .field("format_minor", encoded[5])
                .field("blocks", stats.block_count)
                .field("ts1_alp_blocks", stats.ts1_alp_blocks)
                .field("ts1_run_delta_blocks", stats.ts1_run_delta_blocks)
                .field("numeric_blocks", stats.numeric_blocks);
            rows.push(row.build());
        }
    }

    let decimals = generate(Workload::F64DecimalPrice, BYTES);
    let mut alp = JsonObjectBuilder::new();
    alp.field("workload_id", "f64-decimal-price")
        .field("path", "format_1_5_codec")
        .value(
            "alp",
            best_ts1_timing(&decimals, TimeSeriesLayout::ALP_F64)?,
        );
    rows.push(alp.build());
    let outliers = generate(Workload::MonotonicOutliers, BYTES);
    let mut patched = JsonObjectBuilder::new();
    patched
        .field("workload_id", "monotonic-outliers")
        .field("path", "format_1_5_codec")
        .value("patched", patched_timing(&outliers)?);
    rows.push(patched.build());

    let (mut larger, mut checked) = (Vec::new(), 0u64);
    for workload in Workload::ALL {
        let data = generate(workload, SWEEP_BYTES);
        let balanced = AceEngine::new(config(CompressionProfile::Balanced, true))?
            .compress(&data)?
            .len();
        let dense = AceEngine::new(config(CompressionProfile::Dense, true))?
            .compress(&data)?
            .len();
        checked += 1;
        if dense > balanced {
            larger.push(workload.name());
        }
    }
    let mut summary = JsonObjectBuilder::new();
    summary
        .field("workload_id", "summary")
        .field("path", "format_1_5_summary")
        .field("workloads_checked", checked)
        .field("dense_larger_than_balanced", larger.len())
        .field("dense_larger_workloads", larger);
    rows.push(summary.build());
    Ok(rows)
}
