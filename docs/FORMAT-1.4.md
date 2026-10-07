# ACE Format 1.4

Format 1.4 (ACE 0.5.0) is a minimal additive extension of Format 1.3.

Unchanged:
- file magic and the fixed 32-byte file header (only the minor-version byte differs);
- fixed block-header size, dictionary descriptor shape, entropy metadata framing;
- AIDX index layout, ACET trailer layout, CRC32C behaviour.

New:
- minor version = 4;
- primary codec ID 4 = `CodecId::TimeSeries`, whose payload is a self-describing `TS1` stream
  ([`TS1-CODEC.md`](TS1-CODEC.md)): Gorilla XOR for f64 / f32 and RunDelta for u16 / u32 /
  u64 lanes;
- a TS1 block is *plain*: no transform, entropy coder `None`, no dictionary. Any other
  combination is rejected as malformed.

## Minimal-version writer

ACE 0.5.0 declares the **lowest** minor version the file needs:

| File content | Declared version | Readable by |
|---|---|---|
| no TS1 block | 1.3 | ACE 0.4.x and 0.5.x |
| at least one TS1 block | 1.4 | ACE 0.5.x |

Consequences:

* everything the 0.4 pipeline compresses (Corpus V3, every file compressed with
  `--disable-float` / `enable_float_specialization = false`) stays byte-identical to ACE 0.4.6
  and readable by 0.4.x readers;
* an ACE 0.4.x reader rejects a 1.4 file at the file header with `UnsupportedVersion` — a clear
  error instead of `UnsupportedCodec` in the middle of the file.

In-memory compression knows every block before it writes the header. The streaming encoder
(`compress_reader_known_size`) writes a 1.3 header first and, if any block selected TS1,
rewrites the fixed 32-byte header (including its CRC32C) in place at the end; this is why its
sink is `Write + Seek` since 0.5.0. Stream and in-memory output stay byte-identical.

## Reader compatibility

| File format | ACE 0.4.x reader | ACE 0.5.0 reader |
|---|---|---|
| 1.0 | yes | yes |
| 1.1 | yes | yes |
| 1.2 | yes | yes |
| 1.3 | yes | yes |
| 1.4 | rejected (`UnsupportedVersion`) | yes |

A TimeSeries block under a file version below 1.4 is rejected as `UnsupportedVersion`, so an
old container version can never silently acquire new decoder semantics.

Tests: `crates/ace-format/tests/format_1_4.rs`, `format_1_2.rs` (minimal-version rule),
`crates/ace-engine/tests/float_lane.rs` (declared version per file),
`crates/ace-stream/tests/streaming.rs` (header rewrite), `malformed_matrix.rs` (TS1 fixture).
