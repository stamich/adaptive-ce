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
