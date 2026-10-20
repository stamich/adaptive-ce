# Benchmark results

Generated, never committed or packaged (the release packager excludes everything here except
this file).

| File | Producer |
|---|---|
| `benchmark-0.7.0-<family>.json` | `./ace-benchmark0.7.0.sh <family>` (schema 2.2, Harness V3); with `--repeat N` the median of `run-1 … run-N` |
| `run-<i>/benchmark-0.7.0-<family>.json` | the individual runs of `--repeat N` |
| `benchmark-0.7.0-repeatability.json` | repeatability gates (`tools/ace-repeatability0.7.0.py`) |
| `benchmark-0.7.0-regression.json` | Regression V3 (`tools/ace-check_regressions0.7.0.py`) |
| `ab-0.7.0-vs-0.6.1.json` | interleaved A/B (`./ace-ab0.7.0.sh <baseline-tree>`) |
| `release-checklist-0.7.0.md`, `RELEASE-REPORT.md` | `./ace-release0.7.0.sh` |

`ACE_BENCH_OUT_DIR` redirects the output. Accepted reference results are kept under
`examples/baselines/<version>/` together with a `BASELINE.json` descriptor; schema history is
described in `docs/BENCHMARKS.md`.
