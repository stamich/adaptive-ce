# ACE 0.4.6 — Release checklist (acceptance matrix)

`./ace-release0.4.6.sh <0.4.5-buildfix2-tree>` executes the matrix and writes the filled copy
to `examples/results/release-checklist-0.4.6.md`. Items marked *manual* are run separately.

| Area | Gate | Source |
|---|---|---|
| Build / fmt / clippy / rustdoc | PASS, 0 warnings | `ace-build0.4.6.sh` |
| MSRV 1.75 | PASS (Skipped without toolchain) | `ace-ci0.4.6.sh msrv` |
| Unsafe / panic audit | PASS | `tools/ace-code_audit0.4.6.py` |
| Unit + property tests (release budget 10 000) | PASS | `cargo test --release` with `PROPTEST_CASES` |
| Fuzz 12 × 10 min (*manual*) | 0 crashes | `ACE_FUZZ_SECONDS=600 ./ace-ci0.4.6.sh fuzz` |
| Golden SHA-256 (auto + `ACE_SIMD=scalar`) | identical | `tests/golden.rs` |
| Determinism (threads × processes × backend × API path) | identical | release step 5 + `determinism_matrix` |
| Format readers 1.0–1.3 | PASS | `format_1_0_compat`, `format_1_2`, `format_1_3` |
| Malformed / resource limits | rejected, no panic, no large allocation | `malformed_matrix` |
| Random access stress / concurrent readers | PASS | `random_access_stress` |
| Streaming 1 GiB / file 256 MiB | PASS | `large_files` (`--ignored`) |
| Policy quality, compression ratio | PASS | Regression V3 |
| Performance vs 0.4.5-buildfix2 (interleaved A/B) | PASS | `ace-ab0.4.6.sh` |
| Stability | no `unstable` after ≤ 2 retries | Regression V3 |
| Performance report | generated | `docs/PERFORMANCE-0.4.6.md` |
| Reproducible package + name audit | 2 × same SHA-256 | `tools/ace-package0.4.6.py` |
| Tested from archive | PASS | release step 11 |
| Documentation | README, RELEASE-NOTES, METHODOLOGY, audits, CHANGELOG | review |

After acceptance on the reference machine: copy the results to
`examples/baselines/0.4.6/` with a `BASELINE.json` (`methodology: "schema 2.1, interleaved A/B
vs 0.4.5-buildfix2"`).
