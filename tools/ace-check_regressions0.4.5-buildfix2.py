#!/usr/bin/env python3
"""Evaluate ACE 0.4.5-buildfix2 route-aware quality and Planner V4.3 performance gates."""
from __future__ import annotations
import json, pathlib, sys, time
from typing import Any


def load(path: pathlib.Path) -> dict[str, Any]:
    """Load one benchmark JSON document."""
    return json.loads(path.read_text())


def workload(doc: dict[str, Any], path: str) -> dict[str, Any]:
    """Return a workload row by exact path label."""
    for row in doc.get("workloads", []):
        if row.get("path") == path:
            return row
    raise KeyError(f"missing workload path={path!r}")


def gate(metric: str, baseline: float, candidate: float, passed: bool, rule: str) -> dict[str, Any]:
    """Create one machine-readable hard regression gate row."""
    delta = candidate - baseline
    return {
        "metric": metric,
        "baseline": baseline,
        "candidate": candidate,
        "delta": delta,
        "delta_percent": None if baseline == 0 else delta / abs(baseline) * 100.0,
        "rule": rule,
        "status": "pass" if passed else "fail",
    }


def diagnostic(metric: str, candidate: float) -> dict[str, Any]:
    """Create one non-gating diagnostic row retained for planner research."""
    return {"metric": metric, "candidate": candidate, "status": "diagnostic"}


def workload_id(doc: dict[str, Any], identifier: str) -> dict[str, Any]:
    """Return a workload row by exact workload identifier."""
    for row in doc.get("workloads", []):
        if row.get("workload_id") == identifier:
            return row
    raise KeyError(f"missing workload_id={identifier!r}")


def main(argv: list[str]) -> int:
    """Evaluate route-aware quality, performance preservation, NumericFast and NumericGeneral gates."""
    if len(argv) != 4:
        print("usage: ace-check_regressions0.4.5-buildfix2.py BASELINE_DIR RESULT_DIR OUTPUT", file=sys.stderr)
        return 2

    quality_base, result, output = map(pathlib.Path, argv[1:])
    golden = pathlib.Path("examples/baselines/0.3-buildfix9-compilefix")

    q_comp = load(quality_base / "benchmark-0.2.1-buildfix1-compression.json")
    q_plan = load(quality_base / "benchmark-0.2.1-buildfix1-planner.json")
    g_comp = load(golden / "benchmark-0.3-buildfix9-compilefix-compression.json")
    g_ra = load(golden / "benchmark-0.3-buildfix9-compilefix-random-access.json")

    c_comp = load(result / "benchmark-0.4.5-buildfix2-compression.json")
    c_plan = load(result / "benchmark-0.4.5-buildfix2-planner.json")
    c_ra = load(result / "benchmark-0.4.5-buildfix2-random-access.json")
    c_stability = load(result / "benchmark-0.4.5-buildfix2-stability.json")
    c_numeric = load(result / "benchmark-0.4.5-buildfix2-numeric.json")
    c_numeric_general = load(result / "benchmark-0.4.5-buildfix2-numeric-general.json")
    c_hotpath = load(result / "benchmark-0.4.5-buildfix2-planner-hotpath.json")
    bf2 = pathlib.Path("examples/baselines/0.4-buildfix2")
    b2_comp = load(bf2 / "benchmark-0.4-buildfix2-compression.json")
    b2_ra = load(bf2 / "benchmark-0.4-buildfix2-random-access.json")
    b2_numeric = load(bf2 / "benchmark-0.4-buildfix2-numeric.json")

    plan = c_plan["workloads"][0]
    generated = float(plan.get("policy_candidate_generation_recall", plan.get("route_candidate_generation_recall", plan.get("candidate_generation_recall", 0.0))))
    top_k = float(plan.get("policy_top_k_recall", plan.get("route_top_k_recall", plan.get("top_k_recall", 0.0))))
    regret = float(plan.get("policy_regret_bytes_per_block", plan.get("route_regret_bytes_per_block", plan["normalized_regret_bytes_per_block"])))
    p95_regret = float(plan.get("policy_p95_regret_bytes_per_block", plan.get("route_p95_regret_bytes_per_block", plan.get("p95_regret_bytes_per_block", 1e18))))
    p99_regret = float(plan.get("policy_p99_regret_bytes_per_block", plan.get("route_p99_regret_bytes_per_block", plan.get("p99_regret_bytes_per_block", 1e18))))
    global_regret = float(plan.get("global_size_regret_bytes_per_block", 0.0))
    full_trials = float(plan.get("full_trial_encodes_per_block", 999.0))

    c_fast_row = workload(c_comp, "ace-fast")
    c_bal_row = workload(c_comp, "ace-balanced")
    c_dense_row = workload(c_comp, "ace-dense")
    g_fast_row = workload(g_comp, "ace-fast")
    g_bal_row = workload(g_comp, "ace-balanced")
    g_dense_row = workload(g_comp, "ace-dense")

    c_bal_ratio = float(c_bal_row["compression_ratio"])
    c_dense_ratio = float(c_dense_row["compression_ratio"])
    c_fast = float(c_fast_row["compression"]["median_mb_s"])
    c_bal = float(c_bal_row["compression"]["median_mb_s"])
    c_dense = float(c_dense_row["compression"]["median_mb_s"])
    g_fast = float(g_fast_row["compression"]["median_mb_s"])
    g_bal = float(g_bal_row["compression"]["median_mb_s"])
    g_dense = float(g_dense_row["compression"]["median_mb_s"])
    b2_fast = float(workload(b2_comp, "ace-fast")["compression"]["median_mb_s"])
    b2_bal = float(workload(b2_comp, "ace-balanced")["compression"]["median_mb_s"])
    b2_dense = float(workload(b2_comp, "ace-dense")["compression"]["median_mb_s"])

    q_dense_ratio = float(workload(q_comp, "ace-dense")["compression_ratio"])
    q_regret = float(q_plan["workloads"][0]["normalized_regret_bytes_per_block"])
    c_warm = float(workload(c_ra, "range_64k_warm")["timing"]["median_ns"])
    g_warm = float(workload(g_ra, "range_64k_warm")["timing"]["median_ns"])
    b2_warm = float(workload(b2_ra, "range_64k_warm")["timing"]["median_ns"])
    deterministic = bool(c_stability["workloads"][0].get("deterministic_output", False))

    numeric_u32 = workload_id(c_numeric, "u32-counter")
    numeric_u64 = workload_id(c_numeric, "u64-timestamps")
    numeric_delta = workload_id(c_numeric, "delta-variable")
    u32_ratio = float(numeric_u32["compression_ratio"])
    u64_ratio = float(numeric_u64["compression_ratio"])
    delta_ratio = float(numeric_delta["compression_ratio"])
    u32_numeric_blocks = float(numeric_u32.get("numeric_blocks", 0))
    u64_numeric_blocks = float(numeric_u64.get("numeric_blocks", 0))
    b2_u32 = workload_id(b2_numeric, "u32-counter")
    b2_u32_mb_s = float(b2_u32["compression"]["median_mb_s"])
    u64_mb_s = float(numeric_u64["compression"]["median_mb_s"])
    delta_mb_s = float(numeric_delta["compression"]["median_mb_s"])
    outliers_route = workload_id(c_numeric_general, "monotonic-outliers").get("planner_route", "")
    sawtooth_route = workload_id(c_numeric_general, "gauge-sawtooth").get("planner_route", "")
    numeric_fast_false_positives = float(
        int(outliers_route == "NumericFast") + int(sawtooth_route == "NumericFast")
    )

    checks = [
        gate("planner.generated_recall", 0.99, generated, generated >= 0.99, ">= 0.99"),
        gate("planner.top_k_recall", 0.98, top_k, top_k >= 0.98, ">= 0.98"),
        gate("planner.regret_bytes_per_block", q_regret, regret, regret <= 16.0, "<= 16"),
        gate("planner.p95_regret_bytes_per_block", 1024.0, p95_regret, p95_regret <= 64.0, "<= 64"),
        gate("planner.p99_regret_bytes_per_block", 4096.0, p99_regret, p99_regret <= 256.0, "<= 256"),
        gate("planner.full_trial_encodes_per_block", 0.0, full_trials, full_trials == 0.0, "== 0"),
        gate("compression.balanced_ratio", 3.70, c_bal_ratio, c_bal_ratio >= 3.70, ">= 3.70x"),
        gate("compression.dense_ratio", 3.70, c_dense_ratio, c_dense_ratio >= 3.70, ">= 3.70x"),
        gate("compression.dense_vs_hardened_0_2_1", q_dense_ratio * 0.995, c_dense_ratio, c_dense_ratio >= q_dense_ratio * 0.995, ">= 99.5% hardened 0.2.1"),
        gate("compression.fast_mb_s", b2_fast * 0.95, c_fast, c_fast >= b2_fast * 0.95, ">= 95% buildfix2"),
        gate("compression.balanced_mb_s", b2_bal * 0.95, c_bal, c_bal >= b2_bal * 0.95, ">= 95% buildfix2"),
        gate("compression.dense_mb_s", b2_dense * 0.95, c_dense, c_dense >= b2_dense * 0.95, ">= 95% buildfix2"),
        gate("random_access.warm_64k_median_ns", b2_warm * 1.10, c_warm, c_warm <= b2_warm * 1.10, "<= 110% buildfix2"),
        gate("determinism.repeated_output", 1.0, 1.0 if deterministic else 0.0, deterministic, "== true"),
        gate("numeric.u32_counter_ratio", 3.0, u32_ratio, u32_ratio >= 3.0, ">= 3.0x"),
        gate("numeric.u64_timestamp_ratio", 4.0, u64_ratio, u64_ratio >= 4.0, ">= 4.0x"),
        gate("numeric.delta_variable_ratio", 3.0, delta_ratio, delta_ratio >= 3.0, ">= 3.0x"),
        gate("numeric.u32_selected", 1.0, 1.0 if u32_numeric_blocks > 0 else 0.0, u32_numeric_blocks > 0, "> 0 numeric blocks"),
        gate("numeric.u32_planner_mb_s", b2_u32_mb_s * 0.95, float(numeric_u32["compression"]["median_mb_s"]), float(numeric_u32["compression"]["median_mb_s"]) >= b2_u32_mb_s * 0.95, ">= 95% buildfix2"),
        gate("numeric.u64_selected", 1.0, 1.0 if u64_numeric_blocks > 0 else 0.0, u64_numeric_blocks > 0, "> 0 numeric blocks"),
        gate("numeric.u64_planner_mb_s", 90.0, u64_mb_s, u64_mb_s >= 90.0, ">= 90 MB/s"),
        gate("numeric.delta_variable_mb_s", 75.0, delta_mb_s, delta_mb_s >= 75.0, ">= 75 MB/s"),
        gate("numeric.fast_false_positive_count", 0.0, numeric_fast_false_positives, numeric_fast_false_positives == 0.0, "== 0"),
    ]

    diagnostics = [
        diagnostic("planner.global_size_regret_bytes_per_block", global_regret),
        diagnostic("planner.oracle_top2_after_sampling", float(plan.get("oracle_top2_rate_after_sampling", 0.0))),
        diagnostic("planner.oracle_top3_after_sampling", float(plan.get("oracle_top3_rate_after_sampling", 0.0))),
        diagnostic("planner.quality_pool_recall", float(plan.get("quality_pool_recall", 0.0))),
        diagnostic("planner.final_selection_recall", float(plan.get("final_selection_recall", 0.0))),
        diagnostic("compression.fast_cv_percent", float(c_fast_row["compression"].get("cv_percent", 0.0))),
        diagnostic("compression.balanced_cv_percent", float(c_bal_row["compression"].get("cv_percent", 0.0))),
        diagnostic("compression.dense_cv_percent", float(c_dense_row["compression"].get("cv_percent", 0.0))),
        diagnostic("numeric.u32_compress_mb_s", float(numeric_u32["compression"]["median_mb_s"])),
        diagnostic("numeric.u32_decompress_mb_s", float(numeric_u32["decompression"]["median_mb_s"])),
        diagnostic("numeric.u64_compress_mb_s", float(numeric_u64["compression"]["median_mb_s"])),
        diagnostic("numeric.u64_decompress_mb_s", float(numeric_u64["decompression"]["median_mb_s"])),
        diagnostic("planner_hotpath.mixed_route_classify_ns", float(workload_id(c_hotpath, "mixed-fast")["stage_timing"]["route_classify_ns"])),
        diagnostic("planner_hotpath.mixed_generic_analysis_ns", float(workload_id(c_hotpath, "mixed-fast")["stage_timing"]["generic_analysis_ns"])),
        diagnostic("planner_hotpath.u32_route_classify_ns", float(workload_id(c_hotpath, "u32-counter")["stage_timing"]["route_classify_ns"])),
    ]

    status = "pass" if all(row["status"] == "pass" for row in checks) else "fail"
    doc = {
        "schema_version": "2.0",
        "project": "ace",
        "milestone": "0.4.5-buildfix2",
        "base": "0.4.5-buildfix1",
        "scope": "regression",
        "benchmark_contract_origin": "ace-0.4.5-buildfix2",
        "generated_at_utc_epoch_seconds": int(time.time()),
        "environment": {},
        "configuration": {
            "legacy_quality_baseline": "0.2.1-buildfix1",
            "performance_reference": "0.3-buildfix9-compilefix",
            "gate_policy": "Planner V4.3 policy-oracle quality + buildfix2 performance preservation",
        },
        "workloads": [{
            "workload_id": "release_gates",
            "path": "0.4.5-buildfix2-release-gates",
            "status": status,
            "checks": checks,
            "diagnostics": diagnostics,
        }],
    }

    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(doc, indent=2) + "\n")
    print(f"ACE 0.4.5-buildfix2 regression gates: {status}; results written to {output}")
    for row in checks:
        print(f"  {row['status'].upper():4} {row['metric']}: {row['candidate']} ({row['rule']})")
    for row in diagnostics:
        print(f"  INFO {row['metric']}: {row['candidate']}")
    return 0 if status == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
