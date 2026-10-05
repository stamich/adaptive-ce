#!/usr/bin/env python3
"""Evaluate ACE 0.3.1 hardened quality, performance, determinism and variance gates."""
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


def main(argv: list[str]) -> int:
    """Evaluate ACE 0.3.1 hardened quality, performance and stability gates."""
    if len(argv) != 4:
        print("usage: check_regressions.py BASELINE_DIR RESULT_DIR OUTPUT", file=sys.stderr)
        return 2

    quality_base, result, output = map(pathlib.Path, argv[1:])
    golden = pathlib.Path("examples/baselines/0.3-buildfix9-compilefix")

    q_comp = load(quality_base / "0.2.1-buildfix1-compression.json")
    q_plan = load(quality_base / "0.2.1-buildfix1-planner.json")
    g_comp = load(golden / "0.3-buildfix9-compilefix-compression.json")
    g_ra = load(golden / "0.3-buildfix9-compilefix-random-access.json")

    c_comp = load(result / "0.3.1-compression.json")
    c_plan = load(result / "0.3.1-planner.json")
    c_ra = load(result / "0.3.1-random-access.json")
    c_stability = load(result / "0.3.1-stability.json")

    plan = c_plan["workloads"][0]
    generated = float(plan.get("candidate_generation_recall", plan.get("candidate_recall", 0.0)))
    top_k = float(plan.get("top_k_recall", 0.0))
    regret = float(plan["normalized_regret_bytes_per_block"])
    p95_regret = float(plan.get("p95_regret_bytes_per_block", 1e18))
    p99_regret = float(plan.get("p99_regret_bytes_per_block", 1e18))
    full_trials = float(plan.get("full_trial_encodes_per_block", 999.0))

    c_fast_row = workload(c_comp, "ace-fast")
    c_bal_row = workload(c_comp, "ace-balanced")
    c_dense_row = workload(c_comp, "ace-dense")
    g_fast_row = workload(g_comp, "ace-fast")
    g_bal_row = workload(g_comp, "ace-balanced")
    g_dense_row = workload(g_comp, "ace-dense")

    c_fast_ratio = float(c_fast_row["compression_ratio"])
    c_bal_ratio = float(c_bal_row["compression_ratio"])
    c_dense_ratio = float(c_dense_row["compression_ratio"])
    c_fast = float(c_fast_row["compression"]["median_mb_s"])
    c_bal = float(c_bal_row["compression"]["median_mb_s"])
    c_dense = float(c_dense_row["compression"]["median_mb_s"])
    g_fast = float(g_fast_row["compression"]["median_mb_s"])
    g_bal = float(g_bal_row["compression"]["median_mb_s"])
    g_dense = float(g_dense_row["compression"]["median_mb_s"])

    q_dense_ratio = float(workload(q_comp, "ace-dense")["compression_ratio"])
    q_regret = float(q_plan["workloads"][0]["normalized_regret_bytes_per_block"])
    c_warm = float(workload(c_ra, "range_64k_warm")["timing"]["median_ns"])
    g_warm = float(workload(g_ra, "range_64k_warm")["timing"]["median_ns"])
    deterministic = bool(c_stability["workloads"][0].get("deterministic_output", False))

    checks = [
        gate("planner.generated_recall", 0.99, generated, generated >= 0.99, ">= 0.99"),
        gate("planner.top_k_recall", 0.98, top_k, top_k >= 0.98, ">= 0.98"),
        gate("planner.regret_bytes_per_block", q_regret, regret, regret <= 256.0, "<= 256"),
        gate("planner.p95_regret_bytes_per_block", 1024.0, p95_regret, p95_regret <= 1024.0, "<= 1024"),
        gate("planner.p99_regret_bytes_per_block", 4096.0, p99_regret, p99_regret <= 4096.0, "<= 4096"),
        gate("planner.full_trial_encodes_per_block", 0.0, full_trials, full_trials == 0.0, "== 0"),
        gate("compression.balanced_ratio", 3.45, c_bal_ratio, c_bal_ratio >= 3.45, ">= 3.45x"),
        gate("compression.dense_ratio", 3.45, c_dense_ratio, c_dense_ratio >= 3.45, ">= 3.45x"),
        gate("compression.dense_vs_hardened_0_2_1", q_dense_ratio * 0.995, c_dense_ratio, c_dense_ratio >= q_dense_ratio * 0.995, ">= 99.5% hardened 0.2.1"),
        gate("compression.fast_mb_s", g_fast * 0.95, c_fast, c_fast >= g_fast * 0.95, ">= 95% buildfix9 golden"),
        gate("compression.balanced_mb_s", g_bal * 0.95, c_bal, c_bal >= g_bal * 0.95, ">= 95% buildfix9 golden"),
        gate("compression.dense_mb_s", g_dense * 0.95, c_dense, c_dense >= g_dense * 0.95, ">= 95% buildfix9 golden"),
        gate("random_access.warm_64k_median_ns", g_warm * 1.075, c_warm, c_warm <= g_warm * 1.075, "<= 107.5% buildfix9 golden"),
        gate("determinism.repeated_output", 1.0, 1.0 if deterministic else 0.0, deterministic, "== true"),
    ]

    diagnostics = [
        diagnostic("planner.oracle_top2_after_sampling", float(plan.get("oracle_top2_rate_after_sampling", 0.0))),
        diagnostic("planner.oracle_top3_after_sampling", float(plan.get("oracle_top3_rate_after_sampling", 0.0))),
        diagnostic("planner.quality_pool_recall", float(plan.get("quality_pool_recall", 0.0))),
        diagnostic("planner.final_selection_recall", float(plan.get("final_selection_recall", 0.0))),
        diagnostic("planner.hybrid_lz_sample_fraction", float(plan.get("hybrid_lz_sample_fraction", 0.0))),
        diagnostic("compression.fast_cv_percent", float(c_fast_row["compression"].get("cv_percent", 0.0))),
        diagnostic("compression.balanced_cv_percent", float(c_bal_row["compression"].get("cv_percent", 0.0))),
        diagnostic("compression.dense_cv_percent", float(c_dense_row["compression"].get("cv_percent", 0.0))),
    ]

    status = "pass" if all(row["status"] == "pass" for row in checks) else "fail"
    doc = {
        "schema_version": "1.9",
        "project": "ace",
        "milestone": "0.3.1",
        "base": "0.3-buildfix9-compilefix",
        "scope": "regression",
        "benchmark_contract_origin": "ace-0.3.1",
        "generated_at_utc_epoch_seconds": int(time.time()),
        "environment": {},
        "configuration": {
            "quality_baseline": "0.2.1-buildfix1",
            "performance_reference": "0.3-buildfix9-compilefix",
            "gate_policy": "hardened quality + 5% throughput / 7.5% latency variance tolerance",
        },
        "workloads": [{
            "workload_id": "release_gates",
            "path": "0.3.1-hardened-release-gates",
            "status": status,
            "checks": checks,
            "diagnostics": diagnostics,
        }],
    }

    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(doc, indent=2) + "\n")
    print(f"ACE 0.3.1 regression gates: {status}; results written to {output}")
    for row in checks:
        print(f"  {row['status'].upper():4} {row['metric']}: {row['candidate']} ({row['rule']})")
    for row in diagnostics:
        print(f"  INFO {row['metric']}: {row['candidate']}")
    return 0 if status == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
