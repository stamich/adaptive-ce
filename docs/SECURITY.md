# ACE 0.2 security and resource model

All serialized lengths and counts are untrusted. Decoders validate configured limits before allocating block, dictionary or index buffers.

The decoder validates:

- file magic/version/feature flags,
- file and block CRC32C,
- block reconstructed and encoded size limits,
- transform count,
- LZ distance/output bounds,
- Huffman code metadata,
- rANS normalized-frequency sum/state/input bounds,
- block-index count, ordering, logical contiguity and file spans,
- index and trailer CRC32C,
- requested random-access range boundaries.

CRC32C detects accidental corruption and codec reconstruction errors; it is not a cryptographic authenticity primitive. GraphNet or another authenticated protocol must continue to verify its own cryptographic hashes/signatures after decompression.
