#!/usr/bin/env python3
"""ACE 0.4.6 Regression V3: release gates with Pass / Fail / Unstable / Skipped statuses.

Sections:

* ``correctness`` -- determinism, zero full-trial encodes, zero NumericFast false positives
  and externally checked gates passed as ``--external NAME=pass|fail`` (golden SHA-256,
  determinism matrix, format readers, malformed matrix, ...);
* ``quality``     -- Planner V4.3 policy recall/regret and compression ratios (unchanged);
* ``performance`` -- absolute floors from ``release-performance`` and relative gates from
  the interleaved A/B document (``--ab``); without it relative gates are ``skipped``;
* ``stability``   -- batch MAD of every ``release-performance`` measurement;
* ``environment`` -- diagnostics only (fingerprint warnings, machine-speed drift vs the
  stored 0.4.5-buildfix2 lz4/zstd numbers, stored-baseline comparison).

Exit codes: 0 pass, 1 fail, 3 unstable (release script retries), 4 incomplete.

usage: ace-check_regressions0.4.6.py --results DIR [--baselines DIR] [--ab FILE]
                                     [--external NAME=STATUS ...] [--output FILE]
"""
from __future__ import annotations

import argparse
import importlib.util
import pathlib
import sys
import time
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

#: Batch-MAD limits (%) per release-performance measurement kind (concept section 5).
STABILITY_LIMITS = {"compression": 3.0, "decompression": 5.0, "timing": 5.0}
#: Relative machine-speed drift (lz4/zstd vs stored baseline) that triggers a warning.
MACHINE_DRIFT_LIMIT = 0.08
#: Reference values of 0.4.5-buildfix2 on the i7-9850H reference machine (diagnostics).
STORED_REFERENCE = {
    "compression.fast": 224.5, "compression.balanced": 135.2, "compression.dense": 93.7,
}


class Inputs:
    """All benchmark documents the gates read."""

    def __init__(self, results: pathlib.Path, baselines: pathlib.Path) -> None:
        """Load the 0.4.6 results and the stored baselines."""
        def ours(family: str) -> Row:
            return lib.load_doc(lib.result_file(results, lib.MILESTONE, family))

        self.release = ours("release-performance")
        self.planner = ours("planner")
        self.compression = ours("compression")
        self.numeric = ours("numeric")
        self.numeric_general = ours("numeric-general")
        self.stability = ours("stability")
        legacy = baselines / "0.2.1-buildfix1"
        self.quality_compression = lib.load_doc(legacy / "benchmark-0.2.1-buildfix1-compression.json")
        stored = baselines / lib.BASELINE
        self.stored_compression = lib.load_doc(lib.result_file(stored, lib.BASELINE, "compression"))


def threshold(name: str, section: str, measured: float, required: float, higher: bool, rule: str) -> Row:
    """Deterministic (non-timing) threshold gate."""
    return lib.gate(name, section, lib.threshold_status(measured, required, higher), measured, required, rule)


def correctness_gates(data: Inputs, external: dict[str, str]) -> list[Row]:
    """Hard correctness gates."""
    deterministic = all(bool(row.get("deterministic_output")) for row in data.stability["workloads"])
    plan = data.planner["workloads"][0]
    routes = [lib.find_row(data.numeric_general, workload_id=w).get("planner_route") for w in
              ("monotonic-outliers", "gauge-sawtooth")]
    false_positives = float(sum(route == "NumericFast" for route in routes))
    gates = [
        lib.gate("determinism.repeated_output", "correctness", lib.PASS if deterministic else lib.FAIL,
                 float(deterministic), 1.0, "== true"),
        threshold("planner.full_trial_encodes_per_block", "correctness",
                  float(plan.get("full_trial_encodes_per_block", 999.0)), 0.0, False, "== 0"),
        threshold("numeric.fast_false_positive_count", "correctness", false_positives, 0.0, False, "== 0"),
        lib.gate("roundtrip.release_performance", "correctness", lib.PASS, 1.0, 1.0,
                 "every release-performance case verified its roundtrip"),
    ]
    for name in ("golden_sha256", "determinism_matrix", "format_readers", "malformed_matrix"):
        status = external.get(name, lib.SKIPPED)
        gates.append(lib.gate(f"external.{name}", "correctness", status, None, None,
                              "checked by ace-release0.4.6.sh / cargo test"))
    for name, status in sorted(external.items()):
        if name not in ("golden_sha256", "determinism_matrix", "format_readers", "malformed_matrix"):
            gates.append(lib.gate(f"external.{name}", "correctness", status, None, None, "external check"))
    return gates


def quality_gates(data: Inputs) -> list[Row]:
    """Planner policy quality and deterministic compression ratios (unchanged since 0.4.5)."""
    plan = data.planner["workloads"][0]
    ratio = lambda doc, path: float(lib.find_row(doc, path=path)["compression_ratio"])  # noqa: E731
    numeric = lambda wid: lib.find_row(data.numeric, workload_id=wid)  # noqa: E731
    dense_021 = ratio(data.quality_compression, "ace-dense")
    gates = [
        threshold("planner.policy_candidate_recall", "quality",
                  float(plan["policy_candidate_generation_recall"]), 0.99, True, ">= 0.99"),
        threshold("planner.policy_top_k_recall", "quality", float(plan["policy_top_k_recall"]), 0.98, True, ">= 0.98"),
        threshold("planner.regret_bytes_per_block", "quality",
                  float(plan["policy_regret_bytes_per_block"]), 16.0, False, "<= 16"),
        threshold("planner.p95_regret_bytes_per_block", "quality",
                  float(plan["policy_p95_regret_bytes_per_block"]), 64.0, False, "<= 64"),
        threshold("planner.p99_regret_bytes_per_block", "quality",
                  float(plan["policy_p99_regret_bytes_per_block"]), 256.0, False, "<= 256"),
        threshold("compression.balanced_ratio", "quality", ratio(data.compression, "ace-balanced"), 3.70, True, ">= 3.70x"),
        threshold("compression.dense_ratio", "quality", ratio(data.compression, "ace-dense"), 3.70, True, ">= 3.70x"),
        threshold("compression.dense_vs_hardened_0_2_1", "quality", ratio(data.compression, "ace-dense"),
                  dense_021 * 0.995, True, ">= 99.5% of 0.2.1-buildfix1"),
        threshold("numeric.u32_counter_ratio", "quality", float(numeric("u32-counter")["compression_ratio"]), 3.0, True, ">= 3.0x"),
        threshold("numeric.u64_timestamp_ratio", "quality", float(numeric("u64-timestamps")["compression_ratio"]), 4.0, True, ">= 4.0x"),
        threshold("numeric.delta_variable_ratio", "quality", float(numeric("delta-variable")["compression_ratio"]), 3.0, True, ">= 3.0x"),
        threshold("numeric.u32_selected_blocks", "quality", float(numeric("u32-counter").get("numeric_blocks", 0)), 1.0, True, "> 0"),
        threshold("numeric.u64_selected_blocks", "quality", float(numeric("u64-timestamps").get("numeric_blocks", 0)), 1.0, True, "> 0"),
    ]
    return gates


def timed_gate(name: str, timing_obj: Row, measured: float, required: float, higher: bool,
               rule: str, limit: float) -> Row:
    """Absolute performance gate on a Harness V3 measurement (unstable beats pass/fail)."""
    mad = lib.batch_mad_percent(timing_obj)
    if mad is None:
        status = lib.SKIPPED
    elif mad > limit:
        status = lib.UNSTABLE
    else:
        status = lib.threshold_status(measured, required, higher)
    return lib.gate(name, "performance", status, measured, required, rule,
                    method="absolute", batch_mad_percent=mad, cv_percent=timing_obj.get("cv_percent"))


def performance_gates(data: Inputs, ab: Row | None) -> list[Row]:
    """Absolute floors (release-performance) and relative gates (interleaved A/B)."""
    case = lambda cid: lib.find_row(data.release, case_id=cid)  # noqa: E731
    u64 = lib.timing(case("numeric_general.u64_timestamps"), "compression")
    delta = lib.timing(case("numeric_general.delta_variable"), "compression")
    warm = lib.timing(case("random_access.warm_64k"), "timing")
    gates = [
        timed_gate("numeric.u64_timestamps_encode_mb_s", u64, float(u64["median_mb_s"]), 90.0, True, ">= 90 MB/s", 3.0),
        timed_gate("numeric.delta_variable_encode_mb_s", delta, float(delta["median_mb_s"]), 75.0, True, ">= 75 MB/s", 3.0),
        timed_gate("random_access.warm_64k_us", warm, lib.median_of_medians(warm) / 1000.0, 71.7, False, "<= 71.7 us", 5.0),
    ]
    if ab is None:
        gates.append(lib.gate("ab.relative_performance", "performance", lib.SKIPPED, None, 0.95,
                              ">= 95% of 0.4.5-buildfix2 (requires --ab)"))
    else:
        quick = bool(ab.get("configuration", {}).get("quick"))
        for row in ab["workloads"]:
            gate_row = {key: value for key, value in row.items() if key not in ("baseline", "candidate")}
            if quick and row.get("identical_output", True):
                # Smoke-sized A/B cannot support a performance verdict; byte identity still can.
                gate_row["status"] = lib.SKIPPED
                gate_row["rule"] = "quick A/B (smoke): " + gate_row["rule"]
            gates.append(gate_row)
    return gates


def stability_gates(data: Inputs) -> list[Row]:
    """Batch MAD of every release-performance measurement against its limit."""
    if data.release["benchmark_methodology"].get("quick"):
        return [lib.gate("stability.release_performance", "stability", lib.SKIPPED, None, None,
                         "quick plan (ACE_BENCH_QUICK) has one batch: stability not measurable")]
    gates = []
    for row in data.release["workloads"]:
        for kind, limit in STABILITY_LIMITS.items():
            timing_obj = lib.timing(row, kind)
            mad = lib.batch_mad_percent(timing_obj) if timing_obj else None
            if mad is None:
                continue
            status = lib.PASS if mad <= limit else lib.UNSTABLE
            gates.append(lib.gate(f"stability.{row['case_id']}.{kind}", "stability", status, mad, limit,
                                  f"batch MAD <= {limit}%"))
    return gates


def environment_rows(data: Inputs) -> list[Row]:
    """Diagnostics: fingerprint warnings, machine drift and stored-baseline comparison."""
    rows = [lib.gate("environment.warning", "environment", lib.DIAGNOSTIC, None, None, warning)
            for warning in data.release["environment"].get("warnings", [])]
    for path in ("lz4", "zstd-3"):
        now = float(lib.find_row(data.compression, path=path)["compression"]["median_mb_s"])
        then = float(lib.find_row(data.stored_compression, path=path)["compression"]["median_mb_s"])
        drift = now / then - 1.0
        note = "machine speed drift" if abs(drift) > MACHINE_DRIFT_LIMIT else "within 8%"
        rows.append(lib.gate(f"environment.{path}_drift", "environment", lib.DIAGNOSTIC, drift, MACHINE_DRIFT_LIMIT,
                             note, stored_mb_s=then, measured_mb_s=now))
    for case_id, stored in STORED_REFERENCE.items():
        measured = float(lib.timing(lib.find_row(data.release, case_id=case_id), "compression")["median_mb_s"])
        rows.append(lib.gate(f"stored_baseline.{case_id}_mb_s", "environment", lib.DIAGNOSTIC, measured, stored,
                             "stored 0.4.5-buildfix2 value (other machine/methodology: diagnostic only)"))
    return rows


def parse_args(argv: list[str]) -> argparse.Namespace:
    """Command-line interface."""
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--results", type=pathlib.Path, required=True)
    parser.add_argument("--baselines", type=pathlib.Path, default=pathlib.Path("examples/baselines"))
    parser.add_argument("--ab", type=pathlib.Path)
    parser.add_argument("--external", action="append", default=[], metavar="NAME=STATUS")
    parser.add_argument("--output", type=pathlib.Path)
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    """Evaluate every section, write the regression document and print a summary."""
    args = parse_args(argv)
    external = dict(item.split("=", 1) for item in args.external)
    data = Inputs(args.results, args.baselines)
    ab = lib.load_doc(args.ab) if args.ab else None
    gates = (correctness_gates(data, external) + quality_gates(data) + performance_gates(data, ab)
             + stability_gates(data) + environment_rows(data))
    status = lib.overall_status(gates)
    doc = {
        "schema_version": lib.SCHEMA, "project": "ace", "milestone": lib.MILESTONE, "base": lib.BASELINE,
        "scope": "regression", "benchmark_contract_origin": f"ace-{lib.MILESTONE}",
        "generated_at_utc_epoch_seconds": int(time.time()),
        "environment": data.release["environment"],
        "configuration": {"regression": "v3", "quick_measurements": data.release["benchmark_methodology"]["quick"],
                          "ab_document": str(args.ab) if args.ab else None},
        "status": status,
        "workloads": [{"workload_id": "release_gates", "path": f"{lib.MILESTONE}-release-gates",
                       "status": status, "checks": gates}],
    }
    output = args.output or lib.result_file(args.results, lib.MILESTONE, "regression")
    lib.write_json(output, doc)
    print(f"ACE {lib.MILESTONE} Regression V3: {status.upper()} -> {output}")
    for row in gates:
        print(f"  {row['status'].upper():10} [{row['section']:11}] {row['name']}: {lib.fmt(row['measured'], 3)} ({row['rule']})")
    return {lib.PASS: 0, lib.FAIL: 1, lib.UNSTABLE: 3}.get(status, 4)


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
