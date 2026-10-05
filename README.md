# Adaptive Compression Engine (ACE) 0.3.1

ACE 0.3.1 is the hardened stabilization release of the ACE 0.3 line. It freezes the successful
Planner V3.6 architecture from `0.3-buildfix9-compilefix` and strengthens correctness,
determinism, malformed-input handling, fuzzability and benchmark reproducibility.

## Frozen compression architecture

```text
Block input
  -> Block Analyzer / FAST Analyzer Lite
  -> Candidate Generator
  -> analytical estimator
  -> adaptive Top-K
  -> bounded Hybrid LZ micro-trials
  -> deterministic sample verification
  -> QualityEnvelope
  -> deterministic CostModel V3
  -> one production encode
  -> Format 1.2 + AIDX + ACET
```

ACE 0.3.1 does **not** redesign Planner V3.6, Hybrid LZ, QualityEnvelope, entropy codecs or the
wire format.

## Format compatibility

- writer: Format 1.2
- reader: Format 1.0 / 1.1 / 1.2
- random access: AIDX index + ACET trailer
- corruption detection: header/index/trailer checks and block CRC32C
- decoder resource limits: `DecodeLimits`

See `docs/FORMAT-COMPATIBILITY.md`.

## 0.3.1 hardening

The release adds:
- property-based arbitrary-byte roundtrip tests;
- property-based indexed-range equality tests;
- byte-identical determinism matrix across profiles, block sizes and worker counts;
- malformed index/trailer and resource-limit tests;
- streaming premature-EOF/extra-data/resource-limit tests;
- standalone `cargo-fuzz` project;
- deterministic Corpus V2;
- block-size benchmark matrix;
- extended random-access benchmark matrix;
- benchmark variance diagnostics;
- p99 planner regret;
- golden buildfix9 performance regression baseline.

## Corpus V2

Generate deterministic local corpora:

```bash
python3 tools/generate_hardening_corpus.py \
  --output-dir examples/corpus/generated \
  --sizes-mib 1,16,64
```

Classes:
- zeros
- low-cardinality
- runs
- numeric-u32
- delta-series
- structured-json
- random
- mixed

No external downloads are required.

## Build and test

```bash
cargo build --workspace --release
cargo test --workspace
```

For fuzzing:

```bash
cargo install cargo-fuzz
cargo fuzz run container_decode
cargo fuzz run index_parse
cargo fuzz run trailer_parse
cargo fuzz run range_open
```

## Demo

```bash
./demo/run-demo-0.3.1.sh
```

The demo builds/tests the workspace, generates Corpus V2, demonstrates thread-count determinism,
verifies a Format 1.2 archive, confirms corruption rejection and runs focused hardening benchmarks.

## Benchmarks

Run the complete release contract:

```bash
./benchmark.sh all
```

Existing families:
- compression
- entropy
- planner
- parallel
- random-access
- streaming
- memory

0.3.1 hardening families:
- corpus
- block-matrix
- random-access-extended
- stability

Results are written as `examples/results/0.3.1-<family>.json`.

Benchmark schema remains 1.9. Timing objects now also report `stddev_ns`, `cv_percent` and
`unstable_measurement`.

See `docs/BENCHMARKS-0.3.1.md`.

## Release gates

Quality:
- generated recall >= 0.99
- Top-K recall >= 0.98
- mean regret <= 256 B/block
- p95 regret <= 1024 B
- p99 regret <= 4096 B
- BALANCED ratio >= 3.45x
- DENSE ratio >= 3.45x
- full trial encodes/block == 0

Performance/stability:
- FAST/BALANCED/DENSE throughput >= 95% of the buildfix9 golden baseline
- warm 64 KiB latency <= 107.5% of buildfix9 golden latency
- repeated output must be byte-identical

Exact oracle Top-N rank remains diagnostic-only because earlier 0.3 experiments showed actual
regret is the more meaningful product-quality metric.

## Important documentation

- `TASKS-0.3.1.md`
- `docs/BASELINE-0.3.1.md`
- `docs/HARDENING-0.3.1.md`
- `docs/BENCHMARKS-0.3.1.md`
- `docs/FORMAT-COMPATIBILITY.md`
- `docs/FUZZING-0.3.1.md`
- `CHANGELOG.md`
- `ROADMAP.md`

## Direction after 0.3.1

ACE 0.3.1 closes the 0.3 planner-hardening line. New compression capabilities, new codecs,
dictionary learning, format changes or learned planning belong to ACE 0.4 rather than another
0.3 buildfix.
