# ACE 0.3-buildfix3 — Planner V3.1

Planner V3.1 keeps the no-full-trial performance architecture introduced in 0.3 but corrects its quality regression.

## Pipeline

```text
BlockProfile
   ↓
CandidateGenerator
   ↓
CandidateEstimator
   ↓
adaptive Top-K + semantic-family anchors
   ↓
stage 1 codec-specific deterministic samples
   ↓
uncertainty widening
   ↓
stage 2 larger deterministic samples
   ↓
CostModelV3
   ↓
selected PhysicalCompressionPlan
   ↓
one full encode performed by the engine
```

## Why buildfix3 is necessary

0.3-buildfix2 achieved high throughput and zero full-block planner trials, but benchmark quality regressed: candidate generation recall remained 1.0 while regret rose to about 35.3 KiB per block and Dense ratio fell from about 3.48x to 2.37x. This proves that the oracle candidate was generated but lost or misranked after generation.

## Metadata projection fix

A sample contains entropy metadata once. The old verifier scaled both payload and metadata from sample size to block size. Buildfix3 scales only payload and adds entropy metadata once. This is especially important for rANS and rANS4x, whose model metadata is larger than Huffman's.

## LZ sampling

LZ candidates receive larger stratified windows than RAW/RLE candidates. Windows are encoded independently rather than concatenated; concatenation would create artificial cross-window matches that do not exist in the source block.

## Quality telemetry

The benchmark records:

- `candidate_generation_recall`
- `top_k_recall`
- `sample_verifier_recall`
- `final_selection_recall`
- regret per data class
- per-block stage survival flags

This separates candidate-generation quality from pruning and final cost-model decisions.
