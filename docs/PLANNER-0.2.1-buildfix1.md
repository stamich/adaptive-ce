# Planner hardening in ACE 0.2.1-buildfix1

The failed 0.2.1 benchmark exposed two separate problems: candidate recall remained below the 0.95 release target while FAST throughput regressed because the score's raw byte-size term dominated already-normalized CPU terms.

Buildfix1 therefore separates candidate breadth by profile and normalizes every score dimension before weighting it. FAST is intentionally narrow and never evaluates balanced LZ. BALANCED and DENSE cover the complete offline-oracle decoder-semantic family, making recall failure diagnosable independently from cost-model selection quality.

The planner remains fully deterministic: no clock readings, randomized iteration order, worker timing or machine load influences the selected physical plan.
