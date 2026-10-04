# Planner V3

Planner V3 is designed around estimation-first search.

1. Analyzer computes a block profile once.
2. High-confidence fast paths may resolve obviously random/run/repetitive blocks.
3. Candidate generator creates stable decoder-semantic candidates.
4. `DefaultCandidateEstimator` predicts size, CPU, memory and confidence without executing codecs.
5. Candidates are sorted with stable tie-breaking.
6. Only profile-dependent Top-K candidates are encoded on deterministic samples.
7. Sample observations refine predicted size.
8. `CostModelV3` selects the winner.
9. Engine fully encodes the selected plan exactly once.

Default budgets:

| profile | sample bytes | samples | max Top-K |
|---|---:|---:|---:|
| FAST | 4 KiB | 1 | 1 |
| BALANCED | 4 KiB | 2 | 2 |
| DENSE | 8 KiB | 3 | 3 |

Confidence can reduce K further. No random sampling is used.
