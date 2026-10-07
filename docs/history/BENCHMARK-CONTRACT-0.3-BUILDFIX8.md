# ACE 0.3-buildfix8 benchmark contract — schema 1.8

Result families remain compression, entropy, planner, parallel, random-access, streaming, memory and regression.
Official filenames use `0.3-buildfix8-<family>.json`.

## Outcome release gates

- generated recall >= 0.99;
- Top-K recall >= 0.98;
- oracle Top-2 after sampling >= 0.95;
- oracle Top-3 after sampling >= 0.99;
- quality-pool recall >= 0.95;
- regret <= 1024 B/block;
- hot-path full trials == 0;
- BALANCED ratio >= 3.40x;
- DENSE ratio >= 99.5% hardened baseline;
- profile ordering DENSE >= BALANCED >= FAST;
- historical throughput and random-access gates retained for continuity.

## Diagnostic-only calibration

MAE, MAPE, signed bias and p95 absolute error are reported by codec family and data class. They are
not release gates in buildfix8. Hybrid telemetry also reports candidates/block, sampled bytes/block and
maximum analytical-vs-micro-trial disagreement.
