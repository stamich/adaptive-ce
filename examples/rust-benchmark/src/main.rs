use std::collections::BTreeMap;
use std::fs;
use std::hint::black_box;
use std::io::{Cursor, Read, Write};
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use ace_analysis::{AnalysisLevel, BlockAnalyzer, DefaultBlockAnalyzer};
use ace_core::{
    AceConfig, CandidateTier, CodecId, CompressionProfile, DecodingPlan, EntropyCodecId, LzMode,
    PhysicalCompressionPlan, TransformId,
};
use ace_cost::{CandidateEstimator, DefaultCandidateEstimator};
use ace_engine::{AceEngine, AceIndexedDecoder};
use ace_entropy::{huffman_encode, rans4x_encode, rans_encode};
use ace_planner::{
    encode_plan_payload, evaluate_candidates_v3, CompressionPlanner, DefaultCompressionPlanner,
};
use ace_stream::{compress_reader_known_size, StreamLimits};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use serde::Serialize;
use serde_json::{Map, Value};

const RUNS: usize = 7;
const WARMUPS: usize = 3;

/// Incremental JSON object builder used to keep benchmark serialization compile-friendly.
///
/// Large `serde_json::json!` object literals expand recursively at compile time. Benchmark
/// documents evolve frequently and can therefore exceed Rust's macro recursion limit even
/// though the runtime data itself is not recursive. This builder keeps the public JSON schema
/// unchanged while constructing large objects from small, independently reviewable sections.
#[derive(Debug, Default)]
struct JsonObjectBuilder {
    fields: Map<String, Value>,
}

impl JsonObjectBuilder {
    /// Creates an empty JSON object builder.
    fn new() -> Self {
        Self::default()
    }

    /// Inserts any serializable value under `key`.
    fn field<T: Serialize>(&mut self, key: &str, value: T) -> &mut Self {
        let value = serde_json::to_value(value).expect("benchmark JSON field must be serializable");
        self.fields.insert(key.to_string(), value);
        self
    }

    /// Inserts a value that is already represented as `serde_json::Value`.
    fn value(&mut self, key: &str, value: Value) -> &mut Self {
        self.fields.insert(key.to_string(), value);
        self
    }

    /// Merges another section into this object without adding a schema nesting level.
    fn extend(&mut self, section: JsonObjectBuilder) -> &mut Self {
        self.fields.extend(section.fields);
        self
    }

    /// Finalizes the builder as a JSON object.
    fn build(self) -> Value {
        Value::Object(self.fields)
    }
}

/// Top-level JSON document shared by every official ACE 0.3 benchmark family.
#[derive(Debug, Serialize)]
struct BenchmarkDocument {
    schema_version: &'static str,
    project: &'static str,
    milestone: &'static str,
    base: &'static str,
    scope: String,
    benchmark_contract_origin: &'static str,
    generated_at_utc_epoch_seconds: u64,
    environment: Value,
    configuration: Value,
    workloads: Vec<Value>,
}

/// Repeated timing samples and standard robust summary statistics.
#[derive(Debug, Clone)]
struct SampleStats {
    samples_ns: Vec<u64>,
    median_ns: f64,
    p95_ns: f64,
    p99_ns: f64,
    mean_ns: f64,
    min_ns: u64,
    max_ns: u64,
}

/// Aggregates analytical estimator error for one deterministic benchmark bucket.
#[derive(Debug, Default, Clone)]
struct EstimatorErrorStats {
    /// Number of candidate observations in the bucket.
    count: u64,
    /// Sum of absolute prediction errors in bytes.
    absolute_error_sum: u128,
    /// Sum of signed prediction errors in bytes (`predicted - actual`).
    signed_error_sum: i128,
    /// Sum of absolute percentage errors.
    absolute_percentage_error_sum: f64,
    /// Individual absolute errors used to report deterministic p95.
    absolute_errors: Vec<u64>,
}

impl EstimatorErrorStats {
    /// Records one prediction and its diagnostic full-encode size.
    fn observe(&mut self, predicted: u64, actual: u64) {
        let signed = predicted as i128 - actual as i128;
        let absolute = if signed < 0 {
            (-signed) as u128
        } else {
            signed as u128
        };
        let absolute = absolute.min(u64::MAX as u128) as u64;
        self.count = self.count.saturating_add(1);
        self.absolute_error_sum = self.absolute_error_sum.saturating_add(absolute as u128);
        self.signed_error_sum = self.signed_error_sum.saturating_add(signed);
        if actual != 0 {
            self.absolute_percentage_error_sum += absolute as f64 / actual as f64;
        }
        self.absolute_errors.push(absolute);
    }

    /// Serializes MAE, MAPE, signed bias and p95 absolute error for JSON output.
    fn json(&self) -> Value {
        if self.count == 0 {
            let mut row = JsonObjectBuilder::new();
            row.field("count", 0u64)
                .field("mae_bytes", 0.0f64)
                .field("mape", 0.0f64)
                .field("bias_bytes", 0.0f64)
                .field("p95_absolute_error_bytes", 0u64);
            return row.build();
        }
        let mut errors = self.absolute_errors.clone();
        errors.sort_unstable();
        let p95_index = ((errors.len() - 1) * 95) / 100;
        let mut row = JsonObjectBuilder::new();
        row.field("count", self.count)
            .field(
                "mae_bytes",
                self.absolute_error_sum as f64 / self.count as f64,
            )
            .field(
                "mape",
                self.absolute_percentage_error_sum / self.count as f64,
            )
            .field(
                "bias_bytes",
                self.signed_error_sum as f64 / self.count as f64,
            )
            .field("p95_absolute_error_bytes", errors[p95_index]);
        row.build()
    }
}

/// Executes one benchmark family or the full ACE 0.3 contract.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let family = std::env::args().nth(1).unwrap_or_else(|| "all".to_string());
    let families: Vec<&str> = if family == "all" {
        vec![
            "compression",
            "entropy",
            "planner",
            "parallel",
            "random-access",
            "streaming",
            "memory",
        ]
    } else {
        vec![family.as_str()]
    };

    for family in families {
        let workloads = match family {
            "compression" => compression_family()?,
            "entropy" => entropy_family()?,
            "planner" => planner_family()?,
            "parallel" => parallel_family()?,
            "random-access" => random_access_family()?,
            "streaming" => streaming_family()?,
            "memory" => memory_family()?,
            other => return Err(format!("unknown benchmark family: {other}").into()),
        };
        let document = BenchmarkDocument {
            schema_version: "1.9",
            project: "ace",
            milestone: "0.3-buildfix9-compilefix",
            base: "0.3-buildfix9",
            scope: family.to_string(),
            benchmark_contract_origin: "ace-0.3-buildfix9-compilefix",
            generated_at_utc_epoch_seconds: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            environment: environment_json(),
            configuration: benchmark_configuration_json(),
            workloads,
        };
        let path = result_path(family);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, serde_json::to_vec_pretty(&document)?)?;
        println!("results written to {}", path.display());
    }
    Ok(())
}

/// Returns machine/build metadata required for reproducible cross-milestone comparisons.
fn environment_json() -> Value {
    let cpu_model = fs::read_to_string("/proc/cpuinfo").ok().and_then(|text| {
        text.lines()
            .find_map(|line| line.strip_prefix("model name\t: ").map(str::to_string))
    });
    let memory_bytes = fs::read_to_string("/proc/meminfo").ok().and_then(|text| {
        text.lines().find_map(|line| {
            line.strip_prefix("MemTotal:")
                .and_then(|rest| rest.split_whitespace().next())
                .and_then(|kb| kb.parse::<u64>().ok())
                .map(|kb| kb * 1024)
        })
    });

    let mut row = JsonObjectBuilder::new();
    row.field("os", std::env::consts::OS)
        .field("arch", std::env::consts::ARCH)
        .field("cpu_model", cpu_model)
        .field("physical_cores", num_cpus::get_physical())
        .field("logical_cpus", num_cpus::get())
        .field("memory_bytes", memory_bytes)
        .field(
            "build_profile",
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            },
        )
        .field(
            "target_features",
            option_env!("CARGO_CFG_TARGET_FEATURE").unwrap_or("unknown"),
        );
    row.build()
}

/// Returns static benchmark configuration metadata shared by every benchmark family.
fn benchmark_configuration_json() -> Value {
    let mut row = JsonObjectBuilder::new();
    row.field("warmup_iterations", WARMUPS)
        .field("runs", RUNS)
        .field("default_block_size_bytes", 262_144usize)
        .field("format_version", "1.2")
        .field(
            "simd_backend",
            format!("{:?}", ace_simd::selected_backend()),
        );
    row.build()
}

/// Resolves the canonical `examples/results/0.3-<family>.json` path.
fn result_path(family: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("benchmark crate lives under examples")
        .join("results")
        .join(format!("0.3-buildfix9-compilefix-{family}.json"))
}

/// Runs warmups and seven measured invocations while retaining the final operation result.
fn measure<F, T>(mut operation: F) -> Result<(SampleStats, T), Box<dyn std::error::Error>>
where
    F: FnMut() -> Result<T, Box<dyn std::error::Error>>,
{
    for _ in 0..WARMUPS {
        black_box(operation()?);
    }
    let mut samples = Vec::with_capacity(RUNS);
    let mut last = None;
    for _ in 0..RUNS {
        let start = Instant::now();
        let value = operation()?;
        samples.push(start.elapsed().as_nanos().min(u64::MAX as u128) as u64);
        last = Some(value);
    }
    Ok((summarize(samples), last.expect("RUNS is non-zero")))
}

/// Calculates deterministic percentile summaries from nanosecond samples.
fn summarize(mut samples: Vec<u64>) -> SampleStats {
    let raw = samples.clone();
    samples.sort_unstable();
    let percentile = |p: f64| -> f64 {
        let idx = (((samples.len() - 1) as f64) * p).ceil() as usize;
        samples[idx.min(samples.len() - 1)] as f64
    };
    let mean = raw.iter().copied().map(|v| v as f64).sum::<f64>() / raw.len() as f64;
    SampleStats {
        samples_ns: raw,
        median_ns: percentile(0.50),
        p95_ns: percentile(0.95),
        p99_ns: percentile(0.99),
        mean_ns: mean,
        min_ns: *samples.first().unwrap_or(&0),
        max_ns: *samples.last().unwrap_or(&0),
    }
}

/// Converts samples to the common ACE timing and throughput representation.
fn timing_json(stats: &SampleStats, bytes: usize) -> Value {
    let seconds = stats.median_ns / 1_000_000_000.0;
    let mb_s = if seconds == 0.0 {
        0.0
    } else {
        bytes as f64 / 1_048_576.0 / seconds
    };
    let mut row = JsonObjectBuilder::new();
    row.field("runs", RUNS)
        .field("warmup_iterations", WARMUPS)
        .field("median_ns", stats.median_ns)
        .field("p95_ns", stats.p95_ns)
        .field("p99_ns", stats.p99_ns)
        .field("mean_ns", stats.mean_ns)
        .field("min_ns", stats.min_ns)
        .field("max_ns", stats.max_ns)
        .field("median_mb_s", mb_s)
        .field("samples_ns", &stats.samples_ns);
    row.build()
}

/// Builds one compression comparison row without a monolithic JSON macro.
fn compression_comparison_row(
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

/// Generates the deterministic heterogeneous corpus used across benchmark families.
fn mixed_data(mebibytes: usize) -> Vec<u8> {
    let target = mebibytes * 1024 * 1024;
    let quarter = target / 4;
    let mut data = Vec::with_capacity(target);
    data.extend(std::iter::repeat(0u8).take(quarter));
    while data.len() < quarter * 2 {
        let v = (data.len() as u32 / 16).to_le_bytes();
        data.extend_from_slice(&v);
    }
    while data.len() < quarter * 3 {
        data.extend_from_slice(
            b"{\"status\":\"ACTIVE\",\"service\":\"graphnet\",\"region\":\"eu\"}\n",
        );
    }
    let mut x = 0x9e37_79b9u32;
    while data.len() < target {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        data.push((x & 0xff) as u8);
    }
    data.truncate(target);
    data
}

/// Returns a stable coarse data-class label for one quarter of the synthetic mixed corpus.
fn data_class(block_index: usize, block_count: usize) -> &'static str {
    let quarter = (block_count / 4).max(1);
    match block_index / quarter {
        0 => "zeros",
        1 => "numeric",
        2 => "structured",
        _ => "random",
    }
}

/// Benchmarks end-to-end ACE profiles against LZ4, Zstd level 3 and gzip level 6.
fn compression_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let data = mixed_data(16);
    let mut results = Vec::new();
    for (name, profile) in [
        ("ace-fast", CompressionProfile::Fast),
        ("ace-balanced", CompressionProfile::Balanced),
        ("ace-dense", CompressionProfile::Dense),
    ] {
        let mut cfg = AceConfig::default();
        cfg.profile = profile;
        cfg.threads = 1;
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
fn entropy_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
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

/// Measures Planner V3.6 quality, work budgets and isolated timing components.
fn planner_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let data = mixed_data(8);
    let cfg = AceConfig::default();
    let analyzer = DefaultBlockAnalyzer;
    let planner = DefaultCompressionPlanner;
    let blocks = data.chunks(cfg.block_size).collect::<Vec<_>>();

    let mut regret = 0i64;
    let mut generated_recall = 0u64;
    let mut top_k_recall = 0u64;
    let mut top_k_eligible = 0u64;
    let mut sampled_recall = 0u64;
    let mut sampled_eligible = 0u64;
    let mut final_selection_recall = 0u64;
    let mut quality_pool_recall = 0u64;
    let mut quality_pool_eligible = 0u64;
    let mut oracle_top1_after = 0u64;
    let mut oracle_top2_after = 0u64;
    let mut oracle_top3_after = 0u64;
    let mut oracle_rank_analytical_sum = 0u64;
    let mut oracle_rank_stage1_sum = 0u64;
    let mut oracle_rank_after_sum = 0u64;
    let mut oracle_rank_final_sum = 0u64;
    let mut oracle_rank_eligible = 0u64;
    let mut quality_candidates_total = 0u64;
    let mut selected_size_rank_total = 0u64;
    let mut selected_cost_rank_total = 0u64;
    let mut predicted_size_regret_total = 0u64;
    let mut fast_paths = 0u64;
    let mut estimated_total = 0u64;
    let mut sampled_total = 0u64;
    let mut second_stage_total = 0u64;
    let mut full_trial_total = 0u64;
    let mut hybrid_lz_candidates_total = 0u64;
    let mut hybrid_lz_stage1_total = 0u64;
    let mut hybrid_lz_stage2_total = 0u64;
    let mut hybrid_lz_skipped_total = 0u64;
    let mut hybrid_lz_high_confidence_skips_total = 0u64;
    let mut hybrid_lz_sample_bytes_total = 0u64;
    let mut hybrid_lz_max_disagreement_ppm = 0u64;
    let mut regret_samples = Vec::<u64>::new();
    let mut generated_recall_by_class: BTreeMap<&str, (u64, u64)> = BTreeMap::new();
    let mut regret_by_class: BTreeMap<&str, (i64, u64)> = BTreeMap::new();
    let mut estimator_by_family: BTreeMap<String, EstimatorErrorStats> = BTreeMap::new();
    let mut estimator_by_class: BTreeMap<String, EstimatorErrorStats> = BTreeMap::new();
    let estimator = DefaultCandidateEstimator;
    let mut details = Vec::new();

    for (idx, block) in blocks.iter().enumerate() {
        let profile = analyzer.analyze(block);
        let candidates = planner.candidates(&profile, &cfg);
        let class = data_class(idx, blocks.len());

        // Diagnostic-only full encodes calibrate the cheap buildfix6 analytical model. They are
        // outside the timed planner hot path and do not count as planner full-trial encodes.
        for candidate in &candidates {
            let estimate = estimator.estimate(candidate, &profile, cfg.profile);
            let actual = encoded_plan_size(block, candidate)? as u64;
            let family = estimator_family_id(candidate);
            estimator_by_family
                .entry(family.clone())
                .or_default()
                .observe(estimate.analytical_size_bytes, actual);
            estimator_by_class
                .entry(format!("{class}:{family}"))
                .or_default()
                .observe(estimate.analytical_size_bytes, actual);
        }

        let decision = evaluate_candidates_v3(block, &profile, &candidates, &cfg)?;
        fast_paths += if decision.telemetry.fast_path_hit {
            1
        } else {
            0
        };
        estimated_total += decision.telemetry.estimated_candidates as u64;
        sampled_total += decision.telemetry.sampled_candidates as u64;
        second_stage_total += decision.telemetry.second_stage_candidates as u64;
        full_trial_total += decision.telemetry.full_trial_encodes as u64;
        hybrid_lz_candidates_total += decision.telemetry.hybrid_lz_candidates as u64;
        hybrid_lz_stage1_total += decision.telemetry.hybrid_lz_stage1_candidates as u64;
        hybrid_lz_stage2_total += decision.telemetry.hybrid_lz_stage2_candidates as u64;
        hybrid_lz_skipped_total += decision.telemetry.hybrid_lz_skipped_candidates as u64;
        hybrid_lz_high_confidence_skips_total +=
            decision.telemetry.hybrid_lz_high_confidence_skips as u64;
        hybrid_lz_sample_bytes_total += decision.telemetry.hybrid_lz_sample_bytes as u64;
        hybrid_lz_max_disagreement_ppm =
            hybrid_lz_max_disagreement_ppm.max(decision.telemetry.hybrid_lz_max_disagreement_ppm);
        quality_candidates_total += decision.telemetry.quality_qualified_candidates as u64;
        selected_size_rank_total += decision.telemetry.selected_size_rank as u64;
        selected_cost_rank_total += decision.telemetry.selected_cost_rank as u64;
        predicted_size_regret_total += decision
            .telemetry
            .selected_blended_size_bytes
            .saturating_sub(decision.telemetry.best_blended_size_bytes);

        let oracle = oracle_plans()
            .into_iter()
            .map(|plan| {
                let size = encoded_plan_size(block, &plan).unwrap_or(usize::MAX);
                (size, plan)
            })
            .min_by_key(|(size, _)| *size)
            .expect("oracle plans are non-empty");

        let generated_hit = candidates.iter().any(|p| same_plan(p, &oracle.1));
        let top_k_hit = if decision.telemetry.fast_path_hit {
            same_plan(&decision.plan, &oracle.1)
        } else {
            decision.top_k_plans.iter().any(|p| same_plan(p, &oracle.1))
        };
        let sampled_hit = if decision.telemetry.fast_path_hit {
            same_plan(&decision.plan, &oracle.1)
        } else {
            decision
                .final_ranked_plans
                .iter()
                .any(|p| same_plan(p, &oracle.1))
        };
        let final_hit = same_plan(&decision.plan, &oracle.1);
        let quality_pool_hit = if decision.telemetry.fast_path_hit {
            final_hit
        } else {
            decision
                .quality_qualified_plans
                .iter()
                .any(|p| same_plan(p, &oracle.1))
        };
        let oracle_rank_analytical = if decision.telemetry.fast_path_hit {
            if final_hit {
                Some(1usize)
            } else {
                None
            }
        } else {
            decision
                .analytical_ranked_plans
                .iter()
                .position(|p| same_plan(p, &oracle.1))
                .map(|i| i + 1)
        };
        let oracle_rank_stage1 = if decision.telemetry.fast_path_hit {
            if final_hit {
                Some(1usize)
            } else {
                None
            }
        } else {
            decision
                .stage_one_ranked_plans
                .iter()
                .position(|p| same_plan(p, &oracle.1))
                .map(|i| i + 1)
        };
        let oracle_rank_after = if decision.telemetry.fast_path_hit {
            if final_hit {
                Some(1usize)
            } else {
                None
            }
        } else {
            decision
                .final_ranked_plans
                .iter()
                .position(|p| same_plan(p, &oracle.1))
                .map(|i| i + 1)
        };
        let oracle_rank_final = if decision.telemetry.fast_path_hit {
            if final_hit {
                Some(1usize)
            } else {
                None
            }
        } else {
            decision
                .quality_qualified_plans
                .iter()
                .position(|p| same_plan(p, &oracle.1))
                .map(|i| i + 1)
        };
        if let (Some(analytical), Some(stage1), Some(after)) = (
            oracle_rank_analytical,
            oracle_rank_stage1,
            oracle_rank_after,
        ) {
            oracle_rank_eligible += 1;
            oracle_rank_analytical_sum += analytical as u64;
            oracle_rank_stage1_sum += stage1 as u64;
            oracle_rank_after_sum += after as u64;
            oracle_rank_final_sum += oracle_rank_final.unwrap_or(0) as u64;
            oracle_top1_after += (after <= 1) as u64;
            oracle_top2_after += (after <= 2) as u64;
            oracle_top3_after += (after <= 3) as u64;
        }

        generated_recall += generated_hit as u64;
        if !decision.telemetry.fast_path_hit {
            top_k_eligible += 1;
            sampled_eligible += 1;
            top_k_recall += top_k_hit as u64;
            sampled_recall += sampled_hit as u64;
        }
        final_selection_recall += final_hit as u64;
        if !decision.telemetry.fast_path_hit {
            quality_pool_eligible += 1;
            quality_pool_recall += quality_pool_hit as u64;
        }

        let selected = decision.plan;
        let selected_size = encoded_plan_size(block, &selected)? as i64;
        let block_regret = selected_size - oracle.0 as i64;
        regret += block_regret;
        regret_samples.push(block_regret.max(0) as u64);
        let recall_entry = generated_recall_by_class.entry(class).or_insert((0, 0));
        recall_entry.1 += 1;
        if generated_hit {
            recall_entry.0 += 1;
        }
        let regret_entry = regret_by_class.entry(class).or_insert((0, 0));
        regret_entry.0 += block_regret;
        regret_entry.1 += 1;

        // Build block diagnostics in small compile-time-independent sections. The emitted
        // JSON remains flat and schema-compatible with benchmark contract 1.9.
        let mut identity = JsonObjectBuilder::new();
        identity
            .field("block_id", idx)
            .field("data_class", class)
            .field("candidate_count", candidates.len())
            .field("generated_oracle", generated_hit)
            .field("top_k_applied", !decision.telemetry.fast_path_hit)
            .field("top_k_oracle", top_k_hit)
            .field("sampled_oracle", sampled_hit);

        let mut ranking = JsonObjectBuilder::new();
        ranking
            .field("oracle_rank_analytical", oracle_rank_analytical)
            .field("oracle_rank_after_stage1", oracle_rank_stage1)
            .field("oracle_rank_after_sampling", oracle_rank_after)
            .field("oracle_rank_final_quality_pool", oracle_rank_final)
            .field("oracle_in_quality_pool", quality_pool_hit)
            .field("selected_is_oracle", final_hit)
            .field("selected_size_rank", decision.telemetry.selected_size_rank)
            .field("selected_cost_rank", decision.telemetry.selected_cost_rank);

        let mut quality = JsonObjectBuilder::new();
        quality
            .field("top_k_count", decision.telemetry.sampled_candidates)
            .field(
                "second_stage_count",
                decision.telemetry.second_stage_candidates,
            )
            .field(
                "quality_qualified_count",
                decision.telemetry.quality_qualified_candidates,
            )
            .field(
                "best_blended_size_bytes",
                decision.telemetry.best_blended_size_bytes,
            )
            .field(
                "quality_limit_bytes",
                decision.telemetry.quality_limit_bytes,
            )
            .field(
                "selected_blended_size_bytes",
                decision.telemetry.selected_blended_size_bytes,
            )
            .field(
                "predicted_size_regret_bytes",
                decision
                    .telemetry
                    .selected_blended_size_bytes
                    .saturating_sub(decision.telemetry.best_blended_size_bytes),
            );

        let mut hybrid = JsonObjectBuilder::new();
        hybrid
            .field(
                "hybrid_lz_candidates",
                decision.telemetry.hybrid_lz_candidates,
            )
            .field(
                "hybrid_lz_stage1_candidates",
                decision.telemetry.hybrid_lz_stage1_candidates,
            )
            .field(
                "hybrid_lz_stage2_candidates",
                decision.telemetry.hybrid_lz_stage2_candidates,
            )
            .field(
                "hybrid_lz_skipped_candidates",
                decision.telemetry.hybrid_lz_skipped_candidates,
            )
            .field(
                "hybrid_lz_high_confidence_skips",
                decision.telemetry.hybrid_lz_high_confidence_skips,
            )
            .field(
                "hybrid_lz_sample_bytes",
                decision.telemetry.hybrid_lz_sample_bytes,
            )
            .field(
                "hybrid_lz_max_disagreement_ppm",
                decision.telemetry.hybrid_lz_max_disagreement_ppm,
            );

        let prediction_error_percent = if selected_size == 0 {
            0.0
        } else {
            (decision.telemetry.selected_blended_size_bytes as f64 - selected_size as f64)
                / selected_size as f64
        };
        let mut outcome = JsonObjectBuilder::new();
        outcome
            .field(
                "selected_prediction_error_bytes",
                decision.telemetry.selected_blended_size_bytes as i64 - selected_size,
            )
            .field(
                "selected_prediction_error_percent",
                prediction_error_percent,
            )
            .field("selected_plan", plan_id(&selected))
            .field("oracle_plan", plan_id(&oracle.1))
            .field("selected_tier", tier_name(selected.tier))
            .field("selected_bytes", selected_size)
            .field("oracle_bytes", oracle.0)
            .field("regret_bytes", block_regret);

        let mut block_detail = JsonObjectBuilder::new();
        block_detail
            .extend(identity)
            .extend(ranking)
            .extend(quality)
            .extend(hybrid)
            .extend(outcome);
        details.push(block_detail.build());
    }

    let analysis_stats = measure(|| {
        for block in &blocks {
            black_box(analyzer.analyze(block));
        }
        Ok(())
    })?
    .0;
    let fast_analysis_stats = measure(|| {
        for block in &blocks {
            black_box(analyzer.analyze_with_level(block, AnalysisLevel::Fast));
        }
        Ok(())
    })?
    .0;
    let candidate_stats = measure(|| {
        for block in &blocks {
            let p = analyzer.analyze(block);
            black_box(planner.candidates(&p, &cfg));
        }
        Ok(())
    })?
    .0;
    let evaluation_stats = measure(|| {
        for block in &blocks {
            let p = analyzer.analyze(block);
            let c = planner.candidates(&p, &cfg);
            black_box(evaluate_candidates_v3(block, &p, &c, &cfg)?);
        }
        Ok(())
    })?
    .0;

    let by_class = generated_recall_by_class
        .into_iter()
        .map(|(k, (hits, total))| {
            (
                k,
                if total == 0 {
                    0.0
                } else {
                    hits as f64 / total as f64
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let regret_classes = regret_by_class
        .into_iter()
        .map(|(k, (bytes, total))| {
            (
                k,
                if total == 0 {
                    0.0
                } else {
                    bytes as f64 / total as f64
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let block_count = blocks.len().max(1) as f64;
    let estimator_calibration = estimator_by_family
        .into_iter()
        .map(|(family, stats)| (family, stats.json()))
        .collect::<BTreeMap<_, _>>();
    let estimator_calibration_by_class = estimator_by_class
        .into_iter()
        .map(|(bucket, stats)| (bucket, stats.json()))
        .collect::<BTreeMap<_, _>>();
    regret_samples.sort_unstable();
    let p95_regret_bytes_per_block = if regret_samples.is_empty() {
        0u64
    } else {
        regret_samples[((regret_samples.len() - 1) * 95) / 100]
    };

    let top_k_rate = if top_k_eligible == 0 {
        1.0
    } else {
        top_k_recall as f64 / top_k_eligible as f64
    };
    let sample_rate = if sampled_eligible == 0 {
        1.0
    } else {
        sampled_recall as f64 / sampled_eligible as f64
    };
    let quality_pool_rate = if quality_pool_eligible == 0 {
        1.0
    } else {
        quality_pool_recall as f64 / quality_pool_eligible as f64
    };
    let rank_denominator = oracle_rank_eligible.max(1) as f64;

    let mut identity = JsonObjectBuilder::new();
    identity
        .field("workload_id", "mixed_8m")
        .field("path", "planner_v3_6_hardened_adaptive_budget")
        .field("blocks", blocks.len());

    let mut quality = JsonObjectBuilder::new();
    quality
        .field("oracle_regret_bytes", regret)
        .field(
            "normalized_regret_bytes_per_block",
            regret as f64 / block_count,
        )
        .field("p95_regret_bytes_per_block", p95_regret_bytes_per_block)
        .field("regret_bytes_per_block_by_class", &regret_classes)
        .field("candidate_recall", generated_recall as f64 / block_count)
        .field(
            "candidate_generation_recall",
            generated_recall as f64 / block_count,
        )
        .field("top_k_recall", top_k_rate)
        .field("top_k_recall_denominator_blocks", top_k_eligible)
        .field("sample_verifier_recall", sample_rate)
        .field(
            "sample_verifier_recall_denominator_blocks",
            sampled_eligible,
        )
        .field("quality_pool_recall", quality_pool_rate)
        .field(
            "quality_pool_recall_denominator_blocks",
            quality_pool_eligible,
        )
        .field(
            "final_selection_recall",
            final_selection_recall as f64 / block_count,
        )
        .field("candidate_recall_by_class", &by_class);

    let mut ranking = JsonObjectBuilder::new();
    ranking
        .field(
            "oracle_mean_rank_analytical",
            if oracle_rank_eligible == 0 {
                0.0
            } else {
                oracle_rank_analytical_sum as f64 / rank_denominator
            },
        )
        .field(
            "oracle_mean_rank_after_stage1",
            if oracle_rank_eligible == 0 {
                0.0
            } else {
                oracle_rank_stage1_sum as f64 / rank_denominator
            },
        )
        .field(
            "oracle_mean_rank_after_sampling",
            if oracle_rank_eligible == 0 {
                0.0
            } else {
                oracle_rank_after_sum as f64 / rank_denominator
            },
        )
        .field(
            "oracle_mean_rank_final_quality_pool",
            if oracle_rank_eligible == 0 {
                0.0
            } else {
                oracle_rank_final_sum as f64 / rank_denominator
            },
        )
        .field(
            "oracle_top1_rate_after_sampling",
            if oracle_rank_eligible == 0 {
                1.0
            } else {
                oracle_top1_after as f64 / rank_denominator
            },
        )
        .field(
            "oracle_top2_rate_after_sampling",
            if oracle_rank_eligible == 0 {
                1.0
            } else {
                oracle_top2_after as f64 / rank_denominator
            },
        )
        .field(
            "oracle_top3_rate_after_sampling",
            if oracle_rank_eligible == 0 {
                1.0
            } else {
                oracle_top3_after as f64 / rank_denominator
            },
        )
        .field(
            "quality_qualified_candidates_per_block",
            quality_candidates_total as f64 / block_count,
        )
        .field(
            "selected_size_rank_mean",
            selected_size_rank_total as f64 / block_count,
        )
        .field(
            "selected_cost_rank_mean",
            selected_cost_rank_total as f64 / block_count,
        )
        .field(
            "predicted_size_regret_bytes_per_block",
            predicted_size_regret_total as f64 / block_count,
        );

    let mut planner_work = JsonObjectBuilder::new();
    planner_work
        .field("fast_path_rate", fast_paths as f64 / block_count)
        .field(
            "estimated_candidates_per_block",
            estimated_total as f64 / block_count,
        )
        .field(
            "sampled_candidates_per_block",
            sampled_total as f64 / block_count,
        )
        .field(
            "second_stage_candidates_per_block",
            second_stage_total as f64 / block_count,
        )
        .field(
            "full_trial_encodes_per_block",
            full_trial_total as f64 / block_count,
        );

    let mut hybrid = JsonObjectBuilder::new();
    hybrid
        .field(
            "hybrid_lz_candidates_per_block",
            hybrid_lz_candidates_total as f64 / block_count,
        )
        .field(
            "hybrid_lz_stage1_candidates_per_block",
            hybrid_lz_stage1_total as f64 / block_count,
        )
        .field(
            "hybrid_lz_stage2_candidates_per_block",
            hybrid_lz_stage2_total as f64 / block_count,
        )
        .field(
            "hybrid_lz_skipped_candidates_per_block",
            hybrid_lz_skipped_total as f64 / block_count,
        )
        .field(
            "hybrid_lz_high_confidence_skips_per_block",
            hybrid_lz_high_confidence_skips_total as f64 / block_count,
        )
        .field(
            "hybrid_lz_sample_bytes_per_block",
            hybrid_lz_sample_bytes_total as f64 / block_count,
        )
        .field(
            "hybrid_lz_sample_fraction",
            hybrid_lz_sample_bytes_total as f64 / data.len().max(1) as f64,
        )
        .field(
            "hybrid_lz_max_disagreement_ppm",
            hybrid_lz_max_disagreement_ppm,
        );

    let mut calibration = JsonObjectBuilder::new();
    calibration
        .field("estimator_calibration", &estimator_calibration)
        .field(
            "estimator_calibration_by_data_class",
            &estimator_calibration_by_class,
        );

    let mut timing = JsonObjectBuilder::new();
    timing
        .value(
            "analysis_per_file",
            timing_json(&analysis_stats, data.len()),
        )
        .value(
            "fast_analysis_per_file",
            timing_json(&fast_analysis_stats, data.len()),
        )
        .value(
            "candidate_generation_per_file",
            timing_json(&candidate_stats, data.len()),
        )
        .value(
            "evaluation_per_file",
            timing_json(&evaluation_stats, data.len()),
        );

    let mut row = JsonObjectBuilder::new();
    row.extend(identity)
        .extend(quality)
        .extend(ranking)
        .extend(planner_work)
        .extend(hybrid)
        .extend(calibration)
        .value("timing", timing.build())
        .field("block_details", details);

    Ok(vec![row.build()])
}

/// Benchmarks worker-count scaling and verifies bit-identical output across worker counts.
fn parallel_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
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
        let mut cfg = AceConfig::default();
        cfg.threads = threads;
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

/// Benchmarks full reconstruction, codec-diverse single blocks and crossing logical ranges.
fn random_access_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
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

/// Benchmarks bounded-memory reader-to-writer compression for representative stream sizes.
fn streaming_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    for mib in [16usize, 64usize] {
        let data = mixed_data(mib);
        let cfg = AceConfig::default();
        let limits = StreamLimits::default();
        let (stats, (encoded, stream_stats)) = measure(|| {
            let mut out = Vec::new();
            let telemetry = compress_reader_known_size(
                Cursor::new(&data),
                &mut out,
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

/// Reports reusable scratch capacity and planner trial-encode elimination introduced in ACE 0.3.
fn memory_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let data = mixed_data(16);
    let mut cfg = AceConfig::default();
    cfg.threads = 1;
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
    Ok(vec![row.build()])
}

/// Returns serialized bytes produced by one internal physical plan.
fn encoded_plan_size(
    input: &[u8],
    plan: &PhysicalCompressionPlan,
) -> Result<usize, Box<dyn std::error::Error>> {
    let (metadata, payload, _) = encode_plan_payload(input, plan)?;
    let prefix = if matches!(plan.decoding.entropy, EntropyCodecId::None) {
        0
    } else {
        4
    };
    Ok(metadata.len() + payload.len() + prefix)
}

/// Returns true when two candidates have identical decoder semantics and LZ search policy.
fn same_plan(a: &PhysicalCompressionPlan, b: &PhysicalCompressionPlan) -> bool {
    a.decoding == b.decoding && a.lz_mode == b.lz_mode
}

/// Returns a stable estimator-calibration bucket for one transform/codec family.
fn estimator_family_id(plan: &PhysicalCompressionPlan) -> String {
    let delta = plan
        .decoding
        .transforms
        .iter()
        .any(|t| matches!(t, TransformId::DeltaByte));
    let base = match (plan.decoding.codec, plan.lz_mode) {
        (CodecId::Raw, _) => "raw",
        (CodecId::Rle, _) => "rle",
        (CodecId::Lz, Some(LzMode::Fast)) => "lz_fast",
        (CodecId::Lz, Some(LzMode::Balanced)) => "lz_balanced",
        (CodecId::Lz, None) => "lz",
    };
    if delta {
        format!("delta_{base}")
    } else {
        base.to_string()
    }
}

/// Returns a stable textual physical-plan identifier for JSON diagnostics.
fn plan_id(p: &PhysicalCompressionPlan) -> String {
    let mut parts = p
        .decoding
        .transforms
        .iter()
        .map(|t| match t {
            TransformId::None => "none",
            TransformId::DeltaByte => "delta",
        })
        .collect::<Vec<_>>();
    parts.push(match (p.decoding.codec, p.lz_mode) {
        (CodecId::Raw, _) => "raw",
        (CodecId::Rle, _) => "rle",
        (CodecId::Lz, Some(LzMode::Balanced)) => "lz_balanced",
        (CodecId::Lz, _) => "lz_fast",
    });
    parts.push(match p.decoding.entropy {
        EntropyCodecId::None => "none",
        EntropyCodecId::Huffman => "huffman",
        EntropyCodecId::Rans => "rans",
        EntropyCodecId::Rans4x => "rans4x",
    });
    parts.join("+")
}

/// Returns the JSON label for one candidate tier.
fn tier_name(t: CandidateTier) -> &'static str {
    match t {
        CandidateTier::Mandatory => "mandatory",
        CandidateTier::Likely => "likely",
        CandidateTier::Exploratory => "exploratory",
    }
}

/// Builds the offline oracle search space; it is intentionally broader than runtime candidate pruning.
fn oracle_plans() -> Vec<PhysicalCompressionPlan> {
    let mut plans = Vec::new();
    let mut add = |transforms: Vec<TransformId>,
                   codec: CodecId,
                   entropy: EntropyCodecId,
                   lz: Option<LzMode>| {
        plans.push(PhysicalCompressionPlan {
            decoding: DecodingPlan {
                transforms,
                codec,
                dictionary: None,
                entropy,
            },
            lz_mode: lz,
            tier: CandidateTier::Exploratory,
            cost: Default::default(),
            score: 0,
            reason: "oracle",
        })
    };
    add(vec![], CodecId::Raw, EntropyCodecId::None, None);
    add(vec![], CodecId::Rle, EntropyCodecId::None, None);
    for e in [
        EntropyCodecId::Huffman,
        EntropyCodecId::Rans,
        EntropyCodecId::Rans4x,
    ] {
        add(vec![], CodecId::Raw, e, None);
        add(vec![], CodecId::Rle, e, None);
        add(vec![TransformId::DeltaByte], CodecId::Raw, e, None);
        for lz in [LzMode::Fast, LzMode::Balanced] {
            add(vec![], CodecId::Lz, e, Some(lz));
            add(vec![TransformId::DeltaByte], CodecId::Lz, e, Some(lz));
        }
    }
    plans
}
