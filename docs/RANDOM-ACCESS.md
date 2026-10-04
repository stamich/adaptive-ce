# Random access

ACE 0.2 blocks remain independent and Format 1.2 adds an explicit block index. `AceIndexedDecoder` loads the index once and supports:

```rust
let block = decoder.decode_block(173)?;
let range = decoder.read_range(10_000_000..11_000_000)?;
```

`read_range` finds index entries intersecting the requested logical range, decodes only those blocks and trims the first/last block boundaries. No preceding block state is needed in milestone 0.2.
