//! Numeric, ablation, NumericFast and NumericGeneral families.

use crate::prelude::*;

/// Benchmarks Planner V4 over representative integer/time-series workloads.
pub(crate) fn numeric_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    for kind in [
        "u32-counter",
        "u64-timestamps",
        "gauge-sawtooth",
        "monotonic-outliers",
        "delta-variable",
    ] {
        let data = workload_bytes(kind, 16 * 1024 * 1024);
        let profile = analyze_numeric(&data);
        let estimate = estimate_numeric(&data);
        let cfg = AceConfig {
            profile: CompressionProfile::Balanced,
            threads: 1,
            ..AceConfig::default()
        };
        let engine = AceEngine::new(cfg)?;
        let (enc_stats, encoded) = measure(|| Ok(engine.compress(&data)?))?;
        let (_, telemetry) = engine.compress_with_stats(&data)?;
        let (dec_stats, decoded) = measure(|| Ok(engine.decompress(&encoded)?))?;
        assert_eq!(decoded, data);
        let mut numeric = JsonObjectBuilder::new();
        numeric
            .field("detected", profile.detected)
            .field("width", format!("{:?}", profile.width))
            .field("confidence", profile.confidence)
            .field("monotonic_ratio", profile.monotonic_ratio)
            .field("delta_bit_width_p95", profile.delta_bit_width_p95)
            .field("dod_zero_ratio", profile.dod_zero_ratio)
            .field("dod_bit_width_p95", profile.dod_bit_width_p95)
            .field("exception_ratio", profile.exception_ratio);
        let mut row = JsonObjectBuilder::new();
        row.field("workload_id", kind)
            .field("path", "planner_v4_3")
            .field("input_bytes", data.len())
            .field("compressed_bytes", encoded.len())
            .field(
                "compression_ratio",
                data.len() as f64 / encoded.len().max(1) as f64,
            )
            .field("numeric_blocks", telemetry.numeric_blocks)
            .field("plan_distribution", &telemetry.plan_distribution)
            .field(
                "numeric_estimate",
                estimate.map(|e| format!("{:?}/{:?}/{}b", e.width, e.mode, e.bit_width)),
            )
            .value("numeric_profile", numeric.build())
            .value("compression", timing_json(&enc_stats, data.len()))
            .value("decompression", timing_json(&dec_stats, data.len()));
        rows.push(row.build());
    }
    Ok(rows)
}

/// Compares generic 0.3.1 planning against direct numeric codec and Planner V4 selection.
pub(crate) fn numeric_ablation_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    for kind in ["u32-counter", "u64-timestamps", "delta-variable"] {
        let data = workload_bytes(kind, 8 * 1024 * 1024);
        let generic_cfg = AceConfig {
            enable_numeric_specialization: false,
            threads: 1,
            ..AceConfig::default()
        };
        let generic = AceEngine::new(generic_cfg)?;
        let (generic_stats, generic_bytes) = measure(|| Ok(generic.compress(&data)?))?;
        let (numeric_stats, numeric_bytes) = measure(|| Ok(numeric_encode(&data)?))?;
        let planner_cfg = AceConfig {
            threads: 1,
            ..AceConfig::default()
        };
        let planner = AceEngine::new(planner_cfg)?;
        let (planner_stats, planner_bytes) = measure(|| Ok(planner.compress(&data)?))?;
        for (path, bytes, stats) in [
            ("generic-0.3.1-path", generic_bytes.len(), &generic_stats),
            ("direct-numeric-codec", numeric_bytes.len(), &numeric_stats),
            ("planner-v4", planner_bytes.len(), &planner_stats),
        ] {
            let mut row = JsonObjectBuilder::new();
            row.field("workload_id", kind)
                .field("path", path)
                .field("input_bytes", data.len())
                .field("compressed_bytes", bytes)
                .field("compression_ratio", data.len() as f64 / bytes.max(1) as f64)
                .value("compression", timing_json(stats, data.len()));
            rows.push(row.build());
        }
    }
    Ok(rows)
}

/// Measures end-to-end Planner V4.3 strong-numeric fast-path cost and stage telemetry.
pub(crate) fn numeric_fastpath_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    for kind in ["u32-counter", "u64-fixed-step"] {
        let data = workload_bytes(kind, 16 * 1024 * 1024);
        let cfg = AceConfig {
            profile: CompressionProfile::Balanced,
            threads: 1,
            ..AceConfig::default()
        };
        let engine = AceEngine::new(cfg)?;

        let (compression, encoded) = measure(|| Ok(engine.compress(&data)?))?;
        let (_, telemetry) = engine.compress_with_stats(&data)?;
        let (decompression, restored) = measure(|| Ok(engine.decompress(&encoded)?))?;
        assert_eq!(restored, data);

        let mut stage = JsonObjectBuilder::new();
        stage
            .field(
                "route_classify_ns",
                telemetry
                    .route_classify_time
                    .as_nanos()
                    .min(u64::MAX as u128) as u64,
            )
            .field(
                "generic_analysis_ns",
                telemetry
                    .generic_analysis_time
                    .as_nanos()
                    .min(u64::MAX as u128) as u64,
            )
            .field(
                "analysis_ns",
                telemetry.analysis_time.as_nanos().min(u64::MAX as u128) as u64,
            )
            .field(
                "planning_ns",
                telemetry.planning_time.as_nanos().min(u64::MAX as u128) as u64,
            )
            .field(
                "encoding_ns",
                telemetry.encoding_time.as_nanos().min(u64::MAX as u128) as u64,
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

        let mut row = JsonObjectBuilder::new();
        row.field("workload_id", kind)
            .field("path", "planner-v4.3-numeric-fast")
            .field("input_bytes", data.len())
            .field("compressed_bytes", encoded.len())
            .field(
                "compression_ratio",
                data.len() as f64 / encoded.len().max(1) as f64,
            )
            .field("numeric_blocks", telemetry.numeric_blocks)
            .field("plan_distribution", &telemetry.plan_distribution)
            .value("stage_timing", stage.build())
            .value("compression", timing_json(&compression, data.len()))
            .value("decompression", timing_json(&decompression, data.len()));
        rows.push(row.build());
    }
    Ok(rows)
}

/// Benchmarks the reduced NumericGeneral search against direct Numeric and end-to-end Planner V4.3.
pub(crate) fn numeric_general_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    for kind in [
        "u64-timestamps",
        "delta-variable",
        "monotonic-outliers",
        "gauge-sawtooth",
    ] {
        let data = workload_bytes(kind, 16 * 1024 * 1024);
        let direct_estimate = estimate_numeric(&data)
            .ok_or("numeric-general benchmark expected a direct numeric estimate")?;
        let (direct_timing, direct) = measure(|| Ok(numeric_encode(black_box(&data))?))?;

        let cfg = AceConfig {
            profile: CompressionProfile::Balanced,
            threads: 1,
            ..AceConfig::default()
        };
        let engine = AceEngine::new(cfg.clone())?;
        let route = RoutePolicy::classify(&data, &cfg);
        let (planner_timing, encoded) = measure(|| Ok(engine.compress(&data)?))?;
        let (_, telemetry) = engine.compress_with_stats(&data)?;
        assert_eq!(engine.decompress(&encoded)?, data);

        let mut row = JsonObjectBuilder::new();
        row.field("workload_id", kind)
            .field("path", "planner-v4.3-numeric-general")
            .field("planner_route", format!("{:?}", route.route))
            .field("input_bytes", data.len())
            .field("direct_numeric_bytes", direct.len())
            .field(
                "direct_numeric_estimated_bytes",
                direct_estimate.encoded_bytes,
            )
            .field("planner_encoded_bytes", encoded.len())
            .field(
                "planner_ratio",
                data.len() as f64 / encoded.len().max(1) as f64,
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
            )
            .value("direct_numeric", timing_json(&direct_timing, data.len()))
            .value("planner_v4_3", timing_json(&planner_timing, data.len()));
        rows.push(row.build());
    }
    Ok(rows)
}
