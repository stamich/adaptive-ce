# ACE Planner V3.3 — 0.3-buildfix6

## Problem addressed

`0.3-buildfix5` preserved oracle candidates through generation, Top-K and ranking-only sampling,
but the final scalar cost function could still select a substantially larger plan because encode/decode
CPU terms outweighed compressed-size quality.

Planner V3.3 separates the decision into two explicit policies:

1. **Quality qualification** — keep only candidates close enough to the best blended size.
2. **Cost optimization** — among those quality-safe candidates, choose the lowest deterministic scalar cost.

## Candidate size model

Every `EstimatedCandidate` now retains three size views:

- `analytical_size_bytes`: full-block statistical estimate before sample encoding;
- `sampled_size_bytes`: optional projection derived from deterministic sample encoding;
- `blended_size_bytes`: confidence-weighted size used by the quality envelope.

`PlanCost.predicted_size_bytes` mirrors the blended value for compatibility with CostModelV3.

The original analytical estimate is never overwritten by stage-one or stage-two sampling.

## QualityEnvelope

The envelope uses integer PPM slack around the smallest blended candidate:

```text
FAST      +250000 ppm = +25.0%
BALANCED   +15000 ppm = +1.5%
DENSE       +3000 ppm = +0.3%
```

The large FAST slack preserves speed-first semantics. BALANCED limits ratio loss while retaining
meaningful CPU freedom. DENSE keeps final selection almost size-optimal.

## Final selection

```text
post-sampling candidates
        ↓
best blended size
        ↓
profile quality limit
        ↓
quality-qualified candidates
        ↓
CostModelV3 deterministic order
        ↓
stable plan-key tie-break
        ↓
winner
```

No full-block trial candidate encoding is introduced.

## Diagnostics

Planner telemetry exposes:

- `quality_qualified_candidates`;
- `best_blended_size_bytes`;
- `quality_limit_bytes`;
- `selected_blended_size_bytes`;
- `selected_size_rank`;
- `selected_cost_rank`.

The planner benchmark also records oracle ranks at analytical, stage-one, post-sampling and final
quality-pool stages. This distinguishes estimator error, sample-ranking error and final-policy error.

## Determinism

QualityEnvelope uses only integer arithmetic, candidate statistics and static profile configuration.
It does not depend on elapsed time, thread scheduling, hash-map iteration order or previous blocks.

## Quality-pool recall

Because the envelope itself can exclude an oracle candidate when blended-size estimation is wrong,
the benchmark records `quality_pool_recall` separately from Top-K and post-sampling oracle rank.
This makes quality-envelope miscalibration observable rather than hiding it behind final regret.
