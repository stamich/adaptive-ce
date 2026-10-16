# ACE 0.6.0 — fuzz campaign

| Item | Value |
|---|---|
| Date | 2026-10-16 |
| Machine | AMD Ryzen 9 5950X, Gentoo Linux |
| Toolchain | Rust nightly (rustup) + `cargo-fuzz`, AddressSanitizer (default), entropic power schedule |
| Budget | 15 targets × 600 s (`-max_total_time=600`) |
| Result | **15 / 15 finished (601 s each), no crash, OOM, timeout or sanitizer report in any log** |

| Target | Executions |
|---|---|
| `container_decode` | 263,827,501 |
| `engine_roundtrip` | 370,632 |
| `bitstream_roundtrip` | 79,100,147 |
| `numeric_roundtrip` | 27,141,678 |
| `index_parse` | 489,403,865 |
| `numeric_decode` | 454,344,332 |
| `bitpack_decode` | 473,937,630 |
| `planner_sample_offsets` | 467,029,355 |
| `rans_decoder` | 228,691,234 |
| `range_open` | 510,163,224 |
| `rle_roundtrip` | 84,142,103 |
| `rans4x_decoder` | 679,719,467 |
| `trailer_parse` | 654,168,056 |
| `ts1_decode` | 434,605,497 |
| `ts1_roundtrip` | 1,471,189 |

Total: 4,848,115,910 executions; the counts are the `Done N runs in 601 second(s)` line of each log.
`engine_roundtrip` and `ts1_roundtrip` run a whole-engine / whole-codec round trip per input and
therefore execute two to three orders of magnitude fewer inputs than the parsers and decoders.
