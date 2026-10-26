# ACE 0.8.0 — implementation order (Storage engine: segments, columns, dictionaries)

Base: 0.7.0. Concept: `ROADMAP.md` (0.8.0). Format **1.6** (additive, not 2.0).
Rules for every task: `cargo clippy --workspace --all-targets -- -D warnings` clean, rustdoc /
Javadoc / Scaladoc on every item, one task = one commit, **no `#[cfg(test)]` module in any `src/`
file** (tests live in `crates/<crate>/tests/`; JVM tests in `jvm/<module>/src/test/`), and every
file written by 0.2.1 – 0.7.0 still decodes byte for byte (`examples/compat/` is never
regenerated, 1.6 fixtures are added).
**Default configuration does not change bytes.** A file is written as Format 1.6 only when a
segmented block, a columnar block or a dictionary is used; everything else is written exactly as by
0.7.0 (`examples/golden/0.6.1/` stays the reference for the unchanged workloads, `0.8.0/` is added
for the new ones) — every binding (Rust, C, Java, Scala) must reproduce both.
`unsafe` stays confined to audited files: `ace-simd` (NEON joins AVX2), `ace-ffi`.
Status: ✅ done in this release · ⏩ executed by the release pipeline / manually on the reference
machine.

## Phase A — version, carry-overs

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 1 ✅ | Version 0.8.0; scripts / tools / workflows renamed; 0.7.0 task list and milestone → `docs/history/`; A/B base = 0.7.0 | "Bumps version to 0.8.0 and renames scripts and tools" | `ace version` = 0.8.0; package name audit |
| 2 ✅ | JVM decompress margin (0.90× – 0.97× of the facade over the 0.7.0 runs): measure the caller-owned `decompressInto` path next to the zero-copy path and report it as a diagnostic | "Measures the caller-owned JVM decompress path" | `ffi.jvm_into.decompress_16m` diagnostic; the 0.90× gate is unchanged |
| 3 ✅ | Native libraries for macOS (aarch64) and Windows (x86_64) in CI, packaged under `native/<os>-<arch>/` | "Builds the native library on macOS and Windows in CI" | CI jobs `native-macos`, `native-windows`: ffi tests and the Java SDK tests pass |
| 4 ✅ | NEON backend for aarch64 (`ace-simd`): the scans that AVX2 has | "Adds the NEON backend for aarch64" | equivalence with the scalar backend on random input; ARM64 CI job passes; `ACE_SIMD=scalar` still works |

## Phase B — Format 1.6

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 5 ✅ | `docs/FORMAT-1.6.md`; `CodecId::Segmented` (5) and `Columnar` (6); minor version 6; `payload_minor_version`; reader gate; `DecodeLimits` for segments, columns and dictionaries | "Specifies Format 1.6 and gates its codec ids" | a 1.5 file with codec 5 / 6 is rejected; limits documented in the table on the type |
| 6 ✅ | Dictionary section (`ADIC`) after the file header, `FILE_FLAG_HAS_DICTIONARIES`, writer and reader | "Stores dictionaries in the container" | round trip of the section; forged sizes fail before allocation; CRC per dictionary |
| 7 ✅ | SEG1: segment table framing (build, parse, validate) | "Adds the SEG1 segment table" | sums, order and limits validated; property tests |
| 8 ✅ | COL1: column table framing and the record transpose / un-transpose | "Adds the COL1 column table" | transpose round trip for every width; tail bytes; limits |

## Phase C — adaptive segmentation

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 9 ✅ | Deterministic segmenter: windows classified with the existing analysis, merged into segments | "Adds the segmenter" | same input → same boundaries on every host; min / max segment size honoured |
| 10 ✅ | Segmented block encoder: each segment is an inner block with its own plan; used only if smaller by a margin | "Encodes blocks as segments" | never larger than the unsegmented block; stats count segmented blocks |
| 11 ✅ | Segmented block decoder in every reader (`decompress`, `decompress_into`, streaming, indexed) | "Decodes segmented blocks in every reader" | all readers agree byte for byte; reader gate for 1.5 |
| 12 ✅ | Segment-level random access: a range read decodes only the segments it touches | "Reads ranges at segment granularity" | physical bytes read ≤ the unsegmented block; metrics |
| 13 ✅ | Segmentation tests: malformed matrix, determinism matrix, fuzz target `seg_decode` | "Tests and fuzzes segmented blocks" | no panic; deterministic across threads and SIMD backends |

## Phase D — columnar blocks

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 14 ✅ | COL1 encoder and decoder: columns become inner blocks | "Encodes and decodes columnar blocks" | round trip for random schemas; tail bytes kept |
| 15 ✅ | Per-column codec selection by column kind (integer, float, bytes) | "Selects a codec per column" | deterministic tie-breaks; never worse than the row-wise block by the margin |
| 16 ✅ | `Schema` / `Column` / `ColumnKind` in the facade `Config`; blocks aligned to whole records | "Adds schemas to the configuration" | block size rounded to the record size; invalid schemas rejected |
| 17 ✅ | Mixed-record corpus, benchmark family `storage`, tests, fuzz target `col_decode` | "Benchmarks and fuzzes columnar blocks" | ≥ 10 % smaller than row-wise on the record corpus |

## Phase E — trained dictionaries

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 18 ✅ | Deterministic dictionary trainer (`ace-dictionary`) | "Trains dictionaries from samples" | same samples → same bytes; size cap |
| 19 ✅ | LZ with a preset dictionary (history before the input; the LZ stream is unchanged) | "Adds preset dictionaries to LZ" | round trip; decoder rejects references outside dictionary + output |
| 20 ✅ | Engine: dictionary candidate per block, embedded (`File`) or external, registry lookup on decode, `MissingDictionary` | "Compresses with dictionaries" | embedded files are self-contained; external files need the dictionary |
| 21 ✅ | Facade and CLI: `Dictionary::train`, `Config` options for dictionary, schema and segmentation, `ace train`, `--dictionary`, `--schema`, `--segments` | "Exposes dictionaries, schemas and segments in the facade and CLI" | CLI round trips |

## Phase F — reuse, ABI, SDKs

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 22 ✅ | `WorkerScratch` reuse: encoder buffers live per worker | "Reuses encoder buffers per worker" | allocation bytes per input byte (mixed 4 MiB) not higher than 0.7.0; bytes identical |
| 23 ✅ | C ABI 1.1: dictionary, schema and segmentation functions; `ace_abi_version()` stays 1, `ace_abi_minor()` = 1; `include/ace.h` regenerated | "Extends the C ABI to 1.1" | the 26 functions of 1.0 unchanged; ffi tests and `smoke.c` |
| 24 ✅ | Java SDK: `Dictionary`, `Schema`, options, tests | "Adds dictionaries and schemas to the Java SDK" | JUnit on JDK 22 and 25 |
| 25 ✅ | Scala module `jvm/ace-schema`: schema inference from case classes, dictionary store, tier profiles | "Adds the ace-schema Scala module" | ScalaTest; golden bytes through the SDK |

## Phase G — qualification, release

| # | Task | Commit | Acceptance |
|---|---|---|---|
| 26 ✅ | Golden `0.8.0/`, compat fixtures for 1.6, fuzz targets in CI, audits (unsafe, panic, decoder) | "Adds golden, compatibility fixtures and audits for Format 1.6" | all green; `examples/compat/` older files untouched |
| 27 ✅ | Benchmarks (family `storage`), Regression V3 gates, A/B vs 0.7.0, release notes, migration guide, README, CHANGELOG, ROADMAP | "Documents 0.8.0 and adds the storage gates" | A/B ≥ 0.95× per case on unchanged paths |
| 28 ⏩ (results in, see release notes) | Release run, fuzz campaign (22 targets), baseline `examples/baselines/0.8.0/`, `docs/PERFORMANCE-0.8.0.md`, tag `v0.8.0` | [`docs/RELEASE.md`](docs/RELEASE.md) | Regression V3 pass; A/B vs 0.7.0 pass |

## Decisions recorded here

- **Format 1.6, not 2.0.** Both new block kinds are new codec ids; a file that uses none of them
  stays 1.3 – 1.5 and an old reader keeps reading it. Shared context between blocks is *not* part of
  0.8.0 (it conflicts with independent blocks and random access); candidate for 0.9.0.
- A segment and a column are **inner blocks**: the same serialization as a block (header, CRC,
  metadata, payload), so the existing pipeline, limits and checks apply unchanged. Nesting is one
  level deep; an inner block of codec 5 or 6 is rejected.
- Dictionaries are LZ preset history (at most 64 KiB, the LZ window). They are an engine-level
  candidate, not a planner route: the planner invariants (no trial encodes) are unchanged.
- Rust owns everything that defines bytes (segmenter, column codecs, trainer, LZ, formats). Scala
  owns policy: schema inference, the dictionary store and tier profiles.

## Out of scope

Shared context between blocks, async API, metrics exporters, deterministic mode for GraphNet
(0.9.0); a Maven Central or crates.io release (1.0).
