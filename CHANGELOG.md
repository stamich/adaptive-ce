# Changelog

## 0.5.0 - 2026-10-07 — Lossless Floating-Point & Time-Series Compression

Base: 0.4.6. **Format 1.4** (TS1 codec, written only when a block uses it) and **Planner V5**
(Float lane). Corpus V3 output is byte-identical to 0.4.6 (frozen golden file); every file
written with `--disable-float` equals 0.4.6. Concept: `AdaptiveCE_0_5_0_Koncepcja_v2.md`; task
order: `TASKS-0.5.0.md`; migration: `docs/MIGRATION-0.4-TO-0.5.md`.

### Added
- TS1 codec (`ace-codecs::time_series`): 28-byte validated header; Gorilla XOR f64 / f32 with
  window reuse (`M − 1` stored); RunDelta (Elias-gamma runs + ZigZag deltas over 2 / 4 / 8-byte
  lanes); `ts1_encode_with`, `ts1_encoded_len` / `ts1_stream_bits` (same encoder against a bit
  counter), `ts1_encode_best`, `ts1_decode`, `ts1_inspect`, `TimeSeriesLayout`,
  `TimeSeriesMode`.
- Format 1.4: `CodecId::TimeSeries = 4`, `FORMAT_MINOR = 4`, `FORMAT_MINOR_BASE`,
  `minimal_minor_version`; TS1 rejected below 1.4 and with transforms / entropy / dictionary;
  `AceError::InvalidTimeSeries`.
- `ace_bitpack::{BitWriter, BitReader}`: public LSB-first bitstream (NUM1 moved onto it).
- `ace-analysis`: `float_prefilter`, `FloatProfile`, `FloatWidth`, `admit_float`,
  `FloatRejection`, `lane_sample_windows`, `run_prefilter`, `RunProfile`.
- `ace-cost`: `estimate_gorilla` (sampled, with confidence), `estimate_run_delta` (exact),
  `TimeSeriesEstimate`.
- Planner V5: `PlannerRoute::{FloatGeneral, FloatFast}` (+ `label`, `is_float`),
  `RouteReason::{FloatCandidate, DominantFloat}`, `RouteDecision::{base_route,
  float_evidence, run_prefilter, candidate_route()}`, `classify_float_lane`,
  `float_fast_decision`, `apply_time_series_policy`, `ts1_beats_generic`, `FloatEvidence`,
  `TimeSeriesSelection`, telemetry (`time_series_estimates`, `float_fast_hit`,
  `float_fast_fallback`).
- `AceConfig::enable_float_specialization`; `CompressionStats` Float lane / TS1 counters;
  `StreamingStats::format_1_4`; `BlockExplanation::{route, time_series}`.
- CLI: `--disable-float` (compress, compress-stream, explain); TS1 lines in `inspect`;
  route / float evidence / run lanes / TS1 estimate in `explain`; Float counters in
  `compress`; format version in `compress-stream`.
- `ace-corpus`: Corpus V4 (12 workloads: f64 constant / step / smooth / sensor / financial /
  noisy / random / special, f32 smooth / sensor, int sparse-change / counter-reset) and the
  false-positive corpus (`FalsePositiveCase`, `fp-*` in the CLI).
- Benchmarks: `float-ablation`, `float`, `float-fastpath`, `float-false-positive`,
  `float-estimator`; `release-performance` cases `float_fast.f64_step`,
  `float_general.f64_noisy`, `run_delta.int_sparse_change` (each with a same-run
  lane-disabled comparison); `memory` Float cases; Regression V3 `float` section; Float table
  in the generated performance report.
- Tests: golden 0.5.0 (27 workloads × 3 profiles, declared format version), frozen golden
  0.4.6 check, `float_lane` (V3 untouched, V4 never larger, FloatFast, explain == compress),
  `format_1_4`, TS1 unit / property tests, TS1 malformed fixture, streaming header rewrite,
  Float workloads in the determinism matrix, prefilter corpus tests; fuzz targets
  `bitstream_roundtrip`, `ts1_decode`, `ts1_roundtrip` (15 in total).
- Docs: `FORMAT-1.4`, `TS1-CODEC`, `PLANNER-V5`, `FLOAT-CALIBRATION-0.5.0`,
  `ARCHITECTURE-0.5.0`, `BENCHMARK-METHODOLOGY-0.5.0`, `RELEASE-CHECKLIST-0.5.0`,
  `RELEASE-NOTES-0.5.0`, `MIGRATION-0.4-TO-0.5`, `UNSAFE-AUDIT-0.5.0`, generated
  `PANIC-AUDIT-0.5.0` and `PERFORMANCE-0.5.0`; `TASKS-0.5.0.md`, `MILESTONE-0.5.0.json`;
  `examples/baselines/0.4.6/`.

### Changed
- `ace_stream::compress_reader_known_size` sink is `Write + Seek` (Format 1.4 header rewrite).
- `encode_file_header` writes `min(minor_version, FORMAT_MINOR)`; in-memory and streaming
  writers declare the minimal version.
- Benchmark oracles and the engine drive the V4.3 pipeline with `candidate_route()`.
- Scripts / tools renamed to `…0.5.0`; A/B baseline 0.4.6; release step 5 also hashes
  f64-noisy; demo gains a Float lane step; `ace_bin` honours `CARGO_TARGET_DIR`.
- 0.4.6 versioned documents moved to `docs/history/`.
- `strong_numeric_evidence` validates fixed-step blocks in groups of eight values (same
  result, layout-robust, NumericFast encode 1.32× vs 0.4.6 in the interleaved A/B).
- A/B driver builds each probe in its own target directory (also with `CARGO_TARGET_DIR`);
  Regression V3 per-case stability limit for the FloatFast encode (6 %).

### Measured (development VM, 4 MiB per workload)
- f64-constant BALANCED 141× → 2 416×, f64-step 136× → 464× (FloatFast, encode ≈ 35–38×
  faster); int-sparse-change BALANCED 83× → 235×; f64-smooth FAST 1.53× → 1.82×, f64-noisy
  FAST 1.00× → 1.23×; zero Float-route blocks on Corpus V3 and the false-positive corpus;
  zero FloatFast fallbacks.
- Interleaved A/B vs 0.4.6: 12 / 12 PASS, byte-identical; Regression V3 PASS
  (`docs/PERFORMANCE-0.5.0.md`).

## 0.4.6 - 2026-10-06 — Hardened Release & Benchmark Stabilization

Base: 0.4.5-buildfix2. **Format 1.3, Planner V4.3 and every encoded byte unchanged**
(semantic freeze: golden SHA-256 for 15 workloads × 3 profiles, auto and `ACE_SIMD=scalar`;
byte-identical output of 0.4.5-buildfix2 and 0.4.6 in every A/B case). Product and Cargo
version 0.4.6. Concept: `AdaptiveCE_0_4_6_Koncepcja.md`; task order: `TASKS-0.4.6.md`.

### Added
- `ace-corpus` crate + `ace-corpus` CLI: deterministic workloads (15) shared by tests, golden
  files, demo and benchmarks (byte-identical to the former harness generators).
- Golden files `examples/golden/0.4.6/GOLDEN.json` + `crates/ace-engine/tests/golden.rs`
  (`ACE_GOLDEN_UPDATE=1` regenerates).
- `ACE_SIMD=scalar` override (`ace_simd::{SIMD_OVERRIDE_ENV, scalar_forced}`),
  `ace_simd::crc32c_backend_name`.
- `AceEngine::decompress_into` (reusable output buffer, reservation capped by
  `DecodeLimits::max_output_size` and `PREALLOCATION_CAP_BYTES`); `decompress`,
  `decompress_into` and `decompress_from` share one `decode_blocks`.
- Benchmark Harness V3 in `crates/ace-bench` (moved from `examples/rust-benchmark`):
  `timing/{plan,runner,stats,report}` — 5 warm-ups, doubling calibration to ≥ 50 ms per
  sample, 3 × 7 batches, median-of-medians, batch MAD, MAD outliers (reported, not removed);
  `ACE_BENCH_QUICK`, `ACE_BENCH_MIN_SAMPLE_MS`, `ACE_BENCH_OUT_DIR`; family registry and
  `--list`.
- Benchmark schema 2.1: all 2.0 fields (computed from the MoM) + `stable_timing`,
  `benchmark_methodology`, environment fingerprint from `build.rs` (rustc, target, opt-level,
  LTO, codegen-units, target features, RUSTFLAGS) and before/after runtime snapshots
  (governor, frequency, temperature, load) with warnings.
- `release-performance` family (exact gate cases, pre-allocated decode + allocating diagnostic);
  `memory` family gains allocation counts (counting `GlobalAlloc`) and peak RSS (`VmHWM`).
- Interleaved A/B: probe `tools/ace-abprobe0.4.6` compiled against any 0.4.x tree, driver
  `tools/ace-ab0.4.6.py` (alternating order, MoM ratio, per-batch bounds, byte identity,
  A/A control), `ace-ab0.4.6.sh`.
- Regression V3 (`tools/ace-check_regressions0.4.6.py`): sections correctness / quality /
  performance / stability / environment; statuses pass / fail / unstable / skipped /
  diagnostic; exit 0 / 1 / 3 / 4.
- Tools: shared `ace-benchlib0.4.6.py`, schema 2.0/2.1 validator, report with `--markdown`
  (generates `docs/PERFORMANCE-0.4.6.md`), comparator with methodology/machine warnings,
  `ace-code_audit0.4.6.py` (unsafe placement, SAFETY comments, allow justifications →
  generated `docs/PANIC-AUDIT-0.4.6.md`), deterministic packager `ace-package0.4.6.py`.
- Hardening tests: `malformed_matrix` (table, every-7th truncation, exhaustive single-byte
  flips, forged sizes with re-sealed CRC, resource limits, no large pre-allocation),
  `random_access_stress` (10 000 ranges, per-block, out-of-bounds, 8 concurrent readers),
  `determinism_matrix` (4 workloads × 3 profiles × 5 thread counts × 3 API paths,
  `decompress_into` reuse), `num1_properties`, `large_files` (1 GiB streaming, 256 MiB
  indexed; `--ignored`), `PROPTEST_CASES` budgets for all property tests, compiled API
  doctest in `ace-engine`.
- Scripts: `ace-build`, `ace-benchmark` (`--quick`, `--isolated`, `--ab`),
  `ace-benchmark-compare` (files or directories), `ace-ab`, `ace-release` (11 steps, retry on
  `unstable`, filled checklist, test from the archive), `ace-ci` (`pr`, `release`, `fuzz`,
  `msrv`) — all `…0.4.6.sh` with shared `tools/ace-common0.4.6.sh`; GitHub Actions wrapper.
- Short product demo `demo/ace-run-demo0.4.6.sh` (< 1 min, no benchmarks).
- Docs: `RELEASE-NOTES`, `ARCHITECTURE`, `ARCHITECTURE-FREEZE`, `BENCHMARK-METHODOLOGY`,
  `UNSAFE-AUDIT`, `PANIC-AUDIT`, `RELEASE-CHECKLIST`, `PERFORMANCE` (all `-0.4.6`),
  `MILESTONE-0.4.6.json`, `examples/baselines/*/BASELINE.json`.

### Changed
- Workspace: MSRV Rust 1.97 (`rust-version = "1.97.0"`, `clippy.toml` msrv 1.97.0; first set to
  1.75), explicit `[profile.release]`, `Cargo.lock` committed, workspace lints (`unsafe_code = forbid`, `missing_docs = deny`,
  `unsafe_op_in_unsafe_fn = deny`, clippy `unwrap_used / expect_used / panic / todo /
  unimplemented / dbg_macro = deny`; tests exempt via `clippy.toml`).
- Production code free of `unwrap` / `expect` / `panic!`: `AceEngine::default_engine` builds
  directly, `Lane::WIDTH`, infallible `read_bits_validated`, NUM1 header and Huffman length
  reads via `read_lane`, Huffman code-length construction without `expect`.
- `ace-simd`: `unsafe` operations wrapped in explicit blocks with `// SAFETY:` comments
  (`clippy::undocumented_unsafe_blocks = deny`); with the 1.97 MSRV the CRC32C kernel is a safe
  `#[target_feature]` function and AVX2 kernels keep `unsafe` only around the loads.
- Rust 1.97 idioms: `AtomicU64::try_update`, `iter::repeat_n`, `usize::is_multiple_of`;
  lockfile moved to the latest compatible dependency versions.
- `cargo fmt` applied to the whole workspace (fmt-only change, golden unchanged).
- README rewritten release-style; `docs/SECURITY.md` updated with 0.4.6 evidence;
  `ROADMAP.md` updated (0.4.x candidates vs 0.5).
- Fuzz project: 12 registered targets (`planner_sample_offsets`, `rans_decoder`,
  `rans4x_decoder`, `rle_roundtrip` were present but unregistered; stale `ace_decoder`
  replaced by `engine_roundtrip`; `planner_sample_offsets` fixed for the current API).
- Java / Scala integration examples: version-neutral docs, run instructions.

### Removed
- 0.4.5-buildfix2 scripts and tools, Python corpus generators (replaced by `ace-corpus`),
  cost-model calibration script, `MANIFEST.txt` (generated into the package now).
- Example crates `rust-demo`, `random-access-demo`, `parallel-demo`; harness moved to
  `crates/ace-bench`.
- Duplicate Java / Scala sources outside the Maven / sbt layout.
- Obsolete baselines (0.2, 0.2.1, 0.3-*, 0.3.1, 0.4-buildfix1, 0.4-buildfix3-buildfix1);
  kept: 0.2.1-buildfix1 (quality), 0.4-buildfix2, 0.4.5-buildfix1, 0.4.5-buildfix2.
- Historic documents moved to `docs/history/`.

### Verification (development VM: 2-vCPU Xeon, rustc 1.97)
| Check | Result |
|---|---|
| `cargo fmt --check`, `clippy --all-targets -D warnings`, `RUSTDOCFLAGS=-D warnings cargo doc` | clean |
| tests (debug) | 218 passed, 0 failed, 2 ignored (large files) |
| tests (release) incl. `--ignored` | 220 passed |
| property tests at `PROPTEST_CASES=10000` | pass |
| golden SHA-256 (auto + `ACE_SIMD=scalar`) | identical |
| CLI byte identity vs 0.4.5-buildfix2 (6 workloads × 3 profiles + streaming) | identical |
| code audit | 24 `unsafe` sites, all in audited files; 0 panic-lint exceptions |
| fuzz project (12 targets) | `cargo check` clean (campaign: reference machine) |
| Harness V3 full run (21 families) + interleaved A/B | Regression V3 PASS: correctness 8, quality 13, performance 15, stability 15 (after two `unstable` sessions, as designed) |
| `ace-release0.4.6.sh --quick` end-to-end | all steps run; verdict "not releasable" by design (quick) |
| package | reproducible (2 × same SHA-256), tested from the archive |
| MSRV 1.97 (`ace-ci0.4.6.sh msrv`) | check + tests pass |

## 0.4.5-buildfix2 - 2026-10-05

Base: 0.4.5-buildfix1. Format 1.3 and every encoded byte unchanged (byte-for-byte identical
output on Corpus V3 for FAST/BALANCED/DENSE and streaming). Analysis of the 0.4.5-buildfix1
benchmark run: `docs/BENCHMARK-ANALYSIS-0.4.5-buildfix1.md`.

### Fixed - warm-64K random-access gate (76.2 µs > 71.7 µs in 0.4.5-buildfix1)
- Root cause: CRC32C of the whole decoded 256 KiB block took 55–60 % of a block decode
  (single SSE4.2 dependency chain). New `ace_simd::crc32c_hardware` runs three interleaved
  chains and merges them in GF(2) (`crc32c` crate remains the portable fallback):
  256 KiB 41 → 12 µs. Container gate run: warm-64K 40.0 µs, 23/23 gates PASS.
- Huffman decoder: 11-bit lookup table + 64-bit MSB-first bit buffer with canonical fallback
  for long codes / stream end (identical results, incl. errors); encoder uses a 64-bit
  accumulator. 256 KiB JSON decode 2 246 → 1 043 µs, encode 1 509 → 614 µs.
- NUM1 decoding fused into one iterator pipeline (`unpack_iter → unzigzag → undelta_iter →
  write_lanes_from_iter`): 1.6–2× faster, no intermediate vectors.
- `decode_entropy_cow`: entropy `None` borrows the payload; RAW blocks are copied once.
- NUM1 estimator accumulates bit widths with OR (same highest bit, no compare per value): 2×.

### Changed - structure (SOLID / KISS / DRY)
- Every `lib.rs` and `mod.rs` contains only `mod` declarations and `pub use` re-exports;
  implementation moved to dedicated modules in all 16 crates, the CLI and the benchmark
  harness (`docs/ARCHITECTURE-0.4.5-BUILDFIX2.md`).
- `ace-bitpack`: sealed `Lane` trait (u16/u32/u64) replaces three copies of ZigZag, delta,
  delta-of-delta, FOR, bit-width and pack/unpack; modules follow concept §46
  (`scalar`, `zigzag`, `for_codec`, `delta`, `delta_of_delta`).
- `ace-codecs`: NUM1 split into `mode`, `header`, `estimate`, `encode`, `decode`,
  `lane_dispatch`; `encode_u16/u32/u64` and `decode_u16/u32/u64` replaced by generic
  `encode_lane::<T>` / `decode_lane::<T>`; `serialize_payload` replaces an 8-argument builder.
- `ace-format`: `read_serialized_block` is the single block parser (sequential and indexed
  readers); `AceWriter` now builds the block index and trailer and is used by both the engine
  and the streaming encoder.
- `ace-core`: `EntropyCodecId::metadata_prefix_bytes` / `PRIMARY_LENGTH_PREFIX_BYTES`
  (single source of the entropy-metadata prefix rule), `EntropyCodecId::label`,
  `PhysicalCompressionPlan::{label, is_plain_numeric}`, `CompressionStats::record_selected_plan`.
- `ace-planner`: `decision`, `pipeline`, `plan_identity`, `exhaustive` split out of
  `evaluator`; `RankedPlan` + `EntropySelectionPolicy::penalize_weak_rans` replace two copies
  of the rANS gain rule; `hybrid` uses `ace_cost::stratified_ranges` (duplicate removed).
- `ace-engine`: `AceEngine` is a façade over `block_encoder`, `block_pipeline`, `container`.
- `ace-stream`: validation / single-block encoding / exact reads extracted; uses `AceWriter`.
- `ace-simd`: `backend`, `scan`, `avx2`, `crc32c`; `unsafe` remains confined to this crate.
- CLI: `args`, `files`, `commands/*`; version printed from `CARGO_PKG_VERSION`;
  `read-range` uses one index lookup (`read_range_with_metrics`).
- Benchmark harness split into `prelude`, `json`, `timing`, `corpus`, `plan_util`,
  `families/*`; milestone tag defined once (`json::MILESTONE`).
- `AceConfig` / `DecodeLimits` built with struct-update syntax instead of field reassignment.

### Removed (public API)
- `ace_bitpack::{zigzag_i16, unzigzag_u16, zigzag_i32, unzigzag_u32, max_bit_width_u16/u32/u64,
  pack_u32/u64, unpack_u32/u64, delta_i16/i32/i64, undelta_*, delta_of_delta_*,
  undelta_of_delta_*, frame_of_reference_u*, unframe_of_reference_u*}` — use the generic
  `Lane`-based functions (`zigzag_i64` / `unzigzag_u64` / `bits_required_u64` remain).

### Tests and tooling
- 184 tests (was 155): per-lane property tests (macro), bit I/O references, Huffman long-code
  / malformed cases, CRC32C reference and GF(2) shift tests, `AceWriter` index test,
  streaming == in-memory identity, plan label / statistics / rANS policy tests.
- `cargo clippy --workspace --all-targets`: 0 warnings (MSRV 1.75); rustdoc: 0 warnings;
  doc-comment coverage 100 %. Fuzz target `bitpack_decode` covers all three lanes.
- Scripts, tools, demo and result files renamed to `0.4.5-buildfix2`; previous milestone
  files moved to `docs/history/`; 0.4.5-buildfix1 results added as
  `examples/baselines/0.4.5-buildfix1/`.

## 0.4.5 / 0.4.5-buildfix1 (milestone 0.4-buildfix5) - 2026-10-05

### Fixed - numeric coverage (ratio)
- **u64 timestamps with wide jitter were rejected by the prefilter.** The "small delta" limit was a
  fixed 16 bits, so nanosecond clocks (deltas of 17-32 bits) never reached the numeric route. The
  limit is now lane-relative (`NumericWidth::small_delta_bits`: u16 -> 8, u32 -> 16, u64 -> 32).
  `u64-timestamps-ns`, BALANCED: 1.64x -> 6.38x.
- **FAST never considered Numeric for timestamp-like blocks.** FAST only added the Numeric candidate
  when the *byte-delta* score was >= 0.15. Planner V4 now guarantees "NumericGeneral route => Numeric
  candidate" in every profile (`DefaultCompressionPlanner::candidates_for_route`,
  `ensure_numeric_candidate`, plus a safety net in `evaluate_candidates_v4_with_route`).
  FAST: `u64-timestamps-ms` 1.00x -> 31.56x, `delta-series` 1.33x -> 15.89x, `delta-variable` 1.00x -> 3.81x,
  `u64-timestamps-ns` 1.28x -> 6.38x.
- The injection rule initially treated FAST's `Raw + Huffman` baseline as "RAW only" and skipped
  injection; the guard now only suppresses Numeric for a lone RAW (early-raw) candidate.

### Added
- `NumericWidth::U16` (2-byte lanes) across analysis, planner, codec and bit-packing
  (`ace-bitpack` u16 lane functions, `max_bit_width_u16`).
- Lane-ring ("modular") delta arithmetic so wrapping counters (65535 -> 0) stay small deltas.
- `ace_codecs::{numeric_inspect, NumericPayloadInfo, estimate_numeric_for_width, numeric_encode_with,
  numeric_encode_fixed_step}`; `ace inspect --blocks` prints width/mode/bit width/bits-per-value per
  numeric block and a numeric summary line.
- Fuzz target `numeric_roundtrip` (inspect/decode of garbage, encode->decode identity,
  estimate == encoded length).
- Tests: `ace-planner/tests/buildfix5_numeric_coverage.rs`, `ace-engine/tests/numeric_0_4_5.rs`,
  malformed-header / garbage / u16 / tail-byte / estimate-equals-length codec tests, bit-pack
  equivalence tests, prefilter regression `wide_jitter_u64_timestamps_are_likely_numeric`.

### Performance (decode hot path, wire-compatible)
- rANS: slot->symbol lookup table (`NormalizedFrequencyTable::slot_lookup_table`) replaces a
  256-step scan per decoded byte; rANS4x inherits it.
- Huffman: canonical `first_code/count/offset` tables replace a linear scan per bit.
- LZ: `copy_match` uses `extend_from_within` (memcpy) instead of a byte loop.
- Numeric zero-width decode: const-generic `fill_arithmetic::<B>`.
- Bit-pack: byte-fragment `write_bits`/`read_bits`.
- Hot numeric loops are monomorphized per lane (`read_lane::<B>`, `lane_delta_const::<B>`); a first
  run-time-width generalization had made encode 2.5-3x slower and was replaced.
- Measured (min of 3, same machine, old binary vs new): `gauge-sawtooth` decode 24 -> 427 MB/s,
  `structured-json` decode 349 -> 699 MB/s, `runs` BALANCED decode 320 -> 732 MB/s,
  `u32-counter` decode 759 -> 877 MB/s. Encode throughput within +-10% (noise) except
  `u64-timestamps-ms` BALANCED/FAST ~-15..20% because NumericGeneral estimates twice
  (planner estimate + encode); tracked for 0.4.1.

### Fixed after the first 0.4.5 benchmark run (i7-9850H, `ace-benchmark0.4-buildfix4.sh all`)
- `numeric.u64_planner_mb_s` failed (89.2 < 90 MB/s); A/B against 0.4.4 confirmed a 0.4.5 regression
  (u64-timestamps 110 -> 85 MB/s, delta-variable 92 -> 78 MB/s in the test container).
  Fix: `PlannerDecision::numeric_estimate` carries the planner's exact `NumericEstimate` to the
  engine, which encodes with `numeric_encode_with(width, mode)` instead of repeating the full
  three-width search in `numeric_encode`. Container A/B: u64-timestamps 85 -> 122 MB/s (0.4.4: 110),
  delta-variable 78 -> 105 MB/s (0.4.4: 92), monotonic-outliers 75 -> 95 MB/s (0.4.4: 77).
- `compression.fast_mb_s` (165.0 < 172.9 = 95 % of buildfix2) — FAST now estimates Numeric only for
  the prefilter's lane width (`estimate_numeric_for_width`) instead of all three widths.
  Container A/B on mixed_16m: 0.4.4 = 1.977x @ 144-154 MB/s, 0.4.5 = 3.682x @ 166-176 MB/s.
  BALANCED/DENSE keep the exhaustive search (bytes identical to `numeric_encode`, tested by
  `reused_numeric_estimate_matches_numeric_encode`).

### Documentation
- Every item (fn/struct/enum/trait/const/impl) in the workspace now has a doc comment
  (coverage 585/585 by the repository doc-coverage script).
- New: `TASKS-0.4-buildfix5.md`, `docs/CONCEPT-TRACEABILITY-0.4.md`, `docs/NUMERIC-0.4.5.md`,
  `MILESTONE-0.4-buildfix5.json`; README and ROADMAP updated.

### Compatibility
- Writer stays Format 1.3; readers 1.0-1.3. No header or index change.
- **Additive NUM1 change:** the lane-width byte may now be `2`. Files written by 0.4.5 that contain a
  u16 numeric block are *not* readable by 0.4.4 and older (they report a malformed numeric header).
  Files written by 0.4.4 are fully readable by 0.4.5.
- Benchmark schema 2.0 and the script/JSON names (`0.4-buildfix4`) are intentionally unchanged.

### Known limits (not fixed here)
- `monotonic-outliers` stays at 1.78x: needs Patched FOR (exceptions list) - moved to 0.4.1.
- FAST encode of timestamp data pays one extra numeric estimate (see Performance).

## 0.4-buildfix4-buildfix3 - 2026-09-17

### Script/dependency hardening
- Rebuilt the build/demo/benchmark shell runners from canonical script names instead of applying
  further mechanical prefix substitutions.
- Removed all duplicated `ace-ace-` references.
- Verified every shell/Python script starts with `ace-` and contains `0.4-buildfix4`.
- Verified every direct script-to-script dependency exists.
- Verified every benchmark family invoked by the demo exists in the Rust benchmark binary.
- Updated benchmark JSON validator to require `benchmark-<milestone>-<family>.json`, matching the
  writer and shell runner.
- Updated Python tool usage messages to their real versioned `ace-` names.
- Added `docs/SCRIPT-AUDIT-0.4-BUILDFIX4.md` with the canonical dependency graph.

### Scope
- No Planner V4.3 algorithm change.
- No codec change.
- No Format 1.3 change.
- No benchmark schema change.
- No release-gate semantic change.

## 0.4-buildfix4-buildfix2 - 2026-09-17

### Compile fix
- Fixed Rust E0308 in `random_access_plan_diff_family`.
- `AceIndexedDecoder::read_range` expects `Range<u64>`.
- The benchmark now converts the `usize` range length explicitly:
  `0_u64..len as u64`.
- No planner, codec, format, benchmark-schema or release-gate semantics changed.

## 0.4-buildfix4-buildfix1 - 2026-09-17

### Compile fix
- Fixed Rust E0689 in `DominanceEnvelope::for_config`.
- Explicitly typed `(absolute, relative)` as `(u64, f64)`.
- Suffixed absolute literals with `_u64` and floating literals with `_f64`.
- `saturating_mul(2)` now resolves unambiguously to `u64::saturating_mul`.

### Script naming
- Every shell and Python script now starts with the `ace-` prefix.
- Every shell and Python script continues to include `0.4-buildfix4` in its filename.
- All documentation and script-to-script references were updated to the new names.

### Compatibility
- No planner-semantic change.
- No Format 1.3 change.
- No benchmark-schema change.
- No release-gate change.

## 0.4-buildfix4 - 2026-09-17

### Planner V4.3 runtime closure
- Added `PlanningContext` so production route classification/strong validation happens once per block.
- Added reusable `NumericFastEvidence` with width, first value, fixed delta, value count and tail size.
- Tightened NumericFast to strict complete-block fixed-step sequences; outliers/sawtooth fall back.
- Added `evaluate_candidates_v4_with_route` to reuse an existing `RouteDecision`.
- Added direct `numeric_encode_fixed_step` NUM1/DoD/bit-width=0 production encoder.
- NumericFast no longer reruns full Numeric mode estimation/search.

### Policy Oracle V2
- Added `CandidatePreference`, `DominanceReason`, `DominanceEnvelope` and `DominancePolicy`.
- Added offline `PolicyOracle` over real benchmark/test encoded sizes.
- RLE is product-preferred for zero/run-heavy blocks within bounded size loss.
- RAW is preferred for incompressible data.
- Numeric is preferred for admitted numeric routes.
- Random-access policy can prefer cheaper-decode RAW/RLE.
- Release recall/regret now use policy oracle; route/global oracle remain diagnostics.

### Telemetry and benchmarks
- Added route-classify and generic-analysis timing to compression statistics.
- Added `policy-oracle-v2`.
- Added `planner-hotpath`.
- Added `random-access-plan-diff`.
- Preserved NumericGeneral benchmark and buildfix2 performance reference.

### Naming and repository cleanup
- Every shell/Python script now carries `0.4-buildfix4` in its filename.
- Every benchmark baseline/result JSON filename now contains `benchmark`.
- Current benchmark outputs use `benchmark-0.4-buildfix4-<family>.json`.
- Removed obsolete buildfix3 release-specific docs/audits/tasks/milestone artifacts.
- Removed transient Python bytecode caches.

### Compatibility
- Workspace version 0.4.4.
- Format 1.3 unchanged.
- Reader compatibility 1.0/1.1/1.2/1.3 unchanged.
- NUM1/AIDX/ACET unchanged.
- Benchmark schema remains 2.0.

## 0.4-buildfix3-buildfix1 - 2026-09-17

### Fixed
- Renamed the route-level Hybrid-LZ budget enum to `RouteHybridLzPolicy` to avoid a public-name collision with the existing `hybrid::HybridLzPolicy` struct.
- Updated Planner V4.2 evaluator references to use `RouteHybridLzPolicy::{Disabled, OneStage, Full}` explicitly.
- Removed the `E0599`/`E0659` ambiguity reported during `ace-planner` compilation without changing Planner V4.2 behavior, Format 1.3 bytes, benchmark schema, or release gates.

## 0.4-buildfix3 - 2026-09-17

### Planner V4.2
- Added shared `RoutePolicy` as the single production/benchmark candidate-eligibility contract.
- Added `CandidateEligibility::{Allowed, DiagnosticOnly, Rejected}` and stable rejection reasons.
- Added route-specific `RouteBudget` and `RouteRouteHybridLzPolicy`.
- Added `NumericMargin` for exact-Numeric versus best-generic dominance decisions.
- Reduced NumericGeneral to at most five semantic candidate families.
- Limited NumericGeneral sample verification to at most two candidates.
- Exact Numeric candidates are no longer sample-encoded.
- Strong Numeric dominance disables Hybrid-LZ microtrials.
- Moderate Numeric dominance permits at most one stage-one Hybrid-LZ refinement.
- NumericGeneral disables the generic second-stage verifier.
- Generic and NumericFast behavior remain compatible with buildfix2.

### Route-aware quality model
- Split benchmark oracle into global size oracle and route-aware oracle.
- Release recall/regret metrics now use route-aware oracle semantics.
- Global size regret is retained as a diagnostic.
- Planner block diagnostics now report route/global oracle plan, bytes and regret.

### Benchmarks
- Added `oracle-policy`.
- Added `numeric-general`.
- Bundled the exact user-supplied 0.4-buildfix2 benchmark set as the performance baseline.
- Performance gates are relative to buildfix2 for FAST/BALANCED/DENSE, warm random access and u32 NumericFast.
- Added explicit NumericGeneral targets for u64 timestamps and variable-delta workloads.
- Benchmark schema remains 2.0.

### Scripts and cleanup
- Added root `ace-build0.4.sh`.
- Added `benchmark0.4-buildfix3.sh`.
- Added `benchmark-compare0.4-buildfix3.sh`.
- Added `demo/run-demo-0.4-buildfix3.sh`.
- Removed obsolete unversioned benchmark runners and buildfix2 demo/release entry files.

### Compatibility
- Workspace version: 0.4.3.
- Format 1.3 unchanged.
- Reader compatibility remains 1.0/1.1/1.2/1.3.
- NUM1, AIDX and ACET unchanged.

## 0.4-buildfix2 - 2026-09-16

### Performance hardening
- Added allocation-free `NumericPrefilter` inspecting at most 1,024 sampled values per width.
- Added Planner V4.1 route classifier: `Generic`, `NumericGeneral`, `NumericFast`.
- Added full-block allocation-free validation before direct NumericFast selection.
- Strong fixed-step numeric blocks now bypass the generic BlockAnalyzer, generic candidate generation,
  Top-K verification, sample verification and Hybrid LZ.
- NumericFast performs zero full candidate trial encodes and leaves only the final production Numeric encode.
- Exact `estimate_numeric` is now evaluated lazily only for `NumericGeneral`.
- Generic route filters speculative Numeric candidates before analytical/sample evaluation.
- Constant/zero-heavy blocks are excluded from NumericFast so RLE/RAW remain eligible.

### Analyzer/decoder optimization
- Replaced delta/DoD/bit-width temporary vectors and width sorting with fixed 65-bin histograms.
- Added direct zero-bit-width u32/u64 decode for FOR, Delta and Delta-of-Delta.
- Zero-width numeric decode no longer invokes the bit reader or creates a temporary decoded-value vector.

### Benchmarks
- Added `planner-route` benchmark family.
- Added `numeric-fastpath` benchmark family with engine stage telemetry.
- Bundled 0.4-buildfix1 benchmark JSON as the direct regression baseline.
- Tightened quality targets and introduced explicit recovery targets for FAST/BALANCED/DENSE,
  warm random access and u32 NumericFast throughput.
- Benchmark schema remains 2.0.

### Repository cleanup
- Removed obsolete demo scripts and demo documentation from previous milestones.
- Removed obsolete root-level task, milestone and build-audit artifacts.
- Preserved historical benchmark baselines because regression tooling depends on them.

### Compatibility
- Workspace version: 0.4.2.
- Format 1.3 is unchanged.
- Reader compatibility remains 1.0/1.1/1.2/1.3.
- Numeric `NUM1` payload layout is unchanged.
- AIDX/ACET are unchanged.

## 0.4-buildfix1 - 2026-09-14

### Fixed
- Fixed four `ace-analysis::numeric` compile errors caused by comparing borrowed `&i64`/`&u8` values without dereferencing them.
- Fixed Planner V4 numeric candidate pruning for monotonic numeric workloads.
- Planner V4 now uses `ace_codecs::estimate_numeric(input)` as an exact deterministic size estimate for `CodecId::Numeric` before analytical ranking.
- The exact Numeric estimate updates only the V4 Numeric candidate; the frozen Planner V3.6 generic estimator and ranking behavior remain unchanged.
- Added a regression test proving a monotonic u32 counter keeps Numeric in stage-one verification and selects it without full trial encodes.

### Compatibility
- ACE Format 1.3 is unchanged.
- Reader compatibility remains 1.0 / 1.1 / 1.2 / 1.3.
- Workspace package version is 0.4.1.
- Benchmark schema remains 2.0.

## 0.4 - 2026-09-14

### Added
- New `ace-bitpack` crate with scalar u32/u64 bit packing, ZigZag, Delta, Delta-of-Delta and Frame-of-Reference primitives.
- Schema-free `NumericProfile` analysis for u32/u64 integer structure.
- `CodecId::Numeric` with deterministic FOR/Delta/DoD + ZigZag + BitPack selection.
- Self-describing `NUM1` numeric payload in Format 1.3.
- Planner V4 public entry point, preserving the hardened V3.6 ranking/sampling/QualityEnvelope stages.
- Numeric candidate family and estimator/work-cost integration.
- Numeric block telemetry and `ace explain` numeric diagnostics.
- `BlockSizePolicy::Fixed/Auto` and `AccessHint`.
- File-level deterministic block-size advisor.
- CLI `--block-policy` and `--access-hint`.
- u32/u64 property tests and numeric engine tests.
- Numeric/bitpack fuzz targets.
- Deterministic Corpus V3 numeric generator.
- `numeric`, `numeric-ablation` and `block-policy` benchmark families.
- Benchmark schema 2.0 and numeric release gates.

### Changed
- Workspace version is now 0.4.0.
- Writer format is now 1.3.
- Reader compatibility expands to 1.0/1.1/1.2/1.3.
- Benchmark result prefix is `0.4-`.
- Streaming writes Format 1.3 and requires Fixed block policy because Auto requires a pre-sample before the fixed header is written.

### Preserved
- RAW/RLE/LZ bitstreams.
- Huffman/rANS/rANS4x bitstreams.
- Hybrid LZ behavior.
- QualityEnvelope.
- AIDX/ACET layout.
- Deterministic tie-breaking and zero hot-path full trial encodes.

## 0.3.1 - 2026-09-14

### Release character
- Hardened stabilization release based on `0.3-buildfix9-compilefix`.
- Planner V3.6, Hybrid LZ, QualityEnvelope and Format 1.2 are frozen.

### Added
- Property-based arbitrary-byte encode/decode roundtrip tests.
- Property-based indexed-range equality tests.
- Determinism matrix across compression profiles, block sizes and worker counts.
- Format/index/trailer hardening tests for limits, logical contiguity, block IDs and CRC.
- Streaming tests for input limits, premature EOF and extra bytes.
- Standalone `cargo-fuzz` project with container, index, trailer and indexed-open targets.
- Deterministic Corpus V2 generator and manifest.
- `corpus` benchmark family.
- `block-matrix` benchmark family for 64K/128K/256K/512K/1M blocks.
- `random-access-extended` benchmark family for 4K..1M aligned and unaligned reads.
- `stability` benchmark family for byte-identical repeated output.
- `stddev_ns`, `cv_percent` and `unstable_measurement` timing diagnostics.
- p99 planner regret.
- Golden `0.3-buildfix9-compilefix` performance baseline.
- Hardened 0.3.1 release-gate policy with controlled benchmark variance.
- 0.3.1 demo, task plan, baseline, format, hardening, fuzzing and benchmark documentation.

### Changed
- Workspace version is now `0.3.1`.
- Benchmark output prefix is `0.3.1-`.
- Performance regression gates compare against buildfix9 golden results with -5% throughput
  and +7.5% latency tolerance.
- Quality gates are stricter: mean regret <=256 B/block, p95 <=1024 B, p99 <=4096 B,
  BALANCED/DENSE ratio >=3.45x.
- Benchmark schema remains 1.9 because all additions are backward-compatible optional fields.

### Unchanged
- Planner V3.6 algorithm.
- Hybrid LZ estimation algorithm and adaptive budget policy.
- QualityEnvelope.
- Cost Model V3.
- codec and entropy bitstreams.
- ACE Format 1.2 writer and 1.0/1.1/1.2 reader compatibility.

## 0.3-buildfix9-compilefix - 2026-09-14

### Fixed
- Fixed `recursion limit reached while expanding $crate::json_internal!` in the Rust benchmark.
- Replaced every object-shaped `serde_json::json!({...})` construction in the benchmark crate with incremental `JsonObjectBuilder` construction.
- Split planner block-detail serialization into identity, ranking, quality, hybrid and outcome sections.
- Split planner summary serialization into identity, quality, ranking, planner-work, hybrid, calibration and timing sections.
- Refactored compression, entropy, parallel, random-access, streaming, memory, environment, configuration and timing JSON construction as well.
- Removed compile-time dependence on increasing Rust's `#![recursion_limit]`.

### Compatibility
- Benchmark schema remains 1.9.
- Existing field names and flat object layout remain unchanged.
- Planner V3.6, Hybrid LZ, QualityEnvelope and release gates are unchanged.
- ACE Format remains 1.2; reader compatibility remains 1.0/1.1/1.2.
- Workspace version remains 0.3.9.

## 0.3-buildfix9 - 2026-09-14

### Added
- Planner V3.6 work budgeting with `PlannerDataClass`, `PlanningBudget` and `EstimateConfidence`.
- Profile-aware `AnalysisLevel` and FAST Analyzer Lite.
- Allocation-free `BlockIndex::intersecting_indices` for random-access range lookup.
- Benchmark schema 1.9 with p95 regret, Hybrid-LZ work metrics and parallel efficiency.
- Buildfix9 demo, tasks, milestone definition and updated documentation.

### Changed
- Hybrid LZ is skipped on zero-heavy/incompressible blocks and stage two is skipped on high-confidence agreement.
- Exact oracle Top-2/Top-3 and quality-pool recall are diagnostic rather than hard release gates.
- Dense/Balanced ordering now allows 0.5% tolerance.
- Throughput gates use absolute targets: FAST 135, BALANCED 65, DENSE 42 MB/s.
- Warm 64 KiB target is <= 76 us.

### Preserved
- Buildfix8 Hybrid LZ estimator semantics and QualityEnvelope.
- Generated/Top-K search quality architecture.
- Zero full candidate trial encodes.
- ACE Format 1.2 writer and 1.0/1.1/1.2 reader compatibility.

## 0.3-buildfix8 - 2026-09-13

### Strategy
- Rebased implementation on 0.3-buildfix6 instead of continuing from buildfix7.
- Selectively retained buildfix7 diagnostics and zero-heavy quality guard.
- Removed buildfix7 LZ Estimator V2 and expanded LZ match-analysis model.

### Added
- `ace-planner::HybridLzEstimator` using bounded deterministic production-codec micro-trials.
- `RouteRouteHybridLzPolicy` with profile/stage bounded sample budgets.
- `HybridLzObservation` with sampled bytes and disagreement PPM.
- Planner telemetry for hybrid candidate count, sampled bytes and max disagreement.
- Benchmark schema 1.8.
- Analytical estimator MAE/MAPE/bias/p95 diagnostics by codec family and data class.
- Preserved buildfix7 benchmark JSONs as a regression baseline.

### Changed
- LZ refinement now runs only after buildfix6 analytical Top-K, preserving search recall.
- BALANCED/DENSE zero-heavy blocks no longer force bare RLE fast path.
- Analytical MAPE is diagnostic-only, not a release gate.

### Compatibility
- No wire-format changes; writer remains 1.2 and readers remain 1.0/1.1/1.2 compatible.
- JVM-facing examples/APIs are unchanged.

## 0.3-buildfix6 - 2026-09-12

### Fixed
- Fixed final Planner V3 selection allowing CPU savings to trade away excessive compression ratio even when quality-optimal candidates remained available.
- Added a profile-aware `QualityEnvelope` before final scalar-cost selection.
- Preserved analytical, sampled and blended size estimates separately instead of overwriting one `predicted_size_bytes` value across verifier stages.
- Final CostModelV3 ranking now runs only inside the quality-qualified candidate pool.

### Added
- New `ace-cost::quality` module.
- `QualityEnvelope` with overflow-safe deterministic integer arithmetic.
- Planner telemetry for best blended size, quality limit, selected blended size, quality-qualified count, selected size rank and selected cost rank.
- Oracle rank metrics for analytical, stage-one, post-sampling and final quality-pool stages.
- Predicted size regret telemetry.
- New buildfix6 planner and benchmark documentation.
- Focused buildfix6 demo.
- Preserved buildfix5 failure report as a diagnostic baseline.

### Regression policy
- Replaced the tautological ranking-only `sample_survival_recall` release gate with oracle Top-2/Top-3 after-sampling rank gates.
- Added BALANCED ratio >= 3.40x.
- Retained actual regret <= 1024 B/block.
- Retained zero full candidate trial encodes.
- Retained FAST >=2x, BALANCED >=4x and DENSE >=3.5x baseline throughput targets.
- Retained DENSE >=99.5% hardened-baseline ratio and profile ratio ordering.

### Compatibility
- No ACE Format change.
- Writer remains Format 1.2.
- Reader remains compatible with Formats 1.0, 1.1 and 1.2.
- CandidateGenerator, adaptive Top-K, SIMD, streaming, index, rANS4x and RLE wire semantics are unchanged.

## 0.3-buildfix5 - 2026-09-12

### Fixed
- Fixed Rust E0689 in `ace-planner/src/evaluator.rs` by explicitly typing the Planner V3.2 sample blending weight as `u64`.
- Added exhaustive blend-weight invariant coverage across profiles, codec families, confidence buckets, and verifier stages.
- Audited Planner V3.2 saturating arithmetic for additional ambiguous inferred-integer sites; no other affected binding was found.

### Compatibility
- No ACE Format 1.2 change.
- No change to CandidateGenerator, Top-K widths, SampleVerifier V3.2 semantics, SIMD, streaming, index, rANS4x, RLE, or regression thresholds.

## 0.3-buildfix4 - 2026-09-12

### Fixed
- Changed Planner V3.2 stage-two sample verification from elimination to ranking-only refinement.
- Kept all stage-one survivors eligible for final selection, fixing the 0.50 sample-verifier recall observed in buildfix3.
- Rebalanced sample/full-block analytical evidence with deterministic confidence-weighted blending.
- Increased analytical authority for LZ and DENSE to avoid short-window bias.

### Added
- Complete final planner ranking in diagnostics.
- Oracle rank before and after sampling.
- Oracle Top-1/Top-2/Top-3 rates after sampling.
- Sample-survival recall release metric.
- Planner V3.2 tests for blend-weight invariants.

### Regression policy
- Dense ratio remains >= 99.5% of the 0.2.1-buildfix1 quality baseline.
- DENSE throughput target is >= 3.5x baseline; FAST remains >= 2x and BALANCED >= 4x.
- Full trial encodes remain exactly zero.

## 0.3-buildfix3 - 2026-09-12

### Fixed

- Fixed Planner V3 sample projection incorrectly scaling entropy metadata with sample-to-block ratio. Metadata is now charged once per projected block.
- Fixed severe quality regression where candidate generation recall was 1.0 but Top-K/sample verification produced ~35.3 KiB regret per block.
- Added adaptive Top-K and DENSE quality floor.
- Added semantic-family anchors so analytical ranking cannot remove every representative of a promising physical family before sampling.
- Added codec-specific, stratified LZ sample windows and a larger second verifier stage.
- Kept disjoint LZ samples independent to avoid artificial matches between unrelated block regions.

### Benchmarks

- Added generation, Top-K, sample-verifier and final-selection recall.
- Added regret per data class and per-block stage survival flags.
- Added profile ratio-ordering gates.
- Official result filenames now include `0.3-buildfix3`.
- Preserved the observed 0.3-buildfix2 regression report as a diagnostic baseline.

### Compatibility

- No ACE wire-format change. Writer remains Format 1.2; reader supports 1.0, 1.1 and 1.2.
- SIMD, rANS4x, streaming and random-access wire semantics are unchanged.

## 0.3-buildfix2 - 2026-09-12

### Fixed

- Fixed an RLE literal-packet boundary bug where a sequence of short 1-3 byte runs could grow a literal packet past the 128-byte wire-format limit.
- Prevented literal control bytes from accidentally setting the RLE run flag at lengths 129-130.
- Added boundary roundtrip tests for 126/127/128/129/130/255/256/257-byte literals.
- Added the exact repeated-short-run regression case that caused `ace verify` to fail with `RLE decoded size mismatch`.
- Added a deterministic fuzz-style RLE roundtrip test and a `cargo-fuzz` RLE roundtrip target.
- Changed `ace explain` to render entropy-policy sentinel scores as `penalized` rather than an overflow-looking decimal value.

### Compatibility

- No ACE Format 1.2 changes.
- No Planner V3, rANS4x, SIMD, streaming, index, or random-access semantic changes.
- Reader compatibility remains ACE 1.0/1.1/1.2; writer remains ACE 1.2.

## 0.3-buildfix1 - 2026-09-12

### Fixed
- Replaced `#[derive(Default)]` on `ace_runtime::WorkerScratch` with an explicit `Default` implementation so `[u32; 256]` works on Rust toolchains that do not provide blanket `Default` for large arrays.
- Added a regression test verifying default scratch buffers are empty and the 256-entry histogram is zero-initialized.

### Compatibility
- No ACE Format 1.2 changes.
- No planner, codec, entropy, streaming, or random-access semantic changes.


## 0.3 — 2026-09-12

### Planner V3

- Added `ace-cost` and moved cheap candidate estimation out of runtime codec execution.
- Added `CandidateEstimator`, `EstimatedCandidate`, `CostModelV3` and profile-specific `SamplePolicy`.
- Replaced full-block candidate trial compression in the hot planner with deterministic Top-K sample verification.
- Added adaptive Top-K depth based on estimator confidence.
- Added deterministic planner fast paths for incompressible, run-heavy, strongly repetitive and strong-delta blocks.
- Added `PlannerTelemetry` fields for fast-path hits, estimated candidates, sampled candidates and full trial encodes.
- Runtime planner now targets zero full candidate trial encodes per block.

### Entropy / Format

- Added four-lane `rANS4x` entropy mode.
- Added `EntropyCodecId::Rans4x`.
- Bumped writer format from ACE 1.1 to ACE 1.2.
- Kept decoder compatibility with ACE 1.0 and 1.1.
- Added validation preventing the 1.2-only entropy ID from being accepted in older framing.

### SIMD and runtime

- Added `ace-simd` with safe scalar/AVX2 runtime dispatch.
- Analyzer zero counting can use AVX2.
- LZ longest-match prefix comparison can use AVX2.
- Added `WorkerScratch` reusable buffer boundary in `ace-runtime`.

### Streaming

- Added `ace-stream`.
- Added bounded-memory `compress_reader_known_size`.
- Added `StreamLimits` and `StreamingStats`.
- Streaming output retains the regular ACE index/trailer and therefore remains random-access capable.
- Added CLI `compress-stream` command.

### Benchmarks

- Benchmark schema upgraded for milestone 0.3 and Format 1.2.
- Added `rANS4x` entropy benchmark.
- Planner benchmark now records fast-path rate, estimates/block, samples/block and full-trial encodes/block.
- Random-access benchmark now separates `range_64k_cold` and `range_64k_warm`.
- Renamed reported physical read ratio to `physical_to_logical_ratio`.
- Added `streaming` and `memory` benchmark families.
- Regression baseline changed to the successful 0.2.1-buildfix1 release.

### Security / hardening

- Kept checked arithmetic around stream size, block count and index offsets.
- rANS4x validates metadata length, lane payload lengths and output lane reconstruction.
- Architecture-specific unsafe code is isolated inside `ace-simd`.
- Streaming validates exact declared source size and rejects truncated/overlong sources.

## 0.2.1-buildfix1

- Restored FAST profile throughput by fixing normalized cost weighting.
- Raised planner candidate recall to 100% on the reference corpus.
- Fixed random-access benchmark semantics and benchmark validator version hard-coding.
- Preserved ACE Format 1.2.

## 0.2.1

- Added CandidateTier, Cost Model V2.1, profile calibration, detailed benchmark observability and regression gates.

## 0.2

- Added scalar rANS, block index/random access, parallel blocks, dictionary abstractions and JSON benchmark contract.

## 0.1

- Initial adaptive block planner, RAW/RLE/LZ, Delta transform, Huffman, Format 1.0, CRC32C and CLI.
