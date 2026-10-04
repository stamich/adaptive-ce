# ACE 0.3-buildfix5 benchmark results

`./benchmark.sh all` writes:

- `0.3-buildfix5-compression.json`
- `0.3-buildfix5-entropy.json`
- `0.3-buildfix5-planner.json`
- `0.3-buildfix5-parallel.json`
- `0.3-buildfix5-random-access.json`
- `0.3-buildfix5-streaming.json`
- `0.3-buildfix5-memory.json`
- `0.3-buildfix5-regression.json`

Planner V3.2 additionally records oracle rank before/after sampling and Top-1/Top-2/Top-3 survival statistics.

## 0.3-buildfix6

`0.3-buildfix6` adds Planner V3.3 quality-envelope diagnostics and emits:

```text
0.3-buildfix6-compression.json
0.3-buildfix6-entropy.json
0.3-buildfix6-planner.json
0.3-buildfix6-parallel.json
0.3-buildfix6-random-access.json
0.3-buildfix6-streaming.json
0.3-buildfix6-memory.json
0.3-buildfix6-regression.json
```

The planner family records analytical/sample/final oracle rank, quality-pool recall,
selected size/cost rank and predicted-size regret.

## 0.3-buildfix8

Schema 1.8 results use `0.3-buildfix8-*.json`. Planner output includes hybrid LZ budgets/disagreement and analytical calibration diagnostics.
