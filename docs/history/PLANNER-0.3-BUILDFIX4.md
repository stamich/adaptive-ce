# ACE 0.3-buildfix4 — Planner V3.2

## Problem

0.3-buildfix3 restored most compression quality, but its stage-two sample verifier still reduced oracle survival to 0.50 even though generated recall and Top-K recall were both 1.0. This showed that candidate generation and analytical pruning were no longer the problem; stage-two elimination was.

## V3.2 principle

Sampling is advisory evidence, not an execution oracle.

```text
analytical candidates
      ↓
quality-preserving stage-one pool
      ↓
stage-one sample refinement
      ↓
ranked pool
      ↓
selected stage-two budget ── sample again
      ↓
merge refined candidates + untouched survivors
      ↓
final deterministic ranking
```

No stage-one survivor is removed merely because it was outside the stage-two budget.

## Confidence-weighted blending

Sample-projected bytes and analytical full-block bytes are blended deterministically. High-confidence analytical estimates dominate. LZ receives the highest analytical weight because short windows cannot accurately reproduce long-range matches. DENSE also keeps more analytical authority than BALANCED.

## Quality telemetry

Planner benchmark reports:

- generated recall;
- Top-K recall;
- sample-survival recall;
- final-selection recall;
- oracle rank before sampling;
- oracle rank after sampling;
- oracle Top-1 / Top-2 / Top-3 rates after sampling;
- regret by data class;
- full trial encodes per block.

## Determinism

All ranking, blending and tie-breaking depend only on input, configuration and deterministic statistics. No wall-clock timing, random sampling or thread scheduling contributes to the selected plan.
