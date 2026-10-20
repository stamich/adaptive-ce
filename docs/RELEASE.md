# ACE release process

Canonical process of the current release line (0.7.x). Earlier checklists:
[`history/`](history/) (`RELEASE-CHECKLIST-0.4.6.md`, `RELEASE-CHECKLIST-0.5.0.md`).

## 1. Automatic pipeline

```bash
git worktree add ../ace-0.6.1 v0.6.1            # the previous release, for the interleaved A/B
./ace-release0.7.0.sh ../ace-0.6.1              # on the reference machine, performance governor
```

`ace-release0.7.0.sh` runs every step below, records each in
`examples/results/release-checklist-0.7.0.md`, and writes the distribution to `dist/`:

| # | Step | Gate |
|---|---|---|
| 1 | `ace-build0.7.0.sh`: fmt, clippy `-D warnings`, check, tests, release build, rustdoc (public and private items) | 0 warnings |
| 2 | `tools/ace-code_audit0.7.0.py`: `unsafe` placement and `// SAFETY:`, justified clippy allowances, decoder audit (lints present, every referenced test and fuzz target exists) | no violation |
| 3 | `cargo test --release` (property budget 10 000) incl. `--ignored` large-file tests | all pass |
| 4 | golden SHA-256 0.6.1 and the frozen 0.5.0 and 0.4.6, auto and `ACE_SIMD=scalar` | identical bytes |
| 5 | CLI determinism: 10 processes × threads × SIMD backend, mixed + f64-noisy | one SHA-256 |
| 6 | `ace-benchmark0.7.0.sh all --repeat 3` (includes the `ffi` family and, when Gradle is installed, the JVM benchmark) | repeatability PASS |
| 7 | interleaved A/B vs the previous release + Regression V3 incl. the `ffi` gates (Unstable → ≤ 2 re-measurements) | PASS (`diagnostic` on a non-`performance` governor is not a pass) |
| 8 | `docs/PERFORMANCE-0.7.0.md` from the results | generated |
| 9 | demo | < 1 min, no error |
| 10 | deterministic source archive, built twice | same SHA-256, script-name audit |
| 11 | unpack the archive, run the tests and the golden check from it | PASS |
| 12 | `dist/`: `ace-0.7.0-source.zip`, `ace-0.7.0-benchmarks.zip`, `RELEASE-REPORT.md` (git revision, toolchain, host, every gate verdict), `MANIFEST.sha256` | written |

`--quick` runs the same pipeline with smoke-sized measurements; its verdict is never a
release.

## 2. Acceptance matrix

| Area | Gate | Evidence |
|---|---|---|
| Build, lints, rustdoc (incl. private items) | 0 warnings | step 1 |
| MSRV = build toolchain 1.98.1 | PASS | CI `pr` job (`rust-toolchain.toml`) |
| Unsafe / panic / decoder audit | PASS, no new `unsafe` | step 2, [`DECODER-AUDIT.md`](DECODER-AUDIT.md) |
| Golden 0.5.0 + frozen 0.4.6, auto + scalar | identical | step 4 |
| Compatibility fixtures (0.2.1 – 0.5.0) | every file decodes byte for byte | `compat_fixtures` |
| Format readers 1.0 – 1.4, malformed input, limits before allocation | rejected, no panic, no large allocation | `malformed_matrix`, `header_fields`, `metadata_fields`, `decode_limits`, `limits_before_allocation`, `trailer_limits` |
| Determinism (threads 1 – 32, in-flight batches, API paths, processes, backends) | identical | `determinism_matrix`, step 5 |
| Planner V5 invariants, fixed tie-breaks | PASS | `v5_invariants`, `tie_break` |
| CLI (invalid input, closed pipe, `ace version`) | PASS | `crates/ace-cli/tests/cli.rs` |
| Fuzz smoke 20 × 25 s | 0 crashes | CI `fuzz-smoke` job |
| Fuzz campaign 15 × 10 min (*manual*) | 0 crashes | `examples/baselines/<version>/FUZZ.md` |
| Quality, Float lane, stability, performance vs the previous release | PASS | Regression V3 + A/B |
| Repeatability (3 runs) | deterministic values identical, macro spread ≤ 5 % (decompression ≤ 15 %) | `benchmark-<version>-repeatability.json` |
| Reproducible package, tested from the archive | PASS | steps 10 – 11 |
| Documentation | README, RELEASE-NOTES, COMPATIBILITY, SECURITY, BENCHMARKS, audits, CHANGELOG, ROADMAP | review |

## 3. Manual steps (reference machine: AMD Ryzen 9 5950X)

1. Set the `performance` governor, stop other work, run the pipeline (section 1). Install Gradle
   and JDK 22 or newer first, so that the JVM side of the `ffi` gates is measured; without them
   the verdict is `incomplete`. CI covers the JVM tests (`jvm` job, JDK 22 and 25), the header and
   bindings drift, the C smoke test and the aarch64 golden check on every push.
2. Fuzz campaign, recorded as `examples/baselines/<version>/FUZZ.md`:
   `ACE_FUZZ_SECONDS=600 ACE_FUZZ_CMIN=1 ./ace-ci0.7.0.sh fuzz`.
3. Publish the baseline: copy the median documents of step 6 (`examples/results/benchmark-*.json`,
   `ab-*.json`) to `examples/baselines/<version>/` with a `BASELINE.json` descriptor (machine,
   governor, toolchain, git revision, verdicts), and the generated
   `docs/PERFORMANCE-<version>.md`.
4. Merge, then tag the merge commit:

   ```bash
   git tag -a v0.7.0 -m "ACE 0.7.0 — Embedding" -m "$(cat dist/RELEASE-REPORT.md)"
   git push origin v0.7.0
   ```

5. Attach `dist/*` to the GitHub release.

## 4. Version bump checklist (next release)

Rename the versioned scripts and tools (`ace-*<version>.sh`, `tools/ace-*<version>.*`) — the
package audit rejects stale names — update `ACE_VERSION` / `ACE_BASELINE`
(`tools/ace-common<version>.sh`), `MILESTONE` / `BASELINE` (`tools/ace-benchlib<version>.py`),
`MILESTONE` / `BASE_MILESTONE` (`crates/ace-bench/src/json.rs`), the workspace version, and
add `TASKS-<version>.md` / `MILESTONE-<version>.json` (earlier ones move to `docs/history/`).

## 5. SonarQube Cloud (static analysis and coverage)

Configuration: `sonar-project.properties`; workflow: `.github/workflows/ace-sonarcloud0.7.0.yml`
(separate from `ace-ci`, so it never gates the build). One-time setup:

1. In SonarQube Cloud import `stamich/adaptive-ce` (organization `stamich`, project key
   `stamich_adaptive-ce`; change both in the properties file if yours differ).
2. Administration > Analysis Method: switch **Automatic Analysis off** (CI-based analysis).
3. Add the repository secret `SONAR_TOKEN` (GitHub > Settings > Secrets > Actions).

The workflow runs on pushes to `main` and on pull requests from the same repository, produces
`lcov.info` (`cargo llvm-cov --workspace`) and `clippy-report.json`, and scans. Without the secret
it is skipped with a notice.

The README badges point at the workflow file by name (`ace-ci<version>.yml`) and at the Sonar
project key; when the workflow file is renamed in a version bump, update the build-status badge
in `README.md` in the same commit.
