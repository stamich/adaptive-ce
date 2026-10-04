# Adaptive Compression Engine — ACE 0.3-buildfix6

ACE 0.3-buildfix6 is a focused Planner V3.3 quality-selection buildfix on top of 0.3-buildfix5.

The previous build preserved oracle candidates through generation, Top-K and sample verification,
but final scalar cost ranking could still trade away too much compression ratio for CPU cost.
Buildfix6 introduces an explicit **QualityEnvelope** before final scalar-cost selection.

## Core planner flow

```text
Analyzer
  ↓
CandidateGenerator
  ↓
analytical estimate
  ↓
adaptive Top-K + semantic anchors
  ↓
stage-1 sample refinement
  ↓
stage-2 rank-only refinement
  ↓
QualityEnvelope (size constraint)
  ↓
CostModelV3 chooses cheapest quality-safe plan
  ↓
one full encode
```

## What changed

- new `ace-cost::quality` module with `QualityEnvelope`;
- `EstimatedCandidate` keeps:
  - `analytical_size_bytes`,
  - `sampled_size_bytes`,
  - `blended_size_bytes`;
- sample stages no longer overwrite the original analytical prediction;
- profile quality envelopes:
  - FAST: +25.0% over best blended size,
  - BALANCED: +1.5%,
  - DENSE: +0.3%;
- final CostModel selection operates only on quality-qualified candidates;
- deterministic tie-breaking is unchanged;
- planner telemetry now includes:
  - quality-qualified candidate count,
  - best blended size,
  - quality limit,
  - selected blended size,
  - selected size rank,
  - selected cost rank;
- planner benchmark now reports oracle rank at analytical, stage-1, post-sampling and final quality-pool stages;
- `sample_survival_recall` is no longer a release gate because ranking-only sampling preserves candidates by construction;
- new release gates use oracle Top-2/Top-3 after sampling and explicit BALANCED ratio quality.

## Compatibility

- Engine milestone: `0.3-buildfix6`
- Workspace version: `0.3.6`
- Writer format: ACE 1.2
- Reader formats: ACE 1.0, 1.1 and 1.2
- No wire-format change from 0.3-buildfix5

## Build and test

```bash
cargo build --workspace --release
cargo test --workspace
```

## Demo

```bash
./demo/run-demo-0.3-buildfix6.sh
```

## Benchmarks

```bash
./benchmark.sh all
```

Official result files:

```text
examples/results/0.3-buildfix6-compression.json
examples/results/0.3-buildfix6-entropy.json
examples/results/0.3-buildfix6-planner.json
examples/results/0.3-buildfix6-parallel.json
examples/results/0.3-buildfix6-random-access.json
examples/results/0.3-buildfix6-streaming.json
examples/results/0.3-buildfix6-memory.json
examples/results/0.3-buildfix6-regression.json
```

## Release goals

Planner:

```text
generated recall           >= 0.99
Top-K recall               >= 0.98
oracle Top-2 after sample  >= 0.95
oracle Top-3 after sample  >= 0.99
quality-pool recall        >= 0.95
regret                     <= 1024 B/block
full trial encodes         == 0
```

Compression:

```text
BALANCED ratio             >= 3.40x
DENSE ratio                >= 99.5% of 0.2.1-buildfix1
Dense >= Balanced >= Fast
FAST throughput            >= 2.0x baseline
BALANCED throughput        >= 4.0x baseline
DENSE throughput           >= 3.5x baseline
```

Random access:

```text
warm 64 KiB latency        <= 115% baseline
```

See `TASKS-0.3-buildfix6.md`, `MILESTONE-0.3-buildfix6.json`,
`docs/PLANNER-0.3-BUILDFIX6.md`, and `docs/BENCHMARK-CONTRACT-0.3-BUILDFIX6.md`.
