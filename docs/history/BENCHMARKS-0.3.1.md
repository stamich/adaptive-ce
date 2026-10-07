# ACE 0.3.1 benchmark contract

Schema version remains **1.9**. ACE 0.3.1 adds optional metrics and benchmark families without
changing the top-level document layout.

## Existing families

- compression
- entropy
- planner
- parallel
- random-access
- streaming
- memory

## New hardening families

### corpus
Runs BALANCED over deterministic Corpus V2:
zeros, low-cardinality, long runs, numeric-u32, delta-series, structured JSON, random and mixed.

### block-matrix
Runs BALANCED over 64 KiB, 128 KiB, 256 KiB, 512 KiB and 1 MiB block sizes.

### random-access-extended
Measures aligned and unaligned warm reads for 4 KiB, 16 KiB, 64 KiB, 256 KiB and 1 MiB ranges.

### stability
Repeats the hardened BALANCED workload and checks bit-identical serialized output.

## New timing diagnostics

Every common timing object now contains:
- `stddev_ns`
- `cv_percent`
- `unstable_measurement`

`unstable_measurement` becomes true when CV exceeds 10%. It is diagnostic-only in 0.3.1.

Planner output additionally includes `p99_regret_bytes_per_block`.

## 0.3.1 gates

- generated recall >= 0.99
- Top-K recall >= 0.98
- mean regret <= 256 B/block
- p95 regret <= 1024 B
- p99 regret <= 4096 B
- full trial encodes == 0
- BALANCED ratio >= 3.45x
- DENSE ratio >= 3.45x
- DENSE >= 99.5% hardened 0.2.1 ratio
- profile throughput >= 95% buildfix9 golden
- warm 64 KiB <= 107.5% buildfix9 golden latency
- repeated serialized output must be deterministic
