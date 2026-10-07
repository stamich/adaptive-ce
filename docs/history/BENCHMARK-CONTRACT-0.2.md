# ACE benchmark contract 0.2

Official results are written under `examples/results/` as `<milestone>-<family>.json`.

Every document requires:

- `schema_version`,
- `project`,
- `milestone`,
- `base`,
- `scope`,
- `benchmark_contract_origin`,
- `generated_at_utc_epoch_seconds`,
- `environment`,
- `configuration`,
- `workloads`.

ACE 0.2 defines five families: compression, entropy, planner, parallel, and random-access. Repeated timing records retain raw samples plus median, p95, p99, mean, minimum and maximum. Benchmark timing is observational only and never feeds runtime plan selection.
