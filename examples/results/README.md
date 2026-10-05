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

## 0.3-buildfix9
Schema 1.9 files use `0.3-buildfix9-*.json`. Hard gates are product-quality-first; exact oracle-rank metrics remain diagnostics.

## 0.3-buildfix9-compilefix

Result files use the `0.3-buildfix9-compilefix-*.json` prefix. Schema version remains 1.9.
The compilefix changes only how JSON is built in Rust source; result field names and meanings
are identical to 0.3-buildfix9.

## 0.3.1

ACE 0.3.1 keeps benchmark schema 1.9 and writes `0.3.1-<family>.json`.

New hardening families:
- `corpus`
- `block-matrix`
- `random-access-extended`
- `stability`

Timing objects add `stddev_ns`, `cv_percent` and `unstable_measurement`.
Planner output adds `p99_regret_bytes_per_block`.
