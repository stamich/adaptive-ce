# ACE Roadmap

## 0.4.5-buildfix2 - structure and decode hot path (done)

`lib.rs` = `mod` + `pub use` everywhere, generic `Lane`, single container writer / block
reader, hardware CRC32C, table Huffman, fused NUM1 decode. Encoded bytes unchanged.
Container gate run 23/23 PASS.


## 0.4.5 (0.4-buildfix5) - numeric coverage & decode hot path (done)

u16 lane, lane-relative prefilter, guaranteed Numeric candidate on the NumericGeneral route in all
profiles, table-driven rANS/Huffman decode, memcpy LZ matches, `ace inspect` numeric telemetry.

## 0.4.1 backlog (evidence-driven, requires no format change unless stated)

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

## ACE 0.5 candidates

- Gorilla/XOR float/time-series compression;
- decimal-specific transforms;
- dictionary learning/reuse;
- SIMD BitPack after planner/runtime overhead is closed;
- numeric checkpoints only if production random-access evidence justifies format complexity.

No Format 1.4 work is part of buildfix4.
