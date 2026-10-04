# Adaptive Compression Engine — ACE 0.3-buildfix7

ACE 0.3-buildfix7 is an estimator-calibration buildfix on top of 0.3-buildfix6.

## Why this buildfix exists
Buildfix6 proved that candidate generation, Top-K and QualityEnvelope were no longer the dominant problem. On numeric blocks the analytical/sample size model still predicted LZ output far too large, and BALANCED/DENSE zero-heavy blocks could bypass better entropy plans through an unconditional RLE fast path.

## Main changes
- LZ Analytical Estimator V2 aligned with the production token format;
- new analysis features: p95 match length, match coverage and long-match ratio;
- overlap-aware sampled match extension up to the codec's 130-byte match limit;
- token-cost model for LZ literals and three-byte match tokens;
- LZ-specific entropy factor based on token-stream structure instead of original H0 alone;
- lower reset-window sample authority for LZ;
- zero-heavy RLE fast path restricted to FAST;
- BALANCED/DENSE zero-heavy blocks go through the general planner;
- benchmark schema 1.7 with estimator MAE/MAPE/bias/p95 by plan family and data class;
- new LZ estimator MAPE release gates.

## Compatibility
- milestone: `0.3-buildfix7`
- workspace: `0.3.7`
- writer: Format 1.2
- reader: Format 1.0 / 1.1 / 1.2
- no wire-format change

## Build
```bash
cargo build --workspace --release
cargo test --workspace
```

## Demo
```bash
./demo/run-demo-0.3-buildfix7.sh
```

## Benchmarks
```bash
./benchmark.sh all
```

Official files use the `0.3-buildfix7-*.json` prefix.

## Release targets
```text
generated recall             >= 0.99
Top-K recall                 >= 0.98
oracle Top-2 after sample    >= 0.95
oracle Top-3 after sample    >= 0.99
quality-pool recall          >= 0.95
regret                       <= 1024 B/block
runtime full trials          == 0
LZ FAST estimator MAPE       <= 0.35
LZ BALANCED estimator MAPE   <= 0.35
BALANCED ratio               >= 3.40x
DENSE ratio                  >= 99.5% hardened baseline
FAST throughput              >= 2.0x baseline
BALANCED throughput          >= 4.0x baseline
DENSE throughput             >= 3.5x baseline
warm 64 KiB latency          <= 115% baseline
```

See `TASKS-0.3-buildfix7.md`, `MILESTONE-0.3-buildfix7.json`, `docs/ESTIMATOR-0.3-BUILDFIX7.md`, `docs/FASTPATH-QUALITY-0.3-BUILDFIX7.md` and `docs/BENCHMARK-CONTRACT-0.3-BUILDFIX7.md`.
