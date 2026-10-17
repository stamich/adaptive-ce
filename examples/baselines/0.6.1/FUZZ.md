# ACE 0.6.1 — fuzz campaign

| Item | Value |
|---|---|
| Date | 2026-10-17 |
| Machine | AMD Ryzen 9 5950X, Gentoo Linux |
| Toolchain | Rust nightly (rustup) + `cargo-fuzz`, AddressSanitizer (default), entropic power schedule |
| Budget | 18 targets × 600 s (`-max_total_time=600`) |
| Result | **18 / 18 finished, no crash, OOM or sanitizer report in any log** |

| Target | Executions |
|---|---|
| `container_decode` | 228,938,882 |
| `engine_roundtrip` | 241,141 |
| `bitstream_roundtrip` | 76,137,346 |
| `numeric_roundtrip` | 7,582,289 |
| `index_parse` | 497,210,699 |
| `numeric_decode` | 431,558,390 |
| `bitpack_decode` | 451,371,276 |
| `planner_sample_offsets` | 464,586,820 |
| `rans_decoder` | 215,822,157 |
| `range_open` | 507,433,299 |
| `rle_roundtrip` | 129,036,286 |
| `rans4x_decoder` | 667,362,462 |
| `trailer_parse` | 638,372,876 |
| `ts1_decode` | 441,428,564 |
| `ts1_roundtrip` | 809,533 |
| `alp_decode` | 124,327,118 |
| `alp_roundtrip` | 699,489 |
| `numeric_patched_decode` | 17,202,729 |

Total: 4,900,121,356 executions; the counts are the `Done N runs` line of each log. New in 0.6.1:
`alp_decode`, `alp_roundtrip`, `numeric_patched_decode`. `engine_roundtrip`, `ts1_roundtrip`,
`alp_roundtrip` and `numeric_roundtrip` run a whole-codec round trip per input and therefore execute
two to three orders of magnitude fewer inputs than the parsers and decoders.

## Wall-time anomaly (`ts1_decode`, `ts1_roundtrip`)

Both logs end with `Done … in 5487 second(s)` instead of 601 s, and libFuzzer reported one slow
unit of 4893 s for `ts1_roundtrip` (`slow-unit-28c214878085f14a549a0ddef58978d26c219e8b`). Both
targets stretched by the same amount, `ts1_decode` had run at about 740 000 exec/s until its last
pulse, and no other target is affected, which points to the host being suspended during this part of
the run rather than to an input. The unit is not part of the archive, so this is not proven: a re-run of
the two targets (`ACE_FUZZ_SECONDS=600 ./ace-ci0.6.1.sh fuzz`) or timing of that unit settles it.
