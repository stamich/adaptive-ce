# ACE 0.3.1 hardening design

## Frozen algorithmic surface

The following 0.3 behavior is frozen:
- Planner V3.6;
- buildfix6 analytical estimator baseline;
- adaptive Hybrid LZ introduced in buildfix8/9;
- QualityEnvelope;
- deterministic cost model;
- Format 1.2 writer;
- 1.0/1.1/1.2 reader compatibility;
- AIDX/ACET random-access layout.

0.3.1 is therefore a reliability milestone rather than a compression-ratio redesign.

## Test layers

1. Unit tests validate parser, planner budget and data-structure invariants.
2. Property tests validate arbitrary-byte roundtrip and indexed ranges.
3. Determinism tests compare byte-identical output across profiles, block sizes and worker counts.
4. Malformed input tests exercise index/trailer corruption and DecodeLimits.
5. Streaming tests exercise declared-size and resource-limit failures.
6. Fuzzing targets cover container decode, index parse, trailer parse and indexed open.

## Release principle

Quality regressions are strict. Runtime metrics allow a small machine-noise envelope relative to the
golden buildfix9 baseline: -5% throughput and +7.5% warm-range latency. Oracle rank metrics remain
diagnostic because buildfix8/9 demonstrated that exact oracle position is weakly correlated with
actual regret.
