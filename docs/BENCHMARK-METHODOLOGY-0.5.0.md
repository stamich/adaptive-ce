# ACE 0.5.0 — Benchmark methodology (Harness V3, schema 2.1)

Harness V3 and schema 2.1 are unchanged since 0.4.6; 0.5.0 adds the Float lane families, a
`float` gate section and same-run comparisons for Float lane cases (sections marked *0.5.0*).

## Why (0.4.6)

Up to 0.4.5-buildfix2 a sample was one invocation and a case had 7 samples. For operations of
30 µs – 7 ms the spread came from timer resolution, page faults and fixed costs, and
cross-run drift of the machine (±10 %) was larger than the 5 % regression gates.

## Measurement (crates/ace-bench/src/timing)

| Step | Release plan | Quick plan (`ACE_BENCH_QUICK=1`) |
|---|---|---|
| warm-up invocations | 5 | 1 |
| calibration | double `iterations` until one group lasts ≥ 50 ms (cap 2^20) | ≥ 2 ms |
| batches × samples | 3 × 7 | 1 × 5 |
| sample | mean per-invocation time of `iterations` back-to-back calls | same |

Every result passes through `black_box`; the last result is returned for verification.
`ACE_BENCH_MIN_SAMPLE_MS` overrides the minimum sample time.

Decode cases of `release-performance` use `AceEngine::decompress_into` with one reused buffer,
so they do not measure zeroing / page faults of a fresh 16 MiB vector; the allocating variant is
reported as `decompression_alloc`.

## Statistics

| Name | Definition | Use |
|---|---|---|
| `median_of_medians_ns` (= `median_ns`) | median of the three batch medians | release value |
| `batch_mad_percent` | MAD of the batch medians around the MoM, relative to the MoM | stability gate |
| `global_median_ns`, `mad_ns` | over all samples | diagnostics |
| `outlier_indices` | samples farther than 3 · 1.4826 · MAD from the global median | reported, never removed |
| `cv_percent` | stddev / mean of all samples | diagnostic only (2.0 field) |

## Gate statuses (Regression V3)

| Status | Meaning |
|---|---|
| `pass` | requirement met with a stable measurement |
| `fail` | requirement violated with a stable measurement |
| `unstable` | batch MAD above the limit; the release script re-measures (≤ 2 retries); never a release |
| `skipped` | input not provided / not applicable (quick plan, missing A/B); release verdict `incomplete` |
| `diagnostic` | informational |

Stability limits (batch MAD): compression and numeric encode 3 %, decode 5 %, warm 64 KiB 5 %.
`float_fast.f64_step` compression (0.5.0): 6 % — the FloatFast encode of 16 MiB takes ~5 ms
(memory-bound), and its only gate is a same-run speedup with an ~8× margin.

## Relative gates: interleaved A/B

`tools/ace-ab0.5.0.py` builds the same probe (`tools/ace-abprobe0.5.0`) against the baseline
source tree (0.4.6) and against 0.5.0 (identical release profile), then for each case runs three
batches per side **in alternating order** in one session:

* `speed_ratio = MoM(baseline ns) / MoM(candidate ns)`;
* the output fingerprints of both sides must be identical, else `fail` — every A/B case uses a
  Corpus V3 workload, which 0.5.0 must compress to exactly the 0.4.6 bytes;
* `pass` when even the worst per-batch ratio meets the requirement (noise cannot flip it);
* otherwise `unstable` if either side's batch MAD exceeds the case limit;
* otherwise regression (`fail`) only if `speed_ratio < required` **and** the best per-batch
  ratio is below `required + 0.02`.

**A/A control.** `./ace-ab0.5.0.sh .` compares the candidate with itself (built in a separate
directory). On the 2-vCPU development VM used for 0.4.6, A/A decode ratios moved by up to
±20 % between separately linked but source-identical binaries (code layout, neighbours), while
encode cases stayed within ±4 %. Run the A/A control first on a new machine: only differences
clearly outside its spread are meaningful.

Required ratios: 0.95 for compression / decompression / numeric / full decompression;
`1 / 1.10` for warm 64 KiB latency. Absolute floors (u64 timestamps ≥ 90 MB/s, delta-variable
≥ 75 MB/s, warm 64 KiB ≤ 71.7 µs) are evaluated on `release-performance`.

Comparisons with stored JSON from another machine or methodology (schema 2.0) are printed as
diagnostics only; lz4 / zstd from the same run measure machine-speed drift (> 8 % → warning).

## Schema 2.1

* every 2.0 field is kept; `median_ns`, `median_mb_s` and friends are computed from the MoM;
* each timing object gains `stable_timing { harness, batches, samples_per_batch,
  iterations_per_sample, batch_medians_ns, median_of_medians_ns, global_median_ns, mad_ns,
  batch_mad_percent, outlier_indices, stability }`;
* documents gain `benchmark_methodology` and an extended `environment` (CPU features, governor,
  frequency / temperature / load before and after, rustc, target, opt-level, LTO,
  codegen-units, target features, SIMD / CRC backend, warnings);
* tools read 2.0 and 2.1; a 2.0 measurement is never used for a stability gate.

File names: `benchmark-0.5.0-<family>.json`; other JSON documents do not use the `benchmark`
prefix (`ab-…json`, `MILESTONE-0.5.0.json`, `BASELINE.json`, `GOLDEN.json`). Float lane rows
add fields (`float_stats`, `ratio_gain_vs_disabled`, `encode_speedup_vs_disabled`,
`compression_disabled`, …); the schema allows additive row fields, so the validator is
unchanged.

## Families

`release-performance` holds exactly the gate cases; `memory` adds allocation counts
(counting global allocator) and peak RSS (`VmHWM`); `stability` remains the byte-determinism
family.

### Float lane families (0.5.0)

| Family | Content | Gates fed |
|---|---|---|
| `float-ablation` | every codec (RAW, RLE, LZ fast, NUM1, each TS1 layout) block by block without the planner, the best TS1 layout's codec speed, and the engine result per profile (lane off) | Gorilla codec speed, NUM1 reference ratio |
| `float` | Corpus V4 + integer references × 3 profiles, lane on vs off: bytes, gain, routes, TS1 modes, format version, timing | never larger than the lane off, ratio targets, fallbacks, full trials |
| `float-fastpath` | f64-constant / f64-step: generic-analysis / planning / encoding time, encode speedup over the lane off (same run) | FloatFast skips analysis, all blocks FloatFast |
| `float-false-positive` | Corpus V3, non-float V4 data and the false-positive corpus (`ace_corpus::FalsePositiveCase`) | zero Float-route blocks, Corpus V3 identity |
| `float-estimator` | Gorilla estimate error (MAPE, bias, p95, max) and per-block decision recall / regret against the better of TS1 and the generic plan | estimator p95 ≤ 25 % |

`release-performance` adds `float_fast.f64_step`, `float_general.f64_noisy` and
`run_delta.int_sparse_change`; each also measures the same engine with the lane disabled in the
same process (`compression_disabled`, `encode_speedup_vs_disabled`, `ratio_gain_vs_disabled`).
These are **within-run ratios**: their output intentionally differs from 0.4.6, while byte
identity is an invariant of every interleaved A/B case, and a same-run ratio is immune to
machine drift.

### Float gates (Regression V3, 0.5.0)

| Gate | Requirement | Section |
|---|---|---|
| `float.false_positive_blocks` | 0 | correctness |
| `float.corpus_v3_identical` | true | correctness |
| `float.fast_fallbacks`, `float.full_trial_encodes` | 0 | correctness |
| `float.fastpath_generic_analysis_ns`, `float.fastpath_all_blocks` | 0 / true | correctness |
| `float.min_ratio_gain_vs_disabled` | ≥ 1.0 | float |
| `float.f64_constant_ratio`, `float.f64_smooth_ratio` | ≥ 50×, ≥ 2.5× (BALANCED) | float |
| `float.int_sparse_change_ratio`, `…_vs_num1` | ≥ 5×, ≥ 1.5 × NUM1 | float |
| `float.gorilla_estimate_p95_error` | ≤ 25 % | float |
| `float.gorilla_codec_encode_mb_s`, `…_decode_mb_s` | ≥ 400, ≥ 600 MB/s (timed, batch MAD ≤ 5 %) | float |
| `float.fast_encode_speedup_vs_disabled` | ≥ 5× (timed, batch MAD ≤ 6 %) | float |

The codec-speed floors are the concept's indicative targets, set below the development-VM
values (Gorilla f64 decode ≈ 760–830 MB/s there); they become hard values after the first
reference-machine run.

## Good practice

`./ace-benchmark0.5.0.sh all --isolated` pins to a CPU subset (taskset) and prints hints:
`performance` governor, no concurrent builds, AC power, idle machine.
