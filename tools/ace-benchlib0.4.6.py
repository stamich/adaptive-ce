#!/usr/bin/env python3
"""Shared helpers of the ACE 0.4.6 benchmark tools (schema 2.0 and 2.1).

Loaded by every ``tools/ace-*0.4.6.py`` script through :func:`load_benchlib`-style
``importlib`` boilerplate (the hyphenated, versioned file name cannot be imported directly).

Contents:

* document access  -- :func:`load_doc`, :func:`find_row`, :func:`timing`;
* Harness V3 views -- :func:`stable_timing`, :func:`batch_mad_percent`, :func:`median_of_medians`;
* statistics       -- :func:`median`, :func:`mad`;
* gate model       -- :data:`PASS` / :data:`FAIL` / :data:`UNSTABLE` / :data:`SKIPPED`,
  :func:`gate`, :func:`overall_status`;
* output           -- :func:`markdown_table`, :func:`write_json`.
"""
from __future__ import annotations

import json
import pathlib
from typing import Any, Iterable

#: Milestone produced by this tool set.
MILESTONE = "0.4.6"
#: Predecessor used as the A/B and stored-baseline reference.
BASELINE = "0.4.5-buildfix2"
#: Schema version written by Harness V3.
SCHEMA = "2.1"

#: Gate requirement met with a stable measurement.
PASS = "pass"
#: Gate requirement violated with a stable measurement.
FAIL = "fail"
#: Measurement spread above the stability limit (retried by the release script).
UNSTABLE = "unstable"
#: Not applicable / input not supplied; never acceptable for a release-core gate.
SKIPPED = "skipped"
#: Informational row that never affects the verdict.
DIAGNOSTIC = "diagnostic"

Row = dict[str, Any]


def load_doc(path: pathlib.Path) -> Row:
    """Load one benchmark JSON document."""
    return json.loads(path.read_text())


def result_file(directory: pathlib.Path, milestone: str, family: str) -> pathlib.Path:
    """Path of ``benchmark-<milestone>-<family>.json`` inside ``directory``."""
    return directory / f"benchmark-{milestone}-{family}.json"


def find_row(doc: Row, **match: Any) -> Row:
    """Return the first workload row whose fields equal every ``match`` item."""
    for row in doc.get("workloads", []):
        if all(row.get(key) == value for key, value in match.items()):
            return row
    raise KeyError(f"{doc.get('scope')}: no workload row matching {match}")


def timing(row: Row, key: str) -> Row:
    """Return the timing object ``row[key]`` (``{}`` when absent)."""
    value = row.get(key)
    return value if isinstance(value, dict) else {}


def stable_timing(timing_obj: Row) -> Row | None:
    """Schema-2.1 ``stable_timing`` block, or ``None`` for schema-2.0 measurements."""
    value = timing_obj.get("stable_timing")
    return value if isinstance(value, dict) else None


def batch_mad_percent(timing_obj: Row) -> float | None:
    """Batch MAD in percent, or ``None`` for schema-2.0 measurements (not gateable)."""
    stable = stable_timing(timing_obj)
    return None if stable is None else float(stable["batch_mad_percent"])


def median_of_medians(timing_obj: Row) -> float:
    """Release median in ns (schema 2.1 MoM, schema 2.0 plain median)."""
    stable = stable_timing(timing_obj)
    return float(stable["median_of_medians_ns"] if stable else timing_obj["median_ns"])


def median(values: Iterable[float]) -> float:
    """Median (mean of the middle pair for even lengths); 0.0 for no values."""
    ordered = sorted(values)
    if not ordered:
        return 0.0
    mid = len(ordered) // 2
    return ordered[mid] if len(ordered) % 2 else (ordered[mid - 1] + ordered[mid]) / 2.0


def mad(values: Iterable[float], center: float) -> float:
    """Median absolute deviation of ``values`` around ``center``."""
    return median(abs(value - center) for value in values)


def gate(
    name: str,
    section: str,
    status: str,
    measured: float | None,
    required: float | None,
    rule: str,
    **extra: Any,
) -> Row:
    """Build one machine-readable gate row."""
    row: Row = {
        "name": name,
        "section": section,
        "status": status,
        "measured": measured,
        "required": required,
        "rule": rule,
    }
    row.update(extra)
    return row


def threshold_status(measured: float, required: float, higher_is_better: bool) -> str:
    """``PASS`` when ``measured`` satisfies ``required`` in the given direction."""
    ok = measured >= required if higher_is_better else measured <= required
    return PASS if ok else FAIL


def overall_status(gates: Iterable[Row]) -> str:
    """Release verdict: ``fail`` > ``unstable`` > ``incomplete`` (skipped) > ``pass``."""
    statuses = {row["status"] for row in gates if row["status"] != DIAGNOSTIC}
    if FAIL in statuses:
        return FAIL
    if UNSTABLE in statuses:
        return UNSTABLE
    if SKIPPED in statuses:
        return "incomplete"
    return PASS


def markdown_table(headers: list[str], rows: Iterable[Iterable[Any]]) -> str:
    """Render a GitHub-flavoured Markdown table."""
    lines = [
        "| " + " | ".join(headers) + " |",
        "|" + "|".join("---" for _ in headers) + "|",
    ]
    lines.extend("| " + " | ".join(str(cell) for cell in row) + " |" for row in rows)
    return "\n".join(lines)


def fmt(value: Any, digits: int = 1) -> str:
    """Format a number with ``digits`` decimals; other values via ``str`` (``None`` -> ``-``)."""
    if value is None:
        return "-"
    if isinstance(value, bool):
        return str(value).lower()
    if isinstance(value, (int, float)):
        return f"{value:,.{digits}f}"
    return str(value)


def write_json(path: pathlib.Path, doc: Row) -> None:
    """Write ``doc`` as pretty JSON, creating parent directories."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(doc, indent=2) + "\n")
