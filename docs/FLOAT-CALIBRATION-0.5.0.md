# ACE 0.5.0 — Float lane calibration

This document records the measurements the Planner V5 Float lane thresholds were derived
from (tasks 16 and 19 of [`TASKS-0.5.0.md`](../TASKS-0.5.0.md)). Every number can be
regenerated:

```bash
ACE_BENCH_QUICK=1 target/release/ace-bench float-ablation        # codec ablation + engine per profile
ACE_BENCH_QUICK=1 target/release/ace-bench float-estimator       # estimator error, decision regret
cargo test -p ace-analysis --test float_prefilter_corpus -- --ignored --nocapture   # prefilter profiles
```

Machine: the shared 2-vCPU development VM (smoke plan; ratios are exact, speeds indicative).

## 1. Ablation: what each codec achieves on its own

16 MiB per workload, 256 KiB blocks, codec level (no entropy stage, no planner) next to the
unchanged 0.4.6 engine (`--disable-float`). Ratio = input / output.

| Workload | Best TS1 layout | TS1 ratio | LZ fast | NUM1 | Engine FAST | Engine BALANCED | Engine DENSE |
|---|---|---:|---:|---:|---:|---:|---:|
| f64-constant | run_delta_u64 | 8 192 | 29.30 | 6 553.6 | 89.01 | 141.61 | 145.30 |
| f64-step | run_delta_u64 | 536.13 | 28.97 | 1.36 | 83.63 | 136.83 | 135.27 |
| f64-smooth | gorilla_f64 | 1.30 | 1.92 | 1.47 | 1.44 | 2.72 | 2.73 |
| f64-sensor-temperature | gorilla_f64 | 8.78 | 9.21 | 1.30 | 16.55 | 31.05 | 31.21 |
| f64-financial-price | gorilla_f64 | 1.55 | 2.25 | 1.78 | 3.89 | 5.13 | 5.25 |
| f64-noisy | gorilla_f64 | 1.23 | 1.20 | 1.47 | 1.00 | 1.42 | 1.43 |
| f64-random | gorilla_f64 | 0.97 | 0.99 | 1.00 | 1.00 | 1.00 | 1.00 |
| f64-special | gorilla_f32 | 0.98 | 1.78 | 1.00 | 1.98 | 2.61 | 3.60 |
| f32-smooth | gorilla_f32 | 1.47 | 1.24 | 2.28 | 1.28 | 1.56 | 1.58 |
| f32-sensor | gorilla_f32 | 1.95 | 1.55 | 3.20 | 3.00 | 4.00 | 4.04 |
| int-sparse-change | run_delta_u32 | 256.45 | 35.99 | 1.00 | 88.56 | 83.92 | 83.92 |
| int-counter-reset | run_delta_u64 | 11.97 | 2.06 | 4.57 | 4.56 | 2.66 | 2.66 |

Codec speed (TS1 encode / decode, MB/s): RunDelta on constant data ≈ 10 000 / 2 000,
Gorilla f64 ≈ 1 300–2 500 / 760–980, Gorilla f32 ≈ 670–1 000 / 370–440. The 0.4.6 engine
encodes the same float data at 20–160 MB/s (analysis + planning dominate).

**Findings**

1. **RunDelta is the big win** wherever whole values repeat (constant and step series,
   sparse integer changes): 4–60× better than the engine and two orders of magnitude faster.
2. **Gorilla rarely beats the entropy pipeline.** The Corpus V4 float generators produce
   decimal-like values (prices, quantized sensors) whose low mantissa bits change at every
   sample; LZ + Huffman/rANS finds more structure. Gorilla wins only against FAST on noisy or
   f32 data — and is 10–50× faster to encode. ALP-style decimal coding (0.6) is the real fix.
3. **int-counter-reset** is won by RunDelta acting as a small-delta varint (deltas are never
   zero). The concept admits RunDelta only for mostly-unchanged lanes, so 0.5.0 leaves this
   workload to the 0.4 pipeline; a small-delta admission (and Patched FOR for
   `monotonic-outliers`) is 0.6 work.

## 2. Float prefilter

Sampled profile of the first 256 KiB block of every workload (4 windows × 64 values).
`xz` = XOR-zero ratio, `mb` = mean meaningful XOR bits, `exp` = distinct exponents.

| Workload | Width | non-finite | subnormal | zero | implausible | xz | mb | exp | Verdict |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| f64-constant | f64 | 0 | 0 | 0 | 0 | 1.00 | 0.0 | 1 | admitted |
| f64-smooth | f64 | 0 | 0 | 0 | 0 | 0.06 | 41.3 | 1 | admitted |
| f64-sensor-temperature | f64 | 0 | 0 | 0 | 0 | 0.87 | 46.2 | 1 | admitted |
| f64-noisy | f64 | 0 | 0 | 0 | 0 | 0.00 | 39.7 | 1 | admitted |
| f32-smooth | f32 | 0 | 0 | 0 | 0 | 0.05 | 12.3 | 1 | admitted (f64 reading: 41.7 bits, loses the width vote) |
| f64-random | f64 | 0 | 0 | 0 | 0.83 | 0.00 | 62.2 | 240 | exponent-spread |
| f64-special | f64 | 0.14 | 0.02 | 0.05 | 0.04 | 0.00 | 21.8 | 12 | non-finite |
| u64-timestamps (ms) | f64 | 0 | 1.00 | 0 | 0 | 0.00 | 10.6 | 1 | subnormal |
| u64-timestamps-ns | f64 | 0 | 0 | 0 | 1.00 | 0.00 | 21.5 | 1 | implausible-magnitude |
| int-sparse-change | f64 | 0 | 0.25 | 0 | 0.50 | 0.98 | 9.2 | 4 | subnormal |
| structured-json | f64 | 0 | 0 | 0 | 0.83 | 0.00 | 59.5 | 44 | exponent-spread |
| zeros / mixed (block 0) | f64 | 0 | 0 | 1.00 | 0 | 1.00 | 0.0 | 1 | zero-dominated |
| low-cardinality / runs | f64 | 0 | 0 | ≤ 0.25 | ≥ 0.50 | 1.00 | 0.0 | ≤ 4 | implausible-magnitude |

Thresholds (`crates/ace-analysis/src/float.rs`):

| Rule | Threshold | Rejects |
|---|---|---|
| non-finite ratio | ≤ 0.05 | NaN / Inf heavy buffers |
| zero ratio | ≤ 0.25 | zero padding; the low halves of f64 read as f32 |
| subnormal ratio | ≤ 0.05 | integers read as floats (< 2^52) |
| exponent distinct | ≤ 16 | random / compressed bytes, text |
| implausible magnitude | ≤ 0.05 outside 1e-60 … 1e60 (f64), 1e-18 … 1e18 (f32) | large integers (ns timestamps) and byte patterns read as floats |
| mean meaningful XOR bits | ≤ 0.75 · width | pure noise |

Gate: every Corpus V4 float block admitted with the right width; no Corpus V3 block, no
non-float V4 block and no case of the false-positive corpus admitted
(`crates/ace-analysis/tests/float_prefilter_corpus.rs`, benchmark `float-false-positive`).

## 3. Route and dominance thresholds

| Constant | Value | Why |
|---|---|---|
| `FLOAT_FAST_DIVISOR` | estimate ≤ input / 32 | only RunDelta-dominated floats (constant, step) qualify; Gorilla never reaches 32× on real data, so FloatFast cannot trade ratio for speed |
| `FLOAT_FAST_FALLBACK_DIVISOR` | payload > input / 16 → FloatGeneral | safety net for a wrong sample; 0 fallbacks on Corpus V4 |
| `TS1_MAX_FRACTION_OF_GENERIC` | exact TS1 ≤ 0.90 × generic | RunDelta sizes are exact |
| `TS1_MAX_FRACTION_SAMPLED` | sampled TS1 ≤ 0.75 × generic | at 0.90 `f32-smooth` BALANCED lost 2.5 %: the Gorilla sample and the generic blended estimate erred in opposite directions |
| `TS1_MIN_SAVING_DIVISOR` | saving ≥ input / 32 | blocks the 0.4 pipeline already shrinks below 3 % stay Format 1.3 |
| `MIN_EQUAL_RATIO` (run prefilter) | ≥ 0.80 | concept: RunDelta for mostly-unchanged lanes |
| `MAX_SPLAT_RATIO` (run prefilter) | ≤ 0.50 | lanes of one repeated byte are RLE territory |

The splat rule was added after calibration: the generic blended estimate of `rle+huffman`
on the Corpus V3 `runs` workload is ≈ 6× pessimistic (14 588 B estimated vs 2 391 B real per
block), which let RunDelta replace RLE and move a 0.4.x file to Format 1.4.

## 4. Estimator accuracy

`float-estimator`, BALANCED, every admitted block of the float-family workloads (499 blocks):
Gorilla sample estimate relative error MAPE 6.8 %, bias −5.0 % (optimistic), p95 17.9 %,
max 37.5 % (`f64-sensor-temperature`, where 87 % of the values repeat and window placement
matters). RunDelta estimates are exact by construction (`ts1_encoded_len` runs the encoder
against a bit counter).

## 5. Result

With these thresholds (Regression V3 `float` section):

* Corpus V3 output is byte-identical to 0.4.6 (golden 0.4.6 frozen), every profile;
* no Corpus V4 workload is larger with the lane on than with `--disable-float`;
* FloatFast: f64-constant 2 416×, f64-step 464× (vs 142× / 137× in 0.4.6), encode 20–60×
  faster than the disabled lane, generic analysis skipped, zero fallbacks;
* int-sparse-change BALANCED 235× (vs 84×); f64-smooth / f64-noisy / f32-smooth FAST
  1.03–1.23× smaller with Gorilla blocks.
