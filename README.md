# Adaptive Compression Engine (ACE) 0.4.5-buildfix2

Schema-free adaptive block compression in Rust: every block is analysed, routed (Generic /
NumericGeneral / NumericFast), planned by a deterministic cost model and stored in the
self-describing ACE Format 1.3 container with a random-access index.

## What is new in 0.4.5-buildfix2

**Structure.** Every crate's `lib.rs` (and every `mod.rs`) now contains only `mod` and
`pub use`; implementation is split into one module per responsibility. Duplicated code was
replaced by single generic implementations (e.g. one `Lane` trait instead of three copies of
every bit-packing transform, one `AceWriter` for in-memory and streaming containers, one block
reader for sequential and indexed decoding). See `docs/ARCHITECTURE-0.4.5-BUILDFIX2.md`.

**Decode hot path** (closes the warm-64K gate that failed in 0.4.5-buildfix1):

- CRC32C with three interleaved SSE4.2 chains (`ace_simd::crc32c_hardware`): 256 KiB 41 → 12 µs;
- table-driven Huffman decoder with a 64-bit bit buffer;
- fused NUM1 decoding without intermediate vectors (1.6–2× faster);
- RAW blocks are copied once instead of twice.

Container A/B (same machine, same harness): warm-64K range read 78 → 40 µs, full decompression
of `mixed_16m` +30 %, FAST/BALANCED/DENSE decode +14…+43 %, numeric encode +11…+60 %.

**Compatibility.** Format, planner decisions and every encoded byte are unchanged (verified
byte-for-byte against 0.4.5-buildfix1 on Corpus V3 for FAST/BALANCED/DENSE and streaming).
Removed public API: the width-specific `ace_bitpack::*_u16/_u32/_u64` functions (use the
generic `Lane` functions).

Benchmark analysis of 0.4.5-buildfix1: `docs/BENCHMARK-ANALYSIS-0.4.5-buildfix1.md`.

## Quick start

```bash
./ace-build0.4.5-buildfix2.sh                  # check, test, release build, demo, benchmarks
cargo run --release -p ace-cli -- compress in.bin out.ace --profile balanced
cargo run --release -p ace-cli -- inspect out.ace --blocks
cargo run --release -p ace-cli -- read-range out.ace 1000 65536 part.bin
./ace-benchmark0.4.5-buildfix2.sh all          # full benchmark contract + release gates
```

## Workspace

| Crate | Role |
|---|---|
| `ace-core` | shared model: ids, plans, config, limits, errors, statistics |
| `ace-simd` | runtime-dispatched SIMD scans and hardware CRC32C (only crate with `unsafe`) |
| `ace-bitpack` | generic `Lane` integer transforms and bit packing |
| `ace-analysis` | block statistics, numeric detection, prefilter, block-size advisor |
| `ace-transforms` / `ace-codecs` / `ace-entropy` | reversible pipeline stages (delta; RAW/RLE/LZ/NUM1; Huffman/rANS/rANS4x) |
| `ace-cost` / `ace-planner` | estimation, sampling, Planner V4.3 routing, evaluation and policies |
| `ace-format` / `ace-index` | container format, checksums, block I/O, index |
| `ace-engine` / `ace-stream` / `ace-runtime` | façade, block encoder/pipeline, streaming, parallel runtime |
| `ace-dictionary` | dictionary registry seam (training deferred) |
| `ace-cli` | `ace` command-line tool |

## Planner V4.3 background (0.4-buildfix4)

ACE 0.4-buildfix4 is the **Planner V4.3 Policy Oracle Closure & Runtime Regression Fix** release.

It does not add a new compression algorithm or wire format. It closes the remaining 0.4 policy
semantics and removes duplicate work from the NumericFast hot path.

#### One planning context per block

`PlanningContext` classifies a block once. Its `RouteDecision` is reused by engine and evaluator.
A validated NumericFast route carries `NumericFastEvidence`, so the block is not validated twice.

#### Strict NumericFast invariant

NumericFast now requires a complete-block fixed-step sequence:

- monotonically non-decreasing;
- non-zero first delta;
- every subsequent delta exactly equals the first delta.

Outlier/sawtooth workloads fall back to NumericGeneral.

#### Direct NumericFast encode

`numeric_encode_fixed_step` writes `NUM1 + DeltaOfDelta + bit_width=0` directly from evidence.
It avoids numeric mode search, delta/DoD vectors, bit-width scan and bit packing.

#### Three oracle levels

- **global oracle** — smallest physically possible diagnostic payload;
- **route oracle** — smallest route-eligible payload;
- **policy oracle** — candidate ACE should prefer under route, dominance, access and size-envelope rules.

Only policy oracle drives release recall/regret gates.

#### Product dominance policy

`DominancePolicy` introduces product preferences:

- RLE preferred for zero/run-heavy blocks;
- RAW preferred for incompressible data;
- Numeric preferred for NumericFast/NumericGeneral;
- RandomAccess can prefer cheaper-decode RAW/RLE.

Preference is bounded by `DominanceEnvelope` so decode preference cannot hide an excessive size loss.

## Compatibility

- workspace version: 0.4.5 (milestone 0.4.5-buildfix2)
- Planner: V4.3
- writer: Format 1.3; readers: 1.0 / 1.1 / 1.2 / 1.3
- NUM1 lane width 2 (u16) since 0.4.5; AIDX/ACET unchanged
- MSRV: Rust 1.75 (`clippy.toml`)
- benchmark schema: 2.0

## Versioned scripts

Every script starts with `ace-` and carries the milestone in its filename:

```text
ace-build0.4.5-buildfix2.sh
ace-benchmark0.4.5-buildfix2.sh
ace-benchmark-compare0.4.5-buildfix2.sh
demo/ace-run-demo0.4.5-buildfix2.sh
tools/*0.4.5-buildfix2.py
```

## Benchmark JSON naming

```text
examples/results/benchmark-0.4.5-buildfix2-<family>.json
```

The milestone tag is defined once (`examples/rust-benchmark/src/json.rs::MILESTONE`).
Previous results for A/B comparisons: `examples/baselines/0.4.5-buildfix1/`.

```bash
./ace-benchmark-compare0.4.5-buildfix2.sh \
  examples/baselines/0.4.5-buildfix1/benchmark-0.4.5-buildfix1-numeric.json \
  examples/results/benchmark-0.4.5-buildfix2-numeric.json
```

## Release gates

- policy candidate recall >= 0.99
- policy Top-K recall >= 0.98
- policy mean regret <= 16 B/block
- policy p95 <= 64 B
- policy p99 <= 256 B
- full candidate trials == 0
- BALANCED/DENSE ratio >= 3.70x
- FAST/BALANCED/DENSE throughput >= 95% buildfix2
- warm64K <= 110% buildfix2
- u32 NumericFast >= 95% buildfix2
- u64 timestamps >= 90 MB/s
- delta-variable >= 75 MB/s

Global and route regret remain diagnostic.

## Documentation

- `TASKS-0.4.5-buildfix2.md` — ordered implementation tasks (types, traits, modules)
- `MILESTONE-0.4.5-buildfix2.json` — machine-readable release summary
- `docs/ARCHITECTURE-0.4.5-BUILDFIX2.md` — module map and SOLID/KISS/DRY decisions
- `docs/BENCHMARK-ANALYSIS-0.4.5-buildfix1.md` — detailed analysis of the previous run
- `docs/NUMERIC-0.4.5.md`, `docs/CONCEPT-TRACEABILITY-0.4.md`, `docs/FORMAT-1.3.md`
- `docs/ARCHITECTURE-0.4-BUILDFIX4.md`, `docs/POLICY-ORACLE-0.4-BUILDFIX4.md`
- `docs/history/` — task lists, milestone files and audits of earlier builds
- `CHANGELOG.md`, `ROADMAP.md`
