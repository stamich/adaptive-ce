# ACE Format 1.5

Format 1.5 (ACE 0.6.1) is a minimal additive extension of Format 1.4. Unlike 1.3 and 1.4 it adds
no codec id: it adds **modes inside the self-describing NUM1 and TS1 payloads** and therefore
makes the minor version a property of the *payload*, not only of the codec id.

Unchanged:
- file magic and the fixed 32-byte file header (only the minor-version byte differs);
- fixed block-header size, codec / entropy / transform ids, dictionary descriptors, entropy
  metadata framing;
- AIDX index layout, ACET trailer layout, CRC32C behaviour;
- every payload of mode 1 – 3 (NUM1 FOR / Delta / DoD; TS1 GorillaF64 / GorillaF32 / RunDelta).

New:
- minor version = 5;
- NUM1 mode 4, `PatchedFor` (layout added to this document with the codec);
- TS1 mode 4, `Alp` (defined in [`TS1-CODEC.md`](TS1-CODEC.md) once the codec lands);
- a stream whose header names a mode of Format 1.5 is valid only in a file that declares 1.5.

RunDelta for small non-zero deltas (admission by the run prefilter) is a planner change: it
reuses TS1 mode 3 and needs no format change.

## Payload-aware minimal version

The writer declares the **lowest** minor version the file needs, and since 1.5 it asks each block:

| Block | Needs |
|---|---|
| RAW, RLE, LZ, NUM1 mode 1 – 3 | 1.3 |
| TS1 mode 1 – 3 | 1.4 |
| NUM1 or TS1 of a mode introduced by 1.5 | 1.5 |
| NUM1 with an entropy stage (never written by the planner) | 1.5, conservatively: the primary stream is not visible without decoding |

The file declares the maximum over its blocks. Data that needs no Format 1.5 mode is therefore
written exactly as by ACE 0.6.0 (`examples/golden/0.5.0/`, byte for byte), and an ACE 0.5.x or
0.6.0 reader keeps reading it.

`ace_codecs::payload_minor_version(codec, primary)` reads only the magic and the mode byte
(offset 5 in both headers); `ace_engine::block_format_minor` applies the entropy rule. The
in-memory encoder knows every block before it writes the header; the streaming encoder rewrites
the fixed 32-byte file header (including its CRC32C) in place at the end, exactly as it did for
1.4, so stream and in-memory output stay byte-identical.

## Reader gate

A reader decodes the entropy stage first and then compares `payload_minor_version` with the
minor version of the file. A payload that needs more is rejected with `UnsupportedVersion`
(`major.minor` of the *file*) before the codec sees it. This is the same error a 1.3 file with
a TS1 block gets, so an old container version can never silently acquire new decoder semantics.
The check runs in every reader: `decompress`, `decompress_into`, `decompress_from` (streaming)
and the indexed random-access decoder.

An ACE 0.5.x / 0.6.0 reader that meets a 1.5 file rejects it at the file header with
`UnsupportedVersion` (0.6.0 names the release: "written by ACE 0.6.1 or newer"). ACE 0.6.1
reads 1.0 – 1.5; a file that declares 1.6 or more is rejected with "written by a newer ACE
release".

## Reader compatibility

| File format | ACE 0.5.x reader | ACE 0.6.0 reader | ACE 0.6.1 reader |
|---|---|---|---|
| 1.0 – 1.4 | yes | yes | yes |
| 1.5 | rejected (`UnsupportedVersion`) | rejected, names ACE 0.6.1 | yes |

Tests: `crates/ace-codecs/tests/dispatch.rs` (`payload_minor_version_follows_codec_and_mode`),
`crates/ace-engine/tests/format_1_5.rs` (reader gate), `block_pipeline.rs` (writer rule),
`newer_format.rs` (1.6 rejected by every reader), `crates/ace-format/tests/format_1_2.rs`
(`minimal_minor_version_is_the_highest_block_requirement`), `crates/ace-stream/tests/streaming.rs`
(header rewrite).
