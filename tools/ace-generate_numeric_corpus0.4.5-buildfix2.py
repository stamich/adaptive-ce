#!/usr/bin/env python3
"""Generate deterministic ACE 0.4 numeric/time-series Corpus V3 workloads."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def u32_values(kind: str, count: int) -> list[int]:
    """Generate one deterministic u32 sequence used by numeric codec benchmarks."""
    if kind == "u32-counter":
        value = 10_000
        out = []
        for _ in range(count):
            value = (value + 3) & 0xFFFFFFFF
            out.append(value)
        return out
    if kind == "gauge-sawtooth":
        return [100_000 + (i % 4096) for i in range(count)]
    if kind == "monotonic-outliers":
        value = 1_000_000
        out = []
        for i in range(count):
            value = (value + (100_000 if i % 1024 == 0 else 1)) & 0xFFFFFFFF
            out.append(value)
        return out
    if kind == "delta-variable":
        value = 100_000
        out = []
        for i in range(count):
            value = (value + (i % 17) + 1) & 0xFFFFFFFF
            out.append(value)
        return out
    raise ValueError(f"unknown u32 corpus kind: {kind}")


def u64_values(kind: str, count: int) -> list[int]:
    """Generate one deterministic u64 sequence used by timestamp benchmarks."""
    if kind == "u64-timestamps-ms":
        value = 1_780_000_000_000
        out = []
        for i in range(count):
            value = (value + 1000 + (i % 3)) & 0xFFFFFFFFFFFFFFFF
            out.append(value)
        return out
    if kind == "u64-timestamps-ns":
        value = 1_780_000_000_000_000_000
        out = []
        for i in range(count):
            value = (value + 1_000_000 + (i % 5) * 100) & 0xFFFFFFFFFFFFFFFF
            out.append(value)
        return out
    raise ValueError(f"unknown u64 corpus kind: {kind}")


def encode_values(values: list[int], width: int, target_bytes: int) -> bytes:
    """Serialize little-endian values and trim to exactly `target_bytes`."""
    payload = bytearray()
    mask = (1 << (width * 8)) - 1
    for value in values:
        payload.extend((value & mask).to_bytes(width, "little"))
    if len(payload) < target_bytes:
        raise ValueError("generated sequence shorter than requested target")
    return bytes(payload[:target_bytes])


def main() -> None:
    """Generate all Corpus V3 workloads and a machine-readable manifest."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--output-dir", default="examples/corpus/numeric")
    parser.add_argument("--sizes-mib", default="1,16,64")
    args = parser.parse_args()

    output = Path(args.output_dir)
    output.mkdir(parents=True, exist_ok=True)
    sizes = [int(value) for value in args.sizes_mib.split(",") if value]
    manifest: list[dict[str, object]] = []

    u32_kinds = ["u32-counter", "gauge-sawtooth", "monotonic-outliers", "delta-variable"]
    u64_kinds = ["u64-timestamps-ms", "u64-timestamps-ns"]
    for mib in sizes:
        target = mib * 1024 * 1024
        for kind in u32_kinds:
            count = (target + 3) // 4
            payload = encode_values(u32_values(kind, count), 4, target)
            path = output / f"{kind}-{mib}m.bin"
            path.write_bytes(payload)
            manifest.append({"corpus_id": kind, "width": 32, "bytes": len(payload), "path": str(path)})
            print(path)
        for kind in u64_kinds:
            count = (target + 7) // 8
            payload = encode_values(u64_values(kind, count), 8, target)
            path = output / f"{kind}-{mib}m.bin"
            path.write_bytes(payload)
            manifest.append({"corpus_id": kind, "width": 64, "bytes": len(payload), "path": str(path)})
            print(path)

    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")


if __name__ == "__main__":
    main()
