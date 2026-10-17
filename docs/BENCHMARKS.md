# ACE benchmarks (Harness V3, schema 2.2)

Canonical methodology of the current release (0.5.1). Earlier versions:
[`history/`](history/) (`BENCHMARK-METHODOLOGY-0.4.6.md`, `BENCHMARK-METHODOLOGY-0.5.0.md`).
Harness V3 dates from 0.4.6, the Float lane families and gates from 0.5.0; 0.5.1 adds
schema 2.2, three-run repeatability, the governor gate and the `allocation-profile` family
(sections marked *0.5.1*).

## Running

```bash
./ace-benchmark0.6.0.sh all                     # every family + validation + Regression V3
./ace-benchmark0.6.0.sh all --repeat 3          # release baseline: three runs, repeatability, medians
./ace-benchmark0.6.0.sh release-performance --quick   # smoke (1 batch; never a release verdict)
./ace-ab0.6.0.sh /path/to/ace-0.5.0             # interleaved A/B against the previous release
```

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

`tools/ace-ab0.6.0.py` builds the same probe (`tools/ace-abprobe0.6.0`) against the baseline
source tree (the previous release) and against the candidate (identical release profile), then for each case runs three
batches per side **in alternating order** in one session:

* `speed_ratio = MoM(baseline ns) / MoM(candidate ns)`;
* the output fingerprints of both sides must be identical, else `fail` — every A/B case uses a
  Corpus V3 workload, whose bytes have not changed since 0.4.6 (0.5.1 changes no byte at all);
* `pass` when even the worst per-batch ratio meets the requirement (noise cannot flip it);
* otherwise `unstable` if either side's batch MAD exceeds the case limit;
* otherwise regression (`fail`) only if `speed_ratio < required` **and** the best per-batch
  ratio is below `required + 0.02`.

**A/A control.** `./ace-ab0.6.0.sh .` compares the candidate with itself (built in a separate
directory). On the 2-vCPU development VM used for 0.4.6, A/A decode ratios moved by up to
±20 % between separately linked but source-identical binaries (code layout, neighbours), while
encode cases stayed within ±4 %. Run the A/A control first on a new machine: only differences
clearly outside its spread are meaningful.

Required ratios: 0.95 for compression / decompression / numeric / full decompression;
`1 / 1.10` for warm 64 KiB latency. Absolute floors (u64 timestamps ≥ 90 MB/s, delta-variable
≥ 75 MB/s, warm 64 KiB ≤ 71.7 µs) are evaluated on `release-performance`.

Comparisons with stored JSON from another machine or methodology (schema 2.0) are printed as
diagnostics only; lz4 / zstd from the same run measure machine-speed drift (> 8 % → warning).

## Schema 2.1 and 2.2

* every 2.0 field is kept; `median_ns`, `median_mb_s` and friends are computed from the MoM;
* each timing object gains `stable_timing { harness, batches, samples_per_batch,
  iterations_per_sample, batch_medians_ns, median_of_medians_ns, global_median_ns, mad_ns,
  batch_mad_percent, outlier_indices, stability }`;
* documents gain `benchmark_methodology` and an extended `environment` (CPU features, governor,
  frequency / temperature / load before and after, rustc, target, opt-level, LTO,
  codegen-units, target features, SIMD / CRC backend, warnings);
* *0.5.1 — schema 2.2 (additive)*: `environment.build.git_revision` / `git_dirty` (from
  `build.rs`), `configuration.config_fingerprint` (SHA-256 prefix of the default engine
  configuration and the measurement plan), `configuration.threads` / `threads_resolved` /
  `block_size`, and `workload_fingerprints` (`"<workload>@<bytes>"` → SHA-256 of every
  generated input, recorded during setup, never inside a timed region). Two documents are
  comparable only when their configuration fingerprints match and their input fingerprints
  agree;
* tools read 2.0, 2.1 and 2.2; a 2.0 measurement is never used for a stability gate.

File names: `benchmark-<version>-<family>.json`; other JSON documents do not use the
`benchmark` prefix (`ab-…json`, `MILESTONE-<version>.json`, `BASELINE.json`, `GOLDEN.json`). Float lane rows
add fields (`float_stats`, `ratio_gain_vs_disabled`, `encode_speedup_vs_disabled`,
`compression_disabled`, …); the schema allows additive row fields, so the validator is
unchanged.

## Repeatability and the release baseline (0.5.1)

A release baseline is the **median of three complete runs** on the reference machine
(`./ace-benchmark0.6.0.sh all --repeat 3`, also what `ace-release0.6.0.sh` runs):

* runs go to `examples/results/run-1 … run-3`; `tools/ace-repeatability0.6.0.py` compares them
  and writes, per family, the document whose timing objects are the median run of each
  measurement into `examples/results/` — that directory is then evaluated by Regression V3
  and published as `examples/baselines/<version>/`;
* `deterministic` gate: every value that is not a time or memory measurement (sizes, ratios,
  planner decisions, recall / regret, counters, input fingerprints) must be identical in all
  runs;
* `macro` gate: for every whole-input throughput case of `release-performance`,
  `(max − min) / median` across the runs is ≤ 5 % for compression and ≤ 15 % for
  decompression timings (`decompression*`, `full_decompress.*`; `--max-decode-spread`).
  Whole-input decodes take 3 – 10 ms and follow the CPU boost clock: the Ryzen 9 5950X
  reference machine spread by 5 – 13 % between three runs with the `performance` governor, so
  a 5 % limit rejected every run. Decode speed itself is bounded by the interleaved A/B
  (≥ 95 % of the previous release) and the batch-MAD stability gates of Regression V3; this
  gate only detects an unusable machine. Random-access latencies are diagnostics; quick runs
  cannot qualify;
* exit codes as Regression V3; a failed repeatability check fails the benchmark script.

## Governor gate (0.5.1)

A run whose recorded CPU governor is known and is not `performance` (for example the
`powersave` default of many laptops) can never produce a `pass`: Regression V3 and the
repeatability tool report `diagnostic` (exit 4) instead. Hosts without a cpufreq interface
(most VMs, CI) record the governor as unknown and are judged by their measurements alone.

## Families

`release-performance` holds exactly the gate cases; `memory` adds allocation counts
(counting global allocator) and peak RSS (`VmHWM`); `stability` remains the byte-determinism
family.

### `allocation-profile` (0.5.1)

Replays the first 16 blocks of each `memory` case exactly as the block encoder plans them and
counts allocations per stage (classify, NumericFast, FloatFast, analysis, candidates,
evaluator, TS1 policy, encode), next to the engine total. It explains the allocation churn
seen in the 0.5.0 baseline: for `int-sparse-change` BALANCED (~36 bytes allocated per input
byte) about 94 % came from the generic V4.3 evaluator, whose sampled micro-trials ran before
the TS1 RunDelta candidate replaced the generic plan. The allocations themselves came from
`lz_encode`, which allocated a 512 KiB hash table and an 8-byte-per-byte chain table for every
sample; since 0.6.0 each thread reuses them (output unchanged), and the same row reports about
6 bytes per input byte. Deciding TS1 earlier would change the order of work and is not done.
The performance report prints the table.

### `public-corpus` (0.6.0)

ACE (FAST, BALANCED, DENSE; one thread) next to LZ4, Zstd level 3 and gzip level 6 on files you
supply, so the numbers can be compared with published tables. The corpus is not in the
repository: point `ACE_PUBLIC_CORPUS` at a directory (for example the Silesia corpus, or
`enwik8`) and every regular file in it is measured.

```text
ACE_PUBLIC_CORPUS=/data/silesia cargo run --release -p ace-bench -- public-corpus
ACE_PUBLIC_CORPUS_MAX_BYTES=67108864   # truncate larger files (default 256 MiB)
ACE_BENCH_QUICK=1                      # short measurement for a smoke run
```

Without the variable, or when it is not a directory, the family prints why and writes a document
with no workloads, so `all` still runs everywhere. Each row has the usual timing objects and an
`lzbench` object with the columns of the lzbench table: `compressed_bytes`, `ratio_percent`
(compressed size as a percentage of the input), `compress_mb_s` and `decompress_mb_s`
(10^6 bytes per second, from the median). The fingerprint of each file is recorded in
`workload_fingerprints`. The family takes minutes to hours depending on the corpus and is not a
release gate; it is a reference for the comparison tables.

### DENSE versus BALANCED (0.6.0 decision)

Question: does `Profile::Dense` earn its extra encode time? Data: container sizes of the 27
workloads of the golden file (1 MiB each, deterministic) and the stored 0.5.1 baseline
(`compression`, `mixed_16m`, reference machine).

| | FAST | BALANCED | DENSE |
|---|---|---|---|
| bytes, 27 workloads | 7 671 209 | 6 880 023 | 6 859 138 |
| versus BALANCED | +11.5 % | — | −0.30 % |
| `mixed_16m` ratio | 3.682 | 3.726 | 3.722 |
| `mixed_16m` encode | 334 MB/s | 223 MB/s | 167 MB/s |
| `mixed_16m` decode | 4 821 MB/s | 4 564 MB/s | 4 161 MB/s |

* DENSE is smaller than BALANCED by more than 0.5 % on 5 of 27 workloads: `low-cardinality`
  (−2.6 %), `f64-financial-price` (−2.1 %), `f32-sensor` (−1.7 %), `f64-noisy` (−1.2 %),
  `f32-smooth` (−0.9 %). It is identical on 15 and **larger** on `runs` (+10.9 %), `zeros`
  (+30 %), `f64-special` (+0.9 %) and `mixed` (+0.1 %).
* It costs 34 % more encode time and decodes 9 % slower than BALANCED on the mixed workload,
  where it is also 0.1 % larger.

Decision for 0.6.0: **BALANCED remains the default and the recommendation; DENSE is kept as it
is** (0.6.0 may not change a byte) and documented as "may save up to about 3 % on
floating-point and low-cardinality data for a third more encoding time, and is not guaranteed
to be smaller than BALANCED". The cases where it is larger show that its estimates, not its
search, are the limit; making DENSE never worse than BALANCED changes planner decisions and
is therefore a candidate for 0.6.1 with the other Phase B changes.

**0.6.1 follow-up: DENSE is never larger than BALANCED.** The engine plans every DENSE block a
second time as BALANCED and stores the smaller result (ties keep DENSE), so the guarantee holds
per block and therefore per file. Effect on the golden workloads (1 MiB each): `zeros` 4 488 ->
3 448 bytes, `runs` 10 602 -> 9 564, `mixed` 281 768 -> 281 508, `f64-special` 253 222 -> 250 983,
`f64-decimal-price` (Corpus V5) 252 130 -> 206 328; no other workload changes in DENSE. Cost:
DENSE encoding includes one BALANCED plan per block (measured with the 0.6.1 release run, see
`PERFORMANCE-0.6.1.md`); decoding is unaffected. `tests/dense_vs_balanced.rs` checks every corpus
workload and the false-positive corpus.

### Float lane families (0.5.0)

| Family | Content | Gates fed |
|---|---|---|
| `float-ablation` | every codec (RAW, RLE, LZ fast, NUM1, each TS1 layout) block by block without the planner, the best TS1 layout's codec speed, and the engine result per profile (lane off) | Gorilla codec speed, NUM1 reference ratio |
| `float` | Corpus V4 + integer references × 3 profiles, lane on vs off: bytes, gain, routes, TS1 modes, format version, timing | never larger than the lane off, ratio targets, fallbacks, full trials |
| `float-fastpath` | f64-constant / f64-step: generic-analysis / planning / encoding time, encode speedup over the lane off (same run) | FloatFast skips analysis, all blocks FloatFast |
| `float-false-positive` | Corpus V3, non-float V4 data and the false-positive corpus (`ace_corpus::FalsePositiveCase`) | zero Float-route blocks, Corpus V3 identity |
| `float-estimator` | Gorilla estimate error (MAPE, bias, p95, max) and per-block decision recall / regret against the better of TS1 and the generic plan | estimator p95 ≤ 25 % |
| `chimp-evaluation` | Chimp / Chimp128 against the best bare codec and the engine, per 256 KiB block (0.6.1 decision) | share of shrinking blocks ≥ 5 % to add Chimp |

`release-performance` adds `float_fast.f64_step`, `float_general.f64_noisy` and
`run_delta.int_sparse_change`; each also measures the same engine with the lane disabled in the
same process (`compression_disabled`, `encode_speedup_vs_disabled`, `ratio_gain_vs_disabled`).
These are **within-run ratios**: their output intentionally differs from 0.4.6, while byte
identity is an invariant of every interleaved A/B case, and a same-run ratio is immune to
machine drift.

### `chimp-evaluation` (0.6.1 decision: Chimp is not added)

Question: does Chimp / Chimp128 (Liakos et al., 2022) deserve a TS1 mode next to Gorilla, RunDelta
and ALP? `crates/ace-bench/src/chimp.rs` implements both as working codecs (decoders included,
`tests/chimp.rs` checks exact round trips); they are **not** part of the format.

Method: 9 binary64 workloads x 16 blocks of 256 KiB (144 blocks). Per block the best existing
choice is the smallest of the bare codecs (RAW, RLE, LZ fast, NUM1, every TS1 layout including ALP)
and of the whole engine (BALANCED and DENSE, one block per file, container overhead not
subtracted). A block shrinks when the better Chimp variant, charged a TS1-sized header, is strictly
smaller. Rule: Chimp is kept only if at least 5 % of the blocks shrink.

| Baseline | Blocks where Chimp is smaller |
|---|---:|
| bare codecs only (no entropy stage) | 31 of 144 (21.5 %, all on `f64-special` and `f64-smooth`) |
| bare codecs and the engine | **0 of 144 (0 %)** |

The 31 blocks are won against Gorilla and RAW, but the generic plans (LZ + rANS) already store
them smaller: the engine beats every bare codec on 48 blocks. Decision: **no Chimp mode** in
0.6.1; TS1 mode 5 stays reserved. Re-run with `ace-bench chimp-evaluation` (sizes are exact and do
not depend on the machine); the family writes the per-workload rows and a `chimp_decision` row.

### Phase B gates (0.6.1)

New family `format-1-5` (`ace-bench format-1-5`: ratios and format versions of the Phase B workloads
per profile, codec speed of ALP and Patched FOR, and the DENSE-versus-BALANCED sweep over all
31 corpus workloads) and the section `format15` of Regression V3:

| Gate | Requirement | 0.6.0 |
|---|---|---|
| `format15.dense_larger_than_balanced` | 0 of 31 workloads (correctness) | 5 |
| `format15.int_counter_reset_ratio` | >= 15x | 2.7x |
| `format15.monotonic_outliers_ratio` | >= 100x | 1.8x |
| `format15.gauge_sawtooth_ratio` | >= 500x | 19x |
| `format15.delta_variable_ratio` | >= 8x | 6.4x |
| `format15.f64_decimal_price_ratio`, `…_financial_price_ratio` | >= 4.5x, >= 5.2x | — / 5.1x |
| `format15.int_geometric_delta_gain` | >= 1.25x the disabled lane | — |
| `format15.int_slow_drift_control` | >= 0.99x (NUM1 stays within 10 %) | — |
| `format15.alp_codec_encode_mb_s`, `…_decode_mb_s` | >= 150, >= 300 MB/s (provisional) | — |
| `format15.patched_codec_encode_mb_s`, `…_decode_mb_s` | >= 200, >= 300 MB/s (provisional) | — |

Measured on the development VM (quick run): counter-reset 21.1x, monotonic-outliers 401x,
gauge-sawtooth 1137x, delta-variable 10.2x, decimal price 5.1x, financial price 5.7x,
geometric delta 1.47x, ALP 440 / 866 MB/s, Patched FOR 524 / 807 MB/s. The two codec-speed floors
are provisional: the 0.6.1 reference run (task "release run") confirms or raises them.

Interleaved A/B against 0.6.0 (`tools/ace-ab0.6.1.py`, base tag `v0.6.0`) gets cases for every
path whose planning changed. Byte-identical cases keep the 0.95 floor; cases whose bytes change
on purpose are flagged `output_changes` (no identity check) and carry the documented price of
the change:

| Case | Floor | Why |
|---|---:|---|
| `compression.dense.encode` | 0.50 | DENSE also plans every block as BALANCED (about 0.58x measured) |
| `numeric_general.delta_variable.encode` | 0.70 | Patched FOR estimate and encoder (about 0.74x, bytes -37 %) |
| `numeric_general.monotonic_outliers.encode` | 0.70 | same (about 0.98x, bytes -99.5 %) |
| `numeric_general.int_counter_reset.encode`, `float.f64_decimal_price.encode / .decode` | 0.95 | faster than 0.6.0 (1.3x - 3.7x) |
| `numeric_general.u64_timestamps.encode`, `float.f64_noisy.encode`, `float.f64_smooth.encode` | 0.95 | bytes identical; the Patched and ALP pre-tests keep the cost within noise |

Three pre-tests keep the new modes off the hot path: Patched FOR is estimated only at lane widths
within 25 % of the best base size and only when 16-value chunks of some source differ by at least
8 bits; ALP is counted only for blocks that are not FloatFast-dominant and whose 64-value sample
is at least half decimal-shaped. Without the ALP rule FAST encode of `f64-constant` fell to 4.4x
the disabled lane (gate: 5x); with it 17x.

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

The codec-speed floors are hard gates. They were confirmed by the 0.5.0 runs and are kept
below the slowest measured machine so they stay portable:

| Machine | Gorilla encode MB/s | Gorilla decode MB/s |
|---|---:|---:|
| AMD Ryzen 9 5950X (reference, `examples/baselines/0.5.0/`) | 2 000 | 1 329 |
| 2-vCPU Xeon development VM | 1 446 | 826 |
| Intel i7-9850H laptop (`powersave`) | 1 242 | 753 |
| **floor** | **400** | **600** |

## Good practice

`./ace-benchmark0.6.0.sh all --isolated` pins to a CPU subset (taskset) and prints hints:
`performance` governor, no concurrent builds, AC power, idle machine.
