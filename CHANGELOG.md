# Changelog

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
