# Adaptive Compression Engine (ACE) 0.5.0

ACE is a schema-free, adaptive block compressor written in Rust. Every block of the input is
analysed, routed (Generic / NumericGeneral / NumericFast / FloatGeneral / FloatFast), planned
by a deterministic cost model and stored in the self-describing **ACE Format 1.3 / 1.4**
container, which carries a block index for random access.

**0.5.0 adds lossless floating-point and time-series compression**: the TS1 codec (Gorilla
XOR for f64 / f32, RunDelta for rarely changing values) and Planner V5 with its Float lane.
Data the 0.4 pipeline already handles is written byte-for-byte as in 0.4.6 (Format 1.3); only
files with a TS1 block are Format 1.4. See [`docs/RELEASE-NOTES-0.5.0.md`](docs/RELEASE-NOTES-0.5.0.md)
and [`docs/MIGRATION-0.4-TO-0.5.md`](docs/MIGRATION-0.4-TO-0.5.md).

## Features

- **Adaptive planning** — Planner V5 (= V4.3 + Float lane) evaluates RAW, RLE, LZ, NUM1, TS1 and
  delta pipelines with Huffman / rANS / rANS4x entropy coding per block; quality is gated
  against a policy oracle.
- **Numeric compression** — NUM1 (frame-of-reference, delta, delta-of-delta + bit packing) for
  u16 / u32 / u64 lanes, with a validated fixed-step fast path.
- **Floating-point & time series (0.5)** — TS1: Gorilla XOR (bit-exact f64 / f32, incl. NaN
  payloads and −0.0) and RunDelta (Elias-gamma runs + ZigZag deltas); a float prefilter with
  zero false positives on the 0.4 corpus; FloatFast for dominant series (constant f64:
  2 416× vs 141× in 0.4.6, ~35× faster encode).
- **Random access** — `AceIndexedDecoder` decodes only the blocks that intersect a byte range.
- **Streaming** — bounded-memory encoder whose output is byte-identical to in-memory compression.
- **Determinism** — identical bytes for every thread count, process and SIMD backend.
- **Safety** — `unsafe` only in audited SIMD/CRC kernels, no `unwrap`/`expect`/`panic` in
  production code, decoder resource limits, fuzzed parsers.
- **Hardware acceleration** — AVX2 scans and 3-way SSE4.2 CRC32C, runtime-detected; portable
  scalar fallback (`ACE_SIMD=scalar` forces it).

## Architecture

```text
input ─► chunker ─► route (NumericFast | FloatFast | FloatGeneral | NumericGeneral | Generic)
                                   │
                                   ▼
          Planner V5 = V4.3 (analysis, cost model, sampling, policies) + TS1 policy
                                   │
     transforms ─► codec (RAW/RLE/LZ/NUM1/TS1) ─► entropy (Huffman/rANS/rANS4x; none for TS1)
                                   │
                                   ▼
   Format 1.3 | 1.4 container: header · blocks (CRC32C) · index · trailer
   (1.4 only when a block is TS1)
```

| Crate | Role |
|---|---|
| `ace-core` | shared model: ids, plans, configuration, limits, errors, statistics |
| `ace-simd` | runtime-dispatched SIMD scans and hardware CRC32C (the only `unsafe` crate) |
| `ace-bitpack` | generic `Lane` integer transforms, bit packing, public LSB-first bitstream |
| `ace-analysis` | block statistics, numeric detection, float / run prefilters, block-size advisor |
| `ace-transforms`, `ace-codecs`, `ace-entropy` | reversible stages: delta; RAW/RLE/LZ/NUM1/TS1; Huffman/rANS/rANS4x |
| `ace-cost`, `ace-planner` | estimation, sampling, routing, evaluation and policies |
| `ace-format`, `ace-index` | container format, checksums, block I/O, index |
| `ace-engine`, `ace-stream`, `ace-runtime` | façade, block pipeline, random access, streaming, worker pool |
| `ace-dictionary` | dictionary registry seam |
| `ace-cli` | the `ace` command-line tool |
| `ace-corpus` | deterministic synthetic workloads: Corpus V3 + V4 and the Float false-positive corpus |
| `ace-bench` | Benchmark Harness V3 (`benchmark-0.5.0-<family>.json`, schema 2.1) |

Details: [`docs/ARCHITECTURE-0.5.0.md`](docs/ARCHITECTURE-0.5.0.md),
[`docs/PLANNER-V5.md`](docs/PLANNER-V5.md), [`docs/TS1-CODEC.md`](docs/TS1-CODEC.md),
[`docs/FORMAT-1.4.md`](docs/FORMAT-1.4.md),
[`docs/FLOAT-CALIBRATION-0.5.0.md`](docs/FLOAT-CALIBRATION-0.5.0.md).

## Quick start

```bash
./ace-build0.5.0.sh                      # fmt, clippy, tests, release build, rustdoc, demo
./demo/ace-run-demo0.5.0.sh              # < 1 minute product tour (incl. the Float lane)
```

Requirements: Rust 1.97.0 (MSRV, pinned by `rust-toolchain.toml`; `Cargo.lock` included), Python 3.9+ for the tools.

## Command-line interface

```bash
ace compress in.bin out.ace --profile balanced --threads 0   # fast | balanced | dense
ace compress in.bin out.ace --disable-float                  # 0.4.6 decisions, always Format 1.3
ace compress-stream in.bin out.ace                           # bounded memory
ace decompress out.ace restored.bin
ace verify out.ace                                           # decode + check every CRC
ace inspect out.ace --blocks                                 # format version, per-block codec / NUM1 / TS1 telemetry
ace explain in.bin                                           # route, float evidence, planner decisions
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

| Item | 0.5.0 |
|---|---|
| Writer / readers | Format 1.3, or 1.4 when a block is TS1 / 1.0 – 1.4 |
| Planner | V5 = V4.3 + Float lane (golden 0.5.0; Corpus V3 = frozen golden 0.4.6) |
| Encoded bytes | Corpus V3 and every file written with `--disable-float`: identical to 0.4.6 |
| Public API | streaming sink `Write + Seek`; new `AceConfig` field; new enum variants ([migration](docs/MIGRATION-0.4-TO-0.5.md)) |
| MSRV | Rust 1.97 |
| Benchmark schema | 2.1 (all 2.0 fields kept) |

## Performance

Generated tables: [`docs/PERFORMANCE-0.5.0.md`](docs/PERFORMANCE-0.5.0.md). Methodology
(adaptive iterations, 3 × 7 batches, median-of-medians, MAD, interleaved A/B, Float lane
families and same-run gates):
[`docs/BENCHMARK-METHODOLOGY-0.5.0.md`](docs/BENCHMARK-METHODOLOGY-0.5.0.md).

```bash
./ace-benchmark0.5.0.sh all [--quick] [--isolated]   # Harness V3 + Regression V3
./ace-ab0.5.0.sh /path/to/ace-0.4.6                  # interleaved A/B vs the baseline tree
./ace-benchmark-compare0.5.0.sh OLD NEW              # any two result files or directories
./ace-release0.5.0.sh /path/to/ace-0.4.6             # full release pipeline
```

## Security limits

Decoding enforces `DecodeLimits` (output size, block size, encoded block size, transforms,
dictionary size, index entries) before allocating; every block carries a CRC32C; header,
index and trailer are checksummed. Malformed input is rejected with an error, never a panic —
covered by the malformed matrix (incl. a Format 1.4 / TS1 fixture), property tests and 15
fuzz targets. TS1 headers are validated before any output is allocated. See
[`docs/SECURITY.md`](docs/SECURITY.md), [`docs/UNSAFE-AUDIT-0.5.0.md`](docs/UNSAFE-AUDIT-0.5.0.md),
[`docs/PANIC-AUDIT-0.5.0.md`](docs/PANIC-AUDIT-0.5.0.md).

## Repository map

| Path | Content |
|---|---|
| `ace-*0.5.0.sh` | build, benchmark, compare, A/B, release and CI entry points |
| `demo/` | product demo |
| `tools/` | Python benchmark tools (validator, report, compare, Regression V3, A/B, package, audit) |
| `examples/golden/0.5.0/` | golden SHA-256 of Corpus V3 + V4 (with the declared format version) |
| `examples/golden/0.4.6/` | frozen: Corpus V3 must still produce these bytes |
| `examples/baselines/` | accepted reference results (`BASELINE.json` per version; 0.4.6 is the A/B base of 0.5.0, 0.5.0 = Ryzen 9 5950X reference run) |
| `fuzz/` | `cargo-fuzz` project (15 targets) |
| `integrations/` | Java / Scala CLI-boundary examples |
| `docs/` | current documentation; `docs/history/` keeps earlier milestones |
| `TASKS-0.5.0.md`, `MILESTONE-0.5.0.json` | implementation order and machine-readable summary |

## Roadmap

Next: ACE 0.6 — decimal floats (ALP), small-delta RunDelta admission and Patched FOR,
SIMD bit packing / Gorilla. See [`ROADMAP.md`](ROADMAP.md); history of all builds:
[`CHANGELOG.md`](CHANGELOG.md).

## License

Apache-2.0 (see `LICENSE`).
