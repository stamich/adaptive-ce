# ACE 0.5.0 — Architecture

0.5.0 adds lossless floating-point and time-series compression without disturbing the 0.4
pipeline: one new codec (TS1, Format 1.4), one new planner lane (Planner V5: FloatGeneral /
FloatFast + a RunDelta candidate) and the analysis / estimation they need. Corpus V3 output
is byte-identical to 0.4.6 (frozen golden file). History: [`history/`](history/).

## 1. Crate layout rule (unchanged)

Every `lib.rs` and every `mod.rs` contains only crate documentation, `mod` declarations and
`pub use` re-exports; binaries keep `main.rs` to module declarations and the entry point.
Implementation lives in one file per responsibility. Every public and crate-visible item has
rustdoc (`missing_docs = deny`, `RUSTDOCFLAGS=-D warnings`).

## 2. Module map (0.5.0 additions in bold)

| Crate | Modules (responsibility) |
|---|---|
| `ace-core` | `codec` (**`CodecId::TimeSeries = 4`**), `plan`, `stats` (**Float lane / TS1 counters**), `numeric`, `config` (**`enable_float_specialization`**), `limits`, `error` (**`InvalidTimeSeries`**), `profile`, `block`, `dictionary` |
| `ace-simd` | `backend` (`ACE_SIMD=scalar`), `scan`, `avx2` (unsafe), `crc32c` |
| `ace-bitpack` | `lane`, `zigzag`, `width`, `scalar`, `delta`, `delta_of_delta`, `for_codec`, **`bitstream`** (public LSB-first `BitWriter` / `BitReader`; replaces the private `bit_io`) |
| `ace-analysis` | block statistics, numeric detection / prefilter, block-size advisor, **`float`** (`FloatWidth`, `FloatProfile`, `float_prefilter`, `admit_float`, `lane_sample_windows`), **`run_profile`** (`RunProfile`, `run_prefilter`) |
| `ace-codecs` | `dispatch`, `raw`, `rle`, `lz`, `numeric/`, **`time_series/`** (`mode`, `header`, `sink`, `gamma`, `gorilla`, `run_delta`, `codec`) |
| `ace-entropy`, `ace-transforms` | unchanged |
| `ace-format` | `header` (**`FORMAT_MINOR = 4`, `minimal_minor_version`, plain-TS1 rule**), `index`, `checksum`, `block_io`, `reader`, `writer` |
| `ace-index` | `index_reader` |
| `ace-cost` | `estimator`, `cost_model`, `sampling`, `quality`, **`time_series`** (`estimate_gorilla`, `estimate_run_delta`, `TimeSeriesEstimate`) |
| `ace-planner` | V4.3 modules + **`float_route`** (`classify_float_lane`, `float_fast_decision`, `apply_time_series_policy`, `FloatEvidence`, thresholds); `route` (**Float routes, `base_route`, `candidate_route()`**), `decision` (**`TimeSeriesSelection`**, telemetry) |
| `ace-engine` | `engine`, `block_encoder` (**Planner V5 `plan_block`, TS1 payload hand-over**), `block_pipeline`, `container` (minimal version, **TS1 statistics**), `indexed`, `chunker`, `explain` (**route + TS1 estimate**) |
| `ace-stream` | `limits`, `encoder` (**`Write + Seek` sink, in-place 1.4 header rewrite**), `decoder` |
| `ace-runtime`, `ace-dictionary` | unchanged |
| `ace-cli` | `args` (**`--disable-float`**), `files`, `commands/*` (**TS1 in `inspect`, route in `explain`**) |
| `ace-corpus` | `rng`, `workload` (**Corpus V4**), `generators` (Corpus V3), **`float_generators`**, **`false_positive`**, `bin/ace-corpus` |
| `ace-bench` | Harness V3; `families/` + **`float`** (`float-ablation`, `float`, `float-fastpath`, `float-false-positive`, `float-estimator`) |

## 3. Data flow

```text
encode:  FixedBlockChunker ─► PlanningContext::classify ─► RoutePolicy::classify
           (V4.3 route ─► float_prefilter / run_prefilter ─► V5 route + evidence)
           ─► plan_block: NumericFast | FloatFast (one TS1 encode, bounded fallback)
                          | analysis + V4.3 pipeline of the base route ─► TS1 policy
           ─► block_encoder (NUM1 / TS1 / generic payload, RAW fallback)
           ─► assemble_container: header with the minimal minor version (1.3 | 1.4)
                                  · blocks · index · trailer          [rayon, ordered output]

decode:  AceReader / AceIndexReader ─► read_serialized_block (version and plain-TS1 checks)
           ─► block_pipeline::decode_encoded_block
                ├ split_entropy_metadata ─ decode_entropy_cow ─ decode_codec (… | ts1_decode)
                ├ invert_transform*
                └ CRC32C
           ─► AceEngine::decode_blocks ─► Write sink
```

## 4. Compatibility guards

0.4.6 froze behaviour; 0.5.0 changes behaviour only where the Float lane wins, and proves
it:

| Guard | Mechanism |
|---|---|
| 0.4.x corpus untouched | `examples/golden/0.4.6/GOLDEN.json` frozen, checked by `corpus_v3_matches_0_4_6_golden`; `float-false-positive` benchmark (Corpus V3 identical with the lane on and off) |
| 0.5.0 semantics | `examples/golden/0.5.0/GOLDEN.json` (27 workloads × 3 profiles, with the declared format version); regenerate only with a documented semantic change |
| 0.4.x readers | minimal-version writer: 1.3 unless a block is TS1; `--disable-float` reproduces 0.4.6 |
| no ratio regression | `corpus_v4_never_regresses`; Regression V3 `float.min_ratio_gain_vs_disabled` |
| backends | golden with `ACE_SIMD=scalar`; TS1 is scalar Rust |

## 5. Benchmark Harness V3 (unchanged mechanics)

New families plug into the `FAMILIES` registry; Float lane cases in `release-performance`
measure the same engine with the lane disabled in the same run, so their gates are
within-process ratios. The interleaved A/B against 0.4.6 covers the Corpus V3 cases, where
byte identity with 0.4.6 is required (Float cases differ from 0.4.6 by design).

## 6. Principles applied in 0.5.0

| Principle | Example |
|---|---|
| Single responsibility | TS1 split into `mode` / `header` / `sink` / `gamma` / `gorilla` / `run_delta` / `codec`; float route logic isolated in `float_route` |
| DRY | one bitstream for NUM1 and TS1; one encoder for size and payload (`BitSink`: `BitWriter` or `BitCounter`); one set of sample windows for prefilters and estimators; one false-positive corpus for tests and benchmarks |
| Open/closed | a new route reuses V4.3 by delegation (`candidate_route()`), not by editing its rules; TS1 modes 4–15 reserved |
| KISS / YAGNI | no SIMD Gorilla, no ALP, no new `DecodeLimits` fields (TS1 validation is structural) |
| Measure first | thresholds derived from `float-ablation` before the planner existed ([`FLOAT-CALIBRATION-0.5.0.md`](FLOAT-CALIBRATION-0.5.0.md)) |
| Safety | no new `unsafe`; TS1 decoding validates the header before allocating and reads through a bounds-checked reader |

Details: [`PLANNER-V5.md`](PLANNER-V5.md), [`TS1-CODEC.md`](TS1-CODEC.md),
[`FORMAT-1.4.md`](FORMAT-1.4.md).
