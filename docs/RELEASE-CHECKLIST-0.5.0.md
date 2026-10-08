# ACE 0.5.0 — Release checklist (acceptance matrix)

`./ace-release0.5.0.sh <0.4.6-tree>` executes the matrix and writes the filled copy to
`examples/results/release-checklist-0.5.0.md`. Items marked *manual* are run separately.

| Area | Gate | Source |
|---|---|---|
| Build / fmt / clippy / rustdoc | PASS, 0 warnings | `ace-build0.5.0.sh` |
| MSRV 1.97 | PASS (Skipped without toolchain) | `ace-ci0.5.0.sh msrv` |
| Unsafe / panic audit | PASS, no new `unsafe` | `tools/ace-code_audit0.5.0.py` |
| Unit + property tests (release budget 10 000) | PASS | `cargo test --release` with `PROPTEST_CASES` |
| Fuzz 15 × 10 min (*manual*) | 0 crashes — **done**: 15 / 15 clean (`examples/baselines/0.5.0/FUZZ.md`) | `ACE_FUZZ_SECONDS=600 ./ace-ci0.5.0.sh fuzz` |
| Golden 0.5.0 (Corpus V3 + V4) and frozen golden 0.4.6 (Corpus V3), auto + `ACE_SIMD=scalar` | identical | `tests/golden.rs` |
| Determinism (threads × processes × backend × API path), incl. Float workloads | identical | release step 5 (mixed + f64-noisy) + `determinism_matrix` |
| Format readers 1.0–1.4, TS1 only in 1.4, plain TS1 blocks | PASS | `format_1_0_compat`, `format_1_2`, `format_1_3`, `format_1_4` |
| Malformed / resource limits (incl. TS1 fixture) | rejected, no panic, no large allocation | `malformed_matrix`, TS1 unit / property tests |
| Float lane end to end | V3 untouched, V4 never larger, explain = compress | `float_lane` |
| Streaming == in-memory (incl. Format 1.4 header rewrite) | identical | `streaming`, `determinism_matrix` |
| Random access stress / concurrent readers | PASS | `random_access_stress` |
| Streaming 1 GiB / file 256 MiB | PASS | `large_files` (`--ignored`) |
| Policy quality, compression ratio | PASS | Regression V3 `quality` |
| Float lane: false positives, fallbacks, ratios, estimator, codec speed, FloatFast speedup | PASS | Regression V3 `float` + correctness |
| Performance vs 0.4.6 (interleaved A/B, byte identity on Corpus V3) | PASS | `ace-ab0.5.0.sh` |
| Stability | no `unstable` after ≤ 2 retries | Regression V3 |
| Performance report | generated | `docs/PERFORMANCE-0.5.0.md` |
| Demo | PASS | `demo/ace-run-demo0.5.0.sh` |
| Reproducible package + name audit | 2 × same SHA-256 | `tools/ace-package0.5.0.py` |
| Tested from archive | PASS | release step 11 |
| Documentation | README, RELEASE-NOTES, MIGRATION, FORMAT-1.4, TS1-CODEC, PLANNER-V5, FLOAT-CALIBRATION, METHODOLOGY, audits, CHANGELOG | review |

Accepted run: AMD Ryzen 9 5950X, Regression V3 PASS (73 / 0 fail / 0 unstable), A/B vs 0.4.6
12 / 12 PASS byte-identical — stored in `examples/baselines/0.5.0/` with `BASELINE.json`; the
Float codec-speed floors are confirmed as hard values (`BENCHMARK-METHODOLOGY-0.5.0.md`).
The manual fuzz campaign (15 × 10 min, AddressSanitizer) finished with no crash, OOM or timeout
(`examples/baselines/0.5.0/FUZZ.md`): every item of the matrix is PASS.
