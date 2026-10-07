# ACE 0.5.0 — Release notes

**Lossless Floating-Point & Time-Series Compression** — the first functional release after
the hardened 0.4 line.

## Highlights

* **TS1 codec (Format 1.4)** — Gorilla XOR for f64 / f32 series and RunDelta for values that
  rarely change (u16 / u32 / u64 lanes, floats compared bit-wise). Bit-exact for every IEEE
  value (−0.0, NaN payloads, ±Inf, subnormals). [`TS1-CODEC.md`](TS1-CODEC.md)
* **Planner V5** — the Float lane: `FloatFast` encodes a dominant TS1 candidate once without
  generic analysis; `FloatGeneral` runs the unchanged V4.3 pipeline of the block's base route
  and lets TS1 replace it only when it clearly wins. RunDelta also competes on integer blocks
  with mostly unchanged values. [`PLANNER-V5.md`](PLANNER-V5.md)
* **Minimal-version writer** — files are Format 1.3 (readable by 0.4.x) unless a block is
  TS1; `--disable-float` reproduces 0.4.6 exactly. [`FORMAT-1.4.md`](FORMAT-1.4.md)
* **Measured before planned** — the `float-ablation` family measured every codec on every
  profile before the planner existed; every threshold is documented with its measurement.
  [`FLOAT-CALIBRATION-0.5.0.md`](FLOAT-CALIBRATION-0.5.0.md)

## Results (benchmark `float`, 4 MiB per workload, 1 thread, development VM)

| Workload | Profile | Lane off (= 0.4.6) | 0.5.0 | Encode speed vs lane off |
|---|---|---:|---:|---:|
| f64-constant | BALANCED | 141.4× | 2 416× | ≈ 35× faster (FloatFast) |
| f64-step | BALANCED | 136.4× | 464× | ≈ 38× faster (FloatFast) |
| int-sparse-change | BALANCED | 83.4× | 235× | ≈ 1.3× faster |
| f64-smooth | FAST | 1.53× | 1.82× | — |
| f64-noisy | FAST | 1.00× | 1.23× | — |
| f32-smooth | FAST | 1.28× | 1.32× | — |
| Corpus V3 (all 15 workloads) | all | — | **byte-identical** | unchanged |

Ratios are deterministic; speeds are indicative (shared 2-vCPU VM).

Interleaved A/B against the 0.4.6 tree on the same VM: 12 / 12 cases PASS with
byte-identical output (encode 0.97–1.09×, decode 0.97–1.02×; NumericFast encode 1.32× after
the grouped fixed-step validation); Regression V3: **PASS** (74 gates, no failure or
instability).

Every Corpus V4 workload is at least as small as with the lane off, in every profile; zero
blocks of Corpus V3 and of the false-positive corpus take a Float route; FloatFast never fell
back. Generated tables: [`PERFORMANCE-0.5.0.md`](PERFORMANCE-0.5.0.md).

**Honest limits of the Float lane.** Gorilla rarely beats the existing LZ + entropy pipeline
on decimal-like data (prices, quantized sensors): their low mantissa bits change at every
sample. BALANCED / DENSE therefore keep the generic plan on `f64-smooth`, `f64-sensor-*`,
`f64-financial-price` and the f32 series (by design: the planner only switches when TS1 wins
clearly). Decimal specialisation (ALP) is the 0.6 answer.

## New

* `ace-codecs::time_series` (`ts1_encode_with`, `ts1_encoded_len`, `ts1_stream_bits`,
  `ts1_decode`, `ts1_inspect`, `TimeSeriesLayout`, `TimeSeriesMode`).
* `ace_bitpack::{BitWriter, BitReader}` — public LSB-first bitstream shared by NUM1 and TS1.
* `ace-analysis`: `float_prefilter` / `FloatProfile`, `run_prefilter` / `RunProfile`,
  `lane_sample_windows`.
* `ace-cost`: `estimate_gorilla` (sampled), `estimate_run_delta` (exact).
* `ace-planner`: Float routes, `FloatEvidence`, `classify_float_lane`, `float_fast_decision`,
  `apply_time_series_policy`, `TimeSeriesSelection`, Float telemetry.
* `AceConfig::enable_float_specialization`; CLI `--disable-float`; TS1 in `inspect`, route
  and float evidence in `explain`.
* Corpus V4 (12 workloads) and the false-positive corpus in `ace-corpus`.
* Benchmarks: `float-ablation`, `float`, `float-fastpath`, `float-false-positive`,
  `float-estimator`; Float cases in `release-performance` and `memory`; Regression V3 `float`
  section.
* Tests: golden 0.5.0 (27 workloads), frozen golden 0.4.6, `float_lane`, Format 1.4 readers,
  TS1 malformed fixture, TS1 properties, Float workloads in the determinism matrix and
  streaming tests; fuzz targets `ts1_decode`, `ts1_roundtrip`, `bitstream_roundtrip` (15).
* Demo step for the Float lane.

## Changed

* Streaming encoder sink: `Write + Seek` (Format 1.4 header rewrite) — see
  [`MIGRATION-0.4-TO-0.5.md`](MIGRATION-0.4-TO-0.5.md).
* Public enums / structs gained variants and fields (`CodecId::TimeSeries`,
  `AceError::InvalidTimeSeries`, Planner V5 types, statistics).
* A TS1 block with a transform, entropy stage or dictionary is rejected as malformed.

## Known limitations

* no decimal / ALP, Chimp or SIMD Gorilla; Gorilla is scalar;
* RunDelta is admitted only for mostly unchanged lanes — small non-zero deltas
  (`int-counter-reset`) and rare outliers (`monotonic-outliers`, Patched FOR) stay with the
  0.4 pipeline;
* FAST may choose Gorilla where its ratio beats FAST's generic plan; BALANCED / DENSE choose
  TS1 only with a clear margin — some small gains are left on the table on purpose;
* no float column-phase detection (data starting at an unaligned offset);
* the earlier 0.4.6 limitations (scalar rANS, fixed block size, no trained dictionaries,
  decode allocations, x86_64-only acceleration) still apply;
* indicative Float codec-speed floors become hard gates after the reference-machine run.

## Upgrade

Drop-in for reading. For writing, read [`MIGRATION-0.4-TO-0.5.md`](MIGRATION-0.4-TO-0.5.md):
two signature-level changes (streaming sink, `AceConfig` literal) and additive enum variants.
