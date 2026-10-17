# ACE architecture

Canonical description of the current release (0.6.0). Earlier versions of this document:
[`history/`](history/) (`ARCHITECTURE-0.4.6.md`, `ARCHITECTURE-0.5.0.md`).

ACE is a deterministic, block-based lossless compressor. Every block is planned and encoded
independently (so encoding is parallel and decoding is random-access), the container records
which reversible pipeline each block used, and the same input with the same configuration
always produces the same bytes on every host, thread count and SIMD backend.

## 1. Crate layout rule

Every `lib.rs` and `mod.rs` contains only crate documentation, `mod` declarations and
`pub use` re-exports; binaries keep `main.rs` to module declarations and the entry point.
Implementation lives in one file per responsibility. Every item — public, crate-visible and
private — has rustdoc (`missing_docs = deny`; `cargo doc` runs with `-D warnings` for the
public API and with `--document-private-items`). Public fallible functions document
`# Errors`, unsafe functions `# Safety`; planner policies, estimators and encoders document
`# Determinism` / `# Invariants`.

## 2. Crates

| Crate | Kind | Responsibility |
|---|---|---|
| `ace` | public | the only supported entry point: `Engine`, `Config` / `ConfigBuilder`, `DecodeLimits`, `RangeReader`, `Stats`, `Error` / `ErrorKind` (non-exhaustive types over the internal crates; feature `explain` adds `BlockReport`) |
| `ace-core` | internal | model types: `AceConfig`, `DecodeLimits`, `CodecId` / `EntropyCodecId` / `TransformId`, physical plans, `CompressionStats`, `AceError` |
| `ace-engine` | internal | `AceEngine` (compress / decompress / explain), block encoder (Planner V5 `plan_block`), block pipeline, container assembly, `AceIndexedDecoder` |
| `ace-stream` | internal | bounded-memory streaming encoder (`Write + Seek` sink, in-place 1.4 header rewrite) and decoder |
| `ace-format` | internal | Format 1.0 – 1.4: file / block headers, AIDX index, ACET trailer, CRC-32C, `read_serialized_block` |
| `ace-index` | internal | seekable index loading and indexed block access |
| `ace-codecs` | internal | RAW, RLE, LZ, NUM1 (FOR / Delta / DoD + bit packing), TS1 (Gorilla f64 / f32, RunDelta) |
| `ace-entropy` | internal | canonical Huffman, scalar rANS, interleaved rANS4x |
| `ace-transforms` | internal | reversible byte transforms (delta) |
| `ace-bitpack` | internal | lanes, ZigZag, bit widths, bit packing, LSB-first bitstream |
| `ace-analysis` | internal | block statistics, numeric / float / run prefilters, block-size advisor |
| `ace-cost` | internal | analytical estimators, deterministic sampling, quality envelope, TS1 estimators |
| `ace-planner` | internal | routes (Generic, NumericGeneral, NumericFast, FloatGeneral, FloatFast), candidate generation, V4.3 evaluator, TS1 policy, telemetry |
| `ace-runtime` | internal | bounded parallel runtime configuration, worker scratch |
| `ace-simd` | internal | the only production `unsafe`: runtime-dispatched AVX2 scans and SSE4.2 CRC-32C (`ACE_SIMD=scalar` override) |
| `ace-dictionary` | internal | dictionary identity / registry seam (no trained dictionaries yet) |
| `ace-corpus` | internal | deterministic synthetic workloads (Corpus V3 + V4, false-positive corpus) |
| `ace-cli` | binary | the `ace` command |
| `ace-bench` | binary | Benchmark Harness V3 ([`BENCHMARKS.md`](BENCHMARKS.md)) |

The `ace` crate is the API applications use: it wraps the engine, the streaming encoder and the
indexed decoder in non-exhaustive types and one error type. Internal crates (`publish = false`)
may change in any release ([`API-INVENTORY.md`](API-INVENTORY.md)); `ace-cli` uses the facade
for compression and decoding and the internals for `inspect`, `explain` and `version`.

## 3. Data flow

```text
encode:  FixedBlockChunker ─► PlanningContext::classify ─► RoutePolicy::classify
           (V4.3 route ─► float_prefilter / run_prefilter ─► V5 route + evidence)
           ─► plan_block: NumericFast | FloatFast (one TS1 encode, bounded fallback)
                          | analysis + V4.3 pipeline of the base route ─► TS1 policy
           ─► block_encoder (NUM1 / TS1 / generic payload, RAW fallback)
           ─► assemble_container: header with the minimal minor version (1.3 | 1.4)
                                  · blocks · index · trailer          [rayon, ordered output]

decode:  AceReader / AceIndexReader ─► DecodeLimits checks ─► read_serialized_block
           (header CRC, version gates, plain-TS1 rule, sizes ≤ limits before allocation)
           ─► block_pipeline::decode_encoded_block
                ├ split_entropy_metadata ─ decode_entropy_cow ─ decode_codec
                ├ invert_transform*
                └ CRC-32C of the reconstructed block
           ─► AceEngine::decode_blocks ─► Write sink
```

Planner details: [`PLANNER-V5.md`](PLANNER-V5.md). Codec details: [`TS1-CODEC.md`](TS1-CODEC.md),
[`RANS.md`](RANS.md). Container: [`FORMAT.md`](FORMAT.md).

## 4. Decoder hardening

* Every integer read from a file reaches an allocation only through
  `try_from → checked_* → DecodeLimits → allocation`; the parser modules compile with
  `clippy::arithmetic_side_effects` and `clippy::cast_possible_truncation`, so an unchecked
  operation fails the build. Field-by-field evidence: [`DECODER-AUDIT.md`](DECODER-AUDIT.md)
  (generated, verified by `tools/ace-code_audit0.6.0.py`).
* `DecodeLimits` is a contract: the table on the type says which reader applies which limit;
  `crates/ace-bench/tests/limits_before_allocation.rs` measures that a forged size fails
  before a buffer of that size exists.
* Streams must end exactly where the encoder stopped (final rANS state, Huffman padding,
  TS1 bit counts), so a forged size is caught by the codec, not only by the block CRC.
* No panic in production code (`unwrap` / `expect` / `panic` / `unreachable` / `todo` are
  denied); `unsafe` only in `ace-simd` (and the benchmark allocator), each block with a
  `// SAFETY:` proof ([`UNSAFE-AUDIT.md`](UNSAFE-AUDIT.md), [`PANIC-AUDIT.md`](PANIC-AUDIT.md)).

## 5. Guards

| Guard | Protects | Mechanism |
|---|---|---|
| Golden SHA-256 | the writer | `examples/golden/0.6.1/` (Corpus V3 – V5, 31 workloads × 3 profiles, with format version) and the frozen `examples/golden/0.5.0/` and `0.4.6/` (unchanged workloads must match them; the deliberate changes may only shrink); checked with the auto and the scalar SIMD backend |
| Compatibility fixtures | the reader | `examples/compat/` — files written by 0.2.1, 0.3.0, 0.4.4, 0.4.6, 0.5.0 and 0.6.1, never regenerated ([`COMPATIBILITY.md`](COMPATIBILITY.md)) |
| Determinism matrix | thread / API independence | workloads × profiles × 1 – 32 workers × in-flight batch sizes × `compress` / `compress_to` / streaming |
| Planner invariants | Planner V5 rules | property tests: TS1 ⇔ selection + dominance rule, NumericFast without Float evidence, selected ∈ candidates, zero full trial encodes, fixed tie-breaks |
| Malformed input | the decoder | malformed matrix, per-field tests, limits tests, 18 fuzz targets (smoke on every PR, 18 × 10 min per release) |
| Benchmarks | performance | Harness V3, Regression V3, interleaved A/B, three-run repeatability ([`BENCHMARKS.md`](BENCHMARKS.md)) |

## 6. Testing layers

| Layer | Where | Sees |
|---|---|---|
| unit / integration | `crates/<crate>/tests/<module>.rs` | the public items of the crate: one test file per source module |
| facade internals | `crates/ace/tests/` with feature `internal-testing` | the hidden `ace::internal` conversions |
| CLI | `crates/ace-cli/tests/cli.rs` | the `ace` binary: round trips, invalid input, closed pipes |
| golden / compat | `crates/ace-engine/tests/golden.rs`, `compat_fixtures.rs` | bytes on disk |
| fuzz | `fuzz/` | arbitrary input ([`../fuzz/README.md`](../fuzz/README.md)) |

### Test layout

No file under `crates/*/src` contains `#[cfg(test)]` (enforced by `tools/ace-code_audit0.6.1.py`).
A test of `crates/ace-cost/src/quality.rs` lives in `crates/ace-cost/tests/quality.rs` and calls
public items only. When a test needs a private helper, one of three things happens, in this order
of preference: the test restates the helper (a bit reader is ten lines), the helper becomes `pub`
with rustdoc because it is a legitimate low-level API of an internal crate (bit I/O, serializers,
entropy framing), or, for the `ace` facade, the test reaches it through the `internal-testing`
feature, whose `ace::internal` module is `#[doc(hidden)]` and not part of the public API. Test
helpers that unwrap carry `#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]`
because a panic in a test is a failure.

## 7. Principles

| Principle | Example |
|---|---|
| Single responsibility | TS1 split into `mode` / `header` / `sink` / `gamma` / `gorilla` / `run_delta` / `codec`; one `read_serialized_block` for every reader |
| DRY | one bitstream for NUM1 and TS1; one encoder for size and payload (`BitSink`); one decoder-audit table that the audit tool verifies against the tests |
| Open / closed | new routes delegate to V4.3 (`candidate_route()`); TS1 modes 4 – 15 reserved (each new mode is a new format minor) |
| Measure first | thresholds from `float-ablation` ([`FLOAT-CALIBRATION-0.5.0.md`](FLOAT-CALIBRATION-0.5.0.md)); the allocation churn of the generic evaluator is measured (`allocation-profile`) before it is optimized in 0.6 |
| Deterministic by construction | no wall clock, no hash-map iteration order, fixed tie-break keys, ordered parallel output |
