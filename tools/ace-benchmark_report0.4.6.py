#!/usr/bin/env python3
"""Human-readable reports for ACE benchmark JSON (schema 2.0 and 2.1).

Text mode prints one table per document. Markdown mode generates
``docs/PERFORMANCE-0.4.6.md`` from a result directory (plus optional A/B and regression
documents); the performance document is never edited by hand.

usage: ace-benchmark_report0.4.6.py FILE...
       ace-benchmark_report0.4.6.py --markdown OUT --results DIR [--ab FILE] [--regression FILE]
"""
from __future__ import annotations

import argparse
import importlib.util
import pathlib
import sys
from typing import Any, Iterator


def _benchlib():
    """Load the shared ``ace-benchlib0.4.6.py`` module that sits next to this script."""
    path = pathlib.Path(__file__).with_name("ace-benchlib0.4.6.py")
    spec = importlib.util.spec_from_file_location("ace_benchlib", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


lib = _benchlib()
Row = dict[str, Any]

#: Timing keys reported per row, in display order.
TIMING_KEYS = ("compression", "decompression", "decompression_alloc", "encode", "timing",
               "direct_numeric", "planner_v4_3", "wall_clock", "route_timing")


def row_label(row: Row) -> str:
    """Stable display label of a workload row."""
    if "case_id" in row:
        return str(row["case_id"])
    return "/".join(str(row[key]) for key in ("workload_id", "path") if key in row)


def timings(row: Row) -> Iterator[tuple[str, Row]]:
    """Yield ``(key, timing)`` for every known timing object of ``row``."""
    for key in TIMING_KEYS:
        value = lib.timing(row, key)
        if "median_ns" in value:
            yield key, value


def timing_cells(timing_obj: Row) -> list[str]:
    """MB/s, median, batch MAD and stability cells of one timing object."""
    stable = lib.stable_timing(timing_obj) or {}
    return [
        lib.fmt(timing_obj.get("median_mb_s")),
        lib.fmt(lib.median_of_medians(timing_obj) / 1000.0, 1),
        lib.fmt(stable.get("batch_mad_percent"), 2),
        str(stable.get("stability", "n/a (2.0)")),
    ]


def text_report(path: pathlib.Path) -> None:
    """Print one document as a fixed-width table."""
    doc = lib.load_doc(path)
    print(f"\nACE {doc.get('milestone')} scope={doc.get('scope')} schema={doc.get('schema_version')} file={path}")
    if doc.get("scope") in ("regression", "ab"):
        for row in doc["workloads"][0].get("checks", doc["workloads"]):
            print(f"  {row['status'].upper():10} {row['name']}: {lib.fmt(row.get('measured'), 3)} ({row['rule']})")
        return
    print(f"{'case':48} {'timing':20} {'ratio':>9} {'MB/s':>11} {'median us':>11} {'bMAD%':>7} stability")
    for row in doc.get("workloads", []):
        ratio = lib.fmt(row.get("compression_ratio"), 3)
        for key, timing_obj in timings(row):
            mb_s, med, mad, stability = timing_cells(timing_obj)
            print(f"{row_label(row)[:48]:48} {key:20} {ratio:>9} {mb_s:>11} {med:>11} {mad:>7} {stability}")


def environment_section(env: Row) -> str:
    """Markdown summary of the environment fingerprint."""
    cpu = env.get("cpu", {})
    rows = [
        ("CPU", env.get("cpu_model")), ("Cores (physical / logical)", f"{env.get('physical_cores')} / {env.get('logical_cpus')}"),
        ("SIMD backend / CRC32C", f"{env.get('simd_backend')} / {env.get('crc32c_backend')}"),
        ("Governor", cpu.get("governor")), ("rustc", env.get("rust", {}).get("rustc")),
        ("Target", env.get("rust", {}).get("target")),
        ("Profile (opt / lto / cgu)", "{profile} ({opt_level} / {lto} / {codegen_units})".format(**env.get("build", {}))
         if env.get("build") else env.get("build_profile")),
        ("Load average before", env.get("loadavg_before")), ("Warnings", "; ".join(env.get("warnings", [])) or "none"),
    ]
    return lib.markdown_table(["Item", "Value"], [(k, lib.fmt(v)) for k, v in rows])


def release_section(doc: Row) -> str:
    """Markdown table of the release-performance family."""
    rows = []
    for row in doc["workloads"]:
        for key, timing_obj in timings(row):
            rows.append([f"`{row['case_id']}`", key, lib.fmt(row.get("compression_ratio"), 3), *timing_cells(timing_obj)])
    return lib.markdown_table(["Case", "Timing", "Ratio", "MB/s", "Median µs", "Batch MAD %", "Stability"], rows)


def ab_section(doc: Row) -> str:
    """Markdown table of the interleaved A/B session."""
    rows = [[f"`{row['name'][3:]}`", row["status"].upper(), lib.fmt(row["baseline"]["mb_s"]),
             lib.fmt(row["candidate"]["mb_s"]), lib.fmt(row["measured"], 3), lib.fmt(row["required"], 3),
             str(row["identical_output"]).lower()] for row in doc["workloads"]]
    header = f"Baseline `{doc['base']}` vs candidate `{doc['milestone']}`, overall **{doc['status'].upper()}**.\n\n"
    return header + lib.markdown_table(
        ["Case", "Status", "Baseline MB/s", "Candidate MB/s", "Speed ratio", "Required", "Identical bytes"], rows)


def regression_section(doc: Row) -> str:
    """Markdown table of the regression gates (diagnostics included)."""
    checks = doc["workloads"][0]["checks"]
    rows = [[row["section"], f"`{row['name']}`", row["status"].upper(), lib.fmt(row.get("measured"), 3), row["rule"]]
            for row in checks]
    return f"Overall verdict: **{doc['status'].upper()}**.\n\n" + lib.markdown_table(
        ["Section", "Gate", "Status", "Measured", "Rule"], rows)


def comparison_section(doc: Row) -> str:
    """Markdown table of the external-codec comparison (compression family)."""
    rows = []
    for row in doc["workloads"]:
        comp, dec = lib.timing(row, "compression"), lib.timing(row, "decompression")
        rows.append([row["path"], lib.fmt(row.get("compression_ratio"), 3), lib.fmt(comp.get("median_mb_s")),
                     lib.fmt(dec.get("median_mb_s"))])
    return lib.markdown_table(["Path", "Ratio", "Compress MB/s", "Decompress MB/s"], rows)


def markdown_report(args: argparse.Namespace) -> str:
    """Build the full PERFORMANCE document."""
    release = lib.load_doc(lib.result_file(args.results, lib.MILESTONE, "release-performance"))
    methodology = release["benchmark_methodology"]
    parts = [
        f"# ACE {lib.MILESTONE} — Performance",
        "",
        "> Generated by `tools/ace-benchmark_report0.4.6.py --markdown`; do not edit by hand.",
        f"> Methodology: Harness {methodology['harness']}, {methodology['batches']} batches × "
        f"{methodology['samples_per_batch']} samples, ≥ {methodology['min_sample_time_ms']} ms per sample, "
        f"{methodology['aggregation']}" + (" — **QUICK smoke plan, not release grade**" if methodology["quick"] else "") + ".",
        "",
    ]
    if args.note:
        parts += [f"> **Note:** {args.note}", ""]
    parts += [
        "## Environment", "", environment_section(release["environment"]), "",
        "## Release performance", "", release_section(release), "",
    ]
    compression = lib.result_file(args.results, lib.MILESTONE, "compression")
    if compression.exists():
        parts += ["## ACE vs external codecs (mixed 16 MiB, 1 thread)", "",
                  comparison_section(lib.load_doc(compression)), ""]
    if args.ab:
        parts += ["## Interleaved A/B", "", ab_section(lib.load_doc(args.ab)), ""]
    if args.regression:
        parts += ["## Release gates (Regression V3)", "", regression_section(lib.load_doc(args.regression)), ""]
    return "\n".join(parts)


def main(argv: list[str]) -> int:
    """Dispatch to text or Markdown mode."""
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("files", nargs="*", type=pathlib.Path)
    parser.add_argument("--markdown", type=pathlib.Path)
    parser.add_argument("--results", type=pathlib.Path)
    parser.add_argument("--ab", type=pathlib.Path)
    parser.add_argument("--regression", type=pathlib.Path)
    parser.add_argument("--note", help="remark printed under the title (e.g. machine caveats)")
    args = parser.parse_args(argv)
    if args.markdown:
        if not args.results:
            parser.error("--markdown requires --results")
        args.markdown.parent.mkdir(parents=True, exist_ok=True)
        args.markdown.write_text(markdown_report(args) + "\n")
        print(f"performance report written to {args.markdown}")
        return 0
    if not args.files:
        parser.error("no input files")
    for path in args.files:
        text_report(path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
