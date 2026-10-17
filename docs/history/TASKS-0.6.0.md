# ACE 0.6.0 — implementation order (API & Performance, Phase A)

Base: 0.5.1. Concept: `docs/COMPETITIVE-ANALYSIS-KANZI.md` §6 and the 0.6 plan in `ROADMAP.md`.
Rules for every task: `cargo clippy --workspace --all-targets -- -D warnings` clean, rustdoc on
every item, one task = one commit, and **golden 0.5.0 reproduced byte for byte** (auto and
`ACE_SIMD=scalar`) — 0.6.0 changes no planner decision, no codec, no route and no format byte.
The new codecs (RunDelta / NUM1 small deltas, Patched FOR, ALP, Chimp evaluation) and format 1.5
are **Phase B = ACE 0.6.1**.
Status: ✅ done in this release · ⏩ executed by the release pipeline / manually on the reference
machine.

## Phase A — version and toolchain

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 1 ✅ | Version 0.6.0, `MILESTONE-0.6.0.json`, this file; 0.5.1 summaries → `docs/history/`; stored baseline and A/B base = 0.5.1 | Bumps version to 0.6.0 | `ace version` = 0.6.0 |
| 2 ✅ | Scripts / tools / workflow renamed to 0.6.0; README badge; package audit rejects 0.5.1 names | Renames scripts and tools to 0.6.0 | package name audit |
| 3 ✅ | MSRV = build toolchain = 1.98.1 (`rust-version`, `clippy.toml`); the separate `msrv` CI job is removed | Raises the MSRV to Rust 1.98.1 | CI `pr` green |

## Phase B — forward compatibility

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 4 ✅ | A file of a newer minor format (1.5, written by ACE ≥ 0.6.1) is rejected with an error that names the needed ACE version, by every reader | Names the required ACE version when a file is newer | forged-1.5 tests in every reader |

## Phase C — the `ace` facade

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 5 ✅ | Crate `ace`: `Error` (non-exhaustive, `#[from]` of the internal error) and `Result` | Adds the ace facade crate with Error | doctest |
| 6 ✅ | `Profile`, `Config` and `ConfigBuilder` (validated, defaults = `AceConfig::default()`) | Adds Config and ConfigBuilder to the facade | tests: defaults, validation |
| 7 ✅ | `DecodeLimits` with a builder (non-exhaustive; every `ace-core` limit) | Adds DecodeLimits and its builder to the facade | tests: every limit round-trips |
| 8 ✅ | `Stats` (stable subset of `CompressionStats`) | Adds Stats to the facade | test: ratio and counters |
| 9 ✅ | `Engine`: compress / compress_with_stats / compress_to / decompress / decompress_into / decompress_from / `compress_stream` | Adds Engine to the facade | round trip = `AceEngine` bytes |
| 10 ✅ | `RangeReader` (random access over `Read + Seek`) and `RangeStats` | Adds RangeReader to the facade | range = slice of the input |
| 11 ✅ | feature `explain`: `Engine::explain` → `BlockReport` | Adds the explain feature to the facade | doctest |
| 12 ✅ | Facade integration tests: golden SHA-256 through the facade, hostile input through the facade, all limits | Tests the facade end to end | golden identical |
| 13 ✅ | `ace` CLI compresses, decompresses, verifies and reads ranges through the facade (`inspect`, `explain`, `version` show internals and stay on them) | Moves the CLI to the facade API | `cli.rs` unchanged |
| 14 ✅ | Internals hidden: all crates except `ace` and `ace-cli` are `publish = false` (`ace` cannot be published to crates.io until the internals are: see the release notes); the deprecated `evaluate_candidates_v3` / `_v4` are removed; the inventory tool classifies `ace` as the public API and fails on a public item without docs | Hides the internal crates and removes deprecated items | `ace-code_audit` + inventory |

## Phase D — performance recovery (bytes unchanged)

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 15 ✅ | Profile the decoders (dense decode was 0.961× of 0.5.0) and recover what the strict checks cost without weakening them: the 0.5.1 interleave loop of `rans4x_decode` tested every byte; lanes are now checked once (125 → 147 – 157 MB/s isolated) | Restores decode speed of the four-lane rANS interleave | A/B ≥ 0.98× for decode ⏩ (release run) |
| 16 ✅ | Remove the allocation churn of the generic evaluator (≈ 36 B per input byte on `int-sparse-change` BALANCED): it came from `lz_encode`'s per-call tables, now kept per thread for inputs up to 64 KiB (→ 5.9 B) | Reuses the LZ hash tables across planner trial encodes | `allocation-profile` drops, golden identical |
| 17 ✅ | DENSE vs BALANCED: decision recorded (ratio gap, time); finding: DENSE is larger on `zeros` / `runs`, fix deferred to 0.6.1 | Records the DENSE decision | `docs/BENCHMARKS.md` |

## Phase E — benchmarks

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 18 ✅ | `public-corpus` family: Silesia / enwik8 from `ACE_PUBLIC_CORPUS`, ratio and MB/s in the lzbench columns; skipped when the directory is absent | Adds the public-corpus benchmark family | family runs on a directory |

## Phase F — documentation and qualification

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 19 ✅ | `docs/MIGRATION-0.5-TO-0.6.md`, README quick start with the facade, ARCHITECTURE, COMPATIBILITY (MSRV policy), CHANGELOG, RELEASE-NOTES-0.6.0, ROADMAP | Documents the 0.6.0 API | review |
| 20 ✅ / ⏩ | Release run, fuzz campaign, baseline `examples/baselines/0.6.0/`, `docs/PERFORMANCE-0.6.0.md` done on the reference machine (Regression V3 pass 73 / 0 fail, A/B 12 / 12 pass, repeatability pass, fuzz 15 × 600 s without a crash); tag `v0.6.0` after the merge ⏩ | [`docs/RELEASE.md`](docs/RELEASE.md) | checklist all PASS |
