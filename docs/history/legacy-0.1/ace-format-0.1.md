# ACE Format v1.0

All multi-byte integers are little-endian.

## File header (32 bytes)

| Offset | Size | Field |
|---:|---:|---|
| 0 | 4 | ASCII `ACE1` |
| 4 | 1 | major = 1 |
| 5 | 1 | minor = 0 |
| 6 | 2 | flags |
| 8 | 4 | default block size |
| 12 | 8 | original file size |
| 20 | 8 | block count |
| 28 | 4 | CRC32C of bytes 0..28 |

## Block header

A fixed 32-byte header is followed by `transform_count` one-byte transform IDs, entropy metadata and encoded payload.
The fixed header stores block ID, original size, encoded payload size, metadata size, codec ID, entropy ID, transform
count, flags, CRC32C of reconstructed original bytes and CRC32C of fixed header bytes 0..28.

Huffman metadata begins with a four-byte little-endian size of the primary-codec token stream followed by 256 canonical
code lengths. This intentionally favors simple parsing over compact headers in milestone 0.1.
