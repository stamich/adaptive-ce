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
- NUM1 mode 4, `Patched` (Patched FOR over a FOR, Delta or DoD stream, see below);
- TS1 mode 4, `Alp` (decimal floats, see below and [`TS1-CODEC.md`](TS1-CODEC.md));
- a stream whose header names a mode of Format 1.5 is valid only in a file that declares 1.5.

RunDelta for small non-zero deltas (admission by the run prefilter) is a planner change: it
reuses TS1 mode 3 and needs no format change.

## NUM1 mode 4: Patched FOR

Mode 4 stores the stream of a *source* mode (FOR offsets, ZigZag deltas or ZigZag
delta-of-deltas) at a bit width `b` that fits most values, and lists the few values that need
more bits as exceptions. A stream value `x` is split into `low = x mod 2^b`, which stays in its
packed slot, and `high = x >> b`; the values with `high != 0` are the exceptions. The header
keeps the 40-byte NUM1 shape:

| Offset | Field | Mode 4 meaning |
|---|---|---|
| 5 | mode | `4` |
| 6 | bit width | `b`, the packed width of the low parts (0 ..= lane bits) |
| 7 | reserved in modes 1 – 3 | source mode: `1` FOR, `2` Delta, `3` DoD |
| 16 / 24 | first value / first delta | as in the source mode (FOR base, first value, DoD first delta) |
| 32 | packed length | `ceil(stream_len * b / 8)` |
| 36 | reserved in modes 1 – 3 | exception count `m` (u32, `m <= stream_len`) |

The body is the packed low parts, the **exception section** (present only when `m > 0`) and the
tail bytes. The section is `h` (one byte, `1 ..= lane bits - b`), the `m` positions bit-packed at
`bits(stream_len - 1)` bits each, then the `m` high parts bit-packed at `h` bits each (LSB-first,
the same packing as the main stream). Positions are strictly increasing and below `stream_len`;
every high part is non-zero. A decoder rejects anything else, and checks the section length
before it allocates or produces a value. `stream_len` is `n` for FOR, `n - 1` for Delta and
`n - 2` for DoD.

The writer chooses the source and `b` by exact size: a histogram of the bit length of every
stream value gives the exception count and the high-part width for each `b`, so the estimate
(`ace_codecs::estimate_patched`) equals the payload length byte for byte. Ties are broken by
(size, source id, `b`), and Patched is only selected over a base mode when it is strictly smaller.
The planner includes Patched FOR in its exact numeric estimate (`estimate_numeric`): it is
considered when the best base mode needs at least 4 bits per value and is selected only when
strictly smaller. A block that wins with it makes the file Format 1.5; every other block, and
therefore every file without outliers, is written exactly as by ACE 0.6.0.
Example: the counter `monotonic-outliers` (step 1, a +100 000 jump every 1024 values) has a
zero delta-of-delta almost everywhere; mode 4 stores it with `b = 0` and two exceptions per jump
instead of 17-bit deltas.

## TS1 mode 4: ALP

Decimal floats (`12.37`, `1013.25`) are integers in disguise. ALP scales every value by
`10^e · 10^-f`, rounds to an integer `d`, and stores `d` with a frame of reference and a fixed
bit width. A value that does not come back bit for bit (`-0.0`, NaN, ±Inf, a full-precision
double) is an exception and is stored verbatim.

* Lanes: 8 bytes (binary64) or 4 bytes (binary32). The header carries `first_bits = 0` and no
  flags; `value_count`, `tail_len` and `stream_bits` keep their TS1 meaning.
* Stream: `e:5 f:5 width:7 base:64 gamma(exception_count)`, then per exception
  `gamma(gap) raw_bits`, then `value_count` offsets of `width` bits. `0 <= f <= e <= 18`,
  `width <= 64`; the offset of an exception slot is zero.
* `d = round(v · 10^e · 10^-f)` and `v = d · 10^f · 10^-e` are evaluated in binary64 in this
  order (binary32 lanes are widened, decoded in binary64 and narrowed). The encoder keeps a
  value only when the decoded bits equal the original bits and `|v · 10^e · 10^-f| < 2^53`.
* The decoder rejects exponents or widths out of range, more exceptions than values, positions
  out of order or past the end, an exception wider than the lane, a stream whose length is not
  exactly the declared sections, a non-zero exception slot and an offset that leaves `i64`.

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
