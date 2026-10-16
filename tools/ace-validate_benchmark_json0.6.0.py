#!/usr/bin/env python3
"""Validate ACE benchmark JSON documents (schema 2.0, 2.1 and 2.2).

Schema 2.0 checks the version-independent contract (identity fields, result file naming,
environment keys). Schema 2.1 additionally requires the ``benchmark_methodology`` block and a
complete ``stable_timing`` object next to every timing object (any dict with ``median_ns``).
Schema 2.2 (additive) also requires the git revision / dirty flag of the measured build, the
configuration fingerprint, threads and block size, and a SHA-256 for every generated input.

usage: ace-validate_benchmark_json0.6.0.py FILE...
"""
from __future__ import annotations

import json
import pathlib
import sys
from typing import Any, Iterator


REQUIRED = {
    "schema_version", "project", "milestone", "base", "scope",
    "benchmark_contract_origin", "environment", "configuration", "workloads",
}
ENVIRONMENT_20 = ("os", "arch", "logical_cpus", "build_profile")
ENVIRONMENT_21 = ("cpu", "rust", "build", "simd_backend", "crc32c_backend", "warnings")
METHODOLOGY_21 = ("harness", "warmups", "batches", "samples_per_batch", "min_sample_time_ms", "aggregation")
BUILD_22 = ("git_revision", "git_dirty")
CONFIGURATION_22 = ("config_fingerprint", "threads", "block_size")
STABLE_TIMING_21 = (
    "harness", "batches", "samples_per_batch", "iterations_per_sample", "batch_medians_ns",
    "median_of_medians_ns", "mad_ns", "batch_mad_percent", "outlier_indices", "stability",
)


class ValidationError(ValueError):
    """A benchmark document violates the contract."""


def _non_empty(path: pathlib.Path, data: dict, key: str) -> str:
    """Return a required non-empty string field."""
    value = data.get(key)
    if not isinstance(value, str) or not value.strip():
        raise ValidationError(f"{path}: {key} must be a non-empty string")
    return value


def _timing_objects(value: Any, where: str = "") -> Iterator[tuple[str, dict]]:
    """Yield ``(json_path, object)`` for every timing object (dict containing ``median_ns``)."""
    if isinstance(value, dict):
        if "median_ns" in value and "samples_ns" in value:
            yield where, value
        for key, nested in value.items():
            if key != "stable_timing":
                yield from _timing_objects(nested, f"{where}.{key}")
    elif isinstance(value, list):
        for index, nested in enumerate(value):
            yield from _timing_objects(nested, f"{where}[{index}]")


def _validate_identity(path: pathlib.Path, data: dict) -> str:
    """Identity, origin and file-name contract shared by every schema."""
    missing = REQUIRED.difference(data)
    if missing:
        raise ValidationError(f"{path}: missing fields {sorted(missing)}")
    if data["project"] != "ace":
        raise ValidationError(f"{path}: project must be 'ace'")
    milestone = _non_empty(path, data, "milestone")
    _non_empty(path, data, "base")
    scope = _non_empty(path, data, "scope")
    origin = _non_empty(path, data, "benchmark_contract_origin")
    if origin != f"ace-{milestone}":
        raise ValidationError(f"{path}: benchmark_contract_origin must be 'ace-{milestone}'")
    if "results" in path.parts and not path.name.startswith(f"benchmark-{milestone}-"):
        raise ValidationError(f"{path}: result file name must start with 'benchmark-{milestone}-'")
    if not isinstance(data["workloads"], list):
        raise ValidationError(f"{path}: workloads must be an array")
    return scope


def _validate_environment(path: pathlib.Path, data: dict, keys: tuple[str, ...]) -> None:
    """Require ``keys`` in the environment object."""
    environment = data.get("environment")
    if not isinstance(environment, dict):
        raise ValidationError(f"{path}: environment must be an object")
    for key in keys:
        if key not in environment:
            raise ValidationError(f"{path}: environment missing {key}")


def _validate_21(path: pathlib.Path, data: dict) -> None:
    """Schema-2.1 additions: methodology block and complete stable_timing objects."""
    _validate_environment(path, data, ENVIRONMENT_21)
    methodology = data.get("benchmark_methodology")
    if not isinstance(methodology, dict):
        raise ValidationError(f"{path}: schema 2.1 requires benchmark_methodology")
    for key in METHODOLOGY_21:
        if key not in methodology:
            raise ValidationError(f"{path}: benchmark_methodology missing {key}")
    for where, obj in _timing_objects(data["workloads"], "workloads"):
        stable = obj.get("stable_timing")
        if not isinstance(stable, dict):
            raise ValidationError(f"{path}: {where} lacks stable_timing")
        for key in STABLE_TIMING_21:
            if key not in stable:
                raise ValidationError(f"{path}: {where}.stable_timing missing {key}")
        if len(obj["samples_ns"]) != stable["batches"] * stable["samples_per_batch"]:
            raise ValidationError(f"{path}: {where} sample count != batches x samples_per_batch")
        if obj["median_ns"] != stable["median_of_medians_ns"]:
            raise ValidationError(f"{path}: {where} median_ns must equal median_of_medians_ns")


def _validate_22(path: pathlib.Path, data: dict) -> None:
    """Schema-2.2 additions: build revision, configuration fingerprint, input fingerprints."""
    build = data["environment"].get("build")
    if not isinstance(build, dict) or any(key not in build for key in BUILD_22):
        raise ValidationError(f"{path}: schema 2.2 requires environment.build.{BUILD_22}")
    if build["git_dirty"] not in ("true", "false", "unknown"):
        raise ValidationError(f"{path}: git_dirty must be 'true', 'false' or 'unknown'")
    configuration = data.get("configuration")
    if not isinstance(configuration, dict) or any(key not in configuration for key in CONFIGURATION_22):
        raise ValidationError(f"{path}: schema 2.2 requires configuration.{CONFIGURATION_22}")
    fingerprints = data.get("workload_fingerprints")
    if not isinstance(fingerprints, dict):
        raise ValidationError(f"{path}: schema 2.2 requires workload_fingerprints")
    for key, digest in fingerprints.items():
        if "@" not in key or not isinstance(digest, str) or len(digest) != 64 \
                or any(c not in "0123456789abcdef" for c in digest):
            raise ValidationError(f"{path}: invalid workload fingerprint {key!r}")


def validate(path: pathlib.Path) -> str:
    """Validate one document and return its schema version."""
    data = json.loads(path.read_text())
    scope = _validate_identity(path, data)
    schema = str(data["schema_version"])
    if schema not in ("2.0", "2.1", "2.2"):
        raise ValidationError(f"{path}: unsupported schema_version {schema!r}")
    if scope not in ("regression", "ab", "repeatability"):
        _validate_environment(path, data, ENVIRONMENT_20)
        if schema in ("2.1", "2.2"):
            _validate_21(path, data)
        if schema == "2.2":
            _validate_22(path, data)
    return schema


def main(argv: list[str]) -> int:
    """Validate every file; exit 1 on the first violation."""
    if len(argv) < 2:
        print(__doc__.strip().splitlines()[-1], file=sys.stderr)
        return 2
    try:
        for raw in argv[1:]:
            schema = validate(pathlib.Path(raw))
            print(f"valid benchmark JSON (schema {schema}): {raw}")
    except ValidationError as error:
        print(f"INVALID: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
