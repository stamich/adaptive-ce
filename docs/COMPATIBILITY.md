# ACE compatibility

What each ACE release promises about files, APIs, toolchains and benchmark documents.

## Format

ACE 0.7.0 reads Format 1.0 – 1.5 and writes the **minimal** version a file needs: 1.3 unless the
file contains a TS1 block (1.4, [`FORMAT-1.4.md`](FORMAT-1.4.md)) or a payload of a Format 1.5
mode (1.5, [`FORMAT-1.5.md`](FORMAT-1.5.md); the version is a property of the payload).

| File format | ACE 0.4.x reader | ACE 0.5.x – 0.6.0 reader | ACE 0.6.1 – 0.7.0 reader |
|---|---|---|---|
| 1.0 | supported | supported | supported |
| 1.1 | supported | supported | supported |
| 1.2 | supported | supported | supported |
| 1.3 | supported | supported | supported |
| 1.4 | rejected at the file header (`UnsupportedVersion`) | supported | supported |
| 1.5 | rejected at the file header | rejected at the file header (0.6.0 names ACE 0.6.1) | supported |

| Version | Adds |
|---|---|
| 1.1 | scalar rANS, dictionary descriptors, block index (AIDX) and trailer (ACET) |
| 1.2 | rANS4x entropy coder |
| 1.3 | primary codec 3 `Numeric` (NUM1 payload; u16 lanes since 0.4.5) |
| 1.4 | primary codec 4 `TimeSeries` (TS1 payload: Gorilla f64 / f32, RunDelta) |
| 1.5 | NUM1 mode 4 (Patched FOR), TS1 mode 4 (ALP): no new codec id, the mode inside the payload needs the version |

Every extension adds only a codec / coder id plus its self-describing payload; outer file and
block header sizes, dictionary descriptors, AIDX and ACET remain unchanged. A block whose
codec is newer than the declared minor version is rejected, preventing accidental use of new
decoder semantics in an older container version.

To produce files for ACE 0.4.x readers with ACE 0.5, compress with `--disable-float`
(`AceConfig::enable_float_specialization = false`): the output is byte-identical to ACE 0.4.6.
Without the flag, data that no TS1 block helps (all of Corpus V3) is also written as 1.3.

### Rules

1. **Readers never drop a format version.** Every reader decodes every earlier minor version
   byte for byte. `examples/compat/` holds files written by the released encoders of 0.2.1,
   0.3.0, 0.4.4, 0.4.6 and 0.5.0 (Formats 1.1 – 1.4); they are never regenerated, and
   `crates/ace-engine/tests/compat_fixtures.rs` decodes each of them on every build. Each
   release adds its own directory.
2. **Every new wire feature is a new format minor version.** A new primary codec, entropy
   coder, transform, flag bit — and in particular **every new TS1 mode** (ids 4 – 15 are
   reserved) — bumps the minor version, and readers reject the feature in files that declare an
   older minor version. An older reader must fail at the file header (`UnsupportedVersion`),
   never inside a block, and since 0.6.0 the error says which release can read the file
   (0.6.0 reading a 1.5 file: `unsupported ACE format version 1.5 (written by ACE 0.6.1 or
   newer; this reader supports up to 1.4)`; 0.6.1 reading a 1.6 file: `... (written by a newer
   ACE release; this reader supports up to 1.5)`; `crates/ace-engine/tests/newer_format.rs`).
   Since 1.5 the rule has a second half: a payload mode newer than the file's declared minor
   version is rejected by the reader even inside an otherwise readable file
   (`crates/ace-engine/tests/format_1_5.rs`). (TS1 modes are self-describing, so a new mode
   inside 1.4 would decode as "invalid TS1 stream" on a 0.5 reader instead of failing
   cleanly — which is why every new mode is a new format minor version.) One historical
   exception predates this rule:
   0.4.5 added u16 NUM1 lanes inside Format 1.3, so 0.4.0 – 0.4.4 readers reject such blocks
   as malformed numeric headers.
3. **The writer declares the minimal version.** A file is written as the lowest minor version
   whose features it uses, so data that needs no new feature stays readable by older readers.
4. **Patch releases never change bytes.** 0.5.1 writes exactly the bytes of 0.5.0
   (`examples/golden/0.5.0/GOLDEN.json`, checked with the auto and the scalar SIMD backend);
   only decoder strictness may increase, and only for streams no released encoder produces
   (all compatibility fixtures still decode).
5. **Two guards, two directions.** The golden SHA-256 files guard the *writer* (same input →
   same bytes); the compatibility fixtures guard the *reader* (old bytes → same input).

### Decoder strictness added in 0.5.1

0.5.1 rejects streams that 0.5.0 accepted but that no ACE encoder can produce (every
compatibility fixture still decodes):

* a scalar rANS lane (also inside rANS4x) whose final state is not the encoder's initial state,
  or a non-empty payload for an empty output;
* a Huffman stream with more than the zero padding of its last byte left after the declared
  number of symbols, or a non-empty payload for an empty output;
* an entropy-coded block whose primary (codec) stream length exceeds twice the block size plus
  1 KiB (no codec expands its input that much);
* a file declaring more blocks than `DecodeLimits::max_block_count()` (= `max_index_entries`),
  also in the sequential reader;
* an index section larger than `max_index_entries` admits (refused before it is read).

Details per field: [`DECODER-AUDIT.md`](DECODER-AUDIT.md).

## API

* ACE is 0.x: a minor release (0.5 → 0.6) may break the Rust API; a patch release (0.5.0 →
  0.5.1) does not. 0.5.1 removed nothing and narrowed no visibility; it only added items
  (`DecodeLimits::max_block_count`, `INDEX_HEADER_SIZE`, the `ace version` command) and
  marked superseded entry points `#[deprecated]` (`evaluate_candidates_v3`,
  `evaluate_candidates_v4`).
* 0.6.0 replaced the three public crates (`ace-core`, `ace-engine`, `ace-stream`) by the `ace`
  facade, which is the only public API; every other crate is internal (`publish = false`) and
  may change in any release ([`API-INVENTORY.md`](API-INVENTORY.md)). The deprecated
  `evaluate_candidates_v3` / `_v4` were removed. Migration: [`MIGRATION-0.5-TO-0.6.md`](MIGRATION-0.5-TO-0.6.md).
* Within a 0.x series the facade changes only additively: its enums and structs that may grow
  are `#[non_exhaustive]`, and `ErrorKind` is the stable contract of failures (messages are for
  people).
* The `ace` CLI keeps its commands and options within a minor series; new commands are
  additive. Exit status: 0 success (also when stdout is closed early), 1 error, 2 usage error.

## C ABI and JVM

* The C ABI has its own version, `ace_abi_version()` (**1** in 0.7.0), independent of the file
  format and of the crate version. Within ABI 1 functions and status values are never removed or
  renumbered; additive changes (a function, a status value, a trailing struct field behind
  `struct_size`) do not change the version. A removed function, a changed signature or status
  meaning, or a changed struct layout bumps it, and the bindings refuse to load a library with
  another ABI version ([`NATIVE-INTEROP.md`](NATIVE-INTEROP.md), rule 6;
  [`FFI.md`](FFI.md), compatibility rules).
* `include/ace.h` is generated and drift-checked; the Java bindings are generated from it
  (`tools/ace-genbindings0.7.0.py --check`).
* The bytes written through the C ABI, Java and Scala are those of the Rust API for the same
  configuration (golden 0.6.1).
* JVM: JDK 22 or newer (`java.lang.foreign` is final from 22), tested on 22 and 25; Scala 3.3 LTS
  (the artifacts do not use features of later Scala 3 releases). The Java and Scala packages
  (`io.ace.*`) follow the 0.x rule of the Rust API: additive within a minor series.

## Toolchain

* MSRV (`rust-version`): **1.98.1** since 0.6.0 (0.5.x: 1.97.0). It equals the pinned build
  toolchain, so the `pr` CI job checks it; there is no separate MSRV job. Until 1.0 the MSRV
  may rise in any minor release (`0.x.0`), always announced in the CHANGELOG, RELEASE-NOTES and
  README; patch releases never raise it.
* Build toolchain (`rust-toolchain.toml`): **1.98.1** — every build, CI run and benchmark
  fingerprint uses the same compiler and clippy. Rust 1.98.0 must not be used (vtable
  miscompilation fixed in 1.98.1).

## Benchmark documents

Schema changes are additive within 2.x: the validator and the tools read 2.0, 2.1 and 2.2.
2.2 (0.5.1) adds the build's git revision and dirty flag, the configuration fingerprint,
threads, block size and the SHA-256 of every generated input. Results are compared only when
the configuration fingerprints match ([`BENCHMARKS.md`](BENCHMARKS.md)).
