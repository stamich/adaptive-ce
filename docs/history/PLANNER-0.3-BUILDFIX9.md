# Planner V3.6 — ACE 0.3-buildfix9

Planner V3.6 is a hardening pass over buildfix8. Search quality and final quality policy are intentionally unchanged. The milestone changes only how much verification work is spent on each block.

## Invariants

- candidate generation stays unchanged from buildfix8;
- analytical Top-K stays quality preserving;
- Hybrid LZ remains a real production-codec micro-trial;
- stage two remains ranking-only;
- QualityEnvelope remains the final quality constraint;
- full candidate trial encodes remain zero.

## Work budgeting

`PlannerDataClass` is derived exclusively from `BlockProfile`. `PlanningBudget` controls how many LZ candidates may use Hybrid LZ. Zero-heavy and incompressible blocks receive no LZ micro-trial budget; numeric blocks retain the largest budget.

## Confidence

`EstimateConfidence` compares analytical and sampled size: High <5%, Medium 5-15%, Low >15%. High-confidence LZ candidates skip stage two.

## Quality metrics

Actual encoded regret and p95 regret are hard release gates. Exact oracle rank remains diagnostic because buildfix8 showed a non-oracle plan can be practically equivalent in output size.
