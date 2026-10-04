# Adaptive Compression Engine — Milestone 0.3

ACE 0.3 is the first performance-architecture milestone after the hardened 0.2.1-buildfix1 baseline.  It keeps the adaptive block model, deterministic output and indexed random access, but removes full-block trial compression from the runtime planner.  Candidate plans are estimated analytically, only a small deterministic Top-K set is sample-verified, and the winning plan is fully encoded exactly once.

## Main changes

- Planner V3: analytical `CandidateEstimator`, deterministic Top-K and `SampleVerifier`.
- Planner fast paths for highly incompressible, run-dominated, strongly repetitive and strong-delta blocks.
- New `ace-cost` crate for cost/size estimation, separated from candidate search.
- New `ace-simd` crate isolating runtime AVX2 dispatch behind safe APIs.
- SIMD-assisted zero counting and LZ match comparison with scalar fallback.
- `rANS4x` entropy mode and ACE Format 1.2 (`EntropyCodecId::Rans4x`).
- Reader compatibility with ACE Format 1.0, 1.1 and 1.2.
- New `ace-stream` crate for bounded-memory compression when source size is known.
- Reusable `WorkerScratch` allocation boundary in `ace-runtime`.
- Cold/warm random-access benchmark split.
- New streaming and memory benchmark families.
- Regression baseline is the successful ACE 0.2.1-buildfix1 release.

## Repository layout

```text
ace-milestone0.3/
├── crates/
│   ├── ace-core/         stable IDs, configuration, plans, statistics, errors
│   ├── ace-analysis/     block statistics and incompressibility analysis
│   ├── ace-transforms/   reversible transforms
│   ├── ace-codecs/       RAW, RLE and LZ
│   ├── ace-entropy/      Huffman, scalar rANS and rANS4x
│   ├── ace-dictionary/   dictionary abstractions
│   ├── ace-cost/         NEW: deterministic candidate estimation and CostModel V3
│   ├── ace-planner/      candidate generation, fast paths and sampled Top-K verification
│   ├── ace-format/       ACE 1.0/1.1/1.2 framing
│   ├── ace-index/        serialized random-access index
│   ├── ace-runtime/      Rayon runtime and reusable worker scratch
│   ├── ace-simd/         NEW: safe SIMD runtime dispatch
│   ├── ace-stream/       NEW: bounded-memory stream adapter
│   ├── ace-engine/       end-to-end compressor/decompressor
│   └── ace-cli/          CLI
├── examples/
│   ├── rust-demo/
│   ├── rust-benchmark/
│   ├── random-access-demo/
│   ├── parallel-demo/
│   ├── baselines/0.2.1-buildfix1/
│   ├── data/
│   └── results/
├── demo/
├── docs/
├── fuzz/
├── integrations/
├── tools/
├── TASKS-0.3.md
├── MILESTONE-0.3.json
└── CHANGELOG.md
```

## Build and tests

```bash
cargo build --workspace --release
cargo test --workspace
```

## Demo

```bash
./demo/run-demo-0.3.sh
```

The demo generates a deterministic mixed corpus, shows Planner V3 decisions, performs ordinary and bounded-stream compression, verifies Format 1.2, demonstrates random access and runs the planner/streaming benchmark families.

## Benchmarks

Run all benchmark families and regression gates:

```bash
./benchmark.sh all
```

Or one family:

```bash
./benchmark.sh planner
./benchmark.sh streaming
./benchmark.sh memory
```

Official result files are written to `examples/results/`:

```text
0.3-compression.json
0.3-entropy.json
0.3-planner.json
0.3-parallel.json
0.3-random-access.json
0.3-streaming.json
0.3-memory.json
0.3-regression.json
```

## Planner V3 invariant

The runtime planner does **not** fully encode multiple complete candidates.  It performs:

```text
BlockProfile
    ↓
CandidateGenerator
    ↓
CandidateEstimator (cheap)
    ↓
Top-K
    ↓
Deterministic sample verification
    ↓
Selected plan
    ↓
ONE full-block encode
```

`planner_full_trial_encodes` is therefore expected to remain zero in the compression hot path.

## Format compatibility

- writer: ACE Format 1.2;
- reader: ACE Format 1.0, 1.1 and 1.2;
- 1.2 adds the new rANS4x entropy identifier without restructuring the index/trailer;
- block independence and deterministic parallel output remain mandatory.

## Scope intentionally deferred

ACE 0.3 does not yet implement CDC, trained global dictionaries, cross-block LZ dependencies, ML-based planning, GPU compression, AdaptiveDB semantic hints, GraphNet semantic transforms or JVM-native FFM/Panama bindings.

See `TASKS-0.3.md`, `docs/ARCHITECTURE-0.3.md` and `CHANGELOG.md` for details.
