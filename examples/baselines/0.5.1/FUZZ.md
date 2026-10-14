# ACE 0.5.1 — fuzz campaign

| Item | Value |
|---|---|
| Date | 2026-10-14 |
| Machine | AMD Ryzen 9 5950X, Gentoo Linux |
| Toolchain | Rust nightly (rustup) + `cargo-fuzz`, AddressSanitizer (default), entropic power schedule |
| Budget | 15 targets × 600 s (`-max_total_time=600`) |
| Result | **15 / 15 finished (601 s each), no crash, OOM, timeout or sanitizer report in any log** |

| Target | Executions |
|---|---|
| `container_decode` | 253,217,192 |
| `engine_roundtrip` | 286,212 |
| `bitstream_roundtrip` | 74,680,862 |
| `numeric_roundtrip` | 27,990,002 |
| `index_parse` | 498,662,491 |
| `numeric_decode` | 456,184,448 |
| `bitpack_decode` | 475,060,326 |
| `planner_sample_offsets` | 466,672,384 |
| `rans_decoder` | 225,120,153 |
| `range_open` | 531,900,294 |
| `rle_roundtrip` | 85,909,416 |
| `rans4x_decoder` | 683,022,802 |
| `trailer_parse` | 656,171,207 |
| `ts1_decode` | 428,426,544 |
| `ts1_roundtrip` | 2,367,748 |

Total: 4,865,672,081 executions; the counts are the `Done N runs in 601 second(s)` line of each
log. `engine_roundtrip` and `ts1_roundtrip` run a whole-engine / whole-codec round trip per input
and therefore execute two to three orders of magnitude fewer inputs than the parsers and decoders.
