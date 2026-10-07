# ACE benchmark contract 0.2.1

Official benchmark results live under `examples/results/` and use `<milestone>-<family>.json` names. ACE 0.2.1 produces five primary families plus a regression report.

Every primary JSON document contains `schema_version`, `project`, `milestone`, `base`, `scope`, `benchmark_contract_origin`, `environment`, `configuration` and `workloads`.

## Families

- `compression`: ACE profiles vs LZ4/Zstd/gzip, plus plan distribution and stage timing.
- `entropy`: Huffman vs scalar rANS.
- `planner`: oracle recall/regret, recall by class and isolated stage timings.
- `parallel`: worker-count scaling and deterministic-output assertion.
- `random-access`: full decode, representative block reads, crossing-range reads and overread metrics.
- `regression`: release gates comparing 0.2.1 with checked-in 0.2 baseline files.

## Release gates

The canonical thresholds are stored in `MILESTONE-0.2.1.json`. The regression tool exits non-zero when any mandatory gate fails.
