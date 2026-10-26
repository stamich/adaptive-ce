# ACE 0.8.0 — fuzz campaign

| Item | Value |
|---|---|
| Date | 2026-10-26 (log timestamps) |
| Machine | AMD Ryzen 9 5950X, Gentoo Linux |
| Toolchain | Rust nightly + `cargo-fuzz`, AddressSanitizer (default) |
| Budget | 22 targets × 600 s (`-max_total_time=600`) |
| Result | **22 / 22 finished, no crash, OOM, timeout or sanitizer report in any log** |

| Target | Executions | Wall time (s) |
|---|---|---|
| `alp_decode` | 64,996,000 | 601 |
| `alp_roundtrip` | 909,479 | 601 |
| `bitpack_decode` | 547,767,456 | 601 |
| `bitstream_roundtrip` | 79,312,557 | 601 |
| `col_decode` | 7,971,037 | 601 |
| `container_decode` | 197,758,842 | 601 |
| `engine_roundtrip` | 247,659 | 601 |
| `ffi_decode` | 28,735,524 | 601 |
| `ffi_handles` | 10,470 | 601 |
| `index_parse` | 370,495,731 | 601 |
| `numeric_decode` | 284,308,802 | 601 |
| `numeric_patched_decode` | 12,821,484 | 601 |
| `numeric_roundtrip` | 6,481,996 | 601 |
| `planner_sample_offsets` | 335,839,000 | 601 |
| `range_open` | 356,776,891 | 601 |
| `rans4x_decoder` | 480,186,687 | 601 |
| `rans_decoder` | 219,388,068 | 601 |
| `rle_roundtrip` | 130,134,981 | 601 |
| `seg_decode` | 11,684,822 | 601 |
| `trailer_parse` | 662,555,641 | 601 |
| `ts1_decode` | 404,306,521 | 601 |
| `ts1_roundtrip` | 946,357 | 601 |

Total: 4,203,636,005 executions; the counts are the `Done N runs` line of each log. New in 0.8.0:
`seg_decode` (SEG1 segment tables and segmented blocks) and `col_decode` (COL1 column tables and
columnar blocks). The word `panicking` appears in the `ffi_decode` log only in libFuzzer's
`NEW_FUNC` coverage lines for the `catch_unwind` wrappers, not as a panic. `engine_roundtrip`,
`ts1_roundtrip`, `alp_roundtrip`, `numeric_roundtrip` and `ffi_handles` run a whole round trip or
handle sequence per input and therefore execute far fewer inputs than the parsers and decoders.
