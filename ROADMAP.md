# ACE Roadmap

## 0.4.6 - Hardened Release & Benchmark Stabilization (done)

Semantic freeze (golden SHA-256), Benchmark Harness V3 (adaptive iterations, 3 x 7 batches,
median-of-medians, batch MAD, schema 2.1), interleaved A/B gates against 0.4.5-buildfix2,
environment fingerprint, determinism matrix incl. SIMD backend, malformed matrix, 12 fuzz
targets, unsafe/panic audits, reproducible package tested from the archive. Closes the 0.4 line.

## Next: format-compatible 0.4.x candidates (only if golden SHA-256 stays unchanged)

- faster `DefaultBlockAnalyzer` (~47 % of BALANCED encode time);
- interleaved rANS decode; parallel block decode;
- decode without per-block buffers (`decompress` allocates ~2.3x the output, measured by the
  0.4.6 `memory` family);
- skipping generic analysis under a dominant NumericMargin **only** if planner decisions stay
  identical (otherwise it is a semantic change and moves to 0.5).

## ACE 0.5 - Advanced Time-Series & Numeric Compression

- Gorilla/XOR floats, decimal specialisation;
- Patched FOR (exceptions) for `monotonic-outliers` (1.78x today);
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
