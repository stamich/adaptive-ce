# ACE 0.4.6 — Benchmark methodology (Harness V3, schema 2.1)

## Why

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

## Relative gates: interleaved A/B

`tools/ace-ab0.4.6.py` builds the same probe (`tools/ace-abprobe0.4.6`) against the baseline
source tree and against 0.4.6 (identical release profile), then for each case runs three
batches per side **in alternating order** in one session:

* `speed_ratio = MoM(baseline ns) / MoM(candidate ns)`;
* the output fingerprints of both sides must be identical (semantic freeze), else `fail`;
* `pass` when even the worst per-batch ratio meets the requirement (noise cannot flip it);
* otherwise `unstable` if either side's batch MAD exceeds the case limit;
* otherwise regression (`fail`) only if `speed_ratio < required` **and** the best per-batch
  ratio is below `required + 0.02`.

**A/A control.** `./ace-ab0.4.6.sh .` compares the candidate with itself (built in a separate
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

File names: `benchmark-0.4.6-<family>.json`; other JSON documents do not use the `benchmark`
prefix (`ab-…json`, `MILESTONE-0.4.6.json`, `BASELINE.json`, `GOLDEN.json`).

## Families

`release-performance` (new) holds exactly the gate cases; `memory` adds allocation counts
(counting global allocator) and peak RSS (`VmHWM`); every other family is unchanged in meaning
and now measured with Harness V3. `stability` remains the byte-determinism family.

## Good practice

`./ace-benchmark0.4.6.sh all --isolated` pins to a CPU subset (taskset) and prints hints:
`performance` governor, no concurrent builds, AC power, idle machine.
