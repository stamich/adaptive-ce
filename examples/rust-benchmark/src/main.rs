//! ACE benchmark harness: runs one benchmark family (or all) and writes schema-2.0 JSON.
//!
//! Module map: `prelude` (shared imports), `json` (document model), `timing` (measurement),
//! `corpus` (synthetic data), `plan_util` (planner helpers), `families` (one module per
//! subsystem).

mod corpus;
mod families;
mod json;
mod plan_util;
mod prelude;
mod timing;

use crate::families::*;
use crate::prelude::*;

/// Executes one benchmark family or the full ACE 0.4 contract.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let family = std::env::args().nth(1).unwrap_or_else(|| "all".to_string());
    let families: Vec<&str> = if family == "all" {
        vec!["compression", "entropy", "planner", "parallel", "random-access", "streaming", "memory", "corpus", "block-matrix", "random-access-extended", "stability", "numeric", "numeric-ablation", "block-policy", "planner-route", "numeric-fastpath", "policy-oracle-v2", "numeric-general", "planner-hotpath", "random-access-plan-diff"]
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
            "corpus" => corpus_family()?,
            "block-matrix" => block_matrix_family()?,
            "random-access-extended" => random_access_extended_family()?,
            "stability" => stability_family()?,
            "numeric" => numeric_family()?,
            "numeric-ablation" => numeric_ablation_family()?,
            "block-policy" => block_policy_family()?,
            "planner-route" => planner_route_family()?,
            "numeric-fastpath" => numeric_fastpath_family()?,
            "policy-oracle-v2" => policy_oracle_v2_family()?,
            "numeric-general" => numeric_general_family()?,
            "planner-hotpath" => planner_hotpath_family()?,
            "random-access-plan-diff" => random_access_plan_diff_family()?,
            other => return Err(format!("unknown benchmark family: {other}").into()),
        };
        let document = BenchmarkDocument {
            schema_version: "2.0",
            project: "ace",
            milestone: MILESTONE,
            base: BASE_MILESTONE,
            scope: family.to_string(),
            benchmark_contract_origin: format!("ace-{MILESTONE}"),
            generated_at_utc_epoch_seconds: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            environment: environment_json(),
            configuration: benchmark_configuration_json(),
            workloads,
        };
        let path = result_path(family);
        if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
        fs::write(&path, serde_json::to_vec_pretty(&document)?)?;
        println!("results written to {}", path.display());
    }
    Ok(())
}
