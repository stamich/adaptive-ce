# ACE 0.5.0 — implementation order (Lossless Floating-Point & Time-Series Compression)

Base: 0.4.6. Concept: `AdaptiveCE_0_5_0_Koncepcja_v2.md` (corrected version of the original
plan). Rules for every task: `cargo clippy --workspace --all-targets -- -D warnings` clean,
rustdoc on every item, one task = one commit, and **the frozen 0.4.6 golden file keeps
passing** (Corpus V3 bytes unchanged) after every step.
Status: ✅ done in this release · ⏩ executed by the release pipeline on the reference machine.

## Phase A — baseline and version

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 1 ✅ | 0.4.6 benchmark baseline (A/B and regression reference) | Adds ACE 0.4.6 benchmark baseline | `examples/baselines/0.4.6/` + `BASELINE.json` |
| 2 ✅ | Version 0.5.0, `MILESTONE-0.5.0.json` | Bumps version to 0.5.0 | `ace --version` = 0.5.0 |
| 3 ✅ | Scripts / tools renamed to 0.5.0, baseline 0.4.6 | Renames scripts and tools to 0.5.0 | package name audit |

## Phase B — bitstream

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 4 ✅ | Public LSB-first `BitWriter` / `BitReader` (NUM1 moved onto it, bytes unchanged) | Adds public LSB-first bitstream to ace-bitpack | unit tests, golden 0.4.6 |
| 5 ✅ | `bitstream_roundtrip` fuzz target | Adds bitstream fuzz target | fuzz project builds |

## Phase C — Format 1.4

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 6 ✅ | `CodecId::TimeSeries = 4`, `AceError::InvalidTimeSeries`, reading 1.4 | Adds TimeSeries codec id and Format 1.4 reading | TS1 rejected below 1.4 |
| 7 ✅ | Minimal-version writer (memory + streaming header rewrite, `Write + Seek`) | Adds minimal-version writer for Format 1.4 | stream == memory |
| 8 ✅ | Reader tests 1.0 – 1.4 | Adds Format 1.4 reader tests | `format_1_4.rs` |
| 39 ✅ | Plain TS1 blocks only (no transform / entropy / dictionary) | Rejects TS1 blocks with transforms, an entropy stage or a dictionary | `format_1_4.rs` |

## Phase D — TS1 codec

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 9 ✅ | TS1 header + Gorilla f64 / f32 (`BitSink`: exact size by construction) | Adds TS1 header and Gorilla XOR codec | bit-exact special values |
| 10 ✅ | RunDelta (Elias-gamma runs + ZigZag deltas, 2 / 4 / 8-byte lanes) | Adds TS1 RunDelta mode | malformed stream table |
| 11 ✅ | Dispatcher, property tests, `ts1_decode` / `ts1_roundtrip` fuzz targets | Wires TS1 into the codec dispatcher; adds property and fuzz tests | 15 fuzz targets |

## Phase E — measurement before the planner

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 12 ✅ | Corpus V4 (12 workloads, IEEE `+ − × ÷` only) | Adds Corpus V4 to ace-corpus | generator stability test |
| 15 ✅ | `float-ablation` family | Adds float-ablation benchmark family | every candidate per block |
| 16 ✅ | Engine result per profile in the ablation | Measures every profile in float-ablation | calibration input |

## Phase F — analysis

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 17 ✅ | `FloatProfile`, `float_prefilter`, width selection | Adds float lane prefilter to ace-analysis | V4 admitted, V3 + FP rejected |
| 18 ✅ | `RunProfile`, `run_prefilter`; splat exclusion | Adds run-lane prefilter for RunDelta; Excludes byte runs from the RunDelta prefilter | Corpus V3 untouched |
| 19 ✅ | Threshold calibration document | Documents the Float lane calibration | `docs/FLOAT-CALIBRATION-0.5.0.md` |

## Phase G — estimators

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 20 ✅ | `estimate_gorilla` (sampled), `estimate_run_delta` (exact), `ts1_stream_bits` | Adds TS1 size estimators to ace-cost; Updates the fuzz lockfile … | estimate within 10 % on smooth data |

## Phase H — Planner V5

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 21–23 ✅ | Float routes, `FloatEvidence`, classification order, TS1 policy, telemetry, `enable_float_specialization` | Adds Planner V5 Float lane routes and TS1 policy | disabled = V4.3 decision |
| 25 ✅ | Sampled-estimate dominance at 75 % | Tightens TS1 dominance for sampled Gorilla estimates | no V4 regression |

## Phase I — engine, CLI, streaming

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 26 ✅ | `plan_block` V5, TS1 payload hand-over, statistics | Wires Planner V5 into the block encoder | golden 0.4.6 passes |
| 27 ✅ | `explain` on the production path; `float_lane` tests | Explains Planner V5 decisions and tests the Float lane end to end | V3 untouched, V4 never larger |
| 28 ✅ | `--disable-float`, TS1 in `inspect`, route in `explain` | Adds --disable-float and TS1 output to the CLI | CLI round trip |
| 29 ✅ | Streaming header rewrite test; Float workloads in the determinism matrix | Adds Float lane workloads to streaming and determinism tests | identical bytes |

## Phase J — correctness guards

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 30–31 ✅ | Golden 0.5.0 (27 workloads, format version); frozen golden 0.4.6 | Adds golden 0.5.0 and keeps the 0.4.6 golden as a frozen guard | auto + scalar |
| 32 ✅ | Format 1.4 / TS1 fixture in the malformed matrix | Adds a Format 1.4 / TS1 fixture to the malformed matrix | rejected, never misdecoded |
| 33 ✅ | False-positive corpus | Adds the Float lane false-positive corpus to ace-corpus | 0 admitted |

## Phase K — benchmarks and gates

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 34 ✅ | `float`, `float-fastpath`, `float-false-positive`, `float-estimator` | Adds float, float-fastpath, float-false-positive and float-estimator benchmarks | schema 2.1 valid |
| 35 ✅ | Float cases in `release-performance` (same-run comparison) and `memory` | Adds Float lane cases to release-performance and memory | — |
| 36 ✅ | Regression V3 `float` section; report table | Adds Float lane gates to Regression V3 and the performance report | quick run: all float gates PASS |
| 37 ✅ | Release pipeline and demo | Updates release pipeline and demo for the Float lane | demo < 1 min |

## Phase L — documentation and release

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 38 ✅ | 0.4.6 docs → `docs/history/`, audits, SECURITY | Moves 0.4.6 documents to docs/history and refreshes the audits | links resolve |
| 40 ✅ | `FORMAT-1.4`, `TS1-CODEC`, `FORMAT-COMPATIBILITY` | Documents Format 1.4 and the TS1 codec | review |
| 41 ✅ | `PLANNER-V5`, `ARCHITECTURE-0.5.0` | Documents Planner V5 and the 0.5.0 architecture | review |
| 42 ✅ | `BENCHMARK-METHODOLOGY-0.5.0`, `RELEASE-CHECKLIST-0.5.0` | Adds the 0.5.0 benchmark methodology and release checklist | review |
| 43 ✅ | `RELEASE-NOTES-0.5.0`, `MIGRATION-0.4-TO-0.5` | Adds 0.5.0 release notes and the 0.4 to 0.5 migration guide | review |
| 44–46 ✅ | README, ROADMAP, CHANGELOG, this file, milestone summary | Updates README / roadmap / changelog … | review |
| 47 ✅ | `PERFORMANCE-0.5.0.md` from a full Harness V3 run + interleaved A/B vs 0.4.6 on the development VM | Adds the 0.5.0 performance report | generated |
| 48 ⏩ | Fuzz campaign 15 × 10 min | `ACE_FUZZ_SECONDS=600 ./ace-ci0.5.0.sh fuzz` | 0 crashes |
| 49 ⏩ | Full release run on the reference machine; publish `examples/baselines/0.5.0/`; harden the indicative Float speed floors | `./ace-release0.5.0.sh <0.4.6 tree>` | checklist all PASS |

Task numbers follow the concept's phase plan; 13–14 and 24 were merged into neighbouring
tasks (Corpus V4 generators and float-family workloads; route budgets live with the routes).
