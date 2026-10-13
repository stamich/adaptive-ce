# ACE release process

Canonical process of the current release line (0.5.x). Earlier checklists:
[`history/`](history/) (`RELEASE-CHECKLIST-0.4.6.md`, `RELEASE-CHECKLIST-0.5.0.md`).

## 1. Automatic pipeline

```bash
git worktree add ../ace-0.5.0 v0.5.0            # the previous release, for the interleaved A/B
./ace-release0.5.1.sh ../ace-0.5.0              # on the reference machine, performance governor
```

`ace-release0.5.1.sh` runs every step below, records each in
`examples/results/release-checklist-0.5.1.md`, and writes the distribution to `dist/`:

| # | Step | Gate |
|---|---|---|
| 1 | `ace-build0.5.1.sh`: fmt, clippy `-D warnings`, check, tests, release build, rustdoc (public and private items) | 0 warnings |
| 2 | `tools/ace-code_audit0.5.1.py`: `unsafe` placement and `// SAFETY:`, justified clippy allowances, decoder audit (lints present, every referenced test and fuzz target exists) | no violation |
| 3 | `cargo test --release` (property budget 10 000) incl. `--ignored` large-file tests | all pass |
| 4 | golden SHA-256 0.5.0 and frozen 0.4.6, auto and `ACE_SIMD=scalar` | identical bytes |
| 5 | CLI determinism: 10 processes × threads × SIMD backend, mixed + f64-noisy | one SHA-256 |
| 6 | `ace-benchmark0.5.1.sh all --repeat 3` | repeatability PASS |
| 7 | interleaved A/B vs the previous release + Regression V3 (Unstable → ≤ 2 re-measurements) | PASS (`diagnostic` on a non-`performance` governor is not a pass) |
| 8 | `docs/PERFORMANCE-0.5.1.md` from the results | generated |
| 9 | demo | < 1 min, no error |
| 10 | deterministic source archive, built twice | same SHA-256, script-name audit |
| 11 | unpack the archive, run the tests and the golden check from it | PASS |
| 12 | `dist/`: `ace-0.5.1-source.zip`, `ace-0.5.1-benchmarks.zip`, `RELEASE-REPORT.md` (git revision, toolchain, host, every gate verdict), `MANIFEST.sha256` | written |

`--quick` runs the same pipeline with smoke-sized measurements; its verdict is never a
release.

## 2. Acceptance matrix

| Area | Gate | Evidence |
|---|---|---|
| Build, lints, rustdoc (incl. private items) | 0 warnings | step 1 |
| MSRV 1.97 | PASS | CI `msrv` job (`cargo +1.97.0`) |
| Build toolchain 1.98.1 | PASS | CI `pr` job (`rust-toolchain.toml`) |
| Unsafe / panic / decoder audit | PASS, no new `unsafe` | step 2, [`DECODER-AUDIT.md`](DECODER-AUDIT.md) |
| Golden 0.5.0 + frozen 0.4.6, auto + scalar | identical | step 4 |
| Compatibility fixtures (0.2.1 – 0.5.0) | every file decodes byte for byte | `compat_fixtures` |
| Format readers 1.0 – 1.4, malformed input, limits before allocation | rejected, no panic, no large allocation | `malformed_matrix`, `header_fields`, `metadata_fields`, `decode_limits`, `limits_before_allocation`, `trailer_limits` |
| Determinism (threads 1 – 32, in-flight batches, API paths, processes, backends) | identical | `determinism_matrix`, step 5 |
| Planner V5 invariants, fixed tie-breaks | PASS | `v5_invariants`, `tie_break` |
| CLI (invalid input, closed pipe, `ace version`) | PASS | `crates/ace-cli/tests/cli.rs` |
| Fuzz smoke 15 × 25 s | 0 crashes | CI `fuzz-smoke` job |
| Fuzz campaign 15 × 10 min (*manual*) | 0 crashes | `examples/baselines/<version>/FUZZ.md` |
| Quality, Float lane, stability, performance vs the previous release | PASS | Regression V3 + A/B |
| Repeatability (3 runs) | deterministic values identical, macro spread ≤ 5 % | `benchmark-<version>-repeatability.json` |
| Reproducible package, tested from the archive | PASS | steps 10 – 11 |
| Documentation | README, RELEASE-NOTES, COMPATIBILITY, SECURITY, BENCHMARKS, audits, CHANGELOG, ROADMAP | review |

## 3. Manual steps (reference machine: AMD Ryzen 9 5950X)

1. Set the `performance` governor, stop other work, run the pipeline (section 1).
2. Fuzz campaign, recorded as `examples/baselines/<version>/FUZZ.md`:
   `ACE_FUZZ_SECONDS=600 ACE_FUZZ_CMIN=1 ./ace-ci0.5.1.sh fuzz`.
3. Publish the baseline: copy the median documents of step 6 (`examples/results/benchmark-*.json`,
   `ab-*.json`) to `examples/baselines/<version>/` with a `BASELINE.json` descriptor (machine,
   governor, toolchain, git revision, verdicts), and the generated
   `docs/PERFORMANCE-<version>.md`.
4. Merge, then tag the merge commit:

   ```bash
   git tag -a v0.5.1 -m "ACE 0.5.1 — hardened release candidate" -m "$(cat dist/RELEASE-REPORT.md)"
   git push origin v0.5.1
   ```

5. Attach `dist/*` to the GitHub release.

## 4. Version bump checklist (next release)

Rename the versioned scripts and tools (`ace-*<version>.sh`, `tools/ace-*<version>.*`) — the
package audit rejects stale names — update `ACE_VERSION` / `ACE_BASELINE`
(`tools/ace-common<version>.sh`), `MILESTONE` / `BASELINE` (`tools/ace-benchlib<version>.py`),
`MILESTONE` / `BASE_MILESTONE` (`crates/ace-bench/src/json.rs`), the workspace version, and
add `TASKS-<version>.md` / `MILESTONE-<version>.json` (earlier ones move to `docs/history/`).
