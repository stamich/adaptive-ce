# Random access

ACE 0.2 blocks remain independent and Format 1.2 adds an explicit block index. `AceIndexedDecoder` loads the index once and supports:

```rust
let block = decoder.decode_block(173)?;
let range = decoder.read_range(10_000_000..11_000_000)?;
```

`read_range` finds index entries intersecting the requested logical range, decodes only those blocks and trims the first/last block boundaries. No preceding block state is needed in milestone 0.2.

## Segment-level access (ACE 0.8.0, Format 1.6)

A segmented block (`SEG1`, [`FORMAT-1.6.md`](FORMAT-1.6.md)) is a table of segments followed by one
inner block per segment, so a range read does not need the whole block. The writer sets the index
entry flag `INDEX_FLAG_SEGMENTED` (`0x1`) on every codec-5 block; for such an entry
`AceIndexedDecoder::read_range` does this instead of reading the block:

1. read the block header (32 bytes and its descriptors) and check that it is a bare container;
2. read the 12-byte `SEG1` head, check the segment count against `DecodeLimits::max_segments`,
   then read the table and validate it against the outer header (sizes and lengths add up);
3. read and decode only the segments whose logical range overlaps the request, each as an inner
   block with its own checksum, and trim the first and last of them.

`RangeAccessMetrics::physical_bytes_read` is the sum of those reads (header + table + touched
segments), so it never exceeds the block's indexed span, and `segments_decoded` counts the decoded
segments (zero for unsegmented blocks). `range_metrics`, which reads nothing, still reports the
spans, an upper bound.

What is not checked on this path: the outer block checksum covers the whole reconstructed block, so
only a full read (`decode_block`, `decompress`, streaming) verifies it. A range read verifies the
table and every segment it decodes. A missing flag is harmless — the block is then read whole; a
flag on a block that is not segmented, or any other flag bit, is `Malformed`.
