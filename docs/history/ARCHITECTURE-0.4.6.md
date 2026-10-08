# ACE 0.4.6 — Architecture

Format 1.3, Planner V4.3 and every encoded byte are frozen
([`ARCHITECTURE-FREEZE-0.4.6.md`](ARCHITECTURE-FREEZE-0.4.6.md)). The structure rules of
0.4.5-buildfix2 remain; 0.4.6 adds the shared corpus crate, the Benchmark Harness V3 crate,
a pre-allocated decode API and the hardening/audit tooling.

## 1. Crate layout rule

Every `lib.rs` and every `mod.rs` contains only crate documentation, `mod` declarations and
`pub use` re-exports; binaries keep `main.rs` to module declarations and the entry point.
Implementation lives in one file per responsibility. Every public and crate-visible item has
rustdoc (`missing_docs = deny`, `RUSTDOCFLAGS=-D warnings`).

## 2. Module map

| Crate | Modules (responsibility) |
|---|---|
| `ace-core` | `codec`, `plan`, `stats`, `numeric`, `config`, `limits` (`DecodeLimits`), `error` (`AceError`), `profile`, `block`, `dictionary` |
| `ace-simd` | `backend` (detection, **`ACE_SIMD=scalar` override**), `scan`, `avx2` (unsafe), `crc32c` (3-way SSE4.2 + portable fallback, `crc32c_backend_name`) |
| `ace-bitpack` | `lane` (`Lane` with `WIDTH`), `zigzag`, `width`, `scalar`, `delta`, `delta_of_delta`, `for_codec`, `bit_io` (private, infallible `read_bits_validated`) |
| `ace-analysis` | block statistics, numeric detection / prefilter, block-size advisor |
| `ace-codecs` | `dispatch`, `raw`, `rle`, `lz`, `numeric/` (`mode`, `header`, `estimate`, `encode`, `decode`, `lane_dispatch`) |
| `ace-entropy` | `dispatch`, `huffman/` (`code_lengths`, `canonical`, `bit_io`, `encoder`, `decoder`), `rans/`, `rans4x` |
| `ace-transforms` | `dispatch`, `delta` |
| `ace-format` | `header`, `index`, `checksum`, `block_io`, `reader`, `writer` |
| `ace-index` | `index_reader` |
| `ace-cost` | `estimator`, `cost_model`, `sampling`, `quality` |
| `ace-planner` | `planner`, `route`, `route_policy`, `planning_context`, `evaluator`, `exhaustive`, `pipeline`, `plan_identity`, `decision`, `policy`, `hybrid`, `fastpath` |
| `ace-engine` | `engine` (`AceEngine` façade, **`decompress_into`**, `PREALLOCATION_CAP_BYTES`), `block_encoder`, `block_pipeline`, `container`, `indexed`, `chunker`, `explain` |
| `ace-stream` | `limits`, `encoder`, `decoder` |
| `ace-runtime` | `config` (`RuntimeConfig`), `scratch` (`WorkerScratch`) |
| `ace-dictionary` | `dictionary`, `provider`, `registry` |
| `ace-cli` | `args`, `files`, `commands/*` |
| **`ace-corpus`** (new) | `rng` (xorshift), `workload` (`Workload` enum, names / aliases), `generators`, `bin/ace-corpus` |
| **`ace-bench`** (new home of the harness) | `timing/` (`plan`, `runner`, `stats`, `report`), `environment`, `alloc`, `json`, `corpus`, `plan_util`, `prelude`, `families/` (`release`, `memory`, `compression`, `numeric`, `planner`, `random_access`) |

## 3. Data flow

```text
encode:  FixedBlockChunker ─► PlanningContext::classify (route)
           ─► Planner V4.3 (analysis, candidates, sampling, policy) ─► block_encoder
           ─► AceWriter (header · blocks · index · trailer)            [rayon, ordered output]

decode:  AceReader / AceIndexReader ─► read_serialized_block
           ─► block_pipeline::decode_encoded_block
                ├ split_entropy_metadata ─ decode_entropy_cow ─ decode_codec
                ├ invert_transform*
                └ CRC32C (3-way SSE4.2 or portable)
           ─► AceEngine::decode_blocks (order, size and limit checks) ─► Write sink
```

`decompress`, `decompress_into` and `decompress_from` share `decode_blocks`; only the sink
differs (fresh `Vec`, caller-owned `Vec` with capped reservation, arbitrary `Write`).

## 4. Benchmark Harness V3

```text
families/*  ─measure(|| op)─►  timing::runner  (warm-up, doubling calibration, 3 × 7 samples)
                                   │
                              timing::stats    (MoM, batch MAD, MAD, outliers)
                                   │
                              timing::report   (schema 2.0 fields + stable_timing)
main ─► environment (build.rs fingerprint + runtime snapshots) ─► json::BenchmarkDocument
```

The `measure(|| op)` signature is unchanged, so all families moved to V3 without edits to
their measurement code. `FAMILIES` in `main.rs` is the single registry of family names.

## 5. Principles applied in 0.4.6

| Principle | Example |
|---|---|
| Single responsibility | timing split into plan / runner / stats / report; environment and allocator are separate modules |
| DRY | one corpus generator crate for tests, golden, demo and benchmarks (Python generators removed); one family registry; one shared Python library (`tools/ace-benchlib0.4.6.py`); `decode_blocks` shared by every decode entry point |
| Open/closed | new benchmark families register one line in `FAMILIES`; gate limits are data (`STABILITY_LIMITS`, `CASES`) |
| KISS | the A/B probe repeats a 20-line calibration loop instead of linking the harness into old trees |
| YAGNI | no plugin system for gates, no new format features, cost-model calibration script removed |
| Safety | `unsafe` only in `ace-simd` kernels and the benchmark allocator; panic lints deny in production |
