#!/usr/bin/env python3
"""Evaluate ACE 0.3-buildfix4 release gates against the hardened quality baseline."""
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
    """Create one machine-readable regression gate row."""
    delta = candidate - baseline
    return {
        "metric": metric,
        "baseline_0_2_1_buildfix1": baseline,
        "candidate": candidate,
        "delta": delta,
        "delta_percent": None if baseline == 0 else delta / abs(baseline) * 100.0,
        "rule": rule,
        "status": "pass" if passed else "fail",
    }


def main(argv: list[str]) -> int:
    """Evaluate buildfix4 quality/performance gates and write the regression document."""
    if len(argv) != 4:
        print("usage: check_regressions.py BASELINE_DIR RESULT_DIR OUTPUT", file=sys.stderr)
        return 2
    base, result, output = map(pathlib.Path, argv[1:])
    b_comp = load(base / "0.2.1-buildfix1-compression.json")
    b_plan = load(base / "0.2.1-buildfix1-planner.json")
    b_ra = load(base / "0.2.1-buildfix1-random-access.json")
    c_comp = load(result / "0.3-buildfix4-compression.json")
    c_plan = load(result / "0.3-buildfix4-planner.json")
    c_ra = load(result / "0.3-buildfix4-random-access.json")

    plan = c_plan["workloads"][0]
    b_recall = float(b_plan["workloads"][0]["candidate_recall"])
    generated = float(plan.get("candidate_generation_recall", plan.get("candidate_recall", 0.0)))
    top_k = float(plan.get("top_k_recall", 0.0))
    sampled = float(plan.get("sample_survival_recall", plan.get("sample_verifier_recall", 0.0)))
    final_recall = float(plan.get("final_selection_recall", 0.0))
    b_regret = float(b_plan["workloads"][0]["normalized_regret_bytes_per_block"])
    c_regret = float(plan["normalized_regret_bytes_per_block"])
    full_trials = float(plan.get("full_trial_encodes_per_block", 999.0))

    b_dense = float(workload(b_comp, "ace-dense")["compression_ratio"])
    c_fast_row = workload(c_comp, "ace-fast")
    c_bal_row = workload(c_comp, "ace-balanced")
    c_dense_row = workload(c_comp, "ace-dense")
    c_fast_ratio = float(c_fast_row["compression_ratio"])
    c_bal_ratio = float(c_bal_row["compression_ratio"])
    c_dense_ratio = float(c_dense_row["compression_ratio"])
    b_fast = float(workload(b_comp, "ace-fast")["compression"]["median_mb_s"])
    b_bal = float(workload(b_comp, "ace-balanced")["compression"]["median_mb_s"])
    b_dense_speed = float(workload(b_comp, "ace-dense")["compression"]["median_mb_s"])
    c_fast = float(c_fast_row["compression"]["median_mb_s"])
    c_bal = float(c_bal_row["compression"]["median_mb_s"])
    c_dense = float(c_dense_row["compression"]["median_mb_s"])
    b_warm = float(workload(b_ra, "range_64k_inside")["timing"]["median_ns"])
    c_warm = float(workload(c_ra, "range_64k_warm")["timing"]["median_ns"])

    checks = [
        gate("planner.generated_recall", b_recall, generated, generated >= 0.99, ">= 0.99"),
        gate("planner.top_k_recall", 1.0, top_k, top_k >= 0.98, ">= 0.98"),
        gate("planner.sample_survival_recall", 1.0, sampled, sampled >= 0.97, ">= 0.97"),
        gate("planner.regret_bytes_per_block", b_regret, c_regret, c_regret <= 1024.0, "<= 1024"),
        gate("planner.full_trial_encodes_per_block", 0.0, full_trials, full_trials <= 0.0, "== 0"),
        gate("compression.dense_ratio", b_dense, c_dense_ratio, c_dense_ratio >= b_dense * 0.995, ">= 99.5% baseline"),
        gate("compression.profile_order_dense_balanced", c_bal_ratio, c_dense_ratio, c_dense_ratio >= c_bal_ratio, "dense ratio >= balanced ratio"),
        gate("compression.profile_order_balanced_fast", c_fast_ratio, c_bal_ratio, c_bal_ratio >= c_fast_ratio, "balanced ratio >= fast ratio"),
        gate("compression.fast_mb_s", b_fast, c_fast, c_fast >= b_fast * 2.0, ">= 2.0x baseline"),
        gate("compression.balanced_mb_s", b_bal, c_bal, c_bal >= b_bal * 4.0, ">= 4.0x baseline"),
        gate("compression.dense_mb_s", b_dense_speed, c_dense, c_dense >= b_dense_speed * 3.5, ">= 3.5x baseline"),
        gate("random_access.warm_64k_median_ns", b_warm, c_warm, c_warm <= b_warm * 1.15, "<= 115% baseline"),
    ]
    status = "pass" if all(row["status"] == "pass" for row in checks) else "fail"
    doc = {
        "schema_version": "1.5", "project": "ace", "milestone": "0.3-buildfix4",
        "base": "0.3-buildfix3", "scope": "regression", "benchmark_contract_origin": "ace-0.3-buildfix4",
        "generated_at_utc_epoch_seconds": int(time.time()), "environment": {},
        "configuration": {
            "quality_baseline": "0.2.1-buildfix1",
            "previous_observation": "0.3-buildfix2",
            "final_selection_recall_observed": final_recall,
        },
        "workloads": [{"workload_id": "release_gates", "path": "0.2.1-buildfix1-vs-0.3-buildfix4", "status": status, "checks": checks}],
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(doc, indent=2) + "\n")
    print(f"ACE 0.3-buildfix4 regression gates: {status}; results written to {output}")
    for row in checks:
        print(f"  {row['status'].upper():4} {row['metric']}: {row['candidate']} ({row['rule']})")
    return 0 if status == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
