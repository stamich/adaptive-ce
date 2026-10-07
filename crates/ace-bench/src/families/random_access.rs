//! Random-access families (index, ranges, plan diff).

use crate::prelude::*;

/// Benchmarks full reconstruction, codec-diverse single blocks and crossing logical ranges.
pub(crate) fn random_access_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let data = mixed_data(16);
    let engine = AceEngine::default_engine();
    let encoded = engine.compress(&data)?;
    let (full, restored) = measure(|| Ok(engine.decompress(&encoded)?))?;
    assert_eq!(restored, data);
    let mut full_row = JsonObjectBuilder::new();
    full_row
        .field("workload_id", "mixed_16m")
        .field("path", "full_decompress")
        .field("bytes_returned", data.len())
        .value("timing", timing_json(&full, data.len()));
    let mut out = vec![full_row.build()];

    // Opening/index validation is measured separately so random-access latency represents
    // an already-open archive, which is the storage-engine usage model.
    let (open_stats, opened) = measure(|| {
        Ok(AceIndexedDecoder::open(
            Cursor::new(&encoded),
            ace_core::DecodeLimits::default(),
        )?)
    })?;
    let mut open_row = JsonObjectBuilder::new();
    open_row
        .field("workload_id", "mixed_16m")
        .field("path", "decoder_open")
        .field("bytes_returned", 0usize)
        .value("timing", timing_json(&open_stats, encoded.len()));
    out.push(open_row.build());
    drop(opened);

    for block_id in [0u64, 20, 40, 60] {
        let metrics_decoder =
            AceIndexedDecoder::open(Cursor::new(&encoded), ace_core::DecodeLimits::default())?;
        let start = block_id * 262_144;
        let end = (start + 262_144).min(data.len() as u64);
        let metrics = metrics_decoder.range_metrics(start..end)?;
        let mut decoder =
            AceIndexedDecoder::open(Cursor::new(&encoded), ace_core::DecodeLimits::default())?;
        let (stats, block) = measure(|| Ok(decoder.decode_block(block_id)?))?;
        let mut row = JsonObjectBuilder::new();
        row.field("workload_id", "mixed_16m")
            .field("path", "decode_block")
            .field("block_id", block_id)
            .field("data_class", data_class(block_id as usize, 64))
            .field("bytes_returned", block.len())
            .field("physical_bytes_read", metrics.physical_bytes_read)
            .field("blocks_touched", metrics.blocks_touched)
            .field("physical_to_logical_ratio", metrics.overread_ratio())
            .value("timing", timing_json(&stats, block.len()));
        out.push(row.build());
    }

    let cold_start = 3_000_000u64;
    let cold_end = cold_start + 65_536u64;
    let (cold_stats, cold_range) = measure(|| {
        let mut decoder =
            AceIndexedDecoder::open(Cursor::new(&encoded), ace_core::DecodeLimits::default())?;
        Ok(decoder.read_range(cold_start..cold_end)?)
    })?;
    assert_eq!(cold_range, data[cold_start as usize..cold_end as usize]);
    let mut cold_row = JsonObjectBuilder::new();
    cold_row
        .field("workload_id", "mixed_16m")
        .field("path", "range_64k_cold")
        .field("logical_bytes_requested", 65_536u64)
        .field("bytes_returned", cold_range.len())
        .value("timing", timing_json(&cold_stats, cold_range.len()));
    out.push(cold_row.build());

    for (name, start, len) in [
        ("range_64k_warm", 3_000_000u64, 65_536u64),
        ("range_cross_2", 262_144 - 32_768, 131_072),
        ("range_cross_4", 262_144 * 3 - 65_536, 786_432),
    ] {
        let end = (start + len).min(data.len() as u64);
        let mut decoder =
            AceIndexedDecoder::open(Cursor::new(&encoded), ace_core::DecodeLimits::default())?;
        let (stats, (range, metrics)) =
            measure(|| Ok(decoder.read_range_with_metrics(start..end)?))?;
        assert_eq!(range, data[start as usize..end as usize]);
        let mut row = JsonObjectBuilder::new();
        row.field("workload_id", "mixed_16m")
            .field("path", name)
            .field("offset", start)
            .field("logical_bytes_requested", end - start)
            .field("bytes_returned", range.len())
            .field("physical_bytes_read", metrics.physical_bytes_read)
            .field("blocks_touched", metrics.blocks_touched)
            .field("blocks_decoded", metrics.blocks_decoded)
            .field("physical_to_logical_ratio", metrics.overread_ratio())
            .value("timing", timing_json(&stats, range.len()));
        out.push(row.build());
    }
    Ok(out)
}

/// Benchmarks aligned and unaligned warm range reads over multiple logical request sizes.
pub(crate) fn random_access_extended_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let data = mixed_data(16);
    let engine = AceEngine::default_engine();
    let encoded = engine.compress(&data)?;
    let mut rows = Vec::new();

    for requested in [
        4 * 1024u64,
        16 * 1024u64,
        64 * 1024u64,
        256 * 1024u64,
        1024 * 1024u64,
    ] {
        for (alignment, start) in [
            ("aligned", 2 * 262_144u64),
            ("unaligned", 2 * 262_144u64 + 12_345),
        ] {
            let end = (start + requested).min(data.len() as u64);
            let mut decoder =
                AceIndexedDecoder::open(Cursor::new(&encoded), ace_core::DecodeLimits::default())?;
            let (stats, (range, metrics)) =
                measure(|| Ok(decoder.read_range_with_metrics(start..end)?))?;
            assert_eq!(range, data[start as usize..end as usize]);

            let mut row = JsonObjectBuilder::new();
            row.field("workload_id", format!("range_{}", end - start))
                .field("path", "warm_range")
                .field("alignment", alignment)
                .field("logical_bytes_requested", end - start)
                .field("bytes_returned", range.len())
                .field("blocks_touched", metrics.blocks_touched)
                .field("physical_bytes_read", metrics.physical_bytes_read)
                .field(
                    "physical_to_logical_ratio",
                    metrics.physical_to_logical_ratio(),
                )
                .value("timing", timing_json(&stats, range.len()));
            rows.push(row.build());
        }
    }
    Ok(rows)
}

/// Captures the current random-access corpus plan distribution for direct diff against the 0.4-buildfix2 baseline.
///
/// Buildfix2 is bundled as JSON under `examples/baselines/0.4-buildfix2`; the comparison tool can
/// compare this family with the archived distribution without contaminating the timed decoder path.
pub(crate) fn random_access_plan_diff_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let data = mixed_data(16);
    let cfg = AceConfig {
        profile: CompressionProfile::Balanced,
        threads: 1,
        access_hint: AccessHint::RandomAccess,
        ..AceConfig::default()
    };
    let engine = AceEngine::new(cfg)?;
    let (encoded, stats) = engine.compress_with_stats(&data)?;
    let mut decoder =
        AceIndexedDecoder::open(Cursor::new(encoded), ace_core::DecodeLimits::default())?;

    let ranges = [
        ("range-4k", 4 * 1024usize),
        ("range-16k", 16 * 1024usize),
        ("range-64k", 64 * 1024usize),
    ];
    let mut rows = Vec::new();
    for (workload_id, len) in ranges {
        let (timing, bytes) = measure(|| Ok(decoder.read_range(0_u64..len as u64)?))?;
        assert_eq!(bytes, data[..len]);
        let mut row = JsonObjectBuilder::new();
        row.field("workload_id", workload_id)
            .field("path", "random-access-plan-diff")
            .field("logical_bytes_requested", len)
            .field("plan_distribution", &stats.plan_distribution)
            .field("baseline_version", "0.4-buildfix2")
            .value("timing", timing_json(&timing, len));
        rows.push(row.build());
    }
    Ok(rows)
}
