# ACE 0.6.1 — implementation order (Numeric & Float II, Phase B of the 0.6 line)

Base: 0.6.0. Concept: `ROADMAP.md` (0.6.1) and `docs/FLOAT-CALIBRATION-0.5.0.md` §1 findings 2 and 3.
Rules for every task: `cargo clippy --workspace --all-targets -- -D warnings` clean, rustdoc on
every item, one task = one commit, **no `#[cfg(test)]` module in any `src/` file** (tests live in
`crates/<crate>/tests/`), and every file written by 0.2.1 – 0.6.0 still decodes byte for byte
(`examples/compat/` is never regenerated; new fixtures are only added).
Unlike 0.6.0, this release **changes bytes**: blocks that gain a better codec shrink. The golden
file of 0.6.0 stays frozen; `examples/golden/0.6.1/` is the new reference.
Status: ✅ done in this release · ⏩ executed by the release pipeline / manually on the reference
machine.

## Phase A — version

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 1 | Version 0.6.1, `MILESTONE-0.6.1.json`, this file; 0.6.0 summaries → `docs/history/`; stored baseline and A/B base = 0.6.0 | Bumps version to 0.6.1 | `ace version` = 0.6.1 |
| 2 | Scripts / tools / workflows renamed to 0.6.1 | Renames scripts and tools to 0.6.1 | package name audit |

## Phase B — test layout

Unit tests move out of `src/` into `crates/<crate>/tests/<module>.rs`, one commit per crate. A
test may use only the crate's public items; where a test needed a private helper, the helper is
tested through the item that uses it, or it becomes `pub` with rustdoc (internal crates are
`publish = false`; the public API is the `ace` crate).

| # | Task | Commit |
|---|---|---|
| 3 | `ace-bitpack`, `ace-simd`, `ace-core`, `ace-corpus`, `ace-runtime`, `ace-transforms`, `ace-dictionary` | one commit each: "Moves the tests of ace-X to tests/" |
| 4 | `ace-entropy`, `ace-codecs`, `ace-format`, `ace-cost`, `ace-analysis` | same |
| 5 | `ace-planner`, `ace-engine`, `ace` | same |
| 6 | `ace-bench` gets a library target so its tests can live in `tests/` | "Splits ace-bench into a library and a binary" + move |
| 7 | `tools/ace-code_audit0.6.1.py` rejects `#[cfg(test)]` in `src/`; test-layout rule in `docs/ARCHITECTURE.md` | "Forbids unit tests inside src" |

## Phase C — Format 1.5

| # | Task | Commit |
|---|---|---|
| 8 | `FORMAT_MINOR = 5`; the writer declares the lowest minor version the **payloads** need (NUM1 mode 4 and TS1 mode 4 → 1.5; everything else 1.3 / 1.4 as before); a reader rejects a payload feature newer than the file's minor version; `docs/FORMAT-1.5.md` | "Adds format 1.5 with payload-aware minimal versions" |

## Phase D — NUM1 Patched FOR

| # | Task | Commit |
|---|---|---|
| 9 | NUM1 mode 4 `PatchedFor`: FOR with a small bit width plus a list of exceptions; exact allocation-free estimator, encoder, hardened decoder; property tests, malformed table | "Adds Patched FOR to NUM1" |
| 10 | `estimate_numeric` and the planner's NumericGeneral route take Patched FOR into account; `monotonic-outliers` improves from 1.78× | "Lets the planner choose Patched FOR" |

## Phase E — RunDelta for small deltas

| # | Task | Commit |
|---|---|---|
| 11 | Run prefilter admits lanes whose deltas are small (counters with resets, slowly drifting integers) in addition to lanes whose values repeat; exact RunDelta size competes with the generic plan under the existing dominance rule; no format change | "Admits RunDelta for small non-zero deltas" |

## Phase F — ALP decimal floats

| # | Task | Commit |
|---|---|---|
| 12 | TS1 mode 4 `Alp` (f64 / f32): per-block exponent and factor, FOR + bit packing of the scaled integers, exception list for values that do not round-trip; exact bit-counting size; hardened decoder | "Adds the ALP decimal-float codec" |
| 13 | Float prefilter / estimator detect decimal data; ALP competes in the Float lane and in FloatFast | "Lets the Float lane choose ALP" |
| 14 | Chimp / Chimp128 implemented in the benchmark crate only; `chimp-evaluation` family; kept in the format only if at least 5 % more blocks shrink than with the best existing choice — decision recorded in `docs/BENCHMARKS.md` | "Evaluates Chimp against the 5 % rule" |

## Phase G — DENSE never larger than BALANCED

| # | Task | Commit |
|---|---|---|
| 15 | DENSE compares its choice with the BALANCED choice by exact size and takes the smaller; `zeros`, `runs`, `mixed` no longer lose; documented cost | "Makes DENSE never larger than BALANCED" |

## Phase H — qualification

| # | Task | Commit |
|---|---|---|
| 16 | Golden 0.6.1 (`examples/golden/0.6.1/`), 0.6.0 golden still reproduced where no new codec applies; new compat fixtures for 1.5; determinism and malformed matrices cover the new modes | "Adds the 0.6.1 golden file and 1.5 fixtures" |
| 17 | Fuzz targets cover NUM1 mode 4 and TS1 mode 4 | "Extends the fuzz targets to the new modes" |
| 18 | Benchmark workloads and Regression V3 gates for `int-counter-reset`, `monotonic-outliers`, decimal floats; A/B base 0.6.0 | "Adds the 0.6.1 benchmark gates" |
| 19 | `docs/RELEASE-NOTES-0.6.1.md`, `CHANGELOG.md`, `README.md`, `ROADMAP.md`, `ARCHITECTURE.md`, `COMPATIBILITY.md`, `TS1-CODEC.md`, `PLANNER-V5.md`, `MIGRATION-0.6-TO-0.6.1.md`, API inventory | "Documents ACE 0.6.1" |
| 20 ⏩ | Release run, fuzz campaign, baseline `examples/baselines/0.6.1/`, `docs/PERFORMANCE-0.6.1.md`, tag `v0.6.1` | [`docs/RELEASE.md`](docs/RELEASE.md) |
