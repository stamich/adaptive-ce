#!/usr/bin/env python3
"""Interleaved A/B performance comparison of ACE 0.5.0 against a baseline source tree.

Both trees get the same probe (``tools/ace-abprobe0.5.0``) compiled against their own
engine crates with identical release settings. For every case the driver runs
``batches`` probe processes per side, alternating which side goes first, so machine drift
(temperature, governor, background load) affects both sides equally.

Per case:

* ``speed_ratio``  = MoM(baseline ns) / MoM(candidate ns)   (> 1: candidate faster);
* ``ratio_upper``  = best per-batch ratio (upper bound used against noise);
* ``fail`` when the two sides produced different bytes (semantic-freeze violation);
* ``pass`` when even the worst per-batch ratio meets the requirement (robust to noise);
* ``unstable`` when either side's batch MAD exceeds the case limit (decision could flip);
* ``fail`` when ``speed_ratio < required`` *and* ``ratio_upper < required + margin``;
* otherwise ``pass``.

usage: ace-ab0.5.0.py --baseline-tree DIR [--candidate-tree DIR] [--quick]
                      [--cases ID,...] [--output FILE]
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import os
import pathlib
import platform
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass


def _benchlib():
    """Load the shared ``ace-benchlib0.5.0.py`` module that sits next to this script."""
    path = pathlib.Path(__file__).with_name("ace-benchlib0.5.0.py")
    spec = importlib.util.spec_from_file_location("ace_benchlib", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


lib = _benchlib()

REPO = pathlib.Path(__file__).resolve().parent.parent
PROBE_SOURCE = REPO / "tools" / "ace-abprobe0.5.0" / "src" / "main.rs"
#: Noise margin added to the required ratio for the per-batch upper bound.
UPPER_MARGIN = 0.02


@dataclass(frozen=True)
class Case:
    """One A/B case: probe id, required speed ratio and batch-MAD stability limit (%)."""

    case_id: str
    required_ratio: float
    stability_limit: float


CASES = [
    Case("compression.fast.encode", 0.95, 3.0),
    Case("compression.balanced.encode", 0.95, 3.0),
    Case("compression.dense.encode", 0.95, 3.0),
    Case("compression.fast.decode", 0.95, 5.0),
    Case("compression.balanced.decode", 0.95, 5.0),
    Case("compression.dense.decode", 0.95, 5.0),
    Case("numeric_fast.u32_counter.encode", 0.95, 3.0),
    Case("numeric_fast.u32_counter.decode", 0.95, 5.0),
    Case("numeric_general.u64_timestamps.encode", 0.95, 3.0),
    Case("numeric_general.delta_variable.encode", 0.95, 3.0),
    Case("full_decompress.mixed", 0.95, 5.0),
    # warm 64 KiB range: latency <= 110 % of baseline  <=>  speed ratio >= 1 / 1.10.
    Case("random_access.warm_64k", 1.0 / 1.10, 5.0),
]

PROBE_MANIFEST = """[package]
name = "ace-abprobe"
version = "0.5.0"
edition = "2021"
publish = false

[workspace]

[dependencies]
ace-core = {{ path = "{tree}/crates/ace-core" }}
ace-engine = {{ path = "{tree}/crates/ace-engine" }}
ace-corpus = {{ path = "{corpus}" }}

[profile.release]
opt-level = 3
lto = false
codegen-units = 16
debug = false
panic = "unwind"
"""


def build_probe(label: str, tree: pathlib.Path, work: pathlib.Path) -> pathlib.Path:
    """Generate the probe crate for ``tree`` under ``work/label`` and build it (release)."""
    crate = work / label
    (crate / "src").mkdir(parents=True, exist_ok=True)
    shutil.copyfile(PROBE_SOURCE, crate / "src" / "main.rs")
    manifest = PROBE_MANIFEST.format(tree=tree.resolve(), corpus=(REPO / "crates" / "ace-corpus").resolve())
    (crate / "Cargo.toml").write_text(manifest)
    lock = tree / "Cargo.lock"
    if lock.exists() and not (crate / "Cargo.lock").exists():
        shutil.copyfile(lock, crate / "Cargo.lock")
    subprocess.run(
        ["cargo", "build", "--release", "--quiet", "--manifest-path", str(crate / "Cargo.toml")],
        check=True,
    )
    return crate / "target" / "release" / "ace-abprobe"


def run_probe(binary: pathlib.Path, case: Case, args: argparse.Namespace) -> dict:
    """Run one probe batch and return its JSON result."""
    output = subprocess.run(
        [str(binary), case.case_id, str(args.samples), str(args.min_sample_ms), str(args.warmups)],
        check=True, capture_output=True, text=True,
    ).stdout
    return json.loads(output)


def side_summary(batches: list[dict]) -> dict:
    """Batch medians, median-of-medians and batch MAD of one side."""
    medians = [lib.median(batch["samples_ns"]) for batch in batches]
    mom = lib.median(medians)
    return {
        "batch_medians_ns": medians,
        "median_of_medians_ns": mom,
        "batch_mad_percent": 0.0 if mom == 0 else lib.mad(medians, mom) / mom * 100.0,
        "iterations_per_sample": [batch["iterations_per_sample"] for batch in batches],
        "mb_s": 0.0 if mom == 0 else batches[0]["bytes"] / 1_048_576 / (mom / 1e9),
    }


def evaluate(case: Case, base_batches: list[dict], cand_batches: list[dict]) -> dict:
    """Compare the two sides of one case and decide its gate status."""
    base, cand = side_summary(base_batches), side_summary(cand_batches)
    ratio = base["median_of_medians_ns"] / cand["median_of_medians_ns"]
    per_batch = [b / c for b, c in zip(base["batch_medians_ns"], cand["batch_medians_ns"])]
    lower, upper = min(per_batch), max(per_batch)
    identical = len({batch["output_fnv1a"] for batch in base_batches + cand_batches}) == 1
    worst_mad = max(base["batch_mad_percent"], cand["batch_mad_percent"])
    if not identical:
        status = lib.FAIL
    elif lower >= case.required_ratio:
        status = lib.PASS
    elif worst_mad > case.stability_limit:
        status = lib.UNSTABLE
    elif ratio < case.required_ratio and upper < case.required_ratio + UPPER_MARGIN:
        status = lib.FAIL
    else:
        status = lib.PASS
    return lib.gate(
        f"ab.{case.case_id}", "performance", status, ratio, case.required_ratio,
        f"speed ratio >= {case.required_ratio:.3f} (upper bound < {case.required_ratio + UPPER_MARGIN:.3f} confirms)",
        method="interleaved-ab", speed_ratio_lower=lower, speed_ratio_upper=upper, per_batch_speed_ratios=per_batch,
        identical_output=identical, stability_limit_percent=case.stability_limit,
        baseline=base, candidate=cand,
    )


def environment() -> dict:
    """Minimal machine fingerprint of the A/B session."""
    load = os.getloadavg() if hasattr(os, "getloadavg") else None
    return {"os": platform.system().lower(), "arch": platform.machine(), "logical_cpus": os.cpu_count(),
            "build_profile": "release", "python": platform.python_version(), "loadavg_before": load}


def parse_args(argv: list[str]) -> argparse.Namespace:
    """Command-line interface."""
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--baseline-tree", type=pathlib.Path, required=True)
    parser.add_argument("--baseline-label", default=lib.BASELINE)
    parser.add_argument("--candidate-tree", type=pathlib.Path, default=REPO)
    parser.add_argument("--work-dir", type=pathlib.Path, default=REPO / "target" / "ab")
    parser.add_argument("--output", type=pathlib.Path)
    parser.add_argument("--cases", help="comma-separated subset of case ids")
    parser.add_argument("--batches", type=int, default=3)
    parser.add_argument("--samples", type=int, default=7)
    parser.add_argument("--min-sample-ms", type=int, default=50)
    parser.add_argument("--warmups", type=int, default=5)
    parser.add_argument("--quick", action="store_true", help="2 batches x 3 samples, 5 ms (smoke only)")
    args = parser.parse_args(argv)
    if args.quick:
        args.batches, args.samples, args.min_sample_ms, args.warmups = 2, 3, 5, 1
    if args.output is None:
        args.output = REPO / "examples" / "results" / f"ab-{lib.MILESTONE}-vs-{args.baseline_label}.json"
    return args


def main(argv: list[str]) -> int:
    """Build both probes, run the interleaved session and write the A/B document."""
    args = parse_args(argv)
    selected = CASES if not args.cases else [c for c in CASES if c.case_id in args.cases.split(",")]
    if not selected:
        print("no matching A/B cases", file=sys.stderr)
        return 2
    print(f"[ab] building probes ({args.baseline_label} vs {lib.MILESTONE})")
    probes = {
        "baseline": build_probe(args.baseline_label, args.baseline_tree, args.work_dir),
        "candidate": build_probe(lib.MILESTONE, args.candidate_tree, args.work_dir),
    }
    rows = []
    for case in selected:
        runs: dict[str, list[dict]] = {"baseline": [], "candidate": []}
        for batch in range(args.batches):
            order = ("baseline", "candidate") if batch % 2 == 0 else ("candidate", "baseline")
            for side in order:
                runs[side].append(run_probe(probes[side], case, args))
        row = evaluate(case, runs["baseline"], runs["candidate"])
        rows.append(row)
        print(f"  {row['status'].upper():8} {case.case_id:40} ratio={row['measured']:.3f} "
              f"upper={row['speed_ratio_upper']:.3f} identical={row['identical_output']}")
    status = lib.overall_status(rows)
    doc = {
        "schema_version": lib.SCHEMA, "project": "ace", "milestone": lib.MILESTONE,
        "base": args.baseline_label, "scope": "ab",
        "benchmark_contract_origin": f"ace-{lib.MILESTONE}",
        "generated_at_utc_epoch_seconds": int(time.time()),
        "environment": environment(),
        "configuration": {"batches": args.batches, "samples_per_batch": args.samples,
                          "min_sample_time_ms": args.min_sample_ms, "warmups": args.warmups,
                          "quick": args.quick, "interleaving": "alternating side order per batch"},
        "status": status,
        "workloads": rows,
    }
    lib.write_json(args.output, doc)
    print(f"[ab] {status}; written to {args.output}")
    return {lib.PASS: 0, lib.UNSTABLE: 3}.get(status, 1)


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
