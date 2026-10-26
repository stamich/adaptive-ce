# Migrating from ACE 0.7.0 to 0.8.0

* **Rust API**: source compatible. New items only: `Config` / `ConfigBuilder` gain `schema`,
  `segmentation`, `dictionary` and `external_dictionary`; `Dictionary` (with `Dictionary::train`),
  `Schema`, `Column`, `ColumnKind`, `Segmentation`; `Engine::with_dictionaries` and
  `RangeReader::open_with_dictionaries`; `Stats` (non-exhaustive) gains `segmented_blocks`, `columnar_blocks` and
  `dictionary_blocks`; `DecodeLimits` gains `max_dictionary_size` (64 KiB by default; the core
  limits for segments, columns and dictionaries have defaults that accept everything 0.8.0
  writes).
* **Bytes**: unchanged for the default configuration. 0.8.0 writes exactly the files of 0.7.0
  until you set a schema, segmentation or a dictionary (`examples/golden/0.6.1/` is still
  reproduced by every binding). Hashes of compressed output stay valid.
* **Format**: 1.6 is additive. A file is declared 1.6 only when it contains a segmented block
  (codec 5), a columnar block (codec 6) or a dictionary section; every other file is still written
  as 1.3 – 1.5. 0.8.0 reads 1.0 – 1.6. **Older readers reject a 1.6 file** with the usual "needs a
  newer ACE" error, so upgrade readers before you enable the new options on data they must read.
* **Dictionaries need care.** An *embedded* dictionary (default) makes the file self-contained.
  An *external* dictionary is only named in the file: every reader must be given the same
  dictionary (`Engine::with_dictionaries`, `ace decompress --dictionary`, `ace_engine_new_ex`,
  `Engine.create(config, limits, dictionaries)`, `Ace.open(config, limits, dictionaries)`), or
  decoding fails with `MissingDictionary`. Keep the dictionary file as long as the data.
* **CLI**: new `ace train`, and `--schema`, `--segments`, `--dictionary`, `--external-dictionary`
  on `ace compress`; `--dictionary` on `ace decompress`, `verify`, `read-range` and `decode-block`.
  Nothing existing changed.
* **C ABI 1.1**: `ace_abi_version()` is still 1; the new `ace_abi_minor()` returns 1. The 26
  functions of 1.0 are unchanged. New: `AceDictionary` and `AceSchema` handles, the dictionary and
  schema functions, `ace_engine_new_ex`. `AceConfig` grew from 56 to 112 bytes behind
  `struct_size`, so a 1.0 caller that sets `struct_size = 56` keeps working. The Stats JSON
  appends `segmented_blocks`, `columnar_blocks` and `dictionary_blocks` after the old keys.
  Regenerate your header copy from `include/ace.h`.
* **Java SDK**: new `Dictionary`, `Schema`, `Column`, `ColumnKind`, `Segmentation`;
  `Config.Builder` gains `schema`, `dictionary`, `externalDictionary` and `segmentation`;
  `Engine.create(config, limits, Collection<Dictionary>)`; `Stats` gains three counters (the
  11-argument constructor stays). `io.ace.Dictionary` clashes with `java.util.Dictionary` under a
  `java.util.*` wildcard import: import it explicitly.
* **Scala SDK**: `AceConfig` gains `schema`, `dictionary` and `segmentation`; `CompressionStats`
  gains three counters (positional construction needs them); `Ace.open(config, limits,
  dictionaries)`, `Ace.trainDictionary`, `TierCodec.open(policy, dictionaries)`. New module
  `jvm/ace-schema` (`RecordCodec`, `DictionaryStore`, `StorageProfiles`); add it to your build to
  derive schemas from case classes.
* **New platforms**: the native library is built for macOS (aarch64) and Windows (x86_64) in CI
  and packaged under `native/<os>-<arch>/`; aarch64 uses the NEON backend (`ACE_SIMD=scalar`
  still forces the scalar kernels).
* **Contributors.**
  * CI workflow `ace-ci0.8.0` (was `ace-ci0.7.0`): update required status-check names. New jobs:
    `native-macos`, `native-windows`; the aarch64 job now runs the NEON backend.
  * Scripts and tools carry the suffix 0.8.0; the A/B base and the stored baseline are 0.7.0.
  * Tests never live in `src/`: `crates/<crate>/tests/` (enforced by the code audit).
  * Fuzz: 22 targets (new: `seg_decode`, `col_decode`).
  * Regenerate the generated files with `tools/ace-check_header0.8.0.sh --write` and
    `python3 tools/ace-genbindings0.8.0.py --write`.
