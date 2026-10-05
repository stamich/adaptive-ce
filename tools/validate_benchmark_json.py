#!/usr/bin/env python3
"""Validate the unified ACE/AdaptiveDB/GraphNet-style benchmark JSON contract."""
from __future__ import annotations

import json
import pathlib
import sys

REQUIRED = {
    "schema_version",
    "project",
    "milestone",
    "base",
    "scope",
    "benchmark_contract_origin",
    "environment",
    "configuration",
    "workloads",
}


def _require_non_empty_string(path: pathlib.Path, data: dict, key: str) -> str:
    """Return a required non-empty string field or raise a descriptive validation error."""
    value = data.get(key)
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"{path}: {key} must be a non-empty string")
    return value


def validate(path: pathlib.Path) -> None:
    """Load one benchmark JSON document and validate version-independent contract invariants."""
    data = json.loads(path.read_text())
    missing = REQUIRED.difference(data)
    if missing:
        raise ValueError(f"{path}: missing fields {sorted(missing)}")

    if data["project"] != "ace":
        raise ValueError(f"{path}: project must be 'ace'")

    milestone = _require_non_empty_string(path, data, "milestone")
    _require_non_empty_string(path, data, "base")
    scope = _require_non_empty_string(path, data, "scope")
    origin = _require_non_empty_string(path, data, "benchmark_contract_origin")

    expected_origin = f"ace-{milestone}"
    if origin != expected_origin:
        raise ValueError(
            f"{path}: benchmark_contract_origin must be {expected_origin!r}, got {origin!r}"
        )

    # Official result files are named <milestone>-<family>.json. Keep this check
    # generic so the validator works for buildfixes and future ACE milestones.
    if "results" in path.parts and not path.name.startswith(f"{milestone}-"):
        raise ValueError(
            f"{path}: result filename must start with milestone prefix {milestone!r}"
        )

    if not isinstance(data["workloads"], list):
        raise ValueError(f"{path}: workloads must be an array")

    if scope != "regression":
        environment = data.get("environment", {})
        if not isinstance(environment, dict):
            raise ValueError(f"{path}: environment must be an object")
        for key in ("os", "arch", "logical_cpus", "build_profile"):
            if key not in environment:
                raise ValueError(f"{path}: environment missing {key}")


def main(argv: list[str]) -> int:
    """Validate every supplied benchmark file and print one success line per document."""
    if len(argv) < 2:
        print("usage: validate_benchmark_json.py FILE...", file=sys.stderr)
        return 2
    for raw in argv[1:]:
        path = pathlib.Path(raw)
        validate(path)
        print(f"valid benchmark JSON: {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
