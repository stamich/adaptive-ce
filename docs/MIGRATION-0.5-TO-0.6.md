# Migrating from ACE 0.5.x to 0.6.0

0.6.0 changes how you call ACE, not what it writes. Files are byte-identical to 0.5.1 (golden
0.5.0 is reproduced exactly, with the auto and the scalar SIMD backend) and every file written
by an earlier release stays readable. The Rust API is replaced by one crate, `ace`.

## Files and compatibility

| Situation | What happens |
|---|---|
| 0.6.0 reads files from any earlier release (Format 1.0 – 1.4) | unchanged |
| 0.6.0 writes the same data with the same settings | the same bytes as 0.5.1 (Format 1.3, or 1.4 when a block uses TS1) |
| 0.6.0 meets a file of a newer format (1.5, written by ACE 0.6.1 or later) | rejected; the error names the ACE release that reads it (`… (written by ACE 0.6.1 or newer; this reader supports up to 1.4)`), through every reader and the CLI |
| a pre-0.6 reader meets a 1.5 file | rejected with the plain `UnsupportedVersion` message |

## Build requirements

* Rust **1.98.1** (MSRV and pinned toolchain). Rust 1.98.0 must not be used: it miscompiles
  some trait-object calls. Until 1.0 the MSRV may rise in any minor release.

## Rust API: from three crates to `ace`

Only the `ace` crate is public. `ace-core`, `ace-engine`, `ace-stream` and the other crates are
internal building blocks (`publish = false`, documented as such); do not depend on them.

| 0.5 | 0.6 |
|---|---|
| `AceEngine::new(AceConfig { profile, .. })?` | `Engine::new(Config::builder().profile(Profile::Balanced).build()?)?` |
| `AceEngine::default_engine()` | `Engine::default()` |
| `engine.with_decode_limits(DecodeLimits { max_output_size, ..Default::default() })` | `engine.with_decode_limits(DecodeLimits::builder().max_output_size(n).build())` |
| `compress`, `compress_with_stats`, `compress_to`, `decompress`, `decompress_into`, `decompress_from` | the same names on `ace::Engine`; `compress_with_stats` returns `ace::Stats` |
| `ace_stream::compress_reader_known_size(reader, writer, size, config, limits)` | `engine.compress_stream(reader, writer, size)` (writer: `Write + Seek`) |
| `ace_stream::decompress_stream(reader, writer, limits)` | `engine.decompress_from(reader, writer)` |
| `AceIndexedDecoder::open(reader, limits)` | `engine.open_range_reader(reader)` or `RangeReader::open(reader)` |
| `decoder.read_range(a..b)`, `read_range_with_metrics` | `reader.read_range(a..b)`, `read_range_with_stats` → `RangeStats` |
| `decoder.decode_block(id)`, `block_count()` | `reader.read_block(id)`, `block_count()`; new `len()` |
| `engine.explain(&data)` → `Vec<BlockExplanation>` | feature `explain`: `engine.explain(&data)` → `Vec<BlockReport>` (the full planner records stay internal) |
| `AceError`, `AceResult` | `ace::Error`, `ace::Result`; match on `error.kind()` (`ErrorKind`, non-exhaustive) |

### Settings

`AceConfig` fields map to builder methods: `block_size` → `.block_size(n)`, `profile` →
`.profile(..)`, `threads` → `.threads(n)`, `memory_budget_bytes` → `.memory_budget(n)`,
`write_index` → `.write_index(b)`, `block_size_policy` → `.block_size_policy(..)`,
`access_hint` → `.access_hint(..)`, `enable_numeric_specialization` →
`.numeric_specialization(b)`, `enable_float_specialization` → `.float_specialization(b)`
(`false` gives the decisions of 0.4.6, always Format 1.3). `build()` validates and returns
`ErrorKind::InvalidConfig` for a zero or oversized block size. The tuning fields
`min_gain_bytes`, `enable_early_raw`, `max_in_flight_blocks` and `enable_planner_fast_paths`
are not exposed: they keep their defaults.

### Errors

```rust
match engine.decompress(&bytes) {
    Err(e) if e.kind() == ace::ErrorKind::Unsupported => { /* newer format: e names the release */ }
    Err(e) if e.kind() == ace::ErrorKind::ChecksumMismatch => { /* e.block_id() */ }
    Err(e) => return Err(e.into()),
    Ok(data) => { /* ... */ }
}
```

`ace::Error` implements `std::error::Error` (so `?` into `anyhow` and `Box<dyn Error>` works);
an I/O failure keeps its `std::io::Error` as the `source()`. Input that ends early is
`ErrorKind::Corrupt`, not `Io`.

### Removed

`ace_planner::evaluate_candidates_v3` and `evaluate_candidates_v4` (deprecated in 0.5.1); call
`evaluate_candidates_v4_with_route` with `RoutePolicy::classify(..)`. They were internal.

## CLI

The commands, options and exit statuses are unchanged. Two output details differ:

* `compress` prints one line: `ACE 0.6.0 compressed IN -> OUT bytes ratio=… blocks=… raw=…
  rle=… lz=… numeric=… ts1=… format=1.N`. The planner counters (`rANS`, `Huffman`, `fast-path`,
  `sampled`) and the second Float-lane line are gone; `ace explain` and `ace inspect` still show
  the details.
* `compress-stream` prints `ACE 0.6.0 streamed IN -> OUT bytes blocks=… format=1.N`
  (`peak_source_buffer` is gone).

Scripts that parse the first words of these lines (`ACE <version> compressed|streamed`) keep
working.

## Benchmarks and tooling

* result files are `benchmark-0.6.0-<family>.json` (schema 2.2 unchanged); the stored baseline
  and the A/B base are 0.5.1 (`examples/baselines/0.5.1/`);
* new family `public-corpus` (`ACE_PUBLIC_CORPUS=<dir>`), see [`BENCHMARKS.md`](BENCHMARKS.md);
* scripts and workflows carry the version in their names (`ace-ci0.6.0.sh`,
  `ace-ci-0.6.0` workflow): update required status checks in the repository settings. The
  separate `msrv` job is gone — the `pr` job already builds with the MSRV.
