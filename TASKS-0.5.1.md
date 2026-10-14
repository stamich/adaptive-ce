# ACE 0.5.1 — implementation order (Hardened Release Candidate)

Base: 0.5.0. Concept: `AdaptiveCE_0_5_1_Koncepcja_v2.md`. Rules for every task:
`cargo clippy --workspace --all-targets -- -D warnings` clean, rustdoc on every item, one task =
one commit, and **golden 0.5.0 reproduced byte for byte** (auto and `ACE_SIMD=scalar`) — 0.5.1
changes no planner decision, no codec, no route and no format byte.
Status: ✅ done in this release · ⏩ executed by the release pipeline / manually on the reference
machine.

## Phase A — version and toolchain

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 1 ✅ | Version 0.5.1, `MILESTONE-0.5.1.json`, this file; 0.5.0 summaries → `docs/history/` | Bumps version to 0.5.1 | `ace --version` = 0.5.1 |
| 2 ✅ | Scripts / tools renamed to 0.5.1; A/B and stored baseline = 0.5.0; package audit rejects 0.5.0 names | Renames scripts and tools to 0.5.1 | package name audit |
| 3 ✅ | `as_chunks` for constant chunk sizes outside hot kernels; hot kernels unchanged with a justified allow (clippy 1.98 `chunks_exact_to_as_chunks`) | Adopts as_chunks for constant chunk sizes outside hot kernels | clippy 1.98.1 clean (CI) |
| 4 ✅ | Build toolchain 1.98.1 (vtable fix of 1.98.0), MSRV 1.97.0 with an explicit `+1.97.0` CI job | Pins the build toolchain to Rust 1.98.1 | CI `pr` + `msrv` green |

## Phase B — lints and rustdoc

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 5 ✅ | `clippy::unreachable = deny` everywhere; the one use replaced by a closed table | Denies unreachable! in every crate | clippy clean |
| 6 ✅ | CI keeps the job log and re-emits errors / failed tests / panics as annotations | Reports CI failures as check-run annotations | failure visible in the check run |
| 7 ✅ | `missing_errors_doc` / `missing_safety_doc` = deny; `# Errors` on 81 public fallible functions; stale "1.0–1.3" docs fixed | Requires Errors and Safety sections in public docs | clippy + rustdoc clean |
| 8 ✅ | rustdoc `-D warnings --document-private-items` as a build gate | Builds rustdoc for private items as a gate | `ace-build0.5.1.sh` |
| 9 ✅ | `# Determinism` / `# Invariants` for prefilters, estimators, planner policies, encoders | Documents determinism and invariants of the planner, estimators and encoders | review |

## Phase C — decoder audit and resource limits

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 10 ✅ | `ace-format` parsers under the arithmetic lints; index count `try_from` before arithmetic; descriptors split without index arithmetic; writer rejects > 255 transforms | Checks arithmetic in the ace-format parsers | per-field tests |
| 11 ✅ | NUM1 / TS1 header parsers under the lints; `try_from` counts; `unsigned_abs` delta check | Checks arithmetic in the NUM1 and TS1 header parsers | `header_fields.rs` |
| 12 ✅ | rANS model / rANS4x under the lints; lanes split with `split_at_checked`; overflow-free lane sizes | Checks arithmetic in the rANS and rANS4x metadata parsers | `metadata_fields.rs`, decode speed within noise |
| 13 ✅ | Strict stream ends: rANS final state, Huffman padding, empty output ⇒ empty payload | Rejects rANS and Huffman streams that do not end where the encoder stopped | all compat fixtures still decode |
| 14 ✅ | Bit unpacking under the lints | Checks arithmetic in the bit-unpacking parser | clippy clean |
| 15 ✅ | Index allocation bounded by `max_index_entries`; trailer offset constant | Bounds the index allocation by the index entry limit | `trailer_limits.rs` |
| 16 ✅ | `DecodeLimits::max_block_count()` in every reader; limit table on `DecodeLimits` | Applies the block-count limit in every reader and documents each limit | `decode_limits.rs` |
| 17 ✅ | "Limit ⇒ error before allocation" with a counting allocator (tests serialized: the allocator counts every thread) | Tests that forged sizes fail before allocation; Runs the allocation-measuring tests one at a time | peak < 1 MiB |
| 18 ✅ | Generated `docs/DECODER-AUDIT.md`; the audit fails when a referenced test / fuzz target / lint disappears | Generates and verifies the decoder audit | `tools/ace-code_audit0.5.1.py` |
| 19 ✅ | Primary stream of entropy-coded blocks ≤ 2 × block + 1 KiB (closes probability-one rANS amplification); block pipeline under the lints | Bounds the primary stream length of entropy-coded blocks | `forged_primary_stream_length_fails_before_allocation` |

## Phase D — compatibility

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 20 ✅ | `examples/compat/`: 48 files written by 0.2.1, 0.3.0, 0.4.4, 0.4.6, 0.5.0 (Formats 1.1 – 1.4), manifest, frozen generators | Adds frozen compatibility fixtures written by ACE 0.2.1 to 0.5.0 | `compat_fixtures.rs` |
| 21 ✅ | Compatibility rules (every new TS1 mode = new format minor) and 0.5.1 decoder strictness | Documents the format compatibility rules and the 0.5.1 decoder strictness | review |

## Phase E — determinism

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 22 ✅ | 12 / 16 / 32 workers, in-flight 1 / 7 / 64, streaming at 32 | Extends the determinism matrix to 12, 16 and 32 workers | identical bytes |
| 23 ✅ | Fixed tie-breaks: NUM1 width / mode ranks, candidate-order independence | Tests that ties are broken by fixed keys, not by candidate order | `tie_break.rs` |

## Phase F — Planner V5 invariants

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 24 ✅ | TS1 ⇔ selection + dominance rule, NumericFast without Float evidence, selected ∈ candidates, zero full trials — corpus × profiles × lane on / off + generated input | Adds Planner V5 invariant property tests | `v5_invariants.rs` |

## Phase G — fuzzing

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 25 ✅ | `fuzz-smoke` mode and CI job (15 × 25 s), corpus as cache / artifact, `cmin` in the campaign, policy document | Adds a fuzz smoke run to CI and documents the corpus policy | CI job |
| 26 ⏩ | Fuzz campaign 15 × 10 min (reference machine) | `ACE_FUZZ_SECONDS=600 ACE_FUZZ_CMIN=1 ./ace-ci0.5.1.sh fuzz` | 0 crashes, `examples/baselines/0.5.1/FUZZ.md` |

## Phase H — benchmarks

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 27 ✅ | Schema 2.2 (git revision + dirty, config fingerprint, threads, block size, input SHA-256); validator 2.0 – 2.2 | Adds benchmark schema 2.2 with build revision, configuration and input fingerprints | validator |
| 28 ✅ | `tools/ace-repeatability0.5.1.py` (deterministic values identical, macro spread ≤ 5 % / decompression ≤ 15 %, median documents) | Adds the benchmark repeatability tool | detects a forged mismatch |
| 29 ✅ | `--repeat N`, release runs three times, governor gate (`diagnostic` instead of `pass`) | Runs benchmarks three times for a release and refuses a pass on a non-performance governor | scripts |
| 30 ✅ | `allocation-profile` family + report table | Adds the allocation-profile benchmark family | evaluator = 94 % of the int-sparse-change churn |
| 31 ⏩ | Three runs on the Ryzen 9 5950X → `examples/baselines/0.5.1/` | `./ace-release0.5.1.sh ../ace-0.5.0` | checklist all PASS |

## Phase I — CLI

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 32 ✅ | `BrokenPipe` → quiet exit 0 (no `unsafe`, fallible writes) | Exits quietly instead of panicking when stdout is closed | `cli.rs` |
| 33 ✅ | `ace version` (build.rs git revision) | Adds the ace version command | `cli.rs` |
| 34 ✅ | Output naming the input is refused | Refuses to write the output over the input file | `cli.rs` |
| 35 ✅ | End-to-end tests: round trip, invalid input, usage errors, permissions, closed pipe | Adds end-to-end CLI tests for invalid input, version and closed pipes | 7 tests |

## Phase J — API inventory

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 36 ✅ | Public vs internal crates stated in every crate's docs | Declares which crates are public API and which are internal | review |
| 37 ✅ | `#[deprecated]` Planner V3 / route-less V4 entry points (nothing removed) | Deprecates the Planner V3 and route-less V4 entry points | no breaking change |
| 38 ✅ | Generated `docs/API-INVENTORY.md` with the 0.6 facade plan | Generates the public API inventory | generated |

## Phase K — documentation

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 39 ✅ | `ARCHITECTURE.md` | Adds the canonical architecture document | links resolve |
| 40 ✅ | `FORMAT.md`; `FORMAT-1.1.md` title fixed | Adds the canonical container format document | review |
| 41 ✅ | `COMPATIBILITY.md` (format, API, toolchain, benchmark documents) | Turns the format compatibility notes into the canonical compatibility document | review |
| 42 ✅ | `SECURITY.md` (reporting, 0.5.1 evidence) | Updates the security model for the 0.5.1 decoder hardening | review |
| 43 ✅ | `BENCHMARKS.md` | Adds the canonical benchmark methodology document | review |
| 44 ✅ | `RELEASE.md` | Adds the canonical release process document | review |
| 45 ✅ | `UNSAFE-AUDIT.md`, generated `PANIC-AUDIT.md` (audit covers test `unsafe` too) | Makes the unsafe and panic audits canonical documents | audit tool |
| 46 ✅ | `RELEASE-NOTES-0.5.1.md` | Adds the 0.5.1 release notes | review |
| 47 ✅ | README as a product document | Rewrites the README as a product document | review |
| 48 ✅ | ROADMAP 0.6 – 1.0 and platform phases | Updates the roadmap with the 0.6 - 1.0 plan and the platform phases | review |
| 49 ✅ | Demo / golden doc references | Refreshes release references in the demo and golden test docs | review |
| 50 ✅ | CHANGELOG | Adds the 0.5.1 changelog | review |
| 51 ✅ | This file and the milestone summary | Completes the 0.5.1 task list and milestone summary | review |

## Phase L — release

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 52 ✅ | `dist/` with source / benchmark archives, `RELEASE-REPORT.md`, `MANIFEST.sha256` | Writes the release distribution and release report | generated |
| 53 ⏩ | Release run, fuzz campaign, baseline `examples/baselines/0.5.1/`, `docs/PERFORMANCE-0.5.1.md`, tag `v0.5.1` | [`docs/RELEASE.md`](docs/RELEASE.md) | checklist all PASS |
