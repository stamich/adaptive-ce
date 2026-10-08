# Benchmark results

Generated, never committed or packaged (the release packager excludes everything here except
this file).

| File | Producer |
|---|---|
| `benchmark-0.5.0-<family>.json` | `./ace-benchmark0.5.0.sh <family>` (schema 2.1, Harness V3) |
| `benchmark-0.5.0-regression.json` | Regression V3 (`tools/ace-check_regressions0.5.0.py`) |
| `ab-0.5.0-vs-0.4.6.json` | interleaved A/B (`./ace-ab0.5.0.sh <baseline-tree>`) |

`ACE_BENCH_OUT_DIR` redirects the output. Accepted reference results are kept under
`examples/baselines/<version>/` together with a `BASELINE.json` descriptor; schema history is
described in `docs/BENCHMARK-METHODOLOGY-0.5.0.md`.
