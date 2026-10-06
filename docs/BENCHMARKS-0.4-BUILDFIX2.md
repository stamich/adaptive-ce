# ACE 0.4-buildfix2 benchmark contract

Schema: **2.0**

## New families

### planner-route
Measures route-classifier cost and reports:
- selected route;
- route reason;
- prefilter confidence;
- likely/strong Numeric flags;
- width hint;
- route timing.

Workloads:
- u32 counter;
- u64 timestamps;
- variable deltas;
- mixed;
- random.

### numeric-fastpath
Measures end-to-end strong NumericFast behavior and reports:
- compression/decompression throughput;
- ratio;
- Numeric block count;
- plan distribution;
- analysis/planning/encoding stage time;
- planner fast-path count;
- estimated/sampled/full-trial candidate counts.

## Direct baseline

`examples/baselines/0.4-buildfix1/` contains the benchmark files supplied after buildfix1. They are
retained because they document the exact pre-hardening behavior.

## Release targets

Quality:
- generated recall >= 0.99
- Top-K recall >= 0.98
- mean regret <= 16 B/block
- p95 <= 64 B
- p99 <= 256 B
- zero full trial encodes
- BALANCED/DENSE ratio >= 3.70x

Performance:
- FAST >= 160 MB/s
- BALANCED >= 85 MB/s
- DENSE >= 50 MB/s
- warm 64K <= 100 us
- u32-counter Planner V4.3 >= 120 MB/s

Numeric ratio/selection gates from 0.4 remain active.
