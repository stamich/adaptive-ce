# ACE 0.4.6 — implementation order (Hardened Release & Benchmark Stabilization)

Base: 0.4.5-buildfix2. Rule for every task: **golden SHA-256 unchanged** (semantic freeze),
`cargo clippy --workspace --all-targets -- -D warnings` clean, rustdoc on every item.
Status: ✅ done in this release · ⏩ executed by the release pipeline on the reference machine.

## Phase A — freeze and baseline

| # | Task | Files / types | Acceptance |
|---|---|---|---|
| A1 ✅ | Semantic-freeze rule | `docs/ARCHITECTURE-FREEZE-0.4.6.md` | rule + guard table |
| A2 ✅ | Shared deterministic corpus crate (generators byte-identical to the old harness) | `crates/ace-corpus`: `Workload`, `XorShift32`, `generators`, `bin/ace-corpus` | generator unit tests; old Python generators removed |
| A3 ✅ | Golden generator + `GOLDEN.json` from the unchanged 0.4.5-buildfix2 code | `crates/ace-engine/tests/golden.rs`, `examples/golden/0.4.6/` | `ACE_GOLDEN_UPDATE=1` regenerates; test compares |
| A4 ✅ | `ACE_SIMD=scalar` override (read once) | `ace_simd::{SIMD_OVERRIDE_ENV, scalar_forced}`, `crc32c_backend_name` | golden passes with and without override |
| A5 ✅ | Baseline descriptors; prune obsolete baselines | `examples/baselines/*/BASELINE.json` (0.2.1-buildfix1, 0.4-buildfix2, 0.4.5-buildfix1, 0.4.5-buildfix2) | regression V3 needs nothing else |

## Phase B — code hygiene without byte changes

| # | Task | Files / types | Acceptance |
|---|---|---|---|
| B1 ✅ | `rust-version = "1.75"`, explicit `[profile.release]`, MSRV-aware `Cargo.lock` (lz4_flex 0.11.3) | `Cargo.toml`, `Cargo.lock` | `--locked` builds |
| B2 ✅ | Workspace lints: `unsafe_code = forbid`, `missing_docs = deny`, `unsafe_op_in_unsafe_fn = deny`, clippy `unwrap_used/expect_used/panic/todo/unimplemented/dbg_macro = deny` | `[workspace.lints]`, `clippy.toml` (tests exempt) | clippy -D warnings clean |
| B3 ✅ | Remove production `unwrap`/`expect`/`panic` | `AceEngine::default_engine`, `Lane::WIDTH`, bit I/O (`read_bits_validated` infallible), NUM1 header, Huffman, error types | 0 panic-lint exceptions (`docs/PANIC-AUDIT-0.4.6.md`) |
| B4 ✅ | `unsafe` blocks inside `unsafe fn`, `// SAFETY:` everywhere | `ace-simd/src/{avx2,crc32c,scan}.rs` | `undocumented_unsafe_blocks = deny` |
| B5 ✅ | Integration-test naming + crate docs | `crates/*/tests/*.rs` | `missing_docs` clean |
| B6 ✅ | Version 0.4.6 for product and Cargo | `[workspace.package] version` | `ace --version` = 0.4.6 |

## Phase C — Benchmark Harness V3

| # | Task | Files / types | Acceptance |
|---|---|---|---|
| C1 ✅ | Move harness to `crates/ace-bench`; results to `$ACE_BENCH_OUT_DIR` or `<workspace>/examples/results` | `json::result_path`, `OUT_DIR_ENV` | path independent of crate location |
| C2 ✅ | Measurement plan (release 5 warm-ups, 3 × 7, ≥ 50 ms; quick 1 × 5, 2 ms) | `timing/plan.rs`: `MeasurementPlan::{RELEASE, QUICK, current}` | unit tests |
| C3 ✅ | Adaptive doubling calibration, batched sampling | `timing/runner.rs`: `measure`, `measure_with`, `next_iterations` | unit tests; `measure(|| op)` API unchanged (72 call sites) |
| C4 ✅ | Robust statistics | `timing/stats.rs`: `SampleStats::from_batches`, `median`, `mad`, MoM, `batch_mad_percent`, outliers (3·1.4826·MAD) | unit tests (bad batch, drift, outlier, constant) |
| C5 ✅ | Schema 2.1 timing object (2.0 fields from MoM + `stable_timing`) | `timing/report.rs`: `timing_json` | validator 2.1 |
| C6 ✅ | Pre-allocated decode API | `AceEngine::decompress_into`, `decode_blocks`, `PREALLOCATION_CAP_BYTES` | determinism + malformed tests |
| C7 ✅ | Build fingerprint | `crates/ace-bench/build.rs` (`ACE_BUILD_RUSTC/TARGET/PROFILE/OPT_LEVEL/LTO/CODEGEN_UNITS/TARGET_FEATURES/RUSTFLAGS`) | `target_features` no longer `"unknown"` |
| C8 ✅ | Runtime fingerprint before/after each family + warnings | `environment.rs`: `RuntimeSnapshot`, `environment_json` | governor, freq, thermal, loadavg, SIMD/CRC backend |
| C9 ✅ | Counting allocator + peak RSS | `alloc.rs`: `CountingAllocator`, `AllocationProbe`, `count_allocations`, `peak_rss_bytes` | unit test; `memory` family rows |
| C10 ✅ | `release-performance` family (exact gate cases, prealloc decode) | `families/release.rs` | 21 timing objects with `case_id` |
| C11 ✅ | Family registry, `--list`, timing per family | `main.rs`: `FAMILIES`, `select_families`, `run_family` | unit tests |

## Phase D — gates V3 and A/B

| # | Task | Files | Acceptance |
|---|---|---|---|
| D1 ✅ | Shared Python library (gate model Pass/Fail/Unstable/Skipped/Diagnostic, stats, markdown) | `tools/ace-benchlib0.4.6.py` | used by all tools |
| D2 ✅ | Validator for schema 2.0 and 2.1 | `tools/ace-validate_benchmark_json0.4.6.py` | validates baselines (2.0) and results (2.1) |
| D3 ✅ | A/B probe compiled against any 0.4.x tree | `tools/ace-abprobe0.4.6/src/main.rs` | builds against 0.4.5-buildfix2 and 0.4.6 |
| D4 ✅ | Interleaved A/B driver (alternating order, ratio + upper bound, byte identity) | `tools/ace-ab0.4.6.py`, `ace-ab0.4.6.sh` | `ab-0.4.6-vs-0.4.5-buildfix2.json` |
| D5 ✅ | Regression V3 (correctness / quality / performance / stability / environment; exit 0/1/3/4) | `tools/ace-check_regressions0.4.6.py` | quick plan → stability `skipped` |
| D6 ✅ | Report (text + `--markdown` → PERFORMANCE) and comparator (2.0/2.1, warnings) | `tools/ace-benchmark_report0.4.6.py`, `tools/ace-benchmark_compare0.4.6.py` | generated `docs/PERFORMANCE-0.4.6.md` |

## Phase E — hardening

| # | Task | Files | Acceptance |
|---|---|---|---|
| E1 ✅ | Determinism matrix: workloads × profiles × threads {1,2,4,8,all} × `compress`/`compress_to`/streaming; `decompress_into` reuse | `ace-stream/tests/determinism_matrix.rs` | identical bytes |
| E2 ✅ | Process / backend axis | `ace-release0.4.6.sh` step 5, demo step 4 | 10 runs + scalar, one SHA-256 |
| E3 ✅ | Malformed matrix (table, every-7th truncation, single-byte flip sweep, forged sizes, limits) | `ace-engine/tests/malformed_matrix.rs` | rejected by ≥ 1 reader, misdecoded by none |
| E4 ✅ | Property budgets via `PROPTEST_CASES`; NUM1 properties | `ace-codecs/tests/num1_properties.rs`, engine/bitpack properties | release budget 10 000 |
| E5 ✅ | 12 registered fuzz targets (stale `ace_decoder` replaced by `engine_roundtrip`, fixed `planner_sample_offsets`) | `fuzz/Cargo.toml`, `fuzz/fuzz_targets/*` | `cargo check` of fuzz project |
| E6 ✅ | 10 000 ranges, per-block decode, out-of-bounds, 8 concurrent readers | `ace-engine/tests/random_access_stress.rs` | pass |
| E7 ✅ | 1 GiB streaming / 256 MiB indexed (`--ignored`) | `ace-stream/tests/large_files.rs` | pass in release CI |
| E8 ✅ | Code audit tool (unsafe placement, SAFETY comments, allow justifications) | `tools/ace-code_audit0.4.6.py` → `docs/PANIC-AUDIT-0.4.6.md` | exit 0 |
| E9 ⏩ | Fuzz campaign 12 × 10 min | `ACE_FUZZ_SECONDS=600 ./ace-ci0.4.6.sh fuzz` | 0 crashes |

## Phase F — scripts, demo, documentation

| # | Task | Files | Acceptance |
|---|---|---|---|
| F1 ✅ | Scripts (shared helpers sourced) | `ace-build/benchmark/benchmark-compare/ab/release/ci0.4.6.sh`, `tools/ace-common0.4.6.sh` | names audited by the packager |
| F2 ✅ | Short product demo using `ace-corpus` | `demo/ace-run-demo0.4.6.sh` | < 1 min after build |
| F3 ✅ | Remove obsolete files | 0.4.5-buildfix2 scripts/tools, Python corpus generators, cost-model calibration script, duplicate Java/Scala sources, 11 obsolete baseline dirs, historic docs → `docs/history/` | package audit clean |
| F4 ✅ | Docs | README, RELEASE-NOTES, ARCHITECTURE, ARCHITECTURE-FREEZE, BENCHMARK-METHODOLOGY, UNSAFE-AUDIT, PANIC-AUDIT (generated), RELEASE-CHECKLIST, SECURITY, PERFORMANCE (generated), CHANGELOG, ROADMAP | links resolve |
| F5 ✅ | Compiled API example | `ace-engine` crate doctest | `cargo test --doc` |

## Phase G — release

| # | Task | Files | Acceptance |
|---|---|---|---|
| G1 ✅ | Deterministic ZIP (sorted, 1980-01-01, 0644/0755, MANIFEST + SHA256SUMS) | `tools/ace-package0.4.6.py --verify-reproducible` | 2 × same SHA-256 |
| G2 ✅ | Test from archive | `ace-release0.4.6.sh` step 11 | tests + golden pass from the unpacked ZIP |
| G3 ⏩ | Full release run on the reference machine (i7-9850H) | `./ace-release0.4.6.sh <0.4.5-buildfix2 tree>` | checklist all PASS |
| G4 ⏩ | Publish `examples/baselines/0.4.6/` with `BASELINE.json` | results of G3 | methodology "schema 2.1, interleaved A/B" |
