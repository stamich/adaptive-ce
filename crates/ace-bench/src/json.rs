//! JSON document model and serialization helpers (schema 2.1).

use crate::prelude::*;

/// Milestone tag written into every document and into result file names
/// (`benchmark-<MILESTONE>-<family>.json`); the single place to bump per release.
pub(crate) const MILESTONE: &str = "0.5.0";
/// Milestone whose results are the direct predecessor of this one.
pub(crate) const BASE_MILESTONE: &str = "0.4.6";
/// Benchmark document schema version (2.1 = 2.0 + Harness V3 statistics and fingerprint).
pub(crate) const SCHEMA_VERSION: &str = "2.1";
/// Environment variable overriding the result directory.
pub(crate) const OUT_DIR_ENV: &str = "ACE_BENCH_OUT_DIR";

/// Incremental JSON object builder used to keep benchmark serialization compile-friendly.
///
/// Large `serde_json::json!` object literals expand recursively at compile time. Benchmark
/// documents evolve frequently and can therefore exceed Rust's macro recursion limit even
/// though the runtime data itself is not recursive. This builder keeps the public JSON schema
/// unchanged while constructing large objects from small, independently reviewable sections.
#[derive(Debug, Default)]
pub(crate) struct JsonObjectBuilder {
    /// Fields in insertion-independent (sorted) order.
    pub(crate) fields: Map<String, Value>,
}

/// Inherent methods of [`JsonObjectBuilder`].
impl JsonObjectBuilder {
    /// Creates an empty JSON object builder.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Inserts any serializable value under `key`.
    pub(crate) fn field<T: Serialize>(&mut self, key: &str, value: T) -> &mut Self {
        let value = serde_json::to_value(value).expect("benchmark JSON field must be serializable");
        self.fields.insert(key.to_string(), value);
        self
    }

    /// Inserts a value that is already represented as `serde_json::Value`.
    pub(crate) fn value(&mut self, key: &str, value: Value) -> &mut Self {
        self.fields.insert(key.to_string(), value);
        self
    }

    /// Merges another section into this object without adding a schema nesting level.
    pub(crate) fn extend(&mut self, section: JsonObjectBuilder) -> &mut Self {
        self.fields.extend(section.fields);
        self
    }

    /// Finalizes the builder as a JSON object.
    pub(crate) fn build(self) -> Value {
        Value::Object(self.fields)
    }
}

/// Top-level JSON document shared by every official ACE benchmark family.
#[derive(Debug, Serialize)]
pub(crate) struct BenchmarkDocument {
    /// Document schema version ([`SCHEMA_VERSION`]).
    pub(crate) schema_version: &'static str,
    /// Always `"ace"`.
    pub(crate) project: &'static str,
    /// Milestone that produced the document ([`MILESTONE`]).
    pub(crate) milestone: &'static str,
    /// Predecessor milestone used as comparison base ([`BASE_MILESTONE`]).
    pub(crate) base: &'static str,
    /// Benchmark family name.
    pub(crate) scope: String,
    /// Contract identifier (`ace-<milestone>`).
    pub(crate) benchmark_contract_origin: String,
    /// Wall-clock generation time.
    pub(crate) generated_at_utc_epoch_seconds: u64,
    /// Environment fingerprint (see `environment.rs`).
    pub(crate) environment: Value,
    /// Static harness configuration (schema 2.0).
    pub(crate) configuration: Value,
    /// Harness V3 methodology (schema 2.1).
    pub(crate) benchmark_methodology: Value,
    /// One object per measured workload/case.
    pub(crate) workloads: Vec<Value>,
}

/// Returns static benchmark configuration metadata shared by every benchmark family.
pub(crate) fn benchmark_configuration_json() -> Value {
    let plan = MeasurementPlan::current();
    let mut row = JsonObjectBuilder::new();
    row.field("warmup_iterations", plan.warmups)
        .field("runs", plan.total_samples())
        .field("default_block_size_bytes", 262_144usize)
        .field("format_version", "1.3")
        .field("planner_version", "4.3")
        .field(
            "simd_backend",
            format!("{:?}", ace_simd::selected_backend()),
        );
    row.build()
}

/// Returns the schema-2.1 `benchmark_methodology` block describing Harness V3.
pub(crate) fn benchmark_methodology_json() -> Value {
    let plan = MeasurementPlan::current();
    let mut row = JsonObjectBuilder::new();
    row.field("harness", HARNESS_VERSION)
        .field("quick", plan.quick)
        .field("warmups", plan.warmups)
        .field("batches", plan.batches)
        .field("samples_per_batch", plan.samples_per_batch)
        .field(
            "min_sample_time_ms",
            plan.min_sample_ns as f64 / 1_000_000.0,
        )
        .field("max_iterations_per_sample", MAX_ITERATIONS_PER_SAMPLE)
        .field("calibration", "doubling")
        .field("aggregation", "median-of-medians")
        .field("stability_statistic", "batch_mad_percent")
        .field(
            "default_stable_batch_mad_percent",
            DEFAULT_STABLE_BATCH_MAD_PERCENT,
        )
        .field(
            "outlier_rule",
            format!("median +- {OUTLIER_SIGMAS}*{MAD_SIGMA_SCALE}*MAD (reported, not removed)"),
        );
    row.build()
}

/// Resolves `<out_dir>/benchmark-<MILESTONE>-<family>.json`, where `<out_dir>` is
/// `$ACE_BENCH_OUT_DIR` or `<workspace>/examples/results`.
pub(crate) fn result_path(family: &str) -> PathBuf {
    let directory = std::env::var_os(OUT_DIR_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(default_result_dir);
    directory.join(format!("benchmark-{MILESTONE}-{family}.json"))
}

/// `<workspace>/examples/results` (this crate lives at `<workspace>/crates/ace-bench`).
fn default_result_dir() -> PathBuf {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .nth(2)
        .unwrap_or(manifest)
        .join("examples")
        .join("results")
}
