//! Compression, entropy, parallel, streaming, corpus, block and stability families.

use crate::prelude::*;

/// Builds one compression comparison row without a monolithic JSON macro.
pub(crate) fn compression_comparison_row(
    path: &str,
    input_bytes: usize,
    compressed_bytes: usize,
    compression: &SampleStats,
    decompression: &SampleStats,
) -> Value {
    let mut row = JsonObjectBuilder::new();
    row.field("workload_id", "mixed_16m")
        .field("path", path)
        .field("input_bytes", input_bytes)
        .field("compressed_bytes", compressed_bytes)
        .field(
            "compression_ratio",
            input_bytes as f64 / compressed_bytes.max(1) as f64,
        )
        .value("compression", timing_json(compression, input_bytes))
        .value("decompression", timing_json(decompression, input_bytes));
    row.build()
}

/// Benchmarks end-to-end ACE profiles against LZ4, Zstd level 3 and gzip level 6.
pub(crate) fn compression_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let data = mixed_data(16);
    let mut results = Vec::new();
    for (name, profile) in [
        ("ace-fast", CompressionProfile::Fast),
        ("ace-balanced", CompressionProfile::Balanced),
        ("ace-dense", CompressionProfile::Dense),
    ] {
        let cfg = AceConfig {
            profile,
            threads: 1,
            ..AceConfig::default()
        };
        let engine = AceEngine::new(cfg)?;
        let (enc_stats, encoded) = measure(|| Ok(engine.compress(&data)?))?;
        let (_, telemetry) = engine.compress_with_stats(&data)?;
        let (dec_stats, decoded) = measure(|| Ok(engine.decompress(&encoded)?))?;
        assert_eq!(decoded, data);
        let mut stage_timing = JsonObjectBuilder::new();
        stage_timing
            .field("analysis", telemetry.analysis_time.as_nanos())
            .field("planning", telemetry.planning_time.as_nanos())
            .field("encoding", telemetry.encoding_time.as_nanos())
            .field("serialization", telemetry.serialization_time.as_nanos())
            .field("index", telemetry.index_time.as_nanos());

        let mut row = JsonObjectBuilder::new();
        row.field("workload_id", "mixed_16m")
            .field("path", name)
            .field("input_bytes", data.len())
            .field("compressed_bytes", encoded.len())
            .field(
                "compression_ratio",
                data.len() as f64 / encoded.len().max(1) as f64,
            )
            .value("compression", timing_json(&enc_stats, data.len()))
            .value("decompression", timing_json(&dec_stats, data.len()))
            .field("plan_distribution", &telemetry.plan_distribution)
            .value("stage_timing_ns", stage_timing.build());
        results.push(row.build());
    }

    let (stats, lz4) = measure(|| Ok(lz4_flex::compress_prepend_size(&data)))?;
    let (dec, restored) = measure(|| Ok(lz4_flex::decompress_size_prepended(&lz4)?))?;
    assert_eq!(restored, data);
    results.push(compression_comparison_row(
        "lz4",
        data.len(),
        lz4.len(),
        &stats,
        &dec,
    ));

    let (stats, zstd_data) = measure(|| Ok(zstd::stream::encode_all(Cursor::new(&data), 3)?))?;
    let (dec, restored) = measure(|| Ok(zstd::stream::decode_all(Cursor::new(&zstd_data))?))?;
    assert_eq!(restored, data);
    results.push(compression_comparison_row(
        "zstd-3",
        data.len(),
        zstd_data.len(),
        &stats,
        &dec,
    ));

    let (stats, gzip) = measure(|| {
        let mut e = GzEncoder::new(Vec::new(), Compression::new(6));
        e.write_all(&data)?;
        Ok(e.finish()?)
    })?;
    let (gzip_dec, restored) = measure(|| {
        let mut d = GzDecoder::new(Cursor::new(&gzip));
        let mut out = Vec::new();
        d.read_to_end(&mut out)?;
        Ok(out)
    })?;
    assert_eq!(restored, data);
    results.push(compression_comparison_row(
        "gzip-6",
        data.len(),
        gzip.len(),
        &stats,
        &gzip_dec,
    ));
    Ok(results)
}

/// Benchmarks Huffman and scalar rANS independently on representative symbol distributions.
pub(crate) fn entropy_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let mut workloads = Vec::new();
    let datasets = vec![
        ("skewed", b"aaaaabbbbcccdde".repeat(200_000)),
        ("mixed", mixed_data(4)),
    ];
    for (id, data) in datasets {
        for path in ["huffman", "rans", "rans4x"] {
            let (stats, (metadata, payload)) = match path {
                "huffman" => measure(|| Ok(huffman_encode(&data)?))?,
                "rans" => measure(|| Ok(rans_encode(&data)?))?,
                "rans4x" => measure(|| Ok(rans4x_encode(&data)?))?,
                _ => unreachable!(),
            };
            let encoded_bytes = metadata.len() + payload.len();
            let mut row = JsonObjectBuilder::new();
            row.field("workload_id", id)
                .field("path", path)
                .field("input_bytes", data.len())
                .field("metadata_bytes", metadata.len())
                .field("payload_bytes", payload.len())
                .field("encoded_bytes", encoded_bytes)
                .field(
                    "compression_ratio",
                    data.len() as f64 / encoded_bytes.max(1) as f64,
                )
                .value("encode", timing_json(&stats, data.len()));
            workloads.push(row.build());
        }
    }
    Ok(workloads)
}

/// Benchmarks worker-count scaling and verifies bit-identical output across worker counts.
pub(crate) fn parallel_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let data = mixed_data(16);
    let mut results = Vec::new();
    let mut reference: Option<Vec<u8>> = None;
    let mut one_thread_mb_s = 0.0f64;
    let max_threads = num_cpus::get().max(1);
    let requested = [1usize, 2, 4, 6, 8, 12];
    for threads in requested
        .into_iter()
        .filter(|n| *n <= max_threads || *n == 1)
    {
        let cfg = AceConfig {
            threads,
            ..AceConfig::default()
        };
        let engine = AceEngine::new(cfg)?;
        let (stats, encoded) = measure(|| Ok(engine.compress(&data)?))?;
        if let Some(ref bytes) = reference {
            assert_eq!(bytes, &encoded);
        } else {
            reference = Some(encoded.clone());
        }
        let seconds = stats.median_ns / 1_000_000_000.0;
        let mb_s = if seconds == 0.0 {
            0.0
        } else {
            data.len() as f64 / 1_048_576.0 / seconds
        };
        if threads == 1 {
            one_thread_mb_s = mb_s;
        }
        let speedup = if one_thread_mb_s == 0.0 {
            1.0
        } else {
            mb_s / one_thread_mb_s
        };
        let efficiency = speedup / threads as f64;
        let mut row = JsonObjectBuilder::new();
        row.field("workload_id", "mixed_16m")
            .field("path", format!("threads-{threads}"))
            .field("threads", threads)
            .field("encoded_bytes", encoded.len())
            .field("deterministic_vs_1t", true)
            .field("speedup_vs_1t", speedup)
            .field("parallel_efficiency", efficiency)
            .value("compression", timing_json(&stats, data.len()));
        results.push(row.build());
    }
    Ok(results)
}

/// Benchmarks bounded-memory reader-to-writer compression for representative stream sizes.
pub(crate) fn streaming_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    for mib in [16usize, 64usize] {
        let data = mixed_data(mib);
        let cfg = AceConfig::default();
        let limits = StreamLimits::default();
        let (stats, (encoded, stream_stats)) = measure(|| {
            let mut out = Vec::new();
            let telemetry = compress_reader_known_size(
                Cursor::new(&data),
                Cursor::new(&mut out),
                data.len() as u64,
                cfg.clone(),
                limits,
            )?;
            Ok((out, telemetry))
        })?;
        let restored = AceEngine::default_engine().decompress(&encoded)?;
        assert_eq!(restored, data);
        let mut row = JsonObjectBuilder::new();
        row.field("workload_id", format!("mixed_{mib}m"))
            .field("path", "bounded_stream")
            .field("input_bytes", data.len())
            .field("output_bytes", encoded.len())
            .field(
                "compression_ratio",
                data.len() as f64 / encoded.len().max(1) as f64,
            )
            .field(
                "peak_source_buffer_bytes",
                stream_stats.peak_source_buffer_bytes,
            )
            .field("blocks", stream_stats.blocks)
            .value("timing", timing_json(&stats, data.len()));
        rows.push(row.build());
    }
    Ok(rows)
}

/// Benchmarks BALANCED compression over the deterministic 0.3.1 Corpus V2 suite.
pub(crate) fn corpus_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    for corpus_id in [
        "zeros",
        "low-cardinality",
        "runs",
        "numeric-u32",
        "delta-series",
        "structured-json",
        "random",
        "mixed",
    ] {
        let data = workload_bytes(corpus_id, 16 * 1024 * 1024);
        let config = AceConfig {
            profile: CompressionProfile::Balanced,
            threads: 1,
            ..AceConfig::default()
        };
        let engine = AceEngine::new(config)?;
        let (encode_stats, encoded) = measure(|| Ok(engine.compress(&data)?))?;
        let (decode_stats, decoded) = measure(|| Ok(engine.decompress(&encoded)?))?;
        assert_eq!(decoded, data);

        let mut row = JsonObjectBuilder::new();
        row.field("workload_id", format!("{corpus_id}_16m"))
            .field("path", "ace-balanced")
            .field("corpus_id", corpus_id)
            .field("input_bytes", data.len())
            .field("compressed_bytes", encoded.len())
            .field(
                "compression_ratio",
                data.len() as f64 / encoded.len().max(1) as f64,
            )
            .value("compression", timing_json(&encode_stats, data.len()))
            .value("decompression", timing_json(&decode_stats, data.len()));
        rows.push(row.build());
    }
    Ok(rows)
}

/// Benchmarks the stable block-size matrix used by the 0.3.1 hardening contract.
pub(crate) fn block_matrix_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let data = mixed_data(16);
    let mut rows = Vec::new();
    for block_size in [
        64 * 1024usize,
        128 * 1024usize,
        256 * 1024usize,
        512 * 1024usize,
        1024 * 1024usize,
    ] {
        let config = AceConfig {
            profile: CompressionProfile::Balanced,
            threads: 1,
            block_size,
            ..AceConfig::default()
        };
        let engine = AceEngine::new(config)?;
        let (stats, encoded) = measure(|| Ok(engine.compress(&data)?))?;
        let restored = engine.decompress(&encoded)?;
        assert_eq!(restored, data);

        let mut row = JsonObjectBuilder::new();
        row.field("workload_id", "mixed_16m")
            .field("path", "ace-balanced")
            .field("block_size_bytes", block_size)
            .field("input_bytes", data.len())
            .field("compressed_bytes", encoded.len())
            .field(
                "compression_ratio",
                data.len() as f64 / encoded.len().max(1) as f64,
            )
            .value("compression", timing_json(&stats, data.len()));
        rows.push(row.build());
    }
    Ok(rows)
}

/// Measures repeatability of the hardened BALANCED baseline and records output identity.
pub(crate) fn stability_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let data = mixed_data(16);
    let config = AceConfig {
        profile: CompressionProfile::Balanced,
        threads: 1,
        ..AceConfig::default()
    };
    let engine = AceEngine::new(config)?;

    let reference = engine.compress(&data)?;
    let (stats, candidate) = measure(|| {
        let candidate = engine.compress(&data)?;
        if candidate != reference {
            return Err("non-deterministic serialized output during stability benchmark".into());
        }
        Ok(candidate)
    })?;
    let deterministic = candidate == reference;

    let mut row = JsonObjectBuilder::new();
    row.field("workload_id", "mixed_16m")
        .field("path", "repeatability")
        .field("deterministic_output", deterministic)
        .field("encoded_bytes", reference.len())
        .value("compression", timing_json(&stats, data.len()));
    Ok(vec![row.build()])
}

/// Compares fixed block sizes with ACE 0.4 file-level automatic block-size policy.
pub(crate) fn block_policy_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let workloads = [
        ("mixed", mixed_data(16)),
        ("numeric", workload_bytes("u32-counter", 16 * 1024 * 1024)),
    ];
    let mut rows = Vec::new();
    for (kind, data) in workloads {
        for (label, policy, block_size, access) in [
            (
                "fixed-256k",
                BlockSizePolicy::Fixed,
                256 * 1024usize,
                AccessHint::Balanced,
            ),
            (
                "fixed-512k",
                BlockSizePolicy::Fixed,
                512 * 1024usize,
                AccessHint::Sequential,
            ),
            (
                "fixed-1m",
                BlockSizePolicy::Fixed,
                1024 * 1024usize,
                AccessHint::Sequential,
            ),
            (
                "auto-balanced",
                BlockSizePolicy::Auto,
                256 * 1024usize,
                AccessHint::Balanced,
            ),
            (
                "auto-sequential",
                BlockSizePolicy::Auto,
                256 * 1024usize,
                AccessHint::Sequential,
            ),
            (
                "auto-random-access",
                BlockSizePolicy::Auto,
                256 * 1024usize,
                AccessHint::RandomAccess,
            ),
        ] {
            let cfg = AceConfig {
                block_size_policy: policy,
                block_size,
                access_hint: access,
                threads: 1,
                ..AceConfig::default()
            };
            let engine = AceEngine::new(cfg)?;
            let (stats, encoded) = measure(|| Ok(engine.compress(&data)?))?;
            let mut reader = ace_format::AceReader::new(
                Cursor::new(&encoded),
                ace_core::DecodeLimits::default(),
            );
            let header = reader.read_file_header()?;
            let mut row = JsonObjectBuilder::new();
            row.field("workload_id", kind)
                .field("path", label)
                .field("selected_block_size_bytes", header.default_block_size)
                .field("compressed_bytes", encoded.len())
                .field(
                    "compression_ratio",
                    data.len() as f64 / encoded.len().max(1) as f64,
                )
                .value("compression", timing_json(&stats, data.len()));
            rows.push(row.build());
        }
    }
    Ok(rows)
}
