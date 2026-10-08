# ACE 0.5.0 — fuzz campaign

| Item | Value |
|---|---|
| Date | 2026-10-08 |
| Machine | AMD Ryzen 9 5950X, Gentoo Linux |
| Toolchain | Rust nightly (rustup) + `cargo-fuzz`, AddressSanitizer (default) |
| Budget | 15 targets × 600 s (`-max_total_time=600`), 8 targets in parallel |
| Result | **15 / 15 exit 0, no crash, OOM or timeout artifacts** |

```bash
cargo +nightly fuzz list --fuzz-dir fuzz | \
  xargs -P 8 -I{} sh -c 'cargo +nightly fuzz run --fuzz-dir fuzz {} -- -max_total_time=600 > fuzz-{}.log 2>&1; echo "{}: exit $?"'
```

| Target | Exit |
|---|---|
| `container_decode` | 0 |
| `engine_roundtrip` | 0 |
| `bitstream_roundtrip` | 0 |
| `numeric_roundtrip` | 0 |
| `index_parse` | 0 |
| `numeric_decode` | 0 |
| `bitpack_decode` | 0 |
| `planner_sample_offsets` | 0 |
| `rans_decoder` | 0 |
| `range_open` | 0 |
| `rle_roundtrip` | 0 |
| `rans4x_decoder` | 0 |
| `trailer_parse` | 0 |
| `ts1_decode` | 0 |
| `ts1_roundtrip` | 0 |
