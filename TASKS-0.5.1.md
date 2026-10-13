# ACE 0.5.1 — implementation order (Hardened Release Candidate)

Base: 0.5.0. Concept: `AdaptiveCE_0_5_1_Koncepcja_v2.md`. Rules for every task:
`cargo clippy --workspace --all-targets -- -D warnings` clean, rustdoc on every item, one task =
one commit, and **golden 0.5.0 reproduced byte for byte** (auto and `ACE_SIMD=scalar`) after
every step — 0.5.1 changes no planner decision, no codec, no route and no format byte.
Status: ✅ done in this release · ⏩ executed by the release pipeline on the reference machine.

## Phase A — version and toolchain

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 1 ✅ | Version 0.5.1, `MILESTONE-0.5.1.json`, this file; 0.5.0 summaries → `docs/history/` | Bumps version to 0.5.1 | `ace --version` = 0.5.1 |
| 2 ✅ | Scripts / tools renamed to 0.5.1; A/B and stored baseline = 0.5.0; package audit rejects 0.5.0 names | Renames scripts and tools to 0.5.1 | package name audit |
| 3 ✅ | Build toolchain pinned to 1.98.1 (vtable fix of 1.98.0), MSRV stays 1.97.0 with an explicit `+1.97.0` CI job | Pins the build toolchain to Rust 1.98.1 | CI `pr` + `msrv` green |

## Phase B — lints and rustdoc

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 4 ✅ | `clippy::unreachable = deny` (workspace, `ace-simd`, `ace-bench`); the one use replaced by a closed table | Denies unreachable! in every crate | clippy clean |
| 5 ✅ | CI keeps the job log and re-emits errors / failed tests / panics as check-run annotations | Reports CI failures as check-run annotations | failure visible in the check run |
| 6 ✅ | `missing_errors_doc` / `missing_safety_doc` = deny; `# Errors` on all 81 public fallible functions; stale "1.0–1.3" docs corrected to 1.0–1.4 | Requires Errors and Safety sections in public docs | clippy + rustdoc clean |

## Phase C — decoder audit and resource limits

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 7 ✅ | `ace-format`: `arithmetic_side_effects` + `cast_possible_truncation` in the parser modules; index count checked with `try_from` before any arithmetic; descriptors split without index arithmetic; writer rejects > 255 transforms | Checks arithmetic in the ace-format parsers | per-field tests (count 0 / `u32::MAX` / `u64::MAX` / overflow) |
