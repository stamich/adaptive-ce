# ACE 0.7.0 — fuzz campaign

| Item | Value |
|---|---|
| Date | 2026-10-21 |
| Machine | AMD Ryzen 9 5950X, Gentoo Linux |
| Toolchain | Rust nightly (rustup) + `cargo-fuzz`, AddressSanitizer (default), entropic power schedule |
| Budget | 20 targets × 600 s (`-max_total_time=600`) |
| Result | **20 / 20 finished, no crash, OOM, timeout or sanitizer report in any log** |

| Target | Executions | Wall time (s) |
|---|---|---|
| `alp_decode` | 56,791,082 | 601 |
| `alp_roundtrip` | 668,882 | 601 |
| `bitpack_decode` | 349,127,985 | 601 |
| `bitstream_roundtrip` | 61,322,945 | 601 |
| `container_decode` | 194,280,576 | 601 |
| `engine_roundtrip` | 182,345 | 601 |
| `ffi_decode` | 26,701,325 | 601 |
| `ffi_handles` | 56,550 | 601 |
| `index_parse` | 493,605,131 | 601 |
| `numeric_decode` | 415,433,833 | 601 |
| `numeric_patched_decode` | 17,702,139 | 601 |
| `numeric_roundtrip` | 8,053,258 | 601 |
| `planner_sample_offsets` | 461,649,310 | 601 |
| `range_open` | 516,716,334 | 601 |
| `rans4x_decoder` | 672,706,122 | 601 |
| `rans_decoder` | 214,787,750 | 601 |
| `rle_roundtrip` | 67,309,237 | 601 |
| `trailer_parse` | 661,197,451 | 601 |
| `ts1_decode` | 443,956,630 | 601 |
| `ts1_roundtrip` | 892,299 | 601 |

Total: 4,663,141,184 executions; the counts are the `Done N runs` line of each log. New in 0.7.0:
`ffi_decode` (decoding through the C ABI) and `ffi_handles` (handle lifecycle: open, use, close,
double close, use after close). `engine_roundtrip`, `ts1_roundtrip`, `alp_roundtrip`,
`numeric_roundtrip` and `ffi_handles` run a whole codec round trip or a handle sequence per input and
therefore execute orders of magnitude fewer inputs than the parsers and decoders.

## `ts1_decode` / `ts1_roundtrip` (0.6.1 wall-time anomaly)

Both targets ended with `Done … in 5487 second(s)` for a 600 s budget in the 0.6.1 campaign, which
pointed to a suspended host. In this campaign both finished in 601 s with 443,956,630 and
892,299 executions and no slow unit, so the anomaly did not reproduce.
