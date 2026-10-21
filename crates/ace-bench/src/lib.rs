//! ACE Benchmark Harness V3 library: the families, the measurement harness and the document model
//! behind the `ace-bench` binary, which runs one benchmark family (or all) and writes schema-2.1 JSON
//! to `$ACE_BENCH_OUT_DIR` (default `<workspace>/examples/results`).
//!
//! ```text
//! ace-bench [<family>|all|--list]
//! ACE_BENCH_QUICK=1          quick smoke plan (not release-gate grade)
//! ACE_BENCH_MIN_SAMPLE_MS=n  override the minimum sample duration
//! ```
//!
//! Module map: `prelude` (shared imports), `json` (document model), `timing` (Harness V3
//! measurement and statistics), `chimp` (Chimp / Chimp128 for the codec evaluation), `environment` (fingerprint), `alloc` (counting allocator),
//! `corpus` (synthetic data), `plan_util` (planner helpers), `families` (one module per
//! subsystem).

pub mod alloc;
pub mod chimp;
pub mod corpus;
pub mod environment;
pub mod families;
pub mod json;
pub mod plan_util;
mod prelude;
pub mod timing;

use crate::environment::{environment_json, RuntimeSnapshot};
use crate::families::*;
use crate::prelude::*;

/// Signature shared by every benchmark family.
pub type FamilyFn = fn() -> Result<Vec<Value>, Box<dyn std::error::Error>>;

/// Every benchmark family in `all` order: `(name, runner)`.
pub const FAMILIES: &[(&str, FamilyFn)] = &[
    ("release-performance", release_performance_family),
    ("compression", compression_family),
    ("entropy", entropy_family),
    ("planner", planner_family),
    ("parallel", parallel_family),
    ("random-access", random_access_family),
    ("streaming", streaming_family),
    ("memory", memory_family),
    ("allocation-profile", allocation_profile_family),
    ("corpus", corpus_family),
    ("block-matrix", block_matrix_family),
    ("random-access-extended", random_access_extended_family),
    ("stability", stability_family),
    ("numeric", numeric_family),
    ("numeric-ablation", numeric_ablation_family),
    ("block-policy", block_policy_family),
    ("planner-route", planner_route_family),
    ("numeric-fastpath", numeric_fastpath_family),
    ("policy-oracle-v2", policy_oracle_v2_family),
    ("numeric-general", numeric_general_family),
    ("planner-hotpath", planner_hotpath_family),
    ("random-access-plan-diff", random_access_plan_diff_family),
    ("float-ablation", float_ablation_family),
    ("float", float_family),
    ("float-fastpath", float_fastpath_family),
    ("float-false-positive", float_false_positive_family),
    ("float-estimator", float_estimator_family),
    ("chimp-evaluation", chimp_evaluation_family),
    ("format-1-5", format15_family),
    ("public-corpus", public_corpus_family),
    ("ffi", ffi_family),
];

/// Resolves the command-line selection into the families to run.
///
/// # Errors
///
/// A message naming the selection when it is neither `all` nor a family name.
pub fn select_families(selection: &str) -> Result<Vec<(&'static str, FamilyFn)>, String> {
    if selection == "all" {
        return Ok(FAMILIES.to_vec());
    }
    FAMILIES
        .iter()
        .find(|(name, _)| *name == selection)
        .map(|family| vec![*family])
        .ok_or_else(|| format!("unknown benchmark family: {selection} (try --list)"))
}

/// Runs one family and writes its document; returns the result path.
///
/// # Errors
///
/// Any error of the family, the clock, the serialization or the result file.
pub fn run_family(name: &str, runner: FamilyFn) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let before = RuntimeSnapshot::capture();
    let workloads = runner()?;
    let after = RuntimeSnapshot::capture();
    let document = BenchmarkDocument {
        schema_version: SCHEMA_VERSION,
        project: "ace",
        milestone: MILESTONE,
        base: BASE_MILESTONE,
        scope: name.to_string(),
        benchmark_contract_origin: format!("ace-{MILESTONE}"),
        generated_at_utc_epoch_seconds: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        environment: environment_json(&before, &after),
        configuration: benchmark_configuration_json(),
        benchmark_methodology: benchmark_methodology_json(),
        workload_fingerprints: take_workload_fingerprints(),
        workloads,
    };
    let path = result_path(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_vec_pretty(&document)?)?;
    Ok(path)
}
