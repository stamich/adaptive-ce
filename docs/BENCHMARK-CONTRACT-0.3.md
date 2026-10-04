# ACE 0.3 benchmark contract

Official results live under `examples/results/0.3-buildfix5-<family>.json`.

Families:

- `compression` — FAST/BALANCED/DENSE plus LZ4/Zstd/gzip baselines;
- `entropy` — Huffman/scalar rANS/rANS4x;
- `planner` — recall, regret, stage timing, fast-path and Top-K work;
- `parallel` — 1/2/4/6/8/12 worker determinism and scaling;
- `random-access` — full decode, decoder open, block reads, cold/warm ranges;
- `streaming` — bounded reader/writer compression;
- `memory` — reusable scratch and planner full-trial elimination;
- `regression` — gates against 0.2.1-buildfix1.

The benchmark format remains version-independent: `milestone`, `base`, `scope` and `benchmark_contract_origin` must be mutually consistent.

## Planner V3.2 quality fields

Buildfix4 keeps `candidate_generation_recall`, `top_k_recall`, `final_selection_recall` and `regret_bytes_per_block_by_class`, and adds `sample_survival_recall`, oracle mean rank before/after sampling, plus Top-1/Top-2/Top-3 oracle rates after sampling. Top-K/sample metrics exclude blocks handled directly by `PlannerFastPath`, because those blocks never enter the corresponding stage. Stage-two sampling is ranking-only and therefore cannot discard a stage-one survivor.
