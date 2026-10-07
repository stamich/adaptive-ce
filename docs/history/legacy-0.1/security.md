# Security model

ACE format bytes are untrusted input. Decoder limits are checked before block metadata/payload allocations. LZ distance,
match length and expected output size are validated. Huffman metadata is bounded to 256 code lengths and canonical code
space is validated. CRC32C is used only for accidental-corruption detection, not authentication.

For network or GraphNet integration a cryptographic hash/signature must remain outside the ACE physical encoding layer.
