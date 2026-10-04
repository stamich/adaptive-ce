# Adaptive Compression Engine — ACE 0.3-buildfix8

ACE 0.3-buildfix8 is a **selective rollback** release. Its source baseline is 0.3-buildfix6, not
0.3-buildfix7. Buildfix7 was useful diagnostically, but its LZ Estimator V2 and expanded full-block
match analysis reduced Top-K recall, ratio and throughput.

## Design rule

```text
buildfix6 core
   + buildfix7 calibration telemetry
   + buildfix7 zero-heavy quality guard
   + bounded hybrid LZ micro-trials
   - LZ Estimator V2
   - expensive match-analysis V2
```

## Planner V3.5

```text
BlockAnalyzer (buildfix6)
      ↓
CandidateGenerator
      ↓
DefaultCandidateEstimator (buildfix6)
      ↓
adaptive Top-K + semantic anchors
      ↓
LZ: bounded production-codec micro-trial
RAW/RLE: existing deterministic sample verifier
      ↓
rank-only stage 2
      ↓
QualityEnvelope
      ↓
CostModelV3 inside quality-safe pool
      ↓
one final full encode
```

The hybrid LZ estimator never changes candidate generation or analytical Top-K. This intentionally
preserves the buildfix6 search behavior that achieved `top_k_recall = 1.0`.

## Hybrid LZ budgets

Stage 1:

```text
FAST       1 ×  8 KiB
BALANCED   3 ×  8 KiB
DENSE      3 × 12 KiB
```

Stage 2:

```text
FAST       1 × 16 KiB
BALANCED   2 × 16 KiB
DENSE      2 × 24 KiB
```

Every window is encoded independently with the production transform/codec/entropy pipeline.
Reset-window pessimism is bounded by blending the micro-trial with the original buildfix6
analytical estimate instead of replacing it.

## Fast-path policy

- FAST keeps the zero-heavy `RLE/None` shortcut.
- BALANCED and DENSE send zero-heavy blocks through the general planner so entropy-coded RAW/RLE
  alternatives remain eligible.

## Benchmark methodology

Benchmark schema 1.8 retains buildfix7 calibration diagnostics:

- analytical MAE;
- analytical MAPE;
- signed bias;
- p95 absolute error;
- per codec family;
- per data class.

These are **diagnostics, not release gates**. Release decisions remain outcome-based: recall, regret,
compression ratio, throughput and random-access latency.

## Compatibility

- milestone: `0.3-buildfix8`
- workspace: `0.3.8`
- writer format: ACE 1.2
- reader formats: ACE 1.0 / 1.1 / 1.2
- no wire-format change

## Validation

```bash
cargo build --workspace --release
cargo test --workspace
./demo/run-demo-0.3-buildfix8.sh
./benchmark.sh all
```
