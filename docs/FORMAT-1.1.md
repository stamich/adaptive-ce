# ACE Format 1.2

Format 1.2 keeps the 32-byte `ACE1` file header used by Format 1.0. Minor version changes from `0` to `1`. ACE 0.2 readers accept both minor versions; writers emit 1.1.

## Physical order

```text
32-byte file header
block 0
block 1
...
block N
AIDX block index        (when HAS_INDEX)
ACET trailer            (when HAS_INDEX)
```

Each block consists of the 32-byte fixed block header, transform descriptors, an optional 9-byte dictionary descriptor, entropy metadata, and entropy-coded payload. `payload_crc32c` protects reconstructed uncompressed block bytes.

## Index

`AIDX` contains a count followed by 40-byte entries carrying block id, logical original offset/size, physical file offset, serialized block span and flags. Entries must have strictly increasing block ids and contiguous logical offsets.

## Trailer

`ACET` stores the index offset, index size, index CRC32C and trailer CRC32C. Locating the index therefore requires one seek to the final 28 bytes.

## Compatibility

Format 1.0 files contain neither dictionary descriptors nor index/trailer sections. Their block and entropy metadata layout is otherwise preserved, including the four-byte primary codec stream length prefix before Huffman metadata.
