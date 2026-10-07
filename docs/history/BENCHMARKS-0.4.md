# ACE 0.4 benchmarks — current hardening release

The base ACE 0.4 design remains valid. The current implementation is **0.4-buildfix4 / Planner V4.3**.
See `BENCHMARKS-0.4-BUILDFIX2.md` for the route-first performance-hardening changes.

---

# ACE 0.4 benchmark contract

Benchmark schema is **2.0** because 0.4 introduces structured numeric-analysis and block-policy
telemetry in addition to the 0.3.1 families.

Existing families remain: compression, entropy, planner, parallel, random-access, streaming,
memory, corpus, block-matrix, random-access-extended and stability.

New families:

## numeric
Runs Planner V4 on u32 counter, u64 timestamps, sawtooth gauge, monotonic-with-outliers and
variable-delta workloads. Reports numeric profile, selected numeric blocks, plan distribution,
ratio and encode/decode speed.

## numeric-ablation
Compares:
1. generic 0.3.1-compatible path (`enable_numeric_specialization=false`),
2. direct Numeric codec,
3. Planner V4 selection.

## block-policy
Compares fixed 256K/512K/1M with Auto balanced/sequential/random-access policy.

Primary 0.4 gates preserve generic 0.3 quality while adding numeric ratio/selection gates. Numeric
throughput is diagnostic in the first 0.4 release because scalar bit packing is deliberately the
initial implementation.
