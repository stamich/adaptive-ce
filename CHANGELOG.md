# Changelog

## 0.3-buildfix7 - 2026-09-13

### Fixed
- Replaced the under-calibrated one-dimensional LZ size heuristic with LZ Analytical Estimator V2.
- Added p95 match length, match coverage and long-match ratio to `BlockProfile`.
- Aligned repetition analysis with the production 130-byte LZ match limit and overlap-capable references.
- Prevented BALANCED/DENSE zero-heavy blocks from bypassing entropy alternatives through unconditional RLE/None fast path.
- Reduced reset-window sample authority for LZ candidates.
- Added LZ token-stream entropy estimation based on match structure instead of original byte H0 alone.

### Added
- Estimator calibration benchmark with MAE, MAPE, signed bias and p95 absolute error per codec family and data class.
- Per-block selected prediction error and LZ-analysis telemetry.
- LZ FAST/BALANCED estimator MAPE regression gates.
- Focused buildfix7 demo, milestone definition and architecture docs.
- Preserved complete 0.3-buildfix6 benchmark set as the diagnostic baseline.

### Compatibility
- No Format change; writer remains 1.2 and reader remains 1.0/1.1/1.2 compatible.
- Runtime full candidate trial encodes remain zero.

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
