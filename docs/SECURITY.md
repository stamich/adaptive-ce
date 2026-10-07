# ACE security and resource model (0.4.6)

All serialized lengths and counts are untrusted. Decoders validate configured limits
(`ace_core::DecodeLimits`) **before** allocating block, dictionary, index or output buffers.

## Validation

- file magic, version, feature flags and header CRC32C;
- block reconstructed / encoded size limits and per-block CRC32C of the reconstructed bytes;
- transform count, LZ distance / output bounds, Huffman code metadata, rANS normalized-frequency
  sum / state / input bounds, NUM1 header (width, mode, bit width, value count);
- block-index count, ordering, logical contiguity and file spans; index and trailer CRC32C;
- random-access range boundaries;
- declared original size against the reconstructed size and `max_output_size`.

`AceEngine::decompress_into` reserves at most `min(original_size, max_output_size,
PREALLOCATION_CAP_BYTES = 256 MiB)` from the header; larger outputs grow while decoding.

`default_block_size` in the file header is informational: each block header carries its own
size, so a forged value cannot change what is decoded (tested).

## Evidence (0.4.6)

| Area | Where |
|---|---|
| malformed matrix: truncation at every 7th byte, magic, version, flags, size/count fields with re-sealed CRC, index/trailer damage, exhaustive single-byte flips | `crates/ace-engine/tests/malformed_matrix.rs` |
| resource limits and forged `original_size` (no large pre-allocation) | same file |
| property tests (budget via `PROPTEST_CASES`; release 10 000) | `ace-engine/tests/properties.rs`, `ace-codecs/tests/num1_properties.rs`, `ace-bitpack/tests/property.rs` |
| 12 fuzz targets | `fuzz/README.md` |
| 10 000 random ranges, concurrent readers, out-of-bounds ranges | `ace-engine/tests/random_access_stress.rs` |
| 1 GiB streaming and 256 MiB indexed files (`--ignored`) | `ace-stream/tests/large_files.rs` |
| `unsafe` confined to audited kernels | `docs/UNSAFE-AUDIT-0.4.6.md`, `tools/ace-code_audit0.4.6.py` |
| no `unwrap` / `expect` / `panic!` in production code | `docs/PANIC-AUDIT-0.4.6.md`, workspace clippy lints |

A malformed file must be rejected by at least one reader and misdecoded by none: the
sequential decoder stops after the declared, CRC-checked blocks, so damage confined to the
index / trailer is reported by the indexed reader.

CRC32C detects accidental corruption and codec reconstruction errors; it is not a
cryptographic authenticity primitive. Protocols that need authenticity must verify their own
cryptographic hashes or signatures after decompression.
