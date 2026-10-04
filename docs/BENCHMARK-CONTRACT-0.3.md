# ACE 0.3 benchmark contract

Official results live under `examples/results/0.3-buildfix3-<family>.json`.

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

## Planner V3.1 quality fields

Buildfix3 adds `candidate_generation_recall`, `top_k_recall`, `sample_verifier_recall`, `final_selection_recall`, stage-specific denominators and `regret_bytes_per_block_by_class`. Top-K/sample recall excludes blocks handled directly by `PlannerFastPath`, because those blocks never enter the corresponding stage.
