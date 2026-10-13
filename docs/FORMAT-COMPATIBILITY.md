# ACE format compatibility

ACE 0.5.x reads Format 1.0 – 1.4 and writes the **minimal** version a file needs: 1.3 unless the
file contains a TS1 block, then 1.4 ([`FORMAT-1.4.md`](FORMAT-1.4.md)).

| File format | ACE 0.4.x reader | ACE 0.5.x reader |
|---|---|---|
| 1.0 | supported | supported |
| 1.1 | supported | supported |
| 1.2 | supported | supported |
| 1.3 | supported | supported |
| 1.4 | rejected at the file header (`UnsupportedVersion`) | supported |

| Version | Adds |
|---|---|
| 1.1 | dictionary descriptors |
| 1.2 | rANS4x entropy coder, block index (AIDX) and trailer (ACET) |
| 1.3 | primary codec 3 `Numeric` (NUM1 payload; u16 lanes since 0.4.5) |
| 1.4 | primary codec 4 `TimeSeries` (TS1 payload: Gorilla f64 / f32, RunDelta) |

Every extension adds only a codec / coder id plus its self-describing payload; outer file and
block header sizes, dictionary descriptors, AIDX and ACET remain unchanged. A block whose
codec is newer than the declared minor version is rejected, preventing accidental use of new
decoder semantics in an older container version.

To produce files for ACE 0.4.x readers with ACE 0.5, compress with `--disable-float`
(`AceConfig::enable_float_specialization = false`): the output is byte-identical to ACE 0.4.6.
Without the flag, data that no TS1 block helps (all of Corpus V3) is also written as 1.3.

## Compatibility rules

1. **Readers never drop a format version.** Every reader decodes every earlier minor version
   byte for byte. `examples/compat/` holds files written by the released encoders of 0.2.1,
   0.3.0, 0.4.4, 0.4.6 and 0.5.0 (Formats 1.1 – 1.4); they are never regenerated, and
   `crates/ace-engine/tests/compat_fixtures.rs` decodes each of them on every build. Each
   release adds its own directory.
2. **Every new wire feature is a new format minor version.** A new primary codec, entropy
   coder, transform, flag bit — and in particular **every new TS1 mode** (ids 4 – 15 are
   reserved) — bumps the minor version, and readers reject the feature in files that declare an
   older minor version. An older reader must fail at the file header (`UnsupportedVersion`),
   never inside a block. (TS1 modes are self-describing, so a new mode inside 1.4 would decode
   as "invalid TS1 stream" on a 0.5 reader instead of failing cleanly — therefore Chimp or any
   other mode planned for 0.6 means Format 1.5.)
3. **The writer declares the minimal version.** A file is written as the lowest minor version
   whose features it uses, so data that needs no new feature stays readable by older readers.
4. **Patch releases never change bytes.** 0.5.1 writes exactly the bytes of 0.5.0
   (`examples/golden/0.5.0/GOLDEN.json`, checked with the auto and the scalar SIMD backend);
   only decoder strictness may increase, and only for streams no released encoder produces
   (all compatibility fixtures still decode).
5. **Two guards, two directions.** The golden SHA-256 files guard the *writer* (same input →
   same bytes); the compatibility fixtures guard the *reader* (old bytes → same input).

## Decoder strictness added in 0.5.1

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
