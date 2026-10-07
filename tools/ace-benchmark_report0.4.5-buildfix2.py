#!/usr/bin/env python3
"""Print compact human-readable tables for the unified ACE benchmark JSON contract."""
from __future__ import annotations
import json
import pathlib
import sys


def timing_for(row: dict) -> dict:
    """Return the most representative timing object for one workload row."""
    for key in ("compression", "encode", "timing", "planner_timing"):
        value = row.get(key)
        if isinstance(value, dict):
            if "median_mb_s" in value: return value
            if key == "timing":
                for nested in ("evaluation_per_file", "analysis_per_file", "candidate_generation_per_file"):
                    if isinstance(value.get(nested), dict): return value[nested]
    return {}


def main(argv: list[str]) -> int:
    """Load one or more benchmark JSON files and print common workload metrics or regression gates."""
    if len(argv) < 2:
        print("usage: ace-benchmark_report0.4.5-buildfix2.py FILE...", file=sys.stderr); return 2
    for raw in argv[1:]:
        path = pathlib.Path(raw); doc = json.loads(path.read_text())
        print(f"\nACE {doc.get('milestone')} benchmark scope={doc.get('scope')} file={path}")
        if doc.get("scope") == "regression":
            for workload in doc.get("workloads", []):
                print(f"status={workload.get('status')}")
                for check in workload.get("checks", []):
                    print(f"  {check.get('status','').upper():4} {check.get('metric')}: baseline={check.get('baseline', check.get('baseline_0_2_1_buildfix1'))} candidate={check.get('candidate')} rule={check.get('rule')}")
            continue
        print(f"{'workload':20} {'path':28} {'ratio':>10} {'bytes':>12} {'median MB/s':>14}")
        print("-" * 90)
        for row in doc.get("workloads", []):
            timing = timing_for(row); ratio = row.get("compression_ratio", "-")
            encoded = row.get("compressed_bytes", row.get("encoded_bytes", row.get("payload_bytes", "-")))
            speed = timing.get("median_mb_s", "-")
            ratio_s = f"{ratio:.4f}" if isinstance(ratio, (int, float)) else str(ratio)
            speed_s = f"{speed:.2f}" if isinstance(speed, (int, float)) else str(speed)
            print(f"{str(row.get('workload_id','')):20} {str(row.get('path','')):28} {ratio_s:>10} {str(encoded):>12} {speed_s:>14}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
