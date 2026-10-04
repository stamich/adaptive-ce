#!/usr/bin/env python3
"""Generate deterministic ACE demo/benchmark corpora."""
from __future__ import annotations
import argparse
import json
import random
import struct
from pathlib import Path


def mixed_bytes(mebibytes: int) -> bytes:
    """Build a deterministic heterogeneous byte stream with zero, numeric, structured and pseudo-random quarters."""
    target = mebibytes * 1024 * 1024
    quarter = target // 4
    data = bytearray(b"\x00" * quarter)
    while len(data) < quarter * 2:
        data.extend(struct.pack("<I", len(data) // 16))
    record = b'{"status":"ACTIVE","service":"graphnet","region":"eu"}\n'
    while len(data) < quarter * 3:
        data.extend(record)
    x = 0x9E3779B9
    while len(data) < target:
        x ^= (x << 13) & 0xFFFFFFFF
        x ^= x >> 17
        x ^= (x << 5) & 0xFFFFFFFF
        x &= 0xFFFFFFFF
        data.append(x & 0xFF)
    return bytes(data[:target])


def write_default_tree(root: Path) -> None:
    """Create the category-oriented corpus tree used by manual examples."""
    for name in ["text", "numeric", "repetitive", "structured", "random", "mixed"]:
        (root / name).mkdir(parents=True, exist_ok=True)
    (root / "text" / "logs.txt").write_text("service=ace status=ok region=eu\n" * 5000)
    with (root / "numeric" / "monotonic-u32.bin").open("wb") as handle:
        for i in range(100_000): handle.write(struct.pack("<I", i // 4))
    (root / "repetitive" / "zeros.bin").write_bytes(bytes(512 * 1024))
    (root / "structured" / "records.jsonl").write_text("".join(json.dumps({"id": i, "status": "ACTIVE", "service": "graphnet"}) + "\n" for i in range(10_000)))
    rng = random.Random(0xACE021)
    (root / "random" / "deterministic-random.bin").write_bytes(bytes(rng.randrange(256) for _ in range(512 * 1024)))
    parts = [("text","logs.txt"),("numeric","monotonic-u32.bin"),("repetitive","zeros.bin"),("structured","records.jsonl"),("random","deterministic-random.bin")]
    (root / "mixed" / "mixed.bin").write_bytes(b"".join((root / category / name).read_bytes() for category, name in parts))


def main() -> None:
    """Parse CLI options and generate either the standard tree or one fixed-size mixed file."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, help="optional single mixed-output file")
    parser.add_argument("--mib", type=int, default=16, help="size of --output mixed file in MiB")
    args = parser.parse_args()
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_bytes(mixed_bytes(args.mib))
    else:
        write_default_tree(Path(__file__).resolve().parents[1] / "examples" / "data")


if __name__ == "__main__":
    main()
