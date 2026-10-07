//! JSON document model and serialization helpers (schema 2.0).

use crate::prelude::*;

/// Milestone tag written into every document and into result file names
/// (`benchmark-<MILESTONE>-<family>.json`); the single place to bump per release.
pub(crate) const MILESTONE: &str = "0.4.5-buildfix2";
/// Milestone whose results are the direct predecessor of this one.
pub(crate) const BASE_MILESTONE: &str = "0.4.5-buildfix1";

/// Incremental JSON object builder used to keep benchmark serialization compile-friendly.
///
/// Large `serde_json::json!` object literals expand recursively at compile time. Benchmark
/// documents evolve frequently and can therefore exceed Rust's macro recursion limit even
/// though the runtime data itself is not recursive. This builder keeps the public JSON schema
/// unchanged while constructing large objects from small, independently reviewable sections.
#[derive(Debug, Default)]
pub(crate) struct JsonObjectBuilder {
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
        let value = serde_json::to_value(value)
            .expect("benchmark JSON field must be serializable");
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

/// Top-level JSON document shared by every official ACE 0.4 benchmark family.
#[derive(Debug, Serialize)]
pub(crate) struct BenchmarkDocument {
    pub(crate) schema_version: &'static str,
    pub(crate) project: &'static str,
    pub(crate) milestone: &'static str,
    pub(crate) base: &'static str,
    pub(crate) scope: String,
    pub(crate) benchmark_contract_origin: String,
    pub(crate) generated_at_utc_epoch_seconds: u64,
    pub(crate) environment: Value,
    pub(crate) configuration: Value,
    pub(crate) workloads: Vec<Value>,
}

/// Returns machine/build metadata required for reproducible cross-milestone comparisons.
pub(crate) fn environment_json() -> Value {
    let cpu_model = fs::read_to_string("/proc/cpuinfo").ok().and_then(|text| {
        text.lines().find_map(|line| line.strip_prefix("model name\t: ").map(str::to_string))
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
        .field("build_profile", if cfg!(debug_assertions) { "debug" } else { "release" })
        .field("target_features", option_env!("CARGO_CFG_TARGET_FEATURE").unwrap_or("unknown"));
    row.build()
}

/// Returns static benchmark configuration metadata shared by every benchmark family.
pub(crate) fn benchmark_configuration_json() -> Value {
    let mut row = JsonObjectBuilder::new();
    row.field("warmup_iterations", WARMUPS)
        .field("runs", RUNS)
        .field("default_block_size_bytes", 262_144usize)
        .field("format_version", "1.3")
        .field("simd_backend", format!("{:?}", ace_simd::selected_backend()));
    row.build()
}

/// Resolves `examples/results/benchmark-<MILESTONE>-<family>.json`.
pub(crate) fn result_path(family: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().expect("benchmark crate lives under examples")
        .join("results").join(format!("benchmark-{MILESTONE}-{family}.json"))
}
