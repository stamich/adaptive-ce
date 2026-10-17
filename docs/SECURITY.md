# ACE security and resource model

Applies to the current release (0.5.1). All serialized lengths and counts are untrusted.
Decoders validate configured limits (`ace_core::DecodeLimits`) **before** allocating block,
dictionary, index or output buffers; the type documents which reader applies which limit, and
[`DECODER-AUDIT.md`](DECODER-AUDIT.md) lists every field with its validation, limit and tests.

## Reporting a vulnerability

Report suspected vulnerabilities (a crash, hang, unbounded allocation or misdecode on crafted
input) privately to the maintainer through a GitHub security advisory of the repository
rather than a public issue. Include the input file (or how to produce it), the command or API
call and the ACE version (`ace version` prints version, git revision and backends). Every
confirmed case becomes a regression test before the fix is released.

## Validation

- file magic, version, feature flags and header CRC32C;
- block reconstructed / encoded size limits and per-block CRC32C of the reconstructed bytes;
- transform count, LZ distance / output bounds, Huffman code metadata, rANS normalized-frequency
  sum / state / input bounds, NUM1 header (width, mode, bit width, value count);
- TS1 header (0.5.0): magic, version, mode, lane width allowed for the mode, known flags,
  zero reserved bytes, a tail shorter than one lane, a first value within the lane width,
  `value_count · lane + tail == block size` (checked arithmetic),
  `28 + ceil(stream_bits / 8) + tail == payload size` — all before the output is allocated;
  the stream is read through the bounds-checked `ace_bitpack::BitReader`; Gorilla windows
  with `L + M > width`, RunDelta runs / deltas beyond the value count or the
  lane width, a missing terminator and trailing stream bits are rejected;
- a TS1 block in a file declaring a format minor version below 4 (`UnsupportedVersion`);
- block-index count, ordering, logical contiguity and file spans; index and trailer CRC32C;
- (0.5.1) the block count against `DecodeLimits::max_block_count()` in every reader, the index
  size against `max_index_entries` before the index is read, every header count converted
  with `try_from` (never truncated on 32-bit hosts), the primary-stream length of
  entropy-coded blocks against twice the block size + 1 KiB (a probability-one rANS model
  would otherwise expand four bytes into gigabytes), and streams that must end exactly where
  the encoder stopped (final rANS state, Huffman padding);
- random-access range boundaries;
- declared original size against the reconstructed size and `max_output_size`.

`AceEngine::decompress_into` reserves at most `min(original_size, max_output_size,
PREALLOCATION_CAP_BYTES = 256 MiB)` from the header; larger outputs grow while decoding.

`default_block_size` in the file header is informational: each block header carries its own
size, so a forged value cannot change what is decoded (tested).

## Evidence

| Area | Where |
|---|---|
| malformed matrix: truncation at every 7th byte, magic, version, flags, size/count fields with re-sealed CRC, index/trailer damage, exhaustive single-byte flips | `crates/ace-engine/tests/malformed_matrix.rs` |
| Format 1.4 / TS1 fixture (Gorilla + RunDelta blocks): truncation at every 3rd byte, single-byte flip at every position, header downgraded to 1.3 | same file |
| TS1 header and stream fields one by one (magic, version, mode, lane, flags, reserved, counts, `stream_bits`, `L + M > W`, missing terminator, trailing bits) | `crates/ace-codecs/src/time_series/tests.rs`, `crates/ace-codecs/tests/ts1_properties.rs` |
| resource limits and forged `original_size` (no large pre-allocation) | same file |
| property tests (budget via `PROPTEST_CASES`; release 10 000) | `ace-engine/tests/properties.rs`, `ace-codecs/tests/num1_properties.rs`, `ace-codecs/tests/ts1_properties.rs` (arbitrary f64 / f32 bits incl. NaN payloads, ±0, subnormals), `ace-bitpack/tests/property.rs` |
| 18 fuzz targets (0.5.0: `ts1_decode`, `ts1_roundtrip`, `bitstream_roundtrip`) | `fuzz/README.md` |
| 10 000 random ranges, concurrent readers, out-of-bounds ranges | `ace-engine/tests/random_access_stress.rs` |
| 1 GiB streaming and 256 MiB indexed files (`--ignored`) | `ace-stream/tests/large_files.rs` |
| `unsafe` confined to audited kernels | [`UNSAFE-AUDIT.md`](UNSAFE-AUDIT.md), `tools/ace-code_audit0.6.0.py` |
| no `unwrap` / `expect` / `panic!` / `unreachable!` in production code | [`PANIC-AUDIT.md`](PANIC-AUDIT.md), workspace clippy lints |
| (0.5.1) checked arithmetic and explicit narrowing in every parser module | `clippy::arithmetic_side_effects` + `cast_possible_truncation` per module, [`DECODER-AUDIT.md`](DECODER-AUDIT.md) |
| (0.5.1) every count / size field at 0, `u32::MAX`, `u64::MAX` and overflowing values | `ace-format/tests/index_hardening.rs`, `ace-codecs/tests/header_fields.rs`, `ace-entropy/tests/metadata_fields.rs`, `ace-index/tests/trailer_limits.rs` |
| (0.5.1) every `DecodeLimits` field enforced by the sequential, streamed and indexed readers | `ace-engine/tests/decode_limits.rs` |
| (0.5.1) forged sizes fail before allocation (peak live bytes measured by a counting allocator) | `ace-bench/tests/limits_before_allocation.rs` |
| (0.5.1) files written by 0.2.1 – 0.5.0 still decode | `examples/compat/`, `ace-engine/tests/compat_fixtures.rs` |
| (0.5.1) fuzz smoke (15 × 25 s) on every push; release campaign 15 × 10 min | `fuzz/README.md` |

A malformed file must be rejected by at least one reader and misdecoded by none: the
sequential decoder stops after the declared, CRC-checked blocks, so damage confined to the
index / trailer is reported by the indexed reader.

CRC32C detects accidental corruption and codec reconstruction errors; it is not a
cryptographic authenticity primitive. Protocols that need authenticity must verify their own
cryptographic hashes or signatures after decompression.
