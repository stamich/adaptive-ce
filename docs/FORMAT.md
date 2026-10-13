# ACE container format

Canonical description of the ACE1 container as written and read by the current release
(writer 1.3 | 1.4, readers 1.0 – 1.4). Per-version specifications:
[`FORMAT-1.1.md`](FORMAT-1.1.md), [`FORMAT-1.2.md`](FORMAT-1.2.md),
[`FORMAT-1.3.md`](FORMAT-1.3.md), [`FORMAT-1.4.md`](FORMAT-1.4.md); payload formats:
[`TS1-CODEC.md`](TS1-CODEC.md), [`RANS.md`](RANS.md); compatibility rules:
[`COMPATIBILITY.md`](COMPATIBILITY.md). All integers are little-endian.

## Layout

```text
file header (32 bytes)
block 0 … block N-1          each: fixed header (32) · descriptors · entropy metadata · payload
AIDX block index             when the file header has HAS_INDEX (Format 1.1 and later)
ACET trailer (28 bytes)      when the file header has HAS_INDEX
```

Blocks are independent: each one carries its own pipeline (transforms → primary codec →
entropy coder) and the CRC-32C of its reconstructed bytes, so any block can be decoded alone
through the index.

## File header (32 bytes)

| Offset | Size | Field | Validation |
|---|---|---|---|
| 0 | 4 | magic `ACE1` | exact |
| 4 | 1 | major version (1) | exact |
| 5 | 1 | minor version (0 – 4) | ≤ 4; selects which codec / coder ids are legal |
| 6 | 2 | flags: `0x0001` HAS_INDEX, `0x0002` HAS_DICTIONARIES | unknown bits rejected (1.1+) |
| 8 | 4 | default block size | informational only |
| 12 | 8 | original size | ≤ `DecodeLimits::max_output_size`; equals the sum of the blocks |
| 20 | 8 | block count | ≤ `DecodeLimits::max_block_count()`; equals the index entry count |
| 28 | 4 | CRC-32C of bytes 0 – 27 | exact |

## Block

| Offset | Size | Field | Validation |
|---|---|---|---|
| 0 | 8 | block id | sequential from 0 |
| 8 | 4 | original size | ≤ `max_block_size`; every decoder must produce exactly this many bytes |
| 12 | 4 | encoded (payload) size | with the metadata size ≤ `max_encoded_block_size` |
| 16 | 4 | entropy metadata size | see above |
| 20 | 1 | primary codec: 0 RAW, 1 RLE, 2 LZ, 3 NUM1 (1.3+), 4 TS1 (1.4+) | known and allowed by the minor version |
| 21 | 1 | entropy coder: 0 none, 1 Huffman, 2 rANS, 3 rANS4x (1.2+) | known; rANS4x only in 1.2+ |
| 22 | 1 | transform count | ≤ `max_transforms` |
| 23 | 1 | flags: `0x01` dictionary descriptor present (1.1+) | — |
| 24 | 4 | CRC-32C of the reconstructed block | checked after decoding |
| 28 | 4 | CRC-32C of bytes 0 – 27 | checked before any field is used |

Descriptors follow: one byte per transform (0 none, 1 byte delta), then a 9-byte dictionary
descriptor (8-byte id, 1-byte scope) when the flag is set. Then the entropy metadata and the
payload. For every entropy coder other than `none`, the metadata starts with the `u32`
length of the primary (codec) stream, bounded by twice the block size plus 1 KiB; the coder's
model follows (Huffman: 256 code lengths; rANS: 256 × `u16` frequencies; rANS4x: four rANS
models and four `u32` lane lengths). A TS1 block is plain: no transform, no entropy coder,
no dictionary.

## Index (AIDX) and trailer (ACET)

| Structure | Layout |
|---|---|
| index | magic `AIDX`, `u64` entry count (≤ `max_index_entries`), then 40-byte entries: block id (8), original offset (8), original size (4), file offset (8), serialized span (4), flags (4), reserved (4) |
| trailer | magic `ACET`, index offset (8), index size (8, ≤ 12 + 40 × `max_index_entries`), index CRC-32C (4), CRC-32C of the trailer's first 24 bytes (4) |

Entry ids are strictly increasing, logical offsets contiguous, spans inside the block region.

## Versions

| Minor | Release | Adds |
|---|---|---|
| 1.0 | 0.1 | header, blocks, RAW / RLE / LZ, byte delta, Huffman, CRC-32C |
| 1.1 | 0.2 | scalar rANS, dictionary descriptors, block index (AIDX) and trailer (ACET), file flags |
| 1.2 | 0.3 | rANS4x entropy coder |
| 1.3 | 0.4 | NUM1 numeric codec (u16 lanes since 0.4.5) |
| 1.4 | 0.5 | TS1 time-series codec (Gorilla f64 / f32, RunDelta) |

The writer declares the minimal version a file needs: 1.4 only when a block uses TS1,
otherwise 1.3. Every field and limit above is cross-referenced to its tests in
[`DECODER-AUDIT.md`](DECODER-AUDIT.md).
