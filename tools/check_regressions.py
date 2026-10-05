#!/usr/bin/env python3
"""Evaluate ACE 0.3-buildfix9-compilefix hardening gates against quality baselines and absolute targets."""
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
    """Evaluate buildfix9-compilefix quality/correctness/performance gates and write schema 1.9 output."""
    if len(argv) != 4:
        print("usage: check_regressions.py BASELINE_DIR RESULT_DIR OUTPUT", file=sys.stderr)
        return 2
    base, result, output = map(pathlib.Path, argv[1:])
    b_comp = load(base / "0.2.1-buildfix1-compression.json")
    b_plan = load(base / "0.2.1-buildfix1-planner.json")
    c_comp = load(result / "0.3-buildfix9-compilefix-compression.json")
    c_plan = load(result / "0.3-buildfix9-compilefix-planner.json")
    c_ra = load(result / "0.3-buildfix9-compilefix-random-access.json")

    plan = c_plan["workloads"][0]
    generated = float(plan.get("candidate_generation_recall", plan.get("candidate_recall", 0.0)))
    top_k = float(plan.get("top_k_recall", 0.0))
    regret = float(plan["normalized_regret_bytes_per_block"])
    p95_regret = float(plan.get("p95_regret_bytes_per_block", 1e18))
    full_trials = float(plan.get("full_trial_encodes_per_block", 999.0))

    c_fast_row = workload(c_comp, "ace-fast")
    c_bal_row = workload(c_comp, "ace-balanced")
    c_dense_row = workload(c_comp, "ace-dense")
    c_fast_ratio = float(c_fast_row["compression_ratio"])
    c_bal_ratio = float(c_bal_row["compression_ratio"])
    c_dense_ratio = float(c_dense_row["compression_ratio"])
    c_fast = float(c_fast_row["compression"]["median_mb_s"])
    c_bal = float(c_bal_row["compression"]["median_mb_s"])
    c_dense = float(c_dense_row["compression"]["median_mb_s"])
    b_dense_ratio = float(workload(b_comp, "ace-dense")["compression_ratio"])
    b_regret = float(b_plan["workloads"][0]["normalized_regret_bytes_per_block"])
    c_warm = float(workload(c_ra, "range_64k_warm")["timing"]["median_ns"])

    checks = [
        gate("planner.generated_recall", 0.99, generated, generated >= 0.99, ">= 0.99"),
        gate("planner.top_k_recall", 0.98, top_k, top_k >= 0.98, ">= 0.98"),
        gate("planner.regret_bytes_per_block", b_regret, regret, regret <= 1024.0, "<= 1024"),
        gate("planner.p95_regret_bytes_per_block", 4096.0, p95_regret, p95_regret <= 4096.0, "<= 4096"),
        gate("planner.full_trial_encodes_per_block", 0.0, full_trials, full_trials <= 0.0, "== 0"),
        gate("compression.balanced_ratio", 3.40, c_bal_ratio, c_bal_ratio >= 3.40, ">= 3.40x"),
        gate("compression.dense_ratio", b_dense_ratio * 0.995, c_dense_ratio, c_dense_ratio >= b_dense_ratio * 0.995, ">= 99.5% hardened baseline"),
        gate("compression.profile_order_dense_balanced_tolerance", c_bal_ratio * 0.995, c_dense_ratio, c_dense_ratio >= c_bal_ratio * 0.995, "dense >= balanced * 0.995"),
        gate("compression.profile_order_balanced_fast", c_fast_ratio, c_bal_ratio, c_bal_ratio >= c_fast_ratio, "balanced ratio >= fast ratio"),
        gate("compression.fast_mb_s", 135.0, c_fast, c_fast >= 135.0, ">= 135 MB/s"),
        gate("compression.balanced_mb_s", 65.0, c_bal, c_bal >= 65.0, ">= 65 MB/s"),
        gate("compression.dense_mb_s", 42.0, c_dense, c_dense >= 42.0, ">= 42 MB/s"),
        gate("random_access.warm_64k_median_ns", 76_000.0, c_warm, c_warm <= 76_000.0, "<= 76 us"),
    ]

    diagnostics = [
        diagnostic("planner.oracle_top2_after_sampling", float(plan.get("oracle_top2_rate_after_sampling", 0.0))),
        diagnostic("planner.oracle_top3_after_sampling", float(plan.get("oracle_top3_rate_after_sampling", 0.0))),
        diagnostic("planner.quality_pool_recall", float(plan.get("quality_pool_recall", 0.0))),
        diagnostic("planner.final_selection_recall", float(plan.get("final_selection_recall", 0.0))),
        diagnostic("planner.hybrid_lz_sample_fraction", float(plan.get("hybrid_lz_sample_fraction", 0.0))),
    ]

    status = "pass" if all(row["status"] == "pass" for row in checks) else "fail"
    doc = {
        "schema_version": "1.9",
        "project": "ace",
        "milestone": "0.3-buildfix9-compilefix",
        "base": "0.3-buildfix9",
        "scope": "regression",
        "benchmark_contract_origin": "ace-0.3-buildfix9-compilefix",
        "generated_at_utc_epoch_seconds": int(time.time()),
        "environment": {},
        "configuration": {
            "quality_baseline": "0.2.1-buildfix1",
            "performance_reference": "0.3-buildfix8",
            "gate_policy": "product-quality-first; oracle rank metrics diagnostic-only",
        },
        "workloads": [{
            "workload_id": "release_gates",
            "path": "0.2.1-buildfix1-vs-0.3-buildfix9-compilefix",
            "status": status,
            "checks": checks,
            "diagnostics": diagnostics,
        }],
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(doc, indent=2) + "\n")
    print(f"ACE 0.3-buildfix9-compilefix regression gates: {status}; results written to {output}")
    for row in checks:
        print(f"  {row['status'].upper():4} {row['metric']}: {row['candidate']} ({row['rule']})")
    for row in diagnostics:
        print(f"  INFO {row['metric']}: {row['candidate']}")
    return 0 if status == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
