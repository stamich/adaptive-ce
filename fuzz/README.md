# ACE fuzzing (0.4.6)

`fuzz/` is a separate `cargo-fuzz` project (not a workspace member; requires a nightly
toolchain and `cargo install cargo-fuzz`).

| Target | Surface | Invariant |
|---|---|---|
| `container_decode` | `AceEngine::decompress` | arbitrary bytes: error or success, never panic / unbounded allocation |
| `range_open` | `AceIndexedDecoder::open` | malformed header/index/trailer rejected safely |
| `index_parse` | `ace_format::decode_index` | entry limits enforced before allocation |
| `trailer_parse` | `ace_format` trailer parser | rejected safely |
| `numeric_decode` | NUM1 decoder | bounded output, no panic |
| `numeric_roundtrip` | NUM1 encode → decode | lossless; estimator predicts the exact size |
| `bitpack_decode` | `ace_bitpack::unpack` (u16/u32/u64) | any width/count rejected or decoded |
| `rans_decoder` | scalar rANS | rejected safely |
| `rans4x_decoder` | interleaved rANS4x | rejected safely |
| `rle_roundtrip` | RLE codec | lossless |
| `planner_sample_offsets` | `ace_cost::deterministic_sample_ranges` | ranges stay in bounds |
| `engine_roundtrip` | full planner + encoder, all profiles | lossless |

## Campaigns

```bash
cargo +nightly fuzz run <target> -- -max_total_time=600     # release: 12 x 10 min
cargo +nightly fuzz run <target> -- -max_total_time=60      # nightly CI: 12 x 60 s
./ace-ci0.4.6.sh fuzz                                       # all targets, ACE_FUZZ_SECONDS each
```

Crashes land in `fuzz/artifacts/<target>/`; a minimised corpus may be kept in
`fuzz/corpus/<target>/` (`cargo fuzz cmin <target>`). Neither directory is packaged.
Every crash becomes a regular regression test (for example in
`crates/ace-engine/tests/malformed_matrix.rs`) before it is fixed.
