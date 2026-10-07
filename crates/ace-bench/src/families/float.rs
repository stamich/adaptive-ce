//! Float lane families (ACE 0.5.0).
//!
//! * `float-ablation` — every candidate codec on the same data, block by block, without the
//!   planner, next to the engine result of each profile: the ground truth the Float lane and
//!   its thresholds are calibrated against.

use ace_codecs::{
    lz_encode, numeric_encode, rle_encode, ts1_decode, ts1_encode_with, ts1_encoded_len,
    TimeSeriesLayout,
};
use ace_core::{CompressionStats, LzMode};
use ace_corpus::Workload;

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
