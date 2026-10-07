# ACE 0.3 architecture

## Compression path

```text
input block
  ↓
BlockAnalyzer
  ↓
BlockProfile
  ↓
PlannerFastPath? ── yes ─→ selected plan
  │ no
  ↓
CandidateGenerator
  ↓
CandidateEstimator (ace-cost)
  ↓
ranked estimated candidates
  ↓
Top-K deterministic sample verification
  ↓
CostModelV3
  ↓
selected plan
  ↓
ONE full encode
  ↓
ordered writer → index → trailer
```

The fundamental 0.3 rule is that compression planning must not require full encoding of every candidate. Exhaustive full candidate execution remains available only to benchmark/oracle diagnostics.

## Crate responsibilities

- `ace-core`: stable shared types, IDs, configuration and telemetry.
- `ace-analysis`: statistics only; it must not choose codecs.
- `ace-cost`: cheap deterministic predictions and scalar scoring.
- `ace-planner`: candidate search, fast paths and sample verification.
- `ace-codecs` / `ace-entropy` / `ace-transforms`: reversible byte execution.
- `ace-simd`: target-specific unsafe implementations behind safe dispatch.
- `ace-runtime`: parallel scheduling and reusable worker memory boundary.
- `ace-stream`: bounded source-reader pipeline.
- `ace-engine`: end-to-end orchestration and deterministic ordered format assembly.

## Determinism

No wall-clock measurement, thread scheduling result, random number or unordered map iteration is allowed to change a physical plan. Sampling offsets are deterministic. Parallel workers may finish out of order, but blocks are serialized by `block_id`.

## Planner quality budget

ACE 0.3 accepts a slight reduction from the 100% candidate recall achieved by buildfix1 in exchange for much lower planner cost. The release target is recall >= 98% and regret <= 1 KiB per 256 KiB block.
