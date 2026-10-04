# ACE 0.3-buildfix7 benchmark contract

Schema version: **1.7**.

New planner diagnostics:
- `estimator_calibration` by codec family,
- `estimator_calibration_by_data_class`,
- selected prediction error bytes/percent,
- repetition score,
- mean/p95 match length,
- match coverage,
- long-match ratio.

Estimator calibration is diagnostic-only and excluded from timed planner evaluation.

Release gates include:
```text
generated recall             >= 0.99
Top-K recall                 >= 0.98
oracle Top-2 after sample    >= 0.95
oracle Top-3 after sample    >= 0.99
quality-pool recall          >= 0.95
regret                       <= 1024 B/block
runtime full trials          == 0
LZ FAST analytical MAPE      <= 0.35
LZ BALANCED analytical MAPE  <= 0.35
BALANCED ratio               >= 3.40x
DENSE ratio                  >= 99.5% hardened baseline
```
