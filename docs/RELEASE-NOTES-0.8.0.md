# ACE 0.8.0 — release notes (Storage engine)

0.8.0 adds three storage-engine features that a database or an object store wants: **adaptive
segmentation** (a block that mixes kinds of data is coded as segments, each with its own plan),
**columnar blocks** (fixed-size records are stored column by column) and **trained preset
dictionaries** (small blocks of similar data are coded against shared history). They are
**Format 1.6**, which is additive. **The default configuration still writes the bytes of 0.7.0:**
a file becomes 1.6 only when a segmented block, a columnar block or a dictionary is actually used.

Previous release: [`RELEASE-NOTES-0.7.0.md`](RELEASE-NOTES-0.7.0.md). Coming from 0.7.0:
[`MIGRATION-0.7.0-TO-0.8.0.md`](MIGRATION-0.7.0-TO-0.8.0.md).

## Compatibility

| | 0.8.0 vs 0.7.0 |
|---|---|
| Written bytes (default configuration) | identical (golden 0.6.1, 31 workloads × 3 profiles) |
| Written bytes (new options) | Format 1.6, pinned by `examples/golden/0.8.0/` (6 workloads × 3 profiles) |
| Formats read / written | 1.0 – 1.6 / 1.3 – 1.6 (1.6 only when used) |
| Older readers | reject a 1.6 file as written by a newer ACE; every other file still opens |
| Rust API (`ace`) | source compatible; new options and types |
| CLI | new `ace train`, `--schema`, `--segments`, `--dictionary`, `--external-dictionary` |
| C ABI | 1.1: `ace_abi_version()` = 1, new `ace_abi_minor()` = 1; the 26 functions of 1.0 unchanged |
| JVM | JDK 22 or newer (tested on 22 and 25), Scala 3.3 LTS; new module `ace-schema` |
| Benchmark documents | schema 2.2 (unchanged); family `storage` (new) |

## What is new

### Format 1.6 ([`FORMAT-1.6.md`](FORMAT-1.6.md))

* Codec 5 `Segmented` (`SEG1`) and codec 6 `Columnar` (`COL1`). A segment and a column are
  **inner blocks**: the serialization of an ordinary block (header, CRC, metadata, payload), so the
  existing pipeline, limits and checks apply unchanged. Nesting is one level deep.
* The `ADIC` dictionary section after the file header (flag `0x0002`), with a CRC per
  dictionary, and a dictionary descriptor in the block header with scope `File` (embedded) or
  `External` (only named).
* Reader gate: a 1.5 file that uses codec 5 or 6 is rejected. New `DecodeLimits` bound segments,
  columns and dictionaries before anything is allocated.

### Adaptive segmentation

A deterministic segmenter classifies windows of a block with the existing analysis and merges them
into segments (`SegmentationConfig`: window, minimum and maximum size, maximum segments). Each
segment is planned on its own. The container is used only if it is smaller than the plain block by
a margin, so it is never larger. Range reads decode only the segments they touch, in every reader
(`decompress`, `decompress_into`, streaming, indexed). Boundaries are the same on every host, thread
count and SIMD backend.

Example, 3 MiB block of JSON, counters and random bytes: 1.85× smaller than the unsegmented block.

### Columnar blocks

A `Schema` (list of `Column { width, kind }`, kinds Bytes, Unsigned, Signed, Float) tells the
engine that the data is fixed-size records. The block size is rounded to whole records, the block
is transposed, and every column becomes an inner block with a codec chosen by its kind. A trailing
partial record is kept. The row-wise block is the fallback, so the result is never worse than it by
the margin.

On the record corpus (`ace-corpus`, 4 MiB each; Balanced): trades 2.28×, telemetry 5.73×, events
1.52× smaller than the row-wise block; every case on every profile clears the 1.10× target
(Fast: 5.34×, 5.24×, 1.29×). Columnar encoding costs about twice the row-wise encode time.

### Preset dictionaries

* `ace-dictionary`: a deterministic trainer (same samples, same bytes; at most 64 KiB, the LZ
  window). `Dictionary::train`, `ace train`, `Ace.trainDictionary`, `Dictionary.train`.
* LZ takes the dictionary as history that precedes the input; the LZ stream format is unchanged and
  the decoder rejects references outside dictionary plus output.
* The engine tries the dictionary as a candidate per block (not a planner route: the planner
  invariants are unchanged). **Embedded** dictionaries make the file self-contained; **external**
  ones are named by id and must be given to every reader, otherwise decoding fails with
  `MissingDictionary`.

Example, 768 KiB of JSON in 4 KiB blocks: 2.25× smaller with an embedded and 2.27× with an
external dictionary, against the same blocks without one.

### Encoder buffers per worker

`WorkerScratch` reuse: the LZ hash tables of large inputs live per worker instead of being
allocated per block, and the planner borrows its input when no transform applies. Allocated bytes
per input byte (mixed 4 MiB): Fast 5.94 → 3.07, Balanced 8.15 → 5.05, Dense 15.6 → 8.16. The
bytes are identical.

### C ABI 1.1, Java and Scala

* C ABI: `AceDictionary` and `AceSchema` handles, dictionary, schema and segmentation functions,
  `ace_engine_new_ex`; `AceConfig` grew from 56 to 112 bytes behind `struct_size`.
  [`FFI.md`](FFI.md), [`NATIVE-INTEROP.md`](NATIVE-INTEROP.md).
* Java: `Dictionary`, `Schema`, `Column`, `ColumnKind`, `Segmentation`, builder options and
  `Engine.create(config, limits, dictionaries)`.
* Scala: the model types, `Ace.open(config, limits, dictionaries)`, `Ace.trainDictionary`,
  `TierCodec.open(policy, dictionaries)` and the new module `jvm/ace-schema`: `RecordCodec`
  derives a schema and the little-endian record layout from a case class, with opaque unsigned
  types; `DictionaryStore` keeps dictionaries in memory or in a directory (`ACEDICT1` header and
  CRC-32); `StorageProfiles` gives the hot, warm and cold tier settings.
  [`DICTIONARIES.md`](DICTIONARIES.md).
* Rust owns everything that defines bytes; Scala owns policy.

### Platforms

* NEON backend for aarch64 (`ace-simd`): the scans AVX2 has, equivalent to the scalar backend on
  random input; `ACE_SIMD=scalar` still works.
* The native library is built and tested for macOS (aarch64) and Windows (x86_64) in CI and
  packaged under `native/<os>-<arch>/`.
* The JVM benchmark measures the caller-owned `decompressInto` path next to the zero-copy path
  (diagnostic `ffi.jvm_into.decompress_16m`; the 0.90× gate is unchanged).

### Tests, fuzzing and audits

* Tests live in `crates/<crate>/tests/` (JVM: `jvm/<module>/src/test/`); the code audit rejects a
  `#[cfg(test)]` module in any `src/` file.
* Golden `examples/golden/0.8.0/GOLDEN.json` (three columnar record tables, a segmented block, an
  embedded and an external dictionary) is reproduced by the Rust engine (also with
  `ACE_SIMD=scalar`), the C ABI, the Java SDK and the Scala SDK.
* Compatibility fixtures `examples/compat/0.8.0/` (columnar, segmented, embedded and external
  dictionary); the files of 0.2.1 – 0.6.1 are untouched and still decode byte for byte.
* Fuzz: 22 targets (new: `seg_decode`, `col_decode`).
* Benchmarks: family `storage` (columnar, segmented and dictionary rows); Regression V3 section
  `storage`. Unsafe and panic audits and the API inventory cover the new C ABI.

## What did not change

Planner V5, every earlier codec, the container structures of Formats 1.0 – 1.5, the Rust API of
0.7.0 and the 26 C functions of ABI 1.0. Context shared between blocks is **not** in 0.8.0 (it
conflicts with independent blocks and random access); it is a candidate for 0.9.0.

## Known limits

* A dictionary is at most 64 KiB (the LZ window) and applies to LZ only.
* Segmentation and columns are options, not defaults: a caller decides by knowing the data. A
  schema describes records of one fixed size; a trailing partial record is stored as it is.
* Streaming compression cannot embed a dictionary (use an external one).
* Columnar encoding is about twice as slow as row-wise encoding; its speed is a diagnostic until
  the reference-machine run.
* On the JVM, `io.ace.Dictionary` clashes with `java.util.Dictionary` under a wildcard import.
* Not reproduced in the environment this milestone was built in (no macOS, Windows or aarch64
  host, no Maven Central, and a Gradle that does not run on the installed JDK): the CI jobs for
  macOS, Windows and ARM64/NEON, the fuzz crate build, and the Gradle/JUnit run of the Java and
  Scala tests. Those tests were run on a stand-in runner; the pull request lists what CI has to
  confirm.

## Qualification

Release run on the reference machine (AMD Ryzen 9 5950X, `performance` governor, rustc 1.98.1,
revision 23fa0a4; the harness recorded a dirty working tree). Details in
[`PERFORMANCE-0.8.0.md`](PERFORMANCE-0.8.0.md), data in `examples/baselines/0.8.0/`.

| Check | Verdict |
|---|---|
| Regression V3 | **pass**: 109 pass, 22 diagnostic, 0 fail / unstable / skipped |
| Interleaved A/B vs 0.7.0 | **pass**, 20 / 20 cases, minimum ratio 0.961 (`numeric_general.delta_variable` encode), maximum 1.035, byte-identical output |
| Repeatability (3 runs) | **fail**: 31 checks pass, 3 diagnostic, 1 fails (see below). Deterministic values identical |
| Columnar vs row-wise (Balanced) | trades 2.28×, telemetry 5.73×, events 1.52× (target 1.10×) |
| Segmentation / dictionaries | 1.85× (3 segments); 2.25× embedded, 2.27× external on 4 KiB JSON blocks |
| C ABI vs Rust facade (16 MiB) | compress 0.993×, decompress 1.016× (gate ≥ 0.97×) |
| JVM zero-copy vs facade (16 MiB) | compress 0.972×, decompress 0.979× (gate ≥ 0.90×); `decompressInto` 0.801× (diagnostic) |
| Fuzz | 22 / 22 targets × 600 s, no crash, 4.20 × 10⁹ executions ([`FUZZ.md`](../examples/baselines/0.8.0/FUZZ.md)) |

**Open item: repeatability.** `macro.numeric_general.delta_variable.decompression_alloc` varied by
41.1 % across the three runs (medians 9.03, 12.42 and 8.71 ms; limit 15 %). It is a decompression
timing of an allocation-profile case; the compression and A/B results for the same workload are
within their limits, and the other 34 checks do not fail. The cause has not been investigated, so
the baseline is marked `"accepted": false` until the maintainer decides to accept it, re-run the
three runs, or investigate. Encoder allocation reuse (task 22) touched the neighbouring code paths,
so the allocation profile is the first place to look.
