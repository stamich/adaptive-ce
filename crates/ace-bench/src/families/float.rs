//! Float lane families (ACE 0.5.0).
//!
//! * `float-ablation` — every candidate codec on the same data, block by block, without the
//!   planner, next to the engine result of each profile: the ground truth the Float lane and
//!   its thresholds are calibrated against.
//! * `float` — every float-family workload × profile: Float lane on vs off (bytes, routes, TS1
//!   modes, format version) and the timing with the lane on.
//! * `float-fastpath` — FloatFast blocks skip generic analysis and planning; zero fallbacks.
//! * `float-false-positive` — Corpus V3, the false-positive corpus and non-float V4 data: zero
//!   Float-route blocks and Corpus V3 bytes identical with the lane on and off.
//! * `float-estimator` — Gorilla sample estimates against exact sizes (MAE, MAPE, bias, p95)
//!   and the per-block decision regret against the better of TS1 and the generic plan.

use ace_codecs::{
    lz_encode, numeric_encode, rle_encode, ts1_decode, ts1_encode_with, ts1_encoded_len,
    TimeSeriesLayout,
};
use ace_core::{CompressionStats, LzMode};
use ace_corpus::{FalsePositiveCase, Workload};
use ace_cost::estimate_gorilla;

use crate::prelude::*;

/// Input size of every float-family workload.
const INPUT_BYTES: usize = 16 * 1024 * 1024;
/// Block size used for codec-level ablation (the engine default).
const BLOCK: usize = 256 * 1024;

/// Profiles measured by the float families.
const PROFILES: [CompressionProfile; 3] = [
    CompressionProfile::Fast,
    CompressionProfile::Balanced,
    CompressionProfile::Dense,
];

/// Stable JSON key of `profile`.
pub(crate) fn profile_name(profile: CompressionProfile) -> &'static str {
    match profile {
        CompressionProfile::Fast => "fast",
        CompressionProfile::Balanced => "balanced",
        CompressionProfile::Dense => "dense",
    }
}

/// Workloads of the float families: Corpus V4 plus integer references from Corpus V3.
pub(crate) fn float_family_workloads() -> Vec<Workload> {
    let mut workloads = Workload::CORPUS_V4.to_vec();
    workloads.extend([
        Workload::U64TimestampsMs,
        Workload::GaugeSawtooth,
        Workload::U32Counter,
        Workload::Mixed,
    ]);
    workloads
}

/// Sum over 256 KiB blocks of `encoded_len(block)`.
fn blockwise(data: &[u8], encoded_len: impl Fn(&[u8]) -> Option<usize>) -> Option<usize> {
    data.chunks(BLOCK).map(encoded_len).sum()
}

/// Codec-level sizes of every candidate (no entropy stage, no planner).
fn candidate_sizes(data: &[u8]) -> Vec<(String, Option<usize>)> {
    let mut sizes = vec![
        ("raw".to_string(), Some(data.len())),
        (
            "rle".to_string(),
            blockwise(data, |b| Some(rle_encode(b).len())),
        ),
        (
            "lz_fast".to_string(),
            blockwise(data, |b| Some(lz_encode(b, LzMode::Fast).len())),
        ),
        (
            "num1".to_string(),
            blockwise(data, |b| numeric_encode(b).ok().map(|p| p.len())),
        ),
    ];
    for layout in TimeSeriesLayout::ALL {
        sizes.push((
            format!("ts1_{}", layout.label()),
            blockwise(data, |b| ts1_encoded_len(b, layout).ok()),
        ));
    }
    sizes
}

/// Engine output size and timing for `config`.
fn engine_row(
    data: &[u8],
    config: AceConfig,
) -> Result<(Value, CompressionStats), Box<dyn std::error::Error>> {
    let engine = AceEngine::new(config)?;
    let (compression, encoded) = measure(|| Ok(engine.compress(data)?))?;
    let (_, stats) = engine.compress_with_stats(data)?;
    let mut buffer = Vec::with_capacity(data.len());
    let (decompression, _) = measure(|| Ok(engine.decompress_into(&encoded, &mut buffer)?))?;
    assert_eq!(buffer, data, "engine roundtrip");
    let mut row = JsonObjectBuilder::new();
    row.field("compressed_bytes", encoded.len())
        .field(
            "compression_ratio",
            data.len() as f64 / encoded.len().max(1) as f64,
        )
        .field("plan_distribution", &stats.plan_distribution)
        .value("compression", timing_json(&compression, data.len()))
        .value("decompression", timing_json(&decompression, data.len()));
    Ok((row.build(), stats))
}

/// Codec-level timing of the best TS1 layout (block by block, like the engine).
fn best_ts1_timing(
    data: &[u8],
    layout: TimeSeriesLayout,
) -> Result<Value, Box<dyn std::error::Error>> {
    let (encode, payloads) = measure(|| {
        let mut payloads = Vec::with_capacity(data.len() / BLOCK + 1);
        for block in data.chunks(BLOCK) {
            payloads.push(ts1_encode_with(block, layout)?);
        }
        Ok(payloads)
    })?;
    let (decode, restored) = measure(|| {
        let mut out = Vec::with_capacity(data.len());
        for (payload, block) in payloads.iter().zip(data.chunks(BLOCK)) {
            out.extend_from_slice(&ts1_decode(payload, block.len())?);
        }
        Ok(out)
    })?;
    assert_eq!(restored, data, "TS1 roundtrip");
    let mut row = JsonObjectBuilder::new();
    row.field("layout", layout.label())
        .value("encode", timing_json(&encode, data.len()))
        .value("decode", timing_json(&decode, data.len()));
    Ok(row.build())
}

/// `float-ablation`: candidate sizes, best candidate, TS1 codec speed and the engine result.
pub(crate) fn float_ablation_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    for workload in float_family_workloads() {
        let data = workload.generate(INPUT_BYTES);
        let sizes = candidate_sizes(&data);
        let best = sizes
            .iter()
            .filter_map(|(name, size)| size.map(|s| (s, name.clone())))
            .min()
            .ok_or("no candidate")?;
        let best_ts1 = TimeSeriesLayout::ALL
            .into_iter()
            .filter_map(|layout| {
                blockwise(&data, |b| ts1_encoded_len(b, layout).ok()).map(|s| (s, layout))
            })
            .min_by_key(|(size, _)| *size)
            .ok_or("no TS1 layout")?;
        let mut candidates = JsonObjectBuilder::new();
        for (name, size) in &sizes {
            candidates.field(name, size);
        }
        // One engine row per profile: the Float lane must beat (or match) each of them.
        let mut engine = JsonObjectBuilder::new();
        for profile in PROFILES {
            let config = AceConfig {
                threads: 1,
                profile,
                ..AceConfig::default()
            };
            engine.value(profile_name(profile), engine_row(&data, config)?.0);
        }
        let mut row = JsonObjectBuilder::new();
        row.field("workload_id", workload.name())
            .field("path", "float_ablation")
            .field("input_bytes", data.len())
            .value("candidate_bytes", candidates.build())
            .field("best_candidate", &best.1)
            .field("best_candidate_bytes", best.0)
            .field("best_ts1_bytes", best_ts1.0)
            .value("best_ts1", best_ts1_timing(&data, best_ts1.1)?)
            .value("engine", engine.build());
        rows.push(row.build());
    }
    Ok(rows)
}

/// Input size of the `float`, `float-fastpath` and `float-false-positive` families.
const FLOAT_BYTES: usize = 4 * 1024 * 1024;

/// Single-threaded configuration for `profile` with the Float lane on or off.
fn float_config(profile: CompressionProfile, float: bool) -> AceConfig {
    AceConfig {
        threads: 1,
        profile,
        enable_float_specialization: float,
        ..AceConfig::default()
    }
}

/// Float lane counters of one compression.
fn float_stats_json(stats: &CompressionStats) -> Value {
    let mut row = JsonObjectBuilder::new();
    row.field("blocks", stats.block_count)
        .field("float_route_blocks", stats.float_route_blocks)
        .field("float_fast_blocks", stats.float_fast_blocks)
        .field("float_fast_fallbacks", stats.float_fast_fallbacks)
        .field("time_series_blocks", stats.time_series_blocks)
        .field("ts1_gorilla_f64_blocks", stats.ts1_gorilla_f64_blocks)
        .field("ts1_gorilla_f32_blocks", stats.ts1_gorilla_f32_blocks)
        .field("ts1_run_delta_blocks", stats.ts1_run_delta_blocks)
        .field("time_series_estimates", stats.time_series_estimates)
        .field(
            "planner_full_trial_encodes",
            stats.planner_full_trial_encodes,
        );
    row.build()
}

/// `float`: Float lane on vs off for every float-family workload and profile.
pub(crate) fn float_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    for workload in float_family_workloads() {
        let data = workload.generate(FLOAT_BYTES);
        for profile in PROFILES {
            let (timed, stats) = engine_row(&data, float_config(profile, true))?;
            let encoded = AceEngine::new(float_config(profile, true))?.compress(&data)?;
            let disabled = AceEngine::new(float_config(profile, false))?.compress(&data)?;
            let mut row = JsonObjectBuilder::new();
            row.field("workload_id", workload.name())
                .field("path", "float")
                .field("profile", profile_name(profile))
                .field("input_bytes", data.len())
                .field("compressed_bytes", encoded.len())
                .field("disabled_compressed_bytes", disabled.len())
                .field(
                    "ratio_gain_vs_disabled",
                    disabled.len() as f64 / encoded.len().max(1) as f64,
                )
                .field("format_minor", encoded[5])
                .value("float_stats", float_stats_json(&stats))
                .value("engine", timed);
            rows.push(row.build());
        }
    }
    Ok(rows)
}

/// `float-fastpath`: FloatFast workloads skip generic analysis and never fall back.
pub(crate) fn float_fastpath_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    for workload in [Workload::F64Constant, Workload::F64Step] {
        let data = workload.generate(FLOAT_BYTES);
        for profile in PROFILES {
            let enabled = AceEngine::new(float_config(profile, true))?;
            let disabled = AceEngine::new(float_config(profile, false))?;
            let (_, stats) = enabled.compress_with_stats(&data)?;
            let (with_float, encoded) = measure(|| Ok(enabled.compress(&data)?))?;
            let (without_float, plain) = measure(|| Ok(disabled.compress(&data)?))?;
            let speedup = without_float.median_ns / with_float.median_ns.max(1.0);
            let mut row = JsonObjectBuilder::new();
            row.field("workload_id", workload.name())
                .field("path", "float_fastpath")
                .field("profile", profile_name(profile))
                .field("input_bytes", data.len())
                .field("compressed_bytes", encoded.len())
                .field("disabled_compressed_bytes", plain.len())
                .field(
                    "all_blocks_float_fast",
                    stats.float_fast_blocks == stats.block_count,
                )
                .field(
                    "generic_analysis_ns",
                    stats.generic_analysis_time.as_nanos() as u64,
                )
                .field("planning_ns", stats.planning_time.as_nanos() as u64)
                .field("encoding_ns", stats.encoding_time.as_nanos() as u64)
                .field(
                    "route_classify_ns",
                    stats.route_classify_time.as_nanos() as u64,
                )
                .field("encode_speedup_vs_disabled", speedup)
                .value("float_stats", float_stats_json(&stats))
                .value("compression", timing_json(&with_float, data.len()))
                .value(
                    "compression_disabled",
                    timing_json(&without_float, data.len()),
                );
            rows.push(row.build());
        }
    }
    Ok(rows)
}

/// `float-false-positive`: zero Float-route blocks off the Float lane's territory.
pub(crate) fn float_false_positive_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let mut cases: Vec<(String, Vec<u8>, bool)> = Workload::CORPUS_V3
        .iter()
        .map(|w| (w.name().to_string(), w.generate(FLOAT_BYTES), true))
        .collect();
    for workload in [
        Workload::F64Random,
        Workload::F64Special,
        Workload::IntCounterReset,
    ] {
        cases.push((
            workload.name().into(),
            workload.generate(FLOAT_BYTES),
            false,
        ));
    }
    for case in FalsePositiveCase::ALL {
        cases.push((case.name().into(), case.generate(FLOAT_BYTES), false));
    }
    let mut rows = Vec::new();
    let (mut total_blocks, mut total_float_routes, mut v3_identical) = (0u64, 0u64, true);
    for (name, data, corpus_v3) in &cases {
        for profile in PROFILES {
            let (encoded, stats) =
                AceEngine::new(float_config(profile, true))?.compress_with_stats(data)?;
            let disabled = AceEngine::new(float_config(profile, false))?.compress(data)?;
            let identical = encoded == disabled;
            total_blocks += stats.block_count;
            total_float_routes += stats.float_route_blocks;
            if *corpus_v3 {
                v3_identical &= identical;
            }
            let mut row = JsonObjectBuilder::new();
            row.field("workload_id", name)
                .field("path", "float_false_positive")
                .field("profile", profile_name(profile))
                .field("corpus_v3", corpus_v3)
                .field("identical_to_disabled", identical)
                .field("format_minor", encoded[5])
                .value("float_stats", float_stats_json(&stats));
            rows.push(row.build());
        }
    }
    let mut summary = JsonObjectBuilder::new();
    summary
        .field("workload_id", "summary")
        .field("path", "float_false_positive_summary")
        .field("blocks", total_blocks)
        .field("float_route_blocks", total_float_routes)
        .field("corpus_v3_identical", v3_identical);
    rows.push(summary.build());
    Ok(rows)
}

/// Stored bytes (metadata + payload) of the only block of a one-block container.
fn single_block_bytes(container: &[u8]) -> Result<usize, Box<dyn std::error::Error>> {
    let mut reader =
        ace_format::AceReader::new(Cursor::new(container), ace_core::DecodeLimits::default());
    reader.read_file_header()?;
    let (_, metadata, payload) = reader.read_block()?;
    Ok(metadata.len() + payload.len())
}

/// Percentile `p` (0..=1) of `values` (nearest rank; `0.0` when empty).
fn percentile(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

/// `float-estimator`: Gorilla estimate accuracy and the per-block decision regret.
pub(crate) fn float_estimator_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    let mut all_errors = Vec::new();
    for workload in float_family_workloads() {
        let data = workload.generate(INPUT_BYTES);
        let mut errors = Vec::new();
        let (mut regret_bytes, mut recalled, mut blocks) = (0u64, 0usize, 0usize);
        for profile in PROFILES {
            let enabled = AceEngine::new(float_config(profile, true))?;
            let disabled = AceEngine::new(float_config(profile, false))?;
            for block in data.chunks(BLOCK) {
                let route = RoutePolicy::classify(block, &float_config(profile, true));
                if profile == CompressionProfile::Balanced {
                    if let Some(evidence) = route.float_evidence {
                        let estimate = estimate_gorilla(block, evidence.profile.width)?;
                        let actual = ts1_encoded_len(block, estimate.layout)? as f64;
                        errors.push((estimate.estimated_bytes as f64 - actual) / actual);
                    }
                }
                let chosen = enabled.compress(block)?.len();
                let generic = disabled.compress(block)?;
                let best_ts1 = TimeSeriesLayout::ALL
                    .into_iter()
                    .filter_map(|layout| ts1_encoded_len(block, layout).ok())
                    .min()
                    .unwrap_or(usize::MAX);
                let ts1_total =
                    (generic.len() - single_block_bytes(&generic)?).saturating_add(best_ts1);
                let oracle = generic.len().min(ts1_total);
                regret_bytes += chosen.saturating_sub(oracle) as u64;
                recalled += usize::from(chosen as f64 <= oracle as f64 * 1.01);
                blocks += 1;
            }
        }
        all_errors.extend_from_slice(&errors);
        let mut row = JsonObjectBuilder::new();
        row.field("workload_id", workload.name())
            .field("path", "float_estimator")
            .value("gorilla_estimate", estimate_error_json(&errors))
            .field("blocks", blocks)
            .field("decision_recall", recalled as f64 / blocks.max(1) as f64)
            .field("decision_regret_bytes", regret_bytes)
            .field(
                "decision_regret_bytes_per_block",
                regret_bytes as f64 / blocks.max(1) as f64,
            );
        rows.push(row.build());
    }
    let mut summary = JsonObjectBuilder::new();
    summary
        .field("workload_id", "summary")
        .field("path", "float_estimator_summary")
        .value("gorilla_estimate", estimate_error_json(&all_errors));
    rows.push(summary.build());
    Ok(rows)
}

/// MAE / MAPE / bias / p95 of relative estimate errors (`(estimate − actual) / actual`).
fn estimate_error_json(errors: &[f64]) -> Value {
    let n = errors.len().max(1) as f64;
    let absolute: Vec<f64> = errors.iter().map(|e| e.abs()).collect();
    let mut row = JsonObjectBuilder::new();
    row.field("samples", errors.len())
        .field("mape", absolute.iter().sum::<f64>() / n)
        .field("bias", errors.iter().sum::<f64>() / n)
        .field("p95_abs_error", percentile(&absolute, 0.95))
        .field(
            "max_abs_error",
            absolute.iter().copied().fold(0.0, f64::max),
        );
    row.build()
}
