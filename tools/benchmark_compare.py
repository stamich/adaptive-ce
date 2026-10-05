#!/usr/bin/env python3
"""Compare matching workload/path records in two shared benchmark JSON files."""
from __future__ import annotations
import json
import pathlib
import sys

def index_workloads(doc: dict) -> dict[tuple[str, str], dict]:
    """Index workload records by `(workload_id, path)` for deterministic cross-version comparison."""
    return {(str(row.get("workload_id", "")), str(row.get("path", ""))): row for row in doc.get("workloads", [])}

def scalar_metrics(row: dict) -> dict[str, float]:
    """Extract top-level numeric metrics that can be compared without knowing family-specific nested schemas."""
    result: dict[str, float] = {}
    for key, value in row.items():
        if isinstance(value, (int, float)) and not isinstance(value, bool):
            result[key] = float(value)
    return result

def main(argv: list[str]) -> int:
    """Print percentage deltas for common top-level numeric workload metrics."""
    if len(argv) != 3:
        print("usage: benchmark_compare.py OLD.json NEW.json", file=sys.stderr)
        return 2
    old = json.loads(pathlib.Path(argv[1]).read_text())
    new = json.loads(pathlib.Path(argv[2]).read_text())
    old_rows, new_rows = index_workloads(old), index_workloads(new)
    for key in sorted(old_rows.keys() & new_rows.keys()):
        old_m, new_m = scalar_metrics(old_rows[key]), scalar_metrics(new_rows[key])
        for metric in sorted(old_m.keys() & new_m.keys()):
            a, b = old_m[metric], new_m[metric]
            delta = 0.0 if a == 0.0 else (b - a) / abs(a) * 100.0
            print(f"{key[0]:20} {key[1]:24} {metric:28} {a:14.4f} {b:14.4f} {delta:+9.2f}%")
    return 0

if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
