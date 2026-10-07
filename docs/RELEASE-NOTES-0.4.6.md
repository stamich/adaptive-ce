# ACE 0.4.6 — Release notes

**Hardened Release & Benchmark Stabilization** — the closing release of the 0.4 line.

## Summary of the 0.4 line

| Release | Contribution |
|---|---|
| 0.4 | Planner V4 routes (Generic / NumericGeneral / NumericFast), NUM1 codec, Format 1.3 |
| 0.4-buildfix1…4 | Planner V4.3 policy oracle, quality gates, NumericFast evidence path |
| 0.4.5 / buildfix1 | u16 lanes, numeric coverage, table-driven entropy decode |
| 0.4.5-buildfix2 | module structure (SOLID / DRY), 3-way CRC32C, table Huffman, fused NUM1 decode |
| **0.4.6** | semantic freeze, Benchmark Harness V3, interleaved A/B, hardening, audits, reproducible package |

## What 0.4.6 guarantees

* **Same bytes.** Format 1.3, Planner V4.3 decisions and every encoded byte equal
  0.4.5-buildfix2: golden SHA-256 for 15 workloads × 3 profiles (auto and `ACE_SIMD=scalar`),
  and byte-identical outputs of both versions in every A/B case.
* **Determinism** across threads (1, 2, 4, 8, all), processes, SIMD backends and API paths
  (`compress`, `compress_to`, streaming).
* **Measurable performance.** Harness V3 (adaptive iterations, 3 × 7 batches,
  median-of-medians, batch MAD) and the interleaved A/B against 0.4.5-buildfix2 replace
  cross-run comparisons; unstable measurements never count as PASS.
* **Hardening.** Malformed matrix (incl. exhaustive single-byte flips), resource limits without
  large pre-allocation, 10 000 random ranges, concurrent readers, 1 GiB streaming and 256 MiB
  indexed files, property budgets up to 10 000 cases, 12 registered fuzz targets.
* **Code safety.** `unsafe` only in the audited `ace-simd` kernels (and the benchmark's counting
  allocator); zero `unwrap` / `expect` / `panic!` in production code; rustdoc on every item.
* **Reproducible delivery.** Deterministic ZIP (two builds → same SHA-256) with `Cargo.lock`,
  tested from the archive.

## New

* `AceEngine::decompress_into(&[u8], &mut Vec<u8>)` — decode into a reusable buffer
  (reservation capped by `DecodeLimits::max_output_size` and `PREALLOCATION_CAP_BYTES`).
* `ACE_SIMD=scalar` — force portable code paths (determinism checks, debugging).
* `ace-corpus` crate and CLI — deterministic workloads shared by tests, golden files, demo
  and benchmarks.
* `ace-bench` — Benchmark Harness V3, schema 2.1, `release-performance` family, allocation
  counts and peak RSS in the `memory` family, full environment fingerprint.
* Tools: Regression V3 (Pass / Fail / Unstable / Skipped), interleaved A/B with A/A control,
  report with generated `PERFORMANCE-0.4.6.md`, comparator for schema 2.0 / 2.1, code audit,
  deterministic packager.
* Scripts: `ace-build`, `ace-benchmark`, `ace-benchmark-compare`, `ace-ab`, `ace-release`,
  `ace-ci` (all `…0.4.6.sh`) and `demo/ace-run-demo0.4.6.sh`.

## Results on the development VM

Full Harness V3 run + interleaved A/B on a shared 2-vCPU Xeon VM
([`PERFORMANCE-0.4.6.md`](PERFORMANCE-0.4.6.md)): Regression V3 **PASS** —
correctness 8/8, quality 13/13, performance 15/15 (12 A/B cases incl. byte identity, 3
absolute floors), stability 15/15. Encode ratios vs 0.4.5-buildfix2: 0.98–1.06 (no
regression). The first two sessions on that VM ended `unstable` and were retried, exactly as
the release pipeline does. The reference-machine run (`./ace-release0.4.6.sh`) remains the
release decision.

## Known limitations

* no float (Gorilla/XOR) or decimal specialisation;
* no trained dictionaries;
* no SIMD bit packing; scalar rANS;
* fixed physical block size;
* no Patched FOR (`monotonic-outliers` compresses 1.78×);
* generic encoding ≈ 9× slower than zstd-3 at a similar ratio (analysis + planning cost);
* `decompress` allocates ≈ 2.3× the output size (per-block buffers + output);
  `decompress_into` ≈ 1.3×;
* SIMD / CRC acceleration only on x86_64 (AVX2, SSE4.2); other architectures use the scalar path;
* no GPU support;
* MSRV 1.75 is enforced by `clippy.toml`, the lockfile and `ace-ci0.4.6.sh msrv`; the
  1.75 toolchain itself was not available in the development environment.

## Upgrade

Drop-in for 0.4.5-buildfix2: no API removals, no format change. Benchmark consumers of schema
2.0 keep working (all 2.0 fields are present and computed from the median-of-medians).
