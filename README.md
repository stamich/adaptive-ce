# Adaptive Compression Engine — ACE 0.3-buildfix9-compilefix

This compilefix is based on ACE 0.3-buildfix9 and changes benchmark JSON construction only.

The buildfix9 planner benchmark accumulated enough fields for one large `serde_json::json!({...})`
literal to exceed Rust's macro recursion limit. The compilefix removes all object-shaped `json!`
macros from the Rust benchmark crate and constructs JSON incrementally from small semantic
sections with `JsonObjectBuilder`.

## Compatibility

- Planner V3.6 behavior: unchanged
- Hybrid LZ budgets/confidence: unchanged
- QualityEnvelope: unchanged
- benchmark schema: 1.9 unchanged
- workspace version: 0.3.9
- ACE writer format: 1.2
- ACE reader formats: 1.0 / 1.1 / 1.2

## Validation

```bash
cargo build --workspace --release
cargo test --workspace
./demo/run-demo-0.3-buildfix9-compilefix.sh
./benchmark.sh all
```

See `docs/JSON-SERIALIZATION-0.3-BUILDFIX9-COMPILEFIX.md` and
`TASKS-0.3-buildfix9-compilefix.md`.

---

# Adaptive Compression Engine — ACE 0.3-buildfix9

ACE 0.3-buildfix9 is the hardening/performance milestone after the successful quality recovery in buildfix8. It deliberately **does not redesign Planner V3**. The milestone preserves Hybrid LZ, analytical Top-K, QualityEnvelope and Format 1.2, while removing work that buildfix8 showed was no longer necessary.

## Goals

- preserve buildfix8 quality: ~3.47x BALANCED/DENSE ratio and very low regret;
- improve FAST throughput with a profile-aware analyzer budget;
- reduce unnecessary Hybrid-LZ micro-trials without losing numeric/structured quality;
- normalize release gates around product quality rather than exact oracle ranking;
- shave random-access overhead without changing the wire format or public semantics.

## Planner V3.6

```text
AnalysisLevel::for_profile
        ↓
CandidateGenerator
        ↓
analytical Top-K + semantic anchors
        ↓
PlannerDataClass + PlanningBudget
        ↓
Hybrid LZ stage 1 only where useful
        ↓
EstimateConfidence
        ↓
adaptive stage 2
        ↓
QualityEnvelope
        ↓
CostModelV3
        ↓
one production encode
```

## FAST Analyzer Lite

FAST computes every feature consumed by candidate generation and CostModel, but skips sampled first-order entropy (`entropy_h1`). `entropy_h1` was diagnostic-only and required a 65,536-entry transition table per block. BALANCED and DENSE retain the full analyzer.

## Adaptive Hybrid LZ budget

Planner V3.6 classifies the already-computed profile into `ZeroHeavy`, `Incompressible`, `Numeric` or `Structured`. This classification changes only **work budget**, never decoder semantics or candidate availability.

- zero-heavy / incompressible: no Hybrid-LZ micro-trial;
- numeric: wide LZ budget retained;
- structured: moderate LZ budget;
- FAST: at most one stage-1 LZ micro-trial;
- high analytical/sample agreement skips stage 2.

## Random access

`BlockIndex::intersecting_indices()` returns a contiguous index range without allocating. `AceIndexedDecoder` uses it to avoid allocating/cloning a temporary vector of block index entries for each range read.

## Benchmark schema 1.9

New metrics include:

- `p95_regret_bytes_per_block`;
- `hybrid_lz_stage1_candidates_per_block`;
- `hybrid_lz_stage2_candidates_per_block`;
- `hybrid_lz_skipped_candidates_per_block`;
- `hybrid_lz_high_confidence_skips_per_block`;
- `hybrid_lz_sample_fraction`;
- `fast_analysis_per_file`;
- `speedup_vs_1t`;
- `parallel_efficiency`.

Oracle Top-2/Top-3, quality-pool recall and final-selection recall remain in JSON as **diagnostics**, not hard release gates.

## Release gates

```text
QUALITY
  generated recall            >= 0.99
  Top-K recall                >= 0.98
  regret                      <= 1024 B/block
  p95 regret                  <= 4096 B/block
  BALANCED ratio              >= 3.40x
  DENSE ratio                 >= 99.5% hardened baseline
  DENSE                       >= BALANCED * 0.995

CORRECTNESS
  full trial encodes/block    == 0
  deterministic output        true

PERFORMANCE
  FAST                        >= 135 MB/s
  BALANCED                    >= 65 MB/s
  DENSE                       >= 42 MB/s
  warm 64 KiB                 <= 76 us
```

## Build

```bash
cargo build --workspace --release
cargo test --workspace
```

## Demo

```bash
./demo/run-demo-0.3-buildfix9.sh
```

## Benchmarks

```bash
./benchmark.sh all
```

Format compatibility is unchanged: writer 1.2, readers 1.0/1.1/1.2.
