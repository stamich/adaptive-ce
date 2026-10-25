# ACE fuzzing

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
| `engine_roundtrip` | full planner + encoder (incl. the Float lane), all profiles | lossless |
| `bitstream_roundtrip` | `ace_bitpack::{BitWriter, BitReader}` | any width sequence round-trips; reads past the end return `None` |
| `ts1_decode` | TS1 decoder (`ts1_decode`, `ts1_inspect`) | arbitrary payloads rejected or decoded to the declared size, never panic |
| `ts1_roundtrip` | TS1 encode → decode, every layout | bit-exact (`to_bits`), `ts1_encoded_len` equals the payload size |
| `alp_decode` | ALP stream decoder behind a valid TS1 header (0.6.1) | stream fields rejected or decoded to the declared size, never panic |
| `alp_roundtrip` | ALP encode → decode on decimal-shaped and arbitrary series (0.6.1) | bit-exact, exact size counter |
| `numeric_patched_decode` | NUM1 Patched FOR payloads with fuzzer-placed outliers and byte mutations (0.6.1) | valid payload round-trips; mutated exception section rejected or decoded, never panic |
| `ffi_decode` | the C ABI: `ace_decompress`, `ace_decompress_into`, `ace_range_*` (0.7.0) | arbitrary bytes under a 4 MiB output limit: any status except a contained panic or internal error; failing calls leave outputs null |
| `ffi_handles` | the C ABI: random scripts of create / compress / decompress / open / read / free / invalid-argument calls (0.7.0) | no panic or internal error status, handle and status always agree, every handle released |

## Policy

| When | What | Command | Gate |
|---|---|---|---|
| every push / PR (CI job `fuzz-smoke`) | 20 targets x 25 s | `ACE_FUZZ_REQUIRED=1 ./ace-ci0.8.0.sh fuzz-smoke` | no crash, no timeout, no OOM |
| before every release (reference machine) | 20 targets x 10 min, then `cmin` | `ACE_FUZZ_SECONDS=600 ACE_FUZZ_CMIN=1 ./ace-ci0.8.0.sh fuzz` | no crash; result recorded in `examples/baselines/<version>/FUZZ.md` |
| any time | one target | `cargo +nightly fuzz run --fuzz-dir fuzz <target> -- -max_total_time=600` | — |

* `fuzz-smoke` / `fuzz` skip with a log line when nightly or `cargo-fuzz` is missing; CI sets
  `ACE_FUZZ_REQUIRED=1`, so there a missing toolchain fails the job. Both modes also fail when
  the number of targets differs from 15 (a target cannot silently disappear).
* Every crash becomes a regular regression test (for example in
  `crates/ace-engine/tests/malformed_matrix.rs` or the per-field tests listed in
  `docs/DECODER-AUDIT.md`) before it is fixed; the fix lands in the same pull request.

## Corpus policy

* The corpus is **not committed**. `fuzz/corpus/` and `fuzz/artifacts/` are ignored by git
  and excluded from the release package.
* CI keeps the corpus between runs as a cache (`fuzz-corpus-<run>`, restored from the newest
  entry) and publishes corpus and crash artifacts of every run as the artifact
  `fuzz-corpus-and-artifacts`, so a crash can be reproduced from the run that found it.
* The release campaign minimises every corpus with `cargo fuzz cmin` (`ACE_FUZZ_CMIN=1`);
  a minimised corpus can seed the CI cache by uploading it as a new cache entry.

Example (local):

```bash
rustup toolchain install nightly && cargo install cargo-fuzz
./ace-ci0.8.0.sh fuzz-smoke                                  # 20 x 25 s
ACE_FUZZ_SECONDS=600 ACE_FUZZ_CMIN=1 ./ace-ci0.8.0.sh fuzz   # release campaign
```
