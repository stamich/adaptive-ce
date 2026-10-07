//! ACE Benchmark Harness V3: runs one benchmark family (or all) and writes schema-2.1 JSON
//! to `$ACE_BENCH_OUT_DIR` (default `<workspace>/examples/results`).
//!
//! ```text
//! ace-bench [<family>|all|--list]
//! ACE_BENCH_QUICK=1          quick smoke plan (not release-gate grade)
//! ACE_BENCH_MIN_SAMPLE_MS=n  override the minimum sample duration
//! ```
//!
//! Module map: `prelude` (shared imports), `json` (document model), `timing` (Harness V3
//! measurement and statistics), `environment` (fingerprint), `alloc` (counting allocator),
//! `corpus` (synthetic data), `plan_util` (planner helpers), `families` (one module per
//! subsystem).

mod alloc;
mod corpus;
mod environment;
mod families;
mod json;
mod plan_util;
mod prelude;
mod timing;

use crate::environment::{environment_json, RuntimeSnapshot};
use crate::families::*;
use crate::prelude::*;

/// Signature shared by every benchmark family.
type FamilyFn = fn() -> Result<Vec<Value>, Box<dyn std::error::Error>>;

/// Every benchmark family in `all` order: `(name, runner)`.
const FAMILIES: &[(&str, FamilyFn)] = &[
    ("release-performance", release_performance_family),
    ("compression", compression_family),
    ("entropy", entropy_family),
    ("planner", planner_family),
    ("parallel", parallel_family),
    ("random-access", random_access_family),
    ("streaming", streaming_family),
    ("memory", memory_family),
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
];

/// Resolves the command-line selection into the families to run.
fn select_families(selection: &str) -> Result<Vec<(&'static str, FamilyFn)>, String> {
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
fn run_family(name: &str, runner: FamilyFn) -> Result<PathBuf, Box<dyn std::error::Error>> {
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
        workloads,
    };
    let path = result_path(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_vec_pretty(&document)?)?;
    Ok(path)
}

/// Entry point: `ace-bench [<family>|all|--list]`.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let selection = std::env::args().nth(1).unwrap_or_else(|| "all".to_string());
    if selection == "--list" {
        for (name, _) in FAMILIES {
            println!("{name}");
        }
        return Ok(());
    }
    for (name, runner) in select_families(&selection)? {
        let started = Instant::now();
        let path = run_family(name, runner)?;
        println!(
            "[{name}] {:.1}s -> {}",
            started.elapsed().as_secs_f64(),
            path.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_names_are_unique() {
        let mut names: Vec<&str> = FAMILIES.iter().map(|(name, _)| *name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), FAMILIES.len());
    }

    #[test]
    fn unknown_family_is_rejected() {
        assert!(select_families("no-such-family").is_err());
        assert_eq!(select_families("all").map(|f| f.len()), Ok(FAMILIES.len()));
    }
}
