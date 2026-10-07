# Adaptive Compression Engine (ACE) 0.4.6

ACE is a schema-free, adaptive block compressor written in Rust. Every block of the input is
analysed, routed (Generic / NumericGeneral / NumericFast), planned by a deterministic cost
model and stored in the self-describing **ACE Format 1.3** container, which carries a block
index for random access.

**0.4.6 is the hardened release of the 0.4 line.** It changes no format, no planner decision
and no encoded byte (enforced by golden SHA-256 files); it makes the 0.4 architecture
measurable, reproducible and auditable. See [`docs/RELEASE-NOTES-0.4.6.md`](docs/RELEASE-NOTES-0.4.6.md).

## Features

- **Adaptive planning** — Planner V4.3 evaluates RAW, RLE, LZ, NUM1 and delta pipelines with
  Huffman / rANS / rANS4x entropy coding per block; quality is gated against a policy oracle.
- **Numeric compression** — NUM1 (frame-of-reference, delta, delta-of-delta + bit packing) for
  u16 / u32 / u64 lanes, with a validated fixed-step fast path.
- **Random access** — `AceIndexedDecoder` decodes only the blocks that intersect a byte range.
- **Streaming** — bounded-memory encoder whose output is byte-identical to in-memory compression.
- **Determinism** — identical bytes for every thread count, process and SIMD backend.
- **Safety** — `unsafe` only in audited SIMD/CRC kernels, no `unwrap`/`expect`/`panic` in
  production code, decoder resource limits, fuzzed parsers.
- **Hardware acceleration** — AVX2 scans and 3-way SSE4.2 CRC32C, runtime-detected; portable
  scalar fallback (`ACE_SIMD=scalar` forces it).

## Architecture

```text
input ─► chunker ─► analysis ─► route (Generic | NumericGeneral | NumericFast)
                                   │
                                   ▼
                    Planner V4.3 (cost model, sampling, policies)
                                   │
              transforms ─► codec (RAW/RLE/LZ/NUM1) ─► entropy (Huffman/rANS/rANS4x)
                                   │
                                   ▼
         Format 1.3 container: header · blocks (CRC32C) · index · trailer
```

| Crate | Role |
|---|---|
| `ace-core` | shared model: ids, plans, configuration, limits, errors, statistics |
| `ace-simd` | runtime-dispatched SIMD scans and hardware CRC32C (the only `unsafe` crate) |
| `ace-bitpack` | generic `Lane` integer transforms and bit packing |
| `ace-analysis` | block statistics, numeric detection, block-size advisor |
| `ace-transforms`, `ace-codecs`, `ace-entropy` | reversible stages: delta; RAW/RLE/LZ/NUM1; Huffman/rANS/rANS4x |
| `ace-cost`, `ace-planner` | estimation, sampling, routing, evaluation and policies |
| `ace-format`, `ace-index` | container format, checksums, block I/O, index |
| `ace-engine`, `ace-stream`, `ace-runtime` | façade, block pipeline, random access, streaming, worker pool |
| `ace-dictionary` | dictionary registry seam |
| `ace-cli` | the `ace` command-line tool |
| `ace-corpus` | deterministic synthetic workloads (tests, golden files, demo, benchmarks) |
| `ace-bench` | Benchmark Harness V3 (`benchmark-0.4.6-<family>.json`, schema 2.1) |

Details: [`docs/ARCHITECTURE-0.4.6.md`](docs/ARCHITECTURE-0.4.6.md),
[`docs/ARCHITECTURE-FREEZE-0.4.6.md`](docs/ARCHITECTURE-FREEZE-0.4.6.md).

## Quick start

```bash
./ace-build0.4.6.sh                      # fmt, clippy, tests, release build, rustdoc, demo
./demo/ace-run-demo0.4.6.sh              # < 1 minute product tour
```

Requirements: Rust 1.97.0 (MSRV, pinned by `rust-toolchain.toml`; `Cargo.lock` included), Python 3.9+ for the tools.

## Command-line interface

```bash
ace compress in.bin out.ace --profile balanced --threads 0   # fast | balanced | dense
ace compress-stream in.bin out.ace                           # bounded memory
ace decompress out.ace restored.bin
ace verify out.ace                                           # decode + check every CRC
ace inspect out.ace --blocks                                 # per-block codec / numeric telemetry
ace explain in.bin                                           # planner decisions, no output file
ace read-range out.ace 1000000 65536 part.bin                # random access
ace decode-block out.ace 7 block.bin
```

## Library API

```rust
use std::io::Cursor;
use ace_core::{AceConfig, CompressionProfile, DecodeLimits};
use ace_engine::{AceEngine, AceIndexedDecoder};

let engine = AceEngine::new(AceConfig { profile: CompressionProfile::Balanced, ..AceConfig::default() })?;
let packed = engine.compress(&input)?;
let restored = engine.decompress(&packed)?;              // or decompress_into(&packed, &mut buf)
let mut reader = AceIndexedDecoder::open(Cursor::new(&packed), DecodeLimits::default())?;
let slice = reader.read_range(4_000..4_100)?;
```

The same example is compiled and executed as a doctest of `ace-engine`.

## Compatibility

| Item | 0.4.6 |
|---|---|
| Writer / readers | Format 1.3 / 1.0, 1.1, 1.2, 1.3 |
| Planner | V4.3 (decisions frozen, golden SHA-256) |
| Encoded bytes | identical to 0.4.5-buildfix2 (and to 0.4.5-buildfix1) |
| Public API | additive only: `AceEngine::decompress_into`, `PREALLOCATION_CAP_BYTES` |
| MSRV | Rust 1.97 |
| Benchmark schema | 2.1 (all 2.0 fields kept) |

## Performance

Generated tables: [`docs/PERFORMANCE-0.4.6.md`](docs/PERFORMANCE-0.4.6.md). Methodology
(adaptive iterations, 3 × 7 batches, median-of-medians, MAD, interleaved A/B):
[`docs/BENCHMARK-METHODOLOGY-0.4.6.md`](docs/BENCHMARK-METHODOLOGY-0.4.6.md).

```bash
./ace-benchmark0.4.6.sh all [--quick] [--isolated]   # Harness V3 + Regression V3
./ace-ab0.4.6.sh /path/to/ace-0.4.5-buildfix2        # interleaved A/B vs the baseline tree
./ace-benchmark-compare0.4.6.sh OLD NEW              # any two result files or directories
./ace-release0.4.6.sh /path/to/ace-0.4.5-buildfix2   # full release pipeline
```

## Security limits

Decoding enforces `DecodeLimits` (output size, block size, encoded block size, transforms,
dictionary size, index entries) before allocating; every block carries a CRC32C; header,
index and trailer are checksummed. Malformed input is rejected with an error, never a panic —
covered by the malformed matrix, property tests and 12 fuzz targets. See
[`docs/SECURITY.md`](docs/SECURITY.md), [`docs/UNSAFE-AUDIT-0.4.6.md`](docs/UNSAFE-AUDIT-0.4.6.md),
[`docs/PANIC-AUDIT-0.4.6.md`](docs/PANIC-AUDIT-0.4.6.md).

## Repository map

| Path | Content |
|---|---|
| `ace-*0.4.6.sh` | build, benchmark, compare, A/B, release and CI entry points |
| `demo/` | product demo |
| `tools/` | Python benchmark tools (validator, report, compare, Regression V3, A/B, package, audit) |
| `examples/golden/0.4.6/` | semantic-freeze golden SHA-256 file |
| `examples/baselines/` | accepted reference results (`BASELINE.json` per version) |
| `fuzz/` | `cargo-fuzz` project (12 targets) |
| `integrations/` | Java / Scala CLI-boundary examples |
| `docs/` | current documentation; `docs/history/` keeps earlier milestones |
| `TASKS-0.4.6.md`, `MILESTONE-0.4.6.json` | implementation order and machine-readable summary |

## Roadmap

0.4.6 closes the 0.4 line. Next: ACE 0.5 — advanced time-series and numeric compression
(Gorilla/XOR floats, decimal specialisation, Patched FOR, SIMD bit packing). See
[`ROADMAP.md`](ROADMAP.md); history of all builds: [`CHANGELOG.md`](CHANGELOG.md).

## License

Apache-2.0 (see `LICENSE`).
