# ACE Roadmap

| Version | Theme | Content | Format |
|---|---|---|---|
| **0.5.1** (done) | Hardened RC | decoder audit, limits contract, compat fixtures, determinism to 32 threads, fuzz smoke in CI, repeatable benchmarks, CLI hardening, canonical docs | 1.3 / 1.4, bytes = 0.5.0 |
| 0.5.x | maintenance | defects only: decoder, security, compatibility, critical regressions | unchanged |
| **0.6.0** | API & Performance (Phase A) | one `ace` facade crate (`Engine`, `Config` and `ConfigBuilder`, `DecodeLimits` with a builder, `RangeReader`, `Stats`, `Error`), internal crates hidden, deprecated items removed, MSRV 1.98.1, readers name the ACE version a newer file needs, decode and allocation recovery, public-corpus benchmark family | 1.3 / 1.4, bytes = 0.5.1 |
| **0.6.1** ✅ | Numeric & Float II (Phase B) | RunDelta / NUM1 for small non-zero deltas; Patched FOR; ALP decimal floats; Chimp / Chimp128 measured at codec level and kept only if at least 5 % more blocks shrink than with the best existing choice; earlier TS1 decision; DENSE never larger than BALANCED (see `docs/BENCHMARKS.md`); Chimp not added (0 of 144 blocks), see `docs/RELEASE-NOTES-0.6.1.md` | **1.5** (new modes) |
| **0.7.0** ✅ | Embedding | `ace-ffi` (C ABI v1, opaque handles, error codes, `catch_unwind` at the boundary, generated `include/ace.h`), Java SDK on Panama (JDK 22+, tested on 22 and 25), Scala 3 SDK (model, Either-based API, streams, tier policy), native packaging in the JAR, `ffi` benchmark gates, native aarch64 CI (scalar kernels). Not in 0.7.0: NEON backend, macOS / Windows native builds | unchanged, bytes = 0.6.1 |
| **0.8.0** ✅ | Storage engine | adaptive segmentation (codec per segment inside a block), columnar blocks (a schema of fixed-size records, a codec per column), trained preset dictionaries (embedded or external), `WorkerScratch` reuse, C ABI 1.1, dictionaries and schemas in the Java and Scala SDKs, `ace-schema`, NEON backend, macOS and Windows natives. Moved to 0.9.0: context shared between blocks (it conflicts with independent blocks and random access) | **1.6** (additive; the default writes the bytes of 0.7.0) |
| **0.9.0** | Platform integration | shared context between blocks (design first), page / segment codec API for AdaptiveDB, profiles per storage tier, async / streaming API, metrics (`tracing` / OpenTelemetry), deterministic mode for GraphNet | unchanged |
| **1.0.0** | Stable | frozen Rust API, C ABI v1 and format; LTS policy for 1.x; crates.io / Maven Central | stable |

## Platform: ACE + AdaptiveDB + GraphNet

Principles: the three projects integrate **in Rust** as crates and traits in one process (FFI
only for JVM clients), and dependencies stay acyclic — a shared `platform-contracts` crate
(errors, identifiers, versioning, telemetry) at the bottom, ACE as a leaf that knows neither
AdaptiveDB nor GraphNet, AdaptiveDB on top (AdaptiveDB → ACE for pages / segments and tiering,
AdaptiveDB → GraphNet for CDC / lineage / audit, optionally GraphNet → ACE for closed ledger
segments, which needs ACE's deterministic bytes).

| Phase | Content | When (ACE) |
|---|---|---|
| P0 conventions | `platform-contracts`, shared "Native Interop Conventions" (error codes, handles, ABI versioning, library loading, JAR packaging), common release style (tags, baselines, golden files) | alongside 0.6 |
| P1 storage | AdaptiveDB stores remote / cold-tier pages and segments with ACE | after 0.7 (stable API), fully after 0.8 |
| P2 lineage | AdaptiveDB CDC → GraphNet causal events | after GraphNet v0.10 (security) |
| P3 ledger storage | GraphNet compresses ledger segments with ACE | after 0.8 |
| P4 JVM SDK | one Java / Scala SDK over the three C ABIs | after 0.7 and the GraphNet FFI |
| P5 platform 1.0 | common release train, end-to-end tests, platform benchmark | ACE 1.0 |

The integration concept itself (module boundaries, existing APIs, CDC format) is a separate
document written after reviewing the AdaptiveDB and GraphNet repositories.

## 0.8.0 - Storage engine (done)

Segmented blocks, columnar blocks and preset dictionaries as Format 1.6, with the default
configuration writing the bytes of 0.7.0. Reproduced through the Rust engine, the C ABI (1.1), the
Java SDK and the Scala SDK by `examples/golden/0.8.0/`; frozen fixtures in `examples/compat/0.8.0/`;
fuzz targets `seg_decode` and `col_decode`; benchmark family `storage` with Regression V3 gates.
Also in this release: encoder buffer reuse, the NEON backend and native libraries for macOS and
Windows. Left for later: shared context between blocks (0.9.0), a dictionary for entropy stages,
automatic schema detection, Maven Central (1.0). Reference run: see `docs/RELEASE-NOTES-0.8.0.md`; the tag is the
maintainer's.

## 0.7.0 - Embedding (done)

The bytes of 0.6.1 through a C ABI (`include/ace.h`, ABI version 1), a Java SDK on Panama and a
Scala 3 SDK (`jvm/`), with the checks that keep the bindings honest: generated header and
bindings with drift checks in CI, the golden file reproduced by C, Java and Scala, two fuzz
targets, extended unsafe / panic audits, the `ffi` benchmark gates and native aarch64 CI.
Left for later: the NEON backend (0.8 candidates), native builds for macOS and Windows, a
Maven Central release (1.0), the platform bridges to AdaptiveDB and GraphNet (0.9).

## 0.5.1 - Hardened Release Candidate (done)

Every 0.5.0 byte and planner decision preserved (golden 0.5.0 with auto and scalar SIMD).
Decoder audit (checked arithmetic and explicit narrowing in every parser module, generated
`DECODER-AUDIT.md`, per-field and before-allocation tests, block-count limit in every reader,
bounded primary streams, strict stream ends), frozen compatibility fixtures of 0.2.1 – 0.5.0,
determinism matrix to 32 workers, Planner V5 invariant properties, private-item rustdoc and
`unreachable` / `missing_errors_doc` / `missing_safety_doc` lints, toolchain 1.98.1 (MSRV 1.97),
fuzz smoke in CI, benchmark schema 2.2, three-run repeatability, governor gate,
`allocation-profile`, `ace version`, quiet `EPIPE`, release report, canonical documentation.

## 0.5.0 - Lossless Floating-Point & Time-Series Compression (done)

Format 1.4 with the TS1 codec (Gorilla f64 / f32, RunDelta), minimal-version writer (1.3 unless
a block is TS1), Planner V5 Float lane (FloatFast / FloatGeneral, RunDelta on integer lanes),
float / run prefilters with zero false positives on the 0.4 corpus, sampled Gorilla and exact
RunDelta estimators, Corpus V4 + false-positive corpus, golden 0.5.0 with the 0.4.6 golden
frozen, Float lane benchmark families and gates. Calibration showed that Gorilla rarely beats
LZ + entropy on decimal-like data, which sets the 0.6 priorities below.

## ACE 0.6 candidates as listed at 0.5.0 (folded into the table above)

- **decimal floats (ALP-style)**: prices and quantized sensors, where Gorilla loses today;
- **RunDelta / NUM1 for small non-zero deltas** (`int-counter-reset`: RunDelta 12× at codec
  level vs 2.7–4.6× today) and **Patched FOR** for rare outliers (`monotonic-outliers`, 1.78×);
- Chimp / Chimp128 as TS1 modes 4–5; SIMD Gorilla decode;
- float column-phase detection (series not aligned to the block start);
- faster `DefaultBlockAnalyzer`, interleaved rANS decode, parallel block decode, decode without
  per-block buffers (from the 0.4.6 list);
- trained dictionaries; variable physical block size.

## 0.4.6 - Hardened Release & Benchmark Stabilization (done)

Semantic freeze (golden SHA-256), Benchmark Harness V3 (adaptive iterations, 3 x 7 batches,
median-of-medians, batch MAD, schema 2.1), interleaved A/B gates against 0.4.5-buildfix2,
environment fingerprint, determinism matrix incl. SIMD backend, malformed matrix, 12 fuzz
targets, unsafe/panic audits, reproducible package tested from the archive. Closes the 0.4 line.

## 0.4.x candidates listed at 0.4.6 (historical; carried into the 0.6 list)

- faster `DefaultBlockAnalyzer` (~47 % of BALANCED encode time);
- interleaved rANS decode; parallel block decode;
- decode without per-block buffers (`decompress` allocates ~2.3x the output, measured by the
  0.4.6 `memory` family);
- skipping generic analysis under a dominant NumericMargin **only** if planner decisions stay
  identical (otherwise it is a semantic change and moves to 0.5).

## ACE 0.5 plan as written for 0.4.6 (historical)

- Gorilla/XOR floats (done in 0.5.0), decimal specialisation (moved to 0.6);
- Patched FOR (exceptions) for `monotonic-outliers` (moved to 0.6);
- SIMD bit packing; richer timestamp models;
- trained dictionaries; variable physical block size;
- `--format 1.2` writer option for <= 0.4.4 readers if still needed.

## 0.4.5-buildfix2 - structure and decode hot path (done)

`lib.rs` = `mod` + `pub use` everywhere, generic `Lane`, single container writer / block
reader, hardware CRC32C, table Huffman, fused NUM1 decode. Encoded bytes unchanged.
Container gate run 23/23 PASS.


## 0.4.5 (0.4-buildfix5) - numeric coverage & decode hot path (done)

u16 lane, lane-relative prefilter, guaranteed Numeric candidate on the NumericGeneral route in all
profiles, table-driven rANS/Huffman decode, memcpy LZ matches, `ace inspect` numeric telemetry.

## 0.4.1 backlog (historical; superseded by the lists above)

- **Patched FOR** (exception list) for `monotonic-outliers` (currently 1.78x): NUM1 mode byte 3 would be an
  additive extension like u16 - decide after measuring exception ratios on real data.
- Skip generic block analysis when the exact NumericMargin already dominates (planner overhead is ~50 % of u64-timestamp encode time).
- Ratio-aware FAST throughput gate (FAST now compresses 3.68x instead of 1.98x).
- `--format 1.2` writer option for compatibility with <= 0.4.4 readers when u16 blocks are present.
- Parallel block decode; Gorilla XOR for float time series.


## 0.4-buildfix4 — Policy Oracle Closure & Runtime Regression Fix

Buildfix4 is intended to close the ACE 0.4 hardening line.

Closure criteria:
- policy recall/regret gates pass;
- FAST/BALANCED/DENSE remain >=95% buildfix2;
- NumericFast returns >=95% buildfix2 throughput;
- warm64K returns <=110% buildfix2;
- NumericGeneral keeps buildfix3 targets;
- NumericFast false-positive corpus is zero;
- Format 1.3/determinism/fuzzing remain healthy.

If these gates pass, the next release should be functional ACE 0.5 rather than Planner V4.4.
