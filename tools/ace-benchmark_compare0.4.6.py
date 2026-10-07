#!/usr/bin/env python3
"""Compare matching rows of two ACE benchmark JSON documents (any milestones, schema 2.0/2.1).

Rows are matched by ``case_id`` or ``(workload_id, path)``. Top-level numeric fields and the
``median_mb_s`` / release median of every timing object are compared. When the documents use
different methodologies (2.0 single median vs 2.1 median-of-medians) or different machines,
a warning is printed: such deltas are diagnostics, not gates.

usage: ace-benchmark_compare0.4.6.py OLD.json NEW.json
"""
from __future__ import annotations

import importlib.util
import pathlib
import sys
from typing import Any


def _benchlib():
    """Load the shared ``ace-benchlib0.4.6.py`` module that sits next to this script."""
    path = pathlib.Path(__file__).with_name("ace-benchlib0.4.6.py")
    spec = importlib.util.spec_from_file_location("ace_benchlib", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


lib = _benchlib()
Row = dict[str, Any]


def key_of(row: Row) -> str:
    """Matching key of one workload row."""
    return str(row.get("case_id") or f"{row.get('workload_id', '')}/{row.get('path', '')}")


def metrics(row: Row) -> dict[str, float]:
    """Comparable metrics: top-level numbers plus ``<timing>.median_mb_s`` / ``.median_us``."""
    result: dict[str, float] = {}
    for key, value in row.items():
        if isinstance(value, (int, float)) and not isinstance(value, bool):
            result[key] = float(value)
        elif isinstance(value, dict) and "median_ns" in value:
            result[f"{key}.median_mb_s"] = float(value.get("median_mb_s", 0.0))
            result[f"{key}.median_us"] = lib.median_of_medians(value) / 1000.0
    return result


def warnings(old: Row, new: Row) -> list[str]:
    """Reasons why deltas between the documents are not like-for-like."""
    notes = []
    if old.get("schema_version") != new.get("schema_version"):
        notes.append(f"methodology differs (schema {old.get('schema_version')} vs {new.get('schema_version')})")
    if old.get("environment", {}).get("cpu_model") != new.get("environment", {}).get("cpu_model"):
        notes.append("different CPU models")
    return notes


def main(argv: list[str]) -> int:
    """Print percentage deltas for every common metric of every common row."""
    if len(argv) != 3:
        print(__doc__.strip().splitlines()[-1], file=sys.stderr)
        return 2
    old, new = lib.load_doc(pathlib.Path(argv[1])), lib.load_doc(pathlib.Path(argv[2]))
    for note in warnings(old, new):
        print(f"WARNING: {note}; deltas are diagnostic only")
    old_rows = {key_of(row): row for row in old.get("workloads", [])}
    new_rows = {key_of(row): row for row in new.get("workloads", [])}
    print(f"{'row':44} {'metric':36} {old.get('milestone'):>16} {new.get('milestone'):>16} {'delta':>9}")
    for key in sorted(old_rows.keys() & new_rows.keys()):
        a_metrics, b_metrics = metrics(old_rows[key]), metrics(new_rows[key])
        for metric in sorted(a_metrics.keys() & b_metrics.keys()):
            a, b = a_metrics[metric], b_metrics[metric]
            delta = 0.0 if a == 0 else (b - a) / abs(a) * 100.0
            print(f"{key[:44]:44} {metric[:36]:36} {a:16.4f} {b:16.4f} {delta:+8.2f}%")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
