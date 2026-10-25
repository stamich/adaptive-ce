# ACE Format 1.6

Format 1.6 (ACE 0.8.0) adds three storage-engine features to Format 1.5 without changing any
existing structure:

| Feature | Where | Id |
|---|---|---|
| **Segmented block** (adaptive segmentation) | primary codec 5 `Segmented`, payload `SEG1` | task 7, 10 – 12 |
| **Columnar block** | primary codec 6 `Columnar`, payload `COL1` | task 8, 14 – 16 |
| **Dictionaries** | file flag `0x0002` + dictionary section `ADIC` after the file header; block dictionary descriptor with scope `File` or `External` | task 6, 18 – 20 |

Unchanged: file magic and the 32-byte file header (only the minor-version byte and the
`HAS_DICTIONARIES` flag differ), the 32-byte block header and its descriptors, codec ids 0 – 4,
entropy and transform ids, NUM1 / TS1 payloads of every mode, the AIDX index and the ACET trailer,
CRC-32C. The LZ stream is unchanged: a dictionary is history that precedes the input, never a new
token.

**The default configuration still writes the bytes of 0.7.0.** A file is declared 1.6 only when it
contains a block of codec 5 or 6 or has a dictionary section; otherwise the minimal-version rule
of 1.3 – 1.5 applies ([`FORMAT-1.5.md`](FORMAT-1.5.md)).

## Inner blocks

A segment (`SEG1`) and a column (`COL1`) are **inner blocks**: exactly the bytes that
`AceWriter::write_block` produces for an ordinary block — the 32-byte block header with its
descriptors and CRC-32C, the entropy metadata, the payload. An inner block therefore has its own
codec, entropy stage and transforms and is decoded by the same code as a top-level block, with the
same limits. Rules for an inner block:

* its codec is not 5 or 6 (nesting is one level deep); a nested container is rejected as malformed;
* its `block_id` is its position in the outer block (segment index / column index, from 0);
* its `original_size` equals the size announced in the outer table;
* it may name a dictionary (scope `File` or `External`) like a top-level block;
* its `payload_crc32c` covers its own reconstructed bytes. The outer block's `payload_crc32c`
  covers the whole reconstructed block, so a forged table cannot produce a block that passes.

## SEG1 – segmented block

Outer block header: codec `5`, entropy `0` (none), no transforms, no dictionary, `metadata_size`
`0`, `encoded_size` the payload length, `original_size` the block size. Any other combination is
`Malformed`; a codec 5 block in a file below 1.6 is `UnsupportedVersion`.

Payload (little-endian):

| Offset | Size | Field |
|---|---|---|
| 0 | 4 | magic `SEG1` |
| 4 | 4 | segment count `n`, `2 ..= max_segments` |
| 8 | 4 | reserved, zero |
| 12 | 8 n | table: per segment `original_size: u32` (non-zero), `serialized_len: u32` |
| 12 + 8 n | … | the `n` inner blocks, back to back, in order |

A decoder checks, before it allocates or decodes anything:

* `n` against `max_segments` (before the table is read);
* `Σ original_size == outer original_size` and every `original_size <= max_block_size`;
* `Σ serialized_len == payload length − 12 − 8 n`, each `serialized_len >= 32`;
* then for each inner block: header CRC, `block_id == index`, `original_size` equal to the table,
  the whole serialized block consumed exactly, and the usual block checks.

The table gives the logical start of every segment (prefix sums of `original_size`) and the byte
offset of its inner block without parsing the blocks, so a range read decodes only the segments
that overlap the range ([`RANDOM-ACCESS.md`](RANDOM-ACCESS.md)).

*Writer rule.* Segment boundaries come from the deterministic segmenter (task 9); a segmented
block is written only when it is smaller than the block the planner chose for the same bytes by at
least `max(min_gain_bytes, 1 %)`. A block of one segment is never written (`n >= 2`).

### Index flag

Every AIDX entry has a 32-bit `flags` field. Format 1.6 defines bit 0 (`INDEX_FLAG_SEGMENTED`,
`0x1`): the block is a `SEG1` block, so a range reader can fetch the header, the segment table and
only the needed segments ([`RANDOM-ACCESS.md`](RANDOM-ACCESS.md)). The writer sets it for every
codec-5 block. All other bits are reserved and zero; a reader rejects an entry that sets one
(`Malformed`). A flag on a block whose header is not a bare `SEG1` container is `Malformed` when the
block is read; a missing flag only costs the optimisation.

## COL1 – columnar block

Outer block header as for `SEG1` with codec `6`.

A *schema* is a list of columns, each with a width in bytes; a record is the concatenation of the
columns, `record_size = Σ width`. A block of `original_size` bytes holds
`record_count = original_size / record_size` whole records followed by `tail_len =
original_size mod record_size` bytes of an incomplete last record. Column *i* is the byte range
`[offset_i, offset_i + width_i)` of every record, `offset_i = Σ_{j<i} width_j`; its stream is the
concatenation, record by record, of those bytes (`record_count × width_i` bytes). The writer
encodes each stream as an inner block.

Payload:

| Offset | Size | Field |
|---|---|---|
| 0 | 4 | magic `COL1` |
| 4 | 1 | version, zero |
| 5 | 1 | flags, zero |
| 6 | 2 | column count `c`, `1 ..= max_columns` |
| 8 | 4 | `record_size` (non-zero) |
| 12 | 4 | `record_count` (non-zero) |
| 16 | 4 | `tail_len`, `< record_size` |
| 20 | 4 | reserved, zero |
| 24 | 8 c | table: per column `width: u16` (non-zero), `kind: u8`, `reserved: u8` (zero), `serialized_len: u32` |
| 24 + 8 c | … | the `c` inner blocks (column streams), in column order |
| … | `tail_len` | the tail bytes, verbatim |

`kind` is `0` bytes, `1` unsigned integer, `2` signed integer, `3` IEEE-754 float; integer widths
are 1, 2, 4 or 8, float widths 4 or 8. It does not change decoding (every inner block is
self-describing); it lets tools and column projection know what a column holds. A decoder checks
`Σ width == record_size`, `record_count × record_size + tail_len == outer original_size` (checked
arithmetic), the inner `original_size` of column *i* equal to `record_count × width_i`,
`Σ serialized_len + tail_len == payload length − 24 − 8 c`, and `c`, `record_size` and the block
size against the limits before it allocates.

*Writer rule.* A columnar block is used only when the caller supplies a schema (the planner never
guesses one) and only when it is smaller than the row-wise block by the same margin as `SEG1`.
Blocks are cut at multiples of the record size, so only the last block of a file can have a tail.

*Codec selection per column (ACE 0.8.0).* Each column stream is planned like a block, with the
configuration narrowed by the column kind: `Bytes` searches the generic codecs only (no NUM1, no
TS1); `Unsigned` and `Signed` keep NUM1 and the integer TS1 modes but never try the bits as floats;
`Float` keeps every candidate. A specialization that the block configuration disabled stays
disabled. The rule is a pure function of the configuration and the kind, so it does not affect
determinism; the format itself does not depend on it (every inner block is self-describing).

## ADIC – dictionary section

File header flag `0x0002` (`HAS_DICTIONARIES`) says that the section follows the 32-byte file
header, before the first block; the file is Format 1.6. A reader that sees the flag below 1.6
rejects the file (`UnsupportedFeature`).

| Offset | Size | Field |
|---|---|---|
| 0 | 4 | magic `ADIC` |
| 4 | 4 | dictionary count `n`, `1 ..= max_dictionaries` |
| 8 | … | per dictionary: `id: u64`, `length: u32`, `crc32c: u32` of the bytes, the `length` bytes |
| … | 4 | CRC-32C of everything from the magic to here |

Rules: ids are strictly increasing (no duplicates); `1 <= length <= min(max_dictionary_size,
65 535)`; a size is compared with the limits before the bytes are read; the per-dictionary CRC and
the section CRC must match. Block offsets in the AIDX index are absolute, so the index is
unaffected by the section; the indexed reader reads it right after the file header.

A block (top-level or inner) whose header carries a dictionary descriptor refers to a dictionary
by id and scope:

| Scope | Resolved from |
|---|---|
| `File` (2) | the `ADIC` section of the same file; missing → `MissingDictionary(id)` |
| `External` (3) | the registry of the decoder (`Engine::with_dictionaries`); missing → `MissingDictionary(id)` |
| `Block` (0), `Segment` (1) | reserved: `Malformed("reserved dictionary scope")` |

Only the LZ codec uses a dictionary in 1.6; any other codec with a descriptor is `Malformed`. A
file with only `External` references has no `ADIC` section but is still 1.6, because an older
reader would answer `MissingDictionary` for a reason it cannot explain.

*Writer rule.* A dictionary is an engine-level candidate (`DICTIONARIES.md`): a block, segment or
column is written with a dictionary descriptor only when the LZ form with the dictionary is smaller
than the planned block by at least `min_gain_bytes`, descriptor included, and never for a NUM1 or
TS1 block. The `ADIC` section is written only when a block uses a `File` dictionary and the blocks
together save more than the section costs; otherwise the file is the file without a dictionary.

*Meaning.* The dictionary bytes are the history that precedes the block: LZ matches may point
back into the last 65 535 bytes of `dictionary ‖ output`. The decoder starts with the dictionary
as the first bytes of its output buffer and removes them at the end. Distances must stay inside
`dictionary ‖ output`, so a reference before the dictionary is `InvalidLzReference`.

### Trainer (non-normative)

The format does not say how a dictionary is made; any byte string of 1 – 65 535 bytes is valid.
`ace_dictionary::train_dictionary` (ACE 0.8.0) builds one from samples: it counts, for every 8-byte
substring, in how many samples it occurs, copies the 64-byte segments with the highest total count
(each substring is counted once, so shared content is not selected twice) and puts the best segment
last, where LZ distances are smallest. The result depends only on the samples, their order and the
`TrainerConfig`, with no randomness, so the same samples give the same bytes on every host.
`dictionary_id_of` derives a stable id (FNV-1a 64 of the content) for it.

## Payload-aware minimal version

| Block / file | Needs |
|---|---|
| RAW, RLE, LZ, NUM1 mode 1 – 3 | 1.3 |
| TS1 mode 1 – 3 | 1.4 |
| NUM1 / TS1 of a Format 1.5 mode | 1.5 |
| codec 5 (`SEG1`) or 6 (`COL1`) | 1.6 |
| dictionary section, or a block with a dictionary descriptor | 1.6 |

The file declares the maximum over its blocks (and 1.6 when it has a dictionary section or a
descriptor). The streaming encoder rewrites the 32-byte file header at the end exactly as for 1.4
and 1.5.

## Reader gate

`decode_block_header` rejects codec 5 / 6 below 1.6 (`UnsupportedVersion`) and any outer
transform, entropy stage or dictionary on them (`Malformed`). The engine rejects a nested
container, a dictionary descriptor in a file below 1.6 (`MissingDictionary`, as before: such a file
cannot have been written by any release) and the checks listed above. The check runs in every
reader: `decompress`, `decompress_into`, `decompress_from` (streaming) and the indexed decoder.

An ACE 0.7.x reader that meets a 1.6 file rejects it at the file header with `UnsupportedVersion`
("written by a newer ACE release; this reader supports up to 1.5"). ACE 0.8.0 reads 1.0 – 1.6; a
file that declares 1.7 or more is rejected with "written by a newer ACE release".

| File format | ACE 0.6.1 – 0.7.0 reader | ACE 0.8.0 reader |
|---|---|---|
| 1.0 – 1.5 | yes | yes |
| 1.6 | rejected (`UnsupportedVersion`) | yes |

## Limits

`DecodeLimits` gained `max_segments` (default 1 024), `max_columns` (256) and `max_dictionaries`
(16); `max_dictionary_size` (16 MiB) now applies to the section and to external dictionaries (the
format cap of 65 535 bytes is lower). Every limit is checked before the allocation it protects
(`docs/DECODER-AUDIT.md`).

## Tests

`crates/ace-format/tests/format_1_6.rs` (codec gate, `SEG1` / `COL1` / `ADIC` parsers),
`crates/ace-codecs/tests/dispatch.rs` (`payload_minor_version`), `crates/ace-core/tests/limits.rs`,
`crates/ace-engine/tests/newer_format.rs` (1.7 rejected by every reader) and the round-trip,
malformed, determinism and random-access tests of tasks 10 – 20.
