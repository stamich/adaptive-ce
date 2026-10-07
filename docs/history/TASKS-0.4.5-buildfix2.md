# ACE 0.4.5-buildfix2 — implementation order

Scope: close the last failing gate (warm64K), restructure every crate (`lib.rs` = `mod` +
`pub use` only), remove duplication, keep every encoded byte identical.

## Phase A — analysis and safety net
1. Analyse 0.4.5-buildfix1 results → `docs/BENCHMARK-ANALYSIS-0.4.5-buildfix1.md`.
2. Freeze a pre-refactor binary; byte-compare `compress` (FAST/BALANCED/DENSE) and
   `compress-stream` on Corpus V3 + u16/ns data after every phase.
3. Add `examples/baselines/0.4.5-buildfix1/` (user results) for future A/B.

## Phase B — `ace-bitpack` (generic lanes)
4. `trait Lane` (sealed) with `Signed`, `BITS`, `BYTES`, wrapping ops, ZigZag, LE I/O; `impl_lane!`.
5. `read_lanes`, `write_lanes`, `write_lanes_from_iter` (`lane.rs`).
6. `pack`, `unpack`, `unpack_iter`, `packed_len` (`scalar.rs`); `read_bits_validated` (`bit_io.rs`).
7. `delta`/`undelta`/`undelta_iter`, `delta_of_delta`/`undelta_of_delta(_iter)`,
   `frame_of_reference`/`unframe_of_reference`/`unframe_iter`, `max_bit_width`, `zigzag_i64`.
8. Remove the width-specific `*_u16/_u32/_u64` copies; property tests per lane via macro.

## Phase C — `ace-codecs`
9. `dispatch.rs` (`encode_codec`, `decode_codec`).
10. `numeric/` modules: `mode` (`NumericMode::stream_len`), `header` (`NumericPayloadInfo::decoded_len`,
    `numeric_inspect`, `serialize_payload`), `estimate` (`LaneStats` generic, OR-accumulated
    bit widths, `estimate_every_mode`), `encode` (`encode_lane::<T>`), `decode` (fused
    `decode_lane::<T>`, `decode_zero_width::<T>`), `lane_dispatch` (`dispatch_lane!`, `width_of`).

## Phase D — `ace-entropy`
11. `dispatch.rs` incl. `decode_entropy_cow` (borrow for `None`).
12. `huffman/`: `code_lengths`, `canonical` (`DecodeTables` with 11-bit table), `bit_io`
    (64-bit `BitReader`, accumulator `BitWriter`), `encoder`, `decoder` (fast table + canonical fallback).
13. `rans/`: `model` (`NormalizedFrequencyTable`), `coder`.

## Phase E — container crates
14. `ace-format`: `checksum`, `block_io::read_serialized_block`, `AceWriter` with index/trailer.
15. `ace-index`: `index_reader` on top of `read_serialized_block`.
16. `ace-simd`: `backend`, `scan`, `avx2`, `crc32c::crc32c_hardware`; `ace_format::checksum` uses it.

## Phase F — core, planner, engine, stream
17. `ace-core`: `EntropyCodecId::{metadata_prefix_bytes, label}`, `PRIMARY_LENGTH_PREFIX_BYTES`,
    `PhysicalCompressionPlan::{label, is_plain_numeric}`, `CompressionStats::record_selected_plan`.
18. `ace-planner`: `decision`, `pipeline`, `plan_identity`, `exhaustive`; `RankedPlan` +
    `EntropySelectionPolicy::penalize_weak_rans`; `hybrid` uses `ace_cost::stratified_ranges`.
19. `ace-engine`: `block_encoder` (`encode_block`, `BlockTimings`, `apply_raw_fallback`),
    `block_pipeline` (`wrap/split_entropy_metadata`, `decode_encoded_block`), `container`
    (`assemble_container`), `engine` façade (`explain_block`).
20. `ace-stream`: `limits`, `encoder` (`validate_stream`, `encode_single_block`, `AceWriter`), `decoder`.
21. `ace-cost`, `ace-runtime`, `ace-dictionary`, `ace-transforms`: one module per type/concern.

## Phase G — binaries, tests, tooling
22. `ace-cli`: `args`, `files`, `commands/{compress,decompress,inspect,explain}`.
23. Benchmark harness: `prelude`, `json` (`MILESTONE`), `timing`, `corpus`, `plan_util`, `families/*`.
24. New tests: lane/zigzag/width/scalar/delta/DoD/FOR units, Huffman long-code and malformed
    cases, bit-reader/writer reference test, CRC32C reference + GF(2) shift test, `AceWriter`
    index test, stream == in-memory identity, plan label / stats / entropy policy tests.
25. Fuzz target `bitpack_decode` migrated to generic `unpack::<T>` (all targets compile).
26. Clippy clean (`--all-targets`, MSRV 1.75), rustdoc clean, doc-comment coverage 100 %.

## Phase H — release
27. Rename scripts/tools/results to `0.4.5-buildfix2`; move previous milestone files to `docs/history/`.
28. README, CHANGELOG, ROADMAP, `docs/ARCHITECTURE-0.4.5-BUILDFIX2.md`, `MILESTONE-0.4.5-buildfix2.json`.
29. Full `ace-benchmark0.4.5-buildfix2.sh all` (container: 23/23 gates PASS).
