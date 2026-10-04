#!/usr/bin/env python3
"""Evaluate ACE 0.3 release gates against the hardened 0.2.1-buildfix1 baseline."""
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
        "metric": metric, "baseline_0_2_1_buildfix1": baseline, "candidate": candidate,
        "delta": delta, "delta_percent": None if baseline == 0 else delta / abs(baseline) * 100.0,
        "rule": rule, "status": "pass" if passed else "fail",
    }


def main(argv: list[str]) -> int:
    """Evaluate 0.3 quality/performance gates and write the regression JSON document."""
    if len(argv) != 4:
        print("usage: check_regressions.py BASELINE_DIR RESULT_DIR OUTPUT", file=sys.stderr)
        return 2
    base, result, output = map(pathlib.Path, argv[1:])
    b_comp, c_comp = load(base / "0.2.1-buildfix1-compression.json"), load(result / "0.3-compression.json")
    b_plan, c_plan = load(base / "0.2.1-buildfix1-planner.json"), load(result / "0.3-planner.json")
    b_ra, c_ra = load(base / "0.2.1-buildfix1-random-access.json"), load(result / "0.3-random-access.json")

    b_recall = float(b_plan["workloads"][0]["candidate_recall"])
    c_recall = float(c_plan["workloads"][0]["candidate_recall"])
    b_regret = float(b_plan["workloads"][0]["normalized_regret_bytes_per_block"])
    c_regret = float(c_plan["workloads"][0]["normalized_regret_bytes_per_block"])
    b_dense = float(workload(b_comp, "ace-dense")["compression_ratio"])
    c_dense = float(workload(c_comp, "ace-dense")["compression_ratio"])
    b_fast = float(workload(b_comp, "ace-fast")["compression"]["median_mb_s"])
    c_fast = float(workload(c_comp, "ace-fast")["compression"]["median_mb_s"])
    b_bal = float(workload(b_comp, "ace-balanced")["compression"]["median_mb_s"])
    c_bal = float(workload(c_comp, "ace-balanced")["compression"]["median_mb_s"])
    b_dense_speed = float(workload(b_comp, "ace-dense")["compression"]["median_mb_s"])
    c_dense_speed = float(workload(c_comp, "ace-dense")["compression"]["median_mb_s"])
    b_warm = float(workload(b_ra, "range_64k_inside")["timing"]["median_ns"])
    c_warm = float(workload(c_ra, "range_64k_warm")["timing"]["median_ns"])
    full_trials = float(c_plan["workloads"][0].get("full_trial_encodes_per_block", 999.0))

    checks = [
        gate("planner.candidate_recall", b_recall, c_recall, c_recall >= 0.98, ">= 0.98"),
        gate("planner.regret_bytes_per_block", b_regret, c_regret, c_regret <= 1024.0, "<= 1024"),
        gate("planner.full_trial_encodes_per_block", 0.0, full_trials, full_trials <= 0.0, "== 0"),
        gate("compression.dense_ratio", b_dense, c_dense, c_dense >= b_dense * 0.995, ">= 99.5% baseline"),
        gate("compression.fast_mb_s", b_fast, c_fast, c_fast >= b_fast * 2.0, ">= 2.0x baseline"),
        gate("compression.balanced_mb_s", b_bal, c_bal, c_bal >= b_bal * 4.0, ">= 4.0x baseline"),
        gate("compression.dense_mb_s", b_dense_speed, c_dense_speed, c_dense_speed >= b_dense_speed * 4.0, ">= 4.0x baseline"),
        gate("random_access.warm_64k_median_ns", b_warm, c_warm, c_warm <= b_warm * 1.15, "<= 115% baseline"),
    ]
    status = "pass" if all(row["status"] == "pass" for row in checks) else "fail"
    doc = {
        "schema_version": "1.3", "project": "ace", "milestone": "0.3",
        "base": "0.2.1-buildfix1", "scope": "regression", "benchmark_contract_origin": "ace-0.3",
        "generated_at_utc_epoch_seconds": int(time.time()), "environment": {},
        "configuration": {"baseline": "0.2.1-buildfix1"},
        "workloads": [{"workload_id": "release_gates", "path": "0.2.1-buildfix1-vs-0.3", "status": status, "checks": checks}],
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(doc, indent=2) + "\n")
    print(f"ACE 0.3 regression gates: {status}; results written to {output}")
    for row in checks: print(f"  {row['status'].upper():4} {row['metric']}: {row['candidate']} ({row['rule']})")
    return 0 if status == "pass" else 1

if __name__ == "__main__": raise SystemExit(main(sys.argv))
