# Adaptive Compression Engine (ACE) 0.4-buildfix4

ACE 0.4-buildfix4 is the **Planner V4.3 Policy Oracle Closure & Runtime Regression Fix** release.

It does not add a new compression algorithm or wire format. It closes the remaining 0.4 policy
semantics and removes duplicate work from the NumericFast hot path.

## Core changes

### One planning context per block

`PlanningContext` classifies a block once. Its `RouteDecision` is reused by engine and evaluator.
A validated NumericFast route carries `NumericFastEvidence`, so the block is not validated twice.

### Strict NumericFast invariant

NumericFast now requires a complete-block fixed-step sequence:

- monotonically non-decreasing;
- non-zero first delta;
- every subsequent delta exactly equals the first delta.

Outlier/sawtooth workloads fall back to NumericGeneral.

### Direct NumericFast encode

`numeric_encode_fixed_step` writes `NUM1 + DeltaOfDelta + bit_width=0` directly from evidence.
It avoids numeric mode search, delta/DoD vectors, bit-width scan and bit packing.

### Three oracle levels

- **global oracle** — smallest physically possible diagnostic payload;
- **route oracle** — smallest route-eligible payload;
- **policy oracle** — candidate ACE should prefer under route, dominance, access and size-envelope rules.

Only policy oracle drives release recall/regret gates.

### Product dominance policy

`DominancePolicy` introduces product preferences:

- RLE preferred for zero/run-heavy blocks;
- RAW preferred for incompressible data;
- Numeric preferred for NumericFast/NumericGeneral;
- RandomAccess can prefer cheaper-decode RAW/RLE.

Preference is bounded by `DominanceEnvelope` so decode preference cannot hide an excessive size loss.

## Compatibility

- workspace version: 0.4.4
- Planner: V4.3
- writer: Format 1.3
- readers: 1.0 / 1.1 / 1.2 / 1.3
- NUM1/AIDX/ACET unchanged
- benchmark schema: 2.0

## Versioned scripts

Every executable/script starts with `ace-` and carries the current milestone in its filename:

```text
ace-build0.4-buildfix4.sh
ace-benchmark0.4-buildfix4.sh
ace-benchmark-compare0.4-buildfix4.sh
demo/ace-run-demo0.4-buildfix4.sh
tools/*0.4-buildfix4.py
```

Build/release:

```bash
./ace-build0.4-buildfix4.sh
```

Full benchmark contract:

```bash
./ace-benchmark0.4-buildfix4.sh all
```

## Benchmark JSON naming

Every benchmark baseline/result JSON has `benchmark` in its filename.

Current outputs use:

```text
examples/results/benchmark-0.4-buildfix4-<family>.json
```

New/important families:

```bash
./ace-benchmark0.4-buildfix4.sh policy-oracle-v2
./ace-benchmark0.4-buildfix4.sh planner-hotpath
./ace-benchmark0.4-buildfix4.sh random-access-plan-diff
./ace-benchmark0.4-buildfix4.sh numeric-general
./ace-benchmark0.4-buildfix4.sh numeric-fastpath
```

## Release gates

- policy candidate recall >= 0.99
- policy Top-K recall >= 0.98
- policy mean regret <= 16 B/block
- policy p95 <= 64 B
- policy p99 <= 256 B
- full candidate trials == 0
- BALANCED/DENSE ratio >= 3.70x
- FAST/BALANCED/DENSE throughput >= 95% buildfix2
- warm64K <= 110% buildfix2
- u32 NumericFast >= 95% buildfix2
- u64 timestamps >= 90 MB/s
- delta-variable >= 75 MB/s

Global and route regret remain diagnostic.

## Documentation

- `TASKS-0.4-buildfix4.md`
- `docs/ARCHITECTURE-0.4-BUILDFIX4.md`
- `docs/POLICY-ORACLE-0.4-BUILDFIX4.md`
- `docs/NUMERIC-FAST-RUNTIME-0.4-BUILDFIX4.md`
- `docs/BENCHMARKS-0.4-BUILDFIX4.md`
- `docs/SCRIPT-AUDIT-0.4-BUILDFIX4.md`
- `BUILD-AUDIT-0.4-buildfix4.md`
- `CHANGELOG.md`
- `ROADMAP.md`
