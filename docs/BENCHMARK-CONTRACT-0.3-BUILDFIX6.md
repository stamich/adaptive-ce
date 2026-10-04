# ACE 0.3-buildfix6 benchmark contract

## Families

```text
compression
entropy
planner
parallel
random-access
streaming
memory
regression
```

Official files use the `0.3-buildfix6-` prefix.

## Planner metrics

Required quality diagnostics include:

```text
candidate_generation_recall
top_k_recall
oracle_mean_rank_analytical
oracle_mean_rank_after_stage1
oracle_mean_rank_after_sampling
oracle_mean_rank_final_quality_pool
oracle_top1_rate_after_sampling
oracle_top2_rate_after_sampling
oracle_top3_rate_after_sampling
quality_pool_recall
normalized_regret_bytes_per_block
regret_bytes_per_block_by_class
quality_qualified_candidates_per_block
selected_size_rank_mean
selected_cost_rank_mean
predicted_size_regret_bytes_per_block
full_trial_encodes_per_block
```

`sample_survival_recall` is intentionally not a release gate because ranking-only verification keeps
stage-one candidates alive by construction.

## Release gates

```text
generated recall           >= 0.99
Top-K recall               >= 0.98
oracle Top-2 after sample  >= 0.95
oracle Top-3 after sample  >= 0.99
quality-pool recall        >= 0.95
actual regret              <= 1024 B/block
full trials                == 0
BALANCED ratio             >= 3.40x
DENSE ratio                >= 99.5% hardened baseline
FAST throughput            >= 2.0x baseline
BALANCED throughput        >= 4.0x baseline
DENSE throughput           >= 3.5x baseline
warm 64 KiB latency        <= 115% baseline
```
