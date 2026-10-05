#!/usr/bin/env python3
"""Generate deterministic ACE 0.3.1 Corpus V2 files without external data sources."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def xorshift_bytes(size: int) -> bytes:
    """Return deterministic high-entropy bytes from a fixed xorshift32 seed."""
    out = bytearray()
    x = 0x9E3779B9
    while len(out) < size:
        x ^= (x << 13) & 0xFFFFFFFF
        x ^= x >> 17
        x ^= (x << 5) & 0xFFFFFFFF
        out.append(x & 0xFF)
    return bytes(out)


def repeat_to_size(pattern: bytes, size: int) -> bytes:
    """Repeat `pattern` until exactly `size` bytes have been produced."""
    if not pattern:
        return bytes(size)
    repeats = (size + len(pattern) - 1) // len(pattern)
    return (pattern * repeats)[:size]


def corpus_bytes(kind: str, size: int) -> bytes:
    """Return one deterministic Corpus V2 workload."""
    if kind == "zeros":
        return bytes(size)
    if kind == "low-cardinality":
        return repeat_to_size(bytes([0, 1, 2, 3]), size)
    if kind == "runs":
        out = bytearray()
        value = 0
        while len(out) < size:
            out.extend(bytes([value]) * min(4096, size - len(out)))
            value = (value + 17) & 0xFF
        return bytes(out)
    if kind == "numeric-u32":
        out = bytearray()
        value = 10_000
        while len(out) < size:
            value = (value + 3) & 0xFFFFFFFF
            out.extend(value.to_bytes(4, "little"))
        return bytes(out[:size])
    if kind == "delta-series":
        out = bytearray()
        value = 1_000_000
        while len(out) < size:
            value += (len(out) % 7) + 1
            out.extend(value.to_bytes(8, "little"))
        return bytes(out[:size])
    if kind == "structured-json":
        return repeat_to_size(
            b'{"service":"ace","status":"ACTIVE","region":"eu","value":123456}\n',
            size,
        )
    if kind == "random":
        return xorshift_bytes(size)
    if kind == "mixed":
        quarter = size // 4
        parts = [
            corpus_bytes("zeros", quarter),
            corpus_bytes("numeric-u32", quarter),
            corpus_bytes("structured-json", quarter),
            corpus_bytes("random", size - quarter * 3),
        ]
        return b"".join(parts)
    raise ValueError(f"unknown corpus kind: {kind}")


def main() -> None:
    """Generate every requested size/class and write a machine-readable manifest."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--output-dir", default="examples/corpus/generated")
    parser.add_argument("--sizes-mib", default="1,16,64")
    args = parser.parse_args()

    output = Path(args.output_dir)
    output.mkdir(parents=True, exist_ok=True)
    sizes = [int(value) for value in args.sizes_mib.split(",") if value]
    kinds = [
        "zeros",
        "low-cardinality",
        "runs",
        "numeric-u32",
        "delta-series",
        "structured-json",
        "random",
        "mixed",
    ]

    manifest = []
    for mib in sizes:
        size = mib * 1024 * 1024
        for kind in kinds:
            path = output / f"{kind}-{mib}m.bin"
            payload = corpus_bytes(kind, size)
            path.write_bytes(payload)
            manifest.append(
                {
                    "corpus_id": kind,
                    "mib": mib,
                    "bytes": len(payload),
                    "path": str(path),
                }
            )
            print(path)

    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")


if __name__ == "__main__":
    main()
