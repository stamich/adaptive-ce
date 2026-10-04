# Adaptive Compression Engine — Milestone 0.2.1-buildfix1

ACE 0.2.1-buildfix1 is a targeted hardening release built from 0.2.1. It does not change ACE Format 1.1. It fixes the failed 0.2.1 release gates: planner candidate recall, FAST throughput/profile separation and random-access benchmark semantics, while removing the reported compiler warnings.

## What changed

- profile-specific candidate generators: narrow FAST, recall-oriented BALANCED/DENSE;
- corrected deterministic Cost Model V2.1 normalization;
- no `LzMode::Balanced` or mandatory rANS in FAST;
- full offline-oracle family coverage in BALANCED/DENSE;
- `read_range_with_metrics()` reuses one block-index intersection;
- random-access benchmark separates `decoder_open` from already-open range latency;
- regression output compares against both ACE 0.2 absolute gates and observed ACE 0.2.1 values;
- no Format 1.1 wire change.

## Workspace

```text
crates/
  ace-core/        stable model/config/errors/plan types
  ace-analysis/    block statistics
  ace-transforms/  reversible DeltaByte preprocessing
  ace-codecs/      RAW/RLE/LZ
  ace-entropy/     canonical Huffman + scalar rANS
  ace-dictionary/  dictionary abstractions
  ace-planner/     Candidate Generator + deterministic Cost Model
  ace-format/      ACE 1.0/1.1 framing
  ace-index/       AIDX/ACET index reader/writer
  ace-runtime/     bounded deterministic parallel runtime
  ace-engine/      high-level compression/decompression/random access
  ace-cli/         CLI
examples/
  rust-demo/
  rust-benchmark/
  random-access-demo/
  parallel-demo/
  baselines/0.2/
  baselines/0.2.1/
  results/
demo/
tools/
docs/
fuzz/
```

## Build and test

```bash
cargo build --workspace --release
cargo test --workspace
```

## Demo

```bash
./demo/run-demo-0.2.1-buildfix1.sh
```

The demo explains FAST/BALANCED/DENSE planning, performs deterministic parallel compression, verifies Format 1.1, reads an indexed 64-KiB range and runs planner/random-access benchmarks.

## Benchmarks

```bash
./benchmark.sh all
```

Generated files:

```text
examples/results/0.2.1-buildfix1-compression.json
examples/results/0.2.1-buildfix1-entropy.json
examples/results/0.2.1-buildfix1-planner.json
examples/results/0.2.1-buildfix1-parallel.json
examples/results/0.2.1-buildfix1-random-access.json
examples/results/0.2.1-buildfix1-regression.json
```

The regression report keeps the original ACE 0.2 gates as the absolute release contract and additionally reports improvement relative to the observed failed ACE 0.2.1 candidate.

## Release gates

- planner candidate recall >= 0.95;
- planner regret <= 8192 bytes/block;
- DENSE ratio >= 99% of ACE 0.2;
- FAST throughput >= 95% of ACE 0.2;
- four-thread efficiency >= 0.80;
- already-open 64-KiB range median latency <= 110% of ACE 0.2;
- bit-identical output across the worker-count matrix.

## Compatibility

Engine release: `0.2.1-buildfix1`  
Writer format: `1.1`  
Reader formats: `1.0`, `1.1`

See `TASKS-0.2.1-buildfix1.md` for implementation order and `CHANGELOG.md` for the exact buildfix delta.
