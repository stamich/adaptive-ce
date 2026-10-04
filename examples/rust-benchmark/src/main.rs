use std::collections::BTreeMap;
use std::fs;
use std::hint::black_box;
use std::io::{Cursor, Read, Write};
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use ace_analysis::{BlockAnalyzer, DefaultBlockAnalyzer};
use ace_core::{
    AceConfig, CandidateTier, CodecId, CompressionProfile, DecodingPlan, EntropyCodecId, LzMode,
    PhysicalCompressionPlan, TransformId,
};
use ace_engine::{AceEngine, AceIndexedDecoder};
use ace_entropy::{huffman_encode, rans4x_encode, rans_encode};
use ace_planner::{
    encode_plan_payload, evaluate_candidates_v3, CompressionPlanner, DefaultCompressionPlanner,
};
use ace_stream::{compress_reader_known_size, StreamLimits};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use serde::Serialize;
use serde_json::{json, Value};

const RUNS: usize = 7;
const WARMUPS: usize = 3;

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
            schema_version: "1.2",
            project: "ace",
            milestone: "0.3-buildfix4",
            base: "0.3-buildfix3",
            scope: family.to_string(),
            benchmark_contract_origin: "ace-0.3-buildfix4",
            generated_at_utc_epoch_seconds: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            environment: environment_json(),
            configuration: json!({
                "warmup_iterations": WARMUPS,
                "runs": RUNS,
                "default_block_size_bytes": 262144,
                "format_version": "1.2",
                "simd_backend": format!("{:?}", ace_simd::selected_backend())
            }),
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
    json!({
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "cpu_model": cpu_model,
        "physical_cores": num_cpus::get_physical(),
        "logical_cpus": num_cpus::get(),
        "memory_bytes": memory_bytes,
        "build_profile": if cfg!(debug_assertions) { "debug" } else { "release" },
        "target_features": option_env!("CARGO_CFG_TARGET_FEATURE").unwrap_or("unknown")
    })
}

/// Resolves the canonical `examples/results/0.3-<family>.json` path.
fn result_path(family: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("benchmark crate lives under examples")
        .join("results")
        .join(format!("0.3-buildfix4-{family}.json"))
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
    json!({
        "runs": RUNS, "warmup_iterations": WARMUPS,
        "median_ns": stats.median_ns, "p95_ns": stats.p95_ns, "p99_ns": stats.p99_ns,
        "mean_ns": stats.mean_ns, "min_ns": stats.min_ns, "max_ns": stats.max_ns,
        "median_mb_s": mb_s, "samples_ns": stats.samples_ns,
    })
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
        results.push(json!({
            "workload_id":"mixed_16m", "path":name, "input_bytes":data.len(), "compressed_bytes":encoded.len(),
            "compression_ratio":data.len() as f64 / encoded.len().max(1) as f64,
            "compression":timing_json(&enc_stats,data.len()), "decompression":timing_json(&dec_stats,data.len()),
            "plan_distribution":telemetry.plan_distribution,
            "stage_timing_ns":{
                "analysis":telemetry.analysis_time.as_nanos(), "planning":telemetry.planning_time.as_nanos(),
                "encoding":telemetry.encoding_time.as_nanos(), "serialization":telemetry.serialization_time.as_nanos(),
                "index":telemetry.index_time.as_nanos()
            }
        }));
    }

    let (stats, lz4) = measure(|| Ok(lz4_flex::compress_prepend_size(&data)))?;
    let (dec, restored) = measure(|| Ok(lz4_flex::decompress_size_prepended(&lz4)?))?;
    assert_eq!(restored, data);
    results.push(json!({"workload_id":"mixed_16m","path":"lz4","input_bytes":data.len(),"compressed_bytes":lz4.len(),"compression_ratio":data.len() as f64/lz4.len() as f64,"compression":timing_json(&stats,data.len()),"decompression":timing_json(&dec,data.len())}));

    let (stats, zstd_data) = measure(|| Ok(zstd::stream::encode_all(Cursor::new(&data), 3)?))?;
    let (dec, restored) = measure(|| Ok(zstd::stream::decode_all(Cursor::new(&zstd_data))?))?;
    assert_eq!(restored, data);
    results.push(json!({"workload_id":"mixed_16m","path":"zstd-3","input_bytes":data.len(),"compressed_bytes":zstd_data.len(),"compression_ratio":data.len() as f64/zstd_data.len() as f64,"compression":timing_json(&stats,data.len()),"decompression":timing_json(&dec,data.len())}));

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
    results.push(json!({"workload_id":"mixed_16m","path":"gzip-6","input_bytes":data.len(),"compressed_bytes":gzip.len(),"compression_ratio":data.len() as f64/gzip.len() as f64,"compression":timing_json(&stats,data.len()),"decompression":timing_json(&gzip_dec,data.len())}));
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
            workloads.push(json!({"workload_id":id,"path":path,"input_bytes":data.len(),"metadata_bytes":metadata.len(),"payload_bytes":payload.len(),"encoded_bytes":metadata.len()+payload.len(),"compression_ratio":data.len() as f64/(metadata.len()+payload.len()).max(1) as f64,"encode":timing_json(&stats,data.len())}));
        }
    }
    Ok(workloads)
}

/// Measures Planner V3.1 quality at every pruning stage plus isolated timing components.
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
    let mut oracle_top1_after = 0u64;
    let mut oracle_top2_after = 0u64;
    let mut oracle_top3_after = 0u64;
    let mut oracle_rank_before_sum = 0u64;
    let mut oracle_rank_after_sum = 0u64;
    let mut oracle_rank_eligible = 0u64;
    let mut fast_paths = 0u64;
    let mut estimated_total = 0u64;
    let mut sampled_total = 0u64;
    let mut second_stage_total = 0u64;
    let mut full_trial_total = 0u64;
    let mut generated_recall_by_class: BTreeMap<&str, (u64, u64)> = BTreeMap::new();
    let mut regret_by_class: BTreeMap<&str, (i64, u64)> = BTreeMap::new();
    let mut details = Vec::new();

    for (idx, block) in blocks.iter().enumerate() {
        let profile = analyzer.analyze(block);
        let candidates = planner.candidates(&profile, &cfg);
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
        let oracle_rank_before = if decision.telemetry.fast_path_hit {
            if final_hit {
                Some(1usize)
            } else {
                None
            }
        } else {
            decision
                .top_k_plans
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
        if let (Some(before), Some(after)) = (oracle_rank_before, oracle_rank_after) {
            oracle_rank_eligible += 1;
            oracle_rank_before_sum += before as u64;
            oracle_rank_after_sum += after as u64;
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

        let selected = decision.plan;
        let selected_size = encoded_plan_size(block, &selected)? as i64;
        let block_regret = selected_size - oracle.0 as i64;
        regret += block_regret;
        let class = data_class(idx, blocks.len());

        let recall_entry = generated_recall_by_class.entry(class).or_insert((0, 0));
        recall_entry.1 += 1;
        if generated_hit {
            recall_entry.0 += 1;
        }
        let regret_entry = regret_by_class.entry(class).or_insert((0, 0));
        regret_entry.0 += block_regret;
        regret_entry.1 += 1;

        details.push(json!({
            "block_id":idx,
            "data_class":class,
            "candidate_count":candidates.len(),
            "generated_oracle":generated_hit,
            "top_k_applied":!decision.telemetry.fast_path_hit,
            "top_k_oracle":top_k_hit,
            "sampled_oracle":sampled_hit,
            "oracle_rank_before_sampling":oracle_rank_before,
            "oracle_rank_after_sampling":oracle_rank_after,
            "selected_is_oracle":final_hit,
            "top_k_count":decision.telemetry.sampled_candidates,
            "second_stage_count":decision.telemetry.second_stage_candidates,
            "selected_plan":plan_id(&selected),
            "oracle_plan":plan_id(&oracle.1),
            "selected_tier":tier_name(selected.tier),
            "selected_bytes":selected_size,
            "oracle_bytes":oracle.0,
            "regret_bytes":block_regret
        }));
    }

    let analysis_stats = measure(|| {
        for block in &blocks {
            black_box(analyzer.analyze(block));
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

    Ok(vec![json!({
        "workload_id":"mixed_8m",
        "path":"planner_v3_2_rank_only_confidence_blend",
        "blocks":blocks.len(),
        "oracle_regret_bytes":regret,
        "normalized_regret_bytes_per_block":regret as f64/block_count,
        "regret_bytes_per_block_by_class":regret_classes,
        "candidate_recall":generated_recall as f64/block_count,
        "candidate_generation_recall":generated_recall as f64/block_count,
        "top_k_recall":if top_k_eligible==0 {1.0} else {top_k_recall as f64/top_k_eligible as f64},
        "top_k_recall_denominator_blocks":top_k_eligible,
        "sample_verifier_recall":if sampled_eligible==0 {1.0} else {sampled_recall as f64/sampled_eligible as f64},
        "sample_survival_recall":if sampled_eligible==0 {1.0} else {sampled_recall as f64/sampled_eligible as f64},
        "sample_verifier_recall_denominator_blocks":sampled_eligible,
        "final_selection_recall":final_selection_recall as f64/block_count,
        "oracle_mean_rank_before_sampling":if oracle_rank_eligible==0 {0.0} else {oracle_rank_before_sum as f64/oracle_rank_eligible as f64},
        "oracle_mean_rank_after_sampling":if oracle_rank_eligible==0 {0.0} else {oracle_rank_after_sum as f64/oracle_rank_eligible as f64},
        "oracle_top1_rate_after_sampling":if oracle_rank_eligible==0 {1.0} else {oracle_top1_after as f64/oracle_rank_eligible as f64},
        "oracle_top2_rate_after_sampling":if oracle_rank_eligible==0 {1.0} else {oracle_top2_after as f64/oracle_rank_eligible as f64},
        "oracle_top3_rate_after_sampling":if oracle_rank_eligible==0 {1.0} else {oracle_top3_after as f64/oracle_rank_eligible as f64},
        "candidate_recall_by_class":by_class,
        "fast_path_rate":fast_paths as f64/block_count,
        "estimated_candidates_per_block":estimated_total as f64/block_count,
        "sampled_candidates_per_block":sampled_total as f64/block_count,
        "second_stage_candidates_per_block":second_stage_total as f64/block_count,
        "full_trial_encodes_per_block":full_trial_total as f64/block_count,
        "timing":{
            "analysis_per_file":timing_json(&analysis_stats,data.len()),
            "candidate_generation_per_file":timing_json(&candidate_stats,data.len()),
            "evaluation_per_file":timing_json(&evaluation_stats,data.len())
        },
        "block_details":details
    })])
}

/// Benchmarks worker-count scaling and verifies bit-identical output across worker counts.
fn parallel_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let data = mixed_data(16);
    let mut results = Vec::new();
    let mut reference: Option<Vec<u8>> = None;
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
        results.push(json!({"workload_id":"mixed_16m","path":format!("threads-{threads}"),"threads":threads,"encoded_bytes":encoded.len(),"deterministic_vs_1t":true,"compression":timing_json(&stats,data.len())}));
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
    let mut out = vec![
        json!({"workload_id":"mixed_16m","path":"full_decompress","bytes_returned":data.len(),"timing":timing_json(&full,data.len())}),
    ];

    // Opening/index validation is measured separately so random-access latency represents
    // an already-open archive, which is the storage-engine usage model.
    let (open_stats, opened) = measure(|| {
        Ok(AceIndexedDecoder::open(
            Cursor::new(&encoded),
            ace_core::DecodeLimits::default(),
        )?)
    })?;
    out.push(json!({
        "workload_id":"mixed_16m", "path":"decoder_open", "bytes_returned":0,
        "timing":timing_json(&open_stats, encoded.len())
    }));
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
        out.push(json!({"workload_id":"mixed_16m","path":"decode_block","block_id":block_id,"data_class":data_class(block_id as usize,64),"bytes_returned":block.len(),"physical_bytes_read":metrics.physical_bytes_read,"blocks_touched":metrics.blocks_touched,"physical_to_logical_ratio":metrics.overread_ratio(),"timing":timing_json(&stats,block.len())}));
    }

    let cold_start = 3_000_000u64;
    let cold_end = cold_start + 65_536u64;
    let (cold_stats, cold_range) = measure(|| {
        let mut decoder =
            AceIndexedDecoder::open(Cursor::new(&encoded), ace_core::DecodeLimits::default())?;
        Ok(decoder.read_range(cold_start..cold_end)?)
    })?;
    assert_eq!(cold_range, data[cold_start as usize..cold_end as usize]);
    out.push(json!({"workload_id":"mixed_16m","path":"range_64k_cold","logical_bytes_requested":65_536,"bytes_returned":cold_range.len(),"timing":timing_json(&cold_stats,cold_range.len())}));

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
        out.push(json!({"workload_id":"mixed_16m","path":name,"offset":start,"logical_bytes_requested":end-start,"bytes_returned":range.len(),"physical_bytes_read":metrics.physical_bytes_read,"blocks_touched":metrics.blocks_touched,"blocks_decoded":metrics.blocks_decoded,"physical_to_logical_ratio":metrics.overread_ratio(),"timing":timing_json(&stats,range.len())}));
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
        rows.push(json!({
            "workload_id":format!("mixed_{mib}m"), "path":"bounded_stream",
            "input_bytes":data.len(), "output_bytes":encoded.len(),
            "compression_ratio":data.len() as f64/encoded.len().max(1) as f64,
            "peak_source_buffer_bytes":stream_stats.peak_source_buffer_bytes,
            "blocks":stream_stats.blocks, "timing":timing_json(&stats,data.len())
        }));
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
    Ok(vec![json!({
        "workload_id":"mixed_16m", "path":"worker_scratch_and_planner",
        "block_size_bytes":cfg.block_size,
        "scratch_reserved_bytes":reserved_before,
        "scratch_reserved_after_reset_bytes":reserved_after_reset,
        "scratch_capacity_preserved":reserved_before==reserved_after_reset,
        "planner_fast_path_blocks":telemetry.planner_fast_path_blocks,
        "planner_estimated_candidates":telemetry.planner_estimated_candidates,
        "planner_sampled_candidates":telemetry.planner_sampled_candidates,
        "planner_full_trial_encodes":telemetry.planner_full_trial_encodes
    })])
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
