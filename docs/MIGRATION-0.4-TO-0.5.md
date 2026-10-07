# Migrating from ACE 0.4.x to 0.5.0

0.5.0 is a functional release: it adds Format 1.4 (TS1 codec) and Planner V5 (Float lane).
Data written by 0.4.x stays readable; data that 0.4.x already compressed well is written
exactly as before. A handful of public types gained fields or variants.

## Files and compatibility

| Situation | What happens |
|---|---|
| 0.5.0 reads files from 0.4.x (Format 1.0 – 1.3) | unchanged |
| 0.5.0 compresses data without a winning TS1 block (e.g. all of the 0.4 corpus) | Format 1.3, byte-identical to 0.4.6 |
| 0.5.0 compresses float / sparse-change series where TS1 wins | Format 1.4 — **0.4.x readers reject it** (`UnsupportedVersion`) |
| you need files for 0.4.x readers | CLI `--disable-float`, or `AceConfig { enable_float_specialization: false, .. }`: decisions and bytes of 0.4.6, always Format 1.3 |

`ace inspect` prints the declared format version on its first line.

## Source changes

| Change | Who is affected | Action |
|---|---|---|
| `ace_stream::compress_reader_known_size` requires `W: Write + Seek` (the 1.4 header is rewritten in place) | callers passing a non-seekable writer (pipe, socket, `&mut Vec<u8>`) | pass a `File`, `BufWriter<File>` or `Cursor<&mut Vec<u8>>`; for a pipe, compress into a `Cursor<Vec<u8>>` first |
| `AceConfig::enable_float_specialization: bool` (default `true`) | struct literals without `..AceConfig::default()` | add the field or use `..Default::default()` |
| `CodecId::TimeSeries`, `AceError::InvalidTimeSeries(&'static str)` | exhaustive `match` on these enums | add an arm |
| `PlannerRoute::{FloatGeneral, FloatFast}`, `RouteReason::{FloatCandidate, DominantFloat}` | exhaustive matches (planner tooling) | add arms; `PlannerRoute::label()` gives stable names |
| `RouteDecision` gains `base_route`, `float_evidence`, `run_prefilter` | code building `RouteDecision` literals; code that used `route.route` to drive the generic pipeline | use `RoutePolicy::classify`; use `route.candidate_route()` where the V4.3 candidate route is meant |
| `PlannerTelemetry` gains `time_series_estimates`, `float_fast_hit`, `float_fast_fallback`; `PlannerDecision` gains `time_series` | struct literals | add the fields (`..PlannerTelemetry::default()`) |
| `CompressionStats` gains Float lane / TS1 counters; `StreamingStats` gains `format_1_4` | struct literals (rare; both derive `Default`) | `..Default::default()` |
| `BlockExplanation` gains `route` and `time_series` | struct literals | none for readers of the struct |
| `ace_format::FORMAT_MINOR` is 4; new `FORMAT_MINOR_BASE` (3) and `minimal_minor_version` | custom container writers | declare `minimal_minor_version(block codecs)`; `encode_file_header` writes `min(minor_version, FORMAT_MINOR)` |
| `ace_bitpack::{BitWriter, BitReader}` are public (the private `bit_io` module became `bitstream`) | nobody (additive) | — |

Everything else — `AceEngine::{compress, compress_with_stats, compress_to, decompress,
decompress_into, decompress_from, explain}`, `AceIndexedDecoder`, `DecodeLimits` — keeps its
signature.

## CLI

* new flag `--disable-float` for `compress`, `compress-stream` and `explain`;
* `compress` prints a second line with Float lane counters when the lane was used;
  `compress-stream` reports `format=1.3|1.4`;
* `inspect --blocks` shows `ts1 mode=… lane=… values=… bits_per_value=…` for TS1 blocks and a
  `ts1 blocks=…` summary; `explain` prints the route, base route, float evidence, admitted run
  lanes and the selected TS1 estimate per block.

Scripts that parse the first output line of `compress` keep working.

## Benchmarks and tooling

* result files are `benchmark-0.5.0-<family>.json` (schema 2.1 unchanged); new families
  `float-ablation`, `float`, `float-fastpath`, `float-false-positive`, `float-estimator`;
* Regression V3 reads those families and has a `float` section;
* the A/B baseline is the 0.4.6 source tree (`./ace-ab0.5.0.sh <0.4.6-tree>`);
* `ace-corpus` lists 27 workloads (Corpus V3 + V4) plus the false-positive cases (`fp-*`).
