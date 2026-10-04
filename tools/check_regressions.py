#!/usr/bin/env python3
"""Evaluate ACE 0.2.1-buildfix1 release gates against ACE 0.2 and ACE 0.2.1 observations."""
from __future__ import annotations

import json
import pathlib
import sys
import time
from typing import Any


def load(path: pathlib.Path) -> dict[str, Any]:
    """Load one JSON document from disk."""
    return json.loads(path.read_text())


def workload(doc: dict[str, Any], path: str) -> dict[str, Any]:
    """Return the first workload row whose path matches exactly."""
    for row in doc.get("workloads", []):
        if row.get("path") == path:
            return row
    raise KeyError(f"missing workload path={path!r}")


def check(
    name: str,
    baseline: float,
    previous: float,
    candidate: float,
    passed: bool,
    rule: str,
) -> dict[str, Any]:
    """Construct one machine-readable buildfix release-gate result."""
    delta = candidate - baseline
    previous_delta = candidate - previous
    pct = None if baseline == 0 else delta / abs(baseline) * 100.0
    previous_pct = None if previous == 0 else previous_delta / abs(previous) * 100.0
    return {
        "metric": name,
        "baseline_0_2": baseline,
        "previous_0_2_1": previous,
        "candidate": candidate,
        "delta_vs_0_2": delta,
        "delta_percent_vs_0_2": pct,
        "delta_vs_0_2_1": previous_delta,
        "delta_percent_vs_0_2_1": previous_pct,
        "rule": rule,
        "status": "pass" if passed else "fail",
    }


def main(argv: list[str]) -> int:
    """Evaluate hardening gates and write `0.2.1-buildfix1-regression.json`."""
    if len(argv) != 5:
        print(
            "usage: check_regressions.py BASELINE_0_2_DIR OBSERVED_0_2_1_JSON RESULT_DIR OUTPUT",
            file=sys.stderr,
        )
        return 2

    base = pathlib.Path(argv[1])
    previous_doc = load(pathlib.Path(argv[2]))
    result = pathlib.Path(argv[3])
    output = pathlib.Path(argv[4])
    previous = previous_doc["metrics"]

    b_comp = load(base / "0.2-compression.json")
    c_comp = load(result / "0.2.1-buildfix1-compression.json")
    b_plan = load(base / "0.2-planner.json")
    c_plan = load(result / "0.2.1-buildfix1-planner.json")
    b_par = load(base / "0.2-parallel.json")
    c_par = load(result / "0.2.1-buildfix1-parallel.json")
    b_ra = load(base / "0.2-random-access.json")
    c_ra = load(result / "0.2.1-buildfix1-random-access.json")

    b_recall = float(b_plan["workloads"][0]["candidate_recall"])
    c_recall = float(c_plan["workloads"][0]["candidate_recall"])
    b_regret = float(b_plan["workloads"][0]["normalized_regret_bytes_per_block"])
    c_regret = float(c_plan["workloads"][0]["normalized_regret_bytes_per_block"])
    b_dense = float(workload(b_comp, "ace-dense")["compression_ratio"])
    c_dense = float(workload(c_comp, "ace-dense")["compression_ratio"])
    b_fast = float(workload(b_comp, "ace-fast")["compression"]["median_mb_s"])
    c_fast = float(workload(c_comp, "ace-fast")["compression"]["median_mb_s"])
    b_4t = float(workload(b_par, "threads-4")["compression"]["median_mb_s"])
    c_4t = float(workload(c_par, "threads-4")["compression"]["median_mb_s"])
    b_1t = float(workload(b_par, "threads-1")["compression"]["median_mb_s"])
    c_1t = float(workload(c_par, "threads-1")["compression"]["median_mb_s"])
    b_eff = b_4t / (4.0 * b_1t)
    c_eff = c_4t / (4.0 * c_1t)
    b_range = float(workload(b_ra, "read_range_64k")["timing"]["median_ns"])
    c_range = float(workload(c_ra, "range_64k_inside")["timing"]["median_ns"])

    checks = [
        check(
            "planner.candidate_recall",
            b_recall,
            float(previous["planner_candidate_recall"]),
            c_recall,
            c_recall >= 0.95,
            ">= 0.95",
        ),
        check(
            "planner.regret_bytes_per_block",
            b_regret,
            float(previous["planner_regret_bytes_per_block"]),
            c_regret,
            c_regret <= 8192.0,
            "<= 8192",
        ),
        check(
            "compression.dense_ratio",
            b_dense,
            float(previous["compression_dense_ratio"]),
            c_dense,
            c_dense >= b_dense * 0.99,
            ">= 99% of 0.2",
        ),
        check(
            "compression.fast_mb_s",
            b_fast,
            float(previous["compression_fast_mb_s"]),
            c_fast,
            c_fast >= b_fast * 0.95,
            ">= 95% of 0.2",
        ),
        check(
            "parallel.4t_efficiency",
            b_eff,
            float(previous["parallel_4t_efficiency"]),
            c_eff,
            c_eff >= 0.80,
            ">= 0.80",
        ),
        check(
            "random_access.64k_median_ns",
            b_range,
            float(previous["random_access_64k_median_ns"]),
            c_range,
            c_range <= b_range * 1.10,
            "<= 110% of 0.2",
        ),
    ]

    status = "pass" if all(row["status"] == "pass" for row in checks) else "fail"
    doc = {
        "schema_version": "1.2",
        "project": "ace",
        "milestone": "0.2.1-buildfix1",
        "base": "0.2.1",
        "scope": "regression",
        "benchmark_contract_origin": "ace-0.2.1-buildfix1",
        "generated_at_utc_epoch_seconds": int(time.time()),
        "environment": {},
        "configuration": {
            "absolute_gate_baseline": "0.2",
            "previous_release_observation": "0.2.1",
        },
        "workloads": [
            {
                "workload_id": "release_gates",
                "path": "0.2-and-0.2.1-vs-0.2.1-buildfix1",
                "status": status,
                "checks": checks,
            }
        ],
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(doc, indent=2) + "\n")
    print(f"ACE 0.2.1-buildfix1 regression gates: {status}; results written to {output}")
    for row in checks:
        print(f"  {row['status'].upper():4} {row['metric']}: {row['candidate']} ({row['rule']})")
    return 0 if status == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
