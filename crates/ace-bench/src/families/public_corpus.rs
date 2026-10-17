//! `public-corpus` (ACE 0.6.0): ACE next to LZ4, Zstd and gzip on a public corpus.
//!
//! The corpus is not part of the repository. Point `ACE_PUBLIC_CORPUS` at a directory with the
//! files to measure — typically the Silesia corpus (`dickens`, `mozilla`, `nci`, …) or
//! `enwik8` — and every regular file in it becomes one workload. Without the variable, or when
//! it does not name a directory, the family is skipped and writes an empty document, so `all`
//! still runs everywhere.
//!
//! Each row carries the columns of the lzbench table: compressed size, ratio in percent of the
//! input, and compression and decompression speed in MB/s (10^6 bytes per second), next to the
//! usual timing objects. `ACE_PUBLIC_CORPUS_MAX_BYTES` (default 256 MiB) truncates larger files
//! so a run stays bounded; `ACE_BENCH_QUICK=1` shortens the measurement.

use std::path::Path;

use crate::prelude::*;

/// Environment variable naming the directory of the corpus files.
pub const CORPUS_ENV: &str = "ACE_PUBLIC_CORPUS";

/// Environment variable limiting the bytes read from one file.
pub const MAX_BYTES_ENV: &str = "ACE_PUBLIC_CORPUS_MAX_BYTES";

/// Default limit of bytes read from one file.
const DEFAULT_MAX_BYTES: usize = 256 * 1024 * 1024;

/// The regular files of `directory`, sorted by name, as `(file name, path)`.
///
/// # Errors
///
/// The error of reading `directory`.
pub fn corpus_files(directory: &Path) -> std::io::Result<Vec<(String, PathBuf)>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            if let Some(name) = entry.file_name().to_str() {
                files.push((name.to_owned(), entry.path()));
            }
        }
    }
    files.sort();
    Ok(files)
}

/// Reads at most `max_bytes` from the start of `path`.
fn read_prefix(path: &Path, max_bytes: usize) -> std::io::Result<Vec<u8>> {
    let mut data = Vec::new();
    fs::File::open(path)?
        .take(u64::try_from(max_bytes).unwrap_or(u64::MAX))
        .read_to_end(&mut data)?;
    Ok(data)
}

/// Megabytes (10^6 bytes) per second for `bytes` processed in a median of `median_ns`.
pub fn megabytes_per_second(bytes: usize, median_ns: f64) -> f64 {
    if median_ns <= 0.0 {
        0.0
    } else {
        bytes as f64 / median_ns * 1e3
    }
}

/// One result row.
fn row(
    workload: &str,
    path: &str,
    input_bytes: usize,
    compressed_bytes: usize,
    compression: &SampleStats,
    decompression: &SampleStats,
) -> Value {
    let mut lzbench = JsonObjectBuilder::new();
    lzbench
        .field("compressed_bytes", compressed_bytes)
        .field(
            "ratio_percent",
            compressed_bytes as f64 / input_bytes.max(1) as f64 * 100.0,
        )
        .field(
            "compress_mb_s",
            megabytes_per_second(input_bytes, compression.median_ns),
        )
        .field(
            "decompress_mb_s",
            megabytes_per_second(input_bytes, decompression.median_ns),
        );
    let mut row = JsonObjectBuilder::new();
    row.field("workload_id", workload)
        .field("path", path)
        .field("input_bytes", input_bytes)
        .field("compressed_bytes", compressed_bytes)
        .field(
            "compression_ratio",
            input_bytes as f64 / compressed_bytes.max(1) as f64,
        )
        .value("lzbench", lzbench.build())
        .value("compression", timing_json(compression, input_bytes))
        .value("decompression", timing_json(decompression, input_bytes));
    row.build()
}

/// Measures ACE (three profiles, one thread), LZ4, Zstd level 3 and gzip level 6 on `data`.
fn measure_file(
    workload: &str,
    data: &[u8],
    plan: MeasurementPlan,
) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    for (name, profile) in [
        ("ace-fast", CompressionProfile::Fast),
        ("ace-balanced", CompressionProfile::Balanced),
        ("ace-dense", CompressionProfile::Dense),
    ] {
        let engine = AceEngine::new(AceConfig {
            profile,
            threads: 1,
            ..AceConfig::default()
        })?;
        let (enc, encoded) = measure_with(plan, || Ok(engine.compress(data)?))?;
        let (dec, decoded) = measure_with(plan, || Ok(engine.decompress(&encoded)?))?;
        assert!(decoded == data, "{name} does not round-trip {workload}");
        rows.push(row(workload, name, data.len(), encoded.len(), &enc, &dec));
    }

    let (enc, lz4) = measure_with(plan, || Ok(lz4_flex::compress_prepend_size(data)))?;
    let (dec, restored) = measure_with(plan, || Ok(lz4_flex::decompress_size_prepended(&lz4)?))?;
    assert!(restored == data, "lz4 does not round-trip {workload}");
    rows.push(row(workload, "lz4", data.len(), lz4.len(), &enc, &dec));

    let (enc, packed) = measure_with(plan, || Ok(zstd::stream::encode_all(Cursor::new(data), 3)?))?;
    let (dec, restored) =
        measure_with(plan, || Ok(zstd::stream::decode_all(Cursor::new(&packed))?))?;
    assert!(restored == data, "zstd does not round-trip {workload}");
    rows.push(row(
        workload,
        "zstd-3",
        data.len(),
        packed.len(),
        &enc,
        &dec,
    ));

    let (enc, gzip) = measure_with(plan, || {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::new(6));
        encoder.write_all(data)?;
        Ok(encoder.finish()?)
    })?;
    let (dec, restored) = measure_with(plan, || {
        let mut out = Vec::new();
        GzDecoder::new(Cursor::new(&gzip)).read_to_end(&mut out)?;
        Ok(out)
    })?;
    assert!(restored == data, "gzip does not round-trip {workload}");
    rows.push(row(workload, "gzip-6", data.len(), gzip.len(), &enc, &dec));
    Ok(rows)
}

/// Measures every file of `directory`.
///
/// # Errors
///
/// The error of reading a file or of an operation under measurement.
pub fn measure_directory(
    directory: &Path,
    max_bytes: usize,
    plan: MeasurementPlan,
) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    for (name, path) in corpus_files(directory)? {
        let data = fingerprinted(&name, read_prefix(&path, max_bytes)?);
        if data.is_empty() {
            continue;
        }
        eprintln!("[public-corpus] {name}: {} bytes", data.len());
        rows.extend(measure_file(&name, &data, plan)?);
    }
    Ok(rows)
}

/// Runs the family: one row per file and compressor, or nothing when no corpus is configured.
///
/// # Errors
///
/// The error of reading the corpus or of an operation under measurement.
pub fn public_corpus_family() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let Some(directory) = std::env::var_os(CORPUS_ENV).map(PathBuf::from) else {
        eprintln!("[public-corpus] skipped: set {CORPUS_ENV} to a directory with the corpus files");
        return Ok(Vec::new());
    };
    if !directory.is_dir() {
        eprintln!(
            "[public-corpus] skipped: {CORPUS_ENV}={} is not a directory",
            directory.display()
        );
        return Ok(Vec::new());
    }
    let max_bytes = std::env::var(MAX_BYTES_ENV)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_MAX_BYTES);
    measure_directory(&directory, max_bytes, MeasurementPlan::current())
}
