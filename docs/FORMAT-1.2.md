# ACE Format 1.2

Format 1.2 is an additive minor revision of the ACE1 framing used by 1.0 and 1.1.

## Unchanged

- 32-byte file header;
- 32-byte fixed block header plus transform/dictionary descriptors;
- CRC32C structural and reconstructed-payload checks;
- independent blocks;
- optional `AIDX` block index and `ACET` trailer;
- little-endian integer encoding.

## New in 1.2

Entropy identifier `3` means `Rans4x`. Older minor versions must not contain this identifier. ACE 0.3 reads 1.0/1.1/1.2 and writes 1.2.

## rANS4x metadata

`Rans4x` stores four fixed 512-byte scalar normalized-frequency models followed by four little-endian `u32` lane payload lengths. Payload bytes are four lane streams concatenated in lane order. Logical bytes are assigned round-robin to lanes and re-interleaved by the decoder.
