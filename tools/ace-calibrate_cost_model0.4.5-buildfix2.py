#!/usr/bin/env python3
"""Derive simple deterministic ACE cost hints from benchmark JSON without changing runtime behavior automatically."""
from __future__ import annotations
import json
import pathlib
import sys


def load(path: pathlib.Path) -> dict:
    """Load one JSON benchmark document."""
    return json.loads(path.read_text())


def main(argv: list[str]) -> int:
    """Print scalar-rANS/Huffman throughput ratios suitable for manual Cost Model V2.1 calibration."""
    if len(argv) != 2:
        print("usage: ace-calibrate_cost_model0.4.5-buildfix2.py ENTROPY_BENCHMARK_JSON", file=sys.stderr); return 2
    doc = load(pathlib.Path(argv[1])); grouped: dict[str, dict[str, float]] = {}
    for row in doc.get("workloads", []):
        grouped.setdefault(str(row["workload_id"]), {})[str(row["path"])] = float(row["encode"]["median_mb_s"])
    print("# ACE deterministic cost calibration hints")
    for workload, values in sorted(grouped.items()):
        h = values.get("huffman"); r = values.get("rans")
        if h and r:
            print(f"{workload}: huffman/rans encode-speed ratio={h/r:.3f}; suggested rANS relative encode coefficient >= {max(1.0,h/r):.3f}")
    print("Runtime cost constants must be reviewed and committed; this tool never rewrites planner code automatically.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
