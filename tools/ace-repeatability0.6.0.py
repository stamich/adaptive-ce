#!/usr/bin/env python3
"""ACE 0.6.0 benchmark repeatability: N complete runs -> median documents + gates.

Gates (exit 1 on violation):

* ``deterministic`` -- every value that does not measure time or memory (compressed sizes,
  ratios, planner decisions, recall / regret, counters, input fingerprints, ...) is identical
  in every run, in every family present in all runs;
* ``macro``         -- the spread ``(max - min) / median`` of every macro throughput median of
  ``release-performance`` (whole-input compression / decompression; random-access latencies
  are reported as diagnostics) across the runs is <= ``--max-spread`` percent (default 5) for
  compression and <= ``--max-decode-spread`` percent (default 15) for decompression timings
  (``decompression*`` and ``full_decompress.*``: whole-input decodes take only 3 - 10 ms and
  follow the CPU boost clock, so three runs spread by up to ~13 % on the reference machine even
  with the ``performance`` governor; the interleaved A/B and the batch-MAD stability gates of
  Regression V3 are what bound decode speed, this gate only detects an unusable machine).
  Quick-plan runs (``ACE_BENCH_QUICK``) cannot qualify: the macro gates are ``skipped``.

The environment of each run is checked as well: a run whose CPU governor is known and is not
``performance`` makes the verdict ``diagnostic`` (exit 4) instead of ``pass`` — such runs are
never a release baseline.

With ``--median-out DIR`` the tool writes, per family, the run document whose timing objects
are replaced by the median run of each timing object (the object whose median-of-medians is
the median of the runs). That directory is the release baseline (median of three clean runs).

usage: ace-repeatability0.6.0.py RUN_DIR RUN_DIR [RUN_DIR ...] [--median-out DIR]
                                 [--max-spread PERCENT] [--max-decode-spread PERCENT]
                                 [--output FILE]
"""
from __future__ import annotations

import argparse
import copy
import importlib.util
import pathlib
import re
import sys
import time
from typing import Any, Iterator


def _benchlib():
    """Load the shared ``ace-benchlib0.6.0.py`` module that sits next to this script."""
    path = pathlib.Path(__file__).with_name("ace-benchlib0.6.0.py")
    spec = importlib.util.spec_from_file_location("ace_benchlib", path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


lib = _benchlib()

#: Families whose documents are not Harness V3 measurements.
NOT_MEASUREMENTS = {"regression", "ab", "repeatability"}
#: Top-level keys that differ between runs by construction.
RUN_SPECIFIC = {"generated_at_utc_epoch_seconds", "environment"}
#: Leaf keys that measure time, speed, memory or the machine (never compared for identity).
MEASURED_KEY = re.compile(
    r"(_ns|_ms|_us|_s|_seconds|mb_s|mib_s|gb_s|throughput|speed|speedup|latency|elapsed|"
    r"duration|time|cv_percent|mad|outlier|stability|unstable|iterations|rss|peak|alloc|"
    r"freq|loadavg|thermal|samples|efficiency)",
    re.IGNORECASE,
)
#: Containers of measurements named with a unit suffix (``stage_timing_ns`` holds one number per
#: stage, none of whose own keys looks like a measurement).
MEASURED_CONTAINER = re.compile(r"_(ns|ms|us)$")
#: Release-performance cases that measure latency, not macro throughput.
MICRO_CASE_PREFIXES = ("random_access.",)
#: Default macro-throughput spread limit (percent).
DEFAULT_MAX_SPREAD = 5.0
#: Default macro spread limit for decompression timings (percent); see the module docstring.
DEFAULT_MAX_DECODE_SPREAD = 15.0


def spread_limit(case: str, kind: str, compression: float, decode: float) -> float:
    """The spread limit of one release-performance measurement."""
    if kind.startswith("decompression") or case.startswith("full_decompress."):
        return decode
    return compression


def is_timing(value: Any) -> bool:
    """A Harness timing object (``median_ns`` + ``samples_ns``)."""
    return isinstance(value, dict) and "median_ns" in value and "samples_ns" in value


def deterministic_leaves(value: Any, where: str = "") -> Iterator[tuple[str, Any]]:
    """``(path, leaf)`` of every value expected to be identical between runs."""
    if is_timing(value):
        return
    if isinstance(value, dict):
        for key in sorted(value):
            if not where and key in RUN_SPECIFIC:
                continue
            nested = value[key]
            if not isinstance(nested, (dict, list)) and MEASURED_KEY.search(key):
                continue
            if isinstance(nested, dict) and MEASURED_CONTAINER.search(key):
                continue
            yield from deterministic_leaves(nested, f"{where}.{key}" if where else key)
    elif isinstance(value, list):
        for index, nested in enumerate(value):
            yield from deterministic_leaves(nested, f"{where}[{index}]")
    else:
        yield where, value


def timing_objects(value: Any, where: str = "") -> Iterator[tuple[str, dict]]:
    """``(path, timing object)`` of every measurement in a document."""
    if is_timing(value):
        yield where, value
        return
    if isinstance(value, dict):
        for key in sorted(value):
            yield from timing_objects(value[key], f"{where}.{key}" if where else key)
    elif isinstance(value, list):
        for index, nested in enumerate(value):
            yield from timing_objects(nested, f"{where}[{index}]")


def resolve(doc: Any, path: str) -> Any:
    """The value at ``path`` (as produced by the walkers above)."""
    current = doc
    for part in re.findall(r"[^.\[\]]+|\[\d+\]", path):
        current = current[int(part[1:-1])] if part.startswith("[") else current[part]
    return current


def assign(doc: Any, path: str, value: Any) -> None:
    """Replace the value at ``path``."""
    parts = re.findall(r"[^.\[\]]+|\[\d+\]", path)
    parent = resolve(doc, "".join(p if p.startswith("[") else f".{p}" for p in parts[:-1]).lstrip("."))
    last = parts[-1]
    if last.startswith("["):
        parent[int(last[1:-1])] = value
    else:
        parent[last] = value


def families(runs: list[pathlib.Path]) -> list[str]:
    """Families present in every run."""
    def names(run: pathlib.Path) -> set[str]:
        prefix = f"benchmark-{lib.MILESTONE}-"
        return {p.name[len(prefix):-5] for p in run.glob(f"{prefix}*.json")} - NOT_MEASUREMENTS
    common = names(runs[0])
    for run in runs[1:]:
        common &= names(run)
    return sorted(common)


def governor(doc: dict) -> str | None:
    """CPU governor recorded in a document's environment (``None`` when unknown)."""
    cpu = doc.get("environment", {}).get("cpu", {})
    return cpu.get("governor") if isinstance(cpu, dict) else None


def main(argv: list[str]) -> int:
    """Compare the runs, write the summary (and median documents) and return the exit code."""
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("runs", nargs="+", type=pathlib.Path)
    parser.add_argument("--median-out", type=pathlib.Path)
    parser.add_argument("--max-spread", type=float, default=DEFAULT_MAX_SPREAD)
    parser.add_argument("--max-decode-spread", type=float, default=DEFAULT_MAX_DECODE_SPREAD)
    parser.add_argument("--output", type=pathlib.Path)
    args = parser.parse_args(argv)
    if len(args.runs) < 2:
        parser.error("at least two run directories are required")

    gates: list[dict] = []
    mismatches: list[str] = []
    governors: set[str | None] = set()
    common = families(args.runs)
    if "release-performance" not in common:
        gates.append(lib.gate("macro.release_performance", "macro", lib.SKIPPED, None, None,
                              "release-performance missing from at least one run"))
    for family in common:
        docs = [lib.load_doc(lib.result_file(run, lib.MILESTONE, family)) for run in args.runs]
        governors.update(governor(doc) for doc in docs)
        reference = dict(deterministic_leaves(docs[0]))
        for index, doc in enumerate(docs[1:], start=2):
            other = dict(deterministic_leaves(doc))
            for key in sorted(set(reference) | set(other)):
                if reference.get(key, "<absent>") != other.get(key, "<absent>"):
                    mismatches.append(f"{family}: {key}: run 1 = {reference.get(key, '<absent>')!r}, "
                                      f"run {index} = {other.get(key, '<absent>')!r}")
        median_doc = copy.deepcopy(docs[0])
        for where, _ in timing_objects(docs[0]["workloads"], "workloads"):
            objects = [resolve(doc, where) for doc in docs]
            values = [lib.median_of_medians(obj) for obj in objects]
            center = lib.median(values)
            chosen = min(range(len(objects)), key=lambda i: (abs(values[i] - center), i))
            assign(median_doc, where, objects[chosen])
            spread = (max(values) - min(values)) / center * 100.0 if center else 0.0
            if family == "release-performance":
                case = resolve(docs[0], where.rsplit(".", 1)[0]).get("case_id", where)
                kind = where.rsplit(".", 1)[1]
                quick = any(doc.get("benchmark_methodology", {}).get("quick") for doc in docs)
                if case.startswith(MICRO_CASE_PREFIXES):
                    status = lib.DIAGNOSTIC
                elif quick:
                    status = lib.SKIPPED
                else:
                    limit = spread_limit(case, kind, args.max_spread, args.max_decode_spread)
                    status = lib.PASS if spread <= limit else lib.FAIL
                gates.append(lib.gate(f"macro.{case}.{kind}", "macro", status, spread, limit,
                                      f"(max - min) / median <= {limit}% over {len(docs)} runs",
                                      medians_ns=values))
        if args.median_out:
            median_doc["repeatability"] = {"runs": [str(run) for run in args.runs],
                                           "rule": "median run per timing object"}
            lib.write_json(lib.result_file(args.median_out, lib.MILESTONE, family), median_doc)

    gates.insert(0, lib.gate("deterministic.values", "deterministic",
                             lib.FAIL if mismatches else lib.PASS, float(len(mismatches)), 0.0,
                             f"non-timing values identical in {len(args.runs)} runs x {len(common)} families",
                             mismatches=mismatches[:50]))
    clean = all(value in (None, "performance") for value in governors)
    gates.append(lib.gate("environment.governor", "environment", lib.DIAGNOSTIC, None, None,
                          f"governors: {sorted(str(g) for g in governors)}"))
    status = lib.overall_status(gates)
    if status == lib.PASS and not clean:
        status = lib.DIAGNOSTIC
    summary = {
        "schema_version": lib.SCHEMA, "project": "ace", "milestone": lib.MILESTONE,
        "base": lib.BASELINE, "scope": "repeatability",
        "benchmark_contract_origin": f"ace-{lib.MILESTONE}",
        "generated_at_utc_epoch_seconds": int(time.time()),
        "environment": {}, "configuration": {"runs": [str(run) for run in args.runs],
                                             "max_spread_percent": args.max_spread},
        "status": status,
        "workloads": [{"workload_id": "repeatability", "path": "repeatability", "status": status,
                       "checks": gates}],
    }
    output = args.output or lib.result_file(args.median_out or args.runs[0], lib.MILESTONE, "repeatability")
    lib.write_json(output, summary)
    print(f"ACE {lib.MILESTONE} repeatability ({len(args.runs)} runs, {len(common)} families): "
          f"{status.upper()} -> {output}")
    for row in gates:
        print(f"  {row['status'].upper():10} [{row['section']:13}] {row['name']}: "
              f"{lib.fmt(row['measured'], 2)} ({row['rule']})")
    for line in mismatches[:20]:
        print(f"    mismatch {line}")
    return {lib.PASS: 0, lib.FAIL: 1, lib.UNSTABLE: 3}.get(status, 4)


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
