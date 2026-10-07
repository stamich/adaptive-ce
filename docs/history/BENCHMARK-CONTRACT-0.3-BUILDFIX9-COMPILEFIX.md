# ACE 0.3-buildfix9-compilefix benchmark contract

The logical benchmark contract is identical to 0.3-buildfix9 schema 1.9.
Only JSON construction was refactored from monolithic `json!` macros to incremental builders.
No field names, gate semantics or workload definitions changed.

Official result families remain compression, entropy, planner, parallel, random-access, streaming, memory and regression. Files use the `0.3-buildfix9-` prefix.

Hard gates measure generated recall, Top-K recall, mean/p95 actual regret, BALANCED/DENSE ratio, tolerant Dense/Balanced ordering, zero full trials, absolute profile throughput and warm 64 KiB latency.

Oracle Top-2, Top-3, quality-pool recall, final-selection recall, estimator MAPE and Hybrid-LZ sample fraction are diagnostics only.

New schema 1.9 fields include `p95_regret_bytes_per_block`, Hybrid-LZ stage/skip counts, `hybrid_lz_sample_fraction`, `fast_analysis_per_file`, `speedup_vs_1t` and `parallel_efficiency`.
